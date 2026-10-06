//! Host-owned plugin discovery and invocation for CLI/Agent surfaces.
//!
//! Filesystem reads/writes and document validation remain here in the host.
//! `opencad-plugin-api` only receives in-memory serializable DTOs. The built-in
//! plugins make the linked/static contract executable without dynamic loading.

use std::fs;

use opencad_ai::{ensure_patch_valid, DesignPatch};
use opencad_core::{OpenCadError, Result};
use opencad_file::{
    apply_patch_with_history, dry_run_patch_document, read_ocad, write_ocad, DocumentHistory,
    DocumentHistoryState,
};
use opencad_plugin_api::{
    ExportRequest, ExportResult, ExporterPlugin, FeatureRequest, FeatureResult, ImportRequest,
    ImportResult, ImporterPlugin, PluginCapability, PluginDiagnostic, PluginKind, PluginManifest,
    PluginRegistry,
};
use opencad_plugin_example::BracketFeaturePlugin;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const IMPORTER_PLUGIN_ID: &str = "example.patch-importer";
const EXPORTER_PLUGIN_ID: &str = "example.json-exporter";

/// Host-owned in-process plugin service. It has no global state.
pub struct PluginHost {
    registry: PluginRegistry,
}

impl PluginHost {
    #[allow(dead_code)]
    pub fn from_registry(registry: PluginRegistry) -> Self {
        Self { registry }
    }

    /// Build the deterministic local example registry used by CLI and Agent.
    pub fn with_builtins() -> Result<Self> {
        let mut registry = PluginRegistry::new();
        registry.register_feature(BracketFeaturePlugin::default())?;
        registry.register_importer(BuiltinImporter::default())?;
        registry.register_exporter(BuiltinExporter::default())?;
        Ok(Self { registry })
    }

    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    pub fn list(&self) -> Vec<PluginManifest> {
        self.registry.list()
    }

    pub fn invoke(&self, id: &str, request: PluginRequest) -> Result<PluginInvocationResult> {
        let manifest = self.registry.manifest(id)?;
        match (manifest.kind, request) {
            (PluginKind::Feature, PluginRequest::Feature(request)) => {
                let result = self.registry.invoke_feature(id, request)?;
                Ok(PluginInvocationResult::from_feature(id, manifest, result))
            }
            (PluginKind::Importer, PluginRequest::Importer(request)) => {
                let result = self.registry.invoke_importer(id, request)?;
                Ok(PluginInvocationResult::from_importer(id, manifest, result))
            }
            (PluginKind::Exporter, PluginRequest::Exporter(request)) => {
                let result = self.registry.invoke_exporter(id, request)?;
                Ok(PluginInvocationResult::from_exporter(id, manifest, result))
            }
            (kind, _) => Err(OpenCadError::validation(format!(
                "plugin '{id}' requires a {kind:?} request"
            ))),
        }
    }
}

/// In-memory request selected from a plugin manifest kind.
pub enum PluginRequest {
    Feature(FeatureRequest),
    Importer(ImportRequest),
    Exporter(ExportRequest),
}

/// Serializable result returned by a linked plugin invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginInvocationResult {
    pub plugin_id: String,
    pub kind: PluginKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<DesignPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default)]
    pub diagnostics: Vec<PluginDiagnostic>,
}

impl PluginInvocationResult {
    fn from_feature(id: &str, manifest: PluginManifest, result: FeatureResult) -> Self {
        Self {
            plugin_id: id.into(),
            kind: manifest.kind,
            patch: Some(result.patch),
            data: None,
            format: None,
            media_type: None,
            diagnostics: result.diagnostics,
        }
    }

    fn from_importer(id: &str, manifest: PluginManifest, result: ImportResult) -> Self {
        Self {
            plugin_id: id.into(),
            kind: manifest.kind,
            patch: Some(result.patch),
            data: None,
            format: None,
            media_type: None,
            diagnostics: result.diagnostics,
        }
    }

    fn from_exporter(id: &str, manifest: PluginManifest, result: ExportResult) -> Self {
        Self {
            plugin_id: id.into(),
            kind: manifest.kind,
            patch: None,
            data: Some(result.data),
            format: Some(result.format),
            media_type: Some(result.media_type),
            diagnostics: result.diagnostics,
        }
    }
}

