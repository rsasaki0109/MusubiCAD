//! AI-native design editing layer.

pub mod agent_api;
pub mod explain;
pub mod intent;
pub mod patch;
pub mod query;
pub mod state;
pub mod validation;

pub use agent_api::{
    handle_json_line, AgentApi, DesignStateSnapshot, DiffParams, JsonRpcError, JsonRpcRequest,
    JsonRpcResponse, PatchApplyParams, PatchApplyResult, PatchDryRunParams,
};
pub use explain::{explain_design, DesignExplanation, ExplainParams};
pub use patch::{DesignPatch, FeatureExprField, PatchOperation};
pub use query::{
    get_semantic_ref, list_semantic_refs, run_query, DesignQuery, FaceGroupInfo, OverlayLineInfo,
    ParameterInfo, QueryParams, QueryResult, SceneQueryContext, SemanticRefInfo, query_needs_scene,
};
pub use state::{diff_design_state, DesignState};
pub use validation::{dry_run_patch, dry_run_patch_state, ensure_patch_valid, PatchDryRunReport};
