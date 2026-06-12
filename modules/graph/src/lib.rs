//! Design graph, parametric graph, feature graph, and semantic diff.

pub mod dependency;
pub mod design_graph;
pub mod diff;
pub mod feature_graph;
pub mod param_eval;
pub mod param_graph;

pub use dependency::{topological_sort, DependencyEdge, EdgeKind};
pub use design_graph::{DesignGraph, GraphNode, GraphNodeKind};
pub use diff::{
    build_summary, diff_param_graphs, diff_semantic_refs, format_mass_kg, DesignDiff, DiffType,
    GeometricDiff, SemanticChange,
};
pub use feature_graph::{FeatureEntry, FeatureGraph};
pub use param_eval::{bracket_parameters, evaluate_param_graph, eval_length_expr};
pub use param_graph::{ParamGraph, ParameterEntry};