/// Host-side result after patch validation, transaction/history, or output
/// persistence has completed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginDocumentResult {
    pub invocation: PluginInvocationResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<opencad_ai::PatchDryRunReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<DocumentHistoryState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    pub dry_run: bool,
    pub applied: bool,
}

/// Decode a serialized request after the host has validated the manifest kind.
pub fn decode_request(kind: PluginKind, value: Value) -> Result<PluginRequest> {
    match kind {
        PluginKind::Feature => serde_json::from_value(value)
            .map(PluginRequest::Feature)
            .map_err(|err| OpenCadError::validation(format!("invalid feature request: {err}"))),
        PluginKind::Importer => serde_json::from_value(value)
            .map(PluginRequest::Importer)
            .map_err(|err| OpenCadError::validation(format!("invalid importer request: {err}"))),
        PluginKind::Exporter => serde_json::from_value(value)
            .map(PluginRequest::Exporter)
            .map_err(|err| OpenCadError::validation(format!("invalid exporter request: {err}"))),
    }
}

/// Parsed `musubicad plugin` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginCliCommand {
    List {
        json: bool,
    },
    Invoke {
        plugin_id: String,
        doc_path: String,
        request_path: String,
        dry_run: bool,
        output: Option<String>,
        json: bool,
    },
}

/// Parse host-owned plugin CLI arguments without invoking plugin code.
pub fn parse_cli_args(args: &[String]) -> Result<PluginCliCommand> {
    let command = args.first().map(String::as_str).ok_or_else(|| {
        OpenCadError::validation(
            "usage: musubicad plugin list [--json] | plugin invoke <id> <document> <request.json> [--dry-run] [--output <path>] [--json]",
        )
    })?;
    match command {
        "list" => {
            let json = args.iter().skip(1).all(|arg| arg == "--json");
            if !json && args.len() > 1 {
                return Err(OpenCadError::validation(
                    "usage: musubicad plugin list [--json]",
                ));
            }
            Ok(PluginCliCommand::List { json })
        }
        "invoke" => {
            let mut positional = Vec::new();
            let mut dry_run = false;
            let mut json = false;
            let mut output = None;
            let mut index = 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--dry-run" => dry_run = true,
                    "--json" => json = true,
                    "--output" => {
                        let path = args.get(index + 1).ok_or_else(|| {
                            OpenCadError::validation("--output requires a path")
                        })?;
                        output = Some(path.clone());
                        index += 1;
                    }
                    value => positional.push(value.to_string()),
                }
                index += 1;
            }
            if positional.len() != 3 {
                return Err(OpenCadError::validation(
                    "usage: musubicad plugin invoke <id> <document> <request.json> [--dry-run] [--output <path>] [--json]",
                ));
            }
            Ok(PluginCliCommand::Invoke {
                plugin_id: positional.remove(0),
                doc_path: positional.remove(0),
                request_path: positional.remove(0),
                dry_run,
                output,
                json,
            })
        }
        _ => Err(OpenCadError::validation(
            "usage: musubicad plugin list [--json] | plugin invoke <id> <document> <request.json> [--dry-run] [--output <path>] [--json]",
        )),
    }
}

pub fn read_request_file(path: &str) -> Result<Value> {
    let text = fs::read_to_string(path).map_err(|err| {
        OpenCadError::Other(format!("failed to read plugin request '{path}': {err}"))
    })?;
    serde_json::from_str(&text)
        .map_err(|err| OpenCadError::validation(format!("invalid plugin request JSON: {err}")))
}

/// Invoke a plugin against a host-owned document path.
pub fn invoke_plugin_on_path(
    host: &PluginHost,
    plugin_id: &str,
    path: &str,
    request: Value,
    dry_run: bool,
    output: Option<&str>,
    history: Option<DocumentHistory>,
) -> Result<PluginDocumentResult> {
    let before = read_ocad(path)?;
    let manifest = host.registry().manifest(plugin_id)?;
    let mut request = decode_request(manifest.kind, request)?;
    if let PluginRequest::Exporter(export_request) = &mut request {
        export_request.state = serde_json::to_value(&before)?;
    }

    let invocation = host.invoke(plugin_id, request)?;
    let mut result = PluginDocumentResult {
        invocation,
        validation: None,
        history: None,
        output_path: None,
        dry_run,
        applied: false,
    };

    if let Some(patch) = result.invocation.patch.as_ref() {
        if output.is_some() {
            return Err(OpenCadError::validation(
                "patch plugin invocation cannot use an output path",
            ));
        }
        let report = dry_run_patch_document(&before, patch);
        ensure_patch_valid(&report)?;
        result.validation = Some(report);
        if !dry_run {
            let mut candidate = before;
            let mut history = history.unwrap_or_default();
            apply_patch_with_history(
                &mut candidate,
                patch,
                &mut history,
                format!("Plugin {plugin_id}"),
            )?;
            write_ocad(path, &candidate)?;
            result.history = Some(DocumentHistoryState::new(history));
            result.applied = true;
        }
    } else if let Some(data) = result.invocation.data.as_ref() {
        if let Some(output) = output {
            fs::write(output, data).map_err(|err| {
                OpenCadError::Other(format!("failed to write plugin output '{output}': {err}"))
            })?;
            result.output_path = Some(output.to_string());
        }
    }

    Ok(result)
}

