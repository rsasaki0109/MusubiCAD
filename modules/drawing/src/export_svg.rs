//! SVG export for drawing sheets (Task-178).

use opencad_core::{OpenCadError, Result};

use crate::dimension::{layout_linear_dimension, DimensionLayout};
use crate::hidden_line::LineVisibility;
use crate::render::{build_sheet_segments, SheetSegment, ViewMesh};
use crate::sheet::Sheet;

const MM_PER_M: f64 = 1000.0;

/// Render one sheet and its projected view meshes to SVG (millimeter user units).
pub fn render_sheet_svg(sheet: &Sheet, meshes: &[ViewMesh]) -> Result<String> {
    if !sheet.radial_dimensions.is_empty() {
        return Err(OpenCadError::validation(
            "radial dimensions require regenerated layouts",
        ));
    }
    render_sheet_svg_with_radial_layouts(sheet, meshes, &[])
}

pub fn render_sheet_svg_with_radial_layouts(
    sheet: &Sheet,
    meshes: &[ViewMesh],
    radial: &[crate::RadialLayout],
) -> Result<String> {
    render_sheet_svg_with_layouts(sheet, meshes, radial, &[])
}

pub fn render_sheet_svg_with_layouts(
    sheet: &Sheet,
    meshes: &[ViewMesh],
    radial: &[crate::RadialLayout],
    sections: &[crate::SectionLayout],
) -> Result<String> {
    if sections.len() != sheet.views.iter().filter(|v| v.section.is_some()).count()
        || sections.iter().any(|s| {
            !sheet
                .views
                .iter()
                .any(|v| v.id == s.view_id && v.section.is_some())
        })
    {
        return Err(OpenCadError::validation(
            "section views require regenerated contour layouts",
        ));
    }
    if radial.len() != sheet.radial_dimensions.len() {
        return Err(OpenCadError::validation("missing radial dimension layouts"));
    }
    let segments = build_sheet_segments(sheet, meshes)?;
    let dimensions = sheet
        .dimensions
        .iter()
        .map(|dimension| {
            let view = sheet
                .views
                .iter()
                .find(|view| view.id == dimension.view_id)
                .ok_or_else(|| {
                    OpenCadError::validation(format!(
                        "dimension '{}' references missing view '{}'",
                        dimension.id, dimension.view_id
                    ))
                })?;
            layout_linear_dimension(dimension, view)
        })
        .collect::<Result<Vec<_>>>()?;
    let angular = sheet
        .angular_dimensions
        .iter()
        .map(|dimension| {
            let view = sheet
                .views
                .iter()
                .find(|v| v.id == dimension.view_id)
                .ok_or_else(|| OpenCadError::validation("angular view missing"))?;
            crate::layout_angular_dimension(dimension, view)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut svg = sheet_content_to_svg(sheet, &segments, &dimensions);
    if !angular.is_empty() {
        svg.truncate(svg.len() - "</svg>\n".len());
        for layout in angular {
            let x1 = layout.arc_start_m[0] * 1000.;
            let y1 = flip_y(layout.arc_start_m[1], sheet.height_m) * 1000.;
            let x2 = layout.arc_end_m[0] * 1000.;
            let y2 = flip_y(layout.arc_end_m[1], sheet.height_m) * 1000.;
            let r = layout.radius_m * 1000.;
            let sweep = u8::from(layout.clockwise);
            let vx = layout.vertex_m[0] * 1000.;
            let vy = flip_y(layout.vertex_m[1], sheet.height_m) * 1000.;
            for (x, y) in [(x1, y1), (x2, y2)] {
                svg.push_str(&format!("<line class=\"angular-witness\" x1=\"{vx:.6}\" y1=\"{vy:.6}\" x2=\"{x:.6}\" y2=\"{y:.6}\" stroke=\"#225588\" stroke-width=\"0.18\"/>\n"));
            }
            svg.push_str(&format!("<path class=\"angular-dimension\" d=\"M {x1:.6} {y1:.6} A {r:.6} {r:.6} 0 0 {sweep} {x2:.6} {y2:.6}\" fill=\"none\" stroke=\"#225588\" stroke-width=\"0.18\"/>\n"));
            let x = layout.text_position_m[0] * 1000.;
            let y = flip_y(layout.text_position_m[1], sheet.height_m) * 1000.;
            svg.push_str(&format!("<text class=\"angular-label\" x=\"{x:.6}\" y=\"{y:.6}\" text-anchor=\"middle\" font-family=\"sans-serif\" font-size=\"3\" fill=\"#225588\">{}</text>\n",layout.label));
        }
        svg.push_str("</svg>\n");
    }
    if !radial.is_empty() {
        svg.truncate(svg.len() - "</svg>\n".len());
        for layout in radial {
            let point = |p: [f64; 2]| [p[0] * 1000., flip_y(p[1], sheet.height_m) * 1000.];
            let a = point(layout.line_start_m);
            let b = point(layout.circle_point_m);
            let t = point(layout.text_position_m);
            svg.push_str(&format!("<line class=\"radial-dimension\" x1=\"{:.6}\" y1=\"{:.6}\" x2=\"{:.6}\" y2=\"{:.6}\" stroke=\"#225588\" stroke-width=\"0.18\"/>\n",a[0],a[1],b[0],b[1]));
            svg.push_str(&format!("<line class=\"radial-leader\" x1=\"{:.6}\" y1=\"{:.6}\" x2=\"{:.6}\" y2=\"{:.6}\" stroke=\"#225588\" stroke-width=\"0.18\"/>\n",b[0],b[1],t[0],t[1]));
            svg.push_str(&format!("<text class=\"radial-label\" x=\"{:.6}\" y=\"{:.6}\" font-family=\"sans-serif\" font-size=\"3\" fill=\"#225588\">{}</text>\n",t[0],t[1]-1.,layout.label));
        }
        svg.push_str("</svg>\n");
    }
    if !sections.is_empty() {
        svg.truncate(svg.len() - "</svg>\n".len());
        for (index, section) in sections.iter().enumerate() {
            let h = section.hatch_spacing_m * 1000. * 2_f64.sqrt();
            svg.push_str(&format!("<defs><pattern id=\"section-hatch-{index}\" patternUnits=\"userSpaceOnUse\" width=\"{h:.6}\" height=\"{h:.6}\"><path d=\"M 0 {h:.6} L {h:.6} 0\" fill=\"none\" stroke=\"#777777\" stroke-width=\"0.15\"/></pattern></defs>\n"));
            let mut path = String::new();
            for points in &section.loops_on_sheet_m {
                for (i, p) in points.iter().enumerate() {
                    path.push_str(&format!(
                        "{} {:.6} {:.6} ",
                        if i == 0 { "M" } else { "L" },
                        p[0] * 1000.,
                        flip_y(p[1], sheet.height_m) * 1000.
                    ));
                }
                path.push_str("Z ");
            }
            svg.push_str(&format!("<path class=\"section-contour\" d=\"{path}\" fill=\"url(#section-hatch-{index})\" fill-rule=\"evenodd\" stroke=\"#111111\" stroke-width=\"0.25\"/>\n"));
        }
        svg.push_str("</svg>\n");
    }
    if sheet.views.iter().any(|v| v.detail.is_some()) {
        svg.truncate(svg.len() - "</svg>\n".len());
        for view in sheet.views.iter().filter(|v| v.detail.is_some()) {
            let parent = crate::validate_detail_parent(view, sheet)?;
            let detail = view
                .detail
                .as_ref()
                .ok_or_else(|| OpenCadError::validation("detail missing"))?;
            let x = (parent.origin_on_sheet_m[0] + detail.region_min_m[0] * parent.scale) * 1000.;
            let y = flip_y(
                parent.origin_on_sheet_m[1] + detail.region_max_m[1] * parent.scale,
                sheet.height_m,
            ) * 1000.;
            let w = (detail.region_max_m[0] - detail.region_min_m[0]) * parent.scale * 1000.;
            let h = (detail.region_max_m[1] - detail.region_min_m[1]) * parent.scale * 1000.;
            if ![x, y, w, h].iter().all(|v| v.is_finite()) {
                return Err(OpenCadError::validation(
                    "detail source marker overflows sheet millimetres",
                ));
            }
            svg.push_str(&format!("<rect class=\"detail-source\" x=\"{x:.6}\" y=\"{y:.6}\" width=\"{w:.6}\" height=\"{h:.6}\" fill=\"none\" stroke=\"#225588\" stroke-width=\"0.18\" stroke-dasharray=\"2,1\"/>\n"));
            let x = view.origin_on_sheet_m[0] * 1000.;
            let y = flip_y(view.origin_on_sheet_m[1], sheet.height_m) * 1000.
                - h * detail.magnification / 2.
                - 4.;
            svg.push_str(&format!("<text class=\"detail-label\" x=\"{x:.6}\" y=\"{y:.6}\" text-anchor=\"middle\" font-family=\"sans-serif\" font-size=\"3\" fill=\"#225588\">DETAIL {:.2}×</text>\n",detail.magnification));
        }
        svg.push_str("</svg>\n");
    }
    if let Some(block) = &sheet.title_block {
        let cells = crate::layout_title_block(sheet, block)?;
        svg.truncate(svg.len() - "</svg>\n".len());
        for cell in cells {
            let x = cell.origin_m[0] * 1000.;
            let y = flip_y(cell.origin_m[1] + cell.height_m, sheet.height_m) * 1000.;
            let w = cell.width_m * 1000.;
            let h = cell.height_m * 1000.;
            svg.push_str(&format!("<rect class=\"title-block-cell\" x=\"{x:.6}\" y=\"{y:.6}\" width=\"{w:.6}\" height=\"{h:.6}\" fill=\"none\" stroke=\"#111111\" stroke-width=\"0.25\"/>\n"));
            let tx = x + 2.;
            let label_y = y + 3.;
            let value_y = y + 9.;
            svg.push_str(&format!("<text class=\"title-block-label\" x=\"{tx:.6}\" y=\"{label_y:.6}\" font-family=\"sans-serif\" font-size=\"2\" fill=\"#555555\">{}</text>\n",cell.label));
            svg.push_str(&format!("<text class=\"title-block-value\" x=\"{tx:.6}\" y=\"{value_y:.6}\" font-family=\"sans-serif\" font-size=\"3\" fill=\"#111111\">{}</text>\n",crate::title_block::escape_svg_text(&cell.value)));
        }
        svg.push_str("</svg>\n");
    }
    Ok(svg)
}

pub fn sheet_segments_to_svg(sheet: &Sheet, segments: &[SheetSegment]) -> String {
    sheet_content_to_svg(sheet, segments, &[])
}

fn sheet_content_to_svg(
    sheet: &Sheet,
    segments: &[SheetSegment],
    dimensions: &[DimensionLayout],
) -> String {
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
    for dimension in dimensions {
        push_dimension_svg(&mut svg, sheet, dimension);
    }
    svg.push_str("</svg>\n");
    svg
}

fn push_dimension_svg(svg: &mut String, sheet: &Sheet, dimension: &DimensionLayout) {
    let points = [
        (dimension.witness_start_m, dimension.line_start_m),
        (dimension.witness_end_m, dimension.line_end_m),
        (dimension.line_start_m, dimension.line_end_m),
    ];
    for (start, end) in points {
        let x1 = start[0] * MM_PER_M;
        let y1 = flip_y(start[1], sheet.height_m) * MM_PER_M;
        let x2 = end[0] * MM_PER_M;
        let y2 = flip_y(end[1], sheet.height_m) * MM_PER_M;
        svg.push_str(&format!(
            "<line class=\"dimension\" x1=\"{x1:.3}\" y1=\"{y1:.3}\" x2=\"{x2:.3}\" y2=\"{y2:.3}\" stroke=\"#225588\" stroke-width=\"0.18\"/>\n"
        ));
    }
    let text_x = dimension.text_position_m[0] * MM_PER_M;
    let text_y = flip_y(dimension.text_position_m[1], sheet.height_m) * MM_PER_M - 1.0;
    svg.push_str(&format!(
        "<text class=\"dimension-label\" x=\"{text_x:.3}\" y=\"{text_y:.3}\" text-anchor=\"middle\" font-family=\"sans-serif\" font-size=\"3\" fill=\"#225588\">{}</text>\n",
        dimension.label
    ));
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
    use crate::{DrawingView, LinearDimension, ModelReference, ProjectionKind};
    use opencad_core::{DimensionId, DocumentId, SheetId, ViewId};
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

    #[test]
    fn partially_occluded_edges_match_svg_golden() -> opencad_core::Result<()> {
        let mut sheet = Sheet::a4_portrait(SheetId::new("sheet:partial_occlusion")?, "HLR");
        let view_id = ViewId::new("view:front")?;
        sheet.views.push(DrawingView::new(
            view_id.clone(),
            "Front",
            ModelReference::new("synthetic.ocad.d", DocumentId::new("doc:synthetic")?),
            ProjectionKind::Front,
            1.0,
            [0.05, 0.05],
        ));
        let mesh = MeshSet {
            positions: vec![
                [-0.01, 0.0, 0.0],
                [0.01, 0.0, 0.0],
                [0.0, -0.01, 0.0],
                [-0.005, -0.005, 0.001],
                [0.005, -0.005, 0.001],
                [0.0, 0.005, 0.001],
            ],
            normals: Vec::new(),
            indices: vec![0, 1, 2, 3, 4, 5],
            triangle_face_ids: vec![1, 2],
        };
        let svg = render_sheet_svg(
            &sheet,
            &[ViewMesh {
                view_id,
                mesh_set: mesh,
            }],
        )?;
        assert_eq!(svg, include_str!("../tests/golden/partial-occlusion.svg"));
        Ok(())
    }

    #[test]
    fn renders_model_driven_dimension_label() -> opencad_core::Result<()> {
        let mut sheet = Sheet::a4_portrait(SheetId::new("sheet:a4")?, "Sheet 1");
        let view_id = ViewId::new("view:front")?;
        sheet.views.push(DrawingView::new(
            view_id.clone(),
            "Front",
            ModelReference::new("part.ocad.d", DocumentId::new("doc:part")?),
            ProjectionKind::Front,
            1.0,
            [0.02, 0.02],
        ));
        sheet.dimensions.push(LinearDimension {
            id: DimensionId::new("dim:width")?,
            view_id: view_id.clone(),
            measurement: crate::LinearMeasurement::Points {
                start_model_m: [0.0, 0.0, 0.0],
                end_model_m: [0.08, 0.0, 0.0],
            },
            offset_m: 0.01,
        });
        let svg = render_sheet_svg(
            &sheet,
            &[ViewMesh {
                view_id,
                mesh_set: MeshSet::box_prism(0.08, 0.006),
            }],
        )?;
        assert!(svg.contains("class=\"dimension-label\""));
        assert!(svg.contains(">80.00 mm</text>"));
        Ok(())
    }
}
