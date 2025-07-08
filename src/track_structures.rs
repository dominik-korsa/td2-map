use crate::track_structures::TrackStructure::{Fork, Slip};
use lazy_regex::regex_captures;
use phf::phf_map;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Debug)]
pub(crate) struct ForkSwitch {
    pub(crate) radius_left: f32,
    pub(crate) radius_right: f32,
    pub(crate) curve_length: f32,
    #[allow(dead_code)]
    pub(crate) tangent: f32,
    /// Length of the straight track added to the switch
    pub(crate) added_length: f32,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct SlipSwitch {
    pub(crate) length: f32,
    pub(crate) radius: f32,
    pub(crate) tangent: f32,
    pub(crate) left_slip: bool,
    pub(crate) right_slip: bool,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct Crossing {
    pub(crate) length: f32,
    pub(crate) tangent: f32,
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
    let mut radius_left: f32 = config.get("radius1")?.parse().ok()?;
    let mut radius_right: f32 = config.get("radius2")?.parse().ok()?;
    let curve_length: f32 = config.get("length")?.parse().ok()?;
    let added_length: f32 = config.get("addLen")?.parse().ok()?;
    let tangent: f32 = config.get("tan_alfa")?.parse().ok()?;
    if is_right {
        (radius_left, radius_right) = (-radius_right, -radius_left);
    }
    Some(ForkSwitch {
        radius_left,
        radius_right,
        curve_length,
        tangent,
        added_length,
    })
}

fn try_parse_slip_switch(config: &HashMap<String, String>) -> Option<SlipSwitch> {
    let (left_slip, right_slip) = match config.get("doubleSwitchType")?.as_str() {
        "0" => (true, true),
        "1" => (true, false),
        _ => return None,
    };
    let length: f32 = config.get("length")?.parse().ok()?;
    let radius: f32 = config.get("radius")?.parse().ok()?;
    let tangent: f32 = config.get("tangent")?.parse().ok()?;
    Some(SlipSwitch {
        radius,
        length,
        tangent,
        left_slip,
        right_slip,
    })
}

// Values for prefabs extracted from game assets, version 2025.2.3.
// `added_length` Rz 60E1-205-1_9 and Rz 60E1-265-1_10 fixed manually.
// Crossings were also added manually.
pub(crate) static TRACK_STRUCTURES: phf::Map<&'static str, TrackStructure> = phf_map! {
    "Rkp 60E1-190-1_9 ab" => Slip(SlipSwitch { length: 40.0, radius: 190.0, tangent: 9.0, left_slip: true, right_slip: false }),
    "Rkp 60E1-190-1_9 ba" => Slip(SlipSwitch { length: 40.0, radius: 190.0, tangent: 9.0, left_slip: true, right_slip: false }),
    "Rkpd 60E1-190-1_9" => Slip(SlipSwitch { length: 40.0, radius: 190.0, tangent: 9.0, left_slip: true, right_slip: true }),
    "Rld 60E1-1200_600-1_15 L" => Fork(ForkSwitch { radius_left: 600.0, radius_right: -1200.0, curve_length: 39.95566, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-1200_600-1_15 R" => Fork(ForkSwitch { radius_left: 1200.0, radius_right: -600.0, curve_length: 39.95566, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-1200_900-1_18.5 L" => Fork(ForkSwitch { radius_left: 900.0, radius_right: -1200.0, curve_length: 48.61317, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-1200_900-1_18.5 R" => Fork(ForkSwitch { radius_left: 1200.0, radius_right: -900.0, curve_length: 48.61317, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-1800_300-1_9 L" => Fork(ForkSwitch { radius_left: 300.0, radius_right: -1800.0, curve_length: 33.23108, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-1800_300-1_9 R" => Fork(ForkSwitch { radius_left: 1800.0, radius_right: -300.0, curve_length: 33.23108, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-1800_600-1_14 L" => Fork(ForkSwitch { radius_left: 600.0, radius_right: -1800.0, curve_length: 42.80262, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-1800_600-1_14 R" => Fork(ForkSwitch { radius_left: 1800.0, radius_right: -600.0, curve_length: 42.80262, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-1800_900-1_18.5 L" => Fork(ForkSwitch { radius_left: 900.0, radius_right: -1800.0, curve_length: 48.61317, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-1800_900-1_18.5 R" => Fork(ForkSwitch { radius_left: 1800.0, radius_right: -900.0, curve_length: 48.61317, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_1200-1_22 L" => Fork(ForkSwitch { radius_left: 1200.0, radius_right: -2500.0, curve_length: 54.51731, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_1200-1_22 R" => Fork(ForkSwitch { radius_left: 2500.0, radius_right: -1200.0, curve_length: 54.51731, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_250-1_8.5 L" => Fork(ForkSwitch { radius_left: 250.0, radius_right: -2500.0, curve_length: 29.31069, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_250-1_8.5 R" => Fork(ForkSwitch { radius_left: 2500.0, radius_right: -250.0, curve_length: 29.31069, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_400-1_10.5 L" => Fork(ForkSwitch { radius_left: 400.0, radius_right: -2500.0, curve_length: 38.00925, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_400-1_10.5 R" => Fork(ForkSwitch { radius_left: 2500.0, radius_right: -400.0, curve_length: 38.00925, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_600-1_14 L" => Fork(ForkSwitch { radius_left: 600.0, radius_right: -2500.0, curve_length: 42.80262, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_600-1_14 R" => Fork(ForkSwitch { radius_left: 2500.0, radius_right: -600.0, curve_length: 42.80262, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_900-1_17 L" => Fork(ForkSwitch { radius_left: 900.0, radius_right: -2500.0, curve_length: 52.14928, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-2500_900-1_17 R" => Fork(ForkSwitch { radius_left: 2500.0, radius_right: -900.0, curve_length: 52.14928, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-600_300-1_9 L" => Fork(ForkSwitch { radius_left: 300.0, radius_right: -600.0, curve_length: 29.92537, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-600_300-1_9 R" => Fork(ForkSwitch { radius_left: 600.0, radius_right: -300.0, curve_length: 29.92537, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-650_450-1_15 L" => Fork(ForkSwitch { radius_left: 450.0, radius_right: -650.0, curve_length: 29.96674, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-650_450-1_15 R" => Fork(ForkSwitch { radius_left: 650.0, radius_right: -450.0, curve_length: 29.96674, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-700_190-1_7.5 L" => Fork(ForkSwitch { radius_left: 190.0, radius_right: -700.0, curve_length: 25.22173, tangent: 9.0, added_length: 0.0 }),
    "Rld 60E1-700_190-1_7.5 R" => Fork(ForkSwitch { radius_left: 700.0, radius_right: -190.0, curve_length: 25.22173, tangent: 9.0, added_length: 0.0 }),
    "Rld 60E1-700_300-1_10 L" => Fork(ForkSwitch { radius_left: 300.0, radius_right: -700.0, curve_length: 29.38637, tangent: 9.0, added_length: 0.0 }),
    "Rld 60E1-700_300-1_10 R" => Fork(ForkSwitch { radius_left: 700.0, radius_right: -300.0, curve_length: 29.38637, tangent: 9.0, added_length: 0.0 }),
    "Rld 60E1-700_500-1_14 L" => Fork(ForkSwitch { radius_left: 500.0, radius_right: -700.0, curve_length: 35.66885, tangent: 9.0, added_length: 0.0 }),
    "Rld 60E1-700_500-1_14 R" => Fork(ForkSwitch { radius_left: 700.0, radius_right: -500.0, curve_length: 35.66885, tangent: 9.0, added_length: 0.0 }),
    "Rld 60E1-900_300-1_9 L" => Fork(ForkSwitch { radius_left: 300.0, radius_right: -900.0, curve_length: 33.23108, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-900_300-1_9 R" => Fork(ForkSwitch { radius_left: 900.0, radius_right: -300.0, curve_length: 33.23108, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-900_450-1_12 L" => Fork(ForkSwitch { radius_left: 450.0, radius_right: -900.0, curve_length: 37.43512, tangent: 9.0, added_length: 0.0 }),
    "Rld 60E1-900_450-1_12 R" => Fork(ForkSwitch { radius_left: 900.0, radius_right: -450.0, curve_length: 37.43512, tangent: 9.0, added_length: 0.0 }),
    "Rld 60E1-900_600-1_15 L" => Fork(ForkSwitch { radius_left: 600.0, radius_right: -900.0, curve_length: 39.95566, tangent: 12.0, added_length: 0.0 }),
    "Rld 60E1-900_600-1_15 R" => Fork(ForkSwitch { radius_left: 900.0, radius_right: -600.0, curve_length: 39.95566, tangent: 12.0, added_length: 0.0 }),
    "Rlds 60E1-1000-1000-1_23" => Fork(ForkSwitch { radius_left: 1000.0, radius_right: -1000.0, curve_length: 43.45773, tangent: 22.0, added_length: 0.0 }),
    "Rlds 60E1-190-190-1_9" => Fork(ForkSwitch { radius_left: 190.0, radius_right: -190.0, curve_length: 21.23, tangent: 9.0, added_length: 0.0 }),
    "Rlds 60E1-600-600-1_18.5" => Fork(ForkSwitch { radius_left: 599.9205, radius_right: -600.0, curve_length: 33.22262, tangent: 9.0, added_length: 0.0 }),
    "Rlj 60E1-1200_300-1_7 L" => Fork(ForkSwitch { radius_left: 300.0, radius_right: 1200.0, curve_length: 42.64069, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1200_300-1_7 R" => Fork(ForkSwitch { radius_left: -1200.0, radius_right: -300.0, curve_length: 42.64069, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1200_600-1_9 L" => Fork(ForkSwitch { radius_left: 600.0, radius_right: 1200.0, curve_length: 68.0, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1200_600-1_9 R" => Fork(ForkSwitch { radius_left: -1200.0, radius_right: -600.0, curve_length: 68.0, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1500_450-1_9 L" => Fork(ForkSwitch { radius_left: 450.0, radius_right: 1500.0, curve_length: 49.84663, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1500_450-1_9 R" => Fork(ForkSwitch { radius_left: -1500.0, radius_right: -450.0, curve_length: 49.84663, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1800_300-1_7.5 L" => Fork(ForkSwitch { radius_left: 300.0, radius_right: 1800.0, curve_length: 39.0, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1800_300-1_7.5 R" => Fork(ForkSwitch { radius_left: -1800.0, radius_right: -300.0, curve_length: 39.0, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1800_450-1_9 L" => Fork(ForkSwitch { radius_left: 450.0, radius_right: 1800.0, curve_length: 49.84663, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1800_450-1_9 R" => Fork(ForkSwitch { radius_left: -1800.0, radius_right: -450.0, curve_length: 49.84663, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1800_600-1_10 L" => Fork(ForkSwitch { radius_left: 599.48096, radius_right: 1800.0, curve_length: 59.85075, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-1800_600-1_10 R" => Fork(ForkSwitch { radius_left: -1800.0, radius_right: -599.48096, curve_length: 59.85075, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-600_300-1_6 L" => Fork(ForkSwitch { radius_left: 300.0, radius_right: 600.0, curve_length: 48.0, tangent: 9.0, added_length: 0.0 }),
    "Rlj 60E1-600_300-1_6 R" => Fork(ForkSwitch { radius_left: -600.0, radius_right: -300.0, curve_length: 48.0, tangent: 9.0, added_length: 0.0 }),
    "Rlj 60E1-650_190-1_20 L" => Fork(ForkSwitch { radius_left: 190.0, radius_right: 650.0, curve_length: 32.47971, tangent: 9.0, added_length: 0.0 }),
    "Rlj 60E1-650_190-1_20 R" => Fork(ForkSwitch { radius_left: -650.0, radius_right: -190.0, curve_length: 32.47971, tangent: 9.0, added_length: 0.0 }),
    "Rlj 60E1-750_190-1_6 L" => Fork(ForkSwitch { radius_left: 190.0, radius_right: 750.0, curve_length: 31.44976, tangent: 9.0, added_length: 0.0 }),
    "Rlj 60E1-750_190-1_6 R" => Fork(ForkSwitch { radius_left: -750.0, radius_right: -190.0, curve_length: 31.44976, tangent: 9.0, added_length: 0.0 }),
    "Rlj 60E1-800_250-1_6.5 L" => Fork(ForkSwitch { radius_left: 250.0, radius_right: 800.0, curve_length: 38.23661, tangent: 9.0, added_length: 0.0 }),
    "Rlj 60E1-800_250-1_6.5 R" => Fork(ForkSwitch { radius_left: -800.0, radius_right: -250.0, curve_length: 38.23661, tangent: 9.0, added_length: 0.0 }),
    "Rlj 60E1-900_300-1_7.5 L" => Fork(ForkSwitch { radius_left: 300.0, radius_right: 900.0, curve_length: 39.82378, tangent: 9.0, added_length: 0.0 }),
    "Rlj 60E1-900_300-1_7.5 R" => Fork(ForkSwitch { radius_left: -900.0, radius_right: -300.0, curve_length: 39.82378, tangent: 9.0, added_length: 0.0 }),
    "Rlj 60E1-900_300-1_9 L" => Fork(ForkSwitch { radius_left: 300.0, radius_right: 900.0, curve_length: 49.84663, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-900_300-1_9 R" => Fork(ForkSwitch { radius_left: -900.0, radius_right: -300.0, curve_length: 49.84663, tangent: 12.0, added_length: 0.0 }),
    "Rlj 60E1-900_450-1_7.5 L" => Fork(ForkSwitch { radius_left: 450.0, radius_right: 900.0, curve_length: 59.0, tangent: 9.0, added_length: 0.0 }),
    "Rlj 60E1-900_450-1_7.5 R" => Fork(ForkSwitch { radius_left: -900.0, radius_right: -450.0, curve_length: 59.0, tangent: 9.0, added_length: 0.0 }),
    "Rz 60E1-1200-1_18.5 L" => Fork(ForkSwitch { radius_left: 1200.0, radius_right: 0.0, curve_length: 64.81756, tangent: 18.5, added_length: 0.0 }),
    "Rz 60E1-1200-1_18.5 R" => Fork(ForkSwitch { radius_left: -0.0, radius_right: -1200.0, curve_length: 64.81756, tangent: 18.5, added_length: 0.0 }),
    "Rz 60E1-190-1_7.5 L" => Fork(ForkSwitch { radius_left: 190.0, radius_right: 0.0, curve_length: 25.221731, tangent: 7.5, added_length: 0.0 }),
    "Rz 60E1-190-1_7.5 R" => Fork(ForkSwitch { radius_left: -0.0, radius_right: -190.0, curve_length: 25.221731, tangent: 7.5, added_length: 0.0 }),
    "Rz 60E1-190-1_9 L" => Fork(ForkSwitch { radius_left: 190.0, radius_right: 0.0, curve_length: 21.046352, tangent: 9.0, added_length: 6.0923653 }),
    "Rz 60E1-190-1_9 R" => Fork(ForkSwitch { radius_left: -0.0, radius_right: -190.0, curve_length: 21.046352, tangent: 9.0, added_length: 6.0923653 }),
    "Rz 60E1-205-1_9 L" => Fork(ForkSwitch { radius_left: 205.0, radius_right: 0.0, curve_length: 22.707907, tangent: 9.0, added_length: 5.42 /* added manually */ }),
    "Rz 60E1-205-1_9 R" => Fork(ForkSwitch { radius_left: -0.0, radius_right: -205.0, curve_length: 22.707907, tangent: 9.0, added_length: 5.42 /* added manually */ }),
    "Rz 60E1-2500-1_26.5 L" => Fork(ForkSwitch { radius_left: 2500.0, radius_right: 0.0, curve_length: 94.30607, tangent: 26.5, added_length: 0.0 }),
    "Rz 60E1-2500-1_26.5 R" => Fork(ForkSwitch { radius_left: -0.0, radius_right: -2500.0, curve_length: 94.30607, tangent: 26.5, added_length: 0.0 }),
    "Rz 60E1-265-1_10 L" => Fork(ForkSwitch { radius_left: 265.0, radius_right: 0.0, curve_length: 26.434078, tangent: 10.0, added_length: 4.75 /* added manually */ }),
    "Rz 60E1-265-1_10 R" => Fork(ForkSwitch { radius_left: -0.0, radius_right: -265.0, curve_length: 26.434078, tangent: 10.0, added_length: 4.75 /* added manually */ }),
    "Rz 60E1-300-1_9 L" => Fork(ForkSwitch { radius_left: 300.0, radius_right: 0.0, curve_length: 33.231083, tangent: 9.0, added_length: 0.0 }),
    "Rz 60E1-300-1_9 R" => Fork(ForkSwitch { radius_left: -0.0, radius_right: -300.0, curve_length: 33.231083, tangent: 9.0, added_length: 0.0 }),
    "Rz 60E1-500-1_12 L" => Fork(ForkSwitch { radius_left: 500.0, radius_right: 0.0, curve_length: 41.59458, tangent: 12.0, added_length: 0.0 }),
    "Rz 60E1-500-1_12 R" => Fork(ForkSwitch { radius_left: -0.0, radius_right: -500.0, curve_length: 41.59458, tangent: 12.0, added_length: 0.0 }),
    "Rz 60E1-760-1_14 L" => Fork(ForkSwitch { radius_left: 760.0, radius_right: 0.0, curve_length: 54.21665, tangent: 14.0, added_length: 0.0 }),
    "Rz 60E1-760-1_14 R" => Fork(ForkSwitch { radius_left: -0.0, radius_right: -760.0, curve_length: 54.21665, tangent: 14.0, added_length: 0.0 }),
    "Crossing" => TrackStructure::Crossing(Crossing { length: 33.2294 /* added manually */, tangent: 9.0 }),
    "Crossing4.444" => TrackStructure::Crossing(Crossing { length: 20.0 /* added manually */, tangent: 4.444 }),
};