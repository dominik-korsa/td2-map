mod parse;
mod svg;
mod math;

use crate::parse::parse;
use crate::svg::create_svg;
use std::fs;
use std::fs::File;
use std::path::PathBuf;

fn main() {
    let input_dir = "/home/dkgl/Documents/TTSK/TrainDriver2/SavedStations";
    fs::create_dir_all("output").unwrap();
    for file in fs::read_dir(input_dir).unwrap() {
        let file = file.unwrap();
        if file.file_type().ok().is_some_and(|x| x.is_dir()) {
            let name = file.file_name().to_str().unwrap().to_string();
            let file = File::open(file.path().join( format!("{name}.sc"))).unwrap();
            let parse_result = parse(file).unwrap();
            let output_path = PathBuf::from(format!("output/{name}.svg"));
            create_svg(&parse_result, &output_path).unwrap();
        }
    }
}
