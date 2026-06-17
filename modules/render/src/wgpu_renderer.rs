//! Headless wgpu renderer for viewport previews and CI.

use std::path::Path;

use opencad_core::{OpenCadError, Result};

use crate::edges::feature_edge_vertices;
use crate::overlay::{label_depth_offset_for_bounds, label_scale_for_bounds, SketchOverlay};
use crate::png::write_png;
use crate::scene::RenderScene;
use crate::selection::{
    create_pick_buffers, create_pick_line_pipeline, create_pick_mesh_pipeline, mesh_pick_vertices,
    overlay_pick_vertices, pick_scene, ScenePickContext, SelectionCatalog,
};
use crate::solid::{
    background_srgb, create_background_pipeline, create_depth_texture, create_label_line_pipeline,
    create_line_bind_group, create_line_buffers, create_line_pipeline, create_mesh_buffers,
    create_solid_pipeline, create_uniform_bind_group, encode_background_pass, encode_line_pass,
    encode_sketch_overlay_passes, encode_solid_pass, pack_scene, SketchOverlayPass, Uniforms,
    EDGE_LINE_COLOR,
};

/// Crease angle (degrees) above which a shared edge is drawn as a feature edge.
const EDGE_CREASE_ANGLE_DEG: f32 = 25.0;

/// Color target for offscreen previews. sRGB so the shader's linear output is
/// gamma-encoded on store, matching the interactive viewport's sRGB surface.
const PREVIEW_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Tightly packed RGBA8 pixels from an offscreen render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub non_background_pixels: usize,
}

/// Summary of an offscreen render pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutput {
    pub width: u32,
    pub height: u32,
    pub non_background_pixels: usize,
}

impl From<RenderImage> for RenderOutput {
    fn from(image: RenderImage) -> Self {
        Self {
            width: image.width,
            height: image.height,
            non_background_pixels: image.non_background_pixels,
        }
    }
}

/// Headless renderer that draws into an offscreen texture.
pub struct OffscreenRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    background_pipeline: wgpu::RenderPipeline,
    pipeline: wgpu::RenderPipeline,
    uniform_layout: wgpu::BindGroupLayout,
    line_pipeline: wgpu::RenderPipeline,
    line_uniform_layout: wgpu::BindGroupLayout,
    label_line_pipeline: wgpu::RenderPipeline,
    pick_mesh_pipeline: wgpu::RenderPipeline,
    pick_line_pipeline: wgpu::RenderPipeline,
    pick_uniform_layout: wgpu::BindGroupLayout,
}

