use crate::math::RotatedCircle;
use anyhow::{bail, ensure};
use glam::{Mat3, Vec3};
use lazy_regex::regex_captures;
use std::io::{BufRead, BufReader, Read};

#[derive(Debug)]
enum State {
    Default,
    Route,
}

#[derive(Debug)]
pub(crate) enum TrackShape {
    Straight {
        start: Vec3,
        end: Vec3,
    },
    Arc {
        start: Vec3,
        center: Vec3,
        end: Vec3,
        rotated_circle: RotatedCircle,
    },
    Bezier {
        start: Vec3,
        control1: Vec3,
        control2: Vec3,
        end: Vec3,
    },
}

#[derive(Debug)]
pub(crate) enum SwitchShape {
    Split {
        first_shape: TrackShape,
    }
}

impl TrackShape {
    pub(crate) fn start(&self) -> &Vec3 {
        match self {
            TrackShape::Straight { start, .. } => start,
            TrackShape::Arc { start, .. } => start,
            TrackShape::Bezier { start, .. } => start,
        }
    }

    pub(crate) fn end(&self) -> &Vec3 {
        match self {
            TrackShape::Straight { end, .. } => end,
            TrackShape::Arc { end, .. } => end,
            TrackShape::Bezier { end, .. } => end,
        }
    }

    pub(crate) fn lowest_y(&self) -> f32 {
        self.start().y.min(self.end().y)
    }
}

#[derive(Debug)]
pub(crate) struct Track {
    pub(crate) id: i32,
    pub(crate) shape: TrackShape,
}

#[derive(Debug)]
pub(crate) struct Switch {
    pub(crate) id: i32,
    pub(crate) shape: SwitchShape,
}

#[derive(Debug)]
pub(crate) struct ParseResult {
    pub(crate) tracks: Vec<Track>,
    pub(crate) switches: Vec<Switch>,
}

fn parse_position(cells: &[&str]) -> anyhow::Result<Vec3> {
    ensure!(cells.len() == 3);
    Ok(Vec3 {
        x: cells[0].parse()?,
        y: cells[1].parse()?,
        z: cells[2].parse()?,
    })
}

fn parse_transform(cells: &[&str]) -> anyhow::Result<Mat3> {
    ensure!(cells.len() == 3);
    let x_deg: f32 = cells[0].parse()?;
    let y_deg: f32 = cells[1].parse()?;
    let z_deg: f32 = cells[2].parse()?;
    let rotation =
        Mat3::from_rotation_y(y_deg.to_radians())
        * Mat3::from_rotation_z(z_deg.to_radians())
        * Mat3::from_rotation_x(x_deg.to_radians());
    Ok(rotation)
}

fn parse_normal_track(cells: &[&str]) -> anyhow::Result<Track> {
    ensure!(cells.len() >= 22);
    let rotation = parse_transform(&cells[6..9])?;
    let start = parse_position(&cells[3..6])?;
    let length: f32 = cells[9].parse()?;
    let radius: f32 = cells[10].parse()?;

    let shape = if radius == 0.0 {
        let end = start + rotation * Vec3 {
            x: 0.0,
            y: 0.0,
            z: length,
        };

        TrackShape::Straight {
            start,
            end,
        }
    } else {
        let radius_vec = Vec3 {
            x: radius,
            y: 0.0,
            z: 0.0,
        };

        let end_rotation = Mat3::from_rotation_y(-length / radius);
        let start_to_end = -radius_vec + end_rotation * radius_vec;

        let center = start - rotation * radius_vec;
        let end = start + rotation * start_to_end;

        TrackShape::Arc {
            start,
            center,
            end,
            rotated_circle: RotatedCircle::new(radius, rotation),
        }
    };

    Ok(Track {
        id: cells[1].parse()?,
        shape,
    })
}

