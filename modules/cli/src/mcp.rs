//! `opencad mcp`: Model Context Protocol server on stdio (ADR-014).
//!
//! A thin adapter: every tool delegates to an existing Agent API method or to
//! a file-layer function the CLI already uses, so MCP adds no mutation path.

use std::io::{self, BufRead, Write};
use std::path::Path;

use opencad_ai::{authoring_patch, JsonRpcRequest};
use opencad_core::{DocumentId, DocumentMetadata, OpenCadError, Result};
use opencad_file::{document_design_state, read_ocad, write_ocad, OcadDocument};
use serde_json::{json, Map, Value};

use crate::agent::handle_agent_request_with_plugins;
use crate::plugin::PluginHost;
use crate::review::{generate_review, ReviewArgs};

/// Protocol versions this server can speak, newest first.
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 4] =
    ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const PATCH_SCHEMA_URI: &str = "musubicad://schema/design-patch";
const GUIDE_URI: &str = "musubicad://guide/structural-patch";
const PATCH_SCHEMA: &str = include_str!("../../../schemas/ocad.patch.schema.json");
const GUIDE: &str = include_str!("../../../docs/api/mcp-authoring-guide.md");

const INSTRUCTIONS: &str =
    "MusubiCAD is a parametric CAD whose source of truth is a Design Graph. \
Change designs only with typed DesignPatch operations: call patch_dry_run first, fix every \
reported problem, optionally call review_patch for a visual before/after, then patch_apply. \
Read the resource musubicad://guide/structural-patch before authoring new objects and \
musubicad://schema/design-patch for the exact JSON shape. Lengths are meters, angles radians.";

/// How a tool is executed.
enum Handler {
    /// Forward the arguments unchanged as params of an Agent API method.
    Agent(&'static str),
    Review,
    Authoring,
    NewDocument,
    ImportStep,
}

struct Tool {
    name: &'static str,
    description: &'static str,
    handler: Handler,
    schema: fn() -> Value,
}

fn path_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "path": { "type": "string", "description": "Path to a .ocad file or .ocad.d directory" } },
        "required": ["path"]
    })
}

fn patch_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Path to a .ocad file or .ocad.d directory" },
            "patch": { "type": "object", "description": format!("DesignPatch; see resource {PATCH_SCHEMA_URI}") }
        },
        "required": ["path", "patch"]
    })
}

fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "new_document",
            description: "Create an empty part, assembly, or drawing document at a new path. Refuses to overwrite.",
            handler: Handler::NewDocument,
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "New .ocad.d directory or .ocad file path" },
                    "kind": { "enum": ["part", "assembly", "drawing"] },
                    "document_id": { "type": "string", "description": "Stable ID such as doc:flange" },
                    "name": { "type": "string" }
                },
                "required": ["path", "kind", "document_id", "name"]
            }),
        },
        Tool {
            name: "inspect_document",
            description: "Summarize a document: kind, parameters, features, sketches.",
            handler: Handler::Agent("opencad.inspect"),
            schema: path_schema,
        },
        Tool {
            name: "validate_document",
            description: "Validate a document's schema and checksums.",
            handler: Handler::Agent("opencad.validate"),
            schema: path_schema,
        },
        Tool {
            name: "explain_document",
            description: "Explain a document's design intent in plain language.",
            handler: Handler::Agent("opencad.explain_document"),
            schema: path_schema,
        },
        Tool {
            name: "query_document",
            description: "Run a typed design query (for example list_parameters, list_features, semantic references).",
            handler: Handler::Agent("opencad.query_document"),
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "query": { "type": "object", "description": "DesignQuery, e.g. {\"kind\": \"list_parameters\"}" }
                },
                "required": ["path", "query"]
            }),
        },
        Tool {
            name: "authoring_patch",
            description: "Express an existing document as one structural DesignPatch that rebuilds it from an empty document. Use it as a worked example of patch syntax.",
            handler: Handler::Authoring,
            schema: path_schema,
        },
        Tool {
            name: "patch_dry_run",
            description: "Validate a DesignPatch without writing: returns validation messages, semantic diff, and change impact.",
            handler: Handler::Agent("opencad.patch_dry_run_document"),
            schema: patch_schema,
        },
        Tool {
            name: "review_patch",
            description: "Regenerate before/after geometry for a DesignPatch and write review.html, review.json, and images to output_dir without changing the document.",
            handler: Handler::Review,
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "patch": { "type": "object" },
                    "output_dir": { "type": "string", "description": "Directory for review artifacts" }
                },
                "required": ["path", "patch", "output_dir"]
            }),
        },
        Tool {
            name: "patch_apply",
            description: "Validate and apply a DesignPatch, then write the document. The document is unchanged if validation fails.",
            handler: Handler::Agent("opencad.patch_apply_document"),
            schema: patch_schema,
        },
        Tool {
            name: "regen_document",
            description: "Regenerate geometry and report mass, bounds, references, and assertions.",
            handler: Handler::Agent("opencad.regen_document"),
            schema: path_schema,
        },
        Tool {
            name: "import_step",
            description: "Import a STEP file (millimetres) into a part as a fixed imported solid: new body, or join/cut against target_feature. Writes the document.",
            handler: Handler::ImportStep,
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Part .ocad.d or .ocad" },
                    "step_path": { "type": "string", "description": "STEP file to import" },
                    "feature_id": { "type": "string", "description": "New feature ID, e.g. feature:motor" },
                    "name": { "type": "string" },
                    "operation": { "enum": ["new_body", "join", "cut"] },
                    "target_feature": { "type": "string" },
                    "translation_mm": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }
                },
                "required": ["path", "step_path", "feature_id"]
            }),
        },
        Tool {
            name: "export_document",
            description: "Regenerate and export geometry: .step/.stp (millimetre B-rep for other CAD/CAM tools), .stl (mesh), or .svg (drawing sheet).",
            handler: Handler::Agent("opencad.export"),
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Source .ocad file or .ocad.d directory" },
                    "output": { "type": "string", "description": "Output file; the extension selects the format" }
                },
                "required": ["path", "output"]
            }),
        },
        Tool {
            name: "diff_document",
            description: "Semantic diff between two documents, or between a document and a patch result.",
            handler: Handler::Agent("opencad.diff_document"),
            schema: || json!({
                "type": "object",
                "properties": {
                    "before": { "type": "string" },
                    "after": { "type": "string" },
                    "patch": { "type": "object" },
                    "geometry": { "type": "boolean" }
                },
                "required": ["before"]
            }),
        },
    ]
}

/// Agent API method behind each delegating tool (for parity checks).
#[cfg(test)]
pub fn delegated_agent_methods() -> Vec<(&'static str, &'static str)> {
    tools()
        .into_iter()
        .filter_map(|tool| match tool.handler {
            Handler::Agent(method) => Some((tool.name, method)),
            _ => None,
        })
        .collect()
}

/// Handle one MCP message.  Returns `None` for notifications.
pub fn handle_mcp_message(message: &Value, host: &PluginHost) -> Option<Value> {
    let id = message.get("id").cloned()?;
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let outcome = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({
            "tools": tools().iter().map(|tool| json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": (tool.schema)(),
            })).collect::<Vec<_>>()
        })),
        "tools/call" => call_tool(&params, host),
        "resources/list" => Ok(json!({
            "resources": [
                { "uri": PATCH_SCHEMA_URI, "name": "DesignPatch JSON Schema",
                  "mimeType": "application/schema+json" },
                { "uri": GUIDE_URI, "name": "Structural patch authoring guide",
                  "mimeType": "text/markdown" }
            ]
        })),
        "resources/read" => read_resource(&params),
        other => Err((-32601, format!("method not found: {other}"))),
    };
    Some(match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
        }
    })
}

fn initialize(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let version = SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .find(|version| **version == requested)
        .copied()
        .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false }, "resources": { "listChanged": false } },
        "serverInfo": { "name": "musubicad", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

fn read_resource(params: &Value) -> std::result::Result<Value, (i32, String)> {
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (mime, text) = match uri {
        PATCH_SCHEMA_URI => ("application/schema+json", PATCH_SCHEMA),
        GUIDE_URI => ("text/markdown", GUIDE),
        other => return Err((-32602, format!("unknown resource: {other}"))),
    };
    Ok(json!({ "contents": [{ "uri": uri, "mimeType": mime, "text": text }] }))
}

fn call_tool(params: &Value, host: &PluginHost) -> std::result::Result<Value, (i32, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    let tool = tools()
        .into_iter()
        .find(|tool| tool.name == name)
        .ok_or_else(|| (-32602, format!("unknown tool: {name}")))?;
    let result = match tool.handler {
        Handler::Agent(method) => {
            let request = JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: json!(1),
                method: method.into(),
                params: arguments,
            };
            let response = handle_agent_request_with_plugins(&request, host);
            match (response.result, response.error) {
                (Some(result), _) => Ok(result),
                (None, Some(error)) if error.code == -32602 => {
                    return Err((-32602, error.message));
                }
                (None, Some(error)) => Err(error.message),
                (None, None) => Err("empty response".to_string()),
            }
        }
        Handler::Review => review(&arguments).map_err(|error| error.to_string()),
        Handler::Authoring => authoring(&arguments).map_err(|error| error.to_string()),
        Handler::NewDocument => new_document(&arguments).map_err(|error| error.to_string()),
        Handler::ImportStep => {
            serde_json::from_value::<crate::import::ImportStepRequest>(arguments)
                .map_err(|error| format!("invalid arguments: {error}"))
                .and_then(|request| {
                    crate::import::import_step(&request)
                        .and_then(|summary| Ok(serde_json::to_value(summary)?))
                        .map_err(|error| error.to_string())
                })
        }
    };
    Ok(tool_result(result))
}

