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
pub use param_eval::{
    bearing_carrier_parameters, bracket_parameters, eval_angle_expr, eval_length_expr,
    evaluate_param_graph, parameter_names_in_expr, revolve_parameters, robot_arm_base_parameters,
    robot_arm_forearm_parameters, robot_arm_gripper_parameters, robot_arm_upper_arm_parameters,
    robot_joint_housing_parameters,
};
pub use param_graph::{ParamGraph, ParameterEntry};
