//! Interactive winit viewport for CAD scenes.

use std::sync::Arc;

use opencad_core::{OpenCadError, Result};
use winit::{
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowBuilder},
};

use crate::camera::OrbitCamera;
use crate::overlay::{
    label_depth_offset_for_bounds, label_scale_for_bounds, OverlayLabel, OverlayLine, SketchOverlay,
};
use crate::scene::RenderScene;
use crate::selection::{
    create_pick_buffers, create_pick_line_pipeline, create_pick_mesh_pipeline,
    face_group_highlight_edges, mesh_pick_vertices, overlay_pick_vertices, pick_scene,
    triangle_edge_vertices, PickDrawBuffers, PickResult, ScenePickContext, SelectionCatalog,
};
use crate::solid::{
    create_background_pipeline, create_depth_texture, create_label_line_pipeline,
    create_line_buffers, create_line_pipeline, create_mesh_buffers, create_solid_pipeline,
    create_uniform_bind_group, encode_background_pass, encode_sketch_overlay_passes,
    encode_solid_pass, pack_scene, LineBuffers, MeshBuffers, SketchOverlayPass, Uniforms,
};

const CLICK_THRESHOLD_PX: f64 = 5.0;

/// Callback invoked after a click-pick in the interactive viewport.
pub type ViewportPickCallback = Box<dyn Fn(f64, f64, u32, u32, PickResult) + Send>;

/// Callback invoked when the orbit camera changes in the interactive viewport.
pub type ViewportCameraCallback = Box<dyn Fn(OrbitCamera) + Send>;

/// Open an interactive viewport window for the given scene.
pub fn run_viewport(
    scene: &RenderScene,
    overlay: Option<&SketchOverlay>,
    title: &str,
) -> Result<()> {
    run_viewport_with_callbacks(scene, overlay, title, None, None)
}

/// Open an interactive viewport and optionally report click-picks.
pub fn run_viewport_with_pick(
    scene: &RenderScene,
    overlay: Option<&SketchOverlay>,
    title: &str,
    on_pick: Option<ViewportPickCallback>,
) -> Result<()> {
    run_viewport_with_callbacks(scene, overlay, title, on_pick, None)
}

