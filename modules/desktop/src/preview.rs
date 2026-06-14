//! Load documents and render PNG previews for the desktop shell.

use std::io::Cursor;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{ImageBuffer, ImageFormat, RgbaImage};
use opencad_core::Result;
use opencad_feature::{apply_parameters, FeatureNode};
use opencad_file::read_ocad;
use opencad_geometry::{FaceDerivation, TopoRef};
use opencad_graph::evaluate_param_graph;
use opencad_render::{
    build_sketch_overlay, OffscreenRenderer, OrbitCamera, RenderImage, RenderScene, SketchOverlay,
};
use serde::{Deserialize, Serialize};

use crate::regen::tessellate_active_body_detailed;

pub const PREVIEW_WIDTH: u32 = 960;
pub const PREVIEW_HEIGHT: u32 = 540;

/// Serializable orbit camera pose (aspect is chosen at render time).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraState {
    pub target: [f32; 3],
    pub distance: f32,
    pub yaw_rad: f32,
    pub pitch_rad: f32,
    pub fov_y_deg: f32,
}

impl From<OrbitCamera> for CameraState {
    fn from(camera: OrbitCamera) -> Self {
        Self {
            target: camera.target,
            distance: camera.distance,
            yaw_rad: camera.yaw_rad,
            pitch_rad: camera.pitch_rad,
            fov_y_deg: camera.fov_y_deg,
        }
    }
}

impl CameraState {
    pub fn to_orbit_camera(&self, aspect: f32) -> OrbitCamera {
        OrbitCamera {
            target: self.target,
            distance: self.distance,
            yaw_rad: self.yaw_rad,
            pitch_rad: self.pitch_rad,
            fov_y_deg: self.fov_y_deg,
            aspect: aspect.max(0.1),
        }
    }
}

/// Document data prepared for viewport or PNG preview.
#[derive(Debug, Clone)]
pub struct ViewData {
    pub scene: RenderScene,
    pub overlay: SketchOverlay,
    pub name: String,
    pub feature_nodes: Vec<FeatureNode>,
    pub semantic_refs: Vec<TopoRef>,
    pub face_history: Vec<FaceDerivation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentPreview {
    pub name: String,
    pub path: String,
    pub triangles: usize,
    pub vertices: usize,
    pub bounds_min_m: [f32; 3],
    pub bounds_max_m: [f32; 3],
    pub sketch_count: usize,
    pub feature_count: usize,
    pub png_base64: String,
}

pub fn load_view_data(input: &str) -> Result<ViewData> {
    let doc = read_ocad(input)?;
    let name = doc.metadata.name.clone();
    let parameters = doc.parameters.clone();
    let feature_nodes = doc.feature_nodes.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    apply_parameters(&mut model, &parameters)?;
    let values = evaluate_param_graph(&parameters)?;
    let sketches: Vec<_> = model.sketches.values().cloned().collect();
    let overlay = build_sketch_overlay(&sketches, &values)?;
    let tessellated =
        tessellate_active_body_detailed(&mut model, Some(&parameters), Some(&semantic_refs))?;
    let scene = RenderScene::from_mesh_set(&tessellated.mesh_set)?;
    Ok(ViewData {
        scene,
        overlay,
        name,
        feature_nodes,
        semantic_refs,
        face_history: tessellated.face_history,
    })
}

pub fn preview_document(path: &str) -> Result<DocumentPreview> {
    let data = load_view_data(path)?;
    let png_base64 = render_preview_png(&data, None)?;
    let vertices = data
        .scene
        .meshes
        .iter()
        .map(|mesh| mesh.vertex_count())
        .sum();
    Ok(DocumentPreview {
        name: data.name,
        path: path.to_string(),
        triangles: data.scene.triangle_count(),
        vertices,
        bounds_min_m: data.scene.bounds.min,
        bounds_max_m: data.scene.bounds.max,
        sketch_count: data
            .feature_nodes
            .iter()
            .filter(|node| node.definition.feature_type() == "sketch")
            .count(),
        feature_count: data.feature_nodes.len(),
        png_base64,
    })
}

/// Render a PNG preview using the default or supplied camera pose.
pub fn render_preview_png(data: &ViewData, camera: Option<CameraState>) -> Result<String> {
    let renderer = OffscreenRenderer::new()?;
    let overlay = if data.overlay.is_empty() {
        None
    } else {
        Some(&data.overlay)
    };
    let aspect = PREVIEW_WIDTH as f32 / PREVIEW_HEIGHT as f32;
    let orbit = camera
        .map(|state| state.to_orbit_camera(aspect))
        .unwrap_or_else(|| data.scene.default_camera(aspect));
    let image = renderer.render_scene_image_with_camera(
        &data.scene,
        overlay,
        PREVIEW_WIDTH,
        PREVIEW_HEIGHT,
        &orbit,
    )?;
    encode_png_base64(&image)
}

fn encode_png_base64(image: &RenderImage) -> Result<String> {
    let buffer: RgbaImage =
        ImageBuffer::from_vec(image.width, image.height, image.rgba.clone()).ok_or_else(|| {
            opencad_core::OpenCadError::validation(format!(
                "invalid RGBA buffer for {}x{} image",
                image.width, image.height
            ))
        })?;
    let mut png_bytes = Vec::new();
    buffer
        .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
        .map_err(|err| opencad_core::OpenCadError::Other(format!("failed to encode PNG: {err}")))?;
    Ok(STANDARD.encode(png_bytes))
}
