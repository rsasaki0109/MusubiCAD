use std::path::{Path, PathBuf};

use opencad_desktop::{
    create_document, inspect_document, preview_document, DocumentInspect, DocumentPreview,
    DocumentTemplate,
};
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running ForgeCAD desktop");
}
