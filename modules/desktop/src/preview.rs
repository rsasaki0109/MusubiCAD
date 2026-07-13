//! Load documents and render PNG previews for the desktop shell.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{ImageBuffer, ImageFormat, RgbaImage};
use opencad_assembly::{
    detect_interferences, regenerate_assembly, tessellate_assembly_instances, ChildPart,
    ResolvedChild,
};
use opencad_core::{DocumentKind, Result};
use opencad_feature::{apply_parameters, FeatureNode, FeatureRegistry};
use opencad_file::{read_ocad, OcadDocument};
use opencad_geometry::{FaceDerivation, TessellationSettings, TopoRef};
use opencad_graph::evaluate_param_graph;
use opencad_render::{
    build_sketch_overlay, OffscreenRenderer, OrbitCamera, RenderImage, RenderScene, SketchOverlay,
};
use opencad_sketch::Sketch;
use serde::{Deserialize, Serialize};

use crate::regen::tessellate_active_body_detailed;

pub const PREVIEW_WIDTH: u32 = 960;
pub const PREVIEW_HEIGHT: u32 = 540;

const INSTANCE_COLORS: [[f32; 4]; 6] = [
    [0.45, 0.65, 0.85, 1.0],
    [0.85, 0.55, 0.35, 1.0],
    [0.55, 0.78, 0.45, 1.0],
    [0.78, 0.45, 0.65, 1.0],
    [0.65, 0.55, 0.85, 1.0],
    [0.85, 0.75, 0.35, 1.0],
];

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
    pub sketches: BTreeMap<String, Sketch>,
    pub parameter_name_to_id: BTreeMap<String, String>,
    pub semantic_refs: Vec<TopoRef>,
    pub face_history: Vec<FaceDerivation>,
    pub parameter_ids: Vec<String>,
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
    if doc.metadata.kind == DocumentKind::Assembly {
        return load_assembly_view_data(input, doc, name);
    }

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
    let parameter_ids = parameters.evaluation_order()?;
    let parameter_name_to_id = parameter_ids
        .iter()
        .filter_map(|id| {
            parameters
                .get(id)
                .map(|entry| (entry.name.clone(), entry.id.clone()))
        })
        .collect();
    let sketches = model.sketches.into_iter().collect::<BTreeMap<_, _>>();
    Ok(ViewData {
        scene,
        overlay,
        name,
        feature_nodes,
        sketches,
        parameter_name_to_id,
        semantic_refs,
        face_history: tessellated.face_history,
        parameter_ids,
    })
}

fn load_assembly_view_data(input: &str, doc: OcadDocument, name: String) -> Result<ViewData> {
    let parameters = doc.parameters.clone();
    let parameter_ids = parameters.evaluation_order().unwrap_or_default();
    let parameter_name_to_id = parameter_ids
        .iter()
        .filter_map(|id| {
            parameters
                .get(id)
                .map(|entry| (entry.name.clone(), entry.id.clone()))
        })
        .collect();

    #[cfg(feature = "occt")]
    {
        let (scene, _) = load_assembly_scene_from_document(input, &doc)?;
        Ok(ViewData {
            scene,
            overlay: SketchOverlay::default(),
            name,
            feature_nodes: Vec::new(),
            sketches: BTreeMap::new(),
            parameter_name_to_id,
            semantic_refs: Vec::new(),
            face_history: Vec::new(),
            parameter_ids,
        })
    }

    #[cfg(not(feature = "occt"))]
    {
        let assembly = doc.assembly.as_ref().ok_or_else(|| {
            opencad_core::OpenCadError::validation("assembly document missing model")
        })?;
        let _ = (
            input,
            doc,
            assembly,
            name,
            parameter_name_to_id,
            parameter_ids,
        );
        Err(opencad_core::OpenCadError::Other(
            "OCCT backend disabled; rebuild with --features occt".into(),
        ))
    }
}

/// Regenerate an assembly document and return its viewport scene and exact interference count.
#[cfg(feature = "occt")]
pub fn load_assembly_scene_from_document(
    input: &str,
    doc: &OcadDocument,
) -> Result<(RenderScene, usize)> {
    use opencad_kernel_occt::OcctGeometryKernel;

    let assembly = doc
        .assembly
        .as_ref()
        .ok_or_else(|| opencad_core::OpenCadError::validation("assembly document missing model"))?;
    let assembly_root = assembly_root_for_path(Path::new(input));
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let report = regenerate_assembly(
        assembly,
        &doc.metadata.id,
        &assembly_root,
        &kernel,
        &registry,
        &mut load_child_document,
    )?;
    let instance_meshes =
        tessellate_assembly_instances(&kernel, &report.scene, &TessellationSettings::default())?;
    let mesh_sets: Vec<_> = instance_meshes
        .iter()
        .map(|instance| instance.mesh_set.clone())
        .collect();
    let colors: Vec<_> = instance_meshes
        .iter()
        .enumerate()
        .map(|(index, _)| INSTANCE_COLORS[index % INSTANCE_COLORS.len()])
        .collect();
    let render_scene = RenderScene::from_mesh_sets_with_colors(&mesh_sets, Some(&colors))?;
    let interference_count = detect_interferences(&kernel, &report.scene, 1e-12)?.len();
    Ok((render_scene, interference_count))
}

fn assembly_root_for_path(path: &Path) -> PathBuf {
    if path.extension().and_then(|ext| ext.to_str()) == Some("ocad") {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| Path::new(".").to_path_buf())
    } else {
        path.to_path_buf()
    }
}

fn load_child_document(path: &Path) -> Result<ResolvedChild> {
    let doc = read_ocad(path)?;
    if doc.metadata.kind == DocumentKind::Assembly {
        let assembly = doc.assembly.ok_or_else(|| {
            opencad_core::OpenCadError::validation(format!(
                "assembly document '{}' is missing assembly model",
                path.display()
            ))
        })?;
        Ok(ResolvedChild::Assembly {
            model: Box::new(assembly),
            doc_id: doc.metadata.id,
        })
    } else {
        let parameters = doc.parameters.clone();
        let semantic_refs = doc.semantic_refs.clone();
        let part = doc.into_part_model();
        Ok(ResolvedChild::Part(Box::new(ChildPart {
            parameters,
            part,
            semantic_refs,
        })))
    }
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
    let buffer: RgbaImage = ImageBuffer::from_vec(image.width, image.height, image.rgba.clone())
        .ok_or_else(|| {
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
