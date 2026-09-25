//! Apply DesignPatch operations to `.ocad` documents.

use opencad_ai::{
    build_patch_candidate, dry_run_patch_state_with_context, DesignPatch, DesignState,
    ImpactContext, PatchDryRunReport, PatchOperation,
};
use opencad_core::Result;

use crate::topo_assign::{apply_assign_face_ref, AssignFaceRefOp};
use crate::{DocumentHistory, OcadDocument};

/// Apply all patch operations to a document in memory.
pub fn apply_patch_to_document(doc: &mut OcadDocument, patch: &DesignPatch) -> Result<()> {
    let mut candidate = doc.clone();
    apply_patch_to_document_in_place(&mut candidate, patch)?;
    *doc = candidate;
    Ok(())
}

/// Apply a validated patch and record the complete before/after document
/// snapshots in backend history.
///
/// The patch is evaluated against a disposable candidate.  If validation or
/// any operation fails, neither `doc` nor `history` is changed.  A successful
/// record clears the redo branch, as required for a new edit after undo.
pub fn apply_patch_with_history(
    doc: &mut OcadDocument,
    patch: &DesignPatch,
    history: &mut DocumentHistory,
    description: impl Into<String>,
) -> Result<()> {
    let before = doc.clone();
    let mut candidate = before.clone();
    apply_patch_to_document_in_place(&mut candidate, patch)?;
    history.record(before, candidate.clone(), description);
    *doc = candidate;
    Ok(())
}

/// Build the complete patchable [`DesignState`] of a document.
///
/// Every document-backed patch, diff, and revision uses this projection so
/// that `musubicad.design-state.v2` digests agree across surfaces.
pub fn document_design_state(doc: &OcadDocument) -> DesignState {
    DesignState::with_models(
        doc.parameters.clone(),
        doc.feature_nodes.clone(),
        doc.semantic_refs.clone(),
        doc.assembly.clone(),
        doc.drawing.clone(),
    )
    .with_authoring(doc.sketches.clone(), doc.assertions.clone())
    .with_feature_order(doc.feature_graph.ordered_ids().to_vec())
}

fn apply_patch_to_document_in_place(doc: &mut OcadDocument, patch: &DesignPatch) -> Result<()> {
    let state = document_design_state(doc);
    let next = build_patch_candidate(&state, patch)?;
    // The persisted Feature Graph is re-derived only when its inputs change,
    // so value edits never rewrite `graph/features.json` edge order.
    if patch.changes_feature_graph() {
        doc.feature_graph = next.derive_feature_graph()?;
    }
    doc.parameters = next.parameters;
    doc.feature_nodes = next.feature_nodes;
    doc.semantic_refs = next.semantic_refs;
    doc.assembly = next.assembly;
    doc.drawing = next.drawing;
    doc.sketches = next.sketches;
    doc.assertions = next.assertions;

    for operation in &patch.operations {
        let PatchOperation::AssignFaceRef {
            ref_id,
            kernel_face_id,
            created_by,
            role,
            normal_m,
        } = operation
        else {
            continue;
        };
        apply_assign_face_ref(
            doc,
            &AssignFaceRefOp::new(ref_id, *kernel_face_id, created_by, role, *normal_m),
        )?;
    }

    Ok(())
}

