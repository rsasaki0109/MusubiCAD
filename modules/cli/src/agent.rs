//! `opencad agent` JSON-RPC stdio server (Task-156+).

use std::io::{self, BufRead, Write};

use opencad_ai::{
    query_needs_scene, AgentApi, DesignQuery, ExplainParams, JsonRpcError, JsonRpcRequest,
    JsonRpcResponse, QueryParams,
};
use opencad_core::Result;
use opencad_file::{
    apply_patch_to_document, apply_patch_with_history, dry_run_patch_document, read_ocad,
    validate_ocad, write_ocad, DocumentHistory, DocumentHistoryState,
};

use crate::diff::{self, DiffOptions};
use crate::export;
use crate::mesh;
use crate::pick::{self, PickOptions};
use crate::plugin::{self, PluginHost};
use crate::regen::{self, RegenBodyParams, RegenResult};
use crate::scene_query;
use crate::topo_sync;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Document path parameters for file-backed RPC methods.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DocumentPathParams {
    pub path: String,
}

/// Regeneration parameters for `opencad.regen_document`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DocumentRegenParams {
    pub path: String,
    #[serde(default)]
    pub sync_topo_refs: bool,
}

/// Patch a document on disk without writing changes.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DocumentPatchParams {
    pub path: String,
    pub patch: opencad_ai::DesignPatch,
    #[serde(default)]
    pub history: Option<DocumentHistory>,
}

/// Transport value for file-backed undo/redo methods. The history snapshots
/// are opaque to the Agent/UI client and must be returned unchanged between
/// commands except for the backend operation being requested.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DocumentHistoryParams {
    pub path: String,
    pub history: DocumentHistory,
}

/// Export a document body to STL.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DocumentExportParams {
    pub path: String,
    pub output: String,
}

/// Compare documents on disk or preview a patch against one document.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DocumentDiffParams {
    pub before: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<opencad_ai::DesignPatch>,
    #[serde(default)]
    pub geometry: bool,
}

/// Query a document on disk.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DocumentQueryParams {
    pub path: String,
    pub query: DesignQuery,
}

/// Pick a viewport target on a document at pixel coordinates.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DocumentPickParams {
    pub path: String,
    pub x: f64,
    pub y: f64,
    #[serde(default = "default_pick_width")]
    pub width: u32,
    #[serde(default = "default_pick_height")]
    pub height: u32,
}

fn default_pick_width() -> u32 {
    crate::mesh::PREVIEW_WIDTH
}

fn default_pick_height() -> u32 {
    crate::mesh::PREVIEW_HEIGHT
}

/// Assign a semantic topo ref to a tessellated B-Rep face.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DocumentAssignFaceRefParams {
    pub path: String,
    pub kernel_face_id: u64,
    pub ref_id: String,
    pub created_by: String,
    pub role: String,
    #[serde(default = "default_top_normal")]
    pub normal_m: [f32; 3],
}

fn default_top_normal() -> [f32; 3] {
    [0.0, 0.0, 1.0]
}

/// Summary returned by `opencad.inspect`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentInspectResult {
    pub id: String,
    pub name: String,
    pub sketches: usize,
    pub features: usize,
    pub parameters: usize,
}

/// Parameters for deterministic plugin discovery.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct PluginListParams {}

/// Host-owned parameters for a linked plugin invocation.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PluginInvokeParams {
    pub plugin_id: String,
    pub path: String,
    pub request: Value,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub history: Option<DocumentHistory>,
}

/// Dispatch JSON-RPC requests, including file-backed methods.
#[allow(dead_code)]
pub fn handle_agent_request(request: &JsonRpcRequest) -> JsonRpcResponse {
    let host = match PluginHost::with_builtins() {
        Ok(host) => host,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            )
        }
    };
    handle_agent_request_with_plugins(request, &host)
}

