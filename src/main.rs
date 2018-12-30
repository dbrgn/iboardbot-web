extern crate actix_web;
extern crate bufstream;
extern crate docopt;
extern crate futures;
extern crate scheduled_executor;
#[macro_use] extern crate log;
extern crate regex;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate serial;
extern crate simplelog;
extern crate svg2polylines;

mod robot;
mod scaling;

use std::convert::From;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{File, DirEntry, read_dir};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use std::time::Duration;
use std::thread::sleep;

use actix_web::{AsyncResponder, HttpMessage};
use actix_web::{App, HttpRequest, HttpResponse, Json, Result as ActixResult, ResponseError};
use actix_web::fs::{StaticFiles, NamedFile};
use actix_web::http::{Method, StatusCode};
use actix_web::server::HttpServer;
use docopt::Docopt;
use futures::Future;
use serial::BaudRate;
use simplelog::{TermLogger, SimpleLogger, LevelFilter, Config as LogConfig};
use svg2polylines::Polyline;

use robot::PrintTask;
use scaling::{Bounds, Range};

type RobotQueue = Arc<Mutex<Sender<PrintTask>>>;

/// The raw configuration obtained when parsing the config file.
#[derive(Debug, Deserialize, Clone)]
struct RawConfig {
    listen: Option<String>,
    device: Option<String>,
    svg_dir: Option<String>,
    static_dir: Option<String>,
    interval_seconds: Option<u64>,
}

/// Note: This struct can be queried over HTTP,
/// so be careful with sensitive data.
#[derive(Debug, Serialize, Clone)]
struct Config {
    listen: String,
    device: String,
    svg_dir: String,
    static_dir: String,
    interval_seconds: u64,
}

impl Config {
    fn from(config: &RawConfig) -> Option<Self> {
        let listen = match config.listen {
            Some(ref val) => val.clone(),
            None => "127.0.0.1:8080".to_string(),
        };
        let device = match config.device {
            Some(ref val) => val.clone(),
            None => {
                info!("Note: Config is missing device key");
                return None;
            }
        };
        let svg_dir = match config.svg_dir {
            Some(ref val) => val.clone(),
            None => {
                info!("Note: Config is missing svg_dir key");
                return None;
            }
        };
        let static_dir = match config.static_dir {
            Some(ref val) => val.clone(),
            None => "static".to_string(),
        };
        let interval_seconds = match config.interval_seconds {
            Some(val) => val,
            None => {
                info!("Note: Config is missing interval_seconds key");
                return None;
            }
        };
        Some(Self { listen, device, svg_dir, static_dir, interval_seconds })
    }
}

#[derive(Debug, Clone)]
struct PreviewConfig {
    listen: String,
    static_dir: String,
}

impl PreviewConfig {
    fn from(config: &RawConfig) -> Self {
        Self {
            listen: config.listen.clone().unwrap_or_else(|| "listen".to_string()),
            static_dir: config.static_dir.clone().unwrap_or_else(|| "static".to_string()),
        }
    }
}

/// Application state.
/// Every worker will have its own copy.
#[derive(Debug, Clone)]
struct State {
    config: Config,
    robot_queue: RobotQueue,
}

#[derive(Debug)]
enum HeadlessError {
    NoFiles,
    Io(io::Error),
    SvgParse(String),
    PolylineScale(String),
    Queue(String),
}

impl From<io::Error> for HeadlessError {
    fn from(e: io::Error) -> Self {
        HeadlessError::Io(e)
    }
}

impl fmt::Display for HeadlessError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HeadlessError::NoFiles => write!(f, "No SVG files found"),
            HeadlessError::Io(e) => write!(f, "I/O Error: {}", e),
            HeadlessError::SvgParse(e) => write!(f, "SVG Parse Error: {}", e),
            HeadlessError::PolylineScale(e) => write!(f, "Polyline Scaling Error: {}", e),
            HeadlessError::Queue(e) => write!(f, "Queue Error: {}", e),
        }
    }
}

const USAGE: &str = "
iBoardBot Web: Cloudless drawing fun.

Usage:
    iboardbot-web [-c <configfile>] [--headless]

Example:

    iboardbot-web -c config.json

Options:
    -h --help        Show this screen.
    -c <configfile>  Path to config file [default: config.json].
    --headless       Headless mode (start drawing immediately)
";

