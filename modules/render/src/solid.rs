//! Shared solid mesh rendering pipeline for viewport and offscreen paths.

use bytemuck::{Pod, Zeroable};
use opencad_core::{OpenCadError, Result};
use wgpu::util::DeviceExt;

use crate::mesh::RenderMesh;
use crate::scene::RenderScene;

pub(crate) const SHADER_SOURCE: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) ao: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) ao: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.view_proj * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.ao = input.ao;
    return output;
}

// Studio three-point lighting (key + fill + rim) on a brushed-steel base, with
// baked ambient occlusion darkening concave junctions (boss bases, bores).
// Output is linear; the sRGB color target applies gamma encoding on store.
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(input.normal);
    let key_dir = normalize(vec3<f32>(0.45, 0.80, 0.55));
    let fill_dir = normalize(vec3<f32>(-0.65, 0.25, 0.35));
    let rim_dir = normalize(vec3<f32>(-0.25, 0.45, -0.85));

    let ao = clamp(input.ao, 0.0, 1.0);
    let key = max(dot(n, key_dir), 0.0);
    let fill = max(dot(n, fill_dir), 0.0) * 0.38;
    let rim = pow(max(dot(n, rim_dir), 0.0), 2.5) * 0.55;

    let base = vec3<f32>(0.52, 0.57, 0.66);
    let ambient = 0.24;
    // AO attenuates ambient and fill (the directionless terms) most strongly,
    // and the key light a little, so creases stay readable without going black.
    let diffuse = ambient * ao + key * 0.95 * (0.35 + 0.65 * ao) + fill * ao;
    let lit = base * diffuse + vec3<f32>(0.55, 0.68, 0.92) * rim;
    return vec4<f32>(clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
"#;

/// Initial clear written before the gradient backdrop is drawn over it.
pub(crate) const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.05,
    g: 0.06,
    b: 0.08,
    a: 1.0,
};

