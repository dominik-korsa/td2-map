use crate::math::{project_circle, project_vec};
use crate::parse::{ParseResult, Track, TrackShape};
use std::fs;
use std::path::Path;
use svg::node::element;
use svg::node::element::path::Data;
use svg::node::element::Rectangle;
use svg::{Document, Node};

fn path_data(track_shape: &TrackShape) -> Data {
    match track_shape {
        TrackShape::Straight { start, end } => {
            let projected_start = project_vec(start);
            let projected_end = project_vec(end);

            let data = Data::new()
                .move_to((projected_start.x, projected_start.y))
                .line_to((projected_end.x, projected_end.y));
            data
        }
        TrackShape::Arc { start, end, rotated_circle, .. } => {
            let projected_start = project_vec(start);
            let projected_end = project_vec(end);
            let projected_circle = project_circle(&rotated_circle);

            let data = Data::new()
                .move_to((projected_start.x, projected_start.y))
                .elliptical_arc_to((
                    projected_circle.major_axis.length(),
                    projected_circle.minor_axis.length(),
                    projected_circle.major_axis.to_angle(),
                    0, // large arc flag off
                    if rotated_circle.original_radius() > 0.0 { 0 } else { 1 },
                    projected_end.x, projected_end.y,
                ));
            data
        }
        TrackShape::Bezier { start, control1, control2, end } => {
            let projected_start = project_vec(start);
            let projected_control1 = project_vec(control1);
            let projected_control2 = project_vec(control2);
            let projected_end = project_vec(end);

            let data = Data::new()
                .move_to((projected_start.x, projected_start.y))
                .cubic_curve_to((
                    projected_control1.x, projected_control1.y,
                    projected_control2.x, projected_control2.y,
                    projected_end.x, projected_end.y,
                ));
            data
        }
    }
}

struct MapElement {
    y: f32,
    node: Box<dyn Node>,
}

pub fn create_svg(parse_result: &ParseResult, output_path: &Path) -> anyhow::Result<()> {
    static BG_COLOR: &str = "#11202D";
    static TRACK_COLOR: &str = "#eee";

    let mut document = Document::new();

    let mut map_elements: Vec<MapElement> = vec![];

    let mut min_x: f32 = f32::MAX;
    let mut max_x: f32 = f32::MIN;
    let mut min_z: f32 = f32::MAX;
    let mut max_z: f32 = f32::MIN;

    let mut add_track = |track: &Track, highlight: Option<&str>| {
        if track.shape.start() != track.shape.end() || true { // TODO: Remove || true
            let data = path_data(&track.shape);

            let label = format!(
                "Track {}, prev: {} next: {}",
                track.ids.own,
                track.ids.prev.map(|x| x.to_string()).unwrap_or("-".to_string()),
                track.ids.next.map(|x| x.to_string()).unwrap_or("-".to_string()),
            );

            let background_path = element::Path::new()
                .set("d", data.clone())
                .set("id", format!("track_bg_{}", track.ids.own))
                .set("fill", "none")
                .set("stroke", BG_COLOR)
                .set("stroke-width", 12.0)
                .set("stroke-linecap", "round");

            let track_path = element::Path::new()
                .set("id", format!("track_{}", track.ids.own))
                .set("inkscape:label", label)
                .set("d", data)
                .set("fill", "none")
                .set("stroke", highlight.unwrap_or(TRACK_COLOR))
                .set("stroke-width", 0.5)
                .set("stroke-linecap", "round");

            map_elements.push(MapElement { y: track.shape.lowest_y() - 4.0, node: Box::new(background_path) });
            map_elements.push(MapElement { y: track.shape.lowest_y(), node: Box::new(track_path) });
        }

        let projected_start = project_vec(track.shape.start());
        let projected_end = project_vec(track.shape.end());
        min_x = min_x.min(projected_start.x).min(projected_end.x);
        max_x = max_x.max(projected_start.x).max(projected_end.x);
        min_z = min_z.min(projected_start.y).min(projected_end.y);
        max_z = max_z.max(projected_start.y).max(projected_end.y);
    };

    let mut tracks = parse_result.tracks.clone();

    for switch in &parse_result.switches {
        tracks.extend(switch.tracks.clone());
        // for (index, track_shape) in switch.shape.track_shapes.iter().enumerate() {
            // add_track(track_shape, &format!("switch_track_{}_{}", switch.id, index), Some("magenta"));
        // }
    }

    for track in &tracks {
        add_track(&track, None);
    }

    let min_x = min_x as i64 - 100;
    let max_x = max_x as i64 + 100;
    let min_z = min_z as i64 - 100;
    let max_z = max_z as i64 + 100;

    document = document
        .set("viewBox", (min_x, min_z, max_x - min_x, max_z - min_z))
        .add(
            Rectangle::new()
                .set("x", min_x)
                .set("y", min_z)
                .set("width", max_x - min_x)
                .set("height", max_z - min_z)
                .set("fill", BG_COLOR)
        );

    map_elements.sort_by_key(|x| x.y as i64);

    for element in map_elements {
        document = document.add(element.node)
    }

    if let Some(dir) = output_path.parent() {
        fs::create_dir_all(dir)?;
    }
    svg::save(output_path, &document).map_err(|e| anyhow::anyhow!("Failed to save SVG: {}", e))?;
    Ok(())
}
