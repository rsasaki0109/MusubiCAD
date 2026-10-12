//! MCAD-P11-006: two-body graph, analytic geometry and transactional patch evidence.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::{FeatureRegistry, RegenerationCache};
use opencad_file::{
    apply_patch_to_document, document_design_state, read_expanded_dir, write_expanded_dir,
    OcadDocument,
};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
use std::path::PathBuf;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn authored() -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:body_boolean").expect("id"),
        "Existing body union",
    ));
    let patch: DesignPatch = serde_json::from_str(
        &std::fs::read_to_string(root().join("examples/agent/create_body_boolean_patch.json"))
            .expect("patch"),
    )
    .expect("json");
    apply_patch_to_document(&mut doc, &patch).expect("apply");
    doc
}
fn patch(value: serde_json::Value) -> DesignPatch {
    serde_json::from_value(value).expect("patch")
}
#[test]
#[ignore = "refresh committed example through DesignPatch"]
fn write_body_boolean_example() {
    write_expanded_dir(root().join("examples/body_boolean.ocad.d"), &authored()).expect("write");
}
#[test]
fn body_boolean_example_round_trip_and_authoring_rebuild() {
    let doc = authored();
    let actual = read_expanded_dir(root().join("examples/body_boolean.ocad.d")).expect("read");
    assert_eq!(document_design_state(&doc), document_design_state(&actual));
    let p = authoring_patch(&document_design_state(&actual)).expect("rebuild patch");
    let mut rebuilt = OcadDocument::new(actual.metadata.clone());
    apply_patch_to_document(&mut rebuilt, &p).expect("rebuild");
    assert_eq!(
        document_design_state(&actual),
        document_design_state(&rebuilt)
    );
    let dir = tempfile::tempdir().expect("temp");
    write_expanded_dir(dir.path(), &rebuilt).expect("write");
    assert_eq!(rebuilt, read_expanded_dir(dir.path()).expect("round trip"));
    let inputs = actual
        .feature_nodes
        .iter()
        .find(|n| n.id == "feature:body_boolean")
        .expect("node")
        .definition
        .feature_inputs();
    assert_eq!(
        inputs,
        vec![
            ("source_feature", "feature:tool"),
            ("target_feature", "feature:plate")
        ]
    );
}
fn contract<K: GeometryKernel>(kernel: &K, analytic: bool) {
    for (operation, volume, bounds) in [
        ("union", 30000.0, [0.080, 0.060, 0.008]),
        ("subtract", 25200.0, [0.080, 0.060, 0.006]),
        ("intersect", 3600.0, [0.050, 0.030, 0.006]),
    ] {
        let mut doc = authored();
        let p = patch(
            serde_json::json!({"operations":[{"type":"replace_feature_definition","id":"feature:body_boolean","definition":{"type":"body_boolean","source_feature":"feature:tool","target_feature":"feature:plate","operation":operation}}]}),
        );
        apply_patch_to_document(&mut doc, &p).expect("operation patch");
        let mut model = doc.clone().into_part_model();
        let mut cache = RegenerationCache::new();
        let registry = FeatureRegistry::with_defaults();
        model
            .regenerate_with_cache(
                kernel,
                &registry,
                Some(&doc.parameters),
                Some(&doc.semantic_refs),
                &mut cache,
            )
            .expect("regen");
        if analytic {
            let body = model.active_body().expect("body");
            let actual = kernel.mass_properties(body, 1.0).expect("mass").volume_m3;
            assert!(
                (actual - volume * 1e-9).abs() < 1e-11,
                "{operation}: {actual}"
            );
            let actual = kernel.bounding_box(body).expect("bounds");
            for (a, b) in actual.max.iter().zip(bounds) {
                assert!((a - b).abs() < 1e-6, "{operation}: {actual:?}");
            }
            if operation == "intersect" {
                for (a, b) in actual.min.iter().zip([0.020, 0.010, 0.]) {
                    assert!((a - b).abs() < 1e-6);
                }
            }
        }
        // Expanded rectangle diagnostics change on the first solve (DOF 0 -> 6).
        // Prime that existing sketch state before asserting unchanged-model reuse.
        model
            .regenerate_with_cache(
                kernel,
                &registry,
                Some(&doc.parameters),
                Some(&doc.semantic_refs),
                &mut cache,
            )
            .expect("prime sketch diagnostics");
        let warm = model
            .regenerate_with_cache(
                kernel,
                &registry,
                Some(&doc.parameters),
                Some(&doc.semantic_refs),
                &mut cache,
            )
            .expect("warm");
        assert!(warm.cached_nodes.contains(&"feature:body_boolean".into()));
        assert_eq!(warm.trace.geometry_kernel_call_count, 0);
        doc.parameters
            .set_expr("param:source_height", "10 mm")
            .expect("edit");
        let edited = model
            .regenerate_with_cache(
                kernel,
                &registry,
                Some(&doc.parameters),
                Some(&doc.semantic_refs),
                &mut cache,
            )
            .expect("edit regen");
        assert!(!edited.cached_nodes.contains(&"feature:body_boolean".into()));
        assert!(edited.cached_nodes.contains(&"feature:plate".into()));
        if analytic {
            let actual = kernel
                .mass_properties(model.active_body().expect("body"), 1.)
                .expect("mass")
                .volume_m3;
            let expected = if operation == "union" { 31200. } else { volume };
            assert!((actual - expected * 1e-9).abs() < 1e-11);
        }
        doc.parameters
            .set_expr("param:target_height", "10 mm")
            .expect("target edit");
        let target_edit = model
            .regenerate_with_cache(
                kernel,
                &registry,
                Some(&doc.parameters),
                Some(&doc.semantic_refs),
                &mut cache,
            )
            .expect("target edit regen");
        assert!(!target_edit
            .cached_nodes
            .contains(&"feature:body_boolean".into()));
        assert!(target_edit.cached_nodes.contains(&"feature:tool".into()));
        if analytic {
            let actual = kernel
                .mass_properties(model.active_body().expect("body"), 1.0)
                .expect("mass")
                .volume_m3;
            let expected = match operation {
                "union" => 48000.0,
                "subtract" => 42000.0,
                _ => 6000.0,
            };
            assert!((actual - expected * 1e-9).abs() < 1e-11);
        }
        let before = model.outputs.clone();
        model
            .nodes
            .get_mut("feature:tool")
            .expect("tool")
            .suppressed = true;
        assert!(model
            .regenerate(
                kernel,
                &registry,
                Some(&doc.parameters),
                Some(&doc.semantic_refs)
            )
            .is_err());
        assert_eq!(before, model.outputs);
    }
}
#[test]
fn body_boolean_mock_contract() {
    contract(&MockGeometryKernel::new(), false);
}
#[test]
fn body_boolean_occt_analytic_boxes_and_cache() {
    contract(&OcctGeometryKernel::new(), true);
}
#[test]
fn body_boolean_invalid_inputs_and_cycles_are_atomic() {
    for source in [
        "feature:missing",
        "feature:plate",
        "feature:sketch_plate",
        "feature:body_boolean",
    ] {
        let mut doc = authored();
        let before = doc.clone();
        let p = patch(
            serde_json::json!({"operations":[{"type":"replace_feature_definition","id":"feature:body_boolean","definition":{"type":"body_boolean","source_feature":source,"target_feature":"feature:plate","operation":"union"}}]}),
        );
        assert!(apply_patch_to_document(&mut doc, &p).is_err(), "{source}");
        assert_eq!(before, doc);
    }
}
#[test]
fn body_boolean_occt_refuses_empty_outputs_and_preserves_inputs() {
    use opencad_core::Length;
    use opencad_geometry::{BooleanOp, ExtrudeExtent, ExtrudeOperation, SolvedSketch};
    let kernel = OcctGeometryKernel::new();
    let wire = kernel
        .make_wire_from_sketch(&SolvedSketch {
            profile_ref: "box".into(),
            points: vec![[0., 0.], [0.02, 0.], [0.02, 0.02], [0., 0.02]],
            closed: true,
            circle: None,
            segments: vec![],
            placement: None,
        })
        .expect("wire");
    let body = kernel
        .extrude(
            wire,
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.01),
            },
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.],
        )
        .expect("body");
    let before = kernel.mass_properties(&body, 1.).expect("mass");
    assert!(kernel
        .body_boolean(body.clone(), body.clone(), BooleanOp::Subtract)
        .is_err());
    let far = kernel
        .transform_body(
            body.clone(),
            opencad_geometry::RigidTransform::from_translation([1., 0., 0.]),
        )
        .expect("far");
    assert!(kernel
        .body_boolean(body.clone(), far, BooleanOp::Intersect)
        .is_err());
    assert_eq!(
        before,
        kernel.mass_properties(&body, 1.).expect("source preserved")
    );
}

