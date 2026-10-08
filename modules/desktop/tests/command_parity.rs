//! Static contract audit for Tauri/UI command parity.
//!
//! Tauri itself is intentionally excluded from the Rust workspace.  Including
//! the small command surfaces here keeps the audit headless and makes drift
//! fail in the normal workspace test rather than only during a GUI build.

const TAURI_SOURCE: &str = include_str!("../../../apps/desktop/src-tauri/src/lib.rs");
const UI_SOURCE: &str = include_str!("../../../apps/desktop/ui/main.js");
const CLI_SOURCE: &str = include_str!("../../../modules/cli/src/commands.rs");
const CLI_PATCH_SOURCE: &str = include_str!("../../../modules/cli/src/patch.rs");
const AGENT_SOURCE: &str = include_str!("../../../modules/cli/src/agent.rs");
const DESKTOP_PARAMETERS_SOURCE: &str = include_str!("../../../modules/desktop/src/parameters.rs");
const DESKTOP_TEMPLATE_SOURCE: &str = include_str!("../../../modules/desktop/src/template.rs");
const DESKTOP_API_DOC: &str = include_str!("../../../docs/api/desktop.md");

#[test]
fn every_ui_model_command_has_a_tauri_handler_and_cli_or_agent_route() {
    // The first item is the exact command string passed to Tauri invoke; the
    // second is the corresponding Rust handler; the third documents the
    // command-line or Agent API route available outside the GUI.
    // `default_example_path` is a shell bootstrap helper: it only resolves a
    // repository-local path before a document is open and has no model command
    // equivalent. Refresh is intentionally only the existing inspect/preview
    // load path; regeneration/export stay reusable backend operations for the
    // headless smoke contract and are not Tauri/UI commands.
    let parity = [
        ("list_templates", "list_templates", "new"),
        ("inspect_document_cmd", "inspect_document_cmd", "inspect"),
        ("preview_document_cmd", "preview_document_cmd", "screenshot"),
        (
            "inspect_regeneration_cmd",
            "inspect_regeneration_cmd",
            "opencad.inspect_regeneration_document",
        ),
        (
            "inspect_parameter_intent_cmd",
            "inspect_parameter_intent_cmd",
            "intent",
        ),
        (
            "inspect_reference_intent_cmd",
            "inspect_reference_intent_cmd",
            "intent",
        ),
        (
            "create_template_document",
            "create_template_document",
            "new",
        ),
        (
            "list_document_parameters_cmd",
            "list_document_parameters_cmd",
            "params",
        ),
        (
            "set_document_parameter_cmd",
            "set_document_parameter_cmd",
            "opencad.patch_apply_document",
        ),
        (
            "undo_document_cmd",
            "undo_document_cmd",
            "opencad.history_undo_document",
        ),
        (
            "redo_document_cmd",
            "redo_document_cmd",
            "opencad.history_redo_document",
        ),
        ("open_viewport_cmd", "open_viewport_cmd", "view"),
        (
            "pick_document_cmd",
            "pick_document_cmd",
            "opencad.pick_document",
        ),
    ];

    let registration = TAURI_SOURCE
        .split("tauri::generate_handler![")
        .nth(1)
        .and_then(|source| source.split("])").next())
        .expect("Tauri command registration must be present");

    for (ui_command, handler, parity_route) in parity {
        assert!(
            UI_SOURCE.contains(&format!("invoke(\"{ui_command}\"")),
            "UI command '{ui_command}' is not invoked by main.js"
        );
        assert!(
            TAURI_SOURCE.contains(&format!("fn {handler}")),
            "UI command '{ui_command}' has no Tauri handler '{handler}'"
        );
        assert!(
            registration.contains(&format!("{handler},")),
            "Tauri handler '{handler}' is not registered in generate_handler"
        );
        assert!(
            CLI_SOURCE.contains(&format!("Some(\"{parity_route}\")"))
                || AGENT_SOURCE.contains(&format!("\"{parity_route}\"")),
            "UI command '{ui_command}' has no CLI/Agent parity route '{parity_route}'"
        );
    }

    assert!(
        !UI_SOURCE.contains("regenerate_document_cmd")
            && !TAURI_SOURCE.contains("regenerate_document_cmd")
            && !UI_SOURCE.contains("export_document_cmd")
            && !TAURI_SOURCE.contains("export_document_cmd"),
        "refresh must not expose removed regenerate/export Tauri commands"
    );
    assert!(
        !UI_SOURCE.contains("paramUndoStack")
            && !UI_SOURCE.contains("paramRedoStack")
            && !UI_SOURCE.contains("applyingParamHistory"),
        "UI must not maintain semantic inverse stacks; backend history is authoritative"
    );
    assert!(
        TAURI_SOURCE.contains("set_document_parameter_with_history(&path, &id, &expr, history)"),
        "Tauri parameter mutation must delegate to the shared desktop history helper"
    );
    assert!(
        DESKTOP_PARAMETERS_SOURCE.contains("DesignPatch::set_parameter(id, expr)")
            && DESKTOP_PARAMETERS_SOURCE.contains("apply_patch_with_history"),
        "desktop parameter edits must use the validated DesignPatch/history boundary"
    );
    assert!(
        CLI_PATCH_SOURCE.contains("apply_patch_to_document(&mut doc, &patch)")
            && AGENT_SOURCE.contains("apply_patch_with_history"),
        "CLI and Agent document patch routes must use the shared file transaction boundary"
    );
    assert!(
        DESKTOP_TEMPLATE_SOURCE.contains("pub fn create_document")
            && CLI_SOURCE.contains("new::create_document"),
        "desktop template creation and CLI new must share the template implementation"
    );
    assert!(
        DESKTOP_API_DOC.contains("Refresh") && DESKTOP_API_DOC.contains("run_desktop_smoke"),
        "desktop API documentation must describe the actual refresh and smoke contracts"
    );
}

#[test]
fn desktop_parameter_command_matches_direct_design_patch_transaction() {
    use opencad_ai::DesignPatch;
    use opencad_desktop::fixture::write_bracket_fixture_at;
    use opencad_desktop::{set_document_parameter_with_history, DocumentHistoryState};
    use opencad_file::{apply_patch_to_document, read_ocad};
    use tempfile::tempdir;

    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("bracket.ocad.d");
    write_bracket_fixture_at(&path);
    let path_str = path.to_str().expect("path");
    let before = read_ocad(path_str).expect("before");
    let mut expected = before.clone();
    apply_patch_to_document(
        &mut expected,
        &DesignPatch::set_parameter("param:width", "100 mm"),
    )
    .expect("direct patch");

    let history: DocumentHistoryState =
        set_document_parameter_with_history(path_str, "param:width", "100 mm", None)
            .expect("desktop command");
    assert!(history.can_undo);
    assert_eq!(read_ocad(path_str).expect("after"), expected);
}

#[test]
fn ui_keeps_document_and_viewport_state_separate() {
    assert!(
        UI_SOURCE.contains("currentPath") && UI_SOURCE.contains("previewSync"),
        "UI must keep document path and viewport preview sync state explicit"
    );
    assert!(
        !UI_SOURCE.contains("currentPath.parameters")
            && !UI_SOURCE.contains("currentPath.feature_nodes"),
        "UI must not mutate the Design Graph directly"
    );
}
