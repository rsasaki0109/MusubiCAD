//! AI-native design editing layer.

pub mod agent_api;
pub mod assembly;
pub mod drawing;
pub mod explain;
pub mod intent;
pub mod merge;
pub mod patch;
pub mod policy;
pub mod query;
pub mod state;
pub mod validation;

pub use agent_api::{
    handle_json_line, AgentApi, DesignStateSnapshot, DiffParams, JsonRpcError, JsonRpcRequest,
    JsonRpcResponse, PatchApplyParams, PatchApplyResult, PatchDryRunParams,
};
pub use assembly::{
    diff_assembly_models, list_assembly_instances, list_assembly_mates, list_connectors,
    AssemblyInstanceInfo, AssemblyMateInfo, ConnectorInfo,
};
pub use drawing::{
    diff_drawing_models, get_drawing_sheet, get_drawing_view, list_drawing_sheets,
    list_drawing_views,
};
pub use explain::{explain_design, DesignExplanation, ExplainParams};
pub use intent::{
    apply_approved_proposal, create_proposal, AgentIntent, AgentProposal, AgentSelection,
    IntentProvider,
};
pub use merge::{
    rebase_patch, semantic_three_way_merge, ConflictKind, SemanticConflict, SemanticMergeResult,
};
pub use patch::{
    DesignPatch, ExpectedEffect, FeatureExprField, FeatureRefField, PatchOperation,
    PatchPrecondition,
};
pub use policy::{
    evaluate_policy, EngineeringMetrics, EngineeringPolicy, PolicyFinding, PolicyReport, PolicyRule,
};
pub use query::{
    get_semantic_ref, list_semantic_refs, query_needs_scene, run_query, DesignQuery, FaceGroupInfo,
    OverlayLineInfo, ParameterInfo, QueryParams, QueryResult, SceneQueryContext, SemanticRefInfo,
};
pub use state::{diff_design_state, DesignState};
pub use validation::{dry_run_patch, dry_run_patch_state, ensure_patch_valid, PatchDryRunReport};
