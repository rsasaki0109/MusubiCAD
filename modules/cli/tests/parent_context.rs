//! Actual MCP/Agent edits of parent constraints, independent of child authoring.
#[path = "../../file/tests/support/context.rs"]
mod context_support;
#[path = "../../file/tests/support/articulated.rs"]
mod support;
#[path = "support/transport.rs"]
mod transport;
use context_support::*;
use opencad_file::read_ocad;
use serde_json::json;
use transport::*;
#[test]
fn actual_mcp_parent_context_review_apply_history_and_refusal() {
    for angular in [false, true] {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("pair.ocad.d");
        let (parent, child) = context_fixture(&path, angular);
        let children = logical_files(&path.join("parts"));
        let source = request(
            "inspect_articulation_source",
            json!({"path":path.join("parts/mechanism.ocad.d")}),
            true,
        );
        assert_eq!(
            source["result"]["structuredContent"]["connectors"][0]["id"],
            "connector:link_tip"
        );
        let original = request("regen_document", json!({"path":path}), true);
        assert_eq!(original["result"]["structuredContent"]["context_dof"], 0);
        let change = context_patch(&child, if angular { "90 deg" } else { "0 mm" }, angular);
        let before = logical_files(&path);
        let review = request(
            "review_patch",
            json!({"path":path,"patch":change,"output_dir":temp.path().join("review")}),
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
        let changed = request("regen_document", json!({"path":path}), true);
        let leaves = changed["result"]["structuredContent"]["assembly_leaves"]
            .as_array()
            .expect("leaves");
        assert_eq!(leaves.len(), 5);
        assert!(
            (changed["result"]["structuredContent"]["volume_m3"]
                .as_f64()
                .expect("volume")
                - 30e-6)
                .abs()
                < 1e-11
        );
        let leaf = leaves
            .iter()
            .find(|l| l["instance_path"] == json!(["instance:left", "instance:link"]))
            .expect("link");
        let translation = leaf["placement"]["translation_m"]
            .as_array()
            .expect("translation");
        for (a, b) in translation.iter().zip([0.1, 0., 0.]) {
            assert!((a.as_f64().expect("SI") - b).abs() < 1e-9);
        }
        let original_leaves = original["result"]["structuredContent"]["assembly_leaves"]
            .as_array()
            .expect("leaves");
        for a in leaves
            .iter()
            .filter(|l| l["instance_path"][0] == "instance:right")
        {
            let b = original_leaves
                .iter()
                .find(|l| l["instance_path"] == a["instance_path"])
                .expect("same copy");
            assert_eq!(a, b);
        }
        let undo = request(
            "opencad.history_undo_document",
            json!({"path":path,"history":applied["result"]["structuredContent"]["history"]}),
            false,
        );
        assert!(undo.get("error").is_none(), "{undo}");
        assert_eq!(read_ocad(&path).expect("parent"), parent);
        let redo = request(
            "opencad.history_redo_document",
            json!({"path":path,"history":undo["result"]["history"]}),
            false,
        );
        assert!(redo.get("error").is_none(), "{redo}");
        assert_eq!(logical_files(&path.join("parts")), children);
        for extension in ["urdf", "html"] {
            let output = temp.path().join(format!("existing.{extension}"));
            std::fs::write(&output, b"KEEP").expect("sentinel");
            let result = cli(&[
                "export",
                path.to_str().expect("path"),
                output.to_str().expect("path"),
            ]);
            assert!(!result.status.success());
            assert!(String::from_utf8_lossy(&result.stderr).contains("hierarchical articulated"));
            assert_eq!(std::fs::read(&output).expect("output"), b"KEEP");
        }
        let stable = logical_files(&path);
        let refused = request(
            "patch_apply",
            json!({"path":path,"patch":context_patch(&child,if angular{"999 deg"}else{"999 mm"},angular),"verify":false}),
            true,
        );
        assert_eq!(refused["result"]["isError"], true);
        assert_eq!(logical_files(&path), stable);
    }
}
