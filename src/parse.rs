use crate::math::RotatedCircle;
use crate::track_structures::{Crossing, ForkSwitch, SlipSwitch, TrackStructure, TRACK_STRUCTURES};
use anyhow::{bail, ensure};
use glam::{Mat3, Vec3, Vec3Swizzles};
use lazy_regex::regex_captures;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::mem::swap;

#[derive(Debug)]
enum State {
    Default,
    Route,
    TerrainGroup,
}

/// Struct representing a point with a rotation
#[derive(Debug, Copy, Clone)]
pub(crate) struct Checkpoint {
    pub(crate) pos: Vec3,
    pub(crate) rotation: Mat3,
}

impl Checkpoint {
    fn new(pos: Vec3, rotation: Mat3) -> Self {
        Checkpoint { pos, rotation }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum TrackShape {
    Straight {
        start: Checkpoint,
        end_pos: Vec3,
        length: f32,
    },
    Arc {
        start_pos: Vec3,
        end: Checkpoint,
        length: f32,
        angle: f32,
        rotated_circle: RotatedCircle,
    },
    Bezier {
        start_pos: Vec3,
        control1: Vec3,
        control2: Vec3,
        end_pos: Vec3,
    },
    Point(Checkpoint),
}

impl TrackShape {
    pub(crate) fn straight(start: Checkpoint, length: f32) -> Self {
        assert!(length >= 0.0, "Length must be non-negative");
        let end_pos = start.pos + start.rotation * length * Vec3::Z;
        TrackShape::Straight { start, end_pos, length }
    }
    pub(crate) fn straight_around_point(point: Checkpoint, start_offset: f32, end_offset: f32) -> Self {
        let length = end_offset - start_offset;
        assert!(length >= 0.0, "end_offset must be greater than start_offset");
        let start_pos = point.pos + point.rotation * start_offset * Vec3::Z;
        let end_pos = point.pos + point.rotation * end_offset * Vec3::Z;
        TrackShape::Straight {
            start: Checkpoint { pos: start_pos, rotation: point.rotation },
            end_pos,
            length: start_offset,
        }
    }

    fn arc(start: Checkpoint, radius: f32, angle: f32, length: f32) -> Self {
        let rotated_circle = RotatedCircle::new(radius, start.rotation);
        let end = rotated_circle.move_by_angle(start.pos, angle);
        TrackShape::Arc {
            start_pos: start.pos,
            end,
            angle,
            length,
            rotated_circle,
        }
    }

    pub(crate) fn arc_or_straight(start: Checkpoint, radius: f32, length: f32) -> Self {
        if radius == 0.0 {
            return TrackShape::straight(start, length);
        }
        let angle = length / radius.abs();
        TrackShape::arc(start, radius, angle, length)
    }

    pub(crate) fn point(point: Checkpoint) -> Self {
        TrackShape::Point(point)
    }

    pub(crate) fn start(&self) -> Checkpoint {
        match self {
            TrackShape::Straight { start, .. } => *start,
            TrackShape::Arc { start_pos, rotated_circle, .. } => {
                Checkpoint { pos: *start_pos, rotation: rotated_circle.start_rotation() }
            },
            TrackShape::Bezier { start_pos: start, .. } => {
                // TODO
                Checkpoint { pos: *start, rotation: Mat3::IDENTITY }
            },
            TrackShape::Point(point) => *point,
        }
    }

    pub(crate) fn end(&self) -> Checkpoint {
        match self {
            TrackShape::Straight { start, end_pos, .. } => Checkpoint { pos: *end_pos, rotation: start.rotation },
            TrackShape::Arc { start_pos, rotated_circle, angle, .. } => {
                rotated_circle.move_by_angle(*start_pos, *angle)
            },
            TrackShape::Bezier { end_pos, .. } => {
                // TODO
                Checkpoint { pos: *end_pos, rotation: Mat3::IDENTITY }
            }
            TrackShape::Point(point) => *point,
        }
    }

    pub(crate) fn lowest_y(&self) -> f32 {
        self.start().pos.y.min(self.end().pos.y)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct TrackIds {
    pub own: i32,
    pub(crate) prev: Option<i32>,
    pub(crate) next: Option<i32>,
}

impl TrackIds {
    pub(crate) fn with_prev(mut self, id: i32) -> Self {
        self.prev = Some(id);
        self
    }

    pub(crate) fn with_next(mut self, id: i32) -> Self {
        self.next = Some(id);
        self
    }

    pub(crate) fn parse(own: &str, prev: &str, next: &str) -> anyhow::Result<Self> {
        let own = own.parse()?;
        let prev = if prev.is_empty() { None } else { Some(prev.parse()?) };
        let next = if next.is_empty() { None } else { Some(next.parse()?) };
        Ok(TrackIds { own, prev, next })
    }
}

impl TrackIds {
    #[deprecated(note = "Temporary solution")]
    pub(crate) fn placeholder() -> Self {
        TrackIds {
            own: -1,
            prev: None,
            next: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Track {
    pub ids: TrackIds,
    pub(crate) shape: TrackShape,
    pub(crate) end_for_structure: Option<String>,
}

impl Track {
    pub(crate) fn new(ids: TrackIds, shape: TrackShape) -> Self {
        Track { ids, shape, end_for_structure: None }
    }

    pub(crate) fn new_structure_end(ids: TrackIds, shape: TrackShape, end_for_structure: String) -> Self {
        Track { ids, shape, end_for_structure: Some(end_for_structure) }
    }
}

#[derive(Debug)]
pub struct Switch {
    pub id: i32,
    pub(crate) tracks: Vec<Track>,
}

#[derive(Debug)]
pub struct FailedConnection {
    pub pos1: Vec3,
    pub pos2: Vec3,
    pub track1: Track,
    pub track2: Track,
}

#[derive(Debug)]
pub struct ParseResult {
    pub tracks: Vec<Track>,
    pub track_indexes: HashMap<i32, usize>,
    pub failed_connections: Vec<FailedConnection>,
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
    let ids = TrackIds::parse(cells[1], cells[12], cells[11])?;

    let start = Checkpoint::new(
        parse_position(&cells[3..6])?,
        parse_transform(&cells[6..9])?,
    );
    let length: f32 = cells[9].parse()?;
    let radius: f32 = cells[10].parse()?;

    let shape = TrackShape::arc_or_straight(start, radius, length);
    Ok(Track::new(ids, shape))
}

fn parse_bezier_track(cells: &[&str]) -> anyhow::Result<Track> {
    ensure!(cells.len() >= 18);

    let ids = TrackIds::parse(cells[1], cells[16], cells[15])?;

    let start_pos = parse_position(&cells[3..6])?;
    let start_to_control1 = parse_position(&cells[6..9])?;
    let start_to_end = parse_position(&cells[9..12])? - start_pos;
    let end_to_control2 = parse_position(&cells[12..15])?;
    let start_to_control2 = start_to_end + end_to_control2;
    let rotation = Mat3::IDENTITY;

    let shape = TrackShape::Bezier {
        start_pos,
        control1: start_pos + rotation * start_to_control1,
        control2: start_pos + rotation * start_to_control2,
        end_pos: start_pos + rotation * start_to_end,
    };
    Ok(Track::new(ids, shape))
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

fn build_fork_switch(
    start: Checkpoint,
    fork: &ForkSwitch,
    subtracks: Vec<TrackIds>,
    structure_name: &str,
) -> anyhow::Result<Vec<Track>> {
    ensure!(subtracks.len() >= 5, "Fork switch must have at least 5 subtracks");
    let [start_id, first_curve_id, second_curve_id] = subtracks[0..3] else {
        panic!("Failed to match subtrack IDs");
    };
    let (first_end_id, second_end_id, extra_ids) = if fork.added_length > 0.0 {
        ensure!(subtracks.len() == 7, "Fork switch with added length must have exactly 7 subtracks");
        let [first_extra_id, second_extra_id, first_end_id, second_end_id] = subtracks[3..7] else {
            panic!("Failed to match subtrack IDs");
        };
        (first_end_id, second_end_id, Some((first_extra_id, second_extra_id)))
    } else {
        ensure!(subtracks.len() == 5, "Fork switch without added length must have exactly 5 subtracks");
        let [second_end_id, first_end_id] = subtracks[3..5] else {
            panic!("Failed to match subtrack IDs");
        };
        (first_end_id, second_end_id, None)
    };

    let (first_after_curve_id, second_after_curve_id) = if let Some((first_extra_id, second_extra_id)) = extra_ids {
        (first_extra_id.own, second_extra_id.own)
    } else {
        (first_end_id.own, second_end_id.own)
    };

    let start_shape = TrackShape::point(start);

    let first_curve_shape = TrackShape::arc_or_straight(start, fork.radius_1, fork.curve_length);
    let mut first_current_end = first_curve_shape.end();
    let mut first_current_end_id = first_curve_id.own;

    let second_curve_shape = TrackShape::arc_or_straight(start, fork.radius_2, fork.curve_length);
    let mut second_current_end = second_curve_shape.end();
    let mut second_current_end_id = second_curve_id.own;

    let mut tracks: Vec<Track> = vec![
        Track::new(start_id, start_shape),
        Track::new(first_curve_id.with_prev(start_id.own).with_next(first_after_curve_id), first_curve_shape),
        Track::new(second_curve_id.with_prev(start_id.own).with_next(second_after_curve_id), second_curve_shape),
    ];

    if let Some((first_extra_id, second_extra_id)) = extra_ids {
        let first_extra_shape = TrackShape::straight(first_current_end, fork.added_length);
        let second_extra_shape = TrackShape::straight(second_current_end, fork.added_length);

        first_current_end = first_extra_shape.end();
        second_current_end = second_extra_shape.end();

        tracks.push(Track::new(first_extra_id.with_prev(first_current_end_id).with_next(first_end_id.own), first_extra_shape));
        tracks.push(Track::new(second_extra_id.with_prev(second_current_end_id).with_next(second_end_id.own), second_extra_shape));

        first_current_end_id = first_extra_id.own;
        second_current_end_id = second_extra_id.own;
    }

    tracks.push(
        Track::new_structure_end(first_end_id.with_prev(first_current_end_id), TrackShape::point(first_current_end), structure_name.to_string()),
    );
    tracks.push(
        Track::new_structure_end(second_end_id.with_prev(second_current_end_id), TrackShape::point(second_current_end), structure_name.to_string()),
    );

    Ok(tracks)
}

fn build_slip_switch(start: Checkpoint, slip: &SlipSwitch, structure_name: &str) -> anyhow::Result<Vec<Track>> {
    let half_angle = (1.0 / slip.tangent).atan() / 2.0;
    let in_half_length = slip.radius * half_angle;
    let out_half_length = slip.length / 2.0;

    // let left_rotation = rotation * Mat3::from_rotation_y(-half_angle);
    // let right_rotation = rotation * Mat3::from_rotation_y(half_angle);
    //
    // let point_a_in = start + rotation * in_half_length * unit_vec_left;
    // let point_b_in = start + rotation * in_half_length * unit_vec_right;
    // let point_c_in = start + rotation * (in_half_length * -unit_vec_left);
    // let point_d_in = start + rotation * (in_half_length * -unit_vec_right);
    //
    // let point_a_out = start + rotation * out_half_length * unit_vec_left;
    // let point_b_out = start + rotation * out_half_length * unit_vec_right;
    // let point_c_out = start + rotation * (out_half_length * -unit_vec_left);
    // let point_d_out = start + rotation * (out_half_length * -unit_vec_right);
    //
    // let mut track_shapes: Vec<TrackShape> = vec![];
    //
    // track_shapes.push(TrackShape::Straight {
    //     start: point_a_in,
    //     end: point_c_in,
    // });
    // track_shapes.push(TrackShape::Straight {
    //     start: point_b_in,
    //     end: point_d_in,
    // });
    //
    // if slip.left_slip {
    //     let left_circle = RotatedCircle::new(190.0, rotation);
    //     track_shapes.push(TrackShape::Arc {
    //         start: point_b_in,
    //         end: point_c_in,
    //         rotated_circle: left_circle,
    //     });
    // }
    // if slip.right_slip {
    //     let right_circle = RotatedCircle::new(-190.0, rotation);
    //     track_shapes.push(TrackShape::Arc {
    //         start: point_a_in,
    //         end: point_d_in,
    //         rotated_circle: right_circle,
    //     });
    // }
    //
    // track_shapes.push(TrackShape::Straight {
    //     start: point_a_out,
    //     end: point_a_in,
    // });
    // track_shapes.push(TrackShape::Straight {
    //     start: point_b_out,
    //     end: point_b_in,
    // });
    // track_shapes.push(TrackShape::Straight {
    //     start: point_c_out,
    //     end: point_c_in,
    // });
    // track_shapes.push(TrackShape::Straight {
    //     start: point_d_out,
    //     end: point_d_in,
    // });
    //
    // Ok(track_shapes.into_iter().map(|shape| Track::new(
    //     TrackIds::placeholder(),
    //     shape,
    // )).collect())

    Ok(vec![])
}

fn build_crossing(start: Checkpoint, crossing: &Crossing, subtracks: Vec<TrackIds>, structure_name: &str) -> anyhow::Result<Vec<Track>> {
    ensure!(subtracks.len() == 2, "Crossing must have exactly two subtracks");

    let half_angle = (1.0 / crossing.tangent).atan() / 2.0;
    let half_length = crossing.length / 2.0;

    let bd_start = Checkpoint {
        pos: start.pos,
        rotation: start.rotation * Mat3::from_rotation_y(-half_angle),
    };
    let ac_start = Checkpoint {
        pos: start.pos,
        rotation: start.rotation * Mat3::from_rotation_y(half_angle),
    };

    let shape_bd = TrackShape::straight_around_point(bd_start, -half_length, half_length);
    let shape_ac = TrackShape::straight_around_point(ac_start, -half_length, half_length);

    Ok(vec![
        Track::new_structure_end(subtracks[0], shape_ac, structure_name.to_string()),
        Track::new_structure_end(subtracks[1], shape_bd, structure_name.to_string()),
    ])
}

fn parse_subtrack_ids(cell: &str) -> anyhow::Result<Vec<TrackIds>> {
    let ids = cell.split(',')
        .filter(|part| !part.is_empty())
        .map(|part| {
            if let Some((_, own, prev, next)) = regex_captures!(r"^(\d+):(\d*):(\d*)$", part) {
                TrackIds::parse(own, prev, next)
            } else {
                bail!("Invalid subtrack ID format: {part}");
            }
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(ids)
}

fn parse_track_structure(cells: &[&str]) -> anyhow::Result<Switch> {
    ensure!(cells.len() >= 19);
    let id = cells[1].parse()?;
    let start = Checkpoint {
        pos: parse_position(&cells[3..6])?,
        rotation: parse_transform(&cells[6..9])?,
    };

    let Some(structure_name) = cells[2].split(',').next() else {
        bail!("Track structure name is missing");
    };

    let subtracks = parse_subtrack_ids(cells[9])?;

    let tracks: Vec<Track> = if let Some(track_structure) = TRACK_STRUCTURES.get(structure_name) {
        match track_structure {
            TrackStructure::Fork(fork) => build_fork_switch(start, fork, subtracks, structure_name)?,
            TrackStructure::Slip(slip) => build_slip_switch(start, slip, structure_name)?,
            TrackStructure::Crossing(crossing) => build_crossing(start, crossing, subtracks, structure_name)?,
        }
    } else {
        bail!("Unknown switch type {structure_name}");
    };

    Ok(Switch {
        id,
        tracks,
    })
}

fn find_failed_connections(tracks: &Vec<Track>, track_indexes: &HashMap<i32, usize>) -> Vec<FailedConnection> {
    let mut failed_connections: Vec<FailedConnection> = vec![];

    for track in tracks {
        let mut check_neighbour = |id: Option<i32>, pos: Vec3| {
            if let Some(other_index) = id.and_then(|id| track_indexes.get(&id)) {
                let other = &tracks[*other_index];
                let mut dist_min = (other.shape.start().pos, (other.shape.start().pos.xz() - pos.xz()).length_squared()) ;
                let mut dist_max = (other.shape.end().pos, (other.shape.end().pos.xz() - pos.xz()).length_squared());
                if dist_max.1 < dist_min.1 {
                    swap(&mut dist_min, &mut dist_max);
                }
                let dist_squared = dist_min.1;
                if dist_squared > 1.0 {
                    failed_connections.push(FailedConnection {
                        pos1: pos,
                        pos2: dist_min.0,
                        track1: track.clone(),
                        track2: other.clone(),
                    });
                    let difference = dist_min.0 - pos;
                    println!(
                        "The tracks {} and {} don't align - their ends are at a distance of {}: (x: {}, y (ignored): {}, z: {})",
                        track.ids.own,
                        other.ids.own,
                        dist_squared.sqrt(),
                        difference.x,
                        difference.y,
                        difference.z,
                    );
                    if let Some(structure_name) = &track.end_for_structure {
                        println!("Track {} is part of the track structure \"{}\"", track.ids.own, structure_name);
                    }
                }
            }
        };
        check_neighbour(track.ids.prev, track.shape.start().pos);
        check_neighbour(track.ids.next, track.shape.end().pos);
    }

    failed_connections
}

pub fn parse<R: Read>(input: R) -> anyhow::Result<ParseResult> {
    let lines = BufReader::new(input).lines();

    let mut tracks: Vec<Track> = vec![];

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
                    Ok(track) => {
                        tracks.push(track);
                    },
                    Err(e) => println!("Failed to parse track: {e}"),
                },
                "TrackStructure" => match parse_track_structure(&cells) {
                    Ok(switch) => {
                        tracks.extend(switch.tracks);
                    },
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

    let mut track_indexes: HashMap<i32, usize> = HashMap::new();
    for (index, track) in tracks.iter().enumerate() {
        let prev = track_indexes.insert(track.ids.own, index);
        if prev.is_some() {
            println!("Duplicate track ID found: {}", track.ids.own);
        }
    }

    let failed_connections = find_failed_connections(&tracks, &track_indexes);

    Ok(ParseResult { tracks, track_indexes, failed_connections })
}