#[derive(Debug, Deserialize)]
struct Args {
    flag_c: String,
    flag_headless: bool,
}

fn index_handler_active(_req: HttpRequest<State>) -> ActixResult<NamedFile> {
    Ok(NamedFile::open("static/index.html")?)
}

fn index_handler_preview(_req: HttpRequest) -> ActixResult<NamedFile> {
    Ok(NamedFile::open("static/index-preview.html")?)
}

fn headless_handler(_req: HttpRequest<State>) -> ActixResult<NamedFile> {
    Ok(NamedFile::open("static/headless.html")?)
}

fn config_handler(req: HttpRequest<State>) -> String {
    serde_json::to_value(&req.state().config)
        .expect("Could not serialize Config object")
        .to_string()
}

/// Return a list of SVG files from the SVG dir.
fn get_svg_files(dir: &str) -> Result<Vec<String>, io::Error> {
    let mut svg_files = read_dir(dir)
        // The `read_dir` function returns an iterator over results.
        // If any iterator entry fails, fail the whole iterator.
        .and_then(|iter| iter.collect::<Result<Vec<DirEntry>, io::Error>>())
        // Filter directory entries
        .map(|entries| entries.iter()
             // Get filepath for entry
            .map(|entry| entry.path())
             // We only want files
            .filter(|path| path.is_file())
            // Map to filename
            .filter_map(|ref path| path.file_name().map(OsStr::to_os_string).and_then(|oss| oss.into_string().ok()))
            // We only want .svg files
            .filter(|filename| filename.ends_with(".svg"))
            // Collect vector of strings
            .collect::<Vec<String>>()
        )?;
    svg_files.sort();
    Ok(svg_files)
}

fn list_handler(req: HttpRequest<State>) -> Result<Json<Vec<String>>, JsonError> {
    let svg_files = get_svg_files(&req.state().config.svg_dir)
        .map_err(|_e| JsonError::ServerError(
            ErrorDetails::from("Could not read files in SVG directory")
        ))?;
    Ok(Json(svg_files))
}

#[derive(Deserialize, Debug)]
struct PreviewRequest {
    svg: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum PrintMode {
    Once,
    Schedule5,
    Schedule15,
    Schedule30,
    Schedule60,
}

impl PrintMode {
    fn to_print_task(&self, polylines: Vec<Polyline>) -> PrintTask {
        match *self {
            PrintMode::Once => PrintTask::Once(polylines),
            PrintMode::Schedule5 => PrintTask::Scheduled(Duration::from_secs(5 * 60), vec![polylines]),
            PrintMode::Schedule15 => PrintTask::Scheduled(Duration::from_secs(15 * 60), vec![polylines]),
            PrintMode::Schedule30 => PrintTask::Scheduled(Duration::from_secs(30 * 60), vec![polylines]),
            PrintMode::Schedule60 => PrintTask::Scheduled(Duration::from_secs(60 * 60), vec![polylines]),
        }
    }
}

#[derive(Deserialize, Debug)]
struct PrintRequest {
    svg: String,
    offset_x: f64,
    offset_y: f64,
    scale_x: f64,
    scale_y: f64,
    mode: PrintMode,
}

#[derive(Serialize, Debug)]
struct ErrorDetails {
    details: String,
}

impl ErrorDetails {
    fn from<S: Into<String>>(details: S) -> Self {
        ErrorDetails {
            details: details.into(),
        }
    }
}

#[derive(Debug)]
enum JsonError {
    ServerError(ErrorDetails),
    ClientError(ErrorDetails),
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let val = serde_json::to_value(match self {
            JsonError::ServerError(details) => details,
            JsonError::ClientError(details) => details,
        });
        write!(f, "{}", val.expect("Could not serialize error details"))
    }

}
impl std::error::Error for JsonError { }
impl ResponseError for JsonError {
    fn error_response(&self) -> HttpResponse {
        let mut builder = match self {
            JsonError::ServerError(_) => HttpResponse::InternalServerError(),
            JsonError::ClientError(_) => HttpResponse::BadRequest(),
        };
        builder
            .content_type("application/json")
            .body(self.to_string())
    }
}

type JsonResult<T> = Result<T, JsonError>;

