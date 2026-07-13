//! `opencad review` — self-contained DesignPatch review artifacts.

use std::fs;
use std::path::{Path, PathBuf};

use opencad_ai::{ensure_patch_valid, ExpectedEffect};
use opencad_core::{OpenCadError, Result};
use opencad_desktop::{load_assembly_scene_from_document, load_view_data, tessellate_active_body};
use opencad_file::{apply_patch_to_document, dry_run_patch_document, read_ocad, OcadDocument};
use opencad_graph::DesignDiff;
use opencad_render::{
    presentation_overlay, write_gif_frames, write_png, OffscreenRenderer, OrbitCamera, RenderImage,
    RenderScene,
};
use serde::{Deserialize, Serialize};

use crate::diff::{build_document_diff, DiffOptions};
use crate::patch::read_patch_file;

const REVIEW_WIDTH_PX: u32 = 800;
const REVIEW_HEIGHT_PX: u32 = 450;
const REVIEW_FPS: u32 = 8;

/// Parsed `opencad review` arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewArgs {
    pub document_path: String,
    pub patch_path: String,
    pub output_dir: String,
}

/// Geometry evidence included in a design review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewGeometry {
    pub before_bounds_m: [[f32; 3]; 2],
    pub after_bounds_m: [[f32; 3]; 2],
    pub before_triangles: usize,
    pub after_triangles: usize,
    pub before_interference_count: Option<usize>,
    pub after_interference_count: Option<usize>,
}

/// Result of checking one declared expected effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectCheck {
    pub effect: ExpectedEffect,
    pub passed: bool,
    pub message: String,
}

/// Machine-readable manifest for a self-contained design review directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewArtifact {
    pub document_id: String,
    pub intent: Option<String>,
    pub rationale: Option<String>,
    pub patch_file: String,
    pub diff: DesignDiff,
    pub geometry: ReviewGeometry,
    pub expected_effects: Vec<EffectCheck>,
    pub before_image: String,
    pub after_image: String,
    pub comparison_gif: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_drawing_svg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_drawing_svg: Option<String>,
}

/// Parse `review <document> <patch> --output <directory>`.
pub fn parse_review_args(args: &[String]) -> Result<ReviewArgs> {
    let mut positional = Vec::new();
    let mut output_dir = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                output_dir = Some(
                    args.get(index + 1)
                        .ok_or_else(|| OpenCadError::validation("--output requires a directory"))?
                        .clone(),
                );
                index += 2;
            }
            value if value.starts_with("--") => {
                return Err(OpenCadError::validation(format!(
                    "unknown review option '{value}'"
                )));
            }
            value => {
                positional.push(value.to_string());
                index += 1;
            }
        }
    }
    Ok(ReviewArgs {
        document_path: positional.first().cloned().ok_or_else(|| {
            OpenCadError::validation(
                "usage: opencad review <document> <patch.json> --output <directory>",
            )
        })?,
        patch_path: positional
            .get(1)
            .cloned()
            .ok_or_else(|| OpenCadError::validation("review requires a DesignPatch JSON file"))?,
        output_dir: output_dir
            .ok_or_else(|| OpenCadError::validation("review requires --output <directory>"))?,
    })
}

