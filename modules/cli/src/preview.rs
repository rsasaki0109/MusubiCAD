//! `musubicad preview`: a GPU-free PNG of a part or assembly (ADR-031).
//!
//! Shared by the CLI, the Agent API (`opencad.preview_document`), and the MCP
//! `preview_document` tool, which returns the PNG as image content so an agent
//! can look at the geometry it just changed.

use opencad_core::{OpenCadError, Result};
use opencad_file::read_ocad;
use opencad_geometry::MeshSet;
use opencad_render::{
    caption_extent, draw_caption, encode_png, render_preview, render_preview_framed,
    write_gif_frames, CaptionCorner, PreviewFraming, PreviewMesh, PreviewView, RenderImage,
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
    let image = render_preview(&meshes, view, params.width, params.height)?;
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

/// One animation frame: meshes in palette order and caption lines.
pub(crate) struct GifFrame {
    pub meshes: Vec<MeshSet>,
    pub caption: Vec<String>,
}

/// Meshes coloured in palette order, matching `musubicad preview`.
fn palette_meshes(meshes: &[MeshSet]) -> Vec<PreviewMesh<'_>> {
    meshes
        .iter()
        .enumerate()
        .map(|(index, mesh)| PreviewMesh {
            mesh,
            color: PALETTE[index % PALETTE.len()],
        })
        .collect()
}

/// Render `frames` on the CPU with one framing fitted to all of them, so the
/// camera stays still, caption each, and write a looping GIF.  Returns the
/// number of frames written.
pub(crate) fn write_preview_gif(
    frames: &[GifFrame],
    view: PreviewView,
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
    let mut framing =
        PreviewFraming::new(view).with_bottom_inset(caption_px as f32 / height.max(1) as f32);
    for frame in frames {
        framing.include(&palette_meshes(&frame.meshes));
    }
    let mut images = Vec::with_capacity(frames.len());
    for frame in frames {
        let mut image =
            render_preview_framed(&palette_meshes(&frame.meshes), &framing, width, height)?;
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
    let usage = "usage: musubicad preview <document> <output.png> [--view iso|front|top|right] [--width N] [--height N]";
    let mut positional = Vec::new();
    let mut params = PreviewParams {
        path: String::new(),
        view: default_view(),
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
    };
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
    }
}
