//! MCAD-P10-003 integration tests: OCCT geometry, CLI and real MCP process.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    for entry in std::fs::read_dir(root).expect("read") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            files.extend(snapshot(&path));
        } else {
            files.insert(path.clone(), std::fs::read(path).expect("bytes"));
        }
    }
    files
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args(args)
        .output()
        .expect("CLI")
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8")
}

fn fixture(dir: &Path) -> PathBuf {
    let path = dir.join("drawing.ocad.d");
    success(cli(&["new", path.to_str().expect("path"), "drawing"]));
    let patch = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/agent/semantic_bracket_dimension_patch.json");
    success(cli(&[
        "patch",
        path.to_str().expect("path"),
        patch.to_str().expect("patch"),
    ]));
    path
}

fn write_patch(dir: &Path, operations: Value) -> PathBuf {
    let path = dir.join("patch.json");
    std::fs::write(&path, json!({"operations": operations}).to_string()).expect("patch");
    path
}

fn mcp(path: &Path, patch: &Value) -> Vec<Value> {
    let mut process = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("MCP");
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"semantic-dimension-test","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"regen_document","arguments":{"path":path}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"patch_dry_run","arguments":{"path":path,"patch":patch}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"patch_apply","arguments":{"path":path,"patch":patch}}}),
    ];
    {
        let mut stdin = process.stdin.take().expect("stdin");
        for request in requests {
            writeln!(stdin, "{request}").expect("write");
        }
    }
    let output = process.wait_with_output().expect("wait");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("JSON"))
        .collect()
}

#[test]
fn semantic_width_tracks_an_occt_parameter_edit_and_exports_deterministically() {
    let dir = tempfile::tempdir().expect("dir");
    let path = fixture(dir.path());
    let before = snapshot(&path);
    let patch = json!({"operations":[{"type":"set_drawing_view_scale","view_id":"view:front","scale":0.5}]});
    let responses = mcp(&path, &patch);
    let regen =
        &responses.iter().find(|r| r["id"] == 2).expect("regen")["result"]["structuredContent"];
    assert!(
        (regen["drawing_dimensions"][0]["measured_length_m"]
            .as_f64()
            .expect("length")
            - 0.08)
            .abs()
            < 1e-9
    );
    assert_eq!(
        regen["drawing_dimensions"][0]["reference_provenance"]["status"],
        "fingerprint"
    );
    assert_eq!(
        responses.iter().find(|r| r["id"] == 3).expect("dry-run")["result"]["structuredContent"]
            ["verification"]["passed"],
        true
    );
    assert_eq!(
        responses.iter().find(|r| r["id"] == 4).expect("apply")["result"]["isError"],
        false
    );
    assert_ne!(snapshot(&path), before);
    let child = path.join("parts/bracket.ocad.d");
    let patch = write_patch(
        dir.path(),
        json!([{"type":"set_parameter","id":"param:width","expr":"100 mm"}]),
    );
    success(cli(&[
        "patch",
        child.to_str().expect("child"),
        patch.to_str().expect("patch"),
    ]));
    let before_regen = snapshot(&path);
    let regen = success(cli(&["regen", path.to_str().expect("path")]));
    let length: f64 = regen
        .lines()
        .find_map(|line| line.strip_prefix("dimension: dim:semantic_width measured_length_m: "))
        .expect("length")
        .parse()
        .expect("number");
    assert!((length - 0.1).abs() < 1e-9, "{regen}");
    let svg = dir.path().join("drawing.svg");
    let export = || {
        success(cli(&[
            "export",
            path.to_str().expect("path"),
            svg.to_str().expect("svg"),
        ]))
    };
    export();
    let first = std::fs::read(&svg).expect("SVG");
    assert!(String::from_utf8_lossy(&first).contains(">100.00 mm</text>"));
    export();
    assert_eq!(std::fs::read(svg).expect("SVG"), first);
    assert_eq!(snapshot(&path), before_regen);
}

#[test]
fn later_sheet_missing_dimension_is_refused_by_mcp_and_cli_without_writing() {
    let dir = tempfile::tempdir().expect("dir");
    let path = fixture(dir.path());
    let operations = json!([
        {"type":"add_sheet","sheet":{"id":"sheet:later","name":"Later","width_m":0.21,"height_m":0.297,"views":[]}},
        {"type":"add_drawing_view","sheet_id":"sheet:later","view":{"id":"view:later","name":"Later","model":{"source_path":"parts/bracket.ocad.d","source_doc":"doc:bracket_001"},"projection":"front","scale":1.0,"origin_on_sheet_m":[0.02,0.02]}},
        {"type":"add_drawing_dimension","sheet_id":"sheet:later","dimension":{"id":"dim:missing","view_id":"view:later","edge_ref":"ref:edge:missing","offset_m":0.01}}
    ]);
    let before = snapshot(&path);
    let patch = json!({"operations":operations});
    let responses = mcp(&path, &patch);
    let dry_run =
        &responses.iter().find(|r| r["id"] == 3).expect("dry-run")["result"]["structuredContent"];
    assert_eq!(dry_run["verification"]["passed"], false);
    assert!(dry_run["verification"]["error"]
        .as_str()
        .expect("error")
        .contains("dim:missing"));
    assert_eq!(
        responses.iter().find(|r| r["id"] == 4).expect("apply")["result"]["isError"],
        true
    );
    assert_eq!(snapshot(&path), before);
    let patch = write_patch(dir.path(), operations);
    let failed = cli(&[
        "patch",
        path.to_str().expect("path"),
        patch.to_str().expect("patch"),
    ]);
    assert!(!failed.status.success());
    assert_eq!(snapshot(&path), before);
    let review = dir.path().join("review");
    assert!(!cli(&[
        "review",
        path.to_str().expect("path"),
        patch.to_str().expect("patch"),
        "--output",
        review.to_str().expect("review"),
    ])
    .status
    .success());
    assert!(!review.exists());
    assert_eq!(snapshot(&path), before);
    // Explicit staged authoring allows storing an unresolved binding; every
    // subsequent regeneration/export still refuses it before writing output.
    success(cli(&[
        "patch",
        path.to_str().expect("path"),
        patch.to_str().expect("patch"),
        "--no-verify",
    ]));
    let staged = snapshot(&path);
    assert!(!cli(&["regen", path.to_str().expect("path")])
        .status
        .success());
    let svg = dir.path().join("sentinel.svg");
    std::fs::write(&svg, b"preserve existing output").expect("sentinel");
    assert!(!cli(&[
        "export",
        path.to_str().expect("path"),
        svg.to_str().expect("svg")
    ])
    .status
    .success());
    assert_eq!(
        std::fs::read(svg).expect("sentinel"),
        b"preserve existing output"
    );
    assert_eq!(snapshot(&path), staged);
}

