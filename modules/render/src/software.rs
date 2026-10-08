//! CPU preview renderer (ADR-031).
//!
//! Renders tessellated meshes to an RGBA image without a GPU, so agents on
//! headless machines and in cloud containers can look at what they built.
//! Orthographic, +Z up as in the model, two-sided Lambert shading, a depth
//! buffer, and dark outlines where depth or surface normal jumps. Rendered at
//! 2× and box-filtered down for smooth edges. Pure f32 math: deterministic for
//! a given platform.

use opencad_core::{OpenCadError, Result};
use opencad_geometry::MeshSet;

/// Camera direction for a preview, in model axes with +Z up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewView {
    /// From +X, -Y, +Z looking at the centre.
    Iso,
    /// From -Y looking along +Y (X right, Z up).
    Front,
    /// From +Z looking down (X right, Y up).
    Top,
    /// From +X looking along -X (Y right, Z up).
    Right,
}

impl PreviewView {
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "iso" => Ok(Self::Iso),
            "front" => Ok(Self::Front),
            "top" => Ok(Self::Top),
            "right" => Ok(Self::Right),
            other => Err(OpenCadError::validation(format!(
                "unknown preview view '{other}'; expected iso, front, top, or right"
            ))),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Iso => "iso",
            Self::Front => "front",
            Self::Top => "top",
            Self::Right => "right",
        }
    }

    /// (right, up, forward) unit vectors; forward points into the screen.
    fn basis(self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        match self {
            Self::Front => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
            Self::Top => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]),
            Self::Right => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [-1.0, 0.0, 0.0]),
            Self::Iso => {
                let forward = normalize([-1.0, 1.0, -1.0]);
                let right = normalize(cross(forward, [0.0, 0.0, 1.0]));
                let up = cross(right, forward);
                (right, up, forward)
            }
        }
    }
}

/// One mesh to draw and its base colour (linear 0–1 RGB).
pub struct PreviewMesh<'a> {
    pub mesh: &'a MeshSet,
    pub color: [f32; 3],
}

/// RGBA8 image, row-major from the top-left.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

const SUPERSAMPLE: usize = 2;
const BACKGROUND: [f32; 3] = [0.965, 0.972, 0.984];
const OUTLINE: [f32; 3] = [0.16, 0.19, 0.24];
/// Normals more than ~35° apart across neighbouring pixels draw an outline.
const CREASE_COS: f32 = 0.82;
const MARGIN: f32 = 0.08;

/// Screen-space extent a preview is framed to.  Fitting it once to every
/// frame of an animation keeps the camera still while the model moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewFraming {
    view: PreviewView,
    min: [f32; 2],
    max: [f32; 2],
    /// Fraction of the image height kept clear at the bottom (for a caption).
    bottom_inset: f32,
}

impl PreviewFraming {
    /// Empty framing for `view`; grow it with [`PreviewFraming::include`].
    pub fn new(view: PreviewView) -> Self {
        Self {
            view,
            min: [f32::INFINITY; 2],
            max: [f32::NEG_INFINITY; 2],
            bottom_inset: 0.0,
        }
    }

