//! Semantic diff between `.ocad` documents (Task-127+).

use opencad_ai::diff_design_state;
use opencad_ai::DesignState;
use opencad_graph::DesignDiff;

use crate::OcadDocument;

/// Compare two in-memory documents and return a semantic diff.
pub fn diff_documents(before: &OcadDocument, after: &OcadDocument) -> DesignDiff {
    diff_design_state(
        &DesignState::with_models(
            before.parameters.clone(),
            before.feature_nodes.clone(),
            before.semantic_refs.clone(),
            before.assembly.clone(),
            before.drawing.clone(),
        ),
        &DesignState::with_models(
            after.parameters.clone(),
            after.feature_nodes.clone(),
            after.semantic_refs.clone(),
            after.assembly.clone(),
            after.drawing.clone(),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{DocumentId, DocumentMetadata, TopoRefId};
    use opencad_feature::bracket_with_hole;
    use opencad_geometry::assign_named_face_ref;
    use opencad_graph::{bracket_parameters, SemanticChange};

    #[test]
    fn diff_documents_reports_parameter_change() {
        let part = bracket_with_hole().expect("model");
        let metadata =
            DocumentMetadata::new(DocumentId::new("doc:bracket_001").expect("id"), "Bracket");
        let mut before = OcadDocument::from_part_model(metadata.clone(), &part);
        before.parameters = bracket_parameters();

        let mut after = before.clone();
        after
            .parameters
            .set_expr("param:width", "100 mm")
            .expect("set expr");

        let diff = diff_documents(&before, &after);
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(
            diff.changes[0],
            SemanticChange::ParameterChanged {
                id: "param:width".into(),
                before: "80 mm".into(),
                after: "100 mm".into(),
            }
        );
    }

    #[test]
    fn identical_documents_have_no_changes() {
        let part = bracket_with_hole().expect("model");
        let metadata =
            DocumentMetadata::new(DocumentId::new("doc:bracket_001").expect("id"), "Bracket");
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();

        let diff = diff_documents(&doc, &doc);
        assert!(diff.changes.is_empty());
        assert_eq!(diff.summary, "No changes");
    }

    #[test]
    fn diff_documents_reports_topo_ref_added() {
        let part = bracket_with_hole().expect("model");
        let metadata =
            DocumentMetadata::new(DocumentId::new("doc:bracket_001").expect("id"), "Bracket");
        let mut before = OcadDocument::from_part_model(metadata.clone(), &part);
        before.parameters = bracket_parameters();
        let mut after = before.clone();
        assign_named_face_ref(
            &mut after.semantic_refs,
            TopoRefId::new("ref:face:bracket_top").expect("id"),
            "feature:extrude_base",
            "top",
            None,
            [0.0, 0.0, 1.0],
        )
        .expect("assign");

        let diff = diff_documents(&before, &after);
        assert!(diff.changes.iter().any(|change| matches!(
            change,
            SemanticChange::TopoRefAdded { ref_id, .. }
                if ref_id == "ref:face:bracket_top"
        )));
    }
}
