//! `musubicad regen` command (Task-123+).

use std::path::Path;

use opencad_ai::{
    evaluate_assertions, required_assertions_pass, AssertionContext, AssertionResult,
};
use opencad_assembly::{regenerate_assembly, ChildPart, InstanceRegenStatus, ResolvedChild};
use opencad_core::{Assertion, DocumentKind, OpenCadError};
use opencad_feature::{
    FeatureRegistry, PartModel, RegenReport, RegenerationFailure, RegenerationTrace,
};
use opencad_file::{read_ocad, write_expanded_dir, OcadDocument};
use opencad_geometry::{GeometryKernel, ReferenceProvenance, ReferenceStatus};
use opencad_graph::{evaluate_param_graph, FeatureGraph, ParamGraph};
use opencad_sketch::Sketch;
use serde::{Deserialize, Serialize};

#[cfg(feature = "occt")]
use opencad_kernel_occt::OcctGeometryKernel;

use opencad_core::Result;

pub(crate) const DEFAULT_DENSITY_KG_PER_M3: f64 = 2700.0;

/// Summary printed by `musubicad regen`.
#[derive(Debug, Clone, PartialEq)]
pub struct RegenSummary {
    pub kernel: String,
    pub report: RegenReport,
    pub volume_m3: Option<f64>,
    pub mass_kg: Option<f64>,
    pub instances: Option<usize>,
    pub mate_dof: Option<i32>,
    pub mate_max_error: Option<f64>,
    /// Executable design assertion results (MCAD-P6-004); empty when the
    /// document declares no assertions.
    pub assertions: Vec<AssertionResult>,
}

/// Serializable regeneration result for Agent API responses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegenResult {
    pub kernel: String,
    pub regenerated: Vec<String>,
    pub skipped_suppressed: Vec<String>,
    pub volume_m3: Option<f64>,
    pub mass_kg: Option<f64>,
    pub density_kg_per_m3: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instances: Option<usize>,
    pub trace: RegenerationTrace,
    /// Fail-closed semantic reference provenance (MCAD-P6-003).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_provenance: Vec<ReferenceProvenance>,
    /// Executable design assertion results (MCAD-P6-004).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assertions: Vec<AssertionResult>,
}

/// Design body required for in-memory regeneration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RegenBodyParams {
    pub parameters: ParamGraph,
    pub sketches: Vec<Sketch>,
    pub feature_graph: FeatureGraph,
    pub feature_nodes: Vec<opencad_feature::FeatureNode>,
}

impl From<RegenSummary> for RegenResult {
    fn from(summary: RegenSummary) -> Self {
        let report = summary.report;
        Self {
            kernel: summary.kernel,
            regenerated: report.regenerated,
            skipped_suppressed: report.skipped_suppressed,
            volume_m3: summary.volume_m3,
            mass_kg: summary.mass_kg,
            density_kg_per_m3: DEFAULT_DENSITY_KG_PER_M3,
            instances: summary.instances,
            trace: report.trace,
            reference_provenance: report.reference_provenance,
            assertions: summary.assertions,
        }
    }
}

pub fn regen_document(path: &str, sync_topo_refs: bool) -> Result<RegenSummary> {
    if sync_topo_refs {
        let mut doc = read_ocad(path)?;
        let summary = regen_ocad_document(path, &doc)?;
        crate::topo_sync::sync_document_topo_refs(&mut doc)?;
        write_expanded_dir(path, &doc)?;
        return Ok(summary);
    }
    let doc = read_ocad(path)?;
    regen_ocad_document(path, &doc)
}

pub fn regen_ocad_document(path: &str, doc: &OcadDocument) -> Result<RegenSummary> {
    if let Some(assembly) = &doc.assembly {
        return regen_assembly_document(path, doc, assembly);
    }

    let parameters = doc.parameters.clone();
    let assertions = doc.assertions.clone();
    let mut model = doc.clone().into_part_model();
    let mut summary = regenerate_part(&mut model, Some(&parameters), Some(&doc.semantic_refs))?;
    if !assertions.is_empty() {
        summary.assertions = evaluate_part_assertions(
            &part_kernel(),
            &model,
            &parameters,
            &summary.report,
            &assertions,
        );
        if !required_assertions_pass(&summary.assertions) {
            return Err(OpenCadError::validation(
                "regeneration violated a required document assertion",
            ));
        }
    }
    Ok(summary)
}

