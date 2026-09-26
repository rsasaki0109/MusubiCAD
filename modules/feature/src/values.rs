//! Numeric validation of feature definitions before any kernel call
//! (MCAD-P7-006).
//!
//! Regeneration used to be the first place a zero fillet radius or an empty
//! pattern failed, deep inside the geometry kernel.  These checks evaluate
//! each value the way regeneration will (expression first, stored value
//! otherwise) and reject impossible ones during dry-run.

use indexmap::IndexMap;
use opencad_core::{OpenCadError, Result};
use opencad_geometry::ExtrudeExtent;
use opencad_graph::{eval_angle_expr, eval_length_expr};

use crate::feature::FeatureDefinition;

/// Smallest accepted feature length (extrude/hole depth, fillet radius,
/// chamfer distance, pattern spacing), in metres.
pub const MIN_FEATURE_LENGTH_M: f64 = 1e-9;

/// Smallest accepted norm of an axis, direction, or plane normal
/// (dimensionless direction vectors).
pub const MIN_DIRECTION_NORM: f64 = 1e-12;

/// Largest accepted revolve angle, `2π` plus a dimensionless `1e-9` rad
/// allowance for expressions that evaluate a full turn.
pub const MAX_REVOLVE_ANGLE_RAD: f64 = std::f64::consts::TAU + 1e-9;

fn length(
    field: &str,
    expr: Option<&String>,
    stored_m: f64,
    scope: &IndexMap<String, f64>,
) -> Result<f64> {
    let value = match expr {
        // An expression this scope cannot evaluate (for example an unknown
        // parameter) is reported by reference validation, not here.
        Some(expr) => match eval_length_expr(expr, scope) {
            Ok(value) => value,
            Err(_) => return Ok(stored_m),
        },
        None => stored_m,
    };
    if !value.is_finite() || value < MIN_FEATURE_LENGTH_M {
        return Err(OpenCadError::validation(format!(
            "{field} must be at least {MIN_FEATURE_LENGTH_M} m, got {value} m"
        )));
    }
    Ok(value)
}

fn extent(
    field: &str,
    extent: &ExtrudeExtent,
    expr: Option<&String>,
    scope: &IndexMap<String, f64>,
) -> Result<()> {
    match extent {
        ExtrudeExtent::ThroughAll => Ok(()),
        ExtrudeExtent::Distance { length: stored }
        | ExtrudeExtent::Symmetric { length: stored } => {
            length(field, expr, stored.meters(), scope).map(|_| ())
        }
    }
}

fn direction(field: &str, vector: &[f64; 3]) -> Result<()> {
    let norm = vector.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !norm.is_finite() || norm < MIN_DIRECTION_NORM {
        return Err(OpenCadError::validation(format!(
            "{field} must be a finite non-zero direction (norm >= {MIN_DIRECTION_NORM})"
        )));
    }
    Ok(())
}

fn count(field: &str, value: u32) -> Result<()> {
    if value == 0 {
        return Err(OpenCadError::validation(format!(
            "{field} must be at least 1"
        )));
    }
    Ok(())
}

