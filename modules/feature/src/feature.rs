//! Feature trait and serializable node model (Task-086+).

use serde::{Deserialize, Serialize};

use opencad_core::Result;
use opencad_geometry::{FaceDerivation, GeometryKernel, KernelBody, TopoRef};
use opencad_sketch::Sketch;

use crate::chamfer::ChamferFeature;
use crate::extrude::ExtrudeFeature;
use crate::fillet::FilletFeature;
use crate::hole::HoleFeature;
use crate::pattern::{CircularPatternFeature, LinearPatternFeature, MirrorPatternFeature};
use crate::revolve::RevolveFeature;
use crate::sketch_feature::SketchFeatureDef;

/// Serializable feature definition stored in the design graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeatureDefinition {
    Sketch(SketchFeatureDef),
    Extrude(ExtrudeFeature),
    Revolve(RevolveFeature),
    Hole(HoleFeature),
    Fillet(FilletFeature),
    Chamfer(ChamferFeature),
    LinearPattern(LinearPatternFeature),
    CircularPattern(CircularPatternFeature),
    MirrorPattern(MirrorPatternFeature),
}

impl FeatureDefinition {
    pub fn feature_type(&self) -> &'static str {
        match self {
            Self::Sketch(_) => "sketch",
            Self::Extrude(_) => "extrude",
            Self::Revolve(_) => "revolve",
            Self::Hole(_) => "hole",
            Self::Fillet(_) => "fillet",
            Self::Chamfer(_) => "chamfer",
            Self::LinearPattern(_) => "linear_pattern",
            Self::CircularPattern(_) => "circular_pattern",
            Self::MirrorPattern(_) => "mirror_pattern",
        }
    }
}

/// A feature node: metadata plus executable definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureNode {
    pub id: String,
    pub name: String,
    pub definition: FeatureDefinition,
    #[serde(default)]
    pub suppressed: bool,
}

impl FeatureNode {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        definition: FeatureDefinition,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            definition,
            suppressed: false,
        }
    }
}

/// Output produced by a single feature execution.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FeatureOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<KernelBody>,
}

/// Read-only regeneration context for feature executors.
pub trait RegenContext {
    fn kernel(&self) -> &dyn GeometryKernel;

    fn sketch_for_feature(&self, sketch_feature_id: &str) -> Result<&Sketch>;

    fn body_for_feature(&self, feature_id: &str) -> Result<KernelBody>;

    fn semantic_refs(&self) -> &[TopoRef] {
        &[]
    }

    fn face_history(&self) -> &[FaceDerivation] {
        &[]
    }

    fn face_discoveries(&self) -> &[opencad_geometry::FaceRefDiscovery] {
        &[]
    }

    fn edge_discoveries(&self) -> &[opencad_geometry::EdgeRefDiscovery] {
        &[]
    }
}

/// Executable feature interface (Task-086).
pub trait Feature: Send + Sync {
    fn feature_type(&self) -> &'static str;

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput>;
}