#[cfg(feature = "occt")]
fn part_kernel() -> OcctGeometryKernel {
    OcctGeometryKernel::new()
}

#[cfg(not(feature = "occt"))]
fn part_kernel() -> opencad_geometry::MockGeometryKernel {
    opencad_geometry::MockGeometryKernel::new()
}

/// Evaluate part document assertions against regenerated evidence.
fn evaluate_part_assertions(
    kernel: &impl GeometryKernel,
    model: &PartModel,
    parameters: &ParamGraph,
    report: &RegenReport,
    assertions: &[Assertion],
) -> Vec<AssertionResult> {
    let parameter_values = evaluate_param_graph(parameters)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let body = model.active_body();
    let mass_kg = body
        .and_then(|body| kernel.mass_properties(body, DEFAULT_DENSITY_KG_PER_M3).ok())
        .map(|mass| mass.mass_kg);
    let bounding_box_size_m = body
        .and_then(|body| kernel.bounding_box(body).ok())
        .map(|bounds| {
            [
                bounds.max[0] - bounds.min[0],
                bounds.max[1] - bounds.min[1],
                bounds.max[2] - bounds.min[2],
            ]
        });
    let context = AssertionContext {
        parameter_values,
        mass_kg,
        bounding_box_size_m,
        body_count: Some(1),
        reference_provenance: report.reference_provenance.clone(),
        assembly_dof: None,
        interference_count: None,
    };
    evaluate_assertions(assertions, &context)
}

pub fn regen_body(body: &RegenBodyParams) -> Result<RegenSummary> {
    let mut model = PartModel::new();
    model.graph = body.feature_graph.clone();
    for sketch in &body.sketches {
        model
            .sketches
            .insert(sketch.id.as_str().to_string(), sketch.clone());
    }
    for node in &body.feature_nodes {
        model.nodes.insert(node.id.clone(), node.clone());
    }
    regenerate_part(&mut model, Some(&body.parameters), None)
}

fn regen_assembly_document(
    path: &str,
    doc: &OcadDocument,
    assembly: &opencad_assembly::AssemblyModel,
) -> Result<RegenSummary> {
    let registry = FeatureRegistry::with_defaults();
    let assembly_root = assembly_root(path);

    #[cfg(feature = "occt")]
    {
        let kernel = OcctGeometryKernel::new();
        let kernel_name = OcctGeometryKernel::occt_version().to_string();
        let report = regenerate_assembly(
            assembly,
            &doc.metadata.id,
            &assembly_root,
            &kernel,
            &registry,
            &mut load_child_document,
        )?;
        let (volume_m3, mass_kg) = report
            .scene
            .mass
            .map(|mass| (Some(mass.volume_m3), Some(mass.mass_kg)))
            .unwrap_or((None, None));
        Ok(RegenSummary {
            kernel: kernel_name,
            report: assembly_regen_report(&report),
            volume_m3,
            mass_kg,
            instances: Some(report.instance_count),
            mate_dof: report.mate_solve.as_ref().map(|solve| solve.dof),
            mate_max_error: report.mate_solve.as_ref().map(|solve| solve.max_error),
            assertions: Vec::new(),
        })
    }

    #[cfg(not(feature = "occt"))]
    {
        let kernel = opencad_geometry::MockGeometryKernel::new();
        let report = regenerate_assembly(
            assembly,
            &doc.metadata.id,
            &assembly_root,
            &kernel,
            &registry,
            &mut load_child_document,
        )?;
        let (volume_m3, mass_kg) = report
            .scene
            .mass
            .map(|mass| (Some(mass.volume_m3), Some(mass.mass_kg)))
            .unwrap_or((None, None));
        Ok(RegenSummary {
            kernel: "MockGeometryKernel".into(),
            report: assembly_regen_report(&report),
            volume_m3,
            mass_kg,
            instances: Some(report.instance_count),
            mate_dof: report.mate_solve.as_ref().map(|solve| solve.dof),
            mate_max_error: report.mate_solve.as_ref().map(|solve| solve.max_error),
            assertions: Vec::new(),
        })
    }
}

