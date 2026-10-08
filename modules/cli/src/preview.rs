//! `musubicad preview`: a GPU-free PNG of a part or assembly (ADR-031).
//!
//! Shared by the CLI, the Agent API (`opencad.preview_document`), and the MCP
//! `preview_document` tool, which returns the PNG as image content so an agent
//! can look at the geometry it just changed.

use opencad_core::{OpenCadError, Result};
use opencad_file::read_ocad;
use opencad_geometry::MeshSet;
use opencad_render::{
    caption_extent, draw_caption, encode_png, render_preview_styled, write_gif_frames,
    CaptionCorner, PreviewFraming, PreviewMesh, PreviewStyle, PreviewView, RenderImage,
};
use serde::{Deserialize, Serialize};

/// Part colour, then one colour per assembly instance in order.
const PALETTE: [[f32; 3]; 6] = [
    [0.13, 0.47, 0.84],
    [0.95, 0.55, 0.15],
    [0.20, 0.70, 0.45],
    [0.62, 0.40, 0.82],
    [0.85, 0.30, 0.35],
    [0.30, 0.33, 0.38],
];

pub const DEFAULT_WIDTH: u32 = 640;
pub const DEFAULT_HEIGHT: u32 = 480;

/// Preview request shared by the Agent API and MCP.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PreviewParams {
    pub path: String,
    #[serde(default = "default_view")]
    pub view: String,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    /// `plain` (default), `studio`, or `dark`.
    #[serde(default = "default_style")]
    pub style: String,
}

fn default_view() -> String {
    "iso".into()
}

fn default_width() -> u32 {
    DEFAULT_WIDTH
}

fn default_height() -> u32 {
    DEFAULT_HEIGHT
}

fn default_style() -> String {
    "plain".into()
}

/// Height for `width` at an aspect ratio written `W:H` (`16:9`, `1:1`, `9:16`).
pub(crate) fn aspect_height(width: u32, aspect: &str) -> Result<u32> {
    let parse = |text: &str| text.trim().parse::<u32>().ok().filter(|value| *value > 0);
    let (w, h) = aspect
        .split_once(':')
        .and_then(|(w, h)| Some((parse(w)?, parse(h)?)))
        .ok_or_else(|| {
            OpenCadError::validation(format!(
                "aspect '{aspect}' must be W:H with positive integers, for example 16:9"
            ))
        })?;
    Ok(((u64::from(width) * u64::from(h) + u64::from(w) / 2) / u64::from(w)) as u32)
}

/// What the image shows, for agents that also read text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewSummary {
    pub view: String,
    pub width: u32,
    pub height: u32,
    pub triangles: usize,
    /// Axis-aligned bounds of everything drawn, millimetres.
    pub bounds_mm: [[f64; 3]; 2],
    /// Names of the drawn objects in palette order.
    pub objects: Vec<String>,
}

/// Regenerate `params.path` and render it to PNG bytes.
pub fn preview_document(params: &PreviewParams) -> Result<(Vec<u8>, PreviewSummary)> {
    let view = PreviewView::parse(&params.view)?;
    let style = PreviewStyle::parse(&params.style)?;
    let doc = read_ocad(&params.path)?;
    if doc.drawing.is_some() {
        return Err(OpenCadError::validation(
            "preview renders parts and assemblies; export a drawing as SVG instead",
        ));
    }
    let objects = crate::export::document_meshes(&params.path, doc)?;
    let meshes: Vec<PreviewMesh<'_>> = objects
        .iter()
        .enumerate()
        .map(|(index, (_, mesh))| PreviewMesh {
            mesh,
            color: PALETTE[index % PALETTE.len()],
        })
        .collect();
    let mut framing = PreviewFraming::new(view).with_shadow(style.shadow_opacity > 0.0);
    framing.include(&meshes);
    let image = render_preview_styled(&meshes, &framing, params.width, params.height, &style)?;
    let png = encode_png(image.width, image.height, &image.rgba)?;

    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut triangles = 0;
    for (_, mesh) in &objects {
        triangles += mesh.triangle_count();
        for position in &mesh.positions {
            for axis in 0..3 {
                let millimetres = f64::from(position[axis]) * 1000.0;
                min[axis] = min[axis].min(millimetres);
                max[axis] = max[axis].max(millimetres);
            }
        }
    }
    let round = |value: f64| (value * 1000.0).round() / 1000.0;
    Ok((
        png,
        PreviewSummary {
            view: view.name().into(),
            width: image.width,
            height: image.height,
            triangles,
            bounds_mm: [min.map(round), max.map(round)],
            objects: objects.into_iter().map(|(name, _)| name).collect(),
        },
    ))
}

