//! MCAD-P6-006: the intent inspector answers "what drives this?" and "what
//! will this change?" from the Design Graph alone (no geometry kernel).

use std::path::PathBuf;

use opencad_ai::{run_query, DesignPatch, DesignQuery, QueryParams, QueryResult};
use opencad_file::{apply_patch_to_document, read_ocad, OcadDocument};
use serde_json::json;

/// The example bracket with a derived `half_width` parameter and one
/// assertion of each kind the inspector reports.
fn bracket() -> OcadDocument {
    let mut doc =
        read_ocad(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bracket.ocad.d"))
            .expect("bracket");
    let patch: DesignPatch = serde_json::from_value(json!({ "operations": [
        { "type": "add_parameter", "id": "param:half_width", "name": "half_width", "expr": "width / 2" },
        { "type": "add_assertion", "assertion": {
            "id": "assertion:half_width", "name": "Half width", "severity": "required",
            "type": "parameter_range", "parameter_name": "half_width", "min_m": 0.01, "max_m": 0.1 } },
        { "type": "add_assertion", "assertion": {
            "id": "assertion:mass", "name": "Mass", "severity": "advisory",
            "type": "mass_range", "min_kg": 0.01, "max_kg": 1.0 } },
        { "type": "add_assertion", "assertion": {
            "id": "assertion:top", "name": "Top face", "severity": "required",
            "type": "required_reference", "ref_id": "ref:face:bracket_top" } }
    ] }))
    .expect("patch");
    apply_patch_to_document(&mut doc, &patch).expect("apply");
    doc
}

fn query(doc: &OcadDocument, query: DesignQuery) -> QueryResult {
    run_query(&doc.clone().into_query_params(query)).expect("query")
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

#[test]
fn a_parameter_reports_what_it_drives() {
    let doc = bracket();
    let QueryResult::ParameterIntent { item } = query(
        &doc,
        DesignQuery::InspectParameter {
            id: "param:width".into(),
        },
    ) else {
        panic!("expected a parameter intent");
    };
    assert_eq!(item.parameter.name, "width");
    assert!(item.driven_by.is_empty());
    assert_eq!(item.drives_parameters, strings(&["param:half_width"]));
    assert_eq!(item.sketches, strings(&["sketch:base"]));
    assert_eq!(
        item.directly_affected_features,
        strings(&["feature:sketch_base"])
    );
    // The hole sits on the base's top face, so it regenerates too.
    assert_eq!(
        item.predicted_dirty_features,
        strings(&[
            "feature:sketch_base",
            "feature:extrude_base",
            "feature:hole_mount"
        ])
    );
    assert_eq!(
        item.assertions,
        strings(&["assertion:half_width", "assertion:mass"])
    );
}

#[test]
fn a_derived_parameter_reports_what_drives_it() {
    let doc = bracket();
    let QueryResult::ParameterIntent { item } = query(
        &doc,
        DesignQuery::InspectParameter {
            id: "param:half_width".into(),
        },
    ) else {
        panic!("expected a parameter intent");
    };
    assert_eq!(item.driven_by, strings(&["param:width"]));
    assert!(item.drives_parameters.is_empty());
    assert!((item.parameter.value_m.expect("value") - 0.04).abs() < 1e-12);
    // Nothing reads it yet, so an edit regenerates nothing and only its own
    // range assertion is re-evaluated.
    assert!(item.predicted_dirty_features.is_empty());
    assert_eq!(item.assertions, strings(&["assertion:half_width"]));
}

#[test]
fn a_reference_reports_its_origin_and_consumers() {
    let doc = bracket();
    let QueryResult::ReferenceIntent { item } = query(
        &doc,
        DesignQuery::InspectReference {
            ref_id: "ref:face:bracket_top".into(),
        },
    ) else {
        panic!("expected a reference intent");
    };
    assert_eq!(item.reference.created_by, "feature:extrude_base");
    assert_eq!(item.reference.role.as_deref(), Some("top"));
    assert_eq!(item.consuming_features, strings(&["feature:hole_mount"]));
    assert_eq!(
        item.predicted_dirty_features,
        strings(&["feature:hole_mount"])
    );
    assert!(item.consuming_mates.is_empty());
    assert_eq!(item.assertions, strings(&["assertion:top"]));
}

#[test]
fn a_mate_consumes_the_reference_it_names() {
    let doc = read_ocad(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/assembly_two_brackets.ocad.d"),
    )
    .expect("assembly");
    let mut params = doc.into_query_params(DesignQuery::InspectReference {
        ref_id: "ref:face:left_origin".into(),
    });
    // Mate entity refs live in the part, not the assembly's own ref table:
    // an unknown ref is reported as not found.
    assert!(run_query(&params).is_err());
    params.semantic_refs.push(
        serde_json::from_value(json!({
            "ref_id": "ref:face:left_origin", "kind": "face",
            "semantic": { "created_by": "feature:extrude_base", "role": "origin" }
        }))
        .expect("ref"),
    );
    let QueryResult::ReferenceIntent { item } = run_query(&params).expect("query") else {
        panic!("expected a reference intent");
    };
    assert_eq!(item.consuming_mates, strings(&["mate:spacing"]));
    assert!(item.consuming_features.is_empty());
}

#[test]
fn inspecting_an_unknown_parameter_fails() {
    let doc = bracket();
    let params = QueryParams {
        parameters: doc.parameters.clone(),
        feature_nodes: Vec::new(),
        feature_graph: None,
        sketches: Vec::new(),
        scene: None,
        semantic_refs: Vec::new(),
        assembly: None,
        drawing: None,
        assertions: Vec::new(),
        query: DesignQuery::InspectParameter {
            id: "param:nope".into(),
        },
    };
    assert!(run_query(&params).is_err());
}
