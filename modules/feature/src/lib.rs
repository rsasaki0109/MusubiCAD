//! Feature tree and regeneration pipeline.

pub mod cache;
pub mod chamfer;
pub mod edge_discover;
pub mod extrude;
pub mod face_discover;
pub mod feature;
pub mod fillet;
pub mod graph_derive;
pub mod hole;
pub mod imported;
pub mod param_apply;
pub mod pattern;
pub mod regenerate;
pub mod registry;
pub mod revolve;
pub mod sketch_bridge;
pub mod sketch_feature;
pub mod topo_resolve;

pub use cache::{CachedFeature, RegenerationCache};
pub use chamfer::{ChamferFeature, ChamferFeatureExecutor};
pub use extrude::{ExtrudeFeature, ExtrudeFeatureExecutor};
pub use feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
pub use fillet::{FilletFeature, FilletFeatureExecutor};
pub use graph_derive::derive_feature_graph;
pub use hole::{HoleFeature, HoleFeatureExecutor};
pub use imported::{ImportedSolidExecutor, ImportedSolidFeature};
pub use param_apply::apply_parameters;
pub use pattern::{
    CircularPatternFeature, CircularPatternFeatureExecutor, LinearPatternFeature,
    LinearPatternFeatureExecutor, MirrorPatternFeature, MirrorPatternFeatureExecutor,
    PatternOperation,
};
pub use regenerate::{
    bearing_carrier, bracket_base_plate, bracket_boss_join, bracket_edge_fillet, bracket_face_pin,
    bracket_hole_ring, bracket_hole_row, bracket_pin_mirror, bracket_pin_ring, bracket_pin_row,
    bracket_semantic_refs, bracket_with_hole, bracket_with_top_chamfer, bracket_with_top_fillet,
    revolve_bushing, revolve_sector, robot_arm_base, robot_arm_forearm, robot_arm_gripper,
    robot_arm_upper_arm, robot_joint_actuator_housing, PartModel, RegenReport, RegenerationTrace,
};
pub use registry::FeatureRegistry;
pub use revolve::{RevolveFeature, RevolveFeatureExecutor};
pub use sketch_bridge::{
    extrude_direction_for_sketch, placement_from_workplane, prepare_sketch, profile_to_solved,
    profile_to_solved_with_context, resolve_sketch_profile,
};
pub use sketch_feature::{SketchFeature, SketchFeatureDef};