impl FeatureDefinition {
    /// Reject numeric values regeneration cannot use, evaluating expressions
    /// against the resolved parameter `scope` (metres / radians by name).
    pub fn validate_values(&self, scope: &IndexMap<String, f64>) -> Result<()> {
        match self {
            Self::Sketch(_) => Ok(()),
            Self::Extrude(def) => extent(
                "extrude length",
                &def.extent,
                def.length_expr.as_ref(),
                scope,
            ),
            Self::Hole(def) => extent("hole depth", &def.depth, def.depth_expr.as_ref(), scope),
            Self::Revolve(def) => {
                direction("revolve axis_direction_m", &def.axis_direction_m)?;
                let angle = match &def.angle_expr {
                    Some(expr) => match eval_angle_expr(expr, scope) {
                        Ok(angle) => angle,
                        Err(_) => return Ok(()),
                    },
                    None => def.angle_rad,
                };
                if !angle.is_finite() || angle <= 0.0 || angle > MAX_REVOLVE_ANGLE_RAD {
                    return Err(OpenCadError::validation(format!(
                        "revolve angle must be in (0, 2π] rad, got {angle} rad"
                    )));
                }
                Ok(())
            }
            Self::Fillet(def) => length(
                "fillet radius",
                def.radius_expr.as_ref(),
                def.radius.meters(),
                scope,
            )
            .map(|_| ()),
            Self::Chamfer(def) => length(
                "chamfer distance",
                def.distance_expr.as_ref(),
                def.distance.meters(),
                scope,
            )
            .map(|_| ()),
            Self::LinearPattern(def) => {
                count("linear pattern count", def.count)?;
                direction("linear pattern direction_m", &def.direction_m)?;
                if def.count > 1 {
                    length(
                        "linear pattern spacing",
                        def.spacing_expr.as_ref(),
                        def.spacing.meters(),
                        scope,
                    )?;
                }
                Ok(())
            }
            Self::CircularPattern(def) => {
                count("circular pattern count", def.count)?;
                direction("circular pattern axis_direction_m", &def.axis_direction_m)
            }
            Self::MirrorPattern(def) => {
                if def.plane_face_ref.is_none() {
                    direction("mirror plane_normal_m", &def.plane_normal_m)?;
                }
                Ok(())
            }
            Self::ImportedSolid(def) => def.validate(),
            Self::Loft(def) => def.validate(),
            Self::Shell(def) => {
                def.validate()?;
                let thickness = match &def.thickness_expr {
                    Some(expr) => match eval_length_expr(expr, scope) {
                        Ok(value) => value,
                        Err(_) => return Ok(()),
                    },
                    None => def.thickness.meters(),
                };
                crate::shell::validate_thickness(thickness)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bracket_with_hole, bracket_with_top_fillet, FeatureNode};

    fn scope(pairs: &[(&str, f64)]) -> IndexMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn node(part: crate::PartModel, id: &str) -> FeatureNode {
        part.nodes.get(id).expect("node").clone()
    }

    #[test]
    fn values_are_evaluated_from_expressions_before_stored_values() {
        let extrude = node(bracket_with_hole().unwrap(), "feature:extrude_base");
        assert!(extrude
            .definition
            .validate_values(&scope(&[("thickness", 0.006)]))
            .is_ok());
        let error = extrude
            .definition
            .validate_values(&scope(&[("thickness", 0.0)]))
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("extrude length must be at least"));

        let fillet = node(bracket_with_top_fillet().unwrap(), "feature:fillet_top");
        let error = fillet
            .definition
            .validate_values(&scope(&[("fillet_radius", -0.001)]))
            .unwrap_err();
        assert!(error.to_string().contains("fillet radius"));
    }

    #[test]
    fn patterns_and_revolves_reject_degenerate_values() {
        let mut part = crate::bracket_hole_row().unwrap();
        let pattern = part
            .nodes
            .values_mut()
            .find(|node| matches!(node.definition, FeatureDefinition::LinearPattern(_)))
            .expect("pattern");
        let FeatureDefinition::LinearPattern(def) = &mut pattern.definition else {
            unreachable!()
        };
        def.count = 0;
        def.spacing_expr = None;
        assert!(pattern
            .definition
            .validate_values(&IndexMap::new())
            .unwrap_err()
            .to_string()
            .contains("count must be at least 1"));

        let mut revolve = crate::revolve_bushing().unwrap();
        let node = revolve
            .nodes
            .values_mut()
            .find(|node| matches!(node.definition, FeatureDefinition::Revolve(_)))
            .expect("revolve");
        let FeatureDefinition::Revolve(def) = &mut node.definition else {
            unreachable!()
        };
        def.angle_expr = None;
        def.angle_rad = 7.0;
        assert!(node
            .definition
            .validate_values(&IndexMap::new())
            .unwrap_err()
            .to_string()
            .contains("revolve angle"));
    }
}