fn assembly_regen_report(report: &opencad_assembly::AssemblyRegenReport) -> RegenReport {
    let regenerated: Vec<String> = report
        .instances
        .iter()
        .filter(|instance| matches!(instance.status, InstanceRegenStatus::Ok))
        .map(|instance| instance.instance_id.as_str().to_string())
        .collect();
    let trace =
        RegenerationTrace::observed(regenerated.clone(), Vec::new(), 0, 0, 0, Default::default());
    RegenReport {
        regenerated,
        skipped_suppressed: Vec::new(),
        cached_nodes: Vec::new(),
        face_history: Vec::new(),
        trace,
        reference_provenance: Vec::new(),
    }
}

pub(crate) fn assembly_root(path: &str) -> std::path::PathBuf {
    let path = Path::new(path);
    if path.extension().and_then(|ext| ext.to_str()) == Some("ocad") {
        path.parent()
            .map(|parent| parent.to_path_buf())
            .unwrap_or_else(|| Path::new(".").to_path_buf())
    } else {
        path.to_path_buf()
    }
}

fn load_child_document(path: &Path) -> Result<ResolvedChild> {
    let doc = read_ocad(path)?;
    match doc.metadata.kind {
        DocumentKind::Assembly => {
            let assembly = doc.assembly.ok_or_else(|| {
                opencad_core::OpenCadError::validation(format!(
                    "assembly document '{}' is missing assembly model",
                    path.display()
                ))
            })?;
            Ok(ResolvedChild::Assembly {
                model: Box::new(assembly),
                doc_id: doc.metadata.id,
            })
        }
        DocumentKind::Part => {
            let doc_id = doc.metadata.id.clone();
            let parameters = doc.parameters.clone();
            let semantic_refs = doc.semantic_refs.clone();
            let part = doc.into_part_model();
            Ok(ResolvedChild::Part(Box::new(ChildPart {
                doc_id,
                parameters,
                part,
                semantic_refs,
            })))
        }
        DocumentKind::Drawing => Err(opencad_core::OpenCadError::validation(format!(
            "child document '{}' has drawing kind; expected part or assembly",
            path.display()
        ))),
    }
}

pub fn regenerate_part(
    model: &mut PartModel,
    parameters: Option<&ParamGraph>,
    semantic_refs: Option<&[opencad_geometry::TopoRef]>,
) -> Result<RegenSummary> {
    let registry = FeatureRegistry::with_defaults();

    #[cfg(feature = "occt")]
    {
        let kernel = OcctGeometryKernel::new();
        let kernel_name = OcctGeometryKernel::occt_version().to_string();
        let report = model.regenerate(&kernel, &registry, parameters, semantic_refs)?;
        let (volume_m3, mass_kg) = mass_for_active_body(model, &kernel);
        Ok(RegenSummary {
            kernel: kernel_name,
            report,
            volume_m3,
            mass_kg,
            instances: None,
            mate_dof: None,
            mate_max_error: None,
            assertions: Vec::new(),
        })
    }

    #[cfg(not(feature = "occt"))]
    {
        let kernel = opencad_geometry::MockGeometryKernel::new();
        let report = model.regenerate(&kernel, &registry, parameters, semantic_refs)?;
        let (volume_m3, mass_kg) = mass_for_active_body(model, &kernel);
        Ok(RegenSummary {
            kernel: "MockGeometryKernel".into(),
            report,
            volume_m3,
            mass_kg,
            instances: None,
            mate_dof: None,
            mate_max_error: None,
            assertions: Vec::new(),
        })
    }
}

/// Whether a regeneration inspection found a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegenInspectionStatus {
    Regenerated,
    Failed,
}