/// Vertical gradient backdrop endpoints in linear space (top, bottom).
/// Kept in sync with the WGSL background shader and `background_srgb` so the
/// offscreen non-background heuristic can reconstruct the backdrop on the CPU.
pub(crate) const BG_TOP_LINEAR: [f32; 3] = [0.040, 0.050, 0.072];
pub(crate) const BG_BOTTOM_LINEAR: [f32; 3] = [0.115, 0.140, 0.200];

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB-encoded backdrop color for framebuffer row `y` of `height` rows.
pub(crate) fn background_srgb(y: u32, height: u32) -> [u8; 3] {
    let t = (y as f32 + 0.5) / height.max(1) as f32;
    let mut out = [0_u8; 3];
    for channel in 0..3 {
        let lin = BG_TOP_LINEAR[channel] + (BG_BOTTOM_LINEAR[channel] - BG_TOP_LINEAR[channel]) * t;
        out[channel] = (linear_to_srgb(lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    out
}

fn background_shader_source() -> String {
    format!(
        r#"
struct VsOut {{
    @builtin(position) clip: vec4<f32>,
    @location(0) t: f32,
}}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {{
    // Fullscreen triangle covering the viewport.
    var xy = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    let p = xy[idx];
    var out: VsOut;
    out.clip = vec4<f32>(p, 0.0, 1.0);
    out.t = (1.0 - p.y) * 0.5;
    return out;
}}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {{
    let top = vec3<f32>({tr}, {tg}, {tb});
    let bottom = vec3<f32>({br}, {bg}, {bb});
    let color = mix(top, bottom, clamp(in.t, 0.0, 1.0));
    return vec4<f32>(color, 1.0);
}}
"#,
        tr = BG_TOP_LINEAR[0],
        tg = BG_TOP_LINEAR[1],
        tb = BG_TOP_LINEAR[2],
        br = BG_BOTTOM_LINEAR[0],
        bg = BG_BOTTOM_LINEAR[1],
        bb = BG_BOTTOM_LINEAR[2],
    )
}

/// Pipeline that draws the gradient backdrop with a fullscreen triangle.
pub(crate) fn create_background_pipeline(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("opencad-background"),
        source: wgpu::ShaderSource::Wgsl(background_shader_source().into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("background-pipeline-layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("background-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
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
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Draw the gradient backdrop, replacing the previous color clear.
pub(crate) fn encode_background_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    color_view: &wgpu::TextureView,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("background-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.draw(0..3, 0..1);
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub ao: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Uniforms {
    pub view_proj: [f32; 16],
}

pub(crate) struct MeshBuffers {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

pub(crate) fn pack_scene(scene: &RenderScene) -> Result<(Vec<GpuVertex>, Vec<u32>)> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut base = 0_u32;

    for mesh in &scene.meshes {
        append_mesh(mesh, &mut vertices, &mut indices, &mut base);
    }

    if vertices.is_empty() || indices.is_empty() {
        return Err(OpenCadError::validation("scene has no triangles to render"));
    }

    Ok((vertices, indices))
}

/// Hemisphere samples per vertex for the baked AO term.
const AO_SAMPLES: usize = 24;
/// Occlusion search radius as a fraction of the mesh bounding diagonal.
const AO_RADIUS_FRACTION: f32 = 0.16;
/// How dark a fully occluded vertex becomes (0 = off, 1 = full).
const AO_STRENGTH: f32 = 0.85;

fn append_mesh(
    mesh: &RenderMesh,
    vertices: &mut Vec<GpuVertex>,
    indices: &mut Vec<u32>,
    base: &mut u32,
) {
    let normals: Vec<[f32; 3]> = (0..mesh.positions.len())
        .map(|index| mesh.normals.get(index).copied().unwrap_or([0.0, 0.0, 1.0]))
        .collect();
    let radius = mesh_diagonal(&mesh.positions) * AO_RADIUS_FRACTION;
    let ao = crate::ao::compute_vertex_ao(
        &mesh.positions,
        &normals,
        &mesh.indices,
        radius,
        AO_SAMPLES,
        AO_STRENGTH,
    );

    for (index, position) in mesh.positions.iter().enumerate() {
        vertices.push(GpuVertex {
            position: *position,
            normal: normals[index],
            ao: ao.get(index).copied().unwrap_or(1.0),
        });
    }
    for index in &mesh.indices {
        indices.push(*base + index);
    }
    *base += mesh.positions.len() as u32;
}

fn mesh_diagonal(positions: &[[f32; 3]]) -> f32 {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(p[axis]);
            max[axis] = max[axis].max(p[axis]);
        }
    }
    let d = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

pub(crate) fn create_solid_pipeline(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("opencad-solid"),
        source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
    });

    let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("uniform-layout"),
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
        label: Some("pipeline-layout"),
        bind_group_layouts: &[&uniform_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("solid-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32],
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
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
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

pub(crate) fn create_mesh_buffers(
    device: &wgpu::Device,
    vertices: &[GpuVertex],
    indices: &[u32],
) -> MeshBuffers {
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vertex-buffer"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("index-buffer"),
        contents: bytemuck::cast_slice(indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    MeshBuffers {
        vertex_buffer,
        index_buffer,
        index_count: indices.len() as u32,
    }
}

pub(crate) fn create_uniform_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &Uniforms,
) -> wgpu::BindGroup {
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("uniform-buffer"),
        contents: bytemuck::bytes_of(uniforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("uniform-bind-group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    })
}

pub(crate) fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth-texture"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

pub(crate) fn encode_solid_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
    mesh_buffers: &MeshBuffers,
    color_view: &wgpu::TextureView,
    depth_view: &wgpu::TextureView,
    preserve_depth: bool,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("solid-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                // The gradient backdrop is drawn first via encode_background_pass.
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: if preserve_depth {
                    wgpu::StoreOp::Store
                } else {
                    wgpu::StoreOp::Discard
                },
            }),
            stencil_ops: None,
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.set_vertex_buffer(0, mesh_buffers.vertex_buffer.slice(..));
    pass.set_index_buffer(
        mesh_buffers.index_buffer.slice(..),
        wgpu::IndexFormat::Uint32,
    );
    pass.draw_indexed(0..mesh_buffers.index_count, 0, 0..1);
}

pub(crate) const LINE_SHADER_SOURCE: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> @builtin(position) vec4<f32> {
    return uniforms.view_proj * vec4<f32>(input.position, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return uniforms.color;
}
"#;

/// Dark feature edges drawn over the shaded solid (linear; sRGB-encoded on
/// store to a near-black graphite line).
pub(crate) const EDGE_LINE_COLOR: [f32; 4] = [0.02, 0.025, 0.035, 1.0];
pub(crate) const MODEL_LINE_COLOR: [f32; 4] = [1.0, 0.55, 0.1, 1.0];
pub(crate) const CONSTRUCTION_LINE_COLOR: [f32; 4] = [0.55, 0.58, 0.62, 0.85];
pub(crate) const LABEL_LINE_COLOR: [f32; 4] = [0.95, 0.92, 0.55, 1.0];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct LineUniforms {
    pub view_proj: [f32; 16],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct LineGpuVertex {
    pub position: [f32; 3],
}

pub(crate) struct LineBuffers {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
}

pub(crate) fn create_line_pipeline(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    create_line_pipeline_with_depth_bias(device, color_format, wgpu::DepthBiasState::default())
}

pub(crate) fn create_label_line_pipeline(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    create_line_pipeline_with_depth_bias(
        device,
        color_format,
        wgpu::DepthBiasState {
            constant: -4,
            slope_scale: -1.0,
            clamp: 0.0,
        },
    )
}

fn create_line_pipeline_with_depth_bias(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    depth_bias: wgpu::DepthBiasState,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("opencad-sketch-lines"),
        source: wgpu::ShaderSource::Wgsl(LINE_SHADER_SOURCE.into()),
    });

    let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("line-uniform-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("line-pipeline-layout"),
        bind_group_layouts: &[&uniform_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sketch-line-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<LineGpuVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: depth_bias,
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    (pipeline, uniform_layout)
}

pub(crate) fn create_line_buffers(device: &wgpu::Device, vertices: &[[f32; 3]]) -> LineBuffers {
    let gpu_vertices: Vec<LineGpuVertex> = vertices
        .iter()
        .map(|position| LineGpuVertex {
            position: *position,
        })
        .collect();

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("line-vertex-buffer"),
        contents: bytemuck::cast_slice(&gpu_vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    LineBuffers {
        vertex_buffer,
        vertex_count: gpu_vertices.len() as u32,
    }
}

pub(crate) fn encode_line_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
    line_buffers: &LineBuffers,
    color_view: &wgpu::TextureView,
    depth_view: &wgpu::TextureView,
) {
    if line_buffers.vertex_count == 0 {
        return;
    }

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("sketch-line-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Discard,
            }),
            stencil_ops: None,
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.set_vertex_buffer(0, line_buffers.vertex_buffer.slice(..));
    pass.draw(0..line_buffers.vertex_count, 0..1);
}

pub(crate) fn create_line_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view_proj: [f32; 16],
    color: [f32; 4],
) -> wgpu::BindGroup {
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("line-uniform-buffer"),
        contents: bytemuck::bytes_of(&LineUniforms { view_proj, color }),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("line-bind-group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    })
}

pub(crate) struct SketchOverlayPass<'a> {
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub pipeline: &'a wgpu::RenderPipeline,
    pub label_pipeline: &'a wgpu::RenderPipeline,
    pub uniform_layout: &'a wgpu::BindGroupLayout,
    pub device: &'a wgpu::Device,
    pub model_lines: &'a LineBuffers,
    pub construction_lines: &'a LineBuffers,
    pub label_lines: Option<&'a LineBuffers>,
    pub view_proj: [f32; 16],
    pub color_view: &'a wgpu::TextureView,
    pub depth_view: &'a wgpu::TextureView,
    pub highlight_lines: Option<&'a LineBuffers>,
}

pub(crate) fn encode_sketch_overlay_passes(pass: SketchOverlayPass<'_>) {
    let SketchOverlayPass {
        encoder,
        pipeline,
        label_pipeline,
        uniform_layout,
        device,
        model_lines,
        construction_lines,
        label_lines,
        view_proj,
        color_view,
        depth_view,
        highlight_lines,
    } = pass;
    if model_lines.vertex_count > 0 {
        let bind_group =
            create_line_bind_group(device, uniform_layout, view_proj, MODEL_LINE_COLOR);
        encode_line_pass(
            encoder,
            pipeline,
            &bind_group,
            model_lines,
            color_view,
            depth_view,
        );
    }

    if construction_lines.vertex_count > 0 {
        let bind_group =
            create_line_bind_group(device, uniform_layout, view_proj, CONSTRUCTION_LINE_COLOR);
        encode_line_pass(
            encoder,
            pipeline,
            &bind_group,
            construction_lines,
            color_view,
            depth_view,
        );
    }

    if let Some(label_lines) = label_lines {
        if label_lines.vertex_count > 0 {
            let bind_group =
                create_line_bind_group(device, uniform_layout, view_proj, LABEL_LINE_COLOR);
            encode_line_pass(
                encoder,
                label_pipeline,
                &bind_group,
                label_lines,
                color_view,
                depth_view,
            );
        }
    }

    if let Some(highlight_lines) = highlight_lines {
        let bind_group = create_line_bind_group(
            device,
            uniform_layout,
            view_proj,
            crate::selection::HIGHLIGHT_LINE_COLOR,
        );
        encode_line_pass(
            encoder,
            pipeline,
            &bind_group,
            highlight_lines,
            color_view,
            depth_view,
        );
    }
}
