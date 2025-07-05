use crate::math::RotatedCircle;
use anyhow::{bail, ensure};
use glam::{Mat3, Vec3};
use lazy_regex::regex_captures;
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

/// Arguments:
/// * `radius`: Negative radius for right curve, positive for left curve
fn build_simple_switch(
    start: Vec3,
    rotation: Mat3,
    radius: f32,
    denominator: f32,
    forced_total_length: Option<f32>,
) -> Vec<TrackShape> {
    let curve_angle = (1.0 / denominator).atan();
    let curve_length = radius.abs() * curve_angle;
    let total_length = forced_total_length.unwrap_or(curve_length);

    let mut track_shapes: Vec<TrackShape> = vec![];

    let extra_straight_length = total_length - curve_length;
    let straight_end = start + rotation * total_length * Vec3::Z;
    track_shapes.push(TrackShape::Straight {
        start,
        end: straight_end,
    });

    let circle = RotatedCircle::new(radius, rotation);
    let circle_end = circle.move_by_angle(start, curve_angle);

    if extra_straight_length > 0.1 {
        let extra_vec = extra_straight_length * circle.end_vec(curve_angle);
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

fn build_curve_switch(
    start: Vec3,
    rotation: Mat3,
    main_radius: f32,
    diverging_radius: f32,
    denominator: f32,
) -> Vec<TrackShape> {
    let diverging_angle = (1.0 / denominator).atan();
    let main_angle = (diverging_angle * diverging_radius / main_radius).abs();

    let main_circle = RotatedCircle::new(main_radius, rotation);
    let main_end = main_circle.move_by_angle(start, main_angle);

    let diverging_circle = RotatedCircle::new(diverging_radius, rotation);
    let diverging_end = diverging_circle.move_by_angle(start, diverging_angle);

    vec![
        TrackShape::Arc {
            start,
            end: main_end,
            rotated_circle: main_circle,
        },
        TrackShape::Arc {
            start,
            end: diverging_end,
            rotated_circle: diverging_circle,
        }
    ]
}

fn build_double_switch(start: Vec3, rotation: Mat3, left_track: bool, right_track: bool) -> Vec<TrackShape> {
    let radius = 190.0;
    let half_angle = (1.0f32 / 18.0).atan();
    let in_half_length = radius * half_angle;
    let out_half_length = 33.17 / 2.0;

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

    if left_track {
        let left_circle = RotatedCircle::new(-190.0, rotation);
        track_shapes.push(TrackShape::Arc {
            start: point_a_in,
            end: point_d_in,
            rotated_circle: left_circle,
        });
    }
    if right_track {
        let right_circle = RotatedCircle::new(190.0, rotation);
        track_shapes.push(TrackShape::Arc {
            start: point_b_in,
            end: point_c_in,
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
        start: point_c_in,
        end: point_c_out,
    });
    track_shapes.push(TrackShape::Straight {
        start: point_d_in,
        end: point_d_out,
    });

    track_shapes
}

fn build_crossing(start: Vec3, rotation: Mat3, half_length: f32, half_angle: f32) -> Vec<TrackShape> {
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

fn parse_switch(cells: &[&str]) -> anyhow::Result<Switch> {
    ensure!(cells.len() >= 19);
    let id = cells[1].parse()?;
    let start = parse_position(&cells[3..6])?;
    let rotation = parse_transform(&cells[6..9])?;

    let mut track_shapes: Vec<TrackShape> = vec![];

    let Some(switch_name) = cells[2].split(',').next() else {
        bail!("Switch name is missing");
    };

    if let Some(captures) = regex_captures!(r"^Rz 60E1-([\d\.]+)-1_([\d\.]+) ([LR])$", switch_name) {
        let (_, radius_str, denominator_str, direction) = captures;
        let mut radius = radius_str.parse::<f32>()?;
        let denominator = denominator_str.parse::<f32>()?;

        let forced_total_length = match (radius_str, denominator_str) {
            ("190", "9") => Some(27.24),
            _ => None,
        };

        if direction == "R" { radius *= -1.0 };

        track_shapes.extend(build_simple_switch(start, rotation, radius, denominator, forced_total_length));
    } else if let Some(captures) = regex_captures!(r"^Rlds 60E1-([\d\.]+)-([\d\.]+)-1_([\d\.]+)$", switch_name) {
        let (_, radius_str_left, radius_string_right, denominator_str) = captures;
        ensure!(radius_str_left == radius_string_right, "Left and right radius must be equal in a symmetrical switch");
        let left_radius = radius_str_left.parse::<f32>()?;
        let right_radius = -radius_string_right.parse::<f32>()?;
        let denominator = denominator_str.parse::<f32>()?;

        track_shapes.extend(build_curve_switch(start, rotation, left_radius, right_radius, denominator));
    } else if let Some(captures) = regex_captures!(r"^Rl([dj]) 60E1-([\d\.]+)_([\d\.]+)-1_([\d\.]+) ([LR])$", switch_name) {
        let (_, kind, diverging_radius, small_radius_str, denominator_str, direction) = captures;
        let main_radius = diverging_radius.parse::<f32>()?;
        let diverging_radius = small_radius_str.parse::<f32>()?;
        ensure!(main_radius >= diverging_radius);
        let denominator = denominator_str.parse::<f32>()?;

        let (main_radius, diverging_radius) = match (kind, direction) {
            ("j", "L") => (main_radius, diverging_radius),
            ("j", "R") => (-main_radius, -diverging_radius),
            ("d", "L") => (-main_radius, diverging_radius),
            ("d", "R") => (main_radius, -diverging_radius),
            _ => bail!("Unknown switch kind {kind} or direction {direction}"),
        };

        track_shapes.extend(build_curve_switch(start, rotation, main_radius, diverging_radius, denominator));
    } else if switch_name == "Rkpd 60E1-190-1_9" {
        track_shapes.extend(build_double_switch(start, rotation, true, true));
    } else if switch_name == "Rkp 60E1-190-1_9 ab" || switch_name == "Rkp 60E1-190-1_9 ba" {
        track_shapes.extend(build_double_switch(start, rotation, false, true));
    } else if switch_name == "Crossing4.444" {
        let half_angle = 0.1111;
        let half_length = 10.0;

        track_shapes.extend(build_crossing(start, rotation, half_length, half_angle));
    } else if switch_name == "Crossing" {
        let half_angle = (1.0f32 / 18.0).atan();
        let half_length = 10.0;

        track_shapes.extend(build_crossing(start, rotation, half_length, half_angle));
    } else {
        bail!("Unknown switch type {switch_name}");
    }

    Ok(Switch {
        id,
        shape: SwitchShape { track_shapes },
    })
}

pub(crate) fn parse<R: Read>(input: R) -> anyhow::Result<ParseResult> {
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
                "TrackStructure" => match parse_switch(&cells) {
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
