//! ADR-018: kernel topology IDs are deterministic, so syncing semantic
//! references writes identical files (integration test: requires OCCT).

use std::path::{Path, PathBuf};
use std::process::Command;

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for entry in std::fs::read_dir(from).expect("read dir") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy");
        }
    }
}

fn synced_refs(workspace: &Path) -> String {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bracket.ocad.d");
    let doc = workspace.join("bracket.ocad.d");
    copy_dir(&source, &doc);
    let output = Command::new(env!("CARGO_BIN_EXE_opencad"))
        .args(["regen", doc.to_str().expect("path"), "--sync-topo-refs"])
        .output()
        .expect("run opencad");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(doc.join("graph/semantic_refs.json")).expect("refs")
}

#[test]
fn syncing_topology_references_is_reproducible() {
    let (first, second) = (
        tempfile::tempdir().expect("tempdir"),
        tempfile::tempdir().expect("tempdir"),
    );
    let (a, b) = (synced_refs(first.path()), synced_refs(second.path()));
    assert_eq!(
        a, b,
        "two syncs of the same document must write the same file"
    );

    // IDs are small enumeration indices, not addresses.
    let refs: serde_json::Value = serde_json::from_str(&a).expect("json");
    let ids: Vec<u64> = refs["semantic_refs"]
        .as_array()
        .expect("refs")
        .iter()
        .filter_map(|topo_ref| topo_ref["geometric_fingerprint"]["kernel_face_id"].as_u64())
        .collect();
    assert!(!ids.is_empty(), "sync persisted kernel face ids");
    assert!(ids.iter().all(|id| (1..1000).contains(id)), "{ids:?}");
}
