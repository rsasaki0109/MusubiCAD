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

/// Look of a preview image.  [`PreviewStyle::PLAIN`] is the agent preview;
/// `studio` and `dark` are presentation looks for shared images and GIFs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewStyle {
    /// Background at the top and bottom edges (0–1 RGB), blended per row.
    pub background_top: [f32; 3],
    pub background_bottom: [f32; 3],
    pub outline: [f32; 3],
    /// Darkening under the model's soft ground shadow, 0–1; 0 draws none.
    pub shadow_opacity: f32,
}

impl PreviewStyle {
    /// Flat light background, no shadow.
    pub const PLAIN: Self = Self {
        background_top: BACKGROUND,
        background_bottom: BACKGROUND,
        outline: OUTLINE,
        shadow_opacity: 0.0,
    };
    /// Light grey gradient with a soft ground shadow.
    pub const STUDIO: Self = Self {
        background_top: [0.975, 0.98, 0.988],
        background_bottom: [0.84, 0.86, 0.895],
        outline: OUTLINE,
        shadow_opacity: 0.38,
    };
    /// Dark slate gradient with a soft ground shadow.
    pub const DARK: Self = Self {
        background_top: [0.17, 0.19, 0.24],
        background_bottom: [0.055, 0.063, 0.082],
        outline: [0.03, 0.035, 0.05],
        shadow_opacity: 0.55,
    };

    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "plain" => Ok(Self::PLAIN),
            "studio" => Ok(Self::STUDIO),
            "dark" => Ok(Self::DARK),
            other => Err(OpenCadError::validation(format!(
                "unknown preview style '{other}'; expected plain, studio, or dark"
            ))),
        }
    }

    fn has_shadow(&self) -> bool {
        self.shadow_opacity > 0.0
    }

    fn background(&self, row: usize, rows: usize) -> [f32; 3] {
        let t = row as f32 / (rows.max(2) - 1) as f32;
        std::array::from_fn(|channel| {
            self.background_top[channel]
                + (self.background_bottom[channel] - self.background_top[channel]) * t
        })
    }
}

impl Default for PreviewStyle {
    fn default() -> Self {
        Self::PLAIN
    }
}

/// Direction toward the light that casts ground shadows: high overhead and
/// behind the iso camera's line of sight, so shadows fall toward the viewer
/// (+X, -Y) where they are visible instead of hiding behind the part, and a
/// tall model's shadow stays about a third of its height long.
const SHADOW_LIGHT: [f32; 3] = [-0.2, 0.3, 1.0];
/// Shadow blur radius as a fraction of the image height.
const SHADOW_SOFTNESS: f32 = 0.035;

/// `point` dropped along the shadow light onto the plane `z = ground`.
fn shadow_point(point: [f32; 3], ground: f32) -> [f32; 3] {
    let drop = (point[2] - ground) / SHADOW_LIGHT[2];
    [
        point[0] - SHADOW_LIGHT[0] * drop,
        point[1] - SHADOW_LIGHT[1] * drop,
        ground,
    ]
}

