//! `musubicad animate` — deterministic presentation GIF export.

use opencad_core::{OpenCadError, Result};
use std::collections::BTreeSet;

use opencad_feature::{FeatureDefinition, FeatureRegistry};
use opencad_file::read_ocad;
use opencad_geometry::{GeometryKernel, TessellationSettings};
use opencad_kernel_occt::OcctGeometryKernel;
use opencad_render::{
    presentation_overlay, render_orbit_gif, write_gif_frames, AnimationOptions, AnimationSummary,
    OffscreenRenderer, RenderScene,
};

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

/// Regenerate a part and reveal its modifying body milestones in Feature Graph
/// order while keeping one final-shape camera fit.
pub fn animate_feature_build_document(
    input: &str,
    output: &str,
    options: AnimationOptions,
) -> Result<AnimationSummary> {
    if !output.to_ascii_lowercase().ends_with(".gif") {
        return Err(OpenCadError::validation("animation output must use .gif"));
    }
    let options = options.validate()?;
    let document = read_ocad(input)?;
    let parameters = document.parameters.clone();
    let semantic_refs = document.semantic_refs.clone();
    let mut model = document.into_part_model();
    let tool_sources = pattern_tool_sources(&model);
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let report = model.regenerate(&kernel, &registry, Some(&parameters), Some(&semantic_refs))?;

    let mut scenes = Vec::new();
    for feature_id in &report.regenerated {
        if tool_sources.contains(feature_id) {
            continue;
        }
        let Some(body) = model
            .outputs
            .get(feature_id)
            .and_then(|output| output.body.as_ref())
        else {
            continue;
        };
        let mesh = kernel.tessellate(body, &TessellationSettings::default())?;
        scenes.push(RenderScene::from_mesh_set(&mesh)?);
    }
    let final_scene = scenes
        .last()
        .ok_or_else(|| OpenCadError::validation("feature animation requires a regenerated body"))?;
    let renderer = OffscreenRenderer::new()?;
    let mut frames = Vec::with_capacity(options.frame_count as usize);
    for frame_index in 0..options.frame_count {
        let stage_index = feature_stage_index(frame_index, options.frame_count, scenes.len());
        let scene = &scenes[stage_index];
        let camera = options.camera(final_scene, frame_index)?;
        let presentation = presentation_overlay(scene, None);
        frames.push(renderer.render_scene_image_with_camera(
            scene,
            Some(&presentation),
            options.width_px,
            options.height_px,
            &camera,
        )?);
    }
    write_gif_frames(&frames, options.frames_per_second, output)
}

fn pattern_tool_sources(model: &opencad_feature::PartModel) -> BTreeSet<String> {
    model
        .nodes
        .values()
        .filter_map(|node| match &node.definition {
            FeatureDefinition::LinearPattern(pattern) => Some(pattern.source_feature.clone()),
            FeatureDefinition::CircularPattern(pattern) => Some(pattern.source_feature.clone()),
            FeatureDefinition::MirrorPattern(pattern) => Some(pattern.source_feature.clone()),
            _ => None,
        })
        .collect()
}

fn feature_stage_index(frame_index: u32, frame_count: u32, stage_count: usize) -> usize {
    if stage_count <= 1 || frame_count <= 1 {
        return 0;
    }
    ((frame_index as usize * stage_count) / frame_count as usize).min(stage_count - 1)
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

    #[test]
    fn feature_stage_sequence_reaches_every_milestone() {
        let stages = (0..18)
            .map(|frame| feature_stage_index(frame, 18, 9))
            .collect::<BTreeSet<_>>();
        assert_eq!(stages, (0..9).collect());
        assert_eq!(feature_stage_index(17, 18, 9), 8);
    }
}
