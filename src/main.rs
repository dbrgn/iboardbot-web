#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate svg2polylines;
extern crate serde_json;
#[macro_use] extern crate rocket_contrib;
#[macro_use] extern crate serde_derive;

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use rocket::response::NamedFile;
use rocket_contrib::{JSON, Value};
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
    rotate_x: f64,
    rotate_y: f64,
}

#[post("/preview", format = "application/json", data = "<request>")]
fn preview(request: JSON<PreviewRequest>) -> JSON<Vec<Polyline>> {
    let polylines = svg2polylines::parse(&request.into_inner().svg).unwrap();
    JSON(polylines)
}

fn main() {
    rocket::ignite().mount("/", routes![index, files, preview]).launch();
}
