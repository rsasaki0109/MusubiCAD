//! `opencad mesh` command — tessellate and summarize viewport scene data.

use std::collections::BTreeMap;

use opencad_core::Result;
use opencad_desktop::tessellate_active_body;
use opencad_file::read_ocad;
use opencad_render::{OffscreenRenderer, RenderScene, SketchOverlay};
use serde::{Deserialize, Serialize};

pub use opencad_desktop::{load_view_data, ViewData};

pub const PREVIEW_WIDTH: u32 = 512;
pub const PREVIEW_HEIGHT: u32 = 512;

/// Options for `opencad mesh`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeshOptions {
    pub render: bool,
    pub png_output: Option<String>,
}

/// Summary printed by `opencad mesh`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshSummary {
    pub triangles: usize,
    pub vertices: usize,
    pub bounds_min_m: [f32; 3],
    pub bounds_max_m: [f32; 3],
    pub camera_distance_m: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendered_pixels: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub png_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay_lines: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay_labels: Option<usize>,
}

pub fn mesh_document(input: &str, options: &MeshOptions) -> Result<MeshSummary> {
    let needs_render = options.render || options.png_output.is_some();
    let data = if needs_render {
        load_view_data(input)?
    } else {
        let scene = {
            let doc = read_ocad(input)?;
            let parameters = doc.parameters.clone();
            let semantic_refs = doc.semantic_refs.clone();
            let mut model = doc.into_part_model();
            let mesh_set =
                tessellate_active_body(&mut model, Some(&parameters), Some(&semantic_refs))?;
            RenderScene::from_mesh_set(&mesh_set)?
        };
        ViewData {
            scene,
            overlay: SketchOverlay::default(),
            name: String::new(),
            feature_nodes: Vec::new(),
            sketches: BTreeMap::new(),
            parameter_name_to_id: BTreeMap::new(),
            semantic_refs: Vec::new(),
            face_history: Vec::new(),
            parameter_ids: Vec::new(),
        }
    };

    let camera = data.scene.default_camera(16.0 / 9.0);
    let vertices = data
        .scene
        .meshes
        .iter()
        .map(|mesh| mesh.vertex_count())
        .sum();
    let overlay_lines = if data.overlay.is_empty() {
        None
    } else {
        Some(data.overlay.lines.len())
    };
    let overlay_labels = if data.overlay.labels.is_empty() {
        None
    } else {
        Some(data.overlay.labels.len())
    };

    let (rendered_pixels, png_output) = if needs_render {
        let renderer = OffscreenRenderer::new()?;
        let overlay = if data.overlay.is_empty() {
            None
        } else {
            Some(&data.overlay)
        };
        if let Some(path) = &options.png_output {
            let output = renderer.render_scene_png(
                &data.scene,
                overlay,
                PREVIEW_WIDTH,
                PREVIEW_HEIGHT,
                path,
            )?;
            (Some(output.non_background_pixels), Some(path.clone()))
        } else {
            let output =
                renderer.render_scene_image(&data.scene, overlay, PREVIEW_WIDTH, PREVIEW_HEIGHT)?;
            (Some(output.non_background_pixels), None)
        }
    } else {
        (None, None)
    };

    Ok(MeshSummary {
        triangles: data.scene.triangle_count(),
        vertices,
        bounds_min_m: data.scene.bounds.min,
        bounds_max_m: data.scene.bounds.max,
        camera_distance_m: camera.distance,
        rendered_pixels,
        png_output,
        overlay_lines,
        overlay_labels,
    })
}

pub fn print_summary(summary: &MeshSummary) {
    println!("triangles: {}", summary.triangles);
    println!("vertices: {}", summary.vertices);
    println!(
        "bounds_min_m: [{:.6}, {:.6}, {:.6}]",
        summary.bounds_min_m[0], summary.bounds_min_m[1], summary.bounds_min_m[2]
    );
    println!(
        "bounds_max_m: [{:.6}, {:.6}, {:.6}]",
        summary.bounds_max_m[0], summary.bounds_max_m[1], summary.bounds_max_m[2]
    );
    println!("camera_distance_m: {:.6}", summary.camera_distance_m);
    if let Some(lines) = summary.overlay_lines {
        println!("overlay_lines: {lines}");
    }
    if let Some(labels) = summary.overlay_labels {
        println!("overlay_labels: {labels}");
    }
    if let Some(pixels) = summary.rendered_pixels {
        println!("rendered_pixels: {pixels}");
    }
    if let Some(path) = &summary.png_output {
        println!("png_output: {path}");
    }
}

#[cfg(test)]
pub(crate) fn write_bracket_fixture_at(path: &std::path::Path) {
    opencad_desktop::fixture::write_bracket_fixture_at(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_bracket_fixture(path: &std::path::Path) {
        write_bracket_fixture_at(path);
    }

    #[test]
    fn mesh_bracket_fixture() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture(&path);

        let summary =
            mesh_document(path.to_str().expect("path"), &MeshOptions::default()).expect("mesh");
        assert!(summary.triangles > 0);
        assert!(summary.vertices > 0);
        assert!(summary.camera_distance_m > 0.0);
        assert!(summary.bounds_max_m[0] > summary.bounds_min_m[0]);
    }

    #[test]
    fn mesh_render_reports_pixels_and_overlay() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture(&path);

        let summary = mesh_document(
            path.to_str().expect("path"),
            &MeshOptions {
                render: true,
                ..MeshOptions::default()
            },
        )
        .expect("mesh");
        assert!(summary.rendered_pixels.expect("pixels") > 0);
        assert!(summary.overlay_lines.expect("lines") > 0);
        assert!(summary.overlay_labels.expect("labels") > 0);
    }

    #[test]
    fn mesh_png_export_writes_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture(&path);
        let png_path = dir.path().join("preview.png");

        let summary = mesh_document(
            path.to_str().expect("path"),
            &MeshOptions {
                png_output: Some(png_path.to_str().expect("png").to_string()),
                ..MeshOptions::default()
            },
        )
        .expect("mesh");

        assert_eq!(
            summary.png_output.as_deref(),
            Some(png_path.to_str().expect("png"))
        );
        let bytes = std::fs::read(&png_path).expect("read png");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(summary.rendered_pixels.expect("pixels") > 0);
        assert!(summary.overlay_lines.expect("lines") > 0);
        assert!(summary.overlay_labels.expect("labels") > 0);
    }
}
