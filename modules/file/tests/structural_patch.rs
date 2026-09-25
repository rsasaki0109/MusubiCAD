//! ADR-013 slice 1: document-backed structural patches persist through
//! `.ocad.d` and leave document and history untouched on failure.

use std::path::PathBuf;

use opencad_ai::{
    design_state_revision, DesignPatch, PatchOperation, PatchPrecondition,
    DESIGN_STATE_REVISION_ALGORITHM, DESIGN_STATE_REVISION_VERSION_V2,
};
use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
use opencad_file::expanded_dir::serialize_document_files;
use opencad_file::{
    apply_patch_with_history, document_design_state, read_ocad, write_expanded_dir, DocumentHistory,
};

fn bracket_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bracket.ocad.d")
}

fn authoring_patch() -> DesignPatch {
    DesignPatch::new(vec![
        PatchOperation::AddParameter {
            id: "param:rib_depth".into(),
            name: "rib_depth".into(),
            expr: "thickness * 2".into(),
        },
        PatchOperation::AddAssertion {
            assertion: Assertion::new(
                "assertion:rib_depth_range",
                "Rib depth range",
                AssertionSeverity::Required,
                AssertionKind::ParameterRange {
                    parameter_name: "rib_depth".into(),
                    min_m: 0.004,
                    max_m: 0.02,
                },
            ),
        },
    ])
}

#[test]
fn structural_patch_persists_parameters_and_assertions_through_ocad_d() {
    let mut doc = read_ocad(bracket_path()).expect("read bracket");
    let mut history = DocumentHistory::default();
    let before = document_design_state(&doc);
    let patch = DesignPatch {
        preconditions: vec![PatchPrecondition::RevisionEquals {
            algorithm: DESIGN_STATE_REVISION_ALGORITHM.into(),
            version: DESIGN_STATE_REVISION_VERSION_V2.into(),
            digest: design_state_revision(&before).expect("revision"),
        }],
        ..authoring_patch()
    };

    apply_patch_with_history(&mut doc, &patch, &mut history, "add rib depth").expect("apply");
    assert!(history.can_undo());

    let dir = tempfile::tempdir().expect("tempdir");
    write_expanded_dir(dir.path(), &doc).expect("write");
    let restored = read_ocad(dir.path()).expect("read back");
    assert_eq!(
        restored
            .parameters
            .get("param:rib_depth")
            .map(|entry| entry.expr.as_str()),
        Some("thickness * 2")
    );
    assert_eq!(restored.assertions.len(), 1);
    assert_eq!(restored.assertions[0].id, "assertion:rib_depth_range");
    assert_eq!(
        serialize_document_files(&restored).expect("serialize"),
        serialize_document_files(&doc).expect("serialize"),
        "structural edits must round-trip to identical canonical bytes"
    );
}

#[test]
fn failed_structural_patch_leaves_document_and_history_unchanged() {
    let mut doc = read_ocad(bracket_path()).expect("read bracket");
    let mut history = DocumentHistory::default();
    let original = serialize_document_files(&doc).expect("serialize");

    // Adding succeeds locally, but removing a referenced parameter makes the
    // final state invalid, so nothing may be committed.
    let mut patch = authoring_patch();
    patch.operations.push(PatchOperation::RemoveParameter {
        id: "param:thickness".into(),
    });
    let error =
        apply_patch_with_history(&mut doc, &patch, &mut history, "invalid").expect_err("reject");
    assert!(error
        .to_string()
        .contains("cannot remove parameter 'param:thickness'"));

    assert_eq!(serialize_document_files(&doc).expect("serialize"), original);
    assert_eq!(history, DocumentHistory::default());
}
