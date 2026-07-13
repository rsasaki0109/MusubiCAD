//! GPU selection buffer for sketch overlays and solid mesh picking.

use bytemuck::{Pod, Zeroable};
use opencad_core::{OpenCadError, Result};
use wgpu::util::DeviceExt;

use crate::overlay::SketchOverlay;
use crate::solid::{create_depth_texture, create_uniform_bind_group, GpuVertex, Uniforms};

pub const HIGHLIGHT_LINE_COLOR: [f32; 4] = [0.2, 0.85, 1.0, 1.0];

const PICK_SHADER_SOURCE: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) selection_id: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) selection_id: u32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.view_proj * vec4<f32>(input.position, 1.0);
    output.selection_id = input.selection_id;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let id = input.selection_id;
    let r = f32((id >> 16u) & 0xffu) / 255.0;
    let g = f32((id >> 8u) & 0xffu) / 255.0;
    let b = f32(id & 0xffu) / 255.0;
    return vec4<f32>(r, g, b, 1.0);
}
"#;

/// Encoded pick target ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionId(pub u32);

impl SelectionId {
    pub const NONE: Self = Self(0);

    pub fn from_line_index(line_index: usize) -> Self {
        Self((line_index as u32).saturating_add(1))
    }

    pub fn from_triangle_index(triangle_index: usize, line_count: usize) -> Self {
        Self(line_count as u32 + triangle_index as u32 + 1)
    }

    pub fn line_index(self) -> Option<usize> {
        if self.0 == 0 {
            None
        } else {
            Some((self.0 - 1) as usize)
        }
    }
}

/// Result of a viewport pick query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickResult {
    None,
    SketchLine(usize),
    SolidTriangle(usize),
}

/// Maps pick IDs to sketch lines and solid triangles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionCatalog {
    pub line_count: usize,
    pub triangle_count: usize,
}

impl SelectionCatalog {
    pub fn from_scene(overlay: &SketchOverlay, triangle_count: usize) -> Self {
        Self {
            line_count: overlay.lines.len(),
            triangle_count,
        }
    }

    pub fn decode(self, id: SelectionId) -> PickResult {
        if id.0 == 0 {
            return PickResult::None;
        }
        if id.0 <= self.line_count as u32 {
            return PickResult::SketchLine((id.0 - 1) as usize);
        }
        let triangle_index = (id.0 - self.line_count as u32 - 1) as usize;
        if triangle_index < self.triangle_count {
            PickResult::SolidTriangle(triangle_index)
        } else {
            PickResult::None
        }
    }
}

pub fn selection_id_to_rgba(id: SelectionId) -> [u8; 4] {
    let value = id.0;
    [
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
        255,
    ]
}

