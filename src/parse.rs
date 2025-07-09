use crate::math::RotatedCircle;
use crate::track_structures::{Crossing, ForkSwitch, SlipSwitch, TrackStructure, TRACK_STRUCTURES};
use anyhow::{bail, ensure};
use glam::{Mat3, Vec3};
use std::io::{BufRead, BufReader, Read};

#[derive(Debug)]
enum State {
    Default,
    Route,
    TerrainGroup,
}

#[derive(Debug)]
pub(crate) enum TrackShape {
    Straight {
        start: Vec3,
        end: Vec3,
    },
    Arc {
        start: Vec3,
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
pub(crate) struct SwitchShape {
    pub(crate) track_shapes: Vec<TrackShape>,
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
pub struct Track {
    pub id: i32,
    pub(crate) shape: TrackShape,
}

#[derive(Debug)]
pub struct Switch {
    pub id: i32,
    pub(crate) shape: SwitchShape,
}

#[derive(Debug)]
pub struct ParseResult {
    pub tracks: Vec<Track>,
    pub switches: Vec<Switch>,
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
    let rotation = Mat3::from_rotation_y(y_deg.to_radians())
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
        let end = start + rotation * length * Vec3::Z;

        TrackShape::Straight { start, end }
    } else {
        let circle = RotatedCircle::new(radius, rotation);
        let angle = length / radius.abs();
        let end = circle.move_by_angle(start, angle);

        TrackShape::Arc {
            start,
            end,
            rotated_circle: circle,
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
        &_ => panic!(),
    })
}

fn build_fork_half(
    start: Vec3,
    rotation: Mat3,
    radius: f32,
    curve_length: f32,
    added_length: f32,
) -> Vec<TrackShape> {
    if radius == 0.0 {
        return vec![TrackShape::Straight {
            start,
            end: start + rotation * (curve_length + added_length) * Vec3::Z,
        }];
    }

    let angle = curve_length / radius.abs();
    let circle = RotatedCircle::new(radius, rotation);
    let circle_end = circle.move_by_angle(start, angle);

    let mut track_shapes: Vec<TrackShape> = vec![

    ];
    if added_length > 0.0 {
        let extra_vec = added_length * circle.end_vec(angle);
        let extra_straight_end = circle_end + extra_vec;
        track_shapes.push(TrackShape::Straight {
            start: circle_end,
            end: extra_straight_end,
        });
    }

    track_shapes.push(TrackShape::Arc {
        start,
        end: circle_end,
        rotated_circle: circle,
    });

    track_shapes
}

fn build_fork_switch(
    start: Vec3,
    rotation: Mat3,
    fork: &ForkSwitch,
) -> Vec<TrackShape> {
    let mut left = build_fork_half(start, rotation, fork.radius_left, fork.curve_length, fork.added_length);
    let right = build_fork_half(start, rotation, fork.radius_right, fork.curve_length, fork.added_length);
    left.extend(right);
    left
}

fn build_slip_switch(start: Vec3, rotation: Mat3, slip: &SlipSwitch) -> Vec<TrackShape> {
    let half_angle = (1.0 / slip.tangent / 2.0).atan();
    let in_half_length = slip.radius * half_angle;
    let out_half_length = slip.length / 2.0;

    let unit_vec_left = Mat3::from_rotation_y(-half_angle) * Vec3::Z;
    let unit_vec_right = Mat3::from_rotation_y(half_angle) * Vec3::Z;

    let point_a_in = start + rotation * in_half_length * unit_vec_left;
    let point_b_in = start + rotation * in_half_length * unit_vec_right;
    let point_c_in = start + rotation * (in_half_length * -unit_vec_left);
    let point_d_in = start + rotation * (in_half_length * -unit_vec_right);

    let point_a_out = start + rotation * out_half_length * unit_vec_left;
    let point_b_out = start + rotation * out_half_length * unit_vec_right;
    let point_c_out = start + rotation * (out_half_length * -unit_vec_left);
    let point_d_out = start + rotation * (out_half_length * -unit_vec_right);

    let mut track_shapes: Vec<TrackShape> = vec![];

    track_shapes.push(TrackShape::Straight {
        start: point_a_in,
        end: point_c_in,
    });
    track_shapes.push(TrackShape::Straight {
        start: point_b_in,
        end: point_d_in,
    });

    if slip.left_slip {
        let left_circle = RotatedCircle::new(190.0, rotation);
        track_shapes.push(TrackShape::Arc {
            start: point_b_in,
            end: point_c_in,
            rotated_circle: left_circle,
        });
    }
    if slip.right_slip {
        let right_circle = RotatedCircle::new(-190.0, rotation);
        track_shapes.push(TrackShape::Arc {
            start: point_a_in,
            end: point_d_in,
            rotated_circle: right_circle,
        });
    }

    track_shapes.push(TrackShape::Straight {
        start: point_a_out,
        end: point_a_in,
    });
    track_shapes.push(TrackShape::Straight {
        start: point_b_out,
        end: point_b_in,
    });
    track_shapes.push(TrackShape::Straight {
        start: point_c_out,
        end: point_c_in,
    });
    track_shapes.push(TrackShape::Straight {
        start: point_d_out,
        end: point_d_in,
    });

    track_shapes
}

fn build_crossing(start: Vec3, rotation: Mat3, crossing: &Crossing) -> Vec<TrackShape> {
    let half_angle = (1.0 / crossing.tangent).atan() / 2.0;
    let half_length = crossing.length / 2.0;

    let unit_vec_left = Mat3::from_rotation_y(-half_angle) * Vec3::Z;
    let unit_vec_right = Mat3::from_rotation_y(half_angle) * Vec3::Z;

    let point_a = start + rotation * half_length * unit_vec_left;
    let point_b = start + rotation * half_length * unit_vec_right;
    let point_c = start + rotation * (half_length * -unit_vec_left);
    let point_d = start + rotation * (half_length * -unit_vec_right);

    vec![
        TrackShape::Straight {
            start: point_a,
            end: point_c,
        },
        TrackShape::Straight {
            start: point_b,
            end: point_d,
        },
    ]
}

fn parse_track_structure(cells: &[&str]) -> anyhow::Result<Switch> {
    ensure!(cells.len() >= 19);
    let id = cells[1].parse()?;
    let start = parse_position(&cells[3..6])?;
    let rotation = parse_transform(&cells[6..9])?;

    let mut track_shapes: Vec<TrackShape> = vec![];

    let Some(structure_name) = cells[2].split(',').next() else {
        bail!("Track structure name is missing");
    };

    if let Some(track_structure) = TRACK_STRUCTURES.get(structure_name) {
        let shapes = match track_structure {
            TrackStructure::Fork(fork) => build_fork_switch(start, rotation, fork),
            TrackStructure::Slip(slip) => build_slip_switch(start, rotation, slip),
            TrackStructure::Crossing(crossing) => build_crossing(start, rotation, crossing),
        };
        track_shapes.extend(shapes);
    } else {
        bail!("Unknown switch type {structure_name}");
    }

    Ok(Switch {
        id,
        shape: SwitchShape { track_shapes },
    })
}

pub fn parse<R: Read>(input: R) -> anyhow::Result<ParseResult> {
    let lines = BufReader::new(input).lines();

    let mut tracks: Vec<Track> = vec![];
    let mut switches: Vec<Switch> = vec![];

    let mut state = State::Default;
    lines.flatten().for_each(|line| {
        if line.is_empty() {
            return;
        }

        let cells: Vec<&str> = line.split(";").collect();
        let row_kind = cells[0];

        match state {
            State::Default => match row_kind {
                "Route" => {
                    state = State::Route;
                }
                "TerrainGroup" => {
                    state = State::TerrainGroup;
                }
                "Track" => match parse_track(&cells) {
                    Ok(track) => tracks.push(track),
                    Err(e) => println!("Failed to parse track: {e}"),
                },
                "TrackStructure" => match parse_track_structure(&cells) {
                    Ok(switch) => switches.push(switch),
                    Err(e) => println!("Failed to parse switch: {e}"),
                },
                "TrackObject" | "Misc" | "Fence" | "Wires" | "TerrainPoint" | "MiscGroup"
                | "EndMiscGroup" | "SSPController" | "SSPRepeater" | "scv029" | "shv001"
                | "WorldRotation" | "WorldTranslation" | "MainCamera" | "CameraHome" => {},
                extra => {
                    println!("Unknown kind: {extra}")
                }
            },
            State::Route => {
                if row_kind == "EndRoute" {
                    state = State::Default;
                }
            },
            State::TerrainGroup => {
                if row_kind == "EndTerrainGroup" {
                    state = State::Default;
                }
            }
        }
    });

    Ok(ParseResult { tracks, switches })
}