/// Failed-regeneration inspection of a part document (MCAD-P6-006).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegenInspectionResult {
    pub kernel: String,
    pub status: RegenInspectionStatus,
    /// The first failing node and how the failure splits the other features.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<RegenerationFailure>,
    /// The feature whose body is reported: the final body on success, the
    /// body the failing feature was building on after a failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_feature: Option<String>,
    pub volume_m3: Option<f64>,
    pub mass_kg: Option<f64>,
    pub density_kg_per_m3: f64,
}

/// Regenerate a part document in memory and report where it fails.
///
/// Read-only: the document on disk is never written, and a failure is a
/// result rather than an error.  Errors are reserved for unreadable or
/// non-part documents.
pub fn inspect_document_regeneration(path: &str) -> Result<RegenInspectionResult> {
    let doc = read_ocad(path)?;
    if doc.metadata.kind != DocumentKind::Part {
        return Err(OpenCadError::validation(
            "regeneration inspection accepts part documents; regenerate an assembly with `regen`",
        ));
    }
    let model = doc.clone().into_part_model();
    let kernel = part_kernel();
    let inspection = model.inspect_regeneration(
        &kernel,
        &FeatureRegistry::with_defaults(),
        Some(&doc.parameters),
        Some(&doc.semantic_refs),
    );
    let body_feature = match &inspection.outcome {
        Ok(_) => inspection
            .model
            .graph
            .ordered_ids()
            .iter()
            .rev()
            .find(|id| {
                inspection
                    .model
                    .outputs
                    .get(*id)
                    .is_some_and(|output| output.body.is_some())
            })
            .cloned(),
        Err(failure) => failure.upstream_body_feature.clone(),
    };
    let mass = body_feature
        .as_ref()
        .and_then(|id| inspection.model.outputs.get(id))
        .and_then(|output| output.body.as_ref())
        .and_then(|body| kernel.mass_properties(body, DEFAULT_DENSITY_KG_PER_M3).ok());
    let (status, failure) = match inspection.outcome {
        Ok(_) => (RegenInspectionStatus::Regenerated, None),
        Err(failure) => (RegenInspectionStatus::Failed, Some(failure)),
    };
    Ok(RegenInspectionResult {
        kernel: part_kernel_name(),
        status,
        failure,
        body_feature,
        volume_m3: mass.as_ref().map(|mass| mass.volume_m3),
        mass_kg: mass.as_ref().map(|mass| mass.mass_kg),
        density_kg_per_m3: DEFAULT_DENSITY_KG_PER_M3,
    })
}

#[cfg(feature = "occt")]
fn part_kernel_name() -> String {
    OcctGeometryKernel::occt_version().to_string()
}

#[cfg(not(feature = "occt"))]
fn part_kernel_name() -> String {
    "MockGeometryKernel".into()
}

fn mass_for_active_body<K: GeometryKernel>(
    model: &PartModel,
    kernel: &K,
) -> (Option<f64>, Option<f64>) {
    let Some(body) = model.active_body() else {
        return (None, None);
    };
    let Ok(mass) = kernel.mass_properties(body, DEFAULT_DENSITY_KG_PER_M3) else {
        return (None, None);
    };
    (Some(mass.volume_m3), Some(mass.mass_kg))
}