struct BuiltinImporter {
    manifest: PluginManifest,
}

impl Default for BuiltinImporter {
    fn default() -> Self {
        Self {
            manifest: PluginManifest::new(
                IMPORTER_PLUGIN_ID,
                "DesignPatch JSON Importer Example",
                "0.1.0",
                PluginKind::Importer,
            )
            .with_capability(PluginCapability::ImportPatch),
        }
    }
}

impl ImporterPlugin for BuiltinImporter {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn import(&self, request: ImportRequest) -> Result<ImportResult> {
        if request.format != "designpatch-json" && request.format != "application/json" {
            return Err(OpenCadError::validation(format!(
                "unsupported importer format '{}'",
                request.format
            )));
        }
        let patch = serde_json::from_slice(&request.data).map_err(|err| {
            OpenCadError::validation(format!("invalid DesignPatch importer bytes: {err}"))
        })?;
        Ok(ImportResult::new(patch))
    }
}

struct BuiltinExporter {
    manifest: PluginManifest,
}

impl Default for BuiltinExporter {
    fn default() -> Self {
        Self {
            manifest: PluginManifest::new(
                EXPORTER_PLUGIN_ID,
                "Design State JSON Exporter Example",
                "0.1.0",
                PluginKind::Exporter,
            )
            .with_capability(PluginCapability::ExportBytes),
        }
    }
}

