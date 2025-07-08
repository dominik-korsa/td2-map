use std::env;
use std::path::Path;
use td2_map::track_structures::parse_track_structure_prefabs;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = &args.get(1).expect("Missing path argument");
    parse_track_structure_prefabs(Path::new(path)).unwrap();
}