//! AI-native design editing layer.

pub mod agent_api;
pub mod assembly;
pub mod assertions;
pub mod attachment_patch;
pub mod authoring;
pub mod drawing;
pub mod explain;
pub mod feature_patch;
pub mod impact;
pub mod intent;
pub mod merge;
pub mod patch;
pub mod policy;
pub mod query;
pub mod sketch_patch;
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
pub use assertions::{
    evaluate_assertion, evaluate_assertions, required_assertions_pass, AssertionContext,
    AssertionResult,
};
pub use authoring::authoring_patch;
pub use drawing::{
    diff_drawing_models, get_drawing_sheet, get_drawing_view, list_drawing_sheets,
    list_drawing_views,
};
pub use explain::{explain_design, DesignExplanation, ExplainParams};
pub use feature_patch::{FeatureOrderEnd, FeaturePosition};
pub use impact::{
    predict_change_impact, ChangeImpact, ChangedInput, ChangedInputKind, ImpactContext,
    CHANGE_IMPACT_VERSION,
};
pub use intent::{
    apply_approved_proposal, create_proposal, AgentIntent, AgentProposal, AgentSelection,
    IntentProvider,
};
pub use merge::{
    rebase_patch, semantic_three_way_merge, ConflictKind, ConflictReason, SemanticConflict,
    SemanticMergeResult,
};
pub use patch::{
    DesignPatch, ExpectedEffect, FeatureExprField, FeatureRefField, PatchOperation,
    PatchPrecondition, MAX_PATCH_OPERATIONS,
};
pub use policy::{
    evaluate_policy, EngineeringMetrics, EngineeringPolicy, PolicyFinding, PolicyReport, PolicyRule,
};
pub use query::{
    get_semantic_ref, list_semantic_refs, query_needs_scene, run_query, DesignQuery, FaceGroupInfo,
    OverlayLineInfo, ParameterInfo, QueryParams, QueryResult, SceneQueryContext, SemanticRefInfo,
};
pub use state::{
    canonical_design_state_bytes, canonical_design_state_bytes_for_version, design_state_revision,
    design_state_revision_for_version, diff_design_state, DesignState,
    DESIGN_STATE_REVISION_ALGORITHM, DESIGN_STATE_REVISION_VERSION,
    DESIGN_STATE_REVISION_VERSION_V1, DESIGN_STATE_REVISION_VERSION_V2,
};
pub use validation::{
    build_patch_candidate, dry_run_patch, dry_run_patch_state, dry_run_patch_state_with_context,
    ensure_patch_valid, validate_design_state, PatchDryRunReport,
};
