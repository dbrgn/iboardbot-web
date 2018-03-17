#![feature(plugin)]
#![feature(decl_macro)]
#![plugin(rocket_codegen)]

extern crate bufstream;
extern crate docopt;
extern crate scheduled_executor;
#[macro_use] extern crate log;
extern crate regex;
extern crate rocket;
extern crate rocket_contrib;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate serial;
extern crate svg2polylines;

mod robot;

use std::ffi::OsStr;
use std::fs::{File, DirEntry, read_dir};
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use std::time::Duration;

use docopt::Docopt;
use rocket::response::{NamedFile, status};
use rocket::http::Status;
use rocket::State;
use rocket_contrib::{Json, JsonValue};
use serial::BaudRate;
use svg2polylines::Polyline;

use robot::PrintTask;

type RobotQueue = Arc<Mutex<Sender<PrintTask>>>;

/// Note: This struct can be queried over HTTP,
/// so be careful with sensitive data.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Config {
    device: String,
    svg_dir: String,
    interval_seconds: u32
}

const USAGE: &'static str = "
iBoardBot Web: Cloudless drawing fun.

Usage:
    iboardbot-web [-c <configfile>]

Example:

    iboardbot-web -c config.json

Options:
    -h --help        Show this screen.
    -c <configfile>  Path to config file [default: config.json].
";

#[derive(Debug, Deserialize)]
struct Args {
    flag_c: String,
}

#[get("/")]
fn index() -> io::Result<NamedFile> {
    NamedFile::open("static/index.html")
}

#[get("/headless")]
fn headless() -> io::Result<NamedFile> {
    NamedFile::open("static/headless.html")
}

#[get("/static/<file..>")]
fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(file)).ok()
}

#[get("/config")]
fn config(config: State<Config>) -> JsonValue {
    serde_json::to_value((*config).clone())
        .expect("Could not serialize Config object")
        .into()
}

#[get("/list")]
fn list(config: State<Config>) -> Result<Json<Vec<String>>, status::Custom<Json<ErrorDetails>>> {
    let mut svg_files = read_dir(&config.svg_dir)
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
        )
        .map_err(|_e| status::Custom(
            Status::InternalServerError,
            Json(ErrorDetails {
                details: "Could not read files in SVG directory".into()
            })
        ))?;
    svg_files.sort();
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
    ScheduleCustom(Duration),
}

impl PrintMode {
    fn to_print_task(&self, polylines: Vec<Polyline>) -> PrintTask {
        match *self {
            PrintMode::Once => PrintTask::Once(polylines),
            PrintMode::Schedule5 => PrintTask::Scheduled(Duration::from_secs(5 * 60), polylines),
            PrintMode::Schedule15 => PrintTask::Scheduled(Duration::from_secs(15 * 60), polylines),
            PrintMode::Schedule30 => PrintTask::Scheduled(Duration::from_secs(30 * 60), polylines),
            PrintMode::Schedule60 => PrintTask::Scheduled(Duration::from_secs(60 * 60), polylines),
            PrintMode::ScheduleCustom(duration) => PrintTask::Scheduled(duration, polylines),
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

#[post("/preview", format = "application/json", data = "<request>")]
fn preview(request: Json<PreviewRequest>) -> Result<Json<Vec<Polyline>>, status::Custom<Json<ErrorDetails>>> {
    match svg2polylines::parse(&request.into_inner().svg) {
        Ok(polylines) => Ok(Json(polylines)),
        Err(errmsg) => Err(status::Custom(Status::BadRequest, Json(ErrorDetails { details: errmsg }))),
    }
}

/// Scale polylines.
fn scale_polylines(polylines: &mut Vec<Polyline>, offset: (f64, f64), scale: (f64, f64)) {
    println!("Scaling polylines with offset {:?} and scale {:?}", offset, scale);
    for polyline in polylines {
        for coord in polyline {
            coord.x = scale.0 * coord.x + offset.0;
            coord.y = scale.1 * coord.y + offset.1;
        }
    }
}

#[post("/print", format = "application/json", data = "<request>")]
fn print(request: Json<PrintRequest>, robot_queue: State<RobotQueue>)
        -> Result<(), status::Custom<Json<ErrorDetails>>> {
    // Parse SVG into list of polylines
    let print_request = request.into_inner();
    println!("Requested print mode: {:?}", print_request.mode);
    let mut polylines = match svg2polylines::parse(&print_request.svg) {
        Ok(polylines) => polylines,
        Err(e) => return Err(status::Custom(Status::BadRequest, Json(ErrorDetails { details: e }))),
    };

    // Scale polylines
    scale_polylines(&mut polylines,
                    (print_request.offset_x, print_request.offset_y),
                    (print_request.scale_x, print_request.scale_y));

    // Get access to queue
    let tx = match robot_queue.inner().lock() {
        Err(e) => return Err(
            status::Custom(
                Status::BadRequest,
                Json(ErrorDetails {
                    details: format!("Could not communicate with robot thread: {}", e),
                })
            )
        ),
        Ok(locked) => locked,
    };
    let task = print_request.mode.to_print_task(polylines);
    if let Err(e) = tx.send(task) {
        return Err(
            status::Custom(
                Status::InternalServerError,
                Json(ErrorDetails {
                    details: format!("Could not send print request to robot thread: {}", e),
                })
            )
        )
    };

    info!("Printing...");
    Ok(())
}

fn main() {
    // Parse args
    let args: Args = Docopt::new(USAGE)
                            .and_then(|d| d.deserialize())
                            .unwrap_or_else(|e| e.exit());

    // Parse config
    let configfile = File::open(&args.flag_c).unwrap_or_else(|e| {
        println!("Could not open configfile ({}): {}", &args.flag_c, e);
        process::exit(1);
    });
    let config: Config = serde_json::from_reader(configfile).unwrap_or_else(|e| {
        println!("Could not parse configfile ({}): {}", &args.flag_c, e);
        process::exit(1);
    });

    // Launch robot thread
    let baud_rate = BaudRate::Baud115200;
    let tx = robot::communicate(&config.device, baud_rate);

    // Initialize server state
    let robot_queue = Arc::new(Mutex::new(tx));

    // Start web server
    rocket::ignite()
        .manage(robot_queue)
        .manage(config)
        .mount("/", routes![index, headless, files, preview, print, list, config])
        .launch();
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
            PrintTask::Duration(d, p) => {
                assert_eq!(d, Duration::from_secs(60 * 5));
                assert_eq!(p, polylines);
            },
            t @ _ => panic!("Task was {:?}", t),
        }
    }
}
