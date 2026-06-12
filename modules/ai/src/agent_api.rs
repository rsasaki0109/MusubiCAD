//! JSON-RPC Agent API (Task-156+).

use opencad_core::{OpenCadError, Result};
use opencad_feature::FeatureNode;
use opencad_geometry::TopoRef;
use opencad_graph::{evaluate_param_graph, DesignDiff, ParamGraph};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::explain::{explain_design, DesignExplanation, ExplainParams};
use crate::patch::DesignPatch;
use crate::query::{run_query, QueryParams, QueryResult};
use crate::validation::{dry_run_patch_state, PatchDryRunReport};
use crate::state::{DesignState, diff_design_state};

/// JSON-RPC 2.0 request.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self {
            code: -32700,
            message: message.into(),
            data: None,
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: -32600,
            message: message.into(),
            data: None,
        }
    }

    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("method not found: {method}"),
            data: None,
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    pub fn application_error(message: impl Into<String>) -> Self {
        Self {
            code: -32000,
            message: message.into(),
            data: None,
        }
    }
}

/// Parsed `opencad.patch_dry_run` parameters.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PatchDryRunParams {
    pub parameters: ParamGraph,
    pub feature_nodes: Vec<FeatureNode>,
    #[serde(default)]
    pub semantic_refs: Vec<TopoRef>,
    pub patch: DesignPatch,
}

/// Parsed `opencad.patch_apply` parameters.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PatchApplyParams {
    pub parameters: ParamGraph,
    pub feature_nodes: Vec<FeatureNode>,
    #[serde(default)]
    pub semantic_refs: Vec<TopoRef>,
    pub patch: DesignPatch,
}

/// Result of applying a patch in memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchApplyResult {
    pub parameters: ParamGraph,
    pub feature_nodes: Vec<FeatureNode>,
    pub semantic_refs: Vec<TopoRef>,
    pub diff: DesignDiff,
}

/// Parsed `opencad.diff` parameters.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DiffParams {
    pub before: DesignStateSnapshot,
    pub after: DesignStateSnapshot,
}

/// Serializable design state snapshot for RPC transport.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DesignStateSnapshot {
    pub parameters: ParamGraph,
    pub feature_nodes: Vec<FeatureNode>,
    #[serde(default)]
    pub semantic_refs: Vec<TopoRef>,
}

impl From<DesignState> for DesignStateSnapshot {
    fn from(state: DesignState) -> Self {
        Self {
            parameters: state.parameters,
            feature_nodes: state.feature_nodes,
            semantic_refs: state.semantic_refs,
        }
    }
}

impl From<DesignStateSnapshot> for DesignState {
    fn from(snapshot: DesignStateSnapshot) -> Self {
        DesignState::with_semantic_refs(
            snapshot.parameters,
            snapshot.feature_nodes,
            snapshot.semantic_refs,
        )
    }
}

/// Agent API dispatcher (in-memory, no file I/O).
#[derive(Debug, Default)]
pub struct AgentApi;

impl AgentApi {
    pub fn handle_request(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        if request.jsonrpc != "2.0" {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_request("jsonrpc must be '2.0'"),
            );
        }

        match request.method.as_str() {
            "opencad.patch_dry_run" => self.handle_patch_dry_run(request),
            "opencad.patch_apply" => self.handle_patch_apply(request),
            "opencad.diff" => self.handle_diff(request),
            "opencad.query" => self.handle_query(request),
            "opencad.explain" => self.handle_explain(request),
            method => JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::method_not_found(method),
            ),
        }
    }

    pub fn patch_dry_run(&self, params: PatchDryRunParams) -> PatchDryRunReport {
        let state = DesignState::with_semantic_refs(
            params.parameters,
            params.feature_nodes,
            params.semantic_refs,
        );
        dry_run_patch_state(&state, &params.patch)
    }

    pub fn patch_apply(&self, params: PatchApplyParams) -> Result<PatchApplyResult> {
        let before = DesignState::with_semantic_refs(
            params.parameters,
            params.feature_nodes,
            params.semantic_refs,
        );
        let mut after = before.clone();
        params.patch.apply_to_document(
            &mut after.parameters,
            &mut after.feature_nodes,
            &mut after.semantic_refs,
        )?;
        evaluate_param_graph(&after.parameters)?;
        let diff = diff_design_state(&before, &after);
        Ok(PatchApplyResult {
            parameters: after.parameters,
            feature_nodes: after.feature_nodes,
            semantic_refs: after.semantic_refs,
            diff,
        })
    }

    pub fn diff(&self, params: DiffParams) -> DesignDiff {
        diff_design_state(&params.before.into(), &params.after.into())
    }

    pub fn query(&self, params: QueryParams) -> Result<QueryResult> {
        run_query(&params)
    }

    pub fn explain(&self, params: ExplainParams) -> Result<DesignExplanation> {
        explain_design(&params)
    }

    fn handle_patch_dry_run(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        match serde_json::from_value::<PatchDryRunParams>(request.params.clone()) {
            Ok(params) => match serde_json::to_value(self.patch_dry_run(params)) {
                Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
                Err(err) => JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::application_error(err.to_string()),
                ),
            },
            Err(err) => JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            ),
        }
    }

    fn handle_patch_apply(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match serde_json::from_value::<PatchApplyParams>(request.params.clone()) {
            Ok(params) => params,
            Err(err) => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::invalid_params(err.to_string()),
                );
            }
        };
        match self.patch_apply(params) {
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

    fn handle_diff(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        match serde_json::from_value::<DiffParams>(request.params.clone()) {
            Ok(params) => match serde_json::to_value(self.diff(params)) {
                Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
                Err(err) => JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::application_error(err.to_string()),
                ),
            },
            Err(err) => JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(err.to_string()),
            ),
        }
    }

    fn handle_query(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match serde_json::from_value::<QueryParams>(request.params.clone()) {
            Ok(params) => params,
            Err(err) => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::invalid_params(err.to_string()),
                );
            }
        };
        match self.query(params) {
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

    fn handle_explain(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match serde_json::from_value::<ExplainParams>(request.params.clone()) {
            Ok(params) => params,
            Err(err) => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::invalid_params(err.to_string()),
                );
            }
        };
        match self.explain(params) {
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
}

