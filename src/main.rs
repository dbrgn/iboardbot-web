#![feature(plugin)]
#![feature(decl_macro)]
#![plugin(rocket_codegen)]

extern crate bufstream;
extern crate docopt;
extern crate job_scheduler;
#[macro_use] extern crate log;
extern crate regex;
extern crate rocket;
extern crate rocket_contrib;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate serial;
extern crate svg2polylines;

mod robot;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;

use docopt::Docopt;
use rocket::response::{NamedFile, status};
use rocket::http::Status;
use rocket::State;
use rocket_contrib::Json;
use serial::BaudRate;
use svg2polylines::Polyline;

use robot::PrintTask;

type RobotQueue = Arc<Mutex<Sender<PrintTask>>>;

const USAGE: &'static str = "
iBoardBot Web: Cloudless drawing fun.

Usage:
    iboardbot-web <device>

Example:

    iboardbot-web /dev/ttyACM0

Options:
    -h --help  Show this screen.
";

#[derive(Debug, Deserialize)]
struct Args {
    arg_device: String,
}

#[get("/")]
fn index() -> io::Result<NamedFile> {
    NamedFile::open("static/index.html")
}

#[get("/static/<file..>")]
fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(file)).ok()
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
            PrintMode::Schedule5 => PrintTask::Every5Min(polylines),
            PrintMode::Schedule15 => PrintTask::Every15Min(polylines),
            PrintMode::Schedule30 => PrintTask::Every30Min(polylines),
            PrintMode::Schedule60 => PrintTask::Every60Min(polylines),
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

    // Launch robot thread
    let baud_rate = BaudRate::Baud115200;
    let tx = robot::communicate(&args.arg_device, baud_rate);

    // Initialize server state
    let robot_queue = Arc::new(Mutex::new(tx));

    // Start web server
    rocket::ignite()
        .manage(robot_queue)
        .mount("/", routes![index, files, preview, print])
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
            PrintTask::Every5Min(p) => assert_eq!(p, polylines),
            t @ _ => panic!("Task was {:?}", t),
        }
    }
}
