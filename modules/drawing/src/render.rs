//! Wireframe extraction and sheet layout for drawing export.

use std::collections::BTreeMap;

use opencad_core::{OpenCadError, Result, ViewId};
use opencad_geometry::MeshSet;

use crate::hidden_line::{classify_hidden_lines, LineVisibility};
use crate::projection::ProjectionKind;
use crate::sheet::Sheet;
use crate::view::DrawingView;

/// A line segment on the sheet in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SheetSegment {
    /// Segment start on the sheet in meters.
    pub start_m: [f64; 2],
    /// Segment end on the sheet in meters.
    pub end_m: [f64; 2],
    /// Line visibility used to select the exported stroke style.
    pub visibility: LineVisibility,
}

/// Tessellated mesh prepared for one drawing view.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewMesh {
    pub view_id: ViewId,
    pub mesh_set: MeshSet,
}

const EDGE_KEY_SCALE: f64 = 1_000_000.0;

/// Extract projected wireframe segments from a mesh in model space (meters).
pub fn project_mesh_wireframe(mesh: &MeshSet, projection: ProjectionKind) -> Vec<SheetSegment> {
    let mut segments = BTreeMap::new();
    for edge in classify_hidden_lines(mesh, projection) {
        if let Some((start, end)) = edge_key(edge.start_m, edge.end_m) {
            segments
                .entry((start, end))
                .and_modify(|visibility| {
                    if edge.visibility == LineVisibility::Visible {
                        *visibility = LineVisibility::Visible;
                    }
                })
                .or_insert(edge.visibility);
        }
    }

    segments
        .into_iter()
        .map(|((start, end), visibility)| SheetSegment {
            start_m: [
                start[0] as f64 / EDGE_KEY_SCALE,
                start[1] as f64 / EDGE_KEY_SCALE,
            ],
            end_m: [
                end[0] as f64 / EDGE_KEY_SCALE,
                end[1] as f64 / EDGE_KEY_SCALE,
            ],
            visibility,
        })
        .collect()
}

fn edge_key(a: [f64; 2], b: [f64; 2]) -> Option<([i64; 2], [i64; 2])> {
    let qa = quantize_point(a);
    let qb = quantize_point(b);
    if qa == qb {
        return None;
    }
    Some(if qa <= qb { (qa, qb) } else { (qb, qa) })
}

fn quantize_point(point: [f64; 2]) -> [i64; 2] {
    [
        (point[0] * EDGE_KEY_SCALE).round() as i64,
        (point[1] * EDGE_KEY_SCALE).round() as i64,
    ]
}

pub fn layout_view_on_sheet(view: &DrawingView, segments: &[SheetSegment]) -> Vec<SheetSegment> {
    segments
        .iter()
        .map(|segment| SheetSegment {
            start_m: [
                view.origin_on_sheet_m[0] + segment.start_m[0] * view.scale,
                view.origin_on_sheet_m[1] + segment.start_m[1] * view.scale,
            ],
            end_m: [
                view.origin_on_sheet_m[0] + segment.end_m[0] * view.scale,
                view.origin_on_sheet_m[1] + segment.end_m[1] * view.scale,
            ],
            visibility: segment.visibility,
        })
        .collect()
}

/// Build sheet segments for all views that have tessellated meshes.
pub fn build_sheet_segments(sheet: &Sheet, meshes: &[ViewMesh]) -> Result<Vec<SheetSegment>> {
    let mut output = Vec::new();
    for view in &sheet.views {
        let Some(mesh) = meshes.iter().find(|entry| entry.view_id == view.id) else {
            return Err(OpenCadError::validation(format!(
                "missing tessellated mesh for view '{}'",
                view.id
            )));
        };
        let wireframe = project_mesh_wireframe(&mesh.mesh_set, view.projection);
        output.extend(layout_view_on_sheet(view, &wireframe));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::ViewId;

    #[test]
    fn projects_box_prism_wireframe() {
        let mesh = MeshSet::box_prism(0.08, 0.006);
        let segments = project_mesh_wireframe(&mesh, ProjectionKind::Front);
        assert!(!segments.is_empty());
    }

    #[test]
    fn builds_sheet_segments_for_view_mesh() -> opencad_core::Result<()> {
        use crate::{DrawingView, ModelReference, Sheet};
        use opencad_core::{DocumentId, SheetId};

        let sheet = Sheet {
            id: SheetId::new("sheet:a4")?,
            name: "Sheet 1".into(),
            width_m: 0.210,
            height_m: 0.297,
            views: vec![DrawingView::new(
                ViewId::new("view:front")?,
                "Front",
                ModelReference::new("parts/bracket.ocad.d", DocumentId::new("doc:bracket_001")?),
                ProjectionKind::Front,
                1.0,
                [0.01, 0.01],
            )],
            dimensions: Vec::new(),
        };
        let meshes = vec![ViewMesh {
            view_id: ViewId::new("view:front")?,
            mesh_set: MeshSet::box_prism(0.08, 0.006),
        }];
        let segments = build_sheet_segments(&sheet, &meshes).expect("segments");
        assert!(!segments.is_empty());
        Ok(())
    }
}