fn preview_handler(req: Json<PreviewRequest>) -> JsonResult<Json<Vec<Polyline>>> {
    match svg2polylines::parse(&req.svg) {
        Ok(polylines) => Ok(Json(polylines)),
        Err(errmsg) => Err(JsonError::ClientError(ErrorDetails::from(errmsg))),
    }
}

fn print_handler(req: HttpRequest<State>) -> impl Future<Item=HttpResponse, Error=JsonError> {
    req.json()
        .map_err(|e| JsonError::ServerError(ErrorDetails::from(
            format!("Could not parse JSON payload: {}", e)
        )))
        .and_then(move |print_request: PrintRequest| {
            // Parse SVG into list of polylines
            info!("Requested print mode: {:?}", print_request.mode);
            let mut polylines = match svg2polylines::parse(&print_request.svg) {
                Ok(polylines) => polylines,
                Err(e) => return Err(JsonError::ClientError(ErrorDetails::from(e))),
            };

            // Scale polylines
            scaling::scale_polylines(
                &mut polylines,
                (print_request.offset_x, print_request.offset_y),
                (print_request.scale_x, print_request.scale_y),
            );

            // Get access to queue
            let tx = req.state().robot_queue.lock()
                .map_err(|e| JsonError::ClientError(ErrorDetails::from(
                    format!("Could not communicate with robot thread: {}", e)
                )))?;
            let task = print_request.mode.to_print_task(polylines);
            tx.send(task)
                .map_err(|e| JsonError::ServerError(ErrorDetails::from(
                    format!("Could not send print request to robot thread: {}", e)
                )))?;

            info!("Printing...");
            Ok(HttpResponse::new(StatusCode::NO_CONTENT))
        })
        .responder()
}

fn headless_start(robot_queue: RobotQueue, config: &Config) -> Result<(), HeadlessError> {
    // Get SVG files to be printed
    let svg_files = get_svg_files(&config.svg_dir)?;
    if svg_files.is_empty() {
        return Err(HeadlessError::NoFiles);
    }

    // Read SVG files
    let mut svgs = vec![];
    let base_path = Path::new(&config.svg_dir);
    for file in svg_files {
        let mut svg = String::new();
        let mut f = File::open(base_path.join(&file))?;
        f.read_to_string(&mut svg)?;
        svgs.push(svg);
    }

    // Specify target area bounds
    let mut bounds = Bounds {
        x: Range { min: 0.0, max: f64::from(robot::IBB_WIDTH) },
        y: Range { min: 0.0, max: f64::from(robot::IBB_HEIGHT) },
    };
    bounds.add_padding(5.0);

    // Parse SVG strings into lists of polylines
    let polylines_set: Vec<Vec<Polyline>> = svgs.iter()
        .map(|ref svg| {
            svg2polylines::parse(svg)
                .map_err(|e| HeadlessError::SvgParse(e))
                .and_then(|mut polylines| {
                    scaling::fit_polylines(&mut polylines, &bounds)
                        .map_err(|e| HeadlessError::PolylineScale(e))?;
                    Ok(polylines)
                })
        })
        .collect::<Result<Vec<_>, HeadlessError>>()?;

    // Get access to queue
    let tx = robot_queue
        .lock()
        .map_err(|e| HeadlessError::Queue(
            format!("Could not communicate with robot thread: {}", e)
        ))?;

    // Create print task
    let interval_duration = Duration::from_secs(config.interval_seconds);
    let task = PrintTask::Scheduled(interval_duration, polylines_set);

    // Send task to robot
    tx.send(task)
        .map_err(|e| HeadlessError::Queue(
            format!("Could not send print request to robot thread: {}", e)
        ))?;

    info!("Printing...");
    Ok(())
}

fn main() {
    // Init logger
    if let Err(_) = TermLogger::init(LevelFilter::Info, LogConfig::default()) {
        eprintln!("Could not initialize TermLogger. Falling back to SimpleLogger.");
        SimpleLogger::init(LevelFilter::Debug, LogConfig::default())
            .expect("Could not initialize SimpleLogger");
    }

    // Parse args
    let args: Args = Docopt::new(USAGE)
                            .and_then(|d| d.deserialize())
                            .unwrap_or_else(|e| e.exit());
    let headless_mode: bool = args.flag_headless;

    // Parse config
    let configfile = File::open(&args.flag_c).unwrap_or_else(|e| {
        error!("Could not open configfile ({}): {}", &args.flag_c, e);
        abort(1);
    });
    let config: RawConfig = serde_json::from_reader(configfile).unwrap_or_else(|e| {
        error!("Could not parse configfile ({}): {}", &args.flag_c, e);
        abort(1);
    });

    // Check if this is an active config
    match Config::from(&config) {
        Some(c) => main_active(c, headless_mode),
        None => main_preview(PreviewConfig::from(&config)),
    }
}