/// Wrap a tool outcome: structured JSON plus a text copy, or `isError`.
fn tool_result(result: std::result::Result<Value, String>) -> Value {
    match result {
        Ok(value) => {
            let text = serde_json::to_string_pretty(&value).unwrap_or_default();
            let structured = if value.is_object() {
                value
            } else {
                json!({ "result": value })
            };
            json!({
                "content": [{ "type": "text", "text": text }],
                "structuredContent": structured,
                "isError": false
            })
        }
        Err(message) => json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true
        }),
    }
}

fn string_arg<'a>(arguments: &'a Value, key: &str) -> Result<&'a str> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| OpenCadError::validation(format!("missing string argument '{key}'")))
}

fn authoring(arguments: &Value) -> Result<Value> {
    let doc = read_ocad(string_arg(arguments, "path")?)?;
    let patch = authoring_patch(&document_design_state(&doc))?;
    Ok(json!({ "patch": serde_json::to_value(patch)? }))
}

fn new_document(arguments: &Value) -> Result<Value> {
    let path = string_arg(arguments, "path")?;
    if Path::new(path).exists() {
        return Err(OpenCadError::validation(format!(
            "'{path}' already exists; new_document never overwrites"
        )));
    }
    let id = DocumentId::new(string_arg(arguments, "document_id")?)?;
    let name = string_arg(arguments, "name")?;
    let metadata = match string_arg(arguments, "kind")? {
        "part" => DocumentMetadata::new(id, name),
        "assembly" => DocumentMetadata::new_assembly(id, name),
        "drawing" => DocumentMetadata::new_drawing(id, name),
        other => {
            return Err(OpenCadError::validation(format!(
                "unknown document kind '{other}'; expected part, assembly, or drawing"
            )))
        }
    };
    write_ocad(path, &OcadDocument::new(metadata))?;
    Ok(json!({ "created": path }))
}

fn review(arguments: &Value) -> Result<Value> {
    let path = string_arg(arguments, "path")?;
    let output_dir = string_arg(arguments, "output_dir")?;
    let patch = arguments
        .get("patch")
        .ok_or_else(|| OpenCadError::validation("missing argument 'patch'"))?;
    // Validate the patch shape before touching the file system.
    let patch: opencad_ai::DesignPatch = serde_json::from_value(patch.clone())
        .map_err(|error| OpenCadError::validation(format!("invalid patch: {error}")))?;
    std::fs::create_dir_all(output_dir)
        .map_err(|error| OpenCadError::Other(format!("cannot create '{output_dir}': {error}")))?;
    let patch_path = Path::new(output_dir).join("patch.json");
    std::fs::write(&patch_path, serde_json::to_vec_pretty(&patch)?)
        .map_err(|error| OpenCadError::Other(format!("cannot write patch: {error}")))?;
    let artifact = generate_review(&ReviewArgs {
        document_path: path.to_string(),
        patch_path: patch_path.to_string_lossy().to_string(),
        output_dir: output_dir.to_string(),
    })?;
    Ok(json!({
        "review_html": Path::new(output_dir).join("review.html").to_string_lossy(),
        "review": serde_json::to_value(&artifact)?,
    }))
}

