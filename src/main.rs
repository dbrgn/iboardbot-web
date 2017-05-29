#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate bufstream;
#[macro_use] extern crate log;
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

use rocket::response::{NamedFile, status};
use rocket::http::Status;
use rocket::State;
use rocket_contrib::JSON;
use serial::BaudRate;
use svg2polylines::Polyline;

type RobotQueue = Arc<Mutex<Sender<Vec<Polyline>>>>;

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
struct PrintRequest {
    svg: String,
    offset_x: f64,
    offset_y: f64,
    scale_x: f64,
    scale_y: f64,
}

#[derive(Serialize, Debug)]
struct ErrorDetails {
    details: String,
}

#[post("/preview", format = "application/json", data = "<request>")]
fn preview(request: JSON<PreviewRequest>) -> Result<JSON<Vec<Polyline>>, status::Custom<JSON<ErrorDetails>>> {
    match svg2polylines::parse(&request.into_inner().svg) {
        Ok(polylines) => Ok(JSON(polylines)),
        Err(errmsg) => Err(status::Custom(Status::BadRequest, JSON(ErrorDetails { details: errmsg }))),
    }
}

#[post("/print", format = "application/json", data = "<request>")]
fn print(request: JSON<PrintRequest>, robot_queue: State<RobotQueue>)
        -> Result<(), status::Custom<JSON<ErrorDetails>>> {
    // Parse SVG into list of polylines
    let print_request = request.into_inner();
    let polylines = match svg2polylines::parse(&print_request.svg) {
        Ok(polylines) => polylines,
        Err(e) => return Err(status::Custom(Status::BadRequest, JSON(ErrorDetails { details: e }))),
    };

    // Get access to queue
    let tx = match robot_queue.inner().lock() {
        Err(e) => return Err(
            status::Custom(
                Status::BadRequest,
                JSON(ErrorDetails {
                    details: format!("Could not communicate with robot thread: {}", e),
                })
            )
        ),
        Ok(locked) => locked,
    };
    if let Err(e) = tx.send(polylines) {
        return Err(
            status::Custom(
                Status::InternalServerError,
                JSON(ErrorDetails {
                    details: format!("Could not send print request to robot thread: {}", e),
                })
            )
        )
    };

    info!("Printing...");
    Ok(())
}

fn main() {
    // Launch robot thread
    let device = "/dev/ttyACM1";
    let baud_rate = BaudRate::Baud115200;
    let tx = robot::communicate(&device, baud_rate);

    // Initialize server state
    let robot_queue = Arc::new(Mutex::new(tx));

    // Start web server
    rocket::ignite()
        .manage(robot_queue)
        .mount("/", routes![index, files, preview, print])
        .launch();
}
