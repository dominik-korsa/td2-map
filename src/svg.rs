use crate::parse::{ParseResult, Track, TrackShape};
use std::fs;
use std::path::Path;
use svg::node::element;
use svg::node::element::path::Data;
use svg::{Document, Node};

fn path_data(track: &Track) -> Data {
    match track.shape {
        TrackShape::Straight { start, end } => {
            let data = Data::new()
                .move_to((start.x, start.z))
                .line_to((end.x, end.z));
            data
        }
        TrackShape::Arc { start, end, projection_first_axis, projection_second_axis, radius, .. } => {
            let data = Data::new()
                .move_to((start.x, start.z))
                .elliptical_arc_to((
                    projection_first_axis.length(),
                    projection_second_axis.length(),
                    projection_first_axis.to_angle(),
                    0, // large arc flag off
                    if radius > 0.0 { 1 } else { 0 },
                    end.x, end.z,
                ));
            data
        }
        TrackShape::Bezier { start, control1, control2, end } => {
            let data = Data::new()
                .move_to((start.x, start.z))
                .cubic_curve_to((
                    control1.x, control1.z,
                    control2.x, control2.z,
                    end.x, end.z,
                ));
            data
        }
    }
}

struct MapElement {
    y: f32,
    node: Box<dyn Node>,
}

pub(crate) fn create_svg(parse_result: &ParseResult, output_path: &Path) -> anyhow::Result<()> {
    let mut document = Document::new();

    let mut map_elements: Vec<MapElement> = vec![];

    let mut min_x: f32 = f32::MAX;
    let mut max_x: f32 = f32::MIN;
    let mut min_z: f32 = f32::MAX;
    let mut max_z: f32 = f32::MIN;

    for track in &parse_result.tracks {
        let data = path_data(track);

        let background_path = element::Path::new()
            .set("d", data.clone())
            .set("id", format!("track_bg_{}", track.id))
            .set("fill", "none")
            .set("stroke", "#fff")
            .set("stroke-width", 12.0)
            .set("stroke-linecap", "round");

        let track_path = element::Path::new()
            .set("id", format!("track_{}", track.id))
            .set("d", data)
            .set("fill", "none")
            .set(
                "stroke",
                match track.shape {
                    TrackShape::Straight { .. } => "black",
                    TrackShape::Arc { .. } => "blue",
                    TrackShape::Bezier { .. } => "red",
                },
            )
            .set("stroke-width", 1.75);

        map_elements.push(MapElement { y: track.shape.lowest_y() - 4.0, node: Box::new(background_path) });
        map_elements.push(MapElement { y: track.shape.lowest_y(), node: Box::new(track_path) });

        min_x = min_x.min(track.shape.start().x).min(track.shape.end().x);
        max_x = max_x.max(track.shape.start().x).max(track.shape.end().x);
        min_z = min_z.min(track.shape.start().z).min(track.shape.end().z);
        max_z = max_z.max(track.shape.start().z).max(track.shape.end().z);
    }

    let min_x = min_x as i64 - 100;
    let max_x = max_x as i64 + 100;
    let min_z = min_z as i64 - 100;
    let max_z = max_z as i64 + 100;

    document = document.set("viewBox", (min_x, min_z, max_x - min_x, max_z - min_z));

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