/// Open an interactive viewport with optional pick and camera callbacks.
pub fn run_viewport_with_callbacks(
    scene: &RenderScene,
    overlay: Option<&SketchOverlay>,
    title: &str,
    on_pick: Option<ViewportPickCallback>,
    on_camera: Option<ViewportCameraCallback>,
) -> Result<()> {
    let (vertices, indices) = pack_scene(scene)?;
    let overlay_lines = overlay
        .map(|overlay| overlay.lines.clone())
        .unwrap_or_default();
    let model_lines = overlay.map(|overlay| overlay.model_line_vertices());
    let construction_lines = overlay.map(|overlay| overlay.construction_line_vertices());
    let has_overlay = overlay.is_some_and(|overlay| !overlay.is_empty());
    let triangle_count = indices.len() / 3;
    let selection_catalog =
        SelectionCatalog::from_scene(overlay.unwrap_or(&SketchOverlay::default()), triangle_count);
    let label_scale = label_scale_for_bounds(scene.bounds.diagonal());
    let label_depth_offset = label_depth_offset_for_bounds(scene.bounds.diagonal());
    let overlay_labels = overlay
        .map(|overlay| overlay.labels.clone())
        .unwrap_or_default();
    let dimension_lines = overlay
        .map(|overlay| overlay.dimension_lines.clone())
        .unwrap_or_default();
    let symbol_lines = overlay
        .map(|overlay| overlay.symbol_lines.clone())
        .unwrap_or_default();
    let event_loop = EventLoop::new()
        .map_err(|err| OpenCadError::Other(format!("failed to create event loop: {err}")))?;
    let window = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
        .build(&event_loop)
        .map_err(|err| OpenCadError::Other(format!("failed to create window: {err}")))?;

    let mut app = pollster::block_on(ViewportApp::new(ViewportInit {
        window: Arc::new(window),
        scene: scene.clone(),
        vertices,
        indices,
        model_lines: model_lines.unwrap_or_default(),
        construction_lines: construction_lines.unwrap_or_default(),
        overlay_labels,
        dimension_lines,
        symbol_lines,
        label_scale,
        label_depth_offset,
        overlay_lines,
        selection_catalog,
        has_overlay,
    }))?;
    let bounds = scene.bounds;

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);

            match event {
                Event::WindowEvent { window_id, event } if window_id == app.window.id() => {
                    match event {
                        WindowEvent::CloseRequested => elwt.exit(),
                        WindowEvent::Resized(size) => {
                            if let Err(err) = app.resize(size.width, size.height) {
                                eprintln!("resize error: {err}");
                                elwt.exit();
                            }
                        }
                        WindowEvent::RedrawRequested => {
                            if let Err(err) = app.render() {
                                eprintln!("render error: {err}");
                                elwt.exit();
                            }
                        }
                        WindowEvent::MouseInput { state, button, .. } => {
                            if button == MouseButton::Left {
                                match state {
                                    ElementState::Pressed => {
                                        app.dragging = true;
                                        app.press_origin = app.cursor_position;
                                        app.drag_origin = app.cursor_position;
                                    }
                                    ElementState::Released => {
                                        if app.did_orbit {
                                            if let Some(ref handler) = on_camera {
                                                handler(app.camera);
                                            }
                                            app.did_orbit = false;
                                        }
                                        if let Some(origin) = app.press_origin.take() {
                                            if let Some(cursor) = app.cursor_position {
                                                let dx = cursor.0 - origin.0;
                                                let dy = cursor.1 - origin.1;
                                                if dx * dx + dy * dy
                                                    <= CLICK_THRESHOLD_PX * CLICK_THRESHOLD_PX
                                                {
                                                    match app.pick_at(cursor.0, cursor.1) {
                                                        Ok(result) => {
                                                            match result {
                                                                PickResult::SketchLine(
                                                                    line_index,
                                                                ) => {
                                                                    app.set_selected_line(
                                                                        line_index,
                                                                    );
                                                                }
                                                                PickResult::SolidTriangle(
                                                                    triangle_index,
                                                                ) => {
                                                                    app.set_selected_triangle(
                                                                        triangle_index,
                                                                    );
                                                                }
                                                                PickResult::None => {}
                                                            }
                                                            app.window.request_redraw();
                                                            if let Some(ref handler) = on_pick {
                                                                handler(
                                                                    cursor.0,
                                                                    cursor.1,
                                                                    app.config.width,
                                                                    app.config.height,
                                                                    result,
                                                                );
                                                            }
                                                        }
                                                        Err(err) => eprintln!("pick error: {err}"),
                                                    }
                                                }
                                            }
                                        }
                                        app.dragging = false;
                                        app.drag_origin = None;
                                    }
                                }
                            }
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            app.cursor_position = Some((position.x, position.y));
                            if app.dragging {
                                if let Some((last_x, last_y)) = app.drag_origin {
                                    let dx = position.x - last_x;
                                    let dy = position.y - last_y;
                                    app.camera.yaw_rad += dx as f32 * 0.01;
                                    app.camera.pitch_rad =
                                        (app.camera.pitch_rad + dy as f32 * 0.01).clamp(-1.4, 1.4);
                                    app.did_orbit = true;
                                    app.window.request_redraw();
                                }
                                app.drag_origin = Some((position.x, position.y));
                            }
                        }
                        WindowEvent::MouseWheel { delta, .. } => {
                            let scroll = match delta {
                                winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                                winit::event::MouseScrollDelta::PixelDelta(pos) => {
                                    (pos.y / 120.0) as f32
                                }
                            };
                            let radius = bounds.diagonal() * 0.5;
                            let min_distance = radius.max(0.01) * 0.5;
                            let max_distance = radius.max(0.01) * 8.0;
                            app.camera.distance = (app.camera.distance * (1.0 - scroll * 0.1))
                                .clamp(min_distance, max_distance);
                            app.did_orbit = true;
                            if let Some(ref handler) = on_camera {
                                handler(app.camera);
                            }
                            app.window.request_redraw();
                        }
                        WindowEvent::KeyboardInput { event, .. }
                            if event.state == ElementState::Pressed
                                && event.logical_key == Key::Named(NamedKey::Escape) =>
                        {
                            elwt.exit();
                        }
                        _ => {}
                    }
                }
                Event::AboutToWait => {
                    app.window.request_redraw();
                }
                _ => {}
            }
        })
        .map_err(|err| OpenCadError::Other(format!("event loop failed: {err}")))?;

    Ok(())
}

struct ViewportInit {
    window: Arc<Window>,
    scene: RenderScene,
    vertices: Vec<crate::solid::GpuVertex>,
    indices: Vec<u32>,
    model_lines: Vec<[f32; 3]>,
    construction_lines: Vec<[f32; 3]>,
    overlay_labels: Vec<OverlayLabel>,
    dimension_lines: Vec<OverlayLine>,
    symbol_lines: Vec<OverlayLine>,
    label_scale: f32,
    label_depth_offset: f32,
    overlay_lines: Vec<OverlayLine>,
    selection_catalog: SelectionCatalog,
    has_overlay: bool,
}

