//! Assembly document model (Phase 3).

pub mod component;
pub mod connector;
pub mod dof;
pub mod instance;
pub mod mate;
pub mod model;
pub mod models;
pub mod pattern;
pub mod pose;
pub mod regen;
pub mod residual;
pub mod solve;

pub use component::{Component, ComponentSourceKind};
pub use connector::{validate_connectors, Connector};
pub use dof::AssemblyDofModel;
pub use instance::{Instance, Placement};
pub use mate::{validate_mates, Mate, MateEntity, MateKind};
pub use model::AssemblyModel;
pub use models::{robot_arm, robot_arm_assembly_model};
pub use pattern::{expand_patterns, validate_patterns, AssemblyPattern};
pub use regen::{
    detect_interferences, detect_interferences_with_tolerance, regenerate_assembly,
    resolve_component_path, tessellate_assembly_instances, tessellate_assembly_scene,
    validate_component_path, AssemblyInterference, AssemblyInterferenceTolerance,
    AssemblyRegenReport, AssemblyScene, ChildPart, InstanceMesh, InstanceRegenResult,
    InstanceRegenStatus, ResolvedChild, DEFAULT_INTERFERENCE_BOUNDS_TOLERANCE_M,
    DEFAULT_INTERFERENCE_VOLUME_TOLERANCE_M3,
};
pub use solve::{solve_assembly_mates, AssemblySolveReport};
