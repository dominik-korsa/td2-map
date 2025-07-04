mod parse;
mod svg;
mod math;

use crate::parse::parse;
use crate::svg::create_svg;
use std::fs::File;

fn main() {
    let src = "/home/dkgl/Documents/TTSK/TrainDriver2/SavedStations/Zwardoń/Zwardoń.sc";
    // let src = "/home/dkgl/Documents/TTSK/TrainDriver2/SavedStations/MapTest/MapTest.sc";
    let file = File::open(src).unwrap();
    let parse_result = parse(file).unwrap();
    println!("{:#?}", parse_result);
    create_svg(&parse_result).unwrap();
}
