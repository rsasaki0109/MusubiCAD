use std::path::{Path, PathBuf};

use opencad_desktop::{
    create_document, inspect_document, list_document_parameters, load_view_data,
    pick_document, preview_document, run_document_viewport_with_sync, set_document_parameter,
    DocumentInspect, DocumentPreview, DocumentTemplate, ParameterRow, PickOptions, PickSummary,
    PreviewSynced, PREVIEW_HEIGHT, PREVIEW_WIDTH,
};
use tauri::{AppHandle, Emitter};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct TemplateInfo {
    id: String,
    label: String,
}

fn map_error(err: opencad_core::OpenCadError) -> String {
    err.to_string()
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
fn set_document_parameter_cmd(path: String, id: String, expr: String) -> Result<(), String> {
    set_document_parameter(&path, &id, &expr).map_err(map_error)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_templates,
            default_example_path,
            inspect_document_cmd,
            preview_document_cmd,
            create_template_document,
            list_document_parameters_cmd,
            set_document_parameter_cmd,
            open_viewport_cmd,
            pick_document_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MusubiCAD desktop");
}
