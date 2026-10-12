//! Feature trait and serializable node model (Task-086+).

use serde::{Deserialize, Serialize};

use opencad_core::Result;
use opencad_geometry::{FaceDerivation, GeometryKernel, KernelBody, TopoRef};
use opencad_sketch::Sketch;

use crate::chamfer::ChamferFeature;
use crate::extrude::ExtrudeFeature;
use crate::fillet::FilletFeature;
use crate::helix_sweep::HelixSweepFeature;
use crate::hole::HoleFeature;
use crate::imported::ImportedSolidFeature;
use crate::loft::LoftFeature;
use crate::pattern::{CircularPatternFeature, LinearPatternFeature, MirrorPatternFeature};
use crate::revolve::RevolveFeature;
use crate::rib::RibFeature;
use crate::shell::ShellFeature;
use crate::sketch_feature::SketchFeatureDef;
use crate::sweep::SweepFeature;

/// Serializable feature definition stored in the design graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeatureDefinition {
    DatumPlane(crate::datum::DatumPlaneFeature),
    DatumAxis(crate::datum::DatumAxisFeature),
    Sketch(SketchFeatureDef),
    Extrude(ExtrudeFeature),
    Revolve(RevolveFeature),
    Hole(HoleFeature),
    StandardHole(crate::standard_hole::StandardHoleFeature),
    Fillet(FilletFeature),
    Chamfer(ChamferFeature),
    LinearPattern(LinearPatternFeature),
    CircularPattern(CircularPatternFeature),
    MirrorPattern(MirrorPatternFeature),
    ImportedSolid(ImportedSolidFeature),
    ImportedMesh(crate::ImportedMeshFeature),
    Shell(ShellFeature),
    Draft(crate::draft::DraftFeature),
    Rib(RibFeature),
    BodyBoolean(crate::body_boolean::BodyBooleanFeature),
    Loft(LoftFeature),
    Sweep(SweepFeature),
    HelixSweep(HelixSweepFeature),
}

impl FeatureDefinition {
    /// Immutable source attachment and expected digest, when applicable.
    pub fn attachment_input(&self) -> Option<(&str, &str)> {
        match self {
            Self::ImportedSolid(def) => Some((&def.source, &def.sha256)),
            Self::ImportedMesh(def) => Some((&def.source, &def.sha256)),
            _ => None,
        }
    }
    pub fn feature_type(&self) -> &'static str {
        match self {
            Self::DatumPlane(_) => "datum_plane",
            Self::DatumAxis(_) => "datum_axis",
            Self::Sketch(_) => "sketch",
            Self::Extrude(_) => "extrude",
            Self::Revolve(_) => "revolve",
            Self::Hole(_) => "hole",
            Self::StandardHole(_) => "standard_hole",
            Self::Fillet(_) => "fillet",
            Self::Chamfer(_) => "chamfer",
            Self::LinearPattern(_) => "linear_pattern",
            Self::CircularPattern(_) => "circular_pattern",
            Self::MirrorPattern(_) => "mirror_pattern",
            Self::ImportedSolid(_) => "imported_solid",
            Self::ImportedMesh(_) => "imported_mesh",
            Self::Shell(_) => "shell",
            Self::Draft(_) => "draft",
            Self::Rib(_) => "rib",
            Self::BodyBoolean(_) => "body_boolean",
            Self::Loft(_) => "loft",
            Self::Sweep(_) => "sweep",
            Self::HelixSweep(_) => "helix_sweep",
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datum: Option<crate::datum::DatumGeometry>,
}

/// Read-only regeneration context for feature executors.
pub trait RegenContext {
    fn kernel(&self) -> &dyn GeometryKernel;

    fn sketch_for_feature(&self, sketch_feature_id: &str) -> Result<&Sketch>;

    fn body_for_feature(&self, feature_id: &str) -> Result<KernelBody>;

    fn datum_for_feature(&self, feature_id: &str) -> Result<crate::datum::DatumGeometry> {
        Err(opencad_core::OpenCadError::not_found(format!(
            "datum for feature '{feature_id}'"
        )))
    }

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

    /// Face discoveries on `body` itself.  [`Self::face_discoveries`]
    /// describes the most recently regenerated body, which is not `body`
    /// when a feature consumes an earlier body; its kernel face IDs would
    /// not name faces of `body`.
    fn face_discoveries_on(
        &self,
        _body: &KernelBody,
    ) -> Result<Vec<opencad_geometry::FaceRefDiscovery>> {
        Ok(self.face_discoveries().to_vec())
    }

    /// Bytes of a document attachment such as `imports/motor.step` (ADR-016).
    fn attachment(&self, path: &str) -> Result<&[u8]> {
        Err(opencad_core::OpenCadError::not_found(format!(
            "attachment '{path}'"
        )))
    }
}

/// Executable feature interface (Task-086).
pub trait Feature: Send + Sync {
    fn feature_type(&self) -> &'static str;

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput>;
}