fn parse_bezier_track(cells: &[&str]) -> anyhow::Result<Track> {
    ensure!(cells.len() >= 18);
    let start = parse_position(&cells[3..6])?;
    let start_to_control1 = parse_position(&cells[6..9])?;
    let start_to_end = parse_position(&cells[9..12])? - start;
    let end_to_control2 = parse_position(&cells[12..15])?;
    let start_to_control2 = start_to_end + end_to_control2;
    let rotation = Mat3::IDENTITY;

    Ok(Track {
        id: cells[1].parse()?,
        shape: TrackShape::Bezier {
            start,
            control1: start + rotation * start_to_control1,
            control2: start + rotation * start_to_control2,
            end: start + rotation * start_to_end,
        },
    })
}

// https://wiki.td2.info.pl/index.php?title=Scenery_format
fn parse_track(cells: &[&str]) -> anyhow::Result<Track> {
    ensure!(cells.len() >= 3);
    Ok(match cells[2] {
        "Track" => parse_normal_track(cells)?,
        "BTrack" => parse_bezier_track(cells)?,
        &_ => panic!()
    })
}

fn parse_switch(cells: &[&str]) -> anyhow::Result<Switch> {
    ensure!(cells.len() >= 19);
    let start = parse_position(&cells[3..6])?;
    let rotation = parse_transform(&cells[6..9])?;

    let Some(switch_name) = cells[2].split(',').next() else {
        bail!("Switch name is missing");
    };
    let Some(captures) = regex_captures!(r"^Rz 60E1-([\d\.]+)-1_([\d\.]+) ([LR])", switch_name)  else {
        bail!("Unknown switch type {switch_name}");
    };
    let (_, radius_str, denominator_str, direction) = captures;
    let radius = radius_str.parse::<f32>()?;
    let denominator = denominator_str.parse::<f32>()?;
    let is_left = direction == "L";
    let angle_rad = (1.0/denominator).atan();
    let curve_length = radius * angle_rad;

    let total_length = match (radius_str, denominator_str) {
        ("190", "9") => Some(27.24),
        // ("190", "1.75") => ,
        _ => None,
    };
    let extra_length = total_length.map(|total_length| total_length - curve_length);

    let end = start + rotation * (curve_length + extra_length.unwrap_or(0.0)) * Vec3::Z;

    Ok(Switch {
        id: cells[1].parse()?,
        shape: SwitchShape::Split {
            first_shape: TrackShape::Straight {
                start,
                end,
            },
        },
    })
}

pub(crate) fn parse<R: Read>(input: R) -> anyhow::Result<ParseResult> {
    let lines = BufReader::new(input).lines();

    let mut tracks: Vec<Track> = vec!();
    let mut switches: Vec<Switch> = vec!();

    let mut state = State::Default;
    lines.flatten().for_each(|line| {
        let cells: Vec<&str> = line.split(";").collect();
        let row_kind = cells[0];

        match state {
            State::Default => {
                match row_kind {
                    "Route" => {
                        state = State::Route;
                    }
                    "Track" => {
                        match parse_track(&cells) {
                            Ok(track) => tracks.push(track),
                            Err(e) => println!("Failed to parse track: {e}"),
                        }
                    }
                    "TrackStructure" => {
                        match parse_switch(&cells) {
                            Ok(switch) => switches.push(switch),
                            Err(e) => println!("Failed to parse switch: {e}"),
                        }
                    }
                    "TrackObject"| "Misc" | "Fence" | "Wires" | "TerrainPoint" | "MiscGroup" | "EndMiscGroup" | "SSPController"
                    | "SSPRepeater" | "scv029" | "shv001" | "WorldRotation" | "WorldTranslation" | "MainCamera" | "CameraHome" => {}
                    extra => {
                        println!("Unknown kind: {extra}")
                    }
                }
            }
            State::Route => {
                if row_kind == "EndRoute" {
                    state = State::Default;
                }
            }
        }
    });

    Ok(ParseResult {
        tracks,
        switches,
    })
}