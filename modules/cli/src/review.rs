//! `opencad review` — self-contained DesignPatch review artifacts.

use std::fs;
use std::path::{Path, PathBuf};

use opencad_ai::{
    ensure_patch_valid, evaluate_assertions, required_assertions_pass, AssertionContext,
    AssertionResult, ExpectedEffect,
};
use opencad_core::{OpenCadError, Result};
use opencad_desktop::{load_assembly_scene_from_document, load_view_data, tessellate_active_body};
use opencad_feature::FeatureRegistry;
use opencad_file::{apply_patch_to_document, dry_run_patch_document, read_ocad, OcadDocument};
use opencad_geometry::{GeometryKernel, ReferenceProvenance};
use opencad_graph::{evaluate_param_graph, DesignDiff, SemanticChange};
use opencad_kernel_occt::OcctGeometryKernel;
use opencad_render::{
    presentation_overlay, write_gif_frames, write_png, OffscreenRenderer, OrbitCamera, RenderImage,
    RenderScene,
};
use serde::{Deserialize, Serialize};

use crate::diff::{build_document_diff, DiffOptions};
use crate::patch::read_patch_file;
use crate::review_gif::{comparison_frames, ReviewGifLabels};

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
    /// Fail-closed semantic reference provenance for the regenerated document
    /// (MCAD-P6-003); populated for part documents with semantic references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_provenance: Vec<ReferenceProvenance>,
    /// Executable design assertion results (MCAD-P6-004); populated for part
    /// documents that declare assertions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assertions: Vec<AssertionResult>,
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

    let reference_provenance = document_reference_provenance(&after);
    let assertions = document_assertion_results(&after);
    if !required_assertions_pass(&assertions) {
        return Err(OpenCadError::validation(
            "reviewed change violates a required document assertion",
        ));
    }
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
    let expected_effects = check_expected_effects(
        &patch.expected_effects,
        &before,
        &after,
        &diff,
        after_interference_count,
    );

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
    let labels = ReviewGifLabels::from_review(&diff, &expected_effects);
    let frames = comparison_frames(&before_image, &after_image, &labels, REVIEW_FPS)?;
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
        reference_provenance,
        assertions,
    };
    let json = serde_json::to_string_pretty(&artifact)? + "\n";
    fs::write(output.join("review.json"), json).map_err(io_error("write review JSON"))?;
    fs::write(output.join("review.html"), review_html(&artifact)?)
        .map_err(io_error("write review HTML"))?;
    fs::write(
        output.join("github-summary.md"),
        github_summary_markdown(&artifact),
    )
    .map_err(io_error("write GitHub review summary"))?;
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

/// Regenerate a part document and return fail-closed reference provenance.
///
/// Assemblies and drawings currently report provenance at the part level only,
/// so this returns an empty list for them (the review still carries geometry
/// and interference evidence).
fn document_reference_provenance(doc: &OcadDocument) -> Vec<ReferenceProvenance> {
    if doc.assembly.is_some() || doc.drawing.is_some() || doc.semantic_refs.is_empty() {
        return Vec::new();
    }
    let parameters = doc.parameters.clone();
    let refs = doc.semantic_refs.clone();
    let mut model = doc.clone().into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let Ok(report) = model.regenerate(&kernel, &registry, Some(&parameters), Some(&refs)) else {
        return Vec::new();
    };
    report.reference_provenance
}