/// Dispatch requests using an injected, non-global plugin host.
pub fn handle_agent_request_with_plugins(
    request: &JsonRpcRequest,
    host: &PluginHost,
) -> JsonRpcResponse {
    if request.jsonrpc != "2.0" {
        return JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::invalid_request("jsonrpc must be '2.0'"),
        );
    }

    match request.method.as_str() {
        "opencad.inspect" => handle_inspect(request),
        "opencad.validate" => handle_validate(request),
        "opencad.patch_dry_run_document" => handle_patch_dry_run_document(request),
        "opencad.patch_apply_document" => handle_patch_apply_document(request),
        "opencad.history_undo_document" => handle_history_undo_document(request),
        "opencad.history_redo_document" => handle_history_redo_document(request),
        "opencad.regen_document" => handle_regen_document(request),
        "opencad.regen" => handle_regen(request),
        "opencad.export" => handle_export(request),
        "opencad.diff_document" => handle_diff_document(request),
        "opencad.query_document" => handle_query_document(request),
        "opencad.pick_document" => handle_pick_document(request),
        "opencad.sync_topo_refs_document" => handle_sync_topo_refs_document(request),
        "opencad.assign_face_ref_document" => handle_assign_face_ref_document(request),
        "opencad.explain_document" => handle_explain_document(request),
        "opencad.plugin_list" => handle_plugin_list(request, host),
        "opencad.plugin_invoke" => handle_plugin_invoke(request, host),
        _ => AgentApi.handle_request(request),
    }
}

/// Read JSON-RPC requests from stdin and write responses to stdout.
pub fn serve_stdio() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let plugin_host = PluginHost::with_builtins()?;
    for line in stdin.lock().lines() {
        let line = line.map_err(|err| {
            opencad_core::OpenCadError::Other(format!("failed to read stdin: {err}"))
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match handle_json_line_with_documents(&line, &plugin_host) {
            Ok(response_line) => response_line,
            Err(err) => error_response_line(&line, err.to_string()),
        };
        writeln!(stdout, "{response}").map_err(|err| {
            opencad_core::OpenCadError::Other(format!("failed to write stdout: {err}"))
        })?;
        stdout.flush().map_err(|err| {
            opencad_core::OpenCadError::Other(format!("failed to flush stdout: {err}"))
        })?;
    }
    Ok(())
}

fn handle_json_line_with_documents(line: &str, host: &PluginHost) -> Result<String> {
    let request: JsonRpcRequest = serde_json::from_str(line).map_err(|err| {
        opencad_core::OpenCadError::validation(format!("invalid JSON-RPC: {err}"))
    })?;
    let response = handle_agent_request_with_plugins(&request, host);
    serde_json::to_string(&response).map_err(|err| {
        opencad_core::OpenCadError::Other(format!("failed to encode response: {err}"))
    })
}

fn error_response_line(line: &str, message: String) -> String {
    let id = serde_json::from_str::<JsonRpcRequest>(line)
        .map(|request| request.id)
        .unwrap_or(Value::Null);
    let response = JsonRpcResponse::error(id, JsonRpcError::parse_error(message));
    serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"response encoding failed"}}"#
            .into()
    })
}

fn handle_plugin_list(request: &JsonRpcRequest, host: &PluginHost) -> JsonRpcResponse {
    if let Err(err) = serde_json::from_value::<PluginListParams>(request.params.clone()) {
        return JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::invalid_params(err.to_string()),
        );
    }
    JsonRpcResponse::success(
        request.id.clone(),
        serde_json::json!({ "plugins": host.list() }),
    )
}

