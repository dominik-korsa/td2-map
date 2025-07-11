use crate::track_structures::TrackStructure::{Fork, Slip};
use lazy_regex::regex_captures;
use phf::phf_map;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Debug)]
pub(crate) struct ForkSwitch {
    pub(crate) radius_1: f32,
    pub(crate) radius_2: f32,
    pub(crate) curve_length: f32,
    #[allow(dead_code)]
    pub(crate) tangent_inv: f32,
    /// Length of the straight track added to the switch
    pub(crate) added_length: f32,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct SlipSwitch {
    pub(crate) total_length: f32,
    pub(crate) outer_length: f32,
    pub(crate) transition_length: f32,
    pub(crate) radius: f32,
    pub(crate) tangent_inv: f32,
    pub(crate) left_slip: bool,
    pub(crate) right_slip: bool,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct Crossing {
    pub(crate) length: f32,
    pub(crate) tangent_inv: f32,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum TrackStructure {
    Fork(ForkSwitch),
    Slip(SlipSwitch),
    Crossing(Crossing),
}

pub fn parse_track_structure_prefabs(path: &Path) -> anyhow::Result<()> {
    let mut candidates: Vec<(PathBuf, String)> = vec![];

    for entry in path.read_dir()? {
        let entry = entry?;
        if entry
            .file_type()
            .expect("Failed to get file type")
            .is_file()
        {
            if !entry.path().extension().is_some_and(|x| x == "prefab") {
                continue;
            }
            if let Some(name) = entry
                .path()
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(|name| name.to_string()) {
            candidates.push((entry.path(), name));
            }
        }
    }

    candidates.sort_by_key(|(_, name)| name.clone());

    for (path, name) in candidates {
        let is_right = name.ends_with('R');
        let prefab = parse_prefab(&path, is_right)?;
        if let Some(switch) = prefab {
            println!("\"{}\" => {:?},", name, switch);
        } else {
            println!("Failed to extract switch info from prefab {}", name);
        }
    }

    Ok(())
}

fn parse_prefab(path: &Path, is_right: bool) -> anyhow::Result<Option<TrackStructure>> {
    let reader = BufReader::new(File::open(path)?);
    let lines_iter = reader
        .lines()
        .map(|line| line.unwrap())
        .filter(|line| !line.trim().starts_with('%'));
    let mut current: Vec<String> = vec![];
    for line in lines_iter {
        if line.starts_with("--- !u!") {
            let switch = parse_prefab_component(std::mem::take(&mut current), is_right);
            if switch.is_some() {
                return Ok(switch);
            }
        } else {
            current.push(line);
        }
    }
    Ok(parse_prefab_component(current, is_right))
}

fn parse_prefab_component(lines: Vec<String>, is_right: bool) -> Option<TrackStructure> {
    let mut lines_iter = lines.into_iter().peekable();
    let first_line = lines_iter.next()?;
    if first_line != "MonoBehaviour:" {
        return None;
    }
    let mut config: HashMap<String, String> = HashMap::new();
    for line in lines_iter {
        if let Some((_, key, value)) = regex_captures!(r"^  ([a-zA-Z0-9_]+):\s*(.+)$", &line) {
            config.insert(key.to_string(), value.to_string());
        }
    }

    if let Some(switch) = try_parse_fork_switch(&config, is_right) {
        return Some(Fork(switch));
    }

    if let Some(switch) = try_parse_slip_switch(&config) {
        return Some(Slip(switch));
    }

    None
}

fn try_parse_fork_switch(config: &HashMap<String, String>, is_right: bool) -> Option<ForkSwitch> {
    let mut radius_1: f32 = config.get("radius1")?.parse().ok()?;
    let mut radius_2: f32 = config.get("radius2")?.parse().ok()?;
    let curve_length: f32 = config.get("length")?.parse().ok()?;
    let added_length: f32 = config.get("addLen")?.parse().ok()?;
    let tangent_inv: f32 = config.get("tan_alfa")?.parse().ok()?;
    if is_right {
        (radius_1, radius_2) = (-radius_1, -radius_2);
    }
    Some(ForkSwitch {
        radius_1,
        radius_2,
        curve_length,
        tangent_inv,
        added_length,
    })
}

fn try_parse_slip_switch(_config: &HashMap<String, String>) -> Option<SlipSwitch> {
    unimplemented!();

    // let (left_slip, right_slip) = match config.get("doubleSwitchType")?.as_str() {
    //     "0" => (true, true),
    //     "1" => (true, false),
    //     _ => return None,
    // };
    // let length: f32 = config.get("length")?.parse().ok()?;
    // let radius: f32 = config.get("radius")?.parse().ok()?;
    // let tangent: f32 = config.get("tangent")?.parse().ok()?;
    // Some(SlipSwitch {
    //     radius,
    //     total_length: length,
    //     tangent,
    //     left_slip,
    //     right_slip,
    // })
}

// Values for prefabs extracted from game assets, version 2025.2.3.
// `added_length` Rz 60E1-205-1_9 and Rz 60E1-265-1_10 fixed manually.
// Crossings and slip switches were also added manually.
pub(crate) static TRACK_STRUCTURES: phf::Map<&'static str, TrackStructure> = phf_map! {
    // TODO: Verify direction
    "Rkp 60E1-190-1_9 ab" => Slip(SlipSwitch { total_length: 33.165, outer_length: 6.06, transition_length: 7.461676, radius: 190.0, tangent_inv: 9.0, left_slip: false, right_slip: true }), // added manually
    "Rkp 60E1-190-1_9 ba" => Slip(SlipSwitch { total_length: 33.165, outer_length: 6.06, transition_length: 7.461676, radius: 190.0, tangent_inv: 9.0, left_slip: false, right_slip: true }), // added manually
    "Rkpd 60E1-190-1_9" => Slip(SlipSwitch { total_length: 33.165, outer_length: 6.06, transition_length: 7.461676, radius: 190.0, tangent_inv: 9.0, left_slip: true, right_slip: true }), // added manually
    "Rld 60E1-1200_600-1_15 L" => Fork(ForkSwitch { radius_1: 600.0, radius_2: -1200.0, curve_length: 39.95566, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-1200_600-1_15 R" => Fork(ForkSwitch { radius_1: -600.0, radius_2: 1200.0, curve_length: 39.95566, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-1200_900-1_18.5 L" => Fork(ForkSwitch { radius_1: 900.0, radius_2: -1200.0, curve_length: 48.61317, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-1200_900-1_18.5 R" => Fork(ForkSwitch { radius_1: -900.0, radius_2: 1200.0, curve_length: 48.61317, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-1800_300-1_9 L" => Fork(ForkSwitch { radius_1: 300.0, radius_2: -1800.0, curve_length: 33.23108, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-1800_300-1_9 R" => Fork(ForkSwitch { radius_1: -300.0, radius_2: 1800.0, curve_length: 33.23108, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-1800_600-1_14 L" => Fork(ForkSwitch { radius_1: 600.0, radius_2: -1800.0, curve_length: 42.80262, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-1800_600-1_14 R" => Fork(ForkSwitch { radius_1: -600.0, radius_2: 1800.0, curve_length: 42.80262, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-1800_900-1_18.5 L" => Fork(ForkSwitch { radius_1: 900.0, radius_2: -1800.0, curve_length: 48.61317, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-1800_900-1_18.5 R" => Fork(ForkSwitch { radius_1: -900.0, radius_2: 1800.0, curve_length: 48.61317, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_1200-1_22 L" => Fork(ForkSwitch { radius_1: 1200.0, radius_2: -2500.0, curve_length: 54.51731, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_1200-1_22 R" => Fork(ForkSwitch { radius_1: -1200.0, radius_2: 2500.0, curve_length: 54.51731, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_250-1_8.5 L" => Fork(ForkSwitch { radius_1: 250.0, radius_2: -2500.0, curve_length: 29.31069, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_250-1_8.5 R" => Fork(ForkSwitch { radius_1: -250.0, radius_2: 2500.0, curve_length: 29.31069, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_400-1_10.5 L" => Fork(ForkSwitch { radius_1: 400.0, radius_2: -2500.0, curve_length: 38.00925, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_400-1_10.5 R" => Fork(ForkSwitch { radius_1: -400.0, radius_2: 2500.0, curve_length: 38.00925, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_600-1_14 L" => Fork(ForkSwitch { radius_1: 600.0, radius_2: -2500.0, curve_length: 42.80262, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_600-1_14 R" => Fork(ForkSwitch { radius_1: -600.0, radius_2: 2500.0, curve_length: 42.80262, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_900-1_17 L" => Fork(ForkSwitch { radius_1: 900.0, radius_2: -2500.0, curve_length: 52.14928, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_900-1_17 R" => Fork(ForkSwitch { radius_1: -900.0, radius_2: 2500.0, curve_length: 52.14928, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-600_300-1_9 L" => Fork(ForkSwitch { radius_1: 300.0, radius_2: -600.0, curve_length: 29.92537, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-600_300-1_9 R" => Fork(ForkSwitch { radius_1: -300.0, radius_2: 600.0, curve_length: 29.92537, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-650_450-1_15 L" => Fork(ForkSwitch { radius_1: 450.0, radius_2: -650.0, curve_length: 29.96674, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-650_450-1_15 R" => Fork(ForkSwitch { radius_1: -450.0, radius_2: 650.0, curve_length: 29.96674, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-700_190-1_7.5 L" => Fork(ForkSwitch { radius_1: 190.0, radius_2: -700.0, curve_length: 25.22173, tangent_inv: 9.0, added_length: 0.0 }),
    "Rld 60E1-700_190-1_7.5 R" => Fork(ForkSwitch { radius_1: -190.0, radius_2: 700.0, curve_length: 25.22173, tangent_inv: 9.0, added_length: 0.0 }),
    "Rld 60E1-700_300-1_10 L" => Fork(ForkSwitch { radius_1: 300.0, radius_2: -700.0, curve_length: 29.38637, tangent_inv: 9.0, added_length: 0.0 }),
    "Rld 60E1-700_300-1_10 R" => Fork(ForkSwitch { radius_1: -300.0, radius_2: 700.0, curve_length: 29.38637, tangent_inv: 9.0, added_length: 0.0 }),
    "Rld 60E1-700_500-1_14 L" => Fork(ForkSwitch { radius_1: 500.0, radius_2: -700.0, curve_length: 35.66885, tangent_inv: 9.0, added_length: 0.0 }),
    "Rld 60E1-700_500-1_14 R" => Fork(ForkSwitch { radius_1: -500.0, radius_2: 700.0, curve_length: 35.66885, tangent_inv: 9.0, added_length: 0.0 }),
    "Rld 60E1-900_300-1_9 L" => Fork(ForkSwitch { radius_1: 300.0, radius_2: -900.0, curve_length: 33.23108, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-900_300-1_9 R" => Fork(ForkSwitch { radius_1: -300.0, radius_2: 900.0, curve_length: 33.23108, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-900_450-1_12 L" => Fork(ForkSwitch { radius_1: 450.0, radius_2: -900.0, curve_length: 37.43512, tangent_inv: 9.0, added_length: 0.0 }),
    "Rld 60E1-900_450-1_12 R" => Fork(ForkSwitch { radius_1: -450.0, radius_2: 900.0, curve_length: 37.43512, tangent_inv: 9.0, added_length: 0.0 }),
    "Rld 60E1-900_600-1_15 L" => Fork(ForkSwitch { radius_1: 600.0, radius_2: -900.0, curve_length: 39.95566, tangent_inv: 12.0, added_length: 0.0 }),
    "Rld 60E1-900_600-1_15 R" => Fork(ForkSwitch { radius_1: -600.0, radius_2: 900.0, curve_length: 39.95566, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlds 60E1-1000-1000-1_23" => Fork(ForkSwitch { radius_1: 1000.0, radius_2: -1000.0, curve_length: 43.45773, tangent_inv: 22.0, added_length: 0.0 }),
    "Rlds 60E1-190-190-1_9" => Fork(ForkSwitch { radius_1: 190.0, radius_2: -190.0, curve_length: 21.23, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlds 60E1-600-600-1_18.5" => Fork(ForkSwitch { radius_1: 599.9205, radius_2: -600.0, curve_length: 33.22262, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlj 60E1-1200_300-1_7 L" => Fork(ForkSwitch { radius_1: 300.0, radius_2: 1200.0, curve_length: 42.64069, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1200_300-1_7 R" => Fork(ForkSwitch { radius_1: -300.0, radius_2: -1200.0, curve_length: 42.64069, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1200_600-1_9 L" => Fork(ForkSwitch { radius_1: 600.0, radius_2: 1200.0, curve_length: 68.0, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1200_600-1_9 R" => Fork(ForkSwitch { radius_1: -600.0, radius_2: -1200.0, curve_length: 68.0, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1500_450-1_9 L" => Fork(ForkSwitch { radius_1: 450.0, radius_2: 1500.0, curve_length: 49.84663, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1500_450-1_9 R" => Fork(ForkSwitch { radius_1: -450.0, radius_2: -1500.0, curve_length: 49.84663, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1800_300-1_7.5 L" => Fork(ForkSwitch { radius_1: 300.0, radius_2: 1800.0, curve_length: 39.0, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1800_300-1_7.5 R" => Fork(ForkSwitch { radius_1: -300.0, radius_2: -1800.0, curve_length: 39.0, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1800_450-1_9 L" => Fork(ForkSwitch { radius_1: 450.0, radius_2: 1800.0, curve_length: 49.84663, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1800_450-1_9 R" => Fork(ForkSwitch { radius_1: -450.0, radius_2: -1800.0, curve_length: 49.84663, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1800_600-1_10 L" => Fork(ForkSwitch { radius_1: 599.48096, radius_2: 1800.0, curve_length: 59.85075, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1800_600-1_10 R" => Fork(ForkSwitch { radius_1: -599.48096, radius_2: -1800.0, curve_length: 59.85075, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-600_300-1_6 L" => Fork(ForkSwitch { radius_1: 300.0, radius_2: 600.0, curve_length: 48.0, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlj 60E1-600_300-1_6 R" => Fork(ForkSwitch { radius_1: -300.0, radius_2: -600.0, curve_length: 48.0, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlj 60E1-650_190-1_20 L" => Fork(ForkSwitch { radius_1: 190.0, radius_2: 650.0, curve_length: 32.47971, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlj 60E1-650_190-1_20 R" => Fork(ForkSwitch { radius_1: -190.0, radius_2: -650.0, curve_length: 32.47971, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlj 60E1-750_190-1_6 L" => Fork(ForkSwitch { radius_1: 190.0, radius_2: 750.0, curve_length: 31.44976, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlj 60E1-750_190-1_6 R" => Fork(ForkSwitch { radius_1: -190.0, radius_2: -750.0, curve_length: 31.44976, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlj 60E1-800_250-1_6.5 L" => Fork(ForkSwitch { radius_1: 250.0, radius_2: 800.0, curve_length: 38.23661, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlj 60E1-800_250-1_6.5 R" => Fork(ForkSwitch { radius_1: -250.0, radius_2: -800.0, curve_length: 38.23661, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlj 60E1-900_300-1_7.5 L" => Fork(ForkSwitch { radius_1: 300.0, radius_2: 900.0, curve_length: 39.82378, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlj 60E1-900_300-1_7.5 R" => Fork(ForkSwitch { radius_1: -300.0, radius_2: -900.0, curve_length: 39.82378, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlj 60E1-900_300-1_9 L" => Fork(ForkSwitch { radius_1: 300.0, radius_2: 900.0, curve_length: 49.84663, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-900_300-1_9 R" => Fork(ForkSwitch { radius_1: -300.0, radius_2: -900.0, curve_length: 49.84663, tangent_inv: 12.0, added_length: 0.0 }),
    "Rlj 60E1-900_450-1_7.5 L" => Fork(ForkSwitch { radius_1: 450.0, radius_2: 900.0, curve_length: 59.0, tangent_inv: 9.0, added_length: 0.0 }),
    "Rlj 60E1-900_450-1_7.5 R" => Fork(ForkSwitch { radius_1: -450.0, radius_2: -900.0, curve_length: 59.0, tangent_inv: 9.0, added_length: 0.0 }),
    "Rz 60E1-1200-1_18.5 L" => Fork(ForkSwitch { radius_1: 1200.0, radius_2: 0.0, curve_length: 64.81756, tangent_inv: 18.5, added_length: 0.0 }),
    "Rz 60E1-1200-1_18.5 R" => Fork(ForkSwitch { radius_1: -1200.0, radius_2: 0.0, curve_length: 64.81756, tangent_inv: 18.5, added_length: 0.0 }),
    "Rz 60E1-190-1_7.5 L" => Fork(ForkSwitch { radius_1: 190.0, radius_2: 0.0, curve_length: 25.221731, tangent_inv: 7.5, added_length: 0.0 }),
    "Rz 60E1-190-1_7.5 R" => Fork(ForkSwitch { radius_1: -190.0, radius_2: 0.0, curve_length: 25.221731, tangent_inv: 7.5, added_length: 0.0 }),
    "Rz 60E1-190-1_9 L" => Fork(ForkSwitch { radius_1: 0.0, radius_2: 190.0, curve_length: 21.046352, tangent_inv: 9.0, added_length: 6.0923653 }),
    "Rz 60E1-190-1_9 R" => Fork(ForkSwitch { radius_1: 0.0, radius_2: -190.0, curve_length: 21.046352, tangent_inv: 9.0, added_length: 6.0923653 }),
    "Rz 60E1-205-1_9 L" => Fork(ForkSwitch { radius_1: 205.0, radius_2: 0.0, curve_length: 22.707907, tangent_inv: 9.0, added_length: 5.42 /* added manually */ }),
    "Rz 60E1-205-1_9 R" => Fork(ForkSwitch { radius_1: -205.0, radius_2: 0.0, curve_length: 22.707907, tangent_inv: 9.0, added_length: 5.42 /* added manually */ }),
    "Rz 60E1-2500-1_26.5 L" => Fork(ForkSwitch { radius_1: 2500.0, radius_2: 0.0, curve_length: 94.30607, tangent_inv: 26.5, added_length: 0.0 }),
    "Rz 60E1-2500-1_26.5 R" => Fork(ForkSwitch { radius_1: -2500.0, radius_2: 0.0, curve_length: 94.30607, tangent_inv: 26.5, added_length: 0.0 }),
    "Rz 60E1-265-1_10 L" => Fork(ForkSwitch { radius_1: 0.0, radius_2: 265.0, curve_length: 26.434078, tangent_inv: 10.0, added_length: 4.75 /* added manually */ }),
    "Rz 60E1-265-1_10 R" => Fork(ForkSwitch { radius_1: 0.0, radius_2: -265.0, curve_length: 26.434078, tangent_inv: 10.0, added_length: 4.75 /* added manually */ }),
    "Rz 60E1-300-1_9 L" => Fork(ForkSwitch { radius_1: 300.0, radius_2: 0.0, curve_length: 33.231083, tangent_inv: 9.0, added_length: 0.0 }),
    "Rz 60E1-300-1_9 R" => Fork(ForkSwitch { radius_1: -300.0, radius_2: 0.0, curve_length: 33.231083, tangent_inv: 9.0, added_length: 0.0 }),
    "Rz 60E1-500-1_12 L" => Fork(ForkSwitch { radius_1: 500.0, radius_2: 0.0, curve_length: 41.59458, tangent_inv: 12.0, added_length: 0.0 }),
    "Rz 60E1-500-1_12 R" => Fork(ForkSwitch { radius_1: -500.0, radius_2: 0.0, curve_length: 41.59458, tangent_inv: 12.0, added_length: 0.0 }),
    "Rz 60E1-760-1_14 L" => Fork(ForkSwitch { radius_1: 760.0, radius_2: 0.0, curve_length: 54.21665, tangent_inv: 14.0, added_length: 0.0 }),
    "Rz 60E1-760-1_14 R" => Fork(ForkSwitch { radius_1: -760.0, radius_2: 0.0, curve_length: 54.21665, tangent_inv: 14.0, added_length: 0.0 }),
    "Crossing" => TrackStructure::Crossing(Crossing { length: 33.2294 /* added manually */, tangent_inv: 9.0 }),
    "Crossing4.444" => TrackStructure::Crossing(Crossing { length: 20.0 /* added manually */, tangent_inv: 4.444 }),
};