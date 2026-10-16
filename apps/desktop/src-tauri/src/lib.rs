use std::path::{Path, PathBuf};

use opencad_desktop::{
    create_document, inspect_document, inspect_document_regeneration, inspect_parameter_intent,
    inspect_reference_intent, list_document_parameters, load_view_data, pick_document,
    preview_document, redo_document_with_history, run_desktop_smoke,
    run_document_viewport_with_sync, set_document_parameter_with_history,
    undo_document_with_history, DocumentHistory, DocumentHistoryState, DocumentInspect,
    DocumentPreview, DocumentTemplate, ParameterIntent, ParameterRow, PickOptions, PickSummary,
    PreviewSynced, ReferenceIntent, RegenInspectionResult, PREVIEW_HEIGHT, PREVIEW_WIDTH,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
struct TemplateInfo {
    id: String,
    label: String,
}

fn map_error(err: opencad_core::OpenCadError) -> String {
    err.to_string()
}

#[tauri::command]
fn inspect_sketch_creation_cmd(
    path: String,
) -> Result<opencad_desktop::sketch_creation::SketchCreationState, String> {
    opencad_desktop::sketch_creation::inspect_document_sketch_creation(&path).map_err(map_error)
}
#[tauri::command]
fn plan_sketch_create_cmd(
    path: String,
    create: opencad_desktop::sketch_creation::SketchCreate,
) -> Result<opencad_desktop::sketch_creation::SketchCreatePlan, String> {
    opencad_desktop::sketch_creation::plan_document_sketch_create(&path, &create).map_err(map_error)
}
#[tauri::command]
fn apply_sketch_create_cmd(
    path: String,
    create: opencad_desktop::sketch_creation::SketchCreate,
    history: Option<DocumentHistory>,
) -> Result<opencad_desktop::FeatureEditResult, String> {
    opencad_desktop::sketch_creation::apply_document_sketch_create(&path, &create, history)
        .map_err(map_error)
}

#[tauri::command]
fn inspect_sketches_cmd(
    path: String,
) -> Result<opencad_desktop::sketch_editor::SketchEditorState, String> {
    opencad_desktop::sketch_editor::inspect_document_sketches(&path).map_err(map_error)
}
#[tauri::command]
fn plan_sketch_edit_cmd(
    path: String,
    edit: opencad_desktop::sketch_editor::SketchEdit,
) -> Result<opencad_desktop::sketch_editor::SketchEditPlan, String> {
    opencad_desktop::sketch_editor::plan_document_sketch_edit(&path, &edit).map_err(map_error)
}
#[tauri::command]
fn apply_sketch_edit_cmd(
    path: String,
    edit: opencad_desktop::sketch_editor::SketchEdit,
    history: Option<DocumentHistory>,
) -> Result<opencad_desktop::FeatureEditResult, String> {
    opencad_desktop::sketch_editor::apply_document_sketch_edit(&path, &edit, history)
        .map_err(map_error)
}

#[tauri::command]
fn plan_selection_edit_cmd(
    path: String,
    edit: opencad_desktop::selection::SelectionEdit,
) -> Result<opencad_desktop::FeatureEditPlan, String> {
    opencad_desktop::selection::plan_document_selection_edit(&path, &edit).map_err(map_error)
}

#[tauri::command]
fn apply_selection_edit_cmd(
    path: String,
    edit: opencad_desktop::selection::SelectionEdit,
    history: Option<DocumentHistory>,
) -> Result<opencad_desktop::FeatureEditResult, String> {
    opencad_desktop::selection::apply_document_selection_edit(&path, &edit, history)
        .map_err(map_error)
}

#[tauri::command]
fn list_document_features_cmd(path: String) -> Result<opencad_desktop::FeatureTree, String> {
    opencad_desktop::list_document_features(&path).map_err(map_error)
}

#[tauri::command]
fn plan_feature_edit_cmd(
    path: String,
    revision: String,
    edit: opencad_desktop::FeatureEdit,
) -> Result<opencad_desktop::FeatureEditPlan, String> {
    opencad_desktop::plan_document_feature_edit(&path, &revision, edit).map_err(map_error)
}

#[tauri::command]
fn apply_feature_edit_cmd(
    path: String,
    revision: String,
    edit: opencad_desktop::FeatureEdit,
    history: Option<DocumentHistory>,
) -> Result<opencad_desktop::FeatureEditResult, String> {
    opencad_desktop::apply_document_feature_edit(&path, &revision, edit, history).map_err(map_error)
}

#[tauri::command]
fn list_templates() -> Vec<TemplateInfo> {
    DocumentTemplate::all()
        .iter()
        .map(|template| TemplateInfo {
            id: template.as_str().to_string(),
            label: template.as_str().replace('-', " "),
        })
        .collect()
}

#[tauri::command]
fn default_example_path() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let mut candidates = vec![
        cwd.join("examples/bracket.ocad.d"),
        cwd.join("../../examples/bracket.ocad.d"),
    ];
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest_dir = PathBuf::from(manifest);
        if let Some(workspace) = manifest_dir.parent().and_then(|p| p.parent()) {
            candidates.push(workspace.join("examples/bracket.ocad.d"));
        }
    }
    candidates
        .into_iter()
        .find(|path| path.is_dir())
        .map(|path| path.to_string_lossy().to_string())
}

#[tauri::command]
fn inspect_document_cmd(path: String) -> Result<DocumentInspect, String> {
    inspect_document(&path).map_err(map_error)
}

#[tauri::command]
fn preview_document_cmd(path: String) -> Result<DocumentPreview, String> {
    preview_document(&path).map_err(map_error)
}

#[tauri::command]
fn inspect_regeneration_cmd(path: String) -> Result<RegenInspectionResult, String> {
    inspect_document_regeneration(&path).map_err(map_error)
}

#[tauri::command]
fn inspect_parameter_intent_cmd(path: String, id: String) -> Result<ParameterIntent, String> {
    inspect_parameter_intent(&path, &id).map_err(map_error)
}

#[tauri::command]
fn inspect_reference_intent_cmd(
    path: String,
    reference: String,
) -> Result<ReferenceIntent, String> {
    inspect_reference_intent(&path, &reference).map_err(map_error)
}

#[tauri::command]
fn create_template_document(path: String, template_id: String) -> Result<(), String> {
    let template = DocumentTemplate::parse(&template_id).map_err(map_error)?;
    if Path::new(&path).exists() {
        return Err(format!("path already exists: {path}"));
    }
    create_document(&path, template).map_err(map_error)
}

#[tauri::command]
fn list_document_parameters_cmd(path: String) -> Result<Vec<ParameterRow>, String> {
    list_document_parameters(&path).map_err(map_error)
}

#[tauri::command]
fn set_document_parameter_cmd(
    path: String,
    id: String,
    expr: String,
    history: Option<DocumentHistory>,
) -> Result<DocumentHistoryState, String> {
    set_document_parameter_with_history(&path, &id, &expr, history).map_err(map_error)
}

#[tauri::command]
fn undo_document_cmd(
    path: String,
    history: DocumentHistory,
) -> Result<DocumentHistoryState, String> {
    undo_document_with_history(&path, history).map_err(map_error)
}

#[tauri::command]
fn redo_document_cmd(
    path: String,
    history: DocumentHistory,
) -> Result<DocumentHistoryState, String> {
    redo_document_with_history(&path, history).map_err(map_error)
}

#[tauri::command]
fn open_viewport_cmd(app: AppHandle, path: String) -> Result<(), String> {
    let data = load_view_data(&path).map_err(map_error)?;
    let title = data.name.clone();
    let app_handle = app.clone();
    let app_syncing = app.clone();
    let app_synced = app.clone();
    let app_aborted = app.clone();
    std::thread::spawn(move || {
        let on_pick = move |summary: PickSummary| {
            if let Err(err) = app_handle.emit("viewport-pick", &summary) {
                eprintln!("failed to emit viewport pick: {err}");
            }
        };
        let on_camera_sync = (
            move || {
                if let Err(err) = app_syncing.emit("preview-syncing", ()) {
                    eprintln!("failed to emit preview syncing: {err}");
                }
            },
            move |synced: PreviewSynced| {
                if let Err(err) = app_synced.emit("preview-synced", &synced) {
                    eprintln!("failed to emit preview sync: {err}");
                }
            },
            move || {
                if let Err(err) = app_aborted.emit("preview-sync-failed", ()) {
                    eprintln!("failed to emit preview sync failed: {err}");
                }
            },
        );
        if let Err(err) =
            run_document_viewport_with_sync(data, &title, Some(on_pick), Some(on_camera_sync))
        {
            eprintln!("viewport error: {err}");
        }
    });
    Ok(())
}

#[tauri::command]
fn pick_document_cmd(path: String, x: f64, y: f64) -> Result<PickSummary, String> {
    let options = PickOptions {
        x,
        y,
        width: PREVIEW_WIDTH,
        height: PREVIEW_HEIGHT,
    };
    pick_document(&path, &options).map_err(map_error)
}

/// Handle the non-GUI command-line modes used by release checks.
///
/// `None` means that the arguments are not a headless mode and the caller
/// should continue into the Tauri event loop.  Returning an exit code keeps
/// this parser testable without starting a native window in CI.
pub fn run_headless(args: &[String]) -> Option<i32> {
    match args {
        [flag] if flag == "--version" || flag == "-V" => {
            println!("musubicad-desktop {}", env!("CARGO_PKG_VERSION"));
            Some(0)
        }
        [flag, source, work_dir] if flag == "--smoke-test" => {
            let summary = run_desktop_smoke(Path::new(source), Path::new(work_dir));
            match summary {
                Ok(summary) => match serde_json::to_string_pretty(&summary) {
                    Ok(json) => {
                        println!("{json}");
                        Some(0)
                    }
                    Err(error) => {
                        eprintln!("desktop smoke summary serialization failed: {error}");
                        Some(1)
                    }
                },
                Err(error) => {
                    eprintln!("desktop smoke test failed: {error}");
                    Some(1)
                }
            }
        }
        [flag, ..] if flag == "--smoke-test" => {
            eprintln!("usage: musubicad-desktop --smoke-test <source> <work-dir>");
            Some(2)
        }
        _ => None,
    }
}

/// Start either a headless release check or the normal Tauri shell.
pub fn run_with_args(args: impl IntoIterator<Item = String>) {
    let args = args.into_iter().collect::<Vec<_>>();
    if let Some(code) = run_headless(&args) {
        if code != 0 {
            std::process::exit(code);
        }
        return;
    }
    run_gui();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    run_with_args(std::env::args().skip(1));
}

fn run_gui() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_document_features_cmd,
            plan_feature_edit_cmd,
            apply_feature_edit_cmd,
            inspect_sketches_cmd,
            inspect_sketch_creation_cmd,
            plan_sketch_create_cmd,
            apply_sketch_create_cmd,
            plan_sketch_edit_cmd,
            apply_sketch_edit_cmd,
            plan_selection_edit_cmd,
            apply_selection_edit_cmd,
            list_templates,
            default_example_path,
            inspect_document_cmd,
            preview_document_cmd,
            inspect_regeneration_cmd,
            inspect_parameter_intent_cmd,
            inspect_reference_intent_cmd,
            create_template_document,
            list_document_parameters_cmd,
            set_document_parameter_cmd,
            undo_document_cmd,
            redo_document_cmd,
            open_viewport_cmd,
            pick_document_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MusubiCAD desktop");
}