/// Lowest Z of every vertex, the plane shadows fall on.
fn ground_z(meshes: &[PreviewMesh<'_>]) -> f32 {
    meshes
        .iter()
        .flat_map(|item| &item.mesh.positions)
        .map(|position| position[2])
        .fold(f32::INFINITY, f32::min)
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
    /// Also fit each included frame's ground shadow.
    shadow: bool,
}

impl PreviewFraming {
    /// Empty framing for `view`; grow it with [`PreviewFraming::include`].
    pub fn new(view: PreviewView) -> Self {
        Self {
            view,
            min: [f32::INFINITY; 2],
            max: [f32::NEG_INFINITY; 2],
            bottom_inset: 0.0,
            shadow: false,
        }
    }

    /// Framing that fits `meshes` seen from `view`.
    pub fn fit(view: PreviewView, meshes: &[PreviewMesh<'_>]) -> Self {
        let mut framing = Self::new(view);
        framing.include(meshes);
        framing
    }

    /// Grow the framing to also fit `meshes` (and their ground shadow when
    /// the framing was made [`with_shadow`](Self::with_shadow)).
    pub fn include(&mut self, meshes: &[PreviewMesh<'_>]) {
        let (right, up, _) = self.view.basis();
        let ground = ground_z(meshes);
        for item in meshes {
            for position in &item.mesh.positions {
                let shadow = self.shadow.then(|| shadow_point(*position, ground));
                for point in std::iter::once(*position).chain(shadow) {
                    let (u, v) = (dot(point, right), dot(point, up));
                    self.min = [self.min[0].min(u), self.min[1].min(v)];
                    self.max = [self.max[0].max(u), self.max[1].max(v)];
                }
            }
        }
    }

    /// Fit ground shadows too; use with a style that draws them.
    pub fn with_shadow(mut self, shadow: bool) -> Self {
        self.shadow = shadow;
        self
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
    render_preview_styled(meshes, framing, width, height, &PreviewStyle::PLAIN)
}

/// Render meshes with a fixed `framing` and a presentation `style`.
pub fn render_preview_styled(
    meshes: &[PreviewMesh<'_>],
    framing: &PreviewFraming,
    width: u32,
    height: u32,
    style: &PreviewStyle,
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
    let mut color: Vec<[f32; 3]> = (0..h)
        .flat_map(|row| std::iter::repeat(style.background(row, h)).take(w))
        .collect();
    let mut normal = vec![[0.0f32; 3]; w * h];

    if style.has_shadow() {
        let ground = ground_z(meshes);
        let mut mask = vec![0.0f32; w * h];
        for item in meshes {
            let mesh = item.mesh;
            for triangle in mesh.indices.chunks_exact(3) {
                let projected = [0, 1, 2].map(|corner| {
                    let point = shadow_point(mesh.positions[triangle[corner] as usize], ground);
                    let (u, v, _) = project(point);
                    let (x, y) = to_pixel(u, v);
                    (x, y, 0.0)
                });
                rasterize(&projected, w, h, |index, _| mask[index] = 1.0);
            }
        }
        let radius = ((h as f32 * SHADOW_SOFTNESS).round() as usize).max(1);
        for _ in 0..2 {
            box_blur(&mut mask, w, h, radius);
        }
        for (pixel, coverage) in color.iter_mut().zip(&mask) {
            let keep = 1.0 - style.shadow_opacity * coverage;
            *pixel = pixel.map(|channel| channel * keep);
        }
    }

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
                outlined[index] = style.outline;
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

/// Separable box blur of a `w` × `h` mask with the given radius in pixels.
fn box_blur(mask: &mut [f32], w: usize, h: usize, radius: usize) {
    let mut line = Vec::new();
    for (stride, count, length) in [(1, h, w), (w, w, h)] {
        for start in 0..count {
            let base = if stride == 1 { start * w } else { start };
            line.clear();
            line.extend((0..length).map(|i| mask[base + i * stride]));
            let mut sum: f32 = line.iter().take(radius + 1).sum();
            for i in 0..length {
                let lo = i.saturating_sub(radius);
                let hi = (i + radius).min(length - 1);
                mask[base + i * stride] = sum / (hi - lo + 1) as f32;
                if i + radius + 1 < length {
                    sum += line[i + radius + 1];
                }
                if i >= radius {
                    sum -= line[i - radius];
                }
            }
        }
    }
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
    fn studio_style_casts_a_soft_shadow_and_plain_matches_the_default() {
        let mesh = cuboid([0.0, 0.0, 0.0], [0.02, 0.02, 0.03]);
        let item = [PreviewMesh {
            mesh: &mesh,
            color: [0.2, 0.5, 0.85],
        }];
        let plain = render_preview(&item, PreviewView::Iso, 120, 90).expect("plain");
        let framing = PreviewFraming::fit(PreviewView::Iso, &item);
        assert_eq!(
            render_preview_styled(&item, &framing, 120, 90, &PreviewStyle::PLAIN)
                .expect("styled plain"),
            plain
        );

        let mut framing = PreviewFraming::new(PreviewView::Iso).with_shadow(true);
        framing.include(&item);
        let studio =
            render_preview_styled(&item, &framing, 120, 90, &PreviewStyle::STUDIO).expect("studio");
        let without = render_preview_styled(
            &item,
            &framing,
            120,
            90,
            &PreviewStyle {
                shadow_opacity: 0.0,
                ..PreviewStyle::STUDIO
            },
        )
        .expect("no shadow");
        // Some background pixels darken under the shadow; none lighten.
        let darker = studio
            .rgba
            .chunks_exact(4)
            .zip(without.rgba.chunks_exact(4))
            .filter(|(a, b)| a[0] + 3 < b[0])
            .count();
        assert!(darker > 50, "{darker} shadowed pixels");
        assert!(studio.rgba.iter().zip(&without.rgba).all(|(a, b)| a <= b));
        // The gradient runs light to dark from top to bottom.
        assert!(pixel(&without, 1, 1)[0] > pixel(&without, 1, 88)[0]);
        assert_eq!(
            PreviewStyle::parse("dark").expect("dark"),
            PreviewStyle::DARK
        );
        assert!(PreviewStyle::parse("neon").is_err());
    }

    #[test]
    fn box_blur_spreads_and_preserves_a_uniform_field() {
        let mut uniform = vec![1.0; 12];
        box_blur(&mut uniform, 4, 3, 1);
        assert!(uniform.iter().all(|value| (value - 1.0).abs() < 1e-6));
        let mut dot = vec![0.0; 25];
        dot[12] = 1.0;
        box_blur(&mut dot, 5, 5, 1);
        assert!((dot[12] - 1.0 / 9.0).abs() < 1e-6);
        assert!(dot[6] > 0.0 && dot[0] == 0.0);
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
