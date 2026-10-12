//! MCAD-P13-002: real verified trim/extend outcomes and source identity.
use opencad_ai::{authoring_patch, DesignPatch};
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
fn request(method: &str, params: Value) -> Value {
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
fn source(path: &Path, x: f64) -> OcadDocument {
    let mut doc = read_ocad(root().join("examples/construction_plate.ocad.d")).expect("plate");
    let patch:DesignPatch=serde_json::from_value(json!({"operations":[
        {"type":"add_sketch_entity","sketch_id":"sketch:base","entity":{"type":"point","id":"ent:cut_a","construction":true,"x":x,"y":0.}},
        {"type":"add_sketch_entity","sketch_id":"sketch:base","entity":{"type":"point","id":"ent:cut_b","construction":true,"x":x,"y":0.06}},
        {"type":"add_sketch_entity","sketch_id":"sketch:base","entity":{"type":"line","id":"ent:cutter","construction":true,"start":"ent:cut_a","end":"ent:cut_b"}}
    ]})).expect("patch");
    apply_patch_with_history(&mut doc, &patch, &mut DocumentHistory::default(), "Cutter")
        .expect("cutter");
    write_expanded_dir(path, &doc).expect("source");
    doc
}
fn plan(path: &Path, mode: &str, remove: bool) -> Value {
    request(
        "opencad.plan_curve_edit",
        json!({"path":path,"edit":{"sketch_id":"sketch:base","target_id":"ent:guide_line","cutter_id":"ent:cutter","mode":mode,"end":"end","new_point_id":"ent:edited_end","remove_constraints":if remove {vec!["con:guide_length"]} else {vec![]}}}),
    )
}
#[test]
fn solver_displaced_trim_endpoint_refuses_verified_application() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("plate.ocad.d");
    let before = source(&path, 0.04);
    let planned = plan(&path, "trim", false);
    assert!(planned["error"].is_null(), "{planned}");
    for verify in [true, false] {
        let response = request(
            "opencad.patch_apply_document",
            json!({"path":path,"patch":planned["result"]["patch"],"verify":verify}),
        );
        assert!(!response["error"].is_null(), "{response}");
        assert_eq!(read_ocad(&path).expect("unchanged"), before);
    }
}
#[test]
fn trim_preserves_shared_endpoints_and_explicit_constraints() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("plate.ocad.d");
    let before = source(&path, 0.04);
    let planned = plan(&path, "trim", true);
    assert!(planned["error"].is_null(), "{planned}");
    assert_eq!(read_ocad(&path).expect("readonly"), before);
    let applied = request(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":planned["result"]["patch"]}),
    );
    assert!(applied["error"].is_null(), "{applied}");
    let after = read_ocad(&path).expect("after");
    let old = &before.sketches[0];
    let new = &after.sketches[0];
    assert_eq!(
        old.find_entity("ent:guide_a"),
        new.find_entity("ent:guide_a")
    );
    assert_eq!(
        old.find_entity("ent:guide_b"),
        new.find_entity("ent:guide_b")
    );
    let entity =
        serde_json::to_value(new.find_entity("ent:guide_line").expect("line")).expect("json");
    assert_eq!(entity["end"], "ent:edited_end");
    let endpoint =
        serde_json::to_value(new.find_entity("ent:edited_end").expect("point")).expect("json");
    assert!((endpoint["x"].as_f64().expect("x") - 0.04).abs() < 1e-12);
    assert!(new
        .constraints
        .iter()
        .any(|c| c.id().as_str() == "con:guide_horizontal"));
    assert!(!new
        .constraints
        .iter()
        .any(|c| c.id().as_str() == "con:guide_length"));
}
#[test]
fn trim_extend_round_trip_history_and_native_geometry() {
    for (mode, x) in [("trim", 0.04), ("extend", 0.07)] {
        let dir = tempfile::tempdir().expect("temp");
        let path = dir.path().join("plate.ocad.d");
        let before = source(&path, x);
        let planned = plan(&path, mode, true);
        assert!(planned["error"].is_null(), "{planned}");
        let applied = request(
            "opencad.patch_apply_document",
            json!({"path":path,"patch":planned["result"]["patch"]}),
        );
        assert!(applied["error"].is_null(), "{applied}");
        let after = read_ocad(&path).expect("after");
        let packed = dir.path().join("plate.ocad");
        write_ocad_zip(&packed, &after).expect("pack");
        assert_eq!(read_ocad_zip(&packed).expect("roundtrip"), after);
        let undo = request(
            "opencad.history_undo_document",
            json!({"path":path,"history":applied["result"]["history"]}),
        );
        assert!(undo["error"].is_null(), "{undo}");
        assert_eq!(read_ocad(&path).expect("undo"), before);
        let redo = request(
            "opencad.history_redo_document",
            json!({"path":path,"history":undo["result"]["history"]}),
        );
        assert!(redo["error"].is_null(), "{redo}");
        assert_eq!(read_ocad(&path).expect("redo"), after);
        let out = Command::new(env!("CARGO_BIN_EXE_musubicad"))
            .args(["regen", path.to_str().expect("path")])
            .output()
            .expect("regen");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8(out.stdout).expect("text");
        let volume: f64 = text
            .lines()
            .find_map(|l| l.strip_prefix("volume_m3:"))
            .expect("volume")
            .trim()
            .parse()
            .expect("number");
        assert!((volume - 2.88e-5).abs() < 1e-12);
    }
}
#[test]
#[ignore = "generate verified trim and extend example"]
fn generate_curve_edit_example() {
    let path = root().join("examples/trim_extend_plate.ocad.d");
    source(&path, 0.04);
    let planned = plan(&path, "trim", true);
    assert!(planned["error"].is_null(), "{planned}");
    let result = request(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":planned["result"]["patch"]}),
    );
    assert!(result["error"].is_null(), "{result}");
    let doc = read_ocad(&path).expect("doc");
    let patch = authoring_patch(&document_design_state(&doc)).expect("authoring");
    std::fs::write(
        root().join("examples/agent/create_trim_extend_plate_patch.json"),
        serde_json::to_string_pretty(&patch).expect("json") + "\n",
    )
    .expect("patch");
}