    /// Framing that fits `meshes` seen from `view`.
    pub fn fit(view: PreviewView, meshes: &[PreviewMesh<'_>]) -> Self {
        let mut framing = Self::new(view);
        framing.include(meshes);
        framing
    }

    /// Grow the framing to also fit `meshes`.
    pub fn include(&mut self, meshes: &[PreviewMesh<'_>]) {
        let (right, up, _) = self.view.basis();
        for item in meshes {
            for position in &item.mesh.positions {
                let (u, v) = (dot(*position, right), dot(*position, up));
                self.min = [self.min[0].min(u), self.min[1].min(v)];
                self.max = [self.max[0].max(u), self.max[1].max(v)];
            }
        }
    }

    pub fn view(&self) -> PreviewView {
        self.view
    }

    /// Keep `fraction` (clamped to 0–0.5) of the image height clear at the
    /// bottom, so a caption there does not cover the model.
    pub fn with_bottom_inset(mut self, fraction: f32) -> Self {
        self.bottom_inset = if fraction.is_finite() {
            fraction.clamp(0.0, 0.5)
        } else {
            0.0
        };
        self
    }
}

/// Render meshes from `view` into a `width` × `height` image.
pub fn render_preview(
    meshes: &[PreviewMesh<'_>],
    view: PreviewView,
    width: u32,
    height: u32,
) -> Result<PreviewImage> {
    render_preview_framed(meshes, &PreviewFraming::fit(view, meshes), width, height)
}

/// Render meshes into a `width` × `height` image with a fixed `framing`.
pub fn render_preview_framed(
    meshes: &[PreviewMesh<'_>],
    framing: &PreviewFraming,
    width: u32,
    height: u32,
) -> Result<PreviewImage> {
    if !(16..=2048).contains(&width) || !(16..=2048).contains(&height) {
        return Err(OpenCadError::validation(
            "preview size must be between 16 and 2048 pixels",
        ));
    }
    let (right, up, forward) = framing.view.basis();
    let project = |p: [f32; 3]| (dot(p, right), dot(p, up), dot(p, forward));
    let (min, max) = (framing.min, framing.max);
    if !min[0].is_finite() || meshes.iter().all(|item| item.mesh.indices.is_empty()) {
        return Err(OpenCadError::validation(
            "nothing to preview: the model has no triangles",
        ));
    }

    let (w, h) = (width as usize * SUPERSAMPLE, height as usize * SUPERSAMPLE);
    let span = [(max[0] - min[0]).max(1e-9), (max[1] - min[1]).max(1e-9)];
    let inset = framing.bottom_inset;
    let scale = ((w as f32) * (1.0 - 2.0 * MARGIN) / span[0])
        .min((h as f32) * (1.0 - 2.0 * MARGIN - inset) / span[1]);
    let centre = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
    let to_pixel = |u: f32, v: f32| {
        (
            (u - centre[0]) * scale + w as f32 * 0.5,
            h as f32 * (1.0 - inset) * 0.5 - (v - centre[1]) * scale,
        )
    };

    let light_a = normalize([0.35, -0.55, 0.75]);
    let light_b = normalize([-0.6, 0.3, 0.4]);
    let mut depth = vec![f32::INFINITY; w * h];
    let mut color = vec![BACKGROUND; w * h];
    let mut normal = vec![[0.0f32; 3]; w * h];

    for item in meshes {
        let mesh = item.mesh;
        for triangle in mesh.indices.chunks_exact(3) {
            let corners = [
                mesh.positions[triangle[0] as usize],
                mesh.positions[triangle[1] as usize],
                mesh.positions[triangle[2] as usize],
            ];
            let face_normal = normalize(cross(
                sub(corners[1], corners[0]),
                sub(corners[2], corners[0]),
            ));
            if face_normal == [0.0; 3] {
                continue;
            }
            let shade = 0.32
                + 0.58 * dot(face_normal, light_a).abs()
                + 0.18 * dot(face_normal, light_b).abs();
            let shaded = item.color.map(|channel| (channel * shade).min(1.0));
            let projected = corners.map(|corner| {
                let (u, v, d) = project(corner);
                let (x, y) = to_pixel(u, v);
                (x, y, d)
            });
            rasterize(&projected, w, h, |index, d| {
                if d < depth[index] {
                    depth[index] = d;
                    color[index] = shaded;
                    normal[index] = face_normal;
                }
            });
        }
    }

    // Outlines where the surface folds or one part occludes another.
    let depth_jump = 0.02 * span[0].max(span[1]);
    let mut outlined = color.clone();
    for y in 0..h {
        for x in 0..w {
            let index = y * w + x;
            let here = depth[index];
            let mut edge = false;
            for (dx, dy) in [(1usize, 0usize), (0, 1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= w || ny >= h {
                    continue;
                }
                let other = ny * w + nx;
                let (a, b) = (here, depth[other]);
                // Silhouette against the background, an occluding step, or a crease.
                let silhouette = a.is_finite() != b.is_finite();
                let step_or_crease = a.is_finite()
                    && b.is_finite()
                    && ((a - b).abs() > depth_jump
                        || dot(normal[index], normal[other]) < CREASE_COS);
                if silhouette || step_or_crease {
                    edge = true;
                }
            }
            if edge {
                outlined[index] = OUTLINE;
            }
        }
    }

    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let mut sum = [0.0f32; 3];
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let sample = outlined[(y * SUPERSAMPLE + sy) * w + x * SUPERSAMPLE + sx];
                    for channel in 0..3 {
                        sum[channel] += sample[channel];
                    }
                }
            }
            let samples = (SUPERSAMPLE * SUPERSAMPLE) as f32;
            for value in sum {
                rgba.push((value / samples * 255.0).round().clamp(0.0, 255.0) as u8);
            }
            rgba.push(255);
        }
    }
    Ok(PreviewImage {
        width,
        height,
        rgba,
    })
}

/// Fill a screen-space triangle, calling `plot(pixel_index, depth)` for
/// every covered pixel centre.
fn rasterize(corners: &[(f32, f32, f32); 3], w: usize, h: usize, mut plot: impl FnMut(usize, f32)) {
    let [(x0, y0, d0), (x1, y1, d1), (x2, y2, d2)] = *corners;
    let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    if area.abs() < 1e-12 {
        return;
    }
    let min_x = x0.min(x1).min(x2).floor().max(0.0) as usize;
    let max_x = (x0.max(x1).max(x2).ceil() as isize).clamp(0, w as isize - 1) as usize;
    let min_y = y0.min(y1).min(y2).floor().max(0.0) as usize;
    let max_y = (y0.max(y1).max(y2).ceil() as isize).clamp(0, h as isize - 1) as usize;
    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
            let w0 = ((x1 - x) * (y2 - y) - (x2 - x) * (y1 - y)) / area;
            let w1 = ((x2 - x) * (y0 - y) - (x0 - x) * (y2 - y)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            plot(py * w + px, w0 * d0 + w1 * d1 + w2 * d2);
        }
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt();
    if length <= f32::EPSILON {
        [0.0; 3]
    } else {
        v.map(|component| component / length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Axis-aligned box from `min` to `max`, two triangles per face.
    fn cuboid(min: [f32; 3], max: [f32; 3]) -> MeshSet {
        let corner = |i: usize| {
            [
                if i & 1 == 0 { min[0] } else { max[0] },
                if i & 2 == 0 { min[1] } else { max[1] },
                if i & 4 == 0 { min[2] } else { max[2] },
            ]
        };
        let faces = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 4, 6, 2],
            [1, 3, 7, 5],
        ];
        let mut mesh = MeshSet {
            positions: (0..8).map(corner).collect(),
            normals: vec![[0.0, 0.0, 1.0]; 8],
            indices: Vec::new(),
            triangle_face_ids: Vec::new(),
        };
        for [a, b, c, d] in faces {
            mesh.indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
        mesh
    }

    fn pixel(image: &PreviewImage, x: u32, y: u32) -> [u8; 3] {
        let index = ((y * image.width + x) * 4) as usize;
        [
            image.rgba[index],
            image.rgba[index + 1],
            image.rgba[index + 2],
        ]
    }

    fn is_background(rgb: [u8; 3]) -> bool {
        rgb.iter()
            .zip(BACKGROUND)
            .all(|(value, bg)| (*value as f32 - bg * 255.0).abs() < 2.0)
    }

    #[test]
    fn a_box_fills_the_centre_and_leaves_the_corners_empty() {
        let mesh = cuboid([0.0, 0.0, 0.0], [0.08, 0.06, 0.01]);
        let item = [PreviewMesh {
            mesh: &mesh,
            color: [0.2, 0.5, 0.85],
        }];
        for view in [
            PreviewView::Iso,
            PreviewView::Front,
            PreviewView::Top,
            PreviewView::Right,
        ] {
            let image = render_preview(&item, view, 160, 120).expect("render");
            assert_eq!(image.rgba.len(), 160 * 120 * 4);
            assert!(
                !is_background(pixel(&image, 80, 60)),
                "{}: centre",
                view.name()
            );
            assert!(
                is_background(pixel(&image, 1, 1)),
                "{}: corner",
                view.name()
            );
        }
    }

    #[test]
    fn the_top_view_frames_the_long_side_horizontally() {
        // 80 × 20 mm plate seen from above fills the width, not the height.
        let mesh = cuboid([0.0, 0.0, 0.0], [0.08, 0.02, 0.005]);
        let item = [PreviewMesh {
            mesh: &mesh,
            color: [0.2, 0.5, 0.85],
        }];
        let image = render_preview(&item, PreviewView::Top, 200, 200).expect("render");
        assert!(!is_background(pixel(&image, 30, 100)));
        assert!(is_background(pixel(&image, 100, 30)));
    }

    #[test]
    fn a_shared_framing_keeps_a_still_part_in_place() {
        // The left box renders the same with or without a far-away second box
        // only when both frames share one framing.
        let left = cuboid([0.0, 0.0, 0.0], [0.02, 0.02, 0.02]);
        let right = cuboid([0.08, 0.0, 0.0], [0.1, 0.02, 0.02]);
        let alone = [PreviewMesh {
            mesh: &left,
            color: [0.2, 0.5, 0.85],
        }];
        let both = [
            PreviewMesh {
                mesh: &left,
                color: [0.2, 0.5, 0.85],
            },
            PreviewMesh {
                mesh: &right,
                color: [0.9, 0.5, 0.1],
            },
        ];
        let mut framing = PreviewFraming::fit(PreviewView::Front, &alone);
        framing.include(&both);
        let first = render_preview_framed(&alone, &framing, 200, 100).expect("alone");
        let second = render_preview_framed(&both, &framing, 200, 100).expect("both");
        for (x, y) in [(30, 50), (100, 50)] {
            assert_eq!(pixel(&first, x, y), pixel(&second, x, y), "({x}, {y})");
        }
        assert!(!is_background(pixel(&first, 30, 50)));
        assert!(is_background(pixel(&first, 170, 50)));
        assert!(!is_background(pixel(&second, 170, 50)));
    }

    #[test]
    fn a_bottom_inset_lifts_the_model_clear_of_the_band() {
        let mesh = cuboid([0.0, 0.0, 0.0], [0.02, 0.02, 0.02]);
        let item = [PreviewMesh {
            mesh: &mesh,
            color: [0.2, 0.5, 0.85],
        }];
        let framing = PreviewFraming::fit(PreviewView::Front, &item).with_bottom_inset(0.3);
        let image = render_preview_framed(&item, &framing, 100, 100).expect("render");
        // The bottom 30 % stays background; the model fills above it.
        for y in 72..100 {
            assert!(is_background(pixel(&image, 50, y)), "row {y}");
        }
        assert!(!is_background(pixel(&image, 50, 35)));
    }

    #[test]
    fn rendering_is_deterministic_and_validates_inputs() {
        let mesh = cuboid([0.0; 3], [0.01; 3]);
        let item = [PreviewMesh {
            mesh: &mesh,
            color: [0.8, 0.4, 0.1],
        }];
        let first = render_preview(&item, PreviewView::Iso, 64, 48).expect("render");
        assert_eq!(
            first,
            render_preview(&item, PreviewView::Iso, 64, 48).expect("again")
        );
        assert!(render_preview(&item, PreviewView::Iso, 8, 48).is_err());
        let empty = MeshSet {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            triangle_face_ids: Vec::new(),
        };
        assert!(render_preview(
            &[PreviewMesh {
                mesh: &empty,
                color: [0.5; 3]
            }],
            PreviewView::Iso,
            64,
            48
        )
        .is_err());
        assert_eq!(PreviewView::parse("top").expect("top"), PreviewView::Top);
        assert!(PreviewView::parse("bottom").is_err());
    }
}
