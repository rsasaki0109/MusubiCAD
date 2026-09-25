//! ADR-015: a real `git merge` of two branches that edited the same
//! `.ocad.d` document, resolved by `opencad merge-driver`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use opencad_ai::DesignPatch;
use opencad_file::{apply_patch_to_document, read_ocad, validate_ocad, write_expanded_dir};
use serde_json::json;

fn opencad() -> &'static str {
    env!("CARGO_BIN_EXE_opencad")
}

fn run(repo: &Path, program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("run {program}: {error}"))
}

fn git(repo: &Path, args: &[&str]) -> Output {
    let output = run(repo, "git", args);
    assert!(
        output.status.success(),
        "git {}: {}{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn bracket() -> opencad_file::OcadDocument {
    read_ocad(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bracket.ocad.d"))
        .expect("read bracket")
}

/// Apply a patch to the committed document on the current branch and commit.
fn commit_patch(repo: &Path, message: &str, operations: serde_json::Value) {
    let path = repo.join("bracket.ocad.d");
    let mut doc = read_ocad(&path).expect("read working document");
    let patch: DesignPatch =
        serde_json::from_value(json!({ "operations": operations })).expect("patch");
    apply_patch_to_document(&mut doc, &patch).expect("apply");
    write_expanded_dir(&path, &doc).expect("write");
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", message]);
}

/// A repository with the driver installed and the bracket committed on main.
fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "user.name", "Test"]);
    git(repo, &["config", "core.autocrlf", "false"]);
    let install = run(repo, opencad(), &["merge-driver", "install"]);
    assert!(
        install.status.success(),
        "{}",
        String::from_utf8_lossy(&install.stderr)
    );
    let attributes = String::from_utf8_lossy(&install.stdout)
        .lines()
        .filter(|line| line.contains("merge=musubicad"))
        .map(|line| format!("{}\n", line.trim()))
        .collect::<String>();
    assert!(
        !attributes.is_empty(),
        "install prints .gitattributes lines"
    );
    std::fs::write(repo.join(".gitattributes"), attributes).expect("attributes");
    write_expanded_dir(repo.join("bracket.ocad.d"), &bracket()).expect("write bracket");
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "base"]);
    dir
}

#[test]
fn independent_structural_edits_merge_through_git() {
    let dir = repository();
    let repo = dir.path();

    git(repo, &["checkout", "-q", "-b", "fillet"]);
    commit_patch(
        repo,
        "add fillet",
        json!([{
            "type": "add_feature",
            "position": { "after": "feature:hole_mount" },
            "node": { "id": "feature:fillet_a", "name": "Fillet A", "definition": {
                "type": "fillet", "target_feature": "feature:hole_mount",
                "radius": { "value_si": 0.001 }, "radius_expr": "fillet_radius",
                "edge_ref": "ref:edge:bracket_top_front", "edge_selector": "top_perimeter" } }
        }]),
    );

    git(repo, &["checkout", "-q", "main"]);
    git(repo, &["checkout", "-q", "-b", "datum"]);
    commit_patch(
        repo,
        "add datum and rib",
        json!([
            { "type": "add_parameter", "id": "param:rib", "name": "rib", "expr": "4 mm" },
            { "type": "add_sketch_entity", "sketch_id": "sketch:base",
              "entity": { "type": "point", "id": "ent:datum", "construction": true, "x": 0.04, "y": 0.03 } }
        ]),
    );

    git(repo, &["checkout", "-q", "fillet"]);
    git(repo, &["merge", "-q", "--no-edit", "datum"]);

    // Checksums verify, so every file of the directory agrees.
    let merged = validate_ocad(repo.join("bracket.ocad.d")).expect("valid merged document");
    assert!(merged.parameters.get("param:rib").is_some());
    assert!(merged.sketches[0].find_entity("ent:datum").is_some());
    assert!(merged
        .feature_nodes
        .iter()
        .any(|node| node.id == "feature:fillet_a"));
    assert!(merged
        .feature_graph
        .dependency_edges()
        .iter()
        .any(|edge| edge.source == "feature:hole_mount" && edge.target == "feature:fillet_a"));
    let status = git(repo, &["status", "--porcelain"]);
    assert!(status.stdout.is_empty(), "clean after merge");
}

#[test]
fn conflicting_intent_stops_the_merge_with_typed_conflicts() {
    let dir = repository();
    let repo = dir.path();

    git(repo, &["checkout", "-q", "-b", "wide"]);
    commit_patch(
        repo,
        "wider",
        json!([
            { "type": "set_parameter", "id": "param:width", "expr": "90 mm" }
        ]),
    );
    git(repo, &["checkout", "-q", "main"]);
    git(repo, &["checkout", "-q", "-b", "narrow"]);
    commit_patch(
        repo,
        "narrower",
        json!([
            { "type": "set_parameter", "id": "param:width", "expr": "70 mm" }
        ]),
    );

    let merge = run(repo, "git", &["merge", "--no-edit", "wide"]);
    assert!(!merge.status.success(), "conflicting widths must not merge");
    let stderr = String::from_utf8_lossy(&merge.stderr);
    assert!(stderr.contains("param:width"), "{stderr}");

    let conflicts = run(repo, opencad(), &["conflicts", "bracket.ocad.d"]);
    assert!(!conflicts.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&conflicts.stdout).expect("conflicts JSON");
    assert_eq!(report[0]["kind"], "parameter");
    assert_eq!(report[0]["id"], "param:width");
    assert_eq!(report[0]["ours"], "70 mm");
    assert_eq!(report[0]["theirs"], "90 mm");
}
