//! Real MCP/Agent and native file-preservation evidence for articulated copies.
#[path = "../../file/tests/support/articulated.rs"]
mod support;
use opencad_file::*;
use serde_json::{json, Value};
use support::*;
#[path = "support/transport.rs"]
mod transport;
use transport::*;
#[test]
fn mcp_source_pose_apply_refusal_and_agent_history_preserve_children() {
    let temporary = tempfile::tempdir().expect("temp");
    let path = temporary.path().join("pair.ocad.d");
    let (parent, child, _) = fixture(&path);
    let children = logical_files(&path.join("parts"));
    let source = request(
        "inspect_articulation_source",
        json!({"path":path.join("parts/mechanism.ocad.d")}),
        true,
    );
    assert_eq!(
        source["result"]["structuredContent"]["source_doc"],
        "doc:art_mechanism"
    );
    assert_eq!(
        source["result"]["structuredContent"]["independent_joints"],
        json!(["joint:hinge"])
    );
    let regen = request("regen_document", json!({"path":path}), true);
    let leaves = &regen["result"]["structuredContent"]["assembly_leaves"];
    assert_eq!(leaves.as_array().expect("leaves").len(), 4);
    assert!(
        (regen["result"]["structuredContent"]["volume_m3"]
            .as_f64()
            .expect("volume")
            - 24e-6)
            .abs()
            < 1e-11
    );
    let change = pose_patch(&child, "0 deg");
    let before = logical_files(&path);
    let review = request(
        "review_patch",
        json!({"path":path,"patch":change,"output_dir":temporary.path().join("review")}),
        true,
    );
    assert!(
        !review["result"]["isError"].as_bool().unwrap_or(false),
        "{review}"
    );
    assert_eq!(logical_files(&path), before);
    let applied = request(
        "patch_apply",
        json!({"path":path,"patch":change,"verify":false}),
        true,
    );
    assert!(
        !applied["result"]["isError"].as_bool().unwrap_or(false),
        "{applied}"
    );
    assert_eq!(logical_files(&path.join("parts")), children);
    let history = applied["result"]["structuredContent"]["history"].clone();
    assert!(history.is_object());
    let undo = request(
        "opencad.history_undo_document",
        json!({"path":path,"history":history}),
        false,
    );
    assert!(undo.get("error").is_none(), "{undo}");
    assert_eq!(read_ocad(&path).expect("source"), parent);
    let redo = request(
        "opencad.history_redo_document",
        json!({"path":path,"history":undo["result"]["history"]}),
        false,
    );
    assert!(redo.get("error").is_none(), "{redo}");
    assert_eq!(logical_files(&path.join("parts")), children);
    for position in ["10", "90 mm", "999 deg"] {
        let stable = logical_files(&path);
        let refusal = request(
            "patch_apply",
            json!({"path":path,"patch":pose_patch(&child,position),"verify":false}),
            true,
        );
        assert_eq!(refusal["result"]["isError"], true, "{refusal}");
        assert_eq!(logical_files(&path), stable);
    }
    let output = temporary.path().join("existing.urdf");
    std::fs::write(&output, b"KEEP").expect("sentinel");
    let result = cli(&[
        "export",
        path.to_str().expect("path"),
        output.to_str().expect("path"),
    ]);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("hierarchical articulated"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(std::fs::read(&output).expect("sentinel"), b"KEEP");
    let html = temporary.path().join("existing.html");
    std::fs::write(&html, b"KEEP").expect("sentinel");
    let result = cli(&[
        "export",
        path.to_str().expect("path"),
        html.to_str().expect("path"),
    ]);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("hierarchical articulated"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(std::fs::read(&html).expect("sentinel"), b"KEEP");
    let bom = cli(&["bom", path.to_str().expect("path")]);
    assert!(
        bom.status.success(),
        "{}",
        String::from_utf8_lossy(&bom.stderr)
    );
    let bom: Value = serde_json::from_slice(&bom.stdout).expect("BOM");
    assert_eq!(bom["leaf_instance_count"], 4);
}

#[test]
fn stale_child_undo_refuses_without_replacing_parent_or_consuming_history() {
    let temporary = tempfile::tempdir().expect("temp");
    let path = temporary.path().join("pair.ocad.d");
    let (_, mut child, _) = fixture(&path);
    let applied = request(
        "patch_apply",
        json!({"path":path,"patch":pose_patch(&child,"0 deg")}),
        true,
    );
    let history = applied["result"]["structuredContent"]["history"].clone();
    let child_before = child.clone();
    child.assembly.as_mut().expect("model").joints[0].motion =
        opencad_assembly::JointMotion::Revolute {
            lower_rad: -1.,
            upper_rad: 1.,
            effort_n_m: 1.,
            velocity_rad_s: 1.,
        };
    write_ocad(path.join("parts/mechanism.ocad.d"), &child).expect("change");
    let stable = logical_files(&path);
    let undo = request(
        "opencad.history_undo_document",
        json!({"path":path,"history":history}),
        false,
    );
    assert!(undo.get("error").is_some(), "{undo}");
    assert_eq!(logical_files(&path), stable);
    write_ocad(path.join("parts/mechanism.ocad.d"), &child_before).expect("restore");
    let undo = request(
        "opencad.history_undo_document",
        json!({"path":path,"history":history}),
        false,
    );
    assert!(
        undo.get("error").is_none(),
        "opaque original history remains usable: {undo}"
    );
}