/// One animation frame: meshes in palette order, extra meshes with their own
/// colours (markers), and caption lines.
pub(crate) struct GifFrame {
    pub meshes: Vec<MeshSet>,
    pub accents: Vec<(MeshSet, [f32; 3])>,
    pub caption: Vec<String>,
}

impl GifFrame {
    /// Meshes coloured in palette order, matching `musubicad preview`, then accents.
    fn preview_meshes(&self) -> Vec<PreviewMesh<'_>> {
        self.meshes
            .iter()
            .enumerate()
            .map(|(index, mesh)| PreviewMesh {
                mesh,
                color: PALETTE[index % PALETTE.len()],
            })
            .chain(self.accents.iter().map(|(mesh, color)| PreviewMesh {
                mesh,
                color: *color,
            }))
            .collect()
    }
}

/// Colour of target markers.
pub(crate) const MARKER_COLOR: [f32; 3] = [0.9, 0.18, 0.22];

/// A small octahedron centred on `centre_m` with the given half size, metres.
pub(crate) fn marker_mesh(centre_m: [f64; 3], half_size_m: f64) -> MeshSet {
    let [x, y, z] = centre_m;
    let r = half_size_m;
    let positions: Vec<[f32; 3]> = [
        [x + r, y, z],
        [x - r, y, z],
        [x, y + r, z],
        [x, y - r, z],
        [x, y, z + r],
        [x, y, z - r],
    ]
    .iter()
    .map(|point| point.map(|value| value as f32))
    .collect();
    let indices = vec![
        0, 2, 4, 2, 1, 4, 1, 3, 4, 3, 0, 4, 2, 0, 5, 1, 2, 5, 3, 1, 5, 0, 3, 5,
    ];
    MeshSet {
        normals: positions.clone(),
        positions,
        triangle_face_ids: vec![0; indices.len() / 3],
        indices,
    }
}

/// Render `frames` on the CPU with one framing fitted to all of them, so the
/// camera stays still, caption each, and write a looping GIF.  Returns the
/// number of frames written.
pub(crate) fn write_preview_gif(
    frames: &[GifFrame],
    view: PreviewView,
    style: &PreviewStyle,
    width: u32,
    height: u32,
    frames_per_second: u32,
    output: &str,
) -> Result<usize> {
    // Caption text grows with the image: 7-pixel glyphs at 180 px high.
    let scale = (height / 180).max(1);
    let caption_px = frames
        .iter()
        .map(|frame| {
            let lines: Vec<&str> = frame.caption.iter().map(String::as_str).collect();
            caption_extent(&lines, scale)
        })
        .max()
        .unwrap_or(0);
    let mut framing = PreviewFraming::new(view)
        .with_bottom_inset(caption_px as f32 / height.max(1) as f32)
        .with_shadow(style.shadow_opacity > 0.0);
    for frame in frames {
        framing.include(&frame.preview_meshes());
    }
    let mut images = Vec::with_capacity(frames.len());
    for frame in frames {
        let mut image =
            render_preview_styled(&frame.preview_meshes(), &framing, width, height, style)?;
        let lines: Vec<&str> = frame.caption.iter().map(String::as_str).collect();
        draw_caption(&mut image, &lines, CaptionCorner::BottomLeft, scale);
        images.push(RenderImage {
            width: image.width,
            height: image.height,
            non_background_pixels: 0,
            rgba: image.rgba,
        });
    }
    write_gif_frames(&images, frames_per_second, output)?;
    Ok(images.len())
}

