//! MCAD-P12-010 native import, file round trips, history and refusal isolation.
use opencad_ai::authoring_patch;
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_file::{
    document_design_state, read_ocad, read_ocad_zip, write_expanded_dir, write_ocad_zip,
    OcadDocument,
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
fn empty(path: &Path) -> OcadDocument {
    let doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:stl_plate").expect("id"),
        "STL plate",
    ));
    write_expanded_dir(path, &doc).expect("write");
    doc
}
fn request(method: &str, params: Value, mcp: bool) -> Value {
    let message = if mcp {
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":method,"arguments":params}})
    } else {
        json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})
    };
    let mut p = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .arg(if mcp { "mcp" } else { "agent" })
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    writeln!(p.stdin.take().expect("stdin"), "{message}").expect("send");
    let output = p.wait_with_output().expect("wait");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("response")
}
fn params(path: &Path) -> Value {
    json!({"path":path,"stl_path":root().join("examples/import/closed_plate_mm.stl"),"feature_id":"feature:stl_plate","source_unit":"mm"})
}

#[test]
fn native_import_dry_run_round_trips_undo_redo_and_source_preservation() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("plate.ocad.d");
    let before = empty(&path);
    let source = root().join("examples/import/closed_plate_mm.stl");
    let bytes = std::fs::read(&source).expect("source");
    let mut dry = params(&path);
    dry["dry_run"] = json!(true);
    let response = request("import_stl", dry, true);
    assert!(response["error"].is_null(), "{response}");
    assert_ne!(response["result"]["isError"], true, "{response}");
    let dry_summary: Value = serde_json::from_str(
        response["result"]["content"][0]["text"]
            .as_str()
            .expect("summary"),
    )
    .expect("JSON");
    assert_eq!(dry_summary["verification"]["passed"], true);
    assert_eq!(read_ocad(&path).expect("dry"), before);
    let response = request("opencad.import_stl", params(&path), false);
    assert!(response["error"].is_null(), "{response}");
    assert_eq!(response["result"]["triangle_count"], 12);
    let imported = read_ocad(&path).expect("imported");
    assert_eq!(imported.attachments.len(), 1);
    assert_eq!(
        imported.attachments.values().next().expect("attachment"),
        &bytes
    );
    let packed = dir.path().join("plate.ocad");
    write_ocad_zip(&packed, &imported).expect("pack");
    assert_eq!(read_ocad_zip(&packed).expect("unpack"), imported);
    let undone = request(
        "opencad.history_undo_document",
        json!({"path":path,"history":response["result"]["history"]["history"]}),
        false,
    );
    assert!(undone["error"].is_null(), "{undone}");
    assert_eq!(read_ocad(&path).expect("undo"), before);
    let redone = request(
        "opencad.history_redo_document",
        json!({"path":path,"history":undone["result"]["history"]}),
        false,
    );
    assert!(redone["error"].is_null(), "{redone}");
    assert_eq!(read_ocad(&path).expect("redo"), imported);
    assert_eq!(std::fs::read(source).expect("source"), bytes);
    let bad_remove = request(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":{"operations":[{"type":"remove_attachment","path":response["result"]["attachment"]}]}}),
        false,
    );
    assert!(!bad_remove["error"].is_null(), "{bad_remove}");
    assert_eq!(read_ocad(&path).expect("unchanged"), imported);
    // A changed opaque asset must invalidate undo, and an expanded write must
    // refuse to delete it before modifying any source model files.
    let asset = path.join(
        response["result"]["attachment"]
            .as_str()
            .expect("attachment"),
    );
    let mut modified = bytes.clone();
    modified[0] ^= 0xff;
    std::fs::write(&asset, &modified).expect("external change");
    let changed = read_ocad(&path).expect("changed");
    let stale = request(
        "opencad.history_undo_document",
        json!({"path":path,"history":redone["result"]["history"]}),
        false,
    );
    assert!(!stale["error"].is_null(), "{stale}");
    assert!(write_expanded_dir(&path, &before).is_err());
    assert_eq!(read_ocad(&path).expect("preserved"), changed);
    assert_eq!(std::fs::read(asset).expect("asset"), modified);
}

#[test]
fn refuses_missing_units_open_shells_improper_placement_and_duplicate_ids_atomically() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("plate.ocad.d");
    let before = empty(&path);
    let mut missing = params(&path);
    missing
        .as_object_mut()
        .expect("object")
        .remove("source_unit");
    let response = request("import_stl", missing, true);
    assert_eq!(response["error"]["code"], -32602, "{response}");
    let source = dir.path().join("open.stl");
    std::fs::write(&source, b"solid open\nendsolid open\n").expect("source");
    let mut open = params(&path);
    open["stl_path"] = json!(source);
    assert_eq!(request("import_stl", open, true)["result"]["isError"], true);
    let mut improper = params(&path);
    improper["transform"] = json!({"translation_m":[0,0,0],"rotation":[[-1,0,0],[0,1,0],[0,0,1]]});
    assert_eq!(
        request("import_stl", improper, true)["result"]["isError"],
        true
    );
    assert_eq!(read_ocad(&path).expect("unchanged"), before);
    assert!(request("opencad.import_stl", params(&path), false)["error"].is_null());
    let imported = read_ocad(&path).expect("doc");
    assert_eq!(
        request("import_stl", params(&path), true)["result"]["isError"],
        true
    );
    assert_eq!(read_ocad(&path).expect("unchanged"), imported);
}

#[test]
#[ignore = "generate verified explicit-unit STL example"]
fn generate_stl_example() {
    let path = root().join("examples/stl_import_plate.ocad.d");
    empty(&path);
    let output = request("opencad.import_stl", params(&path), false);
    assert!(output["error"].is_null(), "{output}");
    let doc = read_ocad(&path).expect("document");
    let patch = authoring_patch(&document_design_state(&doc)).expect("authoring");
    std::fs::write(
        root().join("examples/agent/create_stl_import_plate_patch.json"),
        serde_json::to_string_pretty(&patch).expect("json") + "\n",
    )
    .expect("patch");
}