/// Serve MCP over stdio until end of input.
pub fn serve_stdio() -> Result<()> {
    let host = PluginHost::with_builtins()?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line =
            line.map_err(|error| OpenCadError::Other(format!("failed to read stdin: {error}")))?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle_mcp_message(&message, &host),
            Err(error) => Some(json!({
                "jsonrpc": "2.0", "id": null,
                "error": { "code": -32700, "message": format!("parse error: {error}") }
            })),
        };
        if let Some(response) = response {
            writeln!(stdout, "{response}")
                .and_then(|()| stdout.flush())
                .map_err(|error| OpenCadError::Other(format!("failed to write stdout: {error}")))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> PluginHost {
        PluginHost::with_builtins().expect("plugin host")
    }

    fn call(method: &str, params: Value) -> Value {
        handle_mcp_message(
            &json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params }),
            &host(),
        )
        .expect("response")
    }

    fn bracket() -> String {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/bracket.ocad.d").to_string()
    }

    #[test]
    fn initialize_negotiates_supported_versions() {
        let known = call("initialize", json!({ "protocolVersion": "2025-06-18" }));
        assert_eq!(known["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(known["result"]["serverInfo"]["name"], "musubicad");
        let unknown = call("initialize", json!({ "protocolVersion": "1999-01-01" }));
        assert_eq!(
            unknown["result"]["protocolVersion"],
            SUPPORTED_PROTOCOL_VERSIONS[0]
        );
    }

    #[test]
    fn notifications_get_no_response_and_unknown_methods_error() {
        let notification = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert!(handle_mcp_message(&notification, &host()).is_none());
        assert_eq!(call("nope", json!({}))["error"]["code"], -32601);
        assert_eq!(
            call("tools/call", json!({ "name": "nope", "arguments": {} }))["error"]["code"],
            -32602
        );
    }

    #[test]
    fn every_tool_is_listed_with_an_object_schema() {
        let listed = call("tools/list", json!({}));
        let tools = listed["result"]["tools"].as_array().expect("tools");
        assert_eq!(tools.len(), 13);
        for tool in tools {
            assert_eq!(tool["inputSchema"]["type"], "object", "{}", tool["name"]);
            assert!(tool["description"]
                .as_str()
                .is_some_and(|text| !text.is_empty()));
        }
    }

    #[test]
    fn delegated_tools_map_to_real_agent_methods() {
        for (tool, method) in delegated_agent_methods() {
            let request = JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: json!(1),
                method: method.into(),
                params: json!({}),
            };
            let response = handle_agent_request_with_plugins(&request, &host());
            let error = response.error.map(|error| error.code);
            assert_ne!(
                error,
                Some(-32601),
                "{tool} -> {method} is not an Agent method"
            );
        }
    }

    #[test]
    fn dry_run_returns_structured_results_and_readable_failures() {
        let ok = call(
            "tools/call",
            json!({ "name": "patch_dry_run", "arguments": {
                "path": bracket(),
                "patch": { "operations": [
                    { "type": "add_parameter", "id": "param:rib", "name": "rib", "expr": "4 mm" }
                ] }
            } }),
        );
        assert_eq!(ok["result"]["isError"], false);
        assert!(ok["result"]["structuredContent"]["validation"].is_object());

        let failed = call(
            "tools/call",
            json!({ "name": "patch_dry_run", "arguments": {
                "path": bracket(),
                "patch": { "operations": [ { "type": "remove_parameter", "id": "param:thickness" } ] }
            } }),
        );
        let text = failed["result"].to_string();
        assert!(text.contains("cannot remove parameter"), "{text}");
    }

    #[test]
    fn resources_serve_the_schema_and_guide() {
        let listed = call("resources/list", json!({}));
        assert_eq!(listed["result"]["resources"].as_array().unwrap().len(), 2);
        let schema = call("resources/read", json!({ "uri": PATCH_SCHEMA_URI }));
        let text = schema["result"]["contents"][0]["text"].as_str().unwrap();
        serde_json::from_str::<Value>(text).expect("schema is JSON");
        let guide = call("resources/read", json!({ "uri": GUIDE_URI }));
        assert!(guide["result"]["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("add_feature"));
        assert_eq!(
            call("resources/read", json!({ "uri": "musubicad://nope" }))["error"]["code"],
            -32602
        );
    }

    #[test]
    fn new_document_refuses_to_overwrite() {
        let result = call(
            "tools/call",
            json!({ "name": "new_document", "arguments": {
                "path": bracket(), "kind": "part", "document_id": "doc:x", "name": "X"
            } }),
        );
        assert_eq!(result["result"]["isError"], true);
        assert!(result["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("never overwrites"));
    }
}