/// Validate and preview a patch against a document without persisting changes.
pub fn dry_run_patch_document(before: &OcadDocument, patch: &DesignPatch) -> PatchDryRunReport {
    dry_run_patch_state_with_context(
        &document_design_state(before),
        patch,
        ImpactContext {
            feature_graph: Some(&before.feature_graph),
            sketches: &before.sketches,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_ai::FeatureExprField;
    use opencad_core::{DocumentId, DocumentMetadata, TopoRefId};
    use opencad_feature::{
        bracket_with_hole, bracket_with_top_fillet, robot_joint_actuator_housing,
        FeatureDefinition, FeatureRegistry,
    };
    use opencad_geometry::{assign_named_face_ref, GeometryKernel};
    use opencad_graph::{bracket_parameters, SemanticChange};
    use opencad_kernel_occt::OcctGeometryKernel;

    fn bracket_document() -> OcadDocument {
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        doc
    }

    #[test]
    fn apply_patch_updates_feature_expr_and_parameters() {
        let mut doc = bracket_document();
        let patch = DesignPatch::new(vec![
            opencad_ai::PatchOperation::SetParameter {
                id: "param:thickness".into(),
                expr: "8 mm".into(),
            },
            opencad_ai::PatchOperation::SetFeatureExpr {
                feature_id: "feature:extrude_base".into(),
                field: FeatureExprField::LengthExpr.as_str().to_string(),
                expr: "thickness * 2".into(),
            },
        ]);
        apply_patch_to_document(&mut doc, &patch).expect("patch");

        let values = opencad_graph::evaluate_param_graph(&doc.parameters).expect("eval");
        assert!((values["thickness"] - 0.008).abs() < 1e-9);

        let node = doc
            .feature_nodes
            .iter()
            .find(|node| node.id == "feature:extrude_base")
            .expect("extrude");
        let FeatureDefinition::Extrude(extrude) = &node.definition else {
            panic!("expected extrude");
        };
        assert_eq!(extrude.length_expr.as_deref(), Some("thickness * 2"));
    }

    #[test]
    fn apply_patch_with_history_is_atomic_and_records_full_document() {
        let mut doc = bracket_document();
        let before = doc.clone();
        let mut history = DocumentHistory::default();
        let patch = DesignPatch::set_parameter("param:width", "100 mm");

        apply_patch_with_history(&mut doc, &patch, &mut history, "Set width")
            .expect("history patch");
        assert_ne!(doc, before);
        assert_eq!(history.undo_len(), 1);
        let after = doc.clone();
        history.undo(&mut doc).expect("undo recorded patch");
        assert_eq!(doc, before);
        history.redo(&mut doc).expect("redo recorded patch");
        assert_eq!(doc, after);

        let original_doc = doc.clone();
        let original_history = history.clone();
        let invalid = DesignPatch::set_parameter("param:width", "not_a_length");
        apply_patch_with_history(&mut doc, &invalid, &mut history, "Invalid width")
            .expect_err("invalid patch");
        assert_eq!(doc, original_doc);
        assert_eq!(history, original_history);
    }

    #[test]
    fn dry_run_reports_feature_expr_change() {
        let before = bracket_document();
        let patch = DesignPatch::set_feature_expr(
            "feature:extrude_base",
            FeatureExprField::LengthExpr,
            "thickness * 2",
        );
        let report = dry_run_patch_document(&before, &patch);
        assert!(report.validation.is_ok());
        assert!(report.diff.changes.iter().any(|change| matches!(
            change,
            SemanticChange::FeatureModified { id, field, .. }
                if id == "feature:extrude_base" && field == "definition"
        )));
    }

    #[test]
    fn robot_joint_tower_edit_predicts_exact_downstream_suffix() {
        let part = robot_joint_actuator_housing().expect("robot joint");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:impact_robot_joint").expect("id"),
            "Impact Robot Joint",
        );
        let mut before = OcadDocument::from_part_model(metadata, &part);
        before.parameters = opencad_graph::robot_joint_housing_parameters();
        let report = dry_run_patch_document(
            &before,
            &DesignPatch::set_parameter("param:upper_hub_height", "42 mm"),
        );

        assert!(report.validation.is_ok());
        assert_eq!(
            report.impact.directly_affected_nodes,
            ["feature:upper_hub", "feature:shaft_bore"]
        );
        assert_eq!(
            report.impact.predicted_dirty_nodes,
            [
                "feature:upper_hub",
                "feature:shaft_bore",
                "feature:counterbore",
                "feature:pcd_fasteners",
                "feature:radial_ribs",
                "feature:mounting_ears",
                "feature:mounting_holes",
            ]
        );
    }

    #[test]
    fn apply_and_dry_run_share_validation_error_without_mutation() {
        let before = bracket_document();
        let patch = DesignPatch::set_parameter("param:width", "not_a_length");
        let report = dry_run_patch_document(&before, &patch);
        let mut after = before.clone();
        let apply_error = apply_patch_to_document(&mut after, &patch).expect_err("invalid expr");

        assert!(!report.validation.is_ok());
        assert_eq!(report.validation.messages.len(), 1);
        assert_eq!(
            report.validation.messages[0].message,
            apply_error.to_string()
        );
        assert_eq!(before, after);
    }

    #[test]
    fn assign_face_ref_patch_adds_semantic_ref() {
        let mut doc = bracket_document();
        let patch =
            DesignPatch::assign_face_ref("ref:face:bracket_top", "feature:extrude_base", "top");
        apply_patch_to_document(&mut doc, &patch).expect("patch");
        assert!(doc
            .semantic_refs
            .iter()
            .any(|topo_ref| topo_ref.ref_id.as_str() == "ref:face:bracket_top"));
    }

    #[test]
    fn dry_run_reports_assign_face_ref_change() {
        let before = bracket_document();
        let patch =
            DesignPatch::assign_face_ref("ref:face:bracket_top", "feature:extrude_base", "top");
        let report = dry_run_patch_document(&before, &patch);
        assert!(report.validation.is_ok());
        assert!(report.diff.changes.iter().any(|change| matches!(
            change,
            SemanticChange::TopoRefAdded { ref_id, .. }
                if ref_id == "ref:face:bracket_top"
        )));
    }

    #[test]
    fn diff_documents_reports_topo_ref_assignment() {
        let mut before = bracket_document();
        assign_named_face_ref(
            &mut before.semantic_refs,
            TopoRefId::new("ref:face:bracket_top").expect("id"),
            "feature:extrude_base",
            "top",
            None,
            [0.0, 0.0, 1.0],
        )
        .expect("assign");
        let mut after = before.clone();
        assign_named_face_ref(
            &mut after.semantic_refs,
            TopoRefId::new("ref:face:mount_face").expect("id"),
            "feature:extrude_base",
            "top",
            None,
            [0.0, 0.0, 1.0],
        )
        .expect("assign");

        let diff = crate::diff::diff_documents(&before, &after);
        assert!(diff.changes.iter().any(|change| matches!(
            change,
            SemanticChange::TopoRefAdded { ref_id, .. }
                if ref_id == "ref:face:mount_face"
        )));
    }

    #[test]
    fn feature_expr_patch_doubles_extrude_height() {
        let doc = bracket_document();
        let patch = DesignPatch::set_feature_expr(
            "feature:extrude_base",
            FeatureExprField::LengthExpr,
            "thickness * 2",
        );
        let mut patched = doc.clone();
        apply_patch_to_document(&mut patched, &patch).expect("patch");

        let params = patched.parameters.clone();
        let mut model = patched.into_part_model();
        let kernel = OcctGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        model
            .regenerate(&kernel, &registry, Some(&params), None)
            .expect("regen");
        let body = model.active_body().expect("body");
        let mass = kernel.mass_properties(body, 2700.0).expect("mass");

        let baseline = bracket_document();
        let baseline_params = baseline.parameters.clone();
        let mut baseline_model = baseline.into_part_model();
        baseline_model
            .regenerate(&kernel, &registry, Some(&baseline_params), None)
            .expect("regen");
        let baseline_body = baseline_model.active_body().expect("body");
        let baseline_mass = kernel.mass_properties(baseline_body, 2700.0).expect("mass");

        assert!(mass.volume_m3 > baseline_mass.volume_m3);
    }

    #[test]
    fn fillet_radius_expr_patch_increases_fillet_volume_delta() {
        let part = bracket_with_top_fillet().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_fillet").expect("id"),
            "Bracket with Fillet",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();

        let patch = DesignPatch::set_feature_expr(
            "feature:fillet_top",
            FeatureExprField::RadiusExpr,
            "fillet_radius * 2",
        );
        let mut patched = doc.clone();
        apply_patch_to_document(&mut patched, &patch).expect("patch");

        let kernel = OcctGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();

        let params = patched.parameters.clone();
        let mut model = patched.into_part_model();
        model
            .regenerate(&kernel, &registry, Some(&params), None)
            .expect("regen");
        let body = model.active_body().expect("body");
        let mass = kernel.mass_properties(body, 2700.0).expect("mass");

        let baseline_params = doc.parameters.clone();
        let mut baseline_model = doc.into_part_model();
        baseline_model
            .regenerate(&kernel, &registry, Some(&baseline_params), None)
            .expect("regen");
        let baseline_body = baseline_model.active_body().expect("body");
        let baseline_mass = kernel.mass_properties(baseline_body, 2700.0).expect("mass");

        assert!(
            mass.volume_m3 < baseline_mass.volume_m3,
            "larger fillet radius should remove more material: {} vs {}",
            mass.volume_m3,
            baseline_mass.volume_m3
        );
    }
}