/// Regenerate a part document and evaluate its design assertions (MCAD-P6-004).
fn document_assertion_results(doc: &OcadDocument) -> Vec<AssertionResult> {
    if doc.assembly.is_some() || doc.drawing.is_some() || doc.assertions.is_empty() {
        return Vec::new();
    }
    let parameters = doc.parameters.clone();
    let refs = doc.semantic_refs.clone();
    let mut model = doc.clone().into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let Ok(report) = model.regenerate(&kernel, &registry, Some(&parameters), Some(&refs)) else {
        return Vec::new();
    };
    let Some(body) = model.active_body() else {
        return Vec::new();
    };
    let Ok(mass) = kernel.mass_properties(body, 2700.0) else {
        return Vec::new();
    };
    let Ok(bounds) = kernel.bounding_box(body) else {
        return Vec::new();
    };
    let parameter_values = evaluate_param_graph(&parameters)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let context = AssertionContext {
        parameter_values,
        mass_kg: Some(mass.mass_kg),
        bounding_box_size_m: Some([
            bounds.max[0] - bounds.min[0],
            bounds.max[1] - bounds.min[1],
            bounds.max[2] - bounds.min[2],
        ]),
        body_count: Some(1),
        reference_provenance: report.reference_provenance.clone(),
        assembly_dof: None,
        interference_count: None,
    };
    evaluate_assertions(&doc.assertions, &context)
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

/// Number of declared expected effects that did not pass review.
pub fn failed_expected_effect_count(artifact: &ReviewArtifact) -> usize {
    artifact
        .expected_effects
        .iter()
        .filter(|check| !check.passed)
        .count()
}

/// Return a validation error when a review does not satisfy its declared effects.
pub fn ensure_expected_effects_pass(artifact: &ReviewArtifact, summary_path: &str) -> Result<()> {
    let failed_effects = failed_expected_effect_count(artifact);
    if failed_effects == 0 {
        return Ok(());
    }
    Err(OpenCadError::validation(format!(
        "{failed_effects} expected review effect(s) failed; see {summary_path}"
    )))
}

/// Render a deterministic GitHub Actions job summary for a design review.
pub fn github_summary_markdown(artifact: &ReviewArtifact) -> String {
    let total_effects = artifact.expected_effects.len();
    let failed_effects = failed_expected_effect_count(artifact);
    let status = match (total_effects, failed_effects) {
        (0, _) => "ℹ️ No expected effects declared".to_string(),
        (_, 0) => format!("✅ All {total_effects} expected effects passed"),
        _ => format!("❌ {failed_effects} of {total_effects} expected effects failed"),
    };

    let mut markdown = String::new();
    markdown.push_str("## MusubiCAD Design Review\n\n");
    markdown.push_str(&format!("**Status:** {status}\n\n"));
    markdown.push_str("| Context | Value |\n|---|---|\n");
    markdown.push_str(&format!(
        "| Document | {} |\n",
        markdown_cell(&artifact.document_id)
    ));
    markdown.push_str(&format!(
        "| Intent | {} |\n",
        markdown_cell(artifact.intent.as_deref().unwrap_or("Not supplied"))
    ));
    markdown.push_str(&format!(
        "| Rationale | {} |\n",
        markdown_cell(artifact.rationale.as_deref().unwrap_or("Not supplied"))
    ));
    markdown.push_str(&format!(
        "| Patch | {} |\n\n",
        markdown_cell(&artifact.patch_file)
    ));

    markdown.push_str("### Semantic changes\n\n");
    markdown.push_str("| Change | Before | After |\n|---|---|---|\n");
    if artifact.diff.changes.is_empty() {
        markdown.push_str("| No semantic changes | — | — |\n");
    } else {
        for change in &artifact.diff.changes {
            let (label, before, after) = semantic_change_row(change);
            markdown.push_str(&format!(
                "| {} | {} | {} |\n",
                markdown_cell(&label),
                markdown_cell(&before),
                markdown_cell(&after)
            ));
        }
    }

    markdown.push_str("\n### Regenerated geometry\n\n");
    markdown.push_str("| Property | Before | After |\n|---|---:|---:|\n");
    if let Some(geometry) = &artifact.diff.geometry {
        if let (Some(before), Some(after)) = (geometry.volume_before, geometry.volume_after) {
            markdown.push_str(&format!(
                "| Volume | {:.2} cm³ | {:.2} cm³ |\n",
                before * 1_000_000.0,
                after * 1_000_000.0
            ));
        }
        if let (Some(before), Some(after)) = (geometry.mass_before, geometry.mass_after) {
            markdown.push_str(&format!(
                "| Mass | {:.2} g | {:.2} g |\n",
                before * 1_000.0,
                after * 1_000.0
            ));
        }
    }
    markdown.push_str(&format!(
        "| Bounds | {} | {} |\n",
        format_bounds_mm(artifact.geometry.before_bounds_m),
        format_bounds_mm(artifact.geometry.after_bounds_m)
    ));
    markdown.push_str(&format!(
        "| Triangles (count) | {} | {} |\n",
        artifact.geometry.before_triangles, artifact.geometry.after_triangles
    ));
    if let (Some(before), Some(after)) = (
        artifact.geometry.before_interference_count,
        artifact.geometry.after_interference_count,
    ) {
        markdown.push_str(&format!("| Interferences (count) | {before} | {after} |\n"));
    }

    if !artifact.reference_provenance.is_empty() {
        markdown.push_str("\n### Reference provenance\n\n");
        markdown.push_str("| Reference | Status | Kernel id | Reason |\n|---|---|---:|---|\n");
        for provenance in &artifact.reference_provenance {
            markdown.push_str(&format!(
                "| {} | {:?} | {} | {} |\n",
                markdown_cell(&provenance.ref_id),
                provenance.status,
                provenance
                    .resolved_kernel_id
                    .map_or_else(|| "—".to_string(), |id| id.to_string()),
                markdown_cell(&provenance.reason)
            ));
        }
    }

    if !artifact.assertions.is_empty() {
        markdown.push_str("\n### Design assertions\n\n");
        markdown.push_str("| Status | Assertion | Severity | Evidence |\n|---|---|---|---|\n");
        for result in &artifact.assertions {
            markdown.push_str(&format!(
                "| {} | {} | {:?} | {} |\n",
                if result.passed { "✅" } else { "❌" },
                markdown_cell(&format!("{} ({})", result.name, result.id)),
                result.severity,
                markdown_cell(&result.message)
            ));
        }
    }

    markdown.push_str("\n### Expected effects\n\n");
    markdown.push_str("| Status | Expectation | Evidence |\n|---|---|---|\n");
    if artifact.expected_effects.is_empty() {
        markdown.push_str("| ℹ️ | None declared | — |\n");
    } else {
        for check in &artifact.expected_effects {
            markdown.push_str(&format!(
                "| {} | {} | {} |\n",
                if check.passed { "✅" } else { "❌" },
                markdown_cell(&expected_effect_label(&check.effect)),
                markdown_cell(&check.message)
            ));
        }
    }

    markdown.push_str(
        "\nThe workflow artifact contains `review.html`, `review.json`, `comparison.gif`, and the before/after images.\n",
    );
    markdown
}

fn semantic_change_row(change: &SemanticChange) -> (String, String, String) {
    match change {
        SemanticChange::ParameterChanged { id, before, after } => {
            (format!("Parameter {id}"), before.clone(), after.clone())
        }
        SemanticChange::FeatureAdded { id, feature_type } => (
            format!("Feature {id}"),
            "—".into(),
            format!("Added ({feature_type})"),
        ),
        SemanticChange::FeatureRemoved { id } => {
            (format!("Feature {id}"), "Present".into(), "Removed".into())
        }
        SemanticChange::FeatureModified {
            id,
            field,
            before,
            after,
        } => (
            format!("Feature {id}.{field}"),
            before.clone(),
            after.clone(),
        ),
        SemanticChange::ConstraintModified { id, before, after } => {
            (format!("Constraint {id}"), before.clone(), after.clone())
        }
        SemanticChange::MassChanged { before, after } => {
            ("Mass".into(), before.clone(), after.clone())
        }
        SemanticChange::BboxChanged { before, after } => {
            ("Bounding box".into(), before.clone(), after.clone())
        }
        SemanticChange::TopoRefAdded {
            ref_id,
            created_by,
            role,
        } => (
            format!("Topology reference {ref_id}"),
            "—".into(),
            format!(
                "Added by {created_by}{}",
                role.as_ref()
                    .map_or(String::new(), |role| format!(" ({role})"))
            ),
        ),
        SemanticChange::TopoRefRemoved { ref_id } => (
            format!("Topology reference {ref_id}"),
            "Present".into(),
            "Removed".into(),
        ),
        SemanticChange::TopoRefModified {
            ref_id,
            field,
            before,
            after,
        } => (
            format!("Topology reference {ref_id}.{field}"),
            before.clone(),
            after.clone(),
        ),
        SemanticChange::AssemblyInstanceAdded { id } => (
            format!("Assembly instance {id}"),
            "—".into(),
            "Added".into(),
        ),
        SemanticChange::AssemblyInstanceRemoved { id } => (
            format!("Assembly instance {id}"),
            "Present".into(),
            "Removed".into(),
        ),
        SemanticChange::AssemblyInstanceChanged {
            id,
            field,
            before,
            after,
        } => (
            format!("Assembly instance {id}.{field}"),
            before.clone(),
            after.clone(),
        ),
        SemanticChange::AssemblyMateAdded { id } => {
            (format!("Assembly mate {id}"), "—".into(), "Added".into())
        }
        SemanticChange::AssemblyMateRemoved { id } => (
            format!("Assembly mate {id}"),
            "Present".into(),
            "Removed".into(),
        ),
        SemanticChange::AssemblyMateChanged { id, before, after } => {
            (format!("Assembly mate {id}"), before.clone(), after.clone())
        }
        SemanticChange::AssemblyConnectorAdded { id } => (
            format!("Assembly connector {id}"),
            "—".into(),
            "Added".into(),
        ),
        SemanticChange::AssemblyConnectorRemoved { id } => (
            format!("Assembly connector {id}"),
            "Present".into(),
            "Removed".into(),
        ),
        SemanticChange::AssemblyConnectorChanged { id, before, after } => (
            format!("Assembly connector {id}"),
            before.clone(),
            after.clone(),
        ),
        SemanticChange::DrawingSheetAdded { id } => {
            (format!("Drawing sheet {id}"), "—".into(), "Added".into())
        }
        SemanticChange::DrawingSheetRemoved { id } => (
            format!("Drawing sheet {id}"),
            "Present".into(),
            "Removed".into(),
        ),
        SemanticChange::DrawingSheetChanged { id, before, after } => {
            (format!("Drawing sheet {id}"), before.clone(), after.clone())
        }
        SemanticChange::DrawingViewAdded { id } => {
            (format!("Drawing view {id}"), "—".into(), "Added".into())
        }
        SemanticChange::DrawingViewRemoved { id } => (
            format!("Drawing view {id}"),
            "Present".into(),
            "Removed".into(),
        ),
        SemanticChange::DrawingViewChanged { id, before, after } => {
            (format!("Drawing view {id}"), before.clone(), after.clone())
        }
        SemanticChange::AssertionAdded { id } => {
            (format!("Assertion {id}"), "—".into(), "Added".into())
        }
        SemanticChange::AssertionRemoved { id } => (
            format!("Assertion {id}"),
            "Present".into(),
            "Removed".into(),
        ),
        SemanticChange::AssertionChanged { id, before, after } => {
            (format!("Assertion {id}"), before.clone(), after.clone())
        }
    }
}

fn expected_effect_label(effect: &ExpectedEffect) -> String {
    match effect {
        ExpectedEffect::ParameterExprEquals { id, expr } => {
            format!("Parameter {id} equals {expr}")
        }
        ExpectedEffect::MassDeltaKg { min, max } => {
            format!("Mass delta is between {min} kg and {max} kg")
        }
        ExpectedEffect::DrawingChanged { expected } => {
            format!("Drawing changed is {expected}")
        }
        ExpectedEffect::NoAssemblyInterference => "No assembly interference".into(),
    }
}

fn format_bounds_mm(bounds: [[f32; 3]; 2]) -> String {
    let size = [
        (bounds[1][0] - bounds[0][0]) * 1_000.0,
        (bounds[1][1] - bounds[0][1]) * 1_000.0,
        (bounds[1][2] - bounds[0][2]) * 1_000.0,
    ];
    format!("{:.2} × {:.2} × {:.2} mm", size[0], size[1], size[2])
}

fn markdown_cell(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('|', "\\|")
        .replace("\r\n", "<br>")
        .replace(['\r', '\n'], "<br>")
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
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>MusubiCAD Review</title><style>{}</style></head><body><main><p class=\"eyebrow\">MUSUBICAD DESIGN REVIEW</p><h1>{intent}</h1><p>{rationale}</p><img class=\"hero\" src=\"comparison.gif\" alt=\"Before and after geometry\"><section class=\"compare\"><figure><img src=\"before.png\"><figcaption>Before</figcaption></figure><figure><img src=\"after.png\"><figcaption>After</figcaption></figure></section>{drawing}<h2>Semantic and geometric diff</h2><pre>{diff_json}</pre></main></body></html>\n",
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
    use opencad_ai::{
        AgentApi, DesignPatch, ExpectedEffect, PatchDryRunParams, PatchOperation, PatchPrecondition,
    };
    use opencad_assembly::{
        detect_interferences_with_tolerance, regenerate_assembly, AssemblyInterferenceTolerance,
        ChildPart, ResolvedChild,
    };
    use opencad_core::{Assertion, AssertionKind, AssertionSeverity, DocumentId, SheetId, ViewId};
    use opencad_drawing::{
        render_sheet_svg, DrawingView, ModelReference, ProjectionKind, Sheet, ViewMesh,
    };
    use opencad_feature::{bracket_with_hole, FeatureRegistry};
    use opencad_geometry::{
        resolve_kernel_edge_id_for_topo_ref, resolve_kernel_face_id_for_topo_ref_with_discoveries,
        sync_semantic_refs_with_history, GeometryKernel, MeshSet, TopoRefKind,
    };
    use opencad_graph::bracket_parameters;
    use opencad_kernel_occt::OcctGeometryKernel;
    use serde::Deserialize;
    use std::path::Path;
    use tempfile::tempdir;

    const P5_005_MANIFEST: &str =
        include_str!("../../../fixtures/golden/mcad_p5_005_end_to_end.json");

    #[derive(Debug, Deserialize)]
    struct EndToEndManifest {
        schema: String,
        fixture: String,
        part: PartGolden,
        assembly: AssemblyGolden,
        drawing: DrawingGolden,
        desktop_evidence: DesktopEvidenceGolden,
        review: ReviewGolden,
        agent_evidence: AgentEvidenceGolden,
    }

    #[derive(Debug, Deserialize)]
    struct PartGolden {
        source: String,
        mass_fixture: String,
        density_kg_per_m3: f64,
        volume_m3: f64,
        mass_kg: f64,
        tolerance: MassTolerance,
        bounding_box_m: BoundingBoxGolden,
        topology: TopologyGolden,
    }

    #[derive(Debug, Deserialize)]
    struct MassTolerance {
        density_kg_per_m3: f64,
        volume_m3: f64,
        mass_kg: f64,
    }

    #[derive(Debug, Deserialize)]
    struct BoundingBoxGolden {
        min: [f64; 3],
        max: [f64; 3],
        tolerance_m: f64,
    }

    #[derive(Debug, Deserialize)]
    struct TopologyGolden {
        semantic_identity: Vec<SemanticIdentityGolden>,
        current_refs: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    struct SemanticIdentityGolden {
        ref_id: String,
        kind: String,
        created_by: String,
        role: String,
    }

    #[derive(Debug, Deserialize)]
    struct AssemblyGolden {
        source: String,
        kernel: String,
        density_kg_per_m3: f64,
        instance_count: usize,
        successful_instances: usize,
        interference_count: usize,
        interference_tolerance: InterferenceToleranceGolden,
        volume_m3: f64,
        mass_kg: f64,
        tolerance: AssemblyTolerance,
        bounding_box_m: BoundingBoxExtentsGolden,
    }

    #[derive(Debug, Deserialize)]
    struct AssemblyTolerance {
        density_kg_per_m3: f64,
        volume_m3: f64,
        mass_kg: f64,
        bounding_box_m: f64,
    }

    #[derive(Debug, Deserialize)]
    struct InterferenceToleranceGolden {
        bounds_tolerance_m: f64,
        volume_tolerance_m3: f64,
    }

    #[derive(Debug, Deserialize)]
    struct BoundingBoxExtentsGolden {
        min: [f64; 3],
        max: [f64; 3],
    }

    #[derive(Debug, Deserialize)]
    struct DrawingGolden {
        source: String,
        expected_visible_segments: usize,
        expected_hidden_segments: usize,
    }

    #[derive(Debug, Deserialize)]
    struct DesktopEvidenceGolden {
        expected_name: String,
        expected_triangles: usize,
        expected_sketch_count: usize,
        expected_feature_count: usize,
    }

    #[derive(Debug, Deserialize)]
    struct ReviewGolden {
        document: String,
        patch: String,
        golden_dir: String,
        exact_artifacts: Vec<String>,
        before_bounds_m: [[f64; 3]; 2],
        after_bounds_m: [[f64; 3]; 2],
        geometry_tolerance_m: f64,
    }

    #[derive(Debug, Deserialize)]
    struct AgentEvidenceGolden {
        method: String,
        patch: String,
        operation: String,
        parameter_id: String,
        expression: String,
        expected_diff_summary: String,
    }

    #[derive(Debug, Deserialize)]
    struct ExistingMassFixture {
        document: String,
        density_kg_per_m3: f64,
        cases: Vec<ExistingMassCase>,
    }

    #[derive(Debug, Deserialize)]
    struct ExistingMassCase {
        id: String,
        volume_m3: f64,
        mass_kg: f64,
    }

    fn repo_path(relative: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative)
    }

    fn assert_near(actual: f64, expected: f64, tolerance: f64, label: &str) {
        assert!(actual.is_finite(), "{label}: actual is not finite");
        assert!(expected.is_finite(), "{label}: expected is not finite");
        assert!(
            tolerance.is_finite() && tolerance > 0.0,
            "{label}: invalid tolerance"
        );
        assert!(
            (actual - expected).abs() <= tolerance,
            "{label}: actual={actual} expected={expected} tolerance={tolerance}"
        );
    }

    fn assert_bbox(
        actual: &opencad_geometry::BoundingBox,
        expected_min: [f64; 3],
        expected_max: [f64; 3],
        tolerance: f64,
        label: &str,
    ) {
        for axis in 0..3 {
            assert_near(
                actual.min[axis],
                expected_min[axis],
                tolerance,
                &format!("{label}.min[{axis}]"),
            );
            assert_near(
                actual.max[axis],
                expected_max[axis],
                tolerance,
                &format!("{label}.max[{axis}]"),
            );
        }
    }

    fn kind_name(kind: TopoRefKind) -> &'static str {
        match kind {
            TopoRefKind::Face => "face",
            TopoRefKind::Edge => "edge",
            TopoRefKind::Vertex => "vertex",
        }
    }

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
    fn escapes_github_markdown_table_content() {
        assert_eq!(markdown_cell("a|b\n<c>"), "a\\|b<br>&lt;c&gt;");
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
        ensure_expected_effects_pass(&artifact, "review/github-summary.md").expect("effects pass");
        let review_html = fs::read_to_string(output.join("review.html")).expect("review html");
        assert!(review_html.contains("<title>MusubiCAD Review</title>"));
        let summary = fs::read_to_string(output.join("github-summary.md")).expect("GitHub summary");
        assert!(summary.contains("## MusubiCAD Design Review"));
        assert!(summary.contains("✅ All 1 expected effects passed"));
        assert!(summary.contains("| Parameter param:width | 80 mm | 100 mm |"));
        assert!(summary.contains("| Bounds |"));

        let mut failed_artifact = artifact.clone();
        failed_artifact.expected_effects[0].passed = false;
        assert_eq!(failed_expected_effect_count(&failed_artifact), 1);
        assert!(
            github_summary_markdown(&failed_artifact).contains("❌ 1 of 1 expected effects failed")
        );
        let error = ensure_expected_effects_pass(&failed_artifact, "review/github-summary.md")
            .expect_err("failed effect must fail review");
        assert!(error.to_string().contains("review/github-summary.md"));
        for name in [
            "review.json",
            "review.html",
            "github-summary.md",
            "before.png",
            "after.png",
            "comparison.gif",
        ] {
            assert!(output.join(name).is_file(), "missing {name}");
        }
    }

    #[test]
    fn mcad_p5_005_end_to_end_artifacts_are_deterministic() {
        let manifest: EndToEndManifest =
            serde_json::from_str(P5_005_MANIFEST).expect("P5-005 manifest JSON");
        assert_eq!(manifest.schema, "musubicad.mcad_p5_005.end_to_end.v1");
        assert_eq!(manifest.fixture, "bracket_with_hole");

        let mass_fixture: ExistingMassFixture = serde_json::from_str(
            &fs::read_to_string(repo_path(&manifest.part.mass_fixture))
                .expect("existing mass fixture"),
        )
        .expect("existing mass fixture JSON");
        assert_eq!(mass_fixture.document, manifest.fixture);
        assert_near(
            mass_fixture.density_kg_per_m3,
            manifest.part.density_kg_per_m3,
            manifest.part.tolerance.density_kg_per_m3,
            "part density_kg_per_m3",
        );
        let default_case = mass_fixture
            .cases
            .iter()
            .find(|case| case.id == "default")
            .expect("default mass case");
        assert_near(
            default_case.volume_m3,
            manifest.part.volume_m3,
            manifest.part.tolerance.volume_m3,
            "manifest volume_m3",
        );
        assert_near(
            default_case.mass_kg,
            manifest.part.mass_kg,
            manifest.part.tolerance.mass_kg,
            "manifest mass_kg",
        );

        let part_path = repo_path(&manifest.part.source);
        let part_document = opencad_file::read_expanded_dir(&part_path).expect("part fixture");
        assert_eq!(part_document.metadata.id.as_str(), "doc:bracket_001");
        let mut part_model = part_document.clone().into_part_model();
        let kernel = OcctGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let regen = part_model
            .regenerate(
                &kernel,
                &registry,
                Some(&part_document.parameters),
                Some(&part_document.semantic_refs),
            )
            .expect("part regeneration");
        let body = part_model.active_body().expect("part body");
        let mass = kernel
            .mass_properties(body, manifest.part.density_kg_per_m3)
            .expect("part mass");
        assert_near(
            mass.volume_m3,
            manifest.part.volume_m3,
            manifest.part.tolerance.volume_m3,
            "part volume_m3",
        );
        assert_near(
            mass.mass_kg,
            manifest.part.mass_kg,
            manifest.part.tolerance.mass_kg,
            "part mass_kg",
        );
        let bbox = kernel.bounding_box(body).expect("part bounding box");
        assert_bbox(
            &bbox,
            manifest.part.bounding_box_m.min,
            manifest.part.bounding_box_m.max,
            manifest.part.bounding_box_m.tolerance_m,
            "part bounding_box_m",
        );

        let nodes = part_model.nodes.values().cloned().collect::<Vec<_>>();
        let face_discoveries =
            opencad_feature::face_discover::discover_face_refs_from_body(&kernel, body, &nodes)
                .expect("face discoveries");
        let edge_discoveries =
            opencad_feature::edge_discover::discover_edge_refs_from_body(&kernel, body, &nodes)
                .expect("edge discoveries");
        assert!(
            !face_discoveries.is_empty(),
            "part has no current face refs"
        );
        assert!(
            !edge_discoveries.is_empty(),
            "part has no current edge refs"
        );

        let mut actual_identities = part_document
            .semantic_refs
            .iter()
            .map(|topo_ref| {
                let identity = topo_ref.identity();
                (
                    identity.ref_id.as_str().to_string(),
                    kind_name(identity.kind).to_string(),
                    identity.created_by,
                    identity.role.unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        actual_identities.sort();
        let mut expected_identities = manifest
            .part
            .topology
            .semantic_identity
            .iter()
            .map(|identity| {
                (
                    identity.ref_id.clone(),
                    identity.kind.clone(),
                    identity.created_by.clone(),
                    identity.role.clone(),
                )
            })
            .collect::<Vec<_>>();
        expected_identities.sort();
        assert_eq!(actual_identities, expected_identities);

        let current_refs = sync_semantic_refs_with_history(
            &part_document.semantic_refs,
            &regen.face_history,
            &face_discoveries,
        );
        for ref_id in &manifest.part.topology.current_refs {
            let topo_ref = current_refs
                .iter()
                .find(|topo_ref| topo_ref.ref_id.as_str() == ref_id)
                .unwrap_or_else(|| panic!("missing current semantic ref '{ref_id}'"));
            match topo_ref.kind {
                TopoRefKind::Face => {
                    let kernel_face_id = resolve_kernel_face_id_for_topo_ref_with_discoveries(
                        &current_refs,
                        &regen.face_history,
                        ref_id,
                        Some(&face_discoveries),
                    )
                    .expect("current face ref resolution");
                    assert!(face_discoveries
                        .iter()
                        .any(|discovery| discovery.kernel_face_id == kernel_face_id));
                }
                TopoRefKind::Edge => {
                    let kernel_edge_id = resolve_kernel_edge_id_for_topo_ref(
                        &current_refs,
                        ref_id,
                        Some(&edge_discoveries),
                    )
                    .expect("current edge ref resolution");
                    assert!(edge_discoveries
                        .iter()
                        .any(|discovery| discovery.kernel_edge_id == kernel_edge_id));
                }
                TopoRefKind::Vertex => panic!("unsupported vertex fixture ref '{ref_id}'"),
            }
        }

        assert_eq!(manifest.assembly.kernel, "occt");
        assert_near(
            manifest.assembly.density_kg_per_m3,
            2700.0,
            manifest.assembly.tolerance.density_kg_per_m3,
            "assembly density_kg_per_m3",
        );
        let assembly_path = repo_path(&manifest.assembly.source);
        let assembly_document =
            opencad_file::read_expanded_dir(&assembly_path).expect("assembly fixture");
        let assembly = assembly_document.assembly.as_ref().expect("assembly model");
        let assembly_id = assembly_document.metadata.id.clone();
        let assembly_kernel = OcctGeometryKernel::new();
        let mut loader = |_child_path: &Path| {
            let child_part = bracket_with_hole().expect("assembly child model");
            Ok(ResolvedChild::Part(Box::new(ChildPart {
                doc_id: DocumentId::new("doc:bracket_001").expect("child document id"),
                parameters: bracket_parameters(),
                part: child_part,
                semantic_refs: Vec::new(),
            })))
        };
        let assembly_report = regenerate_assembly(
            assembly,
            &assembly_id,
            &assembly_path,
            &assembly_kernel,
            &registry,
            &mut loader,
        )
        .expect("assembly regeneration");
        assert_eq!(
            assembly_report.instance_count,
            manifest.assembly.instance_count
        );
        assert_eq!(
            assembly_report.successful_instances,
            manifest.assembly.successful_instances
        );
        let interferences = detect_interferences_with_tolerance(
            &assembly_kernel,
            &assembly_report.scene,
            AssemblyInterferenceTolerance {
                bounds_tolerance_m: manifest.assembly.interference_tolerance.bounds_tolerance_m,
                volume_tolerance_m3: manifest.assembly.interference_tolerance.volume_tolerance_m3,
            },
        )
        .expect("assembly interference check");
        assert_eq!(interferences.len(), manifest.assembly.interference_count);
        let assembly_mass = assembly_report.scene.mass.expect("assembly mass");
        assert_near(
            assembly_mass.volume_m3,
            manifest.assembly.volume_m3,
            manifest.assembly.tolerance.volume_m3,
            "assembly volume_m3",
        );
        assert_near(
            assembly_mass.mass_kg,
            manifest.assembly.mass_kg,
            manifest.assembly.tolerance.mass_kg,
            "assembly mass_kg",
        );
        let assembly_bbox = assembly_report
            .scene
            .bounding_box
            .expect("assembly bounding box");
        assert_bbox(
            &assembly_bbox,
            manifest.assembly.bounding_box_m.min,
            manifest.assembly.bounding_box_m.max,
            manifest.assembly.tolerance.bounding_box_m,
            "assembly bounding_box_m",
        );

        let mut sheet = Sheet::a4_portrait(
            SheetId::new("sheet:partial_occlusion").expect("sheet"),
            "HLR",
        );
        let view_id = ViewId::new("view:front").expect("view");
        sheet.views.push(DrawingView::new(
            view_id.clone(),
            "Front",
            ModelReference::new(
                "synthetic.ocad.d",
                DocumentId::new("doc:synthetic").expect("synthetic document"),
            ),
            ProjectionKind::Front,
            1.0,
            [0.05, 0.05],
        ));
        let drawing_mesh = MeshSet {
            positions: vec![
                [-0.01, 0.0, 0.0],
                [0.01, 0.0, 0.0],
                [0.0, -0.01, 0.0],
                [-0.005, -0.005, 0.001],
                [0.005, -0.005, 0.001],
                [0.0, 0.005, 0.001],
            ],
            normals: Vec::new(),
            indices: vec![0, 1, 2, 3, 4, 5],
            triangle_face_ids: vec![1, 2],
        };
        let svg = render_sheet_svg(
            &sheet,
            &[ViewMesh {
                view_id,
                mesh_set: drawing_mesh,
            }],
        )
        .expect("drawing SVG");
        assert_eq!(
            svg.matches("stroke=\"#111111\"").count(),
            manifest.drawing.expected_visible_segments
        );
        assert_eq!(
            svg.matches("stroke=\"#777777\"").count(),
            manifest.drawing.expected_hidden_segments
        );
        let drawing_golden =
            fs::read_to_string(repo_path(&manifest.drawing.source)).expect("drawing golden");
        assert_eq!(svg, drawing_golden);

        let patch: DesignPatch = serde_json::from_str(
            &fs::read_to_string(repo_path(&manifest.agent_evidence.patch)).expect("agent patch"),
        )
        .expect("agent patch JSON");
        assert_eq!(manifest.agent_evidence.method, "opencad.patch_dry_run");
        assert_eq!(manifest.agent_evidence.operation, "set_parameter");
        assert!(matches!(
            patch.operations.as_slice(),
            [PatchOperation::SetParameter { id, expr }]
                if id == &manifest.agent_evidence.parameter_id
                    && expr == &manifest.agent_evidence.expression
        ));
        let agent_report = AgentApi.patch_dry_run(PatchDryRunParams {
            parameters: part_document.parameters.clone(),
            feature_nodes: part_document.feature_nodes.clone(),
            semantic_refs: part_document.semantic_refs.clone(),
            assembly: part_document.assembly.clone(),
            drawing: part_document.drawing.clone(),
            patch: patch.clone(),
        });
        assert!(agent_report.validation.is_ok());
        assert_eq!(
            agent_report.diff.summary,
            manifest.agent_evidence.expected_diff_summary
        );

        let desktop_preview =
            opencad_desktop::preview_document(part_path.to_str().expect("UTF-8 part fixture path"))
                .expect("desktop preview");
        assert_eq!(
            desktop_preview.name,
            manifest.desktop_evidence.expected_name
        );
        assert_eq!(
            desktop_preview.triangles,
            manifest.desktop_evidence.expected_triangles
        );
        assert_eq!(
            desktop_preview.sketch_count,
            manifest.desktop_evidence.expected_sketch_count
        );
        assert_eq!(
            desktop_preview.feature_count,
            manifest.desktop_evidence.expected_feature_count
        );
        for axis in 0..3 {
            assert_near(
                f64::from(desktop_preview.bounds_min_m[axis]),
                manifest.part.bounding_box_m.min[axis],
                manifest.part.bounding_box_m.tolerance_m,
                &format!("desktop bounds_min_m[{axis}]"),
            );
            assert_near(
                f64::from(desktop_preview.bounds_max_m[axis]),
                manifest.part.bounding_box_m.max[axis],
                manifest.part.bounding_box_m.tolerance_m,
                &format!("desktop bounds_max_m[{axis}]"),
            );
        }

        assert!(manifest.review.geometry_tolerance_m > 0.0);
        let first_review = tempdir().expect("first review tempdir");
        let second_review = tempdir().expect("second review tempdir");
        let first_output = first_review.path().join("review");
        let second_output = second_review.path().join("review");
        let first_artifact = generate_review(&ReviewArgs {
            document_path: repo_path(&manifest.review.document)
                .to_string_lossy()
                .into_owned(),
            patch_path: repo_path(&manifest.review.patch)
                .to_string_lossy()
                .into_owned(),
            output_dir: first_output.to_string_lossy().into_owned(),
        })
        .expect("first review generation");
        generate_review(&ReviewArgs {
            document_path: repo_path(&manifest.review.document)
                .to_string_lossy()
                .into_owned(),
            patch_path: repo_path(&manifest.review.patch)
                .to_string_lossy()
                .into_owned(),
            output_dir: second_output.to_string_lossy().into_owned(),
        })
        .expect("second review generation");
        assert_near(
            f64::from(first_artifact.geometry.before_bounds_m[1][0]),
            manifest.review.before_bounds_m[1][0],
            manifest.review.geometry_tolerance_m,
            "review before max bounds x_m",
        );
        assert_near(
            f64::from(first_artifact.geometry.after_bounds_m[1][0]),
            manifest.review.after_bounds_m[1][0],
            manifest.review.geometry_tolerance_m,
            "review after max bounds x_m",
        );
        let golden_review_dir = repo_path(&manifest.review.golden_dir);
        for name in &manifest.review.exact_artifacts {
            let first =
                fs::read_to_string(first_output.join(name)).expect("generated review artifact");
            let second =
                fs::read_to_string(second_output.join(name)).expect("second review artifact");
            let golden =
                fs::read_to_string(golden_review_dir.join(name)).expect("review golden artifact");
            assert_eq!(
                first, second,
                "review artifact is not deterministic: {name}"
            );
            assert_eq!(first, golden, "review artifact differs from golden: {name}");
        }
    }

    #[test]
    fn review_evaluates_document_assertions_and_rejects_violations() {
        let dir = tempdir().expect("tempdir");
        let document = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&document);

        let mut doc = opencad_file::read_ocad(&document).expect("read");
        doc.assertions = vec![
            Assertion::new(
                "assertion:mass",
                "Bracket mass",
                AssertionSeverity::Required,
                AssertionKind::MassRange {
                    min_kg: 0.07,
                    max_kg: 0.09,
                },
            ),
            Assertion::new(
                "assertion:param",
                "Width range",
                AssertionSeverity::Required,
                AssertionKind::ParameterRange {
                    parameter_name: "width".into(),
                    min_m: 0.05,
                    max_m: 0.15,
                },
            ),
        ];
        opencad_file::write_ocad(&document, &doc).expect("write doc");
        let readback = opencad_file::read_ocad(&document).expect("readback");
        assert_eq!(readback.assertions.len(), 2, "assertions must persist");

        let patch_path = dir.path().join("width.patch.json");
        let patch = DesignPatch::set_parameter("param:width", "100 mm").with_review_metadata(
            "Increase bracket width",
            "Fit a wider mounting pattern",
            Vec::new(),
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
        assert_eq!(artifact.assertions.len(), 2);
        assert!(
            artifact.assertions.iter().all(|result| result.passed),
            "assertions must pass: {:?}",
            artifact.assertions
        );

        // A Required assertion the regenerated document violates rejects the review.
        let mut doc = opencad_file::read_ocad(&document).expect("read");
        doc.assertions = vec![Assertion::new(
            "assertion:mass",
            "Bracket mass",
            AssertionSeverity::Required,
            AssertionKind::MassRange {
                min_kg: 0.07,
                max_kg: 0.078,
            },
        )];
        opencad_file::write_ocad(&document, &doc).expect("write doc");
        let rejected = generate_review(&ReviewArgs {
            document_path: document.to_string_lossy().into_owned(),
            patch_path: patch_path.to_string_lossy().into_owned(),
            output_dir: output.to_string_lossy().into_owned(),
        });
        assert!(
            rejected.is_err(),
            "a Required assertion violation must reject the review"
        );
    }

    #[test]
    fn robot_arm_assembly_review_is_deterministic_and_interference_free() {
        let first_dir = tempdir().expect("first tempdir");
        let second_dir = tempdir().expect("second tempdir");
        let first_output = first_dir.path().join("review");
        let second_output = second_dir.path().join("review");
        let review_args = |output: &Path| ReviewArgs {
            document_path: repo_path("examples/robot_arm_assembly.ocad.d")
                .to_string_lossy()
                .into_owned(),
            patch_path: repo_path("examples/agent/review_robot_arm_patch.json")
                .to_string_lossy()
                .into_owned(),
            output_dir: output.to_string_lossy().into_owned(),
        };

        let first = generate_review(&review_args(&first_output)).expect("first review");
        generate_review(&review_args(&second_output)).expect("second review");

        assert_eq!(first.document_id, "doc:robot_arm_assembly_001");
        assert!(
            first.expected_effects.iter().all(|effect| effect.passed),
            "every expected effect must pass: {:?}",
            first.expected_effects
        );
        assert_eq!(first.geometry.before_interference_count, Some(0));
        assert_eq!(first.geometry.after_interference_count, Some(0));
        assert_eq!(first.diff.changes.len(), 2);
        assert!(
            f64::from(first.geometry.after_bounds_m[1][0])
                > f64::from(first.geometry.before_bounds_m[1][0]),
            "bent arm must reach further in x"
        );

        let golden_dir = repo_path("docs/assets/arm-review");
        for name in ["review.json", "review.html", "github-summary.md"] {
            let generated =
                fs::read_to_string(first_output.join(name)).expect("generated review artifact");
            let repeated =
                fs::read_to_string(second_output.join(name)).expect("repeated review artifact");
            let golden =
                fs::read_to_string(golden_dir.join(name)).expect("arm review golden artifact");
            assert_eq!(
                generated, repeated,
                "review artifact is not deterministic: {name}"
            );
            assert_eq!(
                generated, golden,
                "review artifact differs from golden: {name}"
            );
        }
    }
}
