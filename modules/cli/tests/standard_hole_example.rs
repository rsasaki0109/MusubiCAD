//! Standard Hole example, structural authoring and file migration evidence.

use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_file::{
    apply_patch_to_document, document_design_state, read_expanded_dir, write_expanded_dir,
    OcadDocument,
};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn authored() -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:standard_hole_plate").expect("id"),
        "Standard Hole Plate",
    ));
    let patch: DesignPatch = serde_json::from_str(
        &std::fs::read_to_string(
            root().join("examples/agent/create_standard_hole_plate_patch.json"),
        )
        .expect("patch"),
    )
    .expect("json");
    apply_patch_to_document(&mut doc, &patch).expect("apply");
    doc
}

#[test]
#[ignore = "refresh committed standard hole example via structural DesignPatch"]
fn write_standard_hole_example() {
    write_expanded_dir(
        root().join("examples/standard_hole_plate.ocad.d"),
        &authored(),
    )
    .expect("write");
}

#[test]
fn standard_hole_example_round_trips_and_rebuilds_through_patch() {
    let expected = authored();
    let actual =
        read_expanded_dir(root().join("examples/standard_hole_plate.ocad.d")).expect("read");
    assert_eq!(
        document_design_state(&actual),
        document_design_state(&expected)
    );
    let patch = authoring_patch(&document_design_state(&actual)).expect("authoring patch");
    let mut rebuilt = OcadDocument::new(actual.metadata.clone());
    apply_patch_to_document(&mut rebuilt, &patch).expect("rebuild");
    assert_eq!(
        document_design_state(&rebuilt),
        document_design_state(&actual)
    );
    let dir = tempfile::tempdir().expect("temp");
    write_expanded_dir(dir.path(), &rebuilt).expect("write");
    assert_eq!(read_expanded_dir(dir.path()).expect("round trip"), rebuilt);
}

#[test]
fn standard_hole_example_occt_volume_and_invalid_patch_are_atomic() {
    use opencad_feature::FeatureRegistry;
    use opencad_geometry::GeometryKernel;
    let mut doc = authored();
    let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            Some(&doc.semantic_refs),
        )
        .expect("regen");
    let volume = kernel
        .mass_properties(model.active_body().expect("body"), 1.0)
        .expect("mass")
        .volume_m3;
    let expected = (80.0 * 60.0 * 12.0
        - std::f64::consts::PI * (3.3_f64.powi(2) * 12.0 + 2.5_f64.powi(2) * 8.0))
        * 1e-9;
    assert!((volume - expected).abs() < 1e-13);

    let before = doc.clone();
    let patch: DesignPatch = serde_json::from_value(serde_json::json!({
        "operations": [{"type":"set_parameter", "id":"param:tap_depth", "expr":"0 mm"}]
    }))
    .expect("patch");
    assert!(apply_patch_to_document(&mut doc, &patch).is_err());
    assert_eq!(doc, before);

    let patch: DesignPatch = serde_json::from_value(serde_json::json!({
        "operations": [{"type":"set_parameter", "id":"param:tap_depth", "expr":"10 mm"}]
    }))
    .expect("patch");
    apply_patch_to_document(&mut doc, &patch).expect("edit");
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            Some(&doc.semantic_refs),
        )
        .expect("edit regen");
    let volume = kernel
        .mass_properties(model.active_body().expect("body"), 1.0)
        .expect("mass")
        .volume_m3;
    let expected = (80.0 * 60.0 * 12.0
        - std::f64::consts::PI * (3.3_f64.powi(2) * 12.0 + 2.5_f64.powi(2) * 10.0))
        * 1e-9;
    assert!((volume - expected).abs() < 1e-13);
}

#[test]
fn verified_agent_refuses_a_standard_diameter_mismatch_without_persisting() {
    use opencad_ai::JsonRpcRequest;
    use std::io::Write;
    use std::process::{Command, Stdio};
    fn call(request: &JsonRpcRequest) -> opencad_ai::JsonRpcResponse {
        let mut process = Command::new(env!("CARGO_BIN_EXE_musubicad"))
            .arg("agent")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("agent");
        {
            let mut stdin = process.stdin.take().expect("stdin");
            writeln!(stdin, "{}", serde_json::to_string(request).expect("json")).expect("write");
        }
        let output = process.wait_with_output().expect("wait");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("response")
    }
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("standard.ocad.d");
    let doc = authored();
    write_expanded_dir(&path, &doc).expect("write");
    for method in [
        "opencad.patch_dry_run_document",
        "opencad.patch_apply_document",
    ] {
        let request: JsonRpcRequest = serde_json::from_value(serde_json::json!({
            "jsonrpc":"2.0", "id":1, "method":method,
            "params":{"path":path,"patch":{"operations":[{"type":"set_parameter","id":"param:tap_drill_diameter","expr":"6 mm"}]}}
        })).expect("request");
        let response = call(&request);
        // A dry-run reports verification failure; apply returns an RPC error.
        if method == "opencad.patch_dry_run_document" {
            let result = response.result.expect("dry run");
            assert_eq!(result["verification"]["passed"], false, "{result}");
            assert!(result.to_string().contains("diameter mismatch"));
        } else {
            let error = response.error.expect("apply refused");
            assert!(error.message.contains("diameter mismatch"), "{error:?}");
        }
        assert_eq!(read_expanded_dir(&path).expect("unchanged"), doc);
    }
}

#[test]
fn legacy_holes_do_not_acquire_standard_metadata_on_migration() {
    let doc = read_expanded_dir(root().join("examples/bracket.ocad.d")).expect("legacy");
    let normalized = opencad_file::migrate::migrate_to_current(doc.clone()).expect("migrate");
    assert_eq!(doc, normalized);
    assert!(doc
        .feature_nodes
        .iter()
        .any(|node| matches!(node.definition, opencad_feature::FeatureDefinition::Hole(_))));
    assert!(!doc.feature_nodes.iter().any(|node| matches!(
        node.definition,
        opencad_feature::FeatureDefinition::StandardHole(_)
    )));
}

#[test]
fn standard_hole_accepts_depth_expression_design_patch() {
    let mut doc = authored();
    let patch = DesignPatch::set_feature_expr(
        "feature:tapped",
        opencad_ai::FeatureExprField::DepthExpr,
        "tap_depth + 2 mm",
    );
    apply_patch_to_document(&mut doc, &patch).expect("expression patch");
    let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            &kernel,
            &opencad_feature::FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            None,
        )
        .expect("regen");
    use opencad_geometry::GeometryKernel;
    let actual = kernel
        .mass_properties(model.active_body().expect("body"), 1.0)
        .expect("mass")
        .volume_m3;
    let expected =
        (57600.0 - std::f64::consts::PI * (3.3_f64.powi(2) * 12.0 + 2.5_f64.powi(2) * 10.0)) * 1e-9;
    assert!((actual - expected).abs() < 1e-13);
}