/// Generate review JSON, HTML, PNGs, and an animated Before/After GIF.
pub fn generate_review(args: &ReviewArgs) -> Result<ReviewArtifact> {
    let before = read_ocad(&args.document_path)?;
    let patch = read_patch_file(&args.patch_path)?;
    let dry_run = dry_run_patch_document(&before, &patch);
    ensure_patch_valid(&dry_run)?;
    let mut after = before.clone();
    apply_patch_to_document(&mut after, &patch)?;
    let diff = build_document_diff(
        &before,
        &after,
        DiffOptions {
            json: true,
            geometry: true,
        },
    )?;

    let (before_scene, before_interference_count) = document_scene(&args.document_path, &before)?;
    let (after_scene, after_interference_count) = document_scene(&args.document_path, &after)?;
    let renderer = OffscreenRenderer::new()?;
    let mut combined_bounds = before_scene.bounds;
    combined_bounds.merge(&after_scene.bounds);
    let camera = OrbitCamera::fit_bounds(
        &combined_bounds,
        REVIEW_WIDTH_PX as f32 / REVIEW_HEIGHT_PX as f32,
    );
    let before_image = render_review_image(&renderer, &before_scene, &camera)?;
    let after_image = render_review_image(&renderer, &after_scene, &camera)?;

    let output = Path::new(&args.output_dir);
    fs::create_dir_all(output).map_err(io_error("create review directory"))?;
    write_png(
        output.join("before.png"),
        before_image.width,
        before_image.height,
        &before_image.rgba,
    )?;
    write_png(
        output.join("after.png"),
        after_image.width,
        after_image.height,
        &after_image.rgba,
    )?;
    let frames = comparison_frames(&before_image, &after_image);
    write_gif_frames(&frames, REVIEW_FPS, output.join("comparison.gif"))?;

    let drawing_assets: Option<(String, String)> = if before.drawing.is_some() {
        let (before_svg, _) = crate::export::render_drawing_svg(&args.document_path, &before)?;
        let (after_svg, _) = crate::export::render_drawing_svg(&args.document_path, &after)?;
        fs::write(output.join("before-drawing.svg"), before_svg)
            .map_err(io_error("write before drawing SVG"))?;
        fs::write(output.join("after-drawing.svg"), after_svg)
            .map_err(io_error("write after drawing SVG"))?;
        Some(("before-drawing.svg".into(), "after-drawing.svg".into()))
    } else {
        None
    };

    let expected_effects = check_expected_effects(
        &patch.expected_effects,
        &before,
        &after,
        &diff,
        after_interference_count,
    );
    let artifact = ReviewArtifact {
        document_id: before.metadata.id.as_str().to_string(),
        intent: patch.intent,
        rationale: patch.rationale,
        patch_file: file_name(&args.patch_path),
        diff,
        geometry: ReviewGeometry {
            before_bounds_m: [before_scene.bounds.min, before_scene.bounds.max],
            after_bounds_m: [after_scene.bounds.min, after_scene.bounds.max],
            before_triangles: before_scene.triangle_count(),
            after_triangles: after_scene.triangle_count(),
            before_interference_count,
            after_interference_count,
        },
        expected_effects,
        before_image: "before.png".into(),
        after_image: "after.png".into(),
        comparison_gif: "comparison.gif".into(),
        before_drawing_svg: drawing_assets.as_ref().map(|assets| assets.0.clone()),
        after_drawing_svg: drawing_assets.map(|assets| assets.1),
    };
    let json = serde_json::to_string_pretty(&artifact)? + "\n";
    fs::write(output.join("review.json"), json).map_err(io_error("write review JSON"))?;
    fs::write(output.join("review.html"), review_html(&artifact)?)
        .map_err(io_error("write review HTML"))?;
    Ok(artifact)
}

fn document_scene(path: &str, doc: &OcadDocument) -> Result<(RenderScene, Option<usize>)> {
    if doc.assembly.is_some() {
        let (scene, interference_count) = load_assembly_scene_from_document(path, doc)?;
        return Ok((scene, Some(interference_count)));
    }
    if let Some(drawing) = &doc.drawing {
        let view = drawing
            .sheets
            .first()
            .and_then(|sheet| sheet.views.first())
            .ok_or_else(|| OpenCadError::validation("drawing has no view to review"))?;
        let root = if Path::new(path).extension().and_then(|ext| ext.to_str()) == Some("ocad") {
            Path::new(path).parent().unwrap_or_else(|| Path::new("."))
        } else {
            Path::new(path)
        };
        let model_path = root.join(&view.model.source_path);
        let data =
            load_view_data(model_path.to_str().ok_or_else(|| {
                OpenCadError::validation("drawing model path is not valid UTF-8")
            })?)?;
        return Ok((data.scene, None));
    }
    let parameters = doc.parameters.clone();
    let refs = doc.semantic_refs.clone();
    let mut model = doc.clone().into_part_model();
    let mesh = tessellate_active_body(&mut model, Some(&parameters), Some(&refs))?;
    Ok((RenderScene::from_mesh_set(&mesh)?, None))
}

fn render_review_image(
    renderer: &OffscreenRenderer,
    scene: &RenderScene,
    camera: &OrbitCamera,
) -> Result<RenderImage> {
    let overlay = presentation_overlay(scene, None);
    renderer.render_scene_image_with_camera(
        scene,
        Some(&overlay),
        REVIEW_WIDTH_PX,
        REVIEW_HEIGHT_PX,
        camera,
    )
}

fn comparison_frames(before: &RenderImage, after: &RenderImage) -> Vec<RenderImage> {
    let hold = REVIEW_FPS as usize;
    std::iter::repeat(before.clone())
        .take(hold)
        .chain(std::iter::repeat(after.clone()).take(hold))
        .collect()
}

fn check_expected_effects(
    effects: &[ExpectedEffect],
    before: &OcadDocument,
    after: &OcadDocument,
    diff: &DesignDiff,
    after_interference_count: Option<usize>,
) -> Vec<EffectCheck> {
    effects
        .iter()
        .cloned()
        .map(|effect| {
            let (passed, message) = match &effect {
                ExpectedEffect::ParameterExprEquals { id, expr } => {
                    let actual = after.parameters.get(id).map(|entry| entry.expr.as_str());
                    (
                        actual == Some(expr.as_str()),
                        format!(
                            "parameter '{id}' expression is {}",
                            actual.unwrap_or("<missing>")
                        ),
                    )
                }
                ExpectedEffect::MassDeltaKg { min, max } => {
                    let delta = diff
                        .geometry
                        .as_ref()
                        .and_then(|geometry| Some(geometry.mass_after? - geometry.mass_before?));
                    (
                        delta.is_some_and(|value| value >= *min && value <= *max),
                        format!(
                            "mass delta is {} kg",
                            delta.map_or("unavailable".into(), |value| value.to_string())
                        ),
                    )
                }
                ExpectedEffect::DrawingChanged { expected } => {
                    let changed = before.drawing != after.drawing;
                    (changed == *expected, format!("drawing changed: {changed}"))
                }
                ExpectedEffect::NoAssemblyInterference => match after_interference_count {
                    Some(count) => (
                        count == 0,
                        format!("assembly interference count is {count}"),
                    ),
                    None => (false, "document is not an assembly".into()),
                },
            };
            EffectCheck {
                effect,
                passed,
                message,
            }
        })
        .collect()
}