struct ViewportApp {
    window: Arc<Window>,
    scene: RenderScene,
    camera: OrbitCamera,
    surface: wgpu::Surface<'static>,
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
    model_line_buffers: LineBuffers,
    construction_line_buffers: LineBuffers,
    label_line_buffers: LineBuffers,
    highlight_line_buffers: LineBuffers,
    pick_mesh_buffers: PickDrawBuffers,
    pick_line_buffers: PickDrawBuffers,
    cpu_vertices: Vec<crate::solid::GpuVertex>,
    cpu_indices: Vec<u32>,
    overlay_lines: Vec<OverlayLine>,
    overlay_labels: Vec<OverlayLabel>,
    dimension_lines: Vec<OverlayLine>,
    symbol_lines: Vec<OverlayLine>,
    label_scale: f32,
    label_depth_offset: f32,
    selection_catalog: SelectionCatalog,
    selected_line: Option<usize>,
    selected_triangle: Option<usize>,
    has_overlay: bool,
    mesh_buffers: MeshBuffers,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    dragging: bool,
    cursor_position: Option<(f64, f64)>,
    press_origin: Option<(f64, f64)>,
    drag_origin: Option<(f64, f64)>,
    did_orbit: bool,
}

impl ViewportApp {
    async fn new(init: ViewportInit) -> Result<Self> {
        let ViewportInit {
            window,
            scene,
            vertices,
            indices,
            model_lines,
            construction_lines,
            overlay_labels,
            dimension_lines,
            symbol_lines,
            label_scale,
            label_depth_offset,
            overlay_lines,
            selection_catalog,
            has_overlay,
        } = init;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .map_err(|err| OpenCadError::Other(format!("failed to create surface: {err}")))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| OpenCadError::Other("no wgpu adapter available".into()))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("opencad-viewport"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|err| OpenCadError::Other(format!("wgpu device init failed: {err}")))?;

        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .unwrap_or(capabilities.formats[0]);

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let background_pipeline = create_background_pipeline(&device, surface_format);
        let (pipeline, uniform_layout) = create_solid_pipeline(&device, surface_format);
        let (line_pipeline, line_uniform_layout) = create_line_pipeline(&device, surface_format);
        let (label_line_pipeline, _) = create_label_line_pipeline(&device, surface_format);
        let (pick_mesh_pipeline, pick_uniform_layout) =
            create_pick_mesh_pipeline(&device, wgpu::TextureFormat::Rgba8Unorm);
        let (pick_line_pipeline, _) =
            create_pick_line_pipeline(&device, wgpu::TextureFormat::Rgba8Unorm);
        let mesh_buffers = create_mesh_buffers(&device, &vertices, &indices);
        let model_line_buffers = create_line_buffers(&device, &model_lines);
        let construction_line_buffers = create_line_buffers(&device, &construction_lines);
        let label_line_buffers = create_line_buffers(&device, &[]);
        let highlight_line_buffers = create_line_buffers(&device, &[]);
        let mesh_pick_verts = mesh_pick_vertices(&vertices, &indices, selection_catalog.line_count);
        let line_pick_vertices = overlay_pick_vertices_from_lines(&overlay_lines);
        let pick_mesh_buffers = create_pick_buffers(&device, &mesh_pick_verts);
        let pick_line_buffers = create_pick_buffers(&device, &line_pick_vertices);
        let depth_texture = create_depth_texture(&device, config.width, config.height);
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let aspect = config.width as f32 / config.height.max(1) as f32;
        let camera = scene.default_camera(aspect);

