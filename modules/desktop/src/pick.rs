//! Headless viewport pick queries for the desktop shell.

use opencad_core::Result;
use opencad_render::{
    face_group_highlight_edges, triangle_world_positions, OffscreenRenderer, PickResult, RenderScene,
};
use serde::{Deserialize, Serialize};

use crate::preview::{load_view_data, ViewData, CameraState, PREVIEW_HEIGHT, PREVIEW_WIDTH};
use crate::related_parameters::related_parameter_ids_for_features;
use crate::scene_query::{infer_face_refs, topo_ref_for_group};

/// Options for a pick query against the default preview viewport.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PickOptions {
    pub x: f64,
    pub y: f64,
    pub width: u32,
    pub height: u32,
}

impl Default for PickOptions {
    fn default() -> Self {
        Self {
            x: PREVIEW_WIDTH as f64 * 0.5,
            y: PREVIEW_HEIGHT as f64 * 0.5,
            width: PREVIEW_WIDTH,
            height: PREVIEW_HEIGHT,
        }
    }
}

/// Serializable pick target returned to the desktop UI and agents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PickTarget {
    None,
    SketchLine {
        line_index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        sketch_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        entity_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        entity_kind: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        segment_index: Option<usize>,
        construction: bool,
        start_m: [f32; 3],
        end_m: [f32; 3],
    },
    SolidTriangle {
        triangle_index: usize,
        vertices_m: [[f32; 3]; 3],
        #[serde(skip_serializing_if = "Option::is_none")]
        face_group_index: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        face_role: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        face_normal_m: Option<[f32; 3]>,
        #[serde(skip_serializing_if = "Option::is_none")]
        face_centroid_m: Option<[f32; 3]>,
        #[serde(skip_serializing_if = "Option::is_none")]
        kernel_face_id: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        inferred_feature_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        inferred_topo_ref_id: Option<String>,
    },
}

/// Screen-space line segment for preview highlight overlays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenSegment {
    pub start_px: [f64; 2],
    pub end_px: [f64; 2],
}

/// Summary returned by `pick_document`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PickSummary {
    pub x: f64,
    pub y: f64,
    pub width: u32,
    pub height: u32,
    pub overlay_line_count: usize,
    pub triangle_count: usize,
    pub selection: PickTarget,
    pub highlight_segments_px: Vec<ScreenSegment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_parameter_ids: Vec<String>,
}

pub fn pick_document(path: &str, options: &PickOptions) -> Result<PickSummary> {
    let data = load_view_data(path)?;
    let overlay = if data.overlay.is_empty() {
        None
    } else {
        Some(&data.overlay)
    };
    let renderer = OffscreenRenderer::new()?;
    let pick = renderer.pick_scene_at(
        &data.scene,
        overlay,
        options.x,
        options.y,
        options.width,
        options.height,
    )?;
    Ok(build_pick_summary(&data, pick, options))
}

