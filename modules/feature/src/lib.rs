//! Feature tree and regeneration pipeline.

pub mod chamfer;
pub mod extrude;
pub mod face_discover;
pub mod feature;
pub mod fillet;
pub mod hole;
pub mod param_apply;
pub mod pattern;
pub mod regenerate;
pub mod registry;
pub mod sketch_bridge;
pub mod sketch_feature;
pub mod topo_resolve;

pub use chamfer::{ChamferFeature, ChamferFeatureExecutor};
pub use extrude::{ExtrudeFeature, ExtrudeFeatureExecutor};
pub use feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
pub use fillet::{FilletFeature, FilletFeatureExecutor};
pub use hole::{HoleFeature, HoleFeatureExecutor};
pub use param_apply::apply_parameters;
pub use pattern::{
    CircularPatternFeature, CircularPatternFeatureExecutor, LinearPatternFeature,
    LinearPatternFeatureExecutor, MirrorPatternFeature, MirrorPatternFeatureExecutor,
    PatternOperation,
};
pub use regenerate::{
    bracket_base_plate, bracket_hole_ring, bracket_hole_row, bracket_pin_mirror, bracket_pin_ring,
    bracket_pin_row, bracket_semantic_refs, bracket_with_hole, bracket_with_top_chamfer,
    bracket_with_top_fillet, PartModel, RegenReport,
};
pub use registry::FeatureRegistry;
pub use sketch_bridge::{prepare_sketch, profile_to_solved};
pub use sketch_feature::{SketchFeature, SketchFeatureDef};
