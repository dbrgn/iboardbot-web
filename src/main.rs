#![feature(plugin)]
#![plugin(rocket_codegen)]

#[macro_use] extern crate log;
extern crate rocket;
extern crate rocket_contrib;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate svg2polylines;

mod robot;

use std::io;
use std::path::{Path, PathBuf};

use rocket::response::{NamedFile, status};
use rocket::http::Status;
use rocket_contrib::JSON;
use svg2polylines::Polyline;

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
fn print(request: JSON<PrintRequest>) -> Result<(), status::Custom<JSON<ErrorDetails>>> {
    let print_request = request.into_inner();
    let polylines = match svg2polylines::parse(&print_request.svg) {
        Ok(polylines) => polylines,
        Err(errmsg) => return Err(status::Custom(Status::BadRequest, JSON(ErrorDetails { details: errmsg }))),
    };
    info!("Printing...");
    robot::print_polylines(&polylines);
    Ok(())
}

fn main() {
    rocket::ignite().mount("/", routes![index, files, preview, print]).launch();
}