pub fn rgba_to_selection_id(rgba: [u8; 4]) -> SelectionId {
    let value = (u32::from(rgba[0]) << 16) | (u32::from(rgba[1]) << 8) | u32::from(rgba[2]);
    SelectionId(value)
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PickVertex {
    pub position: [f32; 3],
    pub selection_id: u32,
}

pub struct PickDrawBuffers {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
}

pub fn overlay_pick_vertices(overlay: &SketchOverlay) -> Vec<PickVertex> {
    let mut vertices = Vec::new();
    for (line_index, line) in overlay.lines.iter().enumerate() {
        let selection_id = SelectionId::from_line_index(line_index).0;
        vertices.push(PickVertex {
            position: line.start,
            selection_id,
        });
        vertices.push(PickVertex {
            position: line.end,
            selection_id,
        });
    }
    vertices
}

pub(crate) fn mesh_pick_vertices(
    vertices: &[GpuVertex],
    indices: &[u32],
    line_count: usize,
) -> Vec<PickVertex> {
    let mut pick_vertices = Vec::with_capacity(indices.len());
    for (triangle_index, triangle) in indices.chunks_exact(3).enumerate() {
        let selection_id = SelectionId::from_triangle_index(triangle_index, line_count).0;
        for &index in triangle {
            pick_vertices.push(PickVertex {
                position: vertices[index as usize].position,
                selection_id,
            });
        }
    }
    pick_vertices
}

pub fn triangle_world_positions(
    scene: &crate::scene::RenderScene,
    triangle_index: usize,
) -> Option<[[f32; 3]; 3]> {
    let (vertices, indices) = crate::solid::pack_scene(scene).ok()?;
    let base = triangle_index.checked_mul(3)?;
    let i0 = *indices.get(base)? as usize;
    let i1 = *indices.get(base + 1)? as usize;
    let i2 = *indices.get(base + 2)? as usize;
    Some([
        vertices.get(i0)?.position,
        vertices.get(i1)?.position,
        vertices.get(i2)?.position,
    ])
}

/// Boundary edges of a tessellated face group (shared internal edges excluded).
pub fn face_group_boundary_edges(
    scene: &crate::scene::RenderScene,
    group_index: usize,
) -> Vec<([f32; 3], [f32; 3])> {
    use std::collections::HashMap;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct EdgeKey([i32; 3], [i32; 3]);

    #[derive(Debug, Clone, Copy)]
    struct EdgeCount {
        edge: ([f32; 3], [f32; 3]),
        count: u8,
    }

    fn quantize_axis(value: f32) -> i32 {
        (value * 1_000_000.0).round() as i32
    }

    fn quantize_point(point: [f32; 3]) -> [i32; 3] {
        [
            quantize_axis(point[0]),
            quantize_axis(point[1]),
            quantize_axis(point[2]),
        ]
    }

    fn canonical_edge(start: [f32; 3], end: [f32; 3]) -> (EdgeKey, ([f32; 3], [f32; 3])) {
        let start_q = quantize_point(start);
        let end_q = quantize_point(end);
        if start_q <= end_q {
            (EdgeKey(start_q, end_q), (start, end))
        } else {
            (EdgeKey(end_q, start_q), (start, end))
        }
    }

    let triangle_indices = scene.face_catalog.triangle_indices_in_group(group_index);
    let mut edge_counts: HashMap<EdgeKey, EdgeCount> = HashMap::new();

    for triangle_index in triangle_indices {
        let Some(vertices) = triangle_world_positions(scene, triangle_index) else {
            continue;
        };
        for (start, end) in [
            (vertices[0], vertices[1]),
            (vertices[1], vertices[2]),
            (vertices[2], vertices[0]),
        ] {
            let (key, edge) = canonical_edge(start, end);
            edge_counts
                .entry(key)
                .and_modify(|entry| entry.count += 1)
                .or_insert(EdgeCount { edge, count: 1 });
        }
    }

    edge_counts
        .into_values()
        .filter_map(|entry| (entry.count == 1).then_some(entry.edge))
        .collect()
}

/// Highlight edges for a picked face group, using ring outlines for cylindrical faces.
pub fn face_group_highlight_edges(
    scene: &crate::scene::RenderScene,
    group_index: usize,
) -> Vec<([f32; 3], [f32; 3])> {
    let role = scene
        .face_catalog
        .groups
        .get(group_index)
        .map(|group| group.role);
    match role {
        Some(crate::face_catalog::FaceRole::Cylindrical) => {
            cylindrical_ring_highlight_edges(scene, group_index)
        }
        _ => face_group_boundary_edges(scene, group_index),
    }
}

fn cylindrical_ring_highlight_edges(
    scene: &crate::scene::RenderScene,
    group_index: usize,
) -> Vec<([f32; 3], [f32; 3])> {
    const POINT_EPS_M: f32 = 0.00005;

    let mut vertices = Vec::new();
    for triangle_index in scene.face_catalog.triangle_indices_in_group(group_index) {
        if let Some(triangle) = triangle_world_positions(scene, triangle_index) {
            vertices.extend_from_slice(&triangle);
        }
    }
    if vertices.is_empty() {
        return Vec::new();
    }

    let frame = CylinderAxisFrame::from_vertices(&vertices);
    let min_level = extremal_coord_along_axis(&vertices, &frame, false);
    let max_level = extremal_coord_along_axis(&vertices, &frame, true);
    let mid_level = (min_level + max_level) * 0.5;
    let mut top_ring = split_ring_points_oblique(&vertices, &frame, mid_level, true, POINT_EPS_M);
    let mut bottom_ring =
        split_ring_points_oblique(&vertices, &frame, mid_level, false, POINT_EPS_M);
    if top_ring.len() < 3 {
        top_ring = extremal_ring_points_oblique(&vertices, &frame, max_level, POINT_EPS_M);
    }
    if bottom_ring.len() < 3 {
        bottom_ring = extremal_ring_points_oblique(&vertices, &frame, min_level, POINT_EPS_M);
    }

    let mut edges = ring_edges(&top_ring);
    edges.extend(ring_edges(&bottom_ring));

    if let (Some(bottom), Some(top)) = (bottom_ring.first(), top_ring.first()) {
        edges.push((*bottom, *top));
    }

    if edges.len() < 8 {
        return face_group_boundary_edges(scene, group_index);
    }
    edges
}

struct CylinderAxisFrame {
    origin: [f32; 3],
    axis: [f32; 3],
    u: [f32; 3],
    v: [f32; 3],
}

impl CylinderAxisFrame {
    fn from_vertices(vertices: &[[f32; 3]]) -> Self {
        let origin = vertex_mean(vertices);
        let axis = cylinder_axis_from_vertices(vertices, origin);
        let (u, v) = ring_basis(axis);
        Self { origin, axis, u, v }
    }

    fn coord_along_axis(&self, point: [f32; 3]) -> f32 {
        vec3_dot(vec3_sub(point, self.origin), self.axis)
    }

    fn ring_coords(&self, point: [f32; 3], center_along: f32) -> (f32, f32) {
        let center = vec3_add(self.origin, vec3_scale(self.axis, center_along));
        let delta = vec3_sub(point, center);
        (vec3_dot(delta, self.u), vec3_dot(delta, self.v))
    }
}

fn split_ring_points_oblique(
    vertices: &[[f32; 3]],
    frame: &CylinderAxisFrame,
    mid_level: f32,
    top: bool,
    point_eps: f32,
) -> Vec<[f32; 3]> {
    let mut points = Vec::new();
    for vertex in vertices {
        let coord = frame.coord_along_axis(*vertex);
        let keep = if top {
            coord >= mid_level
        } else {
            coord <= mid_level
        };
        if !keep {
            continue;
        }
        if points
            .iter()
            .any(|point| distance3(*point, *vertex) <= point_eps)
        {
            continue;
        }
        points.push(*vertex);
    }
    sort_ring_points_oblique(&mut points, frame, mid_level);
    points
}

fn extremal_ring_points_oblique(
    vertices: &[[f32; 3]],
    frame: &CylinderAxisFrame,
    level: f32,
    point_eps: f32,
) -> Vec<[f32; 3]> {
    let tolerance = ((extremal_coord_along_axis(vertices, frame, true)
        - extremal_coord_along_axis(vertices, frame, false))
        * 0.15)
        .max(0.0001);
    let mut points = Vec::new();
    for vertex in vertices {
        if (frame.coord_along_axis(*vertex) - level).abs() > tolerance {
            continue;
        }
        if points
            .iter()
            .any(|point| distance3(*point, *vertex) <= point_eps)
        {
            continue;
        }
        points.push(*vertex);
    }
    sort_ring_points_oblique(&mut points, frame, level);
    points
}

fn sort_ring_points_oblique(points: &mut [[f32; 3]], frame: &CylinderAxisFrame, center_along: f32) {
    if points.len() < 2 {
        return;
    }
    points.sort_by(|left, right| {
        let (lu, lv) = frame.ring_coords(*left, center_along);
        let (ru, rv) = frame.ring_coords(*right, center_along);
        lu.atan2(lv)
            .partial_cmp(&ru.atan2(rv))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn extremal_coord_along_axis(vertices: &[[f32; 3]], frame: &CylinderAxisFrame, max: bool) -> f32 {
    if max {
        vertices
            .iter()
            .map(|vertex| frame.coord_along_axis(*vertex))
            .fold(f32::NEG_INFINITY, f32::max)
    } else {
        vertices
            .iter()
            .map(|vertex| frame.coord_along_axis(*vertex))
            .fold(f32::INFINITY, f32::min)
    }
}

fn vertex_mean(vertices: &[[f32; 3]]) -> [f32; 3] {
    let mut sum = [0.0_f32; 3];
    for vertex in vertices {
        sum[0] += vertex[0];
        sum[1] += vertex[1];
        sum[2] += vertex[2];
    }
    let count = vertices.len().max(1) as f32;
    [sum[0] / count, sum[1] / count, sum[2] / count]
}

fn cylinder_axis_from_vertices(vertices: &[[f32; 3]], center: [f32; 3]) -> [f32; 3] {
    if vertices.len() < 3 {
        return [0.0, 0.0, 1.0];
    }
    let mut cov = [[0.0_f32; 3]; 3];
    for vertex in vertices {
        let delta = vec3_sub(*vertex, center);
        for (row, row_values) in cov.iter_mut().enumerate() {
            for (col, value) in row_values.iter_mut().enumerate() {
                *value += delta[row] * delta[col];
            }
        }
    }
    let count = vertices.len() as f32;
    for row in &mut cov {
        for value in row {
            *value /= count;
        }
    }
    smallest_eigenvector(&cov)
}

fn smallest_eigenvector(cov: &[[f32; 3]; 3]) -> [f32; 3] {
    let trace = cov[0][0] + cov[1][1] + cov[2][2];
    let mut vector = [1.0_f32, 0.0, 0.0];
    for _ in 0..48 {
        let next = [
            trace * vector[0]
                - (cov[0][0] * vector[0] + cov[0][1] * vector[1] + cov[0][2] * vector[2]),
            trace * vector[1]
                - (cov[1][0] * vector[0] + cov[1][1] * vector[1] + cov[1][2] * vector[2]),
            trace * vector[2]
                - (cov[2][0] * vector[0] + cov[2][1] * vector[1] + cov[2][2] * vector[2]),
        ];
        vector = normalize3(next);
    }
    vector
}

fn ring_basis(axis: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let reference = if axis[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let u = normalize3(vec3_cross(reference, axis));
    let v = vec3_cross(axis, u);
    (u, v)
}

fn vec3_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn vec3_scale(v: [f32; 3], scalar: f32) -> [f32; 3] {
    [v[0] * scalar, v[1] * scalar, v[2] * scalar]
}

fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len <= f32::EPSILON {
        return [0.0, 0.0, 1.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

fn ring_edges(ring: &[[f32; 3]]) -> Vec<([f32; 3], [f32; 3])> {
    if ring.len() < 2 {
        return Vec::new();
    }
    let mut edges = Vec::with_capacity(ring.len());
    for index in 0..ring.len() {
        let next = (index + 1) % ring.len();
        edges.push((ring[index], ring[next]));
    }
    edges
}

fn distance3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

pub(crate) fn triangle_edge_vertices(
    vertices: &[GpuVertex],
    indices: &[u32],
    triangle_index: usize,
) -> Option<Vec<[f32; 3]>> {
    let base = triangle_index.checked_mul(3)?;
    let i0 = *indices.get(base)? as usize;
    let i1 = *indices.get(base + 1)? as usize;
    let i2 = *indices.get(base + 2)? as usize;
    let p0 = vertices.get(i0)?.position;
    let p1 = vertices.get(i1)?.position;
    let p2 = vertices.get(i2)?.position;
    Some(vec![p0, p1, p1, p2, p2, p0])
}

pub fn create_pick_buffers(device: &wgpu::Device, vertices: &[PickVertex]) -> PickDrawBuffers {
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("pick-vertex-buffer"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    PickDrawBuffers {
        vertex_buffer,
        vertex_count: vertices.len() as u32,
    }
}

fn create_pick_pipeline(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    topology: wgpu::PrimitiveTopology,
    cull_mode: Option<wgpu::Face>,
    label: &str,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(PICK_SHADER_SOURCE.into()),
    });

    let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("pick-uniform-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pick-pipeline-layout"),
        bind_group_layouts: &[&uniform_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<PickVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Uint32],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    (pipeline, uniform_layout)
}

pub fn create_pick_mesh_pipeline(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    create_pick_pipeline(
        device,
        color_format,
        wgpu::PrimitiveTopology::TriangleList,
        Some(wgpu::Face::Back),
        "pick-mesh-pipeline",
    )
}

pub fn create_pick_line_pipeline(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    create_pick_pipeline(
        device,
        color_format,
        wgpu::PrimitiveTopology::LineList,
        None,
        "pick-line-pipeline",
    )
}

pub(crate) struct PickDrawPass<'a> {
    encoder: &'a mut wgpu::CommandEncoder,
    pipeline: &'a wgpu::RenderPipeline,
    bind_group: &'a wgpu::BindGroup,
    pick_buffers: &'a PickDrawBuffers,
    color_view: &'a wgpu::TextureView,
    depth_view: &'a wgpu::TextureView,
    clear_color: bool,
    clear_depth: bool,
}

pub(crate) fn encode_pick_draw_pass(pass: PickDrawPass<'_>) {
    let PickDrawPass {
        encoder,
        pipeline,
        bind_group,
        pick_buffers,
        color_view,
        depth_view,
        clear_color,
        clear_depth,
    } = pass;
    if pick_buffers.vertex_count == 0 {
        return;
    }

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("pick-draw-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: if clear_color {
                    wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                } else {
                    wgpu::LoadOp::Load
                },
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth_view,
            depth_ops: Some(wgpu::Operations {
                load: if clear_depth {
                    wgpu::LoadOp::Clear(1.0)
                } else {
                    wgpu::LoadOp::Load
                },
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.set_vertex_buffer(0, pick_buffers.vertex_buffer.slice(..));
    pass.draw(0..pick_buffers.vertex_count, 0..1);
}

pub(crate) struct ScenePickContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub pick_mesh_pipeline: &'a wgpu::RenderPipeline,
    pub pick_line_pipeline: &'a wgpu::RenderPipeline,
    pub pick_uniform_layout: &'a wgpu::BindGroupLayout,
    pub mesh_pick_buffers: &'a PickDrawBuffers,
    pub line_pick_buffers: &'a PickDrawBuffers,
    pub catalog: SelectionCatalog,
    pub view_proj: [f32; 16],
    pub width: u32,
    pub height: u32,
}

pub(crate) fn pick_scene(ctx: ScenePickContext<'_>, x: f64, y: f64) -> Result<PickResult> {
    let ScenePickContext {
        device,
        queue,
        pick_mesh_pipeline,
        pick_line_pipeline,
        pick_uniform_layout,
        mesh_pick_buffers,
        line_pick_buffers,
        catalog,
        view_proj,
        width,
        height,
    } = ctx;

    if catalog.triangle_count == 0 && catalog.line_count == 0 {
        return Ok(PickResult::None);
    }

    let color_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("pick-color-texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let depth_texture = create_depth_texture(device, width, height);
    let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let pick_bind_group = create_uniform_bind_group(
        device,
        pick_uniform_layout,
        &Uniforms {
            view_proj,
            eye_position: [0.0; 4],
        },
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("pick-encoder"),
    });

    if mesh_pick_buffers.vertex_count > 0 {
        encode_pick_draw_pass(PickDrawPass {
            encoder: &mut encoder,
            pipeline: pick_mesh_pipeline,
            bind_group: &pick_bind_group,
            pick_buffers: mesh_pick_buffers,
            color_view: &color_view,
            depth_view: &depth_view,
            clear_color: true,
            clear_depth: true,
        });
    }

    if line_pick_buffers.vertex_count > 0 {
        encode_pick_draw_pass(PickDrawPass {
            encoder: &mut encoder,
            pipeline: pick_line_pipeline,
            bind_group: &pick_bind_group,
            pick_buffers: line_pick_buffers,
            color_view: &color_view,
            depth_view: &depth_view,
            clear_color: mesh_pick_buffers.vertex_count == 0,
            clear_depth: mesh_pick_buffers.vertex_count == 0,
        });
    }

    let pixel_x = x.floor().clamp(0.0, (width.saturating_sub(1)) as f64) as u32;
    let pixel_y = y.floor().clamp(0.0, (height.saturating_sub(1)) as f64) as u32;
    let bytes_per_row = align_to(width * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pick-readback-buffer"),
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

    queue.submit(Some(encoder.finish()));
    device.poll(wgpu::Maintain::Wait);

    let slice = readback_buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    receiver
        .recv()
        .map_err(|_| OpenCadError::Other("pick buffer map cancelled".into()))?
        .map_err(|err| OpenCadError::Other(format!("pick buffer map failed: {err}")))?;

    let data = slice.get_mapped_range();
    let row_start = (pixel_y as usize) * bytes_per_row as usize;
    let col_start = (pixel_x as usize) * 4;
    let rgba = [
        data[row_start + col_start],
        data[row_start + col_start + 1],
        data[row_start + col_start + 2],
        data[row_start + col_start + 3],
    ];
    drop(data);
    readback_buffer.unmap();

    Ok(catalog.decode(rgba_to_selection_id(rgba)))
}

fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_id_round_trip_for_line_index() {
        for line_index in [0, 1, 42, 65534] {
            let id = SelectionId::from_line_index(line_index);
            assert_eq!(id.line_index(), Some(line_index));
            let rgba = selection_id_to_rgba(id);
            assert_eq!(rgba_to_selection_id(rgba), id);
        }
    }

    #[test]
    fn selection_id_round_trip_for_triangle_index() {
        let line_count = 12;
        for triangle_index in [0, 1, 500, 4096] {
            let id = SelectionId::from_triangle_index(triangle_index, line_count);
            let rgba = selection_id_to_rgba(id);
            assert_eq!(rgba_to_selection_id(rgba), id);
            let catalog = SelectionCatalog {
                line_count,
                triangle_count: triangle_index + 1,
            };
            assert_eq!(
                catalog.decode(id),
                PickResult::SolidTriangle(triangle_index)
            );
        }
    }

    #[test]
    fn catalog_prefers_sketch_line_over_triangle_namespace() {
        let catalog = SelectionCatalog {
            line_count: 3,
            triangle_count: 10,
        };
        assert_eq!(
            catalog.decode(SelectionId::from_line_index(1)),
            PickResult::SketchLine(1)
        );
        assert_eq!(
            catalog.decode(SelectionId::from_triangle_index(0, 3)),
            PickResult::SolidTriangle(0)
        );
        assert_eq!(catalog.decode(SelectionId::NONE), PickResult::None);
    }

    #[test]
    fn triangle_edge_vertices_form_closed_loop() {
        let vertices = vec![
            GpuVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                color: [0.72, 0.76, 0.82],
            },
            GpuVertex {
                position: [1.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                color: [0.72, 0.76, 0.82],
            },
            GpuVertex {
                position: [0.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                color: [0.72, 0.76, 0.82],
            },
        ];
        let indices = vec![0, 1, 2];
        let edges = triangle_edge_vertices(&vertices, &indices, 0).expect("edges");
        assert_eq!(edges.len(), 6);
    }

    #[test]
    fn face_group_boundary_edges_outline_planar_face() {
        use crate::scene::RenderScene;
        use opencad_geometry::MeshSet;

        let scene = RenderScene::from_mesh_set(&MeshSet::box_prism(0.08, 0.001)).expect("scene");
        let top_group = scene
            .face_catalog
            .groups
            .iter()
            .find(|group| group.role == crate::face_catalog::FaceRole::Top)
            .expect("top");
        let edges = face_group_highlight_edges(&scene, top_group.index);
        assert_eq!(edges.len(), 4);
    }

    #[test]
    fn oblique_cylinder_ring_highlight_forms_two_rings() {
        let axis = normalize3([1.0, 1.0, 1.0]);
        let (u, v) = ring_basis(axis);
        let radius = 0.01_f32;
        let height = 0.02_f32;
        let segments = 16_usize;
        let mut vertices = Vec::new();
        for ring in [0.0_f32, height] {
            for index in 0..segments {
                let angle = (index as f32 / segments as f32) * std::f32::consts::TAU;
                let offset = vec3_add(
                    vec3_scale(u, radius * angle.cos()),
                    vec3_scale(v, radius * angle.sin()),
                );
                let point = vec3_add(vec3_scale(axis, ring), offset);
                vertices.push(point);
            }
        }

        let frame = CylinderAxisFrame::from_vertices(&vertices);
        let min_level = extremal_coord_along_axis(&vertices, &frame, false);
        let max_level = extremal_coord_along_axis(&vertices, &frame, true);
        let mid_level = (min_level + max_level) * 0.5;
        let top_ring = split_ring_points_oblique(&vertices, &frame, mid_level, true, 0.00005);
        let bottom_ring = split_ring_points_oblique(&vertices, &frame, mid_level, false, 0.00005);

        assert!(top_ring.len() >= 8, "top ring points: {}", top_ring.len());
        assert!(
            bottom_ring.len() >= 8,
            "bottom ring points: {}",
            bottom_ring.len()
        );
        assert!(
            ring_edges(&top_ring).len() >= 8,
            "top ring edges: {}",
            ring_edges(&top_ring).len()
        );
    }

    #[test]
    fn pick_scene_returns_solid_triangle_at_center() {
        use crate::scene::RenderScene;
        use crate::solid::pack_scene;
        use opencad_geometry::MeshSet;

        let scene = RenderScene::from_mesh_set(&MeshSet::box_prism(0.08, 0.006)).expect("scene");
        let (vertices, indices) = pack_scene(&scene).expect("pack");
        let triangle_count = indices.len() / 3;
        let catalog = SelectionCatalog {
            line_count: 0,
            triangle_count,
        };
        let (device, queue) = pollster::block_on(async_device());
        let mesh_pick = mesh_pick_vertices(&vertices, &indices, 0);
        let mesh_buffers = create_pick_buffers(&device, &mesh_pick);
        let empty_lines = create_pick_buffers(&device, &[]);
        let (pick_mesh_pipeline, pick_uniform_layout) =
            create_pick_mesh_pipeline(&device, wgpu::TextureFormat::Rgba8Unorm);
        let (pick_line_pipeline, _) =
            create_pick_line_pipeline(&device, wgpu::TextureFormat::Rgba8Unorm);
        let aspect = 1.0;
        let view_proj = scene.default_camera(aspect).view_projection_matrix();

        let result = pick_scene(
            ScenePickContext {
                device: &device,
                queue: &queue,
                pick_mesh_pipeline: &pick_mesh_pipeline,
                pick_line_pipeline: &pick_line_pipeline,
                pick_uniform_layout: &pick_uniform_layout,
                mesh_pick_buffers: &mesh_buffers,
                line_pick_buffers: &empty_lines,
                catalog,
                view_proj,
                width: 256,
                height: 256,
            },
            128.0,
            128.0,
        )
        .expect("pick");

        assert!(matches!(result, PickResult::SolidTriangle(_)));
    }

    async fn async_device() -> (wgpu::Device, wgpu::Queue) {
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
            .expect("adapter");
        adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("pick-test-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .expect("device")
    }
}