impl ExporterPlugin for BuiltinExporter {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn export(&self, request: ExportRequest) -> Result<ExportResult> {
        let data = serde_json::to_vec(&request.state)?;
        Ok(ExportResult {
            format: request.format,
            media_type: "application/json".into(),
            data,
            diagnostics: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{DocumentId, DocumentMetadata};
    use opencad_feature::bracket_with_hole;
    use opencad_file::{expanded_dir::serialize_document_files, write_expanded_dir, OcadDocument};
    use opencad_graph::bracket_parameters;
    use opencad_plugin_example::PLUGIN_ID as FEATURE_PLUGIN_ID;
    use tempfile::tempdir;

    fn fixture(path: &std::path::Path) {
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:plugin").expect("document"),
            "Plugin fixture",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        write_expanded_dir(path, &doc).expect("fixture");
    }

    fn feature_request(expr: &str) -> Value {
        serde_json::json!({
            "feature_id": "feature:width_edit",
            "feature_type": "example.bracket-feature",
            "parameters": {
                "parameter_id": "param:width",
                "expr": expr
            }
        })
    }

    #[test]
    fn builtins_list_in_stable_order_and_feature_applies_with_history() {
        let host = PluginHost::with_builtins().expect("builtins");
        let ids: Vec<_> = host
            .list()
            .into_iter()
            .map(|manifest| manifest.id)
            .collect();
        assert_eq!(
            ids,
            vec![
                "example.bracket-feature",
                "example.json-exporter",
                "example.patch-importer"
            ]
        );

        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("plugin.ocad.d");
        fixture(&path);
        let result = invoke_plugin_on_path(
            &host,
            FEATURE_PLUGIN_ID,
            path.to_str().expect("path"),
            feature_request("100 mm"),
            false,
            None,
            None,
        )
        .expect("invoke");
        assert!(result.applied);
        assert!(result.history.expect("history").can_undo);
        let after = read_ocad(path.to_str().expect("path")).expect("after");
        assert_eq!(after.parameters.get("param:width").unwrap().expr, "100 mm");
    }

    #[test]
    fn invalid_plugin_patch_is_atomic_and_does_not_write_document() {
        let host = PluginHost::with_builtins().expect("builtins");
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("plugin.ocad.d");
        fixture(&path);
        let before = read_ocad(path.to_str().expect("path")).expect("before");
        let before_files = serialize_document_files(&before).expect("before files");
        let error = invoke_plugin_on_path(
            &host,
            FEATURE_PLUGIN_ID,
            path.to_str().expect("path"),
            feature_request("not_a_length"),
            false,
            None,
            None,
        )
        .expect_err("invalid plugin patch");
        assert!(error.to_string().contains("not_a_length"));
        let after = read_ocad(path.to_str().expect("path")).expect("after");
        assert_eq!(
            serialize_document_files(&after).expect("after files"),
            before_files
        );
    }

    #[test]
    fn plugin_returned_error_is_isolated_from_document_persistence() {
        let host = PluginHost::with_builtins().expect("builtins");
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("plugin.ocad.d");
        fixture(&path);
        let before = read_ocad(path.to_str().expect("path")).expect("before");
        let before_files = serialize_document_files(&before).expect("before files");
        let error = invoke_plugin_on_path(
            &host,
            FEATURE_PLUGIN_ID,
            path.to_str().expect("path"),
            serde_json::json!({
                "feature_id": "feature:width_edit",
                "feature_type": FEATURE_PLUGIN_ID,
                "parameters": { "parameter_id": "param:width" }
            }),
            false,
            None,
            None,
        )
        .expect_err("plugin request error");
        assert!(error.to_string().contains("'expr'"));
        let after = read_ocad(path.to_str().expect("path")).expect("after");
        assert_eq!(
            serialize_document_files(&after).expect("after files"),
            before_files
        );
    }

    #[test]
    fn exporter_returns_host_persistable_bytes_without_mutating_document() {
        let host = PluginHost::with_builtins().expect("builtins");
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("plugin.ocad.d");
        fixture(&path);
        let result = invoke_plugin_on_path(
            &host,
            EXPORTER_PLUGIN_ID,
            path.to_str().expect("path"),
            serde_json::json!({ "format": "json" }),
            false,
            None,
            None,
        )
        .expect("export");
        assert!(result.invocation.data.unwrap_or_default().len() > 10);
        assert!(!result.applied);
    }

    #[test]
    fn importer_and_exporter_outputs_match_checked_in_golden_files() {
        let host = PluginHost::with_builtins().expect("builtins");
        let patch = DesignPatch::set_parameter("param:width", "100 mm");
        let imported = host
            .invoke(
                IMPORTER_PLUGIN_ID,
                PluginRequest::Importer(ImportRequest {
                    format: "designpatch-json".into(),
                    source_name: Some("width.patch.json".into()),
                    data: serde_json::to_vec(&patch).expect("patch JSON"),
                }),
            )
            .expect("import");
        assert_eq!(
            serde_json::to_vec(&imported).expect("import result JSON"),
            include_str!("../tests/golden/plugin-import-result.json")
                .trim_ascii_end()
                .as_bytes()
        );

        let exported = host
            .invoke(
                EXPORTER_PLUGIN_ID,
                PluginRequest::Exporter(ExportRequest {
                    format: "json".into(),
                    state: serde_json::json!({
                        "parameters": {"param:width": "100 mm"}
                    }),
                }),
            )
            .expect("export");
        assert_eq!(
            serde_json::to_vec(&exported).expect("export result JSON"),
            include_str!("../tests/golden/plugin-export-result.json")
                .trim_ascii_end()
                .as_bytes()
        );
    }

    #[test]
    fn cli_plugin_routes_parse_list_and_invoke_options() {
        assert_eq!(
            parse_cli_args(&["list".into(), "--json".into()]).expect("list args"),
            PluginCliCommand::List { json: true }
        );
        assert_eq!(
            parse_cli_args(&[
                "invoke".into(),
                FEATURE_PLUGIN_ID.into(),
                "bracket.ocad.d".into(),
                "request.json".into(),
                "--dry-run".into(),
                "--output".into(),
                "out.json".into(),
                "--json".into(),
            ])
            .expect("invoke args"),
            PluginCliCommand::Invoke {
                plugin_id: FEATURE_PLUGIN_ID.into(),
                doc_path: "bracket.ocad.d".into(),
                request_path: "request.json".into(),
                dry_run: true,
                output: Some("out.json".into()),
                json: true,
            }
        );
    }
}
