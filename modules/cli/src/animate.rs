//! `opencad animate` — deterministic presentation GIF export.

use opencad_core::{OpenCadError, Result};
use opencad_render::{render_orbit_gif, AnimationOptions, AnimationSummary, OffscreenRenderer};

use crate::mesh::load_view_data;

/// Parse explicit animation flags without hidden environment inputs.
pub fn parse_animation_options(args: &[String]) -> Result<AnimationOptions> {
    let mut options = AnimationOptions::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if flag == "--show-sketch" {
            options.show_sketch = true;
            index += 1;
            continue;
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| OpenCadError::validation(format!("{flag} requires a numeric value")))?;
        match flag {
            "--frames" => options.frame_count = parse_u32(value, flag)?,
            "--fps" => options.frames_per_second = parse_u32(value, flag)?,
            "--width" => options.width_px = parse_u32(value, flag)?,
            "--height" => options.height_px = parse_u32(value, flag)?,
            "--orbit-deg" => options.orbit_degrees = parse_f32(value, flag)?,
            "--pitch-deg" => options.pitch_degrees = parse_f32(value, flag)?,
            _ => {
                return Err(OpenCadError::validation(format!(
                    "unknown animation option '{flag}'"
                )));
            }
        }
        index += 2;
    }
    options.validate()
}

/// Load a document, regenerate its scene, and write a deterministic orbit GIF.
pub fn animate_document(
    input: &str,
    output: &str,
    options: AnimationOptions,
) -> Result<AnimationSummary> {
    if !output.to_ascii_lowercase().ends_with(".gif") {
        return Err(OpenCadError::validation("animation output must use .gif"));
    }
    let data = load_view_data(input)?;
    let renderer = OffscreenRenderer::new()?;
    let overlay = (!data.overlay.is_empty()).then_some(&data.overlay);
    render_orbit_gif(&renderer, &data.scene, overlay, options, output)
}

fn parse_u32(value: &str, flag: &str) -> Result<u32> {
    value
        .parse()
        .map_err(|_| OpenCadError::validation(format!("{flag} requires an integer")))
}

fn parse_f32(value: &str, flag: &str) -> Result<f32> {
    value
        .parse()
        .map_err(|_| OpenCadError::validation(format!("{flag} requires a number in degrees")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_animation_options() {
        let args = [
            "--frames",
            "24",
            "--fps",
            "8",
            "--orbit-deg",
            "180",
            "--pitch-deg",
            "30",
            "--show-sketch",
        ]
        .map(str::to_string);
        let options = parse_animation_options(&args).expect("options");
        assert_eq!(options.frame_count, 24);
        assert_eq!(options.frames_per_second, 8);
        assert_eq!(options.orbit_degrees, 180.0);
        assert_eq!(options.pitch_degrees, 30.0);
        assert!(options.show_sketch);
    }

    #[test]
    fn rejects_unknown_animation_option() {
        let args = ["--random-camera".to_string(), "1".to_string()];
        assert!(parse_animation_options(&args).is_err());
    }
}