pub fn build_pick_summary(data: &ViewData, pick: PickResult, options: &PickOptions) -> PickSummary {
    let scene = &data.scene;
    let overlay = &data.overlay;
    let feature_nodes = Some(data.feature_nodes.as_slice());
    let semantic_refs = data.semantic_refs.as_slice();
    let face_history = data.face_history.as_slice();
    let parameter_ids = data.parameter_ids.as_slice();
    let selection = match pick {
        PickResult::None => PickTarget::None,
        PickResult::SketchLine(line_index) => {
            let line = overlay.lines.get(line_index);
            let entity = overlay.pickable_line_at(line_index);
            PickTarget::SketchLine {
                line_index,
                sketch_id: entity.as_ref().map(|entity| entity.sketch_id.clone()),
                entity_id: entity.as_ref().map(|entity| entity.entity_id.clone()),
                entity_kind: entity.as_ref().map(|entity| entity.entity_kind.to_string()),
                segment_index: entity.and_then(|entity| entity.segment_index),
                construction: line.is_some_and(|line| line.construction),
                start_m: line.map(|line| line.start).unwrap_or([0.0; 3]),
                end_m: line.map(|line| line.end).unwrap_or([0.0; 3]),
            }
        }
        PickResult::SolidTriangle(triangle_index) => {
            let face = scene.face_group_at(triangle_index);
            let inferred =
                face.and_then(|face| feature_nodes.map(|nodes| infer_face_refs(nodes, face)));
            PickTarget::SolidTriangle {
                triangle_index,
                vertices_m: triangle_world_positions(scene, triangle_index)
                    .unwrap_or([[0.0; 3]; 3]),
                face_group_index: face.map(|face| face.index),
                face_role: face.map(|face| face.role.as_str().to_string()),
                face_normal_m: face.map(|face| face.normal),
                face_centroid_m: face.map(|face| face.centroid),
                kernel_face_id: face.and_then(|face| face.kernel_face_id),
                inferred_feature_id: inferred.as_ref().and_then(|(id, _)| id.clone()),
                inferred_topo_ref_id: face.and_then(|face| {
                    inferred.as_ref().and_then(|refs| {
                        topo_ref_for_group(face, refs, semantic_refs, face_history)
                    })
                }),
            }
        }
    };

    let highlight_segments_px =
        preview_highlight_segments(scene, &selection);
    let related_ids = related_parameter_ids_for_features(
        &selection,
        parameter_ids,
        data.feature_nodes.as_slice(),
        &data.sketches,
        &data.parameter_name_to_id,
    );

    PickSummary {
        x: options.x,
        y: options.y,
        width: options.width,
        height: options.height,
        overlay_line_count: overlay.lines.len(),
        triangle_count: scene.triangle_count(),
        selection,
        highlight_segments_px,
        related_parameter_ids: related_ids,
    }
}

/// Screen-space highlight segments for the default preview camera.
pub fn preview_highlight_segments(
    scene: &RenderScene,
    selection: &PickTarget,
) -> Vec<ScreenSegment> {
    highlight_segments_for_selection(scene, selection, PREVIEW_WIDTH, PREVIEW_HEIGHT)
}

/// Screen-space highlight segments projected with a synced camera pose.
pub fn highlight_segments_for_camera(
    scene: &RenderScene,
    selection: &PickTarget,
    camera: &CameraState,
    width: u32,
    height: u32,
) -> Vec<ScreenSegment> {
    let aspect = width as f32 / height.max(1) as f32;
    let orbit = camera.to_orbit_camera(aspect);
    highlight_segments_with_camera(scene, selection, &orbit, width, height)
}

fn highlight_segments_for_selection(
    scene: &RenderScene,
    selection: &PickTarget,
    width: u32,
    height: u32,
) -> Vec<ScreenSegment> {
    let aspect = width as f32 / height.max(1) as f32;
    let camera = scene.default_camera(aspect);
    highlight_segments_with_camera(scene, selection, &camera, width, height)
}