pub fn print_summary(summary: &RegenSummary) {
    println!("kernel: {}", summary.kernel);
    if let Some(instances) = summary.instances {
        println!("instances: {instances}");
    }
    println!("regenerated: {} features", summary.report.regenerated.len());
    for id in &summary.report.regenerated {
        println!("  {id}");
    }
    if !summary.report.cached_nodes.is_empty() {
        println!(
            "cached: {} features (unchanged inputs, no kernel work)",
            summary.report.cached_nodes.len()
        );
    }
    println!(
        "trace: solver_calls={} kernel_calls={} elapsed_time_ms={} hash={}",
        summary.report.trace.solver_call_count,
        summary.report.trace.geometry_kernel_call_count,
        summary.report.trace.elapsed_time_ms,
        summary.report.trace.trace_hash_sha256
    );
    if !summary.report.skipped_suppressed.is_empty() {
        println!("suppressed: {}", summary.report.skipped_suppressed.len());
        for id in &summary.report.skipped_suppressed {
            println!("  {id}");
        }
    }
    if let Some(volume) = summary.volume_m3 {
        println!("volume_m3: {volume}");
    }
    if let Some(mass) = summary.mass_kg {
        println!("mass_kg: {mass} (density {DEFAULT_DENSITY_KG_PER_M3} kg/m^3)");
    }
    if let Some(dof) = summary.mate_dof {
        println!("mate_dof: {dof}");
    }
    if let Some(error) = summary.mate_max_error {
        println!("mate_max_error: {error}");
    }
    if !summary.report.reference_provenance.is_empty() {
        let count = |status: ReferenceStatus| {
            summary
                .report
                .reference_provenance
                .iter()
                .filter(|provenance| provenance.status == status)
                .count()
        };
        println!(
            "references: exact={} derived={} fingerprint={} ambiguous={} missing={} consumed={}",
            count(ReferenceStatus::Exact),
            count(ReferenceStatus::Derived),
            count(ReferenceStatus::Fingerprint),
            count(ReferenceStatus::Ambiguous),
            count(ReferenceStatus::Missing),
            count(ReferenceStatus::Consumed)
        );
        for provenance in &summary.report.reference_provenance {
            println!(
                "  {} {:?} {}",
                provenance.ref_id, provenance.status, provenance.reason
            );
        }
    }
    if !summary.assertions.is_empty() {
        for result in &summary.assertions {
            println!(
                "assertion {}: {} ({})",
                result.id,
                if result.passed { "PASS" } else { "FAIL" },
                result.message
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{DocumentId, DocumentMetadata};
    use opencad_feature::bracket_with_hole;
    use opencad_file::{write_expanded_dir, OcadDocument};
    use opencad_graph::bracket_parameters;
    use tempfile::tempdir;

    #[test]
    fn regen_bracket_fixture() {
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Mounting Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        let dir = tempdir().expect("tempdir");
        write_expanded_dir(dir.path(), &doc).expect("write");

        let summary = regen_document(dir.path().to_str().expect("path"), false).expect("regen");
        assert_eq!(summary.report.regenerated.len(), 4);
        assert!(summary.volume_m3.unwrap_or(0.0) > 0.0);
    }

    fn write_bracket(part: &PartModel) -> tempfile::TempDir {
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Mounting Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, part);
        doc.parameters = bracket_parameters();
        let dir = tempdir().expect("tempdir");
        write_expanded_dir(dir.path(), &doc).expect("write");
        dir
    }

    fn file_bytes(dir: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        let mut pending = vec![dir.to_path_buf()];
        while let Some(path) = pending.pop() {
            for entry in std::fs::read_dir(&path).expect("dir") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push((path.clone(), std::fs::read(&path).expect("read")));
                }
            }
        }
        files.sort();
        files
    }

    #[test]
    fn inspecting_a_regenerating_part_reports_its_final_body() {
        let dir = write_bracket(&bracket_with_hole().expect("model"));
        let result =
            inspect_document_regeneration(dir.path().to_str().expect("path")).expect("inspect");
        assert_eq!(result.status, RegenInspectionStatus::Regenerated);
        assert_eq!(result.failure, None);
        assert_eq!(result.body_feature.as_deref(), Some("feature:hole_mount"));
        assert!(result.volume_m3.unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn inspecting_a_failing_part_names_the_failure_and_leaves_the_document_unchanged() {
        let mut part = bracket_with_hole().expect("model");
        part.add_node(opencad_feature::FeatureNode::new(
            "feature:broken",
            "Broken Extrude",
            opencad_feature::FeatureDefinition::Extrude(opencad_feature::ExtrudeFeature {
                sketch_feature: "feature:missing_sketch".into(),
                profile_ref: "sketch:missing/profile:outer".into(),
                extent: opencad_geometry::ExtrudeExtent::Distance {
                    length: opencad_core::Length::from_meters(0.01),
                },
                operation: opencad_geometry::ExtrudeOperation::NewBody,
                length_expr: None,
                target_feature: None,
            }),
        ))
        .expect("node");
        part.add_dependency("feature:extrude_base", "feature:broken")
            .expect("edge");
        let dir = write_bracket(&part);
        let before = file_bytes(dir.path());

        let result =
            inspect_document_regeneration(dir.path().to_str().expect("path")).expect("inspect");
        assert_eq!(result.status, RegenInspectionStatus::Failed);
        let failure = result.failure.expect("failure");
        assert_eq!(failure.node.as_deref(), Some("feature:broken"));
        assert_eq!(result.body_feature.as_deref(), Some("feature:extrude_base"));
        assert!(result.volume_m3.unwrap_or(0.0) > 0.0);
        assert_eq!(file_bytes(dir.path()), before);

        let json = serde_json::to_value(&failure).expect("json");
        assert_eq!(json["stage"], "feature");
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("mkdir");
        for entry in std::fs::read_dir(from).expect("read dir") {
            let entry = entry.expect("entry");
            let target = to.join(entry.file_name());
            if entry.file_type().expect("type").is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).expect("copy");
            }
        }
    }

    /// The scripted inspect-and-repair scenario on the flagship
    /// (MCAD-P6-006, `docs/examples/inspect-and-repair.md`): an unverified
    /// edit breaks the part, inspection names the failing feature and links
    /// it to the edit, and a verified patch repairs it.
    #[test]
    fn the_flagship_inspect_and_repair_scenario() {
        let examples = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
        let dir = tempdir().expect("tempdir");
        let document = dir.path().join("robot_joint_actuator.ocad.d");
        copy_dir(
            &Path::new(examples).join("robot_joint_actuator.ocad.d"),
            &document,
        );
        let document = document.to_str().expect("utf-8").to_string();
        let patch = |name: &str, no_verify: bool| {
            crate::patch::patch_document_with_options(&crate::patch::PatchArgs {
                doc_path: document.clone(),
                patch_path: format!("{examples}/agent/{name}"),
                options: crate::patch::PatchOptions {
                    no_verify,
                    ..Default::default()
                },
            })
        };

        // 1. The 80 mm bore splits the part; only an unverified edit lands.
        assert!(patch("repair_shaft_bore_break_patch.json", false).is_err());
        patch("repair_shaft_bore_break_patch.json", true).expect("staged edit");

        // 2. Inspection names the failing feature, which is not the edited one.
        let broken = inspect_document_regeneration(&document).expect("inspect");
        let failure = broken.failure.expect("failure");
        assert_eq!(failure.stage, opencad_feature::RegenerationStage::Feature);
        assert_eq!(failure.node.as_deref(), Some("feature:radial_ribs"));
        assert!(failure.error.contains("got 2"), "{}", failure.error);
        assert_eq!(
            failure.blocked_features,
            vec!["feature:mounting_ears", "feature:mounting_holes"]
        );
        assert_eq!(
            broken.body_feature.as_deref(),
            Some("feature:pcd_fasteners")
        );

        // 3. The parameter intent links the edit to the failure.
        let query = opencad_ai::DesignQuery::InspectParameter {
            id: "param:shaft_diameter".into(),
        };
        let intent =
            opencad_ai::run_query(&read_ocad(&document).expect("read").into_query_params(query))
                .expect("intent");
        let opencad_ai::QueryResult::ParameterIntent { item } = intent else {
            panic!("unexpected {intent:?}");
        };
        assert!(item
            .predicted_dirty_features
            .iter()
            .any(|id| id == "feature:radial_ribs"));

        // 4. A verified repair regenerates the whole part again.
        patch("repair_shaft_bore_fix_patch.json", false).expect("repair");
        let repaired = inspect_document_regeneration(&document).expect("inspect");
        assert_eq!(repaired.status, RegenInspectionStatus::Regenerated);
        assert_eq!(
            repaired.body_feature.as_deref(),
            Some("feature:mounting_holes")
        );
        let mass = repaired.mass_kg.expect("mass");
        assert!((0.5..0.61).contains(&mass), "{mass} kg");
    }

    #[test]
    fn inspecting_an_assembly_is_rejected() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/assembly_two_brackets.ocad.d"
        );
        let error = inspect_document_regeneration(path).expect_err("assembly");
        assert!(error.to_string().contains("part documents"), "{error}");
    }
}
