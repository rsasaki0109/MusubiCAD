//! Intent inspection for the desktop Intent Inspector (MCAD-P6-006).
//!
//! The same read-only queries the CLI (`musubicad intent`) and the Agent API
//! (`opencad.inspect_regeneration_document`, `opencad.query_document`) use:
//! where a part stops regenerating, and what drives a parameter or semantic
//! reference and what an edit to it changes.  Nothing here writes a document.

use opencad_ai::{DesignQuery, ParameterIntent, QueryResult, ReferenceIntent};
use opencad_core::{DocumentKind, OpenCadError, Result};
use opencad_feature::{FeatureRegistry, RegenerationFailure};
use opencad_file::{read_ocad, OcadDocument};
use opencad_geometry::GeometryKernel;
use serde::{Deserialize, Serialize};

#[cfg(feature = "occt")]
use opencad_kernel_occt::OcctGeometryKernel;

/// Density for the mass a regeneration inspection reports (aluminium).
pub const DEFAULT_DENSITY_KG_PER_M3: f64 = 2700.0;

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
    inspect_ocad_regeneration(&read_ocad(path)?)
}

/// [`inspect_document_regeneration`] for a document already in memory, such
/// as a patched copy that was never written.
pub fn inspect_ocad_regeneration(doc: &OcadDocument) -> Result<RegenInspectionResult> {
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

#[cfg(feature = "occt")]
fn part_kernel() -> OcctGeometryKernel {
    OcctGeometryKernel::new()
}

#[cfg(not(feature = "occt"))]
fn part_kernel() -> opencad_geometry::MockGeometryKernel {
    opencad_geometry::MockGeometryKernel::new()
}

/// What drives a parameter and what an edit to it changes.
pub fn inspect_parameter_intent(path: &str, id: &str) -> Result<ParameterIntent> {
    let query = DesignQuery::InspectParameter { id: id.to_string() };
    match opencad_ai::run_query(&read_ocad(path)?.into_query_params(query))? {
        QueryResult::ParameterIntent { item } => Ok(item),
        _ => Err(OpenCadError::validation(
            "unexpected parameter intent result",
        )),
    }
}

/// A semantic reference's origin and everything that consumes it.
pub fn inspect_reference_intent(path: &str, ref_id: &str) -> Result<ReferenceIntent> {
    let query = DesignQuery::InspectReference {
        ref_id: ref_id.to_string(),
    };
    match opencad_ai::run_query(&read_ocad(path)?.into_query_params(query))? {
        QueryResult::ReferenceIntent { item } => Ok(item),
        _ => Err(OpenCadError::validation(
            "unexpected reference intent result",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BRACKET: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/bracket.ocad.d");

    #[test]
    fn the_bracket_regenerates_to_its_hole() {
        let result = inspect_document_regeneration(BRACKET).expect("inspect");
        assert_eq!(result.status, RegenInspectionStatus::Regenerated);
        assert_eq!(result.body_feature.as_deref(), Some("feature:hole_mount"));
        assert!(result.mass_kg.unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn parameter_intent_predicts_the_features_an_edit_regenerates() {
        let item = inspect_parameter_intent(BRACKET, "param:thickness").expect("intent");
        assert_eq!(
            item.predicted_dirty_features,
            vec!["feature:extrude_base", "feature:hole_mount"]
        );
        assert!(inspect_parameter_intent(BRACKET, "param:missing").is_err());
    }

    #[test]
    fn reference_intent_names_its_creator_and_consumers() {
        let item = inspect_reference_intent(BRACKET, "ref:face:bracket_top").expect("intent");
        assert_eq!(item.reference.created_by, "feature:extrude_base");
        assert!(item
            .consuming_features
            .iter()
            .any(|id| id == "feature:hole_mount"));
    }
}