#[test]
fn body_boolean_verified_agent_refuses_empty_result_without_persistence() {
    use opencad_ai::{JsonRpcRequest, JsonRpcResponse};
    use std::io::Write;
    use std::process::{Command, Stdio};
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("boolean.ocad.d");
    let doc = authored();
    write_expanded_dir(&path, &doc).expect("write");
    for method in [
        "opencad.patch_dry_run_document",
        "opencad.patch_apply_document",
    ] {
        let request:JsonRpcRequest=serde_json::from_value(serde_json::json!({"jsonrpc":"2.0","id":1,"method":method,"params":{"path":path,"patch":{"operations":[
            {"type":"set_parameter","id":"param:source_height","expr":"4 mm"},
            {"type":"replace_feature_definition","id":"feature:body_boolean","definition":{"type":"body_boolean","source_feature":"feature:plate","target_feature":"feature:tool","operation":"subtract"}}
        ]}}})).expect("request");
        let mut process = Command::new(env!("CARGO_BIN_EXE_musubicad"))
            .arg("agent")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("agent");
        {
            let mut stdin = process.stdin.take().expect("stdin");
            writeln!(stdin, "{}", serde_json::to_string(&request).expect("json")).expect("write");
        }
        let output = process.wait_with_output().expect("wait");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let response: JsonRpcResponse = serde_json::from_slice(&output.stdout).expect("response");
        if method == "opencad.patch_dry_run_document" {
            assert_eq!(
                response.result.expect("dry run")["verification"]["passed"],
                false
            );
        } else {
            assert!(response
                .error
                .expect("apply refused")
                .message
                .contains("body_boolean"));
        }
        assert_eq!(read_expanded_dir(&path).expect("unchanged"), doc);
    }
}