        Ok(Self {
            window,
            scene,
            camera,
            surface,
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
            model_line_buffers,
            construction_line_buffers,
            label_line_buffers,
            highlight_line_buffers,
            pick_mesh_buffers,
            pick_line_buffers,
            cpu_vertices: vertices,
            cpu_indices: indices,
            overlay_lines,
            overlay_labels,
            dimension_lines,
            symbol_lines,
            label_scale,
            label_depth_offset,
            selection_catalog,
            selected_line: None,
            selected_triangle: None,
            has_overlay,
            mesh_buffers,
            config,
            depth_texture,
            depth_view,
            dragging: false,
            cursor_position: None,
            press_origin: None,
            drag_origin: None,
            did_orbit: false,
        })
    }

    fn rebuild_label_buffers(&mut self) {
        if self.overlay_labels.is_empty()
            && self.dimension_lines.is_empty()
            && self.symbol_lines.is_empty()
        {
            self.label_line_buffers = create_line_buffers(&self.device, &[]);
            return;
        }
        let overlay = SketchOverlay {
            dimension_lines: self.dimension_lines.clone(),
            symbol_lines: self.symbol_lines.clone(),
            labels: self.overlay_labels.clone(),
            ..Default::default()
        };
        let (right, up) = self.camera.billboard_basis();
        let depth_bias = Some((self.camera.eye_position(), self.label_depth_offset));
        let vertices =
            overlay.label_line_vertices_billboard(self.label_scale, right, up, depth_bias);
        self.label_line_buffers = create_line_buffers(&self.device, &vertices);
    }

    fn set_selected_line(&mut self, line_index: usize) {
        self.selected_line = Some(line_index);
        self.selected_triangle = None;
        let vertices = if let Some(line) = self.overlay_lines.get(line_index) {
            if line.segment_index.is_some() {
                if let Some(entity_id) = &line.entity_id {
                    self.overlay_lines
                        .iter()
                        .filter(|overlay| overlay.entity_id.as_deref() == Some(entity_id.as_str()))
                        .flat_map(|overlay| [overlay.start, overlay.end])
                        .collect()
                } else {
                    vec![line.start, line.end]
                }
            } else {
                vec![line.start, line.end]
            }
        } else {
            Vec::new()
        };
        self.highlight_line_buffers = create_line_buffers(&self.device, &vertices);
    }

    fn set_selected_triangle(&mut self, triangle_index: usize) {
        self.selected_triangle = Some(triangle_index);
        self.selected_line = None;
        let vertices = if let Some(face) = self.scene.face_group_at(triangle_index) {
            face_group_highlight_edges(&self.scene, face.index)
                .into_iter()
                .flat_map(|(start, end)| [start, end])
                .collect()
        } else {
            triangle_edge_vertices(&self.cpu_vertices, &self.cpu_indices, triangle_index)
                .unwrap_or_default()
        };
        self.highlight_line_buffers = create_line_buffers(&self.device, &vertices);
    }

    fn pick_at(&self, x: f64, y: f64) -> Result<PickResult> {
        pick_scene(
            ScenePickContext {
                device: &self.device,
                queue: &self.queue,
                pick_mesh_pipeline: &self.pick_mesh_pipeline,
                pick_line_pipeline: &self.pick_line_pipeline,
                pick_uniform_layout: &self.pick_uniform_layout,
                mesh_pick_buffers: &self.pick_mesh_buffers,
                line_pick_buffers: &self.pick_line_buffers,
                catalog: self.selection_catalog,
                view_proj: self.camera.view_projection_matrix(),
                width: self.config.width,
                height: self.config.height,
            },
            x,
            y,
        )
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.camera.aspect = width as f32 / height as f32;
        self.depth_texture = create_depth_texture(&self.device, width, height);
        self.depth_view = self
            .depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        Ok(())
    }

    fn render(&mut self) -> Result<()> {
        self.rebuild_label_buffers();

        let frame = self.surface.get_current_texture().map_err(|err| {
            OpenCadError::Other(format!("failed to acquire surface frame: {err}"))
        })?;
        let color_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let view_proj = self.camera.view_projection_matrix();
        let bind_group =
            create_uniform_bind_group(&self.device, &self.uniform_layout, &Uniforms { view_proj });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("viewport-encoder"),
            });

        let highlight_lines = if self.highlight_line_buffers.vertex_count > 0 {
            Some(&self.highlight_line_buffers)
        } else {
            None
        };
        let needs_overlay_pass = self.has_overlay || highlight_lines.is_some();

        encode_background_pass(&mut encoder, &self.background_pipeline, &color_view);

        encode_solid_pass(
            &mut encoder,
            &self.pipeline,
            &bind_group,
            &self.mesh_buffers,
            &color_view,
            &self.depth_view,
            needs_overlay_pass,
        );

        if needs_overlay_pass {
            encode_sketch_overlay_passes(SketchOverlayPass {
                encoder: &mut encoder,
                pipeline: &self.line_pipeline,
                label_pipeline: &self.label_line_pipeline,
                uniform_layout: &self.line_uniform_layout,
                device: &self.device,
                model_lines: &self.model_line_buffers,
                construction_lines: &self.construction_line_buffers,
                label_lines: if self.label_line_buffers.vertex_count > 0 {
                    Some(&self.label_line_buffers)
                } else {
                    None
                },
                view_proj,
                color_view: &color_view,
                depth_view: &self.depth_view,
                highlight_lines,
            });
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

fn overlay_pick_vertices_from_lines(lines: &[OverlayLine]) -> Vec<crate::selection::PickVertex> {
    let overlay = SketchOverlay {
        lines: lines.to_vec(),
        ..Default::default()
    };
    overlay_pick_vertices(&overlay)
}