/// Parse `preview <document> <output.png> [--view iso|front|top|right]
/// [--width N] [--height N]`.
pub fn parse_preview_args(args: &[String]) -> Result<(PreviewParams, String)> {
    let usage = "usage: musubicad preview <document> <output.png> [--view iso|front|top|right] [--width N] [--height N | --aspect W:H] [--style plain|studio|dark]";
    let mut positional = Vec::new();
    let mut params = PreviewParams {
        path: String::new(),
        view: default_view(),
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        style: default_style(),
    };
    let mut aspect = None;
    let mut index = 0;
    while index < args.len() {
        let value = || {
            args.get(index + 1)
                .cloned()
                .ok_or_else(|| OpenCadError::validation(usage))
        };
        match args[index].as_str() {
            "--view" => {
                params.view = value()?;
                index += 2;
            }
            "--style" => {
                params.style = value()?;
                index += 2;
            }
            "--aspect" => {
                aspect = Some(value()?);
                index += 2;
            }
            "--width" | "--height" => {
                let number: u32 = value()?
                    .parse()
                    .map_err(|_| OpenCadError::validation(usage))?;
                if args[index] == "--width" {
                    params.width = number;
                } else {
                    params.height = number;
                }
                index += 2;
            }
            other if other.starts_with("--") => return Err(OpenCadError::validation(usage)),
            other => {
                positional.push(other.to_string());
                index += 1;
            }
        }
    }
    let [path, output] =
        <[String; 2]>::try_from(positional).map_err(|_| OpenCadError::validation(usage))?;
    params.path = path;
    if let Some(aspect) = aspect {
        if args.iter().any(|arg| arg == "--height") {
            return Err(OpenCadError::validation(
                "use --height or --aspect, not both",
            ));
        }
        params.height = aspect_height(params.width, &aspect)?;
    }
    Ok((params, output))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(name: &str) -> String {
        format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn previews_a_part_and_an_assembly_as_png() {
        for (name, objects) in [
            ("bearing_carrier.ocad.d", 1),
            ("robot_arm_assembly.ocad.d", 4),
        ] {
            let (png, summary) = preview_document(&PreviewParams {
                path: example(name),
                view: "iso".into(),
                width: 200,
                height: 150,
                style: "plain".into(),
            })
            .expect("preview");
            assert_eq!(&png[1..4], b"PNG", "{name}");
            assert_eq!((summary.width, summary.height), (200, 150));
            assert_eq!(summary.objects.len(), objects, "{name}");
            assert!(summary.triangles > 0);
        }
        let (_, carrier) = preview_document(&PreviewParams {
            path: example("bearing_carrier.ocad.d"),
            view: "top".into(),
            width: 64,
            height: 64,
            style: "studio".into(),
        })
        .expect("top");
        // 96 × 72 × 14 mm carrier.
        assert_eq!(carrier.bounds_mm[1], [96.0, 72.0, 14.0]);
    }

    #[test]
    fn parses_cli_arguments() {
        let args: Vec<String> = ["part.ocad.d", "out.png", "--view", "top", "--width", "320"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (params, output) = parse_preview_args(&args).expect("args");
        assert_eq!(output, "out.png");
        assert_eq!(
            (params.view.as_str(), params.width, params.height),
            ("top", 320, 480)
        );
        assert!(parse_preview_args(&args[..1]).is_err());
        let square: Vec<String> = ["p.ocad.d", "o.png", "--aspect", "1:1", "--style", "dark"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (params, _) = parse_preview_args(&square).expect("square");
        assert_eq!((params.width, params.height), (640, 640));
        assert_eq!(params.style, "dark");
        assert_eq!(aspect_height(720, "16:9").expect("16:9"), 405);
        assert_eq!(aspect_height(720, "9:16").expect("9:16"), 1280);
        assert!(aspect_height(720, "wide").is_err());
        assert!(aspect_height(720, "0:1").is_err());
        let both: Vec<String> = ["p", "o.png", "--height", "10", "--aspect", "1:1"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(parse_preview_args(&both).is_err());
    }
}