fn review_html(artifact: &ReviewArtifact) -> Result<String> {
    let diff_json = html_escape(&serde_json::to_string_pretty(&artifact.diff)?);
    let intent = html_escape(artifact.intent.as_deref().unwrap_or("Unspecified change"));
    let rationale = html_escape(
        artifact
            .rationale
            .as_deref()
            .unwrap_or("No rationale supplied"),
    );
    let drawing = match (&artifact.before_drawing_svg, &artifact.after_drawing_svg) {
        (Some(before), Some(after)) => format!("<h2>Drawing impact</h2><section class=\"compare\"><figure><img src=\"{}\"><figcaption>Before drawing</figcaption></figure><figure><img src=\"{}\"><figcaption>After drawing</figcaption></figure></section>", html_escape(before), html_escape(after)),
        _ => String::new(),
    };
    Ok(format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>ForgeCAD Review</title><style>{}</style></head><body><main><p class=\"eyebrow\">FORGECAD DESIGN REVIEW</p><h1>{intent}</h1><p>{rationale}</p><img class=\"hero\" src=\"comparison.gif\" alt=\"Before and after geometry\"><section class=\"compare\"><figure><img src=\"before.png\"><figcaption>Before</figcaption></figure><figure><img src=\"after.png\"><figcaption>After</figcaption></figure></section>{drawing}<h2>Semantic and geometric diff</h2><pre>{diff_json}</pre></main></body></html>\n",
        "body{margin:0;background:#111722;color:#e7edf7;font:16px system-ui}main{max-width:1100px;margin:auto;padding:48px}.eyebrow{color:#6dd5ff;letter-spacing:.18em}h1{font-size:42px;margin:.2em 0}.hero{width:100%;border-radius:12px}.compare{display:grid;grid-template-columns:1fr 1fr;gap:18px;margin:24px 0}.compare img{width:100%}figure{margin:0;background:#1d2635;padding:12px;border-radius:10px}figcaption{padding-top:8px}pre{white-space:pre-wrap;background:#0a0f18;padding:20px;border-radius:10px;overflow:auto}"
    ))
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn file_name(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

fn io_error(action: &'static str) -> impl FnOnce(std::io::Error) -> OpenCadError {
    move |err| OpenCadError::Other(format!("failed to {action}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::write_bracket_fixture_at;
    use opencad_ai::{DesignPatch, ExpectedEffect, PatchPrecondition};
    use tempfile::tempdir;

    #[test]
    fn parses_review_arguments() {
        let args = ["part.ocad.d", "change.json", "--output", "review"].map(str::to_string);
        let parsed = parse_review_args(&args).expect("args");
        assert_eq!(parsed.output_dir, "review");
    }

    #[test]
    fn escapes_review_html_content() {
        assert_eq!(html_escape("<unsafe>"), "&lt;unsafe&gt;");
    }

    #[test]
    fn generates_self_contained_review_artifacts() {
        let dir = tempdir().expect("tempdir");
        let document = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&document);
        let patch_path = dir.path().join("width.patch.json");
        let patch = DesignPatch::set_parameter("param:width", "100 mm").with_review_metadata(
            "Increase bracket width",
            "Fit a wider mounting pattern",
            vec![PatchPrecondition::ParameterExprEquals {
                id: "param:width".into(),
                expr: "80 mm".into(),
            }],
            vec![ExpectedEffect::ParameterExprEquals {
                id: "param:width".into(),
                expr: "100 mm".into(),
            }],
        );
        fs::write(
            &patch_path,
            serde_json::to_string_pretty(&patch).expect("patch json"),
        )
        .expect("write patch");
        let output = dir.path().join("review");
        let artifact = generate_review(&ReviewArgs {
            document_path: document.to_string_lossy().into_owned(),
            patch_path: patch_path.to_string_lossy().into_owned(),
            output_dir: output.to_string_lossy().into_owned(),
        })
        .expect("review");
        assert_eq!(artifact.intent.as_deref(), Some("Increase bracket width"));
        assert!(artifact.expected_effects.iter().all(|effect| effect.passed));
        for name in [
            "review.json",
            "review.html",
            "before.png",
            "after.png",
            "comparison.gif",
        ] {
            assert!(output.join(name).is_file(), "missing {name}");
        }
    }
}
