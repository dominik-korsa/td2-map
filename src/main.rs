use anyhow::bail;
use indicatif::ParallelProgressIterator;
use parse::parse;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use td2_map::parse;
use td2_map::svg::create_svg;

fn process_scenery(dir: &Path) -> anyhow::Result<()> {
    let Some(filename) = dir.file_name() else {
        bail!("Failed to get file name from path: {}", dir.display());
    };
    let name = filename.to_str().ok_or_else(|| {
        anyhow::anyhow!("Failed to convert file name to string: {}", filename.to_string_lossy())
    })?;
    let file = File::open(dir.join( format!("{name}.sc")))?;
    let parse_result = parse(file)?;
    let output_path = PathBuf::from(format!("output/{name}.svg"));
    create_svg(&parse_result, &output_path)?;
    Ok(())
}

fn main() {
    let input_dir = "/home/dkgl/Documents/TTSK/TrainDriver2/SavedStations";
    fs::create_dir_all("output").unwrap();
    let directories: Vec<PathBuf> =  fs::read_dir(input_dir).unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry
                .file_type()
                .ok()
                .is_some_and(|x| x.is_dir())
                .then(|| entry.path())
        })
        .collect();
    println!("Found {} scenery candidates", directories.len());
    directories
        .par_iter()
        .progress_count(directories.len() as u64)
        .for_each(|entry| {
            process_scenery(&entry).unwrap();
        });
}
