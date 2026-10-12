//! Kernel-neutral construction geometry (ADR-035).
use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::sketch_bridge::{placement_from_workplane, resolve_workplane};
use opencad_core::{Length, OpenCadError, Result};
use opencad_sketch::Workplane;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatumPlaneFeature {
    pub base: Workplane,
    pub offset: Length,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_expr: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatumAxisFeature {
    pub origin_m: [f64; 3],
    pub direction: [f64; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_expr: Option<[Option<String>; 3]>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DatumGeometry {
    Plane {
        workplane: Workplane,
    },
    Axis {
        origin_m: [f64; 3],
        direction: [f64; 3],
    },
}
pub(crate) fn normalized(vector: [f64; 3]) -> Result<[f64; 3]> {
    let norm = vector.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !norm.is_finite() || norm < 1e-12 {
        return Err(OpenCadError::validation(
            "datum direction must be finite and non-zero",
        ));
    }
    Ok(vector.map(|v| v / norm))
}
pub(crate) fn finite_origin(origin: [f64; 3]) -> Result<()> {
    if origin.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(OpenCadError::validation("datum origin must be finite (m)"))
    }
}

pub struct DatumPlaneExecutor;
impl Feature for DatumPlaneExecutor {
    fn feature_type(&self) -> &'static str {
        "datum_plane"
    }
    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::DatumPlane(def) = &node.definition else {
            return Err(OpenCadError::validation("expected datum_plane"));
        };
        if !def.offset.meters().is_finite() {
            return Err(OpenCadError::validation("datum offset must be finite (m)"));
        }
        let base = resolve_workplane(&def.base, ctx)?;
        if let Workplane::Custom {
            origin,
            normal,
            x_axis,
        } = &base
        {
            finite_origin(*origin)?;
            let n = normalized(*normal)?;
            let x = normalized(*x_axis)?;
            if n.iter().zip(x).map(|(a, b)| a * b).sum::<f64>().abs() > 1e-9 {
                return Err(OpenCadError::validation(
                    "datum plane normal and x_axis must be orthogonal",
                ));
            }
        }
        let placement = placement_from_workplane(&base)?;
        finite_origin(placement.origin_m)?;
        let x = normalized(placement.x_axis_m)?;
        let y = normalized(placement.y_axis_m)?;
        let normal = normalized([
            x[1] * y[2] - x[2] * y[1],
            x[2] * y[0] - x[0] * y[2],
            x[0] * y[1] - x[1] * y[0],
        ])?;
        // A frame must be orthogonal: accepting a skew plane would scale sketches.
        if x.iter().zip(y).map(|(a, b)| a * b).sum::<f64>().abs() > 1e-9 {
            return Err(OpenCadError::validation(
                "datum plane axes must be orthogonal",
            ));
        }
        let origin =
            std::array::from_fn(|i| placement.origin_m[i] + normal[i] * def.offset.meters());
        finite_origin(origin)?;
        Ok(FeatureOutput {
            sketch: None,
            body: None,
            datum: Some(DatumGeometry::Plane {
                workplane: Workplane::Custom {
                    origin,
                    normal,
                    x_axis: x,
                },
            }),
        })
    }
}
pub struct DatumAxisExecutor;
impl Feature for DatumAxisExecutor {
    fn feature_type(&self) -> &'static str {
        "datum_axis"
    }
    fn execute(&self, node: &FeatureNode, _: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::DatumAxis(def) = &node.definition else {
            return Err(OpenCadError::validation("expected datum_axis"));
        };
        finite_origin(def.origin_m)?;
        Ok(FeatureOutput {
            sketch: None,
            body: None,
            datum: Some(DatumGeometry::Axis {
                origin_m: def.origin_m,
                direction: normalized(def.direction)?,
            }),
        })
    }
}

/// Copy sketches with datum placements resolved for rendering; preserve authored refs.
pub fn resolved_datum_sketches(
    model: &crate::regenerate::PartModel,
) -> Result<Vec<opencad_sketch::Sketch>> {
    model
        .sketches
        .values()
        .map(|sketch| {
            let mut copy = sketch.clone();
            if let Workplane::DatumPlane { feature_id } = &sketch.workplane {
                match model
                    .outputs
                    .get(feature_id)
                    .and_then(|output| output.datum.as_ref())
                {
                    Some(DatumGeometry::Plane { workplane }) => copy.workplane = workplane.clone(),
                    _ => {
                        return Err(OpenCadError::validation(format!(
                            "unresolved datum plane '{feature_id}'"
                        )))
                    }
                }
            }
            Ok(copy)
        })
        .collect()
}