#[test]
fn an_occt_closed_edge_cannot_become_a_linear_dimension() {
    use opencad_core::{DimensionId, TopoRefId, ViewId};
    use opencad_drawing::{resolve_linear_dimension, LinearDimension, LinearMeasurement};
    use opencad_feature::{edge_discover::discover_edge_refs_from_body, FeatureRegistry};
    use opencad_geometry::{GeometryKernel, TopoRef};
    use opencad_kernel_occt::OcctGeometryKernel;

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bracket.ocad.d");
    let doc = opencad_file::read_ocad(path).expect("part");
    let kernel = OcctGeometryKernel::new();
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            Some(&doc.semantic_refs),
        )
        .expect("regenerate");
    let body = model.active_body().expect("body");
    let nodes = model.nodes.values().cloned().collect::<Vec<_>>();
    let discoveries = discover_edge_refs_from_body(&kernel, body, &nodes).expect("edges");
    let closed = discoveries
        .iter()
        .find(|edge| {
            let points = kernel
                .edge_endpoints(body, edge.kernel_edge_id)
                .expect("endpoints");
            points[0]
                .iter()
                .zip(points[1])
                .all(|(a, b)| (a - b).abs() < 1e-9)
        })
        .expect("hole has a closed circular edge");
    let reference = TopoRef::kernel_edge(
        TopoRefId::new("ref:edge:hole_ring").expect("ref"),
        closed.feature_id.as_deref().expect("producer"),
        &closed.role,
        closed.kernel_edge_id,
        closed.midpoint_m,
        closed.tangent_m,
    );
    let dimension = LinearDimension {
        id: DimensionId::new("dim:hole_ring").expect("id"),
        view_id: ViewId::new("view:front").expect("id"),
        measurement: LinearMeasurement::Edge {
            edge_ref: reference.ref_id.clone(),
        },
        offset_m: 0.01,
    };
    let error = resolve_linear_dimension(&dimension, &[reference], &discoveries, &kernel, body)
        .expect_err("closed chord")
        .to_string();
    assert!(
        error.contains("dim:hole_ring") && error.contains("measured length"),
        "{error}"
    );
    assert!(kernel.edge_endpoints(body, 0).is_err());
    assert!(kernel.edge_endpoints(body, u64::MAX).is_err());
}

#[test]
fn ambiguous_reference_and_wrong_model_identity_refuse_regeneration() {
    let dir = tempfile::tempdir().expect("dir");
    let path = fixture(dir.path());
    let child = path.join("parts/bracket.ocad.d");
    let add_ref = write_patch(
        dir.path(),
        json!([{"type":"add_semantic_ref","topo_ref":{"ref_id":"ref:edge:ambiguous","kind":"edge","semantic":{"created_by":"feature:hole_mount"}}}]),
    );
    success(cli(&[
        "patch",
        child.to_str().expect("child"),
        add_ref.to_str().expect("patch"),
    ]));
    let bind = write_patch(
        dir.path(),
        json!([
            {"type":"add_drawing_dimension","sheet_id":"sheet:a4","dimension":{"id":"dim:ambiguous","view_id":"view:front","edge_ref":"ref:edge:ambiguous","offset_m":0.01}}
        ]),
    );
    let before = snapshot(&path);
    let result = cli(&[
        "patch",
        path.to_str().expect("path"),
        bind.to_str().expect("patch"),
    ]);
    assert!(!result.status.success());
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(
        error.contains("Ambiguous") && error.contains("dim:ambiguous"),
        "{error}"
    );
    assert_eq!(snapshot(&path), before);
    let wrong = write_patch(
        dir.path(),
        json!([
            {"type":"add_sheet","sheet":{"id":"sheet:wrong","name":"Wrong","width_m":0.21,"height_m":0.297,"views":[]}},
            {"type":"add_drawing_view","sheet_id":"sheet:wrong","view":{"id":"view:wrong","name":"Wrong","model":{"source_path":"parts/bracket.ocad.d","source_doc":"doc:wrong"},"projection":"front","scale":1.0,"origin_on_sheet_m":[0.02,0.02]}}
        ]),
    );
    let result = cli(&[
        "patch",
        path.to_str().expect("path"),
        wrong.to_str().expect("patch"),
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("doc:wrong"));
    assert_eq!(snapshot(&path), before);
}
