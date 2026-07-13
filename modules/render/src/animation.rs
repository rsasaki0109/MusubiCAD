//! Deterministic orbit animation export.

use std::fs::File;
use std::path::Path;

use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, RgbaImage};
use opencad_core::{OpenCadError, Result};

use crate::presentation::presentation_overlay;
use crate::{OffscreenRenderer, OrbitCamera, RenderScene, SketchOverlay};

/// Explicit animation settings for reproducible GIF output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationOptions {
    /// Output width in pixels.
    pub width_px: u32,
    /// Output height in pixels.
    pub height_px: u32,
    /// Number of frames in the animation.
    pub frame_count: u32,
    /// Requested playback rate in frames per second.
    pub frames_per_second: u32,
    /// Total camera orbit in degrees.
    pub orbit_degrees: f32,
    /// Constant camera elevation in degrees.
    pub pitch_degrees: f32,
    /// Include sketch entities and constraint annotations.
    pub show_sketch: bool,
}

impl Default for AnimationOptions {
    fn default() -> Self {
        Self {
            width_px: 960,
            height_px: 540,
            frame_count: 48,
            frames_per_second: 12,
            orbit_degrees: 220.0,
            pitch_degrees: 24.0,
            show_sketch: false,
        }
    }
}

impl AnimationOptions {
    /// Validate finite angles, explicit pixel dimensions, and timing.
    pub fn validate(self) -> Result<Self> {
        if self.width_px == 0 || self.height_px == 0 {
            return Err(OpenCadError::validation(
                "animation dimensions must be positive pixels",
            ));
        }
        if self.frame_count < 2 || self.frames_per_second == 0 {
            return Err(OpenCadError::validation(
                "animation requires at least 2 frames and a positive frame rate",
            ));
        }
        if !self.orbit_degrees.is_finite() || !self.pitch_degrees.is_finite() {
            return Err(OpenCadError::validation(
                "animation angles must be finite degrees",
            ));
        }
        Ok(self)
    }

    /// Deterministic camera for a zero-based frame index.
    pub fn camera(self, scene: &RenderScene, frame_index: u32) -> Result<OrbitCamera> {
        let options = self.validate()?;
        if frame_index >= options.frame_count {
            return Err(OpenCadError::validation(
                "animation frame index is out of range",
            ));
        }
        let aspect = options.width_px as f32 / options.height_px as f32;
        let mut camera = scene.default_camera(aspect);
        let progress = frame_index as f32 / (options.frame_count - 1) as f32;
        camera.yaw_rad = 0.55 + options.orbit_degrees.to_radians() * progress;
        camera.pitch_rad = options.pitch_degrees.to_radians();
        camera.distance *= 1.12;
        Ok(camera)
    }
}

/// Metadata from a completed animation export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationSummary {
    /// Encoded width in pixels.
    pub width_px: u32,
    /// Encoded height in pixels.
    pub height_px: u32,
    /// Encoded frame count.
    pub frame_count: u32,
    /// Requested playback rate in frames per second.
    pub frames_per_second: u32,
}

/// Render a deterministic presentation orbit directly to an animated GIF.
pub fn render_orbit_gif(
    renderer: &OffscreenRenderer,
    scene: &RenderScene,
    overlay: Option<&SketchOverlay>,
    options: AnimationOptions,
    output: impl AsRef<Path>,
) -> Result<AnimationSummary> {
    let options = options.validate()?;
    let presentation =
        presentation_overlay(scene, options.show_sketch.then_some(overlay).flatten());
    let file = File::create(output.as_ref())
        .map_err(|err| OpenCadError::Other(format!("failed to create GIF: {err}")))?;
    let mut encoder = GifEncoder::new_with_speed(file, 20);
    encoder
        .set_repeat(Repeat::Infinite)
        .map_err(|err| OpenCadError::Other(format!("failed to configure GIF: {err}")))?;
    let delay = Delay::from_numer_denom_ms(1000, options.frames_per_second);
    for frame_index in 0..options.frame_count {
        let camera = options.camera(scene, frame_index)?;
        let rendered = renderer.render_scene_image_with_camera(
            scene,
            Some(&presentation),
            options.width_px,
            options.height_px,
            &camera,
        )?;
        let image = RgbaImage::from_raw(options.width_px, options.height_px, rendered.rgba)
            .ok_or_else(|| OpenCadError::validation("invalid animation RGBA buffer"))?;
        encoder
            .encode_frame(Frame::from_parts(image, 0, 0, delay))
            .map_err(|err| OpenCadError::Other(format!("failed to encode GIF frame: {err}")))?;
    }
    Ok(AnimationSummary {
        width_px: options.width_px,
        height_px: options.height_px,
        frame_count: options.frame_count,
        frames_per_second: options.frames_per_second,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_geometry::MeshSet;
    use tempfile::tempdir;

    #[test]
    fn camera_sequence_reaches_requested_orbit() {
        let scene = RenderScene::from_mesh_set(&MeshSet::box_prism(0.01, 0.001)).expect("scene");
        let options = AnimationOptions {
            frame_count: 3,
            orbit_degrees: 180.0,
            ..Default::default()
        };
        let first = options.camera(&scene, 0).expect("first");
        let last = options.camera(&scene, 2).expect("last");
        assert!((last.yaw_rad - first.yaw_rad - std::f32::consts::PI).abs() < 1e-5);
    }

    #[test]
    fn writes_animated_gif() {
        let renderer = OffscreenRenderer::new().expect("renderer");
        let scene = RenderScene::from_mesh_set(&MeshSet::box_prism(0.01, 0.001)).expect("scene");
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("orbit.gif");
        let second_path = dir.path().join("orbit-second.gif");
        let options = AnimationOptions {
            width_px: 160,
            height_px: 90,
            frame_count: 3,
            frames_per_second: 6,
            ..Default::default()
        };
        let summary = render_orbit_gif(&renderer, &scene, None, options, &path).expect("gif");
        assert_eq!(summary.frame_count, 3);
        let bytes = std::fs::read(path).expect("read gif");
        assert!(bytes.starts_with(b"GIF89a"));
        render_orbit_gif(&renderer, &scene, None, options, &second_path).expect("second gif");
        assert_eq!(bytes, std::fs::read(second_path).expect("read second gif"));
    }
}
