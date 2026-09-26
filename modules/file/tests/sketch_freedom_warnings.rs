//! Dry-run warns about under-constrained sketches a patch touches
//! (MCAD-P7-009).

use std::path::PathBuf;

use opencad_ai::DesignPatch;
use opencad_core::ValidationLevel;
use opencad_file::{dry_run_patch_document, read_ocad, OcadDocument};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bracket() -> OcadDocument {
    read_ocad(repo().join("examples/bracket.ocad.d")).expect("bracket")
}

/// `(code, target)` of every warning in a dry-run of `patch` on `doc`.
fn warnings(doc: &OcadDocument, patch: &DesignPatch) -> Vec<(String, Option<String>)> {
    let report = dry_run_patch_document(doc, patch);
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    report
        .validation
        .messages
        .iter()
        .filter(|message| message.level == ValidationLevel::Warning)
        .map(|message| (message.code.clone(), message.target_id.clone()))
        .collect()
}

fn plate_patch() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(repo().join("examples/agent/author_plate_from_empty_patch.json"))
            .expect("read"),
    )
    .expect("json")
}

#[test]
fn a_parameter_edit_warns_about_the_sketch_it_can_skew() {
    // The example bracket is fully constrained, so a width edit is quiet.
    assert!(warnings(
        &bracket(),
        &DesignPatch::set_parameter("param:width", "90 mm")
    )
    .is_empty());

    // Without its horizontal/vertical constraints the base sketch is held
    // by its side lengths only, and the same edit warns.
    let mut doc = bracket();
    let operations: Vec<serde_json::Value> = [
        "con:e0_horizontal",
        "con:e2_horizontal",
        "con:e1_vertical",
        "con:e3_vertical",
    ]
    .iter()
    .map(|id| {
        serde_json::json!({
            "type": "remove_sketch_constraint", "sketch_id": "sketch:base", "constraint_id": id
        })
    })
    .collect();
    let loosen: DesignPatch =
        serde_json::from_value(serde_json::json!({ "operations": operations })).expect("patch");
    opencad_file::apply_patch_to_document(&mut doc, &loosen).expect("loosen");
    assert_eq!(
        warnings(&doc, &DesignPatch::set_parameter("param:width", "90 mm")),
        vec![(
            "sketch_under_constrained".to_string(),
            Some("sketch:base".to_string())
        )]
    );
    // A parameter no sketch uses touches no sketch.
    assert!(warnings(
        &doc,
        &DesignPatch::set_parameter("param:fillet_radius", "2 mm")
    )
    .is_empty());
}

#[test]
fn fully_constrained_authoring_passes_and_dropping_shape_constraints_warns() {
    let empty = OcadDocument::new(bracket().metadata);
    let full: DesignPatch = serde_json::from_value(plate_patch()).expect("patch");
    assert!(warnings(&empty, &full).is_empty());

    let mut loose = plate_patch();
    loose["operations"]
        .as_array_mut()
        .expect("operations")
        .retain(|operation| {
            !matches!(
                operation["constraint"]["type"].as_str(),
                Some("horizontal" | "vertical")
            )
        });
    let loose: DesignPatch = serde_json::from_value(loose).expect("patch");
    assert_eq!(
        warnings(&empty, &loose),
        vec![(
            "sketch_under_constrained".to_string(),
            Some("sketch:base".to_string())
        )]
    );
}