impl OffscreenRenderer {
    pub fn new() -> Result<Self> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            })
            .await
            .ok_or_else(|| OpenCadError::Other("no wgpu adapter available".into()))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("opencad-offscreen"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|err| OpenCadError::Other(format!("wgpu device init failed: {err}")))?;

        let color_format = PREVIEW_COLOR_FORMAT;
        let background_pipeline = create_background_pipeline(&device, color_format);
        let (pipeline, uniform_layout) = create_solid_pipeline(&device, color_format);
        let (line_pipeline, line_uniform_layout) = create_line_pipeline(&device, color_format);
        let (label_line_pipeline, _) = create_label_line_pipeline(&device, color_format);
        // Pick pipelines encode integer IDs as raw bytes, so they must use a
        // non-sRGB format to avoid gamma corruption of the IDs.
        let (pick_mesh_pipeline, pick_uniform_layout) =
            create_pick_mesh_pipeline(&device, wgpu::TextureFormat::Rgba8Unorm);
        let (pick_line_pipeline, _) =
            create_pick_line_pipeline(&device, wgpu::TextureFormat::Rgba8Unorm);

        Ok(Self {
            device,
            queue,
            background_pipeline,
            pipeline,
            uniform_layout,
            line_pipeline,
            line_uniform_layout,
            label_line_pipeline,
            pick_mesh_pipeline,
            pick_line_pipeline,
            pick_uniform_layout,
        })
    }

    pub fn render_scene(
        &self,
        scene: &RenderScene,
        width: u32,
        height: u32,
    ) -> Result<RenderOutput> {
        Ok(self.render_scene_image(scene, None, width, height)?.into())
    }

    pub fn render_scene_image(
        &self,
        scene: &RenderScene,
        overlay: Option<&SketchOverlay>,
        width: u32,
        height: u32,
    ) -> Result<RenderImage> {
        let aspect = width as f32 / height.max(1) as f32;
        self.render_scene_image_with_camera(
            scene,
            overlay,
            width,
            height,
            &scene.default_camera(aspect),
        )
    }

    pub fn render_scene_image_with_camera(
        &self,
        scene: &RenderScene,
        overlay: Option<&SketchOverlay>,
        width: u32,
        height: u32,
        camera: &crate::camera::OrbitCamera,
    ) -> Result<RenderImage> {
        let (vertices, indices) = pack_scene(scene)?;

        let mut camera = *camera;
        camera.aspect = width as f32 / height.max(1) as f32;
        let view_proj = camera.view_projection_matrix();
        let bind_group =
            create_uniform_bind_group(&self.device, &self.uniform_layout, &Uniforms { view_proj });
        let mesh_buffers = create_mesh_buffers(&self.device, &vertices, &indices);
        let model_lines = overlay
            .map(|overlay| create_line_buffers(&self.device, &overlay.model_line_vertices()))
            .unwrap_or_else(|| create_line_buffers(&self.device, &[]));
        let construction_lines = overlay
            .map(|overlay| create_line_buffers(&self.device, &overlay.construction_line_vertices()))
            .unwrap_or_else(|| create_line_buffers(&self.device, &[]));
        let label_scale = label_scale_for_bounds(scene.bounds.diagonal());
        let label_depth_offset = label_depth_offset_for_bounds(scene.bounds.diagonal());
        let (right, up) = camera.billboard_basis();
        let depth_bias = Some((camera.eye_position(), label_depth_offset));
        let label_lines = overlay
            .map(|overlay| {
                create_line_buffers(
                    &self.device,
                    &overlay.label_line_vertices_billboard(label_scale, right, up, depth_bias),
                )
            })
            .unwrap_or_else(|| create_line_buffers(&self.device, &[]));
        let has_overlay = overlay.is_some_and(|overlay| !overlay.is_empty());

        let color_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("color-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PREVIEW_COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let depth_texture = create_depth_texture(&self.device, width, height);
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-encoder"),
            });

        encode_background_pass(&mut encoder, &self.background_pipeline, &color_view);

        // Preserve depth so feature edges and overlays test against the solid.
        encode_solid_pass(
            &mut encoder,
            &self.pipeline,
            &bind_group,
            &mesh_buffers,
            &color_view,
            &depth_view,
            true,
        );

        // Feature edges (silhouette + creases) give every raised feature a CAD
        // outline regardless of the camera angle.
        let edge_vertices = feature_edge_vertices(&vertices, &indices, EDGE_CREASE_ANGLE_DEG);
        let edge_lines = create_line_buffers(&self.device, &edge_vertices);
        if edge_lines.vertex_count > 0 {
            let edge_bind_group = create_line_bind_group(
                &self.device,
                &self.line_uniform_layout,
                view_proj,
                EDGE_LINE_COLOR,
            );
            encode_line_pass(
                &mut encoder,
                &self.label_line_pipeline,
                &edge_bind_group,
                &edge_lines,
                &color_view,
                &depth_view,
            );
        }

        if has_overlay {
            encode_sketch_overlay_passes(SketchOverlayPass {
                encoder: &mut encoder,
                pipeline: &self.line_pipeline,
                label_pipeline: &self.label_line_pipeline,
                uniform_layout: &self.line_uniform_layout,
                device: &self.device,
                model_lines: &model_lines,
                construction_lines: &construction_lines,
                label_lines: if label_lines.vertex_count > 0 {
                    Some(&label_lines)
                } else {
                    None
                },
                view_proj,
                color_view: &color_view,
                depth_view: &depth_view,
                highlight_lines: None,
            });
        }

        let bytes_per_row = align_to(width * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback-buffer"),
            size: (bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &color_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        let slice = readback_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .map_err(|_| OpenCadError::Other("wgpu buffer map cancelled".into()))?
            .map_err(|err| OpenCadError::Other(format!("wgpu buffer map failed: {err}")))?;

        let data = slice.get_mapped_range();
        let non_background_pixels =
            count_non_background_pixels(&data, width, height, bytes_per_row);
        let rgba = unpack_rgba(&data, width, height, bytes_per_row);
        drop(data);
        readback_buffer.unmap();

        Ok(RenderImage {
            width,
            height,
            rgba,
            non_background_pixels,
        })
    }

    pub fn render_scene_png(
        &self,
        scene: &RenderScene,
        overlay: Option<&SketchOverlay>,
        width: u32,
        height: u32,
        output: impl AsRef<Path>,
    ) -> Result<RenderOutput> {
        let image = self.render_scene_image(scene, overlay, width, height)?;
        write_png(output.as_ref(), image.width, image.height, &image.rgba)?;
        Ok(image.into())
    }

    /// GPU pick at viewport pixel coordinates using the default orbit camera.
    pub fn pick_scene_at(
        &self,
        scene: &RenderScene,
        overlay: Option<&SketchOverlay>,
        x: f64,
        y: f64,
        width: u32,
        height: u32,
    ) -> Result<crate::selection::PickResult> {
        let (vertices, indices) = pack_scene(scene)?;
        let catalog = SelectionCatalog::from_scene(
            overlay.unwrap_or(&SketchOverlay::default()),
            indices.len() / 3,
        );
        let aspect = width as f32 / height.max(1) as f32;
        let view_proj = scene.default_camera(aspect).view_projection_matrix();
        let mesh_pick = mesh_pick_vertices(&vertices, &indices, catalog.line_count);
        let line_pick = overlay.map(overlay_pick_vertices).unwrap_or_default();
        let mesh_buffers = create_pick_buffers(&self.device, &mesh_pick);
        let line_buffers = create_pick_buffers(&self.device, &line_pick);

        pick_scene(
            ScenePickContext {
                device: &self.device,
                queue: &self.queue,
                pick_mesh_pipeline: &self.pick_mesh_pipeline,
                pick_line_pipeline: &self.pick_line_pipeline,
                pick_uniform_layout: &self.pick_uniform_layout,
                mesh_pick_buffers: &mesh_buffers,
                line_pick_buffers: &line_buffers,
                catalog,
                view_proj,
                width,
                height,
            },
            x,
            y,
        )
    }
}

fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

fn unpack_rgba(data: &[u8], width: u32, height: u32, bytes_per_row: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        let row_start = (y * bytes_per_row) as usize;
        for x in 0..width {
            let offset = row_start + (x * 4) as usize;
            rgba.extend_from_slice(&data[offset..offset + 4]);
        }
    }
    rgba
}

/// Count pixels that differ from the gradient backdrop, i.e. covered by the
/// model or its overlays. The backdrop is reconstructed per row from the same
/// endpoints the shader uses, with a small tolerance for rounding.
fn count_non_background_pixels(data: &[u8], width: u32, height: u32, bytes_per_row: u32) -> usize {
    const TOLERANCE: i32 = 14;

    let mut count = 0_usize;
    for y in 0..height {
        let [bg_r, bg_g, bg_b] = background_srgb(y, height);
        let row_start = (y * bytes_per_row) as usize;
        for x in 0..width {
            let offset = row_start + (x * 4) as usize;
            let pixel = &data[offset..offset + 4];
            let dr = (pixel[0] as i32 - bg_r as i32).abs();
            let dg = (pixel[1] as i32 - bg_g as i32).abs();
            let db = (pixel[2] as i32 - bg_b as i32).abs();
            if dr > TOLERANCE || dg > TOLERANCE || db > TOLERANCE {
                count += 1;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::build_sketch_overlay;
    use opencad_feature::{apply_parameters, bracket_with_hole};
    use opencad_geometry::MeshSet;
    use opencad_graph::bracket_parameters;
    use tempfile::tempdir;

    #[test]
    fn offscreen_renders_scene_pixels() {
        let renderer = OffscreenRenderer::new().expect("renderer");
        let scene = RenderScene::from_mesh_set(&MeshSet::box_prism(0.01, 0.001)).expect("scene");
        let output = renderer
            .render_scene(&scene, 256, 256)
            .expect("render scene");
        assert_eq!(output.width, 256);
        assert!(output.non_background_pixels > 0);
    }

    #[test]
    fn offscreen_writes_png_file() {
        let renderer = OffscreenRenderer::new().expect("renderer");
        let scene = RenderScene::from_mesh_set(&MeshSet::box_prism(0.01, 0.001)).expect("scene");
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("preview.png");
        let output = renderer
            .render_scene_png(&scene, None, 256, 256, &path)
            .expect("render png");
        assert!(output.non_background_pixels > 0);
        let bytes = std::fs::read(&path).expect("read png");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn offscreen_overlay_increases_visible_pixels() {
        let mut model = bracket_with_hole().expect("model");
        let params = bracket_parameters();
        apply_parameters(&mut model, &params).expect("apply");
        let values = opencad_graph::evaluate_param_graph(&params).expect("eval");
        let sketches: Vec<_> = model.sketches.values().cloned().collect();
        let overlay = build_sketch_overlay(&sketches, &values).expect("overlay");
        let scene = RenderScene::from_mesh_set(&MeshSet::box_prism(0.08, 0.001)).expect("scene");

        let renderer = OffscreenRenderer::new().expect("renderer");
        let without = renderer
            .render_scene(&scene, 256, 256)
            .expect("without overlay");
        let with = renderer
            .render_scene_image(&scene, Some(&overlay), 256, 256)
            .expect("with overlay");
        assert!(with.non_background_pixels >= without.non_background_pixels);
    }
}
