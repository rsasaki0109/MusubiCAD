//! ADR-052: native analytic dimensions/volume, identity and verified history.
use opencad_ai::authoring_patch;
use opencad_file::{
    apply_patch_with_history, document_design_state, read_ocad, read_ocad_zip, write_expanded_dir,
    write_ocad_zip, DocumentHistory, OcadDocument,
};
use serde_json::{json, Value};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn rpc(method: &str, params: Value) -> Value {
    let mut p = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .arg("agent")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    writeln!(
        p.stdin.take().expect("stdin"),
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})
    )
    .expect("send");
    let out = p.wait_with_output().expect("wait");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("JSON")
}
fn edit(shape: &str, distance: f64) -> Value {
    let (source, curves, points) = match shape {
        "rectangle" => (
            "sketch:base",
            vec!["ent:e0", "ent:e1", "ent:e2", "ent:e3"],
            4,
        ),
        "circle" => ("sketch:source", vec!["ent:source_circle"], 1),
        _ => (
            "sketch:source",
            vec![
                "ent:source_bottom",
                "ent:source_right",
                "ent:source_top",
                "ent:source_left",
            ],
            6,
        ),
    };
    json!({"source_sketch_id":source,"curve_ids":curves,"distance_m":distance,"association":"independent_copy","join":"mitre_tangent","destination_sketch_id":"sketch:offset","name":"Independent offset","destination_curve_ids":(0..curves.len()).map(|i|format!("ent:offset_curve_{i}")).collect::<Vec<_>>(),"destination_point_ids":(0..points).map(|i|format!("ent:offset_point_{i}")).collect::<Vec<_>>()})
}
fn source(path: &Path, shape: &str) -> OcadDocument {
    let mut doc = read_ocad(root().join("examples/construction_plate.ocad.d")).expect("plate");
    if shape != "rectangle" {
        let mut ops = vec![
            json!({"type":"add_sketch","id":"sketch:source","name":"Offset source","workplane":{"type":"global","plane":"XY"}}),
        ];
        let mut entities = vec![];
        if shape == "circle" {
            entities.push(json!({"type":"point","id":"ent:source_centre","x":0.,"y":0.}));
            entities.push(json!({"type":"circle","id":"ent:source_circle","center":"ent:source_centre","radius":0.02}));
        } else {
            for (id, x, y) in [
                ("ent:source_p0", 0., -0.02),
                ("ent:source_p1", 0.04, -0.02),
                ("ent:source_p2", 0.04, 0.02),
                ("ent:source_p3", 0., 0.02),
                ("ent:source_c0", 0.04, 0.),
                ("ent:source_c1", 0., 0.),
            ] {
                entities.push(json!({"type":"point","id":id,"x":x,"y":y}));
            }
            entities.extend([
                json!({"type":"line","id":"ent:source_bottom","start":"ent:source_p0","end":"ent:source_p1"}),
                json!({"type":"arc","id":"ent:source_right","center":"ent:source_c0","radius":0.02,"start_angle":-std::f64::consts::FRAC_PI_2,"end_angle":std::f64::consts::FRAC_PI_2,"start_point":"ent:source_p1","end_point":"ent:source_p2"}),
                json!({"type":"line","id":"ent:source_top","start":"ent:source_p2","end":"ent:source_p3"}),
                json!({"type":"arc","id":"ent:source_left","center":"ent:source_c1","radius":0.02,"start_angle":std::f64::consts::FRAC_PI_2,"end_angle":3.*std::f64::consts::FRAC_PI_2,"start_point":"ent:source_p3","end_point":"ent:source_p0"})]);
        }
        ops.extend(entities.into_iter().map(|entity|json!({"type":"add_sketch_entity","sketch_id":"sketch:source","entity":entity})));
        let patch = serde_json::from_value(json!({"operations":ops})).expect("patch");
        apply_patch_with_history(&mut doc, &patch, &mut DocumentHistory::default(), "Source")
            .expect("source");
    }
    write_expanded_dir(path, &doc).expect("write");
    doc
}
fn add_consumers(patch: &mut Value) {
    let ops = patch["operations"].as_array_mut().expect("ops");
    ops.push(json!({"type":"add_feature","node":{"id":"feature:offset_sketch","name":"Offset sketch","definition":{"type":"sketch","sketch_id":"sketch:offset"}},"position":opencad_ai::FeaturePosition::end()}));
    ops.push(json!({"type":"add_feature","node":{"id":"feature:offset_body","name":"Offset extrusion","definition":{"type":"extrude","sketch_feature":"feature:offset_sketch","profile_ref":"sketch:offset/profile:outer","extent":{"type":"distance","length":{"value_si":0.006}},"operation":"new_body"}},"position":opencad_ai::FeaturePosition::end()}));
}
// Expanded and packed readers may enumerate nodes differently; graph order
// remains authoritative. Every source value is otherwise preserved exactly.
fn assert_document_roundtrip(a: &OcadDocument, b: &OcadDocument) {
    let mut a = a.clone();
    let mut b = b.clone();
    a.feature_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    b.feature_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(a, b);
}
fn native_volume(path: &Path) -> f64 {
    let out = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args(["regen", path.to_str().expect("path")])
        .output()
        .expect("regen");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("text")
        .lines()
        .find_map(|l| l.strip_prefix("volume_m3:"))
        .expect("volume")
        .trim()
        .parse()
        .expect("number")
}
#[test]
fn native_offsets_match_analytic_rectangle_circle_and_stadium_volumes() {
    for (shape, distance, area) in [
        ("rectangle", 0.003, 0.086 * 0.066),
        ("circle", 0.005, std::f64::consts::PI * 0.025_f64.powi(2)),
        (
            "stadium",
            0.005,
            2. * 0.04 * 0.025 + std::f64::consts::PI * 0.025_f64.powi(2),
        ),
        ("rectangle", -0.01, 0.06 * 0.04),
        (
            "stadium",
            -0.005,
            2. * 0.04 * 0.015 + std::f64::consts::PI * 0.015_f64.powi(2),
        ),
    ] {
        let dir = tempfile::tempdir().expect("temp");
        let path = dir.path().join("part.ocad.d");
        let before = source(&path, shape);
        let planned = rpc(
            "opencad.plan_offset",
            json!({"path":path,"edit":edit(shape,distance)}),
        );
        assert!(planned["error"].is_null(), "{planned}");
        assert_eq!(read_ocad(&path).expect("readonly"), before);
        let mut patch = planned["result"]["patch"].clone();
        add_consumers(&mut patch);
        let dry = rpc(
            "opencad.patch_dry_run_document",
            json!({"path":path,"patch":patch}),
        );
        assert!(dry["error"].is_null(), "{dry}");
        assert_eq!(read_ocad(&path).expect("dry"), before);
        let applied = rpc(
            "opencad.patch_apply_document",
            json!({"path":path,"patch":patch}),
        );
        assert!(applied["error"].is_null(), "{applied}");
        let after = read_ocad(&path).expect("after");
        for original in &before.sketches {
            assert_eq!(
                after
                    .sketches
                    .iter()
                    .find(|s| s.id == original.id)
                    .expect("source"),
                original
            );
        }
        let offset = after
            .sketches
            .iter()
            .find(|s| s.id.as_str() == "sketch:offset")
            .expect("copy");
        assert!(offset.constraints.is_empty());
        let v = native_volume(&path);
        assert!(
            (v - area * 0.006).abs() < 1e-12,
            "{shape}: {v} expected {}",
            area * 0.006
        );
        let packed = dir.path().join("part.ocad");
        write_ocad_zip(&packed, &after).expect("pack");
        assert_document_roundtrip(&read_ocad_zip(&packed).expect("roundtrip"), &after);
        let undo = rpc(
            "opencad.history_undo_document",
            json!({"path":path,"history":applied["result"]["history"]}),
        );
        assert!(undo["error"].is_null(), "{undo}");
        assert_eq!(read_ocad(&path).expect("undo"), before);
        let redo = rpc(
            "opencad.history_redo_document",
            json!({"path":path,"history":undo["result"]["history"]}),
        );
        assert!(redo["error"].is_null(), "{redo}");
        assert_document_roundtrip(&read_ocad(&path).expect("redo"), &after);
    }
}
#[test]
fn invalid_offset_requests_and_stale_plans_preserve_documents() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("part.ocad.d");
    let before = source(&path, "rectangle");
    for field in ["association", "join", "distance_m", "destination_point_ids"] {
        let mut r = edit("rectangle", 0.003);
        r.as_object_mut().expect("object").remove(field);
        let result = rpc("opencad.plan_offset", json!({"path":path,"edit":r}));
        assert!(!result["error"].is_null(), "{field}: {result}");
    }
    for distance in [-0.03, -0.05, 0.] {
        let result = rpc(
            "opencad.plan_offset",
            json!({"path":path,"edit":edit("rectangle",distance)}),
        );
        assert!(!result["error"].is_null(), "{result}");
    }
    let mut duplicate = edit("rectangle", 0.003);
    duplicate["destination_point_ids"][0] = json!("ent:c0");
    assert!(!rpc("opencad.plan_offset", json!({"path":path,"edit":duplicate}))["error"].is_null());
    assert_eq!(read_ocad(&path).expect("unchanged"), before);
    let plan = rpc(
        "opencad.plan_offset",
        json!({"path":path,"edit":edit("rectangle",0.003)}),
    );
    assert!(plan["error"].is_null(), "{plan}");
    let changed = rpc(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":{"operations":[{"type":"set_parameter","id":"param:guide_length","expr":"50 mm"}]}}),
    );
    assert!(changed["error"].is_null(), "{changed}");
    let modified = read_ocad(&path).expect("modified");
    let stale = rpc(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":plan["result"]["patch"]}),
    );
    assert!(!stale["error"].is_null(), "{stale}");
    assert_eq!(read_ocad(&path).expect("unchanged"), modified);
}
#[test]
fn independent_copy_does_not_follow_source_constraint_edits() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("part.ocad.d");
    source(&path, "rectangle");
    // Use the source's construction circle; its source constraint radius changes
    // after copying, while the authored destination radius remains literal.
    let mut r = edit("circle", 0.005);
    r["source_sketch_id"] = json!("sketch:base");
    r["curve_ids"] = json!(["ent:guide_circle"]);
    let plan = rpc("opencad.plan_offset", json!({"path":path,"edit":r}));
    assert!(plan["error"].is_null(), "{plan}");
    let applied = rpc(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":plan["result"]["patch"]}),
    );
    assert!(applied["error"].is_null(), "{applied}");
    let before = read_ocad(&path).expect("copy");
    let changed = rpc(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":{"operations":[{"type":"add_sketch_constraint","sketch_id":"sketch:base","constraint":{"type":"radius","id":"con:new_radius","target":"ent:guide_circle","expr":"20 mm"}}]}}),
    );
    assert!(changed["error"].is_null(), "{changed}");
    let after = read_ocad(&path).expect("after");
    assert_eq!(
        before
            .sketches
            .iter()
            .find(|s| s.id.as_str() == "sketch:offset"),
        after
            .sketches
            .iter()
            .find(|s| s.id.as_str() == "sketch:offset")
    );
}
#[test]
#[ignore = "generate verified independent offset example"]
fn generate_offset_example() {
    let path = root().join("examples/offset_plate.ocad.d");
    source(&path, "rectangle");
    let plan = rpc(
        "opencad.plan_offset",
        json!({"path":path,"edit":edit("rectangle",0.003)}),
    );
    assert!(plan["error"].is_null(), "{plan}");
    let mut patch = plan["result"]["patch"].clone();
    add_consumers(&mut patch);
    let applied = rpc(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":patch}),
    );
    assert!(applied["error"].is_null(), "{applied}");
    let doc = read_ocad(&path).expect("doc");
    let authored = authoring_patch(&document_design_state(&doc)).expect("authoring");
    std::fs::write(
        root().join("examples/agent/create_offset_plate_patch.json"),
        serde_json::to_string_pretty(&authored).expect("JSON") + "\n",
    )
    .expect("write");
}