fn handle_plugin_invoke(request: &JsonRpcRequest, host: &PluginHost) -> JsonRpcResponse {
    let params = match serde_json::from_value::<PluginInvokeParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            )
        }
    };
    match plugin::invoke_plugin_on_path(
        host,
        &params.plugin_id,
        &params.path,
        params.request,
        params.dry_run,
        params.output.as_deref(),
        params.history,
    ) {
        Ok(result) => match serde_json::to_value(result) {
            Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
            Err(err) => JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            ),
        },
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_inspect(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentPathParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    match read_ocad(&params.path) {
        Ok(doc) => {
            let result = DocumentInspectResult {
                id: doc.metadata.id.as_str().to_string(),
                name: doc.metadata.name.clone(),
                sketches: doc.sketches.len(),
                features: doc.feature_nodes.len(),
                parameters: doc
                    .parameters
                    .evaluation_order()
                    .map(|p| p.len())
                    .unwrap_or(0),
            };
            match serde_json::to_value(result) {
                Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
                Err(err) => JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::application_error(err.to_string()),
                ),
            }
        }
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_validate(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentPathParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    match validate_ocad(&params.path) {
        Ok(_) => JsonRpcResponse::success(
            request.id.clone(),
            serde_json::json!({ "valid": true, "path": params.path }),
        ),
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_patch_dry_run_document(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentPatchParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    match read_ocad(&params.path) {
        Ok(doc) => match serde_json::to_value(dry_run_patch_document(&doc, &params.patch)) {
            Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
            Err(err) => JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            ),
        },
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_patch_apply_document(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentPatchParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    let mut doc = match read_ocad(&params.path) {
        Ok(doc) => doc,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            );
        }
    };
    let mut history = params.history.unwrap_or_default();
    if let Err(err) =
        apply_patch_with_history(&mut doc, &params.patch, &mut history, "Apply DesignPatch")
    {
        return JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        );
    }
    if let Err(err) = write_ocad(&params.path, &doc) {
        return JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        );
    }
    let state = DocumentHistoryState::new(history);
    match serde_json::to_value(state) {
        Ok(mut value) => {
            if let Some(object) = value.as_object_mut() {
                object.insert("patched".into(), serde_json::json!(params.path));
            }
            JsonRpcResponse::success(request.id.clone(), value)
        }
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_history_undo_document(request: &JsonRpcRequest) -> JsonRpcResponse {
    handle_history_document(request, HistoryDirection::Undo)
}

fn handle_history_redo_document(request: &JsonRpcRequest) -> JsonRpcResponse {
    handle_history_document(request, HistoryDirection::Redo)
}

#[derive(Clone, Copy)]
enum HistoryDirection {
    Undo,
    Redo,
}

fn handle_history_document(
    request: &JsonRpcRequest,
    direction: HistoryDirection,
) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentHistoryParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    let mut doc = match read_ocad(&params.path) {
        Ok(doc) => doc,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            );
        }
    };
    let mut history = params.history;
    let result = match direction {
        HistoryDirection::Undo => history.undo(&mut doc),
        HistoryDirection::Redo => history.redo(&mut doc),
    };
    if let Err(err) = result {
        return JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        );
    }
    if let Err(err) = write_ocad(&params.path, &doc) {
        return JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        );
    }
    let state = DocumentHistoryState::new(history);
    match serde_json::to_value(state) {
        Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_regen_document(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentRegenParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    match regen::regen_document(&params.path, params.sync_topo_refs) {
        Ok(summary) => match serde_json::to_value(RegenResult::from(summary)) {
            Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
            Err(err) => JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            ),
        },
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_regen(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<RegenBodyParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    match regen::regen_body(&params) {
        Ok(summary) => match serde_json::to_value(RegenResult::from(summary)) {
            Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
            Err(err) => JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            ),
        },
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_export(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentExportParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    match export::export_document(&params.path, &params.output) {
        Ok(summary) => match serde_json::to_value(summary) {
            Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
            Err(err) => JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            ),
        },
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_diff_document(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentDiffParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };

    if params.after.is_some() && params.patch.is_some() {
        return JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::invalid_params("provide either 'after' or 'patch', not both"),
        );
    }
    if params.after.is_none() && params.patch.is_none() {
        return JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::invalid_params("provide either 'after' or 'patch'"),
        );
    }

    let before = match read_ocad(&params.before) {
        Ok(doc) => doc,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            );
        }
    };

    let after = if let Some(after_path) = params.after {
        match read_ocad(&after_path) {
            Ok(doc) => doc,
            Err(err) => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::application_error(err.to_string()),
                );
            }
        }
    } else {
        let patch = params.patch.expect("patch checked above");
        let mut after = before.clone();
        if let Err(err) = apply_patch_to_document(&mut after, &patch) {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            );
        }
        after
    };

    match diff::build_document_diff(
        &before,
        &after,
        DiffOptions {
            json: false,
            geometry: params.geometry,
        },
    ) {
        Ok(diff) => match serde_json::to_value(diff) {
            Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
            Err(err) => JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            ),
        },
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_query_document(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentQueryParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    match read_ocad(&params.path) {
        Ok(doc) => {
            let scene = if query_needs_scene(&params.query) {
                match mesh::load_view_data(&params.path) {
                    Ok(data) => Some(scene_query::build_scene_query_context(&data)),
                    Err(err) => {
                        return JsonRpcResponse::error(
                            request.id.clone(),
                            JsonRpcError::application_error(err.to_string()),
                        );
                    }
                }
            } else {
                None
            };
            let query_params = QueryParams {
                parameters: doc.parameters,
                feature_nodes: doc.feature_nodes,
                feature_graph: Some(doc.feature_graph),
                sketches: doc.sketches,
                scene,
                semantic_refs: doc.semantic_refs,
                assembly: doc.assembly,
                drawing: doc.drawing,
                query: params.query,
            };
            match AgentApi.query(query_params) {
                Ok(result) => match serde_json::to_value(result) {
                    Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
                    Err(err) => JsonRpcResponse::error(
                        request.id.clone(),
                        JsonRpcError::application_error(err.to_string()),
                    ),
                },
                Err(err) => JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::application_error(err.to_string()),
                ),
            }
        }
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_pick_document(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentPickParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    let options = PickOptions {
        x: params.x,
        y: params.y,
        width: params.width,
        height: params.height,
    };
    match pick::pick_document(&params.path, &options) {
        Ok(result) => match serde_json::to_value(result) {
            Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
            Err(err) => JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            ),
        },
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_sync_topo_refs_document(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentPathParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    match topo_sync::sync_topo_refs_document(&params.path) {
        Ok(added) => {
            JsonRpcResponse::success(request.id.clone(), serde_json::json!({ "added": added }))
        }
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_assign_face_ref_document(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentAssignFaceRefParams>(request.params.clone())
    {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    match topo_sync::assign_face_ref_document(
        &params.path,
        params.kernel_face_id,
        &params.ref_id,
        &params.created_by,
        &params.role,
        params.normal_m,
    ) {
        Ok(topo_ref) => match serde_json::to_value(topo_ref) {
            Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
            Err(err) => JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::application_error(err.to_string()),
            ),
        },
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

fn handle_explain_document(request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match serde_json::from_value::<DocumentPathParams>(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            );
        }
    };
    match read_ocad(&params.path) {
        Ok(doc) => {
            let explain_params = ExplainParams {
                parameters: doc.parameters,
                feature_nodes: doc.feature_nodes,
                feature_graph: Some(doc.feature_graph),
                sketch_count: Some(doc.sketches.len()),
                document_name: Some(doc.metadata.name.clone()),
            };
            match AgentApi.explain(explain_params) {
                Ok(result) => match serde_json::to_value(result) {
                    Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
                    Err(err) => JsonRpcResponse::error(
                        request.id.clone(),
                        JsonRpcError::application_error(err.to_string()),
                    ),
                },
                Err(err) => JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::application_error(err.to_string()),
                ),
            }
        }
        Err(err) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(err.to_string()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use opencad_ai::DesignPatch;
    use opencad_core::{DocumentId, DocumentMetadata};
    use opencad_feature::bracket_with_hole;
    use opencad_file::{expanded_dir::serialize_document_files, write_expanded_dir, OcadDocument};
    use opencad_graph::bracket_parameters;
    use tempfile::tempdir;

    fn plugin_fixture(path: &Path) {
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:agent-plugin").expect("document"),
            "Agent plugin fixture",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(path, &doc).expect("fixture");
    }

    fn feature_plugin_request(path: &Path, expr: &str) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(100),
            method: "opencad.plugin_invoke".into(),
            params: serde_json::json!({
                "plugin_id": "example.bracket-feature",
                "path": path.to_str().expect("path"),
                "request": {
                    "feature_id": "feature:width_edit",
                    "feature_type": "example.bracket-feature",
                    "parameters": {
                        "parameter_id": "param:width",
                        "expr": expr
                    }
                }
            }),
        }
    }

    #[test]
    fn plugin_list_route_is_sorted_and_serializable() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(99),
            method: "opencad.plugin_list".into(),
            params: serde_json::json!({}),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none());
        let result = response.result.expect("result");
        let ids: Vec<_> = result["plugins"]
            .as_array()
            .expect("plugins")
            .iter()
            .map(|plugin| plugin["id"].as_str().expect("id"))
            .collect();
        assert_eq!(
            ids,
            vec![
                "example.bracket-feature",
                "example.json-exporter",
                "example.patch-importer"
            ]
        );
    }

    #[test]
    fn plugin_invoke_route_applies_patch_with_history() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("agent-plugin.ocad.d");
        plugin_fixture(&doc_path);
        let response = handle_agent_request(&feature_plugin_request(&doc_path, "100 mm"));
        assert!(response.error.is_none());
        assert_eq!(response.result.as_ref().expect("result")["applied"], true);
        assert_eq!(
            read_ocad(&doc_path)
                .expect("after")
                .parameters
                .get("param:width")
                .unwrap()
                .expr,
            "100 mm"
        );
    }

    #[test]
    fn plugin_invoke_failure_leaves_document_unchanged() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("agent-plugin.ocad.d");
        plugin_fixture(&doc_path);
        let before = read_ocad(&doc_path).expect("before");
        let before_files = serialize_document_files(&before).expect("before files");
        let response = handle_agent_request(&feature_plugin_request(&doc_path, "not_a_length"));
        assert!(response.error.is_some());
        let after = read_ocad(&doc_path).expect("after");
        assert_eq!(
            serialize_document_files(&after).expect("after files"),
            before_files
        );
    }

    #[test]
    fn plugin_export_route_returns_bytes_and_metadata() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("agent-plugin.ocad.d");
        plugin_fixture(&doc_path);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(101),
            method: "opencad.plugin_invoke".into(),
            params: serde_json::json!({
                "plugin_id": "example.json-exporter",
                "path": doc_path.to_str().expect("path"),
                "request": { "format": "json" }
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none());
        let result = response.result.expect("result");
        assert_eq!(result["invocation"]["format"], "json");
        assert_eq!(result["invocation"]["media_type"], "application/json");
        assert!(
            result["invocation"]["data"]
                .as_array()
                .expect("bytes")
                .len()
                > 10
        );
    }

    #[test]
    fn inspect_document_via_json_rpc() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(1),
            method: "opencad.inspect".into(),
            params: serde_json::json!({ "path": doc_path.to_str().expect("path") }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none());
        assert_eq!(response.result.expect("result")["features"], 4);
    }

    #[test]
    fn patch_apply_document_via_json_rpc() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(2),
            method: "opencad.patch_apply_document".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "patch": DesignPatch::set_parameter("param:width", "100 mm"),
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none());
        let restored = read_ocad(&doc_path).expect("read");
        let values = opencad_graph::evaluate_param_graph(&restored.parameters).expect("eval");
        assert!((values["width"] - 0.1).abs() < 1e-9);
    }

    #[test]
    fn history_undo_and_redo_document_via_json_rpc() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut before = OcadDocument::from_part_model(metadata, &part);
        before.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &before).expect("write before");

        let apply_request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(2),
            method: "opencad.patch_apply_document".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "patch": DesignPatch::set_parameter("param:width", "100 mm"),
            }),
        };
        let apply_response = handle_agent_request(&apply_request);
        assert!(apply_response.error.is_none());
        let applied_result = apply_response.result.expect("apply result");
        let history: DocumentHistory =
            serde_json::from_value(applied_result["history"].clone()).expect("apply history");
        let after = read_ocad(&doc_path).expect("read after");
        assert_ne!(before, after);

        let undo_request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(3),
            method: "opencad.history_undo_document".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "history": history,
            }),
        };
        let undo_response = handle_agent_request(&undo_request);
        assert!(undo_response.error.is_none());
        assert_eq!(read_ocad(&doc_path).expect("undo document"), before);
        let undone_history: DocumentHistory =
            serde_json::from_value(undo_response.result.expect("undo result")["history"].clone())
                .expect("undo history");

        let redo_request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(4),
            method: "opencad.history_redo_document".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "history": undone_history,
            }),
        };
        let redo_response = handle_agent_request(&redo_request);
        assert!(redo_response.error.is_none());
        assert_eq!(read_ocad(&doc_path).expect("redo document"), after);
    }

    #[test]
    fn regen_document_via_json_rpc() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(3),
            method: "opencad.regen_document".into(),
            params: serde_json::json!({ "path": doc_path.to_str().expect("path") }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none());
        let result = response.result.expect("result");
        assert_eq!(result["regenerated"].as_array().expect("regen").len(), 4);
        assert!(result["volume_m3"].as_f64().expect("volume") > 0.0);
    }

    #[test]
    fn regen_in_memory_via_json_rpc() {
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let doc = OcadDocument::from_part_model(metadata, &part);
        let body = RegenBodyParams {
            parameters: bracket_parameters(),
            sketches: doc.sketches,
            feature_graph: doc.feature_graph,
            feature_nodes: doc.feature_nodes,
        };

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(4),
            method: "opencad.regen".into(),
            params: serde_json::to_value(body).expect("params"),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none());
        let result = response.result.expect("result");
        assert_eq!(result["regenerated"].as_array().expect("regen").len(), 4);
        assert!(result["mass_kg"].as_f64().expect("mass") > 0.0);
    }

    #[test]
    fn export_document_via_json_rpc() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let output = dir.path().join("bracket.stl");
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(5),
            method: "opencad.export".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "output": output.to_str().expect("stl"),
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        assert_eq!(result["format"], "stl");
        assert!(result["triangles"].as_u64().expect("triangles") > 0);
        assert!(output.is_file());
    }

    #[test]
    fn diff_document_with_patch_via_json_rpc() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(6),
            method: "opencad.diff_document".into(),
            params: serde_json::json!({
                "before": doc_path.to_str().expect("path"),
                "patch": DesignPatch::set_parameter("param:width", "100 mm"),
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        let changes = result["changes"].as_array().expect("changes");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0]["kind"], "parameter_changed");
    }

    #[test]
    fn diff_document_two_paths_via_json_rpc() {
        let dir = tempdir().expect("tempdir");
        let before_path = dir.path().join("before.ocad.d");
        let after_path = dir.path().join("after.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut before_doc = OcadDocument::from_part_model(metadata.clone(), &part);
        before_doc.parameters = bracket_parameters();
        write_expanded_dir(&before_path, &before_doc).expect("write before");

        let mut after_doc = before_doc.clone();
        apply_patch_to_document(
            &mut after_doc,
            &DesignPatch::set_parameter("param:width", "100 mm"),
        )
        .expect("patch");
        write_expanded_dir(&after_path, &after_doc).expect("write after");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(7),
            method: "opencad.diff_document".into(),
            params: serde_json::json!({
                "before": before_path.to_str().expect("before"),
                "after": after_path.to_str().expect("after"),
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        let changes = result["changes"].as_array().expect("changes");
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn query_document_via_json_rpc() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(8),
            method: "opencad.query_document".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "query": { "kind": "feature_order" },
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        assert_eq!(result["kind"], "feature_order");
        assert_eq!(result["order"].as_array().expect("order").len(), 4);
    }

    #[test]
    fn query_document_lists_sketch_constraints() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(10),
            method: "opencad.query_document".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "query": { "kind": "list_sketch_constraints", "sketch_id": "sketch:base" },
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        assert_eq!(result["kind"], "sketch_constraints");
        assert_eq!(result["sketch_id"], "sketch:base");
        assert_eq!(result["items"].as_array().expect("items").len(), 6);
    }

    #[test]
    fn query_document_returns_feature_dependency_neighbors() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(11),
            method: "opencad.query_document".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "query": { "kind": "get_feature_dependencies", "id": "feature:hole_mount" },
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        let upstream = result["item"]["upstream"].as_array().expect("upstream");
        assert!(upstream
            .iter()
            .any(|value| value.as_str() == Some("feature:extrude_base")));
    }

    #[test]
    fn query_document_lists_sketch_entities() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(12),
            method: "opencad.query_document".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "query": { "kind": "list_sketch_entities", "sketch_id": "sketch:hole" },
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        assert_eq!(result["kind"], "sketch_entities");
        assert_eq!(result["items"].as_array().expect("items").len(), 2);
    }

    #[test]
    fn pick_document_via_json_rpc() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(13),
            method: "opencad.pick_document".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "x": 256.0,
                "y": 256.0,
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        assert_eq!(result["width"], 512);
        assert!(result["triangle_count"].as_u64().expect("triangles") > 0);
        assert!(result["overlay_line_count"].as_u64().expect("lines") > 0);
        let kind = result["selection"]["kind"].as_str().expect("kind");
        assert!(
            kind == "solid_triangle" || kind == "sketch_line" || kind == "none",
            "unexpected kind: {kind}"
        );
    }

    #[test]
    fn query_document_lists_overlay_lines() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(14),
            method: "opencad.query_document".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "query": { "kind": "list_overlay_lines" },
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        assert_eq!(result["kind"], "overlay_lines");
        let items = result["items"].as_array().expect("items");
        assert!(!items.is_empty());
        assert!(items
            .iter()
            .any(|item| item["entity_id"].as_str().is_some()));
    }

    #[test]
    fn query_document_lists_face_groups() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(15),
            method: "opencad.query_document".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "query": { "kind": "list_face_groups" },
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        assert_eq!(result["kind"], "face_groups");
        let items = result["items"].as_array().expect("items");
        assert!(!items.is_empty());
        assert!(items
            .iter()
            .any(|item| item["face_role"].as_str() == Some("top")));
    }

    #[test]
    fn query_document_lists_assembly_instances() {
        use opencad_assembly::{AssemblyModel, Component, Instance, Placement};
        use opencad_core::{ComponentId, InstanceId};

        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("assembly.ocad.d");
        let doc = OcadDocument {
            metadata: DocumentMetadata::new_assembly(
                DocumentId::new("doc:assembly_test").expect("id"),
                "Test Assembly",
            ),
            parameters: bracket_parameters(),
            sketches: Vec::new(),
            feature_graph: opencad_graph::FeatureGraph::new(),
            feature_nodes: Vec::new(),
            semantic_refs: Vec::new(),
            assertions: Vec::new(),
            assembly: Some(
                AssemblyModel {
                    components: vec![Component::new(
                        ComponentId::new("component:bracket").expect("id"),
                        "parts/bracket.ocad.d",
                        DocumentId::new("doc:bracket_001").expect("id"),
                    )],
                    instances: vec![
                        Instance::new(
                            InstanceId::new("instance:left").expect("id"),
                            ComponentId::new("component:bracket").expect("id"),
                            Placement::identity(),
                            "Left",
                        ),
                        Instance::new(
                            InstanceId::new("instance:right").expect("id"),
                            ComponentId::new("component:bracket").expect("id"),
                            Placement::identity(),
                            "Right",
                        ),
                    ],
                    ..Default::default()
                }
                .sorted_deterministic(),
            ),
            drawing: None,
            attachments: Default::default(),
        };
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(16),
            method: "opencad.query_document".into(),
            params: serde_json::json!({
                "path": doc_path.to_str().expect("path"),
                "query": { "kind": "list_assembly_instances" },
            }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        assert_eq!(result["kind"], "assembly_instances");
        let items = result["items"].as_array().expect("items");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn explain_document_via_json_rpc() {
        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(&doc_path, &doc).expect("write");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(9),
            method: "opencad.explain_document".into(),
            params: serde_json::json!({ "path": doc_path.to_str().expect("path") }),
        };
        let response = handle_agent_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        assert_eq!(result["document_name"], "Bracket with Hole");
        assert_eq!(result["feature_count"], 4);
        assert!(result["summary"]
            .as_str()
            .expect("summary")
            .contains("feature:hole_mount:hole"));
    }
}
