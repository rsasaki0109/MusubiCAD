//! ADR-019: references to faces a shell opens are reported as consumed
//! (integration test: requires OCCT).

use std::path::PathBuf;

use opencad_ai::{evaluate_assertion, AssertionContext, DesignPatch};
use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
use opencad_feature::{FeatureRegistry, RegenReport};
use opencad_file::{apply_patch_to_document, read_ocad, OcadDocument};
use opencad_geometry::{ReferenceProvenance, ReferenceStatus};
use opencad_kernel_occt::OcctGeometryKernel;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn enclosure() -> OcadDocument {
    let metadata = read_ocad(repo().join("examples/bracket.ocad.d"))
        .expect("bracket")
        .metadata;
    let patch: DesignPatch = serde_json::from_str(
        &std::fs::read_to_string(repo().join("examples/agent/add_shell_patch.json")).expect("read"),
    )
    .expect("patch");
    let mut doc = OcadDocument::new(metadata);
    apply_patch_to_document(&mut doc, &patch).expect("author");
    doc
}

fn regenerate(doc: &OcadDocument) -> RegenReport {
    let parameters = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    doc.clone()
        .into_part_model()
        .regenerate(
            &OcctGeometryKernel::new(),
            &FeatureRegistry::with_defaults(),
            Some(&parameters),
            Some(&semantic_refs),
        )
        .expect("regenerate")
}

fn box_top(report: &RegenReport) -> &ReferenceProvenance {
    report
        .reference_provenance
        .iter()
        .find(|provenance| provenance.ref_id == "ref:face:box_top")
        .expect("box_top provenance")
}

#[test]
fn a_shell_opening_is_reported_as_consumed() {
    let doc = enclosure();
    let report = regenerate(&doc);
    let top = box_top(&report);
    assert_eq!(top.status, ReferenceStatus::Consumed);
    assert_eq!(top.reason, "opened by feature:shell");
    assert_eq!(top.resolved_kernel_id, None);
    assert!(report
        .reference_provenance
        .iter()
        .all(|provenance| provenance.status != ReferenceStatus::Ambiguous));

    // A required reference to the opened face fails and names the shell.
    let assertion = Assertion::new(
        "assertion:top",
        "Top face",
        AssertionSeverity::Required,
        AssertionKind::RequiredReference {
            ref_id: "ref:face:box_top".into(),
        },
    );
    let result = evaluate_assertion(
        &assertion,
        &AssertionContext {
            reference_provenance: report.reference_provenance.clone(),
            ..AssertionContext::default()
        },
    );
    assert!(!result.passed);
    assert!(
        result.message.contains("feature:shell"),
        "{}",
        result.message
    );
}

#[test]
fn a_suppressed_shell_consumes_nothing() {
    let mut doc = enclosure();
    doc.feature_nodes
        .iter_mut()
        .find(|node| node.id == "feature:shell")
        .expect("shell")
        .suppressed = true;
    let report = regenerate(&doc);
    assert_ne!(box_top(&report).status, ReferenceStatus::Consumed);
}
