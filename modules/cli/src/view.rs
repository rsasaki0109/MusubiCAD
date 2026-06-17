//! `opencad view`, `opencad screenshot`, and `opencad turntable` commands.

use std::f32::consts::TAU;
use std::path::Path;

use opencad_core::{OpenCadError, Result};
use opencad_render::{run_viewport, OffscreenRenderer};

use crate::mesh::{load_view_data, PREVIEW_HEIGHT, PREVIEW_WIDTH};

pub fn view_document(input: &str) -> Result<()> {
    let data = load_view_data(input)?;
    let overlay = if data.overlay.is_empty() {
        None
    } else {
        Some(&data.overlay)
    };
    run_viewport(&data.scene, overlay, &data.name)
}

pub fn screenshot_document(input: &str, output: &str) -> Result<()> {
    let data = load_view_data(input)?;
    let renderer = OffscreenRenderer::new()?;
    let overlay = if data.overlay.is_empty() {
        None
    } else {
        Some(&data.overlay)
    };
    renderer.render_scene_png(&data.scene, overlay, PREVIEW_WIDTH, PREVIEW_HEIGHT, output)?;
    println!("screenshot: {output}");
    Ok(())
}

/// Options for `opencad turntable`.
#[derive(Debug, Clone, PartialEq)]
pub struct TurntableOptions {
    pub frames: u32,
    pub width: u32,
    pub height: u32,
    pub pitch_deg: f32,
    /// Starting yaw offset in degrees added to the fitted camera yaw. Lets a
    /// single-frame still be framed to reveal a specific feature (e.g. pins).
    pub yaw_deg: f32,
    pub overlay: bool,
}

impl Default for TurntableOptions {
    fn default() -> Self {
        // 16:9 high-resolution frames; downscaling later supersamples the edges.
        Self {
            frames: 48,
            width: 1600,
            height: 900,
            pitch_deg: 28.0,
            yaw_deg: 0.0,
            overlay: false,
        }
    }
}

/// Render a full 360° orbit of the active body as a numbered PNG sequence.
///
/// Frames are written as `<out_dir>/frame_0000.png` … and can be assembled into
/// a GIF or video (see `docs/assets/generate.sh`).
pub fn turntable_document(
    input: &str,
    out_dir: &str,
    options: &TurntableOptions,
) -> Result<Vec<String>> {
    if options.frames == 0 {
        return Err(OpenCadError::validation(
            "turntable requires at least one frame",
        ));
    }
    let data = load_view_data(input)?;
    let overlay = if options.overlay && !data.overlay.is_empty() {
        Some(&data.overlay)
    } else {
        None
    };

    std::fs::create_dir_all(out_dir)
        .map_err(|err| OpenCadError::Other(format!("failed to create {out_dir}: {err}")))?;

    let renderer = OffscreenRenderer::new()?;
    let aspect = options.width as f32 / options.height.max(1) as f32;
    let mut base_camera = data.scene.default_camera(aspect);
    base_camera.pitch_rad = options.pitch_deg.to_radians();
    let yaw0 = base_camera.yaw_rad + options.yaw_deg.to_radians();

    let mut written = Vec::with_capacity(options.frames as usize);
    for frame in 0..options.frames {
        let mut camera = base_camera;
        camera.yaw_rad = yaw0 + TAU * frame as f32 / options.frames as f32;
        let image = renderer.render_scene_image_with_camera(
            &data.scene,
            overlay,
            options.width,
            options.height,
            &camera,
        )?;
        let path = Path::new(out_dir).join(format!("frame_{frame:04}.png"));
        opencad_render::write_png(&path, image.width, image.height, &image.rgba)?;
        written.push(path.to_string_lossy().into_owned());
    }

    println!(
        "turntable: {} frames ({}x{}) -> {out_dir}",
        written.len(),
        options.width,
        options.height
    );
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::write_bracket_fixture_at;
    use tempfile::tempdir;

    #[test]
    fn loads_scene_and_overlay_for_viewport() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        let data = load_view_data(path.to_str().expect("path")).expect("view data");
        assert!(data.scene.triangle_count() > 0);
        assert!(!data.overlay.lines.is_empty());
    }

    #[test]
    fn screenshot_writes_png_with_overlay() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);
        let png_path = dir.path().join("preview.png");

        screenshot_document(
            path.to_str().expect("path"),
            png_path.to_str().expect("png"),
        )
        .expect("screenshot");
        let bytes = std::fs::read(&png_path).expect("read png");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn turntable_writes_frame_sequence() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);
        let frames_dir = dir.path().join("frames");

        let options = TurntableOptions {
            frames: 4,
            width: 160,
            height: 90,
            ..TurntableOptions::default()
        };
        let written = turntable_document(
            path.to_str().expect("path"),
            frames_dir.to_str().expect("frames dir"),
            &options,
        )
        .expect("turntable");

        assert_eq!(written.len(), 4);
        for frame in 0..4 {
            let frame_path = frames_dir.join(format!("frame_{frame:04}.png"));
            let bytes = std::fs::read(&frame_path).expect("read frame");
            assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        }
    }

    #[test]
    fn turntable_rejects_zero_frames() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);
        let frames_dir = dir.path().join("frames");

        let options = TurntableOptions {
            frames: 0,
            ..TurntableOptions::default()
        };
        let result = turntable_document(
            path.to_str().expect("path"),
            frames_dir.to_str().expect("frames dir"),
            &options,
        );
        assert!(result.is_err());
    }
}