impl JsonRpcResponse {
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Value, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// Parse and dispatch one JSON-RPC request line.
pub fn handle_json_line(line: &str) -> Result<String> {
    let request: JsonRpcRequest = serde_json::from_str(line)
        .map_err(|err| OpenCadError::validation(format!("invalid JSON-RPC request: {err}")))?;
    let api = AgentApi;
    let response = api.handle_request(&request);
    serde_json::to_string(&response)
        .map_err(|err| OpenCadError::Other(format!("failed to encode JSON-RPC response: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FeatureExprField;
    use opencad_feature::bracket_with_hole;
    use opencad_graph::bracket_parameters;

    fn bracket_state() -> DesignState {
        let part = bracket_with_hole().expect("model");
        DesignState::new(
            bracket_parameters(),
            part.nodes.into_values().collect(),
        )
    }

    fn bracket_part() -> opencad_feature::PartModel {
        bracket_with_hole().expect("model")
    }

    #[test]
    fn patch_dry_run_via_json_rpc() {
        let state = bracket_state();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(1),
            method: "opencad.patch_dry_run".into(),
            params: serde_json::json!({
                "parameters": state.parameters,
                "feature_nodes": state.feature_nodes,
                "patch": DesignPatch::set_parameter("param:width", "100 mm"),
            }),
        };
        let response = AgentApi.handle_request(&request);
        assert!(response.error.is_none());
        let result = response.result.expect("result");
        assert_eq!(result["diff"]["summary"], "param:width: 80 mm -> 100 mm");
    }

    #[test]
    fn patch_apply_via_json_rpc() {
        let state = bracket_state();
        let patch = DesignPatch::set_feature_expr(
            "feature:extrude_base",
            FeatureExprField::LengthExpr,
            "thickness * 2",
        );
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(2),
            method: "opencad.patch_apply".into(),
            params: serde_json::json!({
                "parameters": state.parameters,
                "feature_nodes": state.feature_nodes,
                "patch": patch,
            }),
        };
        let response = AgentApi.handle_request(&request);
        assert!(response.error.is_none());
        let result = response.result.expect("result");
        assert!(result["feature_nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .any(|node| node["id"] == "feature:extrude_base"));
    }

    #[test]
    fn diff_via_json_rpc() {
        let before = bracket_state();
        let mut after = before.clone();
        after
            .parameters
            .set_expr("param:width", "100 mm")
            .expect("set");
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(3),
            method: "opencad.diff".into(),
            params: serde_json::json!({
                "before": DesignStateSnapshot::from(before),
                "after": DesignStateSnapshot::from(after),
            }),
        };
        let response = AgentApi.handle_request(&request);
        assert!(response.error.is_none());
        let result = response.result.expect("result");
        assert_eq!(
            result["changes"][0]["kind"],
            "parameter_changed"
        );
    }

    #[test]
    fn unknown_method_returns_error() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(4),
            method: "opencad.unknown".into(),
            params: Value::Null,
        };
        let response = AgentApi.handle_request(&request);
        assert_eq!(response.error.expect("error").code, -32601);
    }

    #[test]
    fn query_lists_features_via_json_rpc() {
        let part = bracket_part();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(5),
            method: "opencad.query".into(),
            params: serde_json::json!({
                "parameters": bracket_parameters(),
                "feature_nodes": part.nodes.values().collect::<Vec<_>>(),
                "feature_graph": part.graph,
                "query": { "kind": "list_features" },
            }),
        };
        let response = AgentApi.handle_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        assert_eq!(result["kind"], "features");
        assert_eq!(result["items"].as_array().expect("items").len(), 4);
    }

    #[test]
    fn explain_returns_summary_via_json_rpc() {
        let part = bracket_part();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(6),
            method: "opencad.explain".into(),
            params: serde_json::json!({
                "parameters": bracket_parameters(),
                "feature_nodes": part.nodes.values().collect::<Vec<_>>(),
                "feature_graph": part.graph,
                "sketch_count": 2,
                "document_name": "Bracket with Hole",
            }),
        };
        let response = AgentApi.handle_request(&request);
        assert!(response.error.is_none(), "{:?}", response.error);
        let result = response.result.expect("result");
        assert!(result["summary"]
            .as_str()
            .expect("summary")
            .contains("Bracket with Hole"));
        assert_eq!(result["feature_count"], 4);
    }

    #[test]
    fn handle_json_line_round_trip() {
        let state = bracket_state();
        let line = serde_json::to_string(&JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!("req-1"),
            method: "opencad.diff".into(),
            params: serde_json::json!({
                "before": DesignStateSnapshot::from(state.clone()),
                "after": DesignStateSnapshot::from(state),
            }),
        })
        .expect("json");
        let response_line = handle_json_line(&line).expect("handle");
        let response: JsonRpcResponse = serde_json::from_str(&response_line).expect("response");
        assert!(response.error.is_none());
    }
}