/// Start the web server in active (printing) mode.
fn main_active(config: Config, headless_mode: bool) {
    info!("Starting server in active mode (with robot attached)");

    // Check for presence of relevant paths
    let device_path = Path::new(&config.device);
    if !device_path.exists() {
        error!("Device {} does not exist", &config.device);
        abort(2);
    }
    let static_dir_path = Path::new(&config.static_dir);
    if !static_dir_path.exists() || !static_dir_path.is_dir() {
        error!("Static files dir does not exist");
        abort(2);
    }
    let svg_dir_path = Path::new(&config.svg_dir);
    if !svg_dir_path.exists() || !svg_dir_path.is_dir() {
        error!("SVG dir {} does not exist", &config.svg_dir);
        abort(2);
    }

    // Launch robot thread
    let baud_rate = BaudRate::Baud115200;
    let tx = robot::communicate(&config.device, baud_rate);

    // Initialize server state
    let robot_queue = Arc::new(Mutex::new(tx));
    let state = State {
        config: config.clone(),
        robot_queue: robot_queue.clone(),
    };

    // Print mode
    match headless_mode {
        true => info!("Starting in headless mode"),
        false => info!("Starting in normal mode"),
    };

    // If we're in headless mode, start the print jobs
    if headless_mode {
        headless_start(robot_queue.clone(), &config)
            .unwrap_or_else(|e| {
                error!("Could not start headless mode: {}", e);
                abort(3);
            });
    }

    // Start web server
    let interface = config.listen.clone();
    info!("Listening on {}", interface);
    HttpServer::new(move || {
        let mut app = App::with_state(state.clone())
            .handler("/static", StaticFiles::new("static").unwrap())
            .route("/config/", Method::GET, config_handler)
            .route("/list/", Method::GET, list_handler)
            .route("/preview/", Method::POST, preview_handler)
            .resource("/print/", |r| r.method(Method::POST).with_async(print_handler));
        if headless_mode {
            app = app.route("/", Method::GET, headless_handler);
        } else{
            app = app.route("/headless/", Method::GET, headless_handler); // For development
            app = app.route("/", Method::GET, index_handler_active);
        };
        app
    })
        .bind(interface)
        .unwrap()
        .run();
}

/// Start the web server in preview-only mode.
fn main_preview(config: PreviewConfig) {
    info!("Starting server in preview-only mode");

    // Check for presence of relevant paths
    let static_dir_path = PathBuf::from(&config.static_dir);
    if !static_dir_path.exists() || !static_dir_path.is_dir() {
        error!("Static files dir does not exist");
        abort(2);
    }

    // Start web server
    let interface = config.listen.clone();
    info!("Listening on {}", interface);
    HttpServer::new(move || {
        App::new()
            .handler("/static", StaticFiles::new(&config.static_dir).unwrap())
            .route("/preview/", Method::POST, preview_handler)
            .route("/", Method::GET, index_handler_preview)
    })
        .bind(interface)
        .unwrap()
        .run();
}

fn abort(exit_code: i32) -> ! {
    io::stdout().flush().expect("Could not flush stdout");
    io::stderr().flush().expect("Could not flush stderr");

    // No idea why this is required, but otherwise the error log doesn't show up :(
    sleep(Duration::from_millis(100));

    process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_mode_to_print_task_once() {
        let mode = PrintMode::Once;
        let polylines = vec![];
        match mode.to_print_task(polylines.clone()) {
            PrintTask::Once(p) => assert_eq!(p, polylines),
            t @ _ => panic!("Task was {:?}", t),
        }
    }

    #[test]
    fn print_mode_to_print_task_every() {
        let mode = PrintMode::Schedule5;
        let polylines = vec![];
        match mode.to_print_task(polylines.clone()) {
            PrintTask::Scheduled(d, p) => {
                assert_eq!(d, Duration::from_secs(60 * 5));
                assert_eq!(p, vec![polylines]);
            },
            t @ _ => panic!("Task was {:?}", t),
        }
    }
}