#[test]
fn trimmed_arc_sector_matches_analytic_native_volume() {
    use opencad_core::{DocumentId, DocumentMetadata};
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("sector.ocad.d");
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:trim_sector").expect("id"),
        "Trim sector",
    ));
    let mut operations = vec![
        json!({"type":"add_sketch","id":"sketch:sector","name":"Sector","workplane":{"type":"global","plane":"XY"}}),
    ];
    for (id, x, y) in [
        ("ent:centre", 0., 0.),
        ("ent:start", 0.05, 0.),
        ("ent:old_end", 0., 0.05),
        ("ent:cut_a", 0.03, 0.),
        ("ent:cut_b", 0.03, 0.1),
    ] {
        operations.push(json!({"type":"add_sketch_entity","sketch_id":"sketch:sector","entity":{"type":"point","id":id,"x":x,"y":y}}));
    }
    for entity in [
        json!({"type":"arc","id":"ent:arc","center":"ent:centre","radius":0.05,"start_angle":0.,"end_angle":std::f64::consts::FRAC_PI_2,"start_point":"ent:start","end_point":"ent:old_end"}),
        json!({"type":"line","id":"ent:radial_end","start":"ent:old_end","end":"ent:centre"}),
        json!({"type":"line","id":"ent:radial_start","start":"ent:centre","end":"ent:start"}),
        json!({"type":"line","id":"ent:cutter","construction":true,"start":"ent:cut_a","end":"ent:cut_b"}),
    ] {
        operations
            .push(json!({"type":"add_sketch_entity","sketch_id":"sketch:sector","entity":entity}));
    }
    operations.push(json!({"type":"add_sketch_constraint","sketch_id":"sketch:sector","constraint":{"type":"radius","id":"con:radius","target":"ent:arc","expr":"50 mm"}}));
    operations.push(json!({"type":"add_feature","node":{"id":"feature:sketch_sector","name":"Sector sketch","definition":{"type":"sketch","sketch_id":"sketch:sector"}},"position":opencad_ai::FeaturePosition::end()}));
    operations.push(json!({"type":"add_feature","node":{"id":"feature:sector","name":"Sector extrusion","definition":{"type":"extrude","sketch_feature":"feature:sketch_sector","profile_ref":"sketch:sector/profile:outer","extent":{"type":"distance","length":{"value_si":0.006}},"operation":"new_body"}},"position":opencad_ai::FeaturePosition::end()}));
    let patch: DesignPatch =
        serde_json::from_value(json!({"operations":operations})).expect("patch");
    apply_patch_with_history(&mut doc, &patch, &mut DocumentHistory::default(), "Sector")
        .expect("build");
    write_expanded_dir(&path, &doc).expect("write");
    let planned = request(
        "opencad.plan_curve_edit",
        json!({"path":path,"edit":{"sketch_id":"sketch:sector","target_id":"ent:arc","cutter_id":"ent:cutter","mode":"trim","end":"end","new_point_id":"ent:new_end"}}),
    );
    assert!(planned["error"].is_null(), "{planned}");
    let mut patch = planned["result"]["patch"].clone();
    patch["operations"].as_array_mut().expect("ops").push(json!({"type":"replace_sketch_entity","sketch_id":"sketch:sector","entity":{"type":"line","id":"ent:radial_end","start":"ent:new_end","end":"ent:centre"}}));
    let applied = request(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":patch}),
    );
    assert!(applied["error"].is_null(), "{applied}");
    let output = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args(["regen", path.to_str().expect("path")])
        .output()
        .expect("regen");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("text");
    let volume: f64 = text
        .lines()
        .find_map(|l| l.strip_prefix("volume_m3:"))
        .expect("volume")
        .trim()
        .parse()
        .expect("number");
    let expected = 0.5 * 0.05 * 0.05 * 4_f64.atan2(3.) * 0.006;
    assert!((volume - expected).abs() < 1e-12, "{volume} vs {expected}");
    let restored = read_ocad(&path).expect("source");
    assert_eq!(
        doc.sketches[0].find_entity("ent:old_end"),
        restored.sketches[0].find_entity("ent:old_end")
    );
}