fn highlight_segments_with_camera(
    scene: &RenderScene,
    selection: &PickTarget,
    camera: &opencad_render::OrbitCamera,
    width: u32,
    height: u32,
) -> Vec<ScreenSegment> {
    let project_segment = |start: [f32; 3], end: [f32; 3]| -> Option<ScreenSegment> {
        let start_px = camera.project_to_screen(width, height, start)?;
        let end_px = camera.project_to_screen(width, height, end)?;
        Some(ScreenSegment { start_px, end_px })
    };

    match selection {
        PickTarget::None => Vec::new(),
        PickTarget::SketchLine { start_m, end_m, .. } => {
            project_segment(*start_m, *end_m).into_iter().collect()
        }
        PickTarget::SolidTriangle {
            face_group_index,
            vertices_m,
            ..
        } => {
            if let Some(group_index) = face_group_index {
                let edges = face_group_highlight_edges(scene, *group_index);
                if !edges.is_empty() {
                    return edges
                        .into_iter()
                        .filter_map(|(start, end)| project_segment(start, end))
                        .collect();
                }
            }
            let [v0, v1, v2] = *vertices_m;
            [
                project_segment(v0, v1),
                project_segment(v1, v2),
                project_segment(v2, v0),
            ]
            .into_iter()
            .flatten()
            .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::write_bracket_fixture_at;
    use tempfile::tempdir;

    #[test]
    fn pick_center_hits_geometry() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        let summary =
            pick_document(path.to_str().expect("path"), &PickOptions::default()).expect("pick");
        assert!(summary.triangle_count > 0);
        assert!(summary.overlay_line_count > 0);
        assert!(matches!(
            summary.selection,
            PickTarget::SolidTriangle {
                face_role: Some(_),
                inferred_feature_id: Some(_),
                ..
            } | PickTarget::SketchLine { .. }
        ));
        assert!(!summary.highlight_segments_px.is_empty());
    }

    #[test]
    fn solid_pick_highlights_face_group_boundary() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        let summary =
            pick_document(path.to_str().expect("path"), &PickOptions::default()).expect("pick");
        if let PickTarget::SolidTriangle { .. } = summary.selection {
            assert!(
                summary.highlight_segments_px.len() >= 4,
                "expected face-group boundary highlight, got {}",
                summary.highlight_segments_px.len()
            );
        }
    }

    #[test]
    fn cylindrical_highlight_edges_form_two_rings_on_bracket() {
        use opencad_core::{DocumentId, DocumentMetadata};
        use opencad_feature::bracket_with_hole;
        use opencad_file::{write_expanded_dir, OcadDocument};
        use opencad_graph::bracket_parameters;
        use opencad_render::{face_group_highlight_edges, FaceRole};

        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket_hole.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_hole").expect("id"),
            "Bracket hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&path, &doc).expect("write");

        let data = load_view_data(path.to_str().expect("path")).expect("view");
        let cylindrical = data
            .scene
            .face_catalog
            .groups
            .iter()
            .find(|group| group.role == FaceRole::Cylindrical)
            .expect("cylindrical");
        let triangle_count = data
            .scene
            .face_catalog
            .triangle_indices_in_group(cylindrical.index)
            .len();
        let edges = face_group_highlight_edges(&data.scene, cylindrical.index);
        assert!(
            edges.len() >= 4,
            "expected cylindrical highlight edges, got {} ({} triangles)",
            edges.len(),
            triangle_count
        );
        if triangle_count >= 12 {
            assert!(
                edges.len() >= 8,
                "expected ring outlines for dense cylinder, got {}",
                edges.len()
            );
        }
    }

    #[test]
    fn preview_highlight_matches_default_preview_dimensions() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);
        let data = load_view_data(path.to_str().expect("path")).expect("view");
        let options = PickOptions {
            x: 640.0,
            y: 360.0,
            width: 1280,
            height: 720,
        };
        let renderer = OffscreenRenderer::new().expect("renderer");
        let pick = renderer
            .pick_scene_at(
                &data.scene,
                Some(&data.overlay),
                options.x,
                options.y,
                options.width,
                options.height,
            )
            .expect("pick");
        let summary = build_pick_summary(&data, pick, &options);
        assert_eq!(summary.width, 1280);
        assert_eq!(summary.height, 720);
        assert!(!summary.highlight_segments_px.is_empty());
        for segment in &summary.highlight_segments_px {
            assert!(segment.start_px[0] <= PREVIEW_WIDTH as f64);
            assert!(segment.end_px[0] <= PREVIEW_WIDTH as f64);
            assert!(segment.start_px[1] <= PREVIEW_HEIGHT as f64);
            assert!(segment.end_px[1] <= PREVIEW_HEIGHT as f64);
        }
    }

    #[test]
    fn pick_corner_returns_none() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        let summary = pick_document(
            path.to_str().expect("path"),
            &PickOptions {
                x: 0.0,
                y: 0.0,
                ..PickOptions::default()
            },
        )
        .expect("pick");
        assert!(matches!(summary.selection, PickTarget::None));
        assert!(summary.highlight_segments_px.is_empty());
    }
}
