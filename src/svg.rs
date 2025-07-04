use crate::parse::{ParseResult, Track, TrackShape};
use std::fs;
use std::path::Path;
use svg::node::element;
use svg::node::element::path::Data;
use svg::Document;

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

pub(crate) fn create_svg(parse_result: &ParseResult, output_path: &Path) -> anyhow::Result<()> {
    let mut document = Document::new().set("viewBox", (-5000, -5000, 5000, 5000));

    for track in &parse_result.tracks {
        let data = path_data(track);
        let path = element::Path::new()
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
            .set("stroke-width", 4.0);
        document = document.add(path);
    }

    if let Some(dir) = output_path.parent() {
        fs::create_dir_all(dir)?;
    }
    svg::save(output_path, &document).map_err(|e| anyhow::anyhow!("Failed to save SVG: {}", e))?;
    Ok(())
}
