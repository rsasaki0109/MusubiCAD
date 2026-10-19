//! ISO metric coarse clearance and simplified tapped holes (ADR-037).

use serde::{Deserialize, Serialize};

use crate::{
    Feature, FeatureDefinition, FeatureNode, FeatureOutput, HoleFeature, HoleFeatureExecutor,
    RegenContext,
};
use opencad_core::{Length, OpenCadError, Result};

/// Supported ISO metric coarse designations; other sizes/fine pitches fail closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricSize {
    M3,
    M4,
    M5,
    M6,
    M8,
    M10,
    M12,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClearanceFit {
    Close,
    Normal,
    Loose,
}

/// This representation cuts the recommended drill cylinder only. It does not
/// model a helix, thread tolerances, tap lead-in or drill-point geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadRepresentation {
    TapDrill,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum HoleStandard {
    Iso273 {
        size: MetricSize,
        fit: ClearanceFit,
    },
    IsoMetricCoarse {
        size: MetricSize,
        representation: ThreadRepresentation,
    },
}

/// Unit-explicit derived manufacturing information, never independently stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandardHoleDimensions {
    pub nominal_diameter: Length,
    pub pitch: Length,
    pub cut_diameter: Length,
}

impl HoleStandard {
    pub fn dimensions(&self) -> StandardHoleDimensions {
        let size = match self {
            Self::Iso273 { size, .. } | Self::IsoMetricCoarse { size, .. } => size,
        };
        // mm: ISO coarse nominal/pitch; ISO 273 close/normal/loose clearance.
        let (nominal, pitch, clearance) = match size {
            MetricSize::M3 => (3.0, 0.5, [3.2, 3.4, 3.6]),
            MetricSize::M4 => (4.0, 0.7, [4.3, 4.5, 4.8]),
            MetricSize::M5 => (5.0, 0.8, [5.3, 5.5, 5.8]),
            MetricSize::M6 => (6.0, 1.0, [6.4, 6.6, 7.0]),
            MetricSize::M8 => (8.0, 1.25, [8.4, 9.0, 10.0]),
            MetricSize::M10 => (10.0, 1.5, [10.5, 11.0, 12.0]),
            MetricSize::M12 => (12.0, 1.75, [13.0, 13.5, 14.5]),
        };
        let cut = match self {
            Self::Iso273 { fit, .. } => {
                clearance[match fit {
                    ClearanceFit::Close => 0,
                    ClearanceFit::Normal => 1,
                    ClearanceFit::Loose => 2,
                }]
            }
            // A recommended nominal-minus-pitch drill, not an ISO tolerance limit.
            Self::IsoMetricCoarse { .. } => nominal - pitch,
        };
        StandardHoleDimensions {
            nominal_diameter: Length::from_meters(nominal * 1e-3),
            pitch: Length::from_meters(pitch * 1e-3),
            cut_diameter: Length::from_meters(cut * 1e-3),
        }
    }
}

/// Existing hole inputs plus a required, explicit standard. The sketch circle
/// must agree with the standard cut diameter within 1e-9 m (diameter).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandardHoleFeature {
    #[serde(flatten)]
    pub hole: HoleFeature,
    pub standard: HoleStandard,
}

pub const STANDARD_DIAMETER_TOLERANCE_M: f64 = 1e-9;

#[derive(Debug, Default)]
pub struct StandardHoleFeatureExecutor;

impl Feature for StandardHoleFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "standard_hole"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::StandardHole(def) = &node.definition else {
            return Err(OpenCadError::validation("expected standard_hole"));
        };
        let sketch = ctx.sketch_for_feature(&def.hole.sketch_feature)?;
        let profile = crate::sketch_bridge::profile_to_solved_with_context(
            sketch,
            &def.hole.profile_ref,
            ctx,
        )?;
        let circle = profile.circle.ok_or_else(|| {
            OpenCadError::validation("standard hole requires one circular profile")
        })?;
        let expected = def.standard.dimensions().cut_diameter.meters();
        let actual = 2.0 * circle.radius_m;
        if !actual.is_finite() || (actual - expected).abs() > STANDARD_DIAMETER_TOLERANCE_M {
            return Err(OpenCadError::validation(format!("standard hole diameter mismatch: expected {expected} m, got {actual} m (tolerance {STANDARD_DIAMETER_TOLERANCE_M} m)")));
        }
        HoleFeatureExecutor.execute(
            &FeatureNode::new(
                &node.id,
                &node.name,
                FeatureDefinition::Hole(def.hole.clone()),
            ),
            ctx,
        )
    }
    fn discover_output_faces(
        &self,
        node: &FeatureNode,
        ctx: &dyn RegenContext,
        output: &FeatureOutput,
    ) -> Result<Option<Vec<opencad_geometry::FaceRefDiscovery>>> {
        let FeatureDefinition::StandardHole(def) = &node.definition else {
            return Err(OpenCadError::validation(
                "expected standard_hole face owner",
            ));
        };
        if def.hole.face_ref.is_none() {
            return Ok(None);
        }
        // Preserve wider legacy source behavior. The reviewed creation gateway
        // separately requires this exact admitted subset and final provenance.
        let Ok(inputs) = crate::seated_standard_hole::admit_owned_hole_inputs(
            def,
            def.standard.dimensions().cut_diameter,
            ctx,
        ) else {
            return Ok(None);
        };
        crate::seated_standard_hole::discover_owned_hole_faces(
            node,
            &def.hole.target_feature,
            ctx,
            output,
            &inputs,
        )
    }
}
