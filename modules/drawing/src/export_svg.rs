//! SVG export for drawing sheets (Task-178).

use opencad_core::{OpenCadError, Result};

use crate::hidden_line::LineVisibility;
use crate::render::{build_sheet_segments, SheetSegment, ViewMesh};
use crate::sheet::Sheet;

const MM_PER_M: f64 = 1000.0;

/// Render one sheet and its projected view meshes to SVG (millimeter user units).
pub fn render_sheet_svg(sheet: &Sheet, meshes: &[ViewMesh]) -> Result<String> {
    let segments = build_sheet_segments(sheet, meshes)?;
    Ok(sheet_segments_to_svg(sheet, &segments))
}

pub fn sheet_segments_to_svg(sheet: &Sheet, segments: &[SheetSegment]) -> String {
    let width_mm = sheet.width_m * MM_PER_M;
    let height_mm = sheet.height_m * MM_PER_M;
    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width_mm:.3} {height_mm:.3}\" width=\"{width_mm:.1}mm\" height=\"{height_mm:.1}mm\">\n"
    ));
    svg.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{width_mm:.3}\" height=\"{height_mm:.3}\" fill=\"white\" stroke=\"#cccccc\" stroke-width=\"0.2\"/>\n"
    ));
    for segment in segments {
        let x1 = segment.start_m[0] * MM_PER_M;
        let y1 = flip_y(segment.start_m[1], sheet.height_m) * MM_PER_M;
        let x2 = segment.end_m[0] * MM_PER_M;
        let y2 = flip_y(segment.end_m[1], sheet.height_m) * MM_PER_M;
        let style = match segment.visibility {
            LineVisibility::Visible => "stroke=\"#111111\" stroke-width=\"0.25\"",
            LineVisibility::Hidden => {
                "stroke=\"#777777\" stroke-width=\"0.18\" stroke-dasharray=\"2,1\""
            }
        };
        svg.push_str(&format!(
            "<line x1=\"{x1:.3}\" y1=\"{y1:.3}\" x2=\"{x2:.3}\" y2=\"{y2:.3}\" {style}/>\n"
        ));
    }
    svg.push_str("</svg>\n");
    svg
}

fn flip_y(y_m: f64, sheet_height_m: f64) -> f64 {
    sheet_height_m - y_m
}

pub fn validate_svg(svg: &str) -> Result<()> {
    if !svg.starts_with("<svg") || !svg.contains("</svg>") {
        return Err(OpenCadError::validation("generated SVG is malformed"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DrawingView, ModelReference, ProjectionKind};
    use opencad_core::{DocumentId, SheetId, ViewId};
    use opencad_geometry::MeshSet;

    #[test]
    fn renders_sheet_svg() -> opencad_core::Result<()> {
        let sheet = Sheet::a4_portrait(SheetId::new("sheet:a4")?, "Sheet 1");
        let view_id = ViewId::new("view:front")?;
        let mut sheet = sheet;
        sheet.views.push(DrawingView::new(
            view_id.clone(),
            "Front",
            ModelReference::new("parts/bracket.ocad.d", DocumentId::new("doc:bracket_001")?),
            ProjectionKind::Front,
            1.0,
            [0.05, 0.05],
        ));
        let meshes = vec![ViewMesh {
            view_id,
            mesh_set: MeshSet::box_prism(0.08, 0.006),
        }];
        let svg = render_sheet_svg(&sheet, &meshes).expect("svg");
        validate_svg(&svg).expect("valid");
        assert!(svg.contains("<line"));
        Ok(())
    }

    #[test]
    fn renders_hidden_segments_as_dashed_lines() {
        let sheet = Sheet::a4_portrait(SheetId::new("sheet:a4").expect("sheet id"), "Sheet 1");
        let svg = sheet_segments_to_svg(
            &sheet,
            &[SheetSegment {
                start_m: [0.01, 0.01],
                end_m: [0.02, 0.01],
                visibility: LineVisibility::Hidden,
            }],
        );
        assert!(svg.contains("stroke-dasharray=\"2,1\""));
    }
}
