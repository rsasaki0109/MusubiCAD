//! Apply DesignPatch operations to `.ocad` documents.

use opencad_ai::{dry_run_patch_state, DesignPatch, DesignState, PatchDryRunReport, PatchOperation};

use crate::topo_assign::{apply_assign_face_ref, AssignFaceRefOp};
use crate::OcadDocument;

/// Apply all patch operations to a document in memory.
pub fn apply_patch_to_document(doc: &mut OcadDocument, patch: &DesignPatch) -> opencad_core::Result<()> {
    patch.apply_to_document(
        &mut doc.parameters,
        &mut doc.feature_nodes,
        &mut doc.semantic_refs,
    )?;

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
    dry_run_patch_state(
        &DesignState::with_semantic_refs(
            before.parameters.clone(),
            before.feature_nodes.clone(),
            before.semantic_refs.clone(),
        ),
        patch,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_ai::FeatureExprField;
    use opencad_core::{DocumentId, DocumentMetadata, TopoRefId};
    use opencad_feature::{
        bracket_with_hole, bracket_with_top_fillet, FeatureDefinition, FeatureRegistry,
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
    fn dry_run_reports_feature_expr_change() {
        let before = bracket_document();
        let patch = DesignPatch::set_feature_expr(
            "feature:extrude_base",
            FeatureExprField::LengthExpr,
            "thickness * 2",
        );
        let report = dry_run_patch_document(&before, &patch);
        assert!(report.validation.is_ok());
        assert!(
            report
                .diff
                .changes
                .iter()
                .any(|change| matches!(
                    change,
                    SemanticChange::FeatureModified { id, field, .. }
                        if id == "feature:extrude_base" && field == "definition"
                ))
        );
    }

    #[test]
    fn assign_face_ref_patch_adds_semantic_ref() {
        let mut doc = bracket_document();
        let patch = DesignPatch::assign_face_ref(
            "ref:face:bracket_top",
            "feature:extrude_base",
            "top",
        );
        apply_patch_to_document(&mut doc, &patch).expect("patch");
        assert!(doc
            .semantic_refs
            .iter()
            .any(|topo_ref| topo_ref.ref_id.as_str() == "ref:face:bracket_top"));
    }

    #[test]
    fn dry_run_reports_assign_face_ref_change() {
        let before = bracket_document();
        let patch = DesignPatch::assign_face_ref(
            "ref:face:bracket_top",
            "feature:extrude_base",
            "top",
        );
        let report = dry_run_patch_document(&before, &patch);
        assert!(report.validation.is_ok());
        assert!(
            report
                .diff
                .changes
                .iter()
                .any(|change| matches!(
                    change,
                    SemanticChange::TopoRefAdded { ref_id, .. }
                        if ref_id == "ref:face:bracket_top"
                ))
        );
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
        assert!(
            diff.changes
                .iter()
                .any(|change| matches!(
                    change,
                    SemanticChange::TopoRefAdded { ref_id, .. }
                        if ref_id == "ref:face:mount_face"
                ))
        );
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
        let baseline_mass = kernel
            .mass_properties(baseline_body, 2700.0)
            .expect("mass");

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
        let baseline_mass = kernel
            .mass_properties(baseline_body, 2700.0)
            .expect("mass");

        assert!(
            mass.volume_m3 < baseline_mass.volume_m3,
            "larger fillet radius should remove more material: {} vs {}",
            mass.volume_m3,
            baseline_mass.volume_m3
        );
    }
}
