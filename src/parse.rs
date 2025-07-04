use crate::math::get_projected_ellipse_axes;
use anyhow::ensure;
use glam::{Mat3, Vec2, Vec3};
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
        ellipse_center: Vec3,
        end: Vec3,
        radius: f32,

        projection_first_axis: Vec2,
        projection_second_axis: Vec2,
    },
    Bezier {
        start: Vec3,
        control1: Vec3,
        control2: Vec3,
        end: Vec3,
    },
}

#[derive(Debug)]
pub(crate) struct Track {
    pub(crate) id: i32,
    pub(crate) shape: TrackShape,
}

#[derive(Debug)]
pub(crate) struct ParseResult {
    pub(crate) tracks: Vec<Track>,
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

        let ellipse_center = start - rotation * radius_vec;
        let end = start + rotation * start_to_end;

        let (projection_first_axis, projection_second_axis) =
            get_projected_ellipse_axes(radius, rotation);

        TrackShape::Arc {
            start,
            ellipse_center,
            end,
            radius,
            projection_first_axis,
            projection_second_axis,
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
    // let rotation = parse_transform(&cells[15..18])?;
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
    assert!(cells.len() >= 3);
    Ok(match cells[2] {
        "Track" => parse_normal_track(cells)?,
        "BTrack" => parse_bezier_track(cells)?,
        &_ => panic!()
    })
}

pub(crate) fn parse<R: Read>(input: R) -> anyhow::Result<ParseResult> {
    let lines = BufReader::new(input).lines();

    let mut tracks: Vec<Track> = vec!();

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
                        let track = parse_track(&cells).expect("Failed to parse track");
                        tracks.push(track);
                    }
                    "TrackObject" | "TrackStructure" => {}
                    "Misc" | "Fence" | "Wires" | "TerrainPoint" | "MiscGroup" | "EndMiscGroup" | "SSPController" | "SSPRepeater"
                    | "scv029" | "shv001" | "WorldRotation" | "WorldTranslation" | "MainCamera" | "CameraHome" => {}
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
    })
}