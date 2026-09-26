//! Apply parametric graph values to sketches before regeneration.

use indexmap::IndexMap;

use opencad_core::{Expression, Length, OpenCadError, Result};
use opencad_geometry::ExtrudeExtent;
use opencad_graph::{eval_angle_expr, eval_length_expr, evaluate_param_graph, ParamGraph};
use opencad_sketch::{solve_sketch, Constraint, Sketch};
use opencad_solver::SolverOptions;

use crate::feature::{FeatureDefinition, FeatureNode};
use crate::regenerate::PartModel;

pub fn apply_parameters(model: &mut PartModel, parameters: &ParamGraph) -> Result<()> {
    if parameters.evaluation_order()?.is_empty() {
        return Ok(());
    }
    let values = evaluate_param_graph(parameters)?;
    for sketch in model.sketches.values_mut() {
        let originals = snapshot_dimension_exprs(sketch);
        resolve_sketch_constraints(sketch, &values)?;
        solve_sketch(sketch, &SolverOptions::default())?;
        restore_dimension_exprs(sketch, &originals);
        sketch.update_profiles()?;
    }
    apply_feature_parameters(model, &values)?;
    Ok(())
}

/// Solve a copy of `sketch` with the parameter `values` (metres / radians by
/// name) and return its constraint state.  The sketch itself is unchanged.
pub fn sketch_solve_state(
    sketch: &Sketch,
    values: &IndexMap<String, f64>,
) -> Result<opencad_sketch::SolveState> {
    let mut copy = sketch.clone();
    resolve_sketch_constraints(&mut copy, values)?;
    solve_sketch(&mut copy, &SolverOptions::default())?;
    Ok(copy.solve_state)
}

pub fn apply_feature_parameters(
    model: &mut PartModel,
    values: &indexmap::IndexMap<String, f64>,
) -> Result<()> {
    for node in model.nodes.values_mut() {
        resolve_feature_node(node, values)?;
    }
    Ok(())
}

fn resolve_feature_node(
    node: &mut FeatureNode,
    values: &indexmap::IndexMap<String, f64>,
) -> Result<()> {
    match &mut node.definition {
        FeatureDefinition::Extrude(extrude) => {
            if let Some(expr) = &extrude.length_expr {
                let meters = eval_length_expr(expr, values)?;
                extrude.extent = ExtrudeExtent::Distance {
                    length: Length::from_meters(meters),
                };
            }
        }
        FeatureDefinition::Hole(hole) => {
            if let Some(expr) = &hole.depth_expr {
                let meters = eval_length_expr(expr, values)?;
                hole.depth = ExtrudeExtent::Distance {
                    length: Length::from_meters(meters),
                };
            }
        }
        FeatureDefinition::Fillet(fillet) => {
            if let Some(expr) = &fillet.radius_expr {
                let meters = eval_length_expr(expr, values)?;
                fillet.radius = Length::from_meters(meters);
            }
        }
        FeatureDefinition::Chamfer(chamfer) => {
            if let Some(expr) = &chamfer.distance_expr {
                let meters = eval_length_expr(expr, values)?;
                chamfer.distance = Length::from_meters(meters);
            }
        }
        FeatureDefinition::Shell(shell) => {
            if let Some(expr) = &shell.thickness_expr {
                let meters = eval_length_expr(expr, values)?;
                shell.thickness = Length::from_meters(meters);
            }
        }
        FeatureDefinition::LinearPattern(pattern) => {
            if let Some(expr) = &pattern.spacing_expr {
                let meters = eval_length_expr(expr, values)?;
                pattern.spacing = Length::from_meters(meters);
            }
        }
        FeatureDefinition::HelixSweep(helix) => {
            if let Some(expr) = &helix.pitch_expr {
                helix.pitch_m = eval_length_expr(expr, values)?;
            }
            if let Some(expr) = &helix.height_expr {
                helix.height_m = eval_length_expr(expr, values)?;
            }
        }
        FeatureDefinition::Revolve(revolve) => {
            if let Some(expr) = &revolve.angle_expr {
                let radians = eval_length_expr(expr, values)?;
                revolve.angle_rad = radians;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn resolve_sketch_constraints(
    sketch: &mut Sketch,
    scope: &IndexMap<String, f64>,
) -> Result<()> {
    // Arc angles may name parameters (ADR-024).  Their expressions stay on
    // the entity; the resolved radians drive solving and kernel input.
    for entity in &mut sketch.entities {
        let opencad_sketch::SketchEntity::Arc(arc) = entity else {
            continue;
        };
        let parametric = [&arc.start_angle, &arc.end_angle]
            .iter()
            .any(|angle| matches!(angle, opencad_sketch::entity::Coord::Expr(_)));
        arc.resolved_angles_rad = if parametric {
            let resolve = |angle: &opencad_sketch::entity::Coord| match angle {
                opencad_sketch::entity::Coord::Literal(value) => Ok(*value),
                opencad_sketch::entity::Coord::Expr(expr) => eval_angle_expr(expr.as_str(), scope),
            };
            Some([resolve(&arc.start_angle)?, resolve(&arc.end_angle)?])
        } else {
            None
        };
    }
    for constraint in &mut sketch.constraints {
        match constraint {
            Constraint::Distance { expr, .. }
            | Constraint::Radius { expr, .. }
            | Constraint::Diameter { expr, .. } => {
                *expr = resolve_expression(expr, scope)?;
            }
            Constraint::Angle { expr, .. } => {
                let value_rad = eval_angle_expr(expr.as_str(), scope)?;
                *expr = Expression::new(format!("{value_rad} rad"))
                    .map_err(|_| OpenCadError::InvalidExpression(expr.as_str().into()))?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn resolve_expression(expr: &Expression, scope: &IndexMap<String, f64>) -> Result<Expression> {
    let value_m = eval_length_expr(expr.as_str(), scope)?;
    Expression::new(format!("{} mm", value_m * 1000.0))
        .map_err(|_| OpenCadError::InvalidExpression(expr.as_str().into()))
}

fn snapshot_dimension_exprs(sketch: &Sketch) -> Vec<(usize, Expression)> {
    sketch
        .constraints
        .iter()
        .enumerate()
        .filter_map(|(index, constraint)| match constraint {
            Constraint::Distance { expr, .. }
            | Constraint::Radius { expr, .. }
            | Constraint::Diameter { expr, .. }
            | Constraint::Angle { expr, .. } => Some((index, expr.clone())),
            _ => None,
        })
        .collect()
}

fn restore_dimension_exprs(sketch: &mut Sketch, originals: &[(usize, Expression)]) {
    for (index, expr) in originals {
        if let Some(
            Constraint::Distance { expr: target, .. }
            | Constraint::Radius { expr: target, .. }
            | Constraint::Diameter { expr: target, .. }
            | Constraint::Angle { expr: target, .. },
        ) = sketch.constraints.get_mut(*index)
        {
            *target = expr.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_graph::ParamGraph;

    #[test]
    fn angle_constraint_expressions_resolve_to_radians_and_restore() {
        use opencad_core::{ConstraintId, EntityId, SketchId};
        use opencad_sketch::workplane::Workplane;

        let mut sketch = Sketch::new(SketchId::new("sketch:a").unwrap(), "A", Workplane::xy());
        sketch
            .add_constraint(Constraint::Angle {
                id: ConstraintId::new("con:tilt").unwrap(),
                line_a: EntityId::new("ent:a").unwrap(),
                line_b: EntityId::new("ent:b").unwrap(),
                expr: Expression::new("tilt_angle").unwrap(),
            })
            .unwrap();
        let originals = snapshot_dimension_exprs(&sketch);
        let scope: IndexMap<String, f64> = [("tilt_angle".to_string(), 0.25)].into_iter().collect();
        resolve_sketch_constraints(&mut sketch, &scope).expect("resolve");
        let Constraint::Angle { expr, .. } = &sketch.constraints[0] else {
            panic!("angle")
        };
        assert_eq!(expr.as_str(), "0.25 rad");
        restore_dimension_exprs(&mut sketch, &originals);
        let Constraint::Angle { expr, .. } = &sketch.constraints[0] else {
            panic!("angle")
        };
        assert_eq!(expr.as_str(), "tilt_angle");
    }

    /// The bracket template's base rectangle is fully constrained, so a
    /// length edit cannot skew it (MCAD-P7-014).
    #[test]
    fn bracket_base_sketch_is_fully_constrained() {
        let model = crate::regenerate::bracket_base_plate().expect("model");
        let sketch = model.sketches.get("sketch:base").expect("sketch");
        let values = evaluate_param_graph(&opencad_graph::bracket_parameters()).expect("values");
        assert_eq!(
            sketch_solve_state(sketch, &values).expect("solve"),
            opencad_sketch::SolveState::FullyConstrained
        );
    }

    #[test]
    fn applies_width_parameter_to_bracket_sketch() {
        let mut model = crate::regenerate::bracket_base_plate().expect("model");
        let mut params = ParamGraph::new();
        params
            .add_parameter(opencad_graph::ParameterEntry::new(
                "param:width",
                "width",
                "100 mm",
            ))
            .expect("param");
        params
            .add_parameter(opencad_graph::ParameterEntry::new(
                "param:height",
                "height",
                "60 mm",
            ))
            .expect("param");
        params
            .add_parameter(opencad_graph::ParameterEntry::new(
                "param:thickness",
                "thickness",
                "6 mm",
            ))
            .expect("param");

        apply_parameters(&mut model, &params).expect("apply");
        let sketch = model.sketches.get("sketch:base").expect("sketch");
        let c1 = sketch
            .find_entity("ent:c1")
            .and_then(|e| match e {
                opencad_sketch::SketchEntity::Point(p) => Some(p),
                _ => None,
            })
            .expect("c1");
        let x = match c1.x {
            opencad_sketch::Coord::Literal(v) => v,
            _ => panic!("literal"),
        };
        assert!((x - 0.1).abs() < 1e-4);

        let width_expr = match &sketch.constraints[0] {
            Constraint::Distance { expr, .. } => expr.as_str(),
            _ => panic!("distance"),
        };
        assert_eq!(width_expr, "width");
    }

    #[test]
    fn applies_thickness_to_extrude_feature() {
        let mut model = crate::regenerate::bracket_base_plate().expect("model");
        let mut params = ParamGraph::new();
        params
            .add_parameter(opencad_graph::ParameterEntry::new(
                "param:width",
                "width",
                "80 mm",
            ))
            .expect("width");
        params
            .add_parameter(opencad_graph::ParameterEntry::new(
                "param:height",
                "height",
                "60 mm",
            ))
            .expect("height");
        params
            .add_parameter(opencad_graph::ParameterEntry::new(
                "param:thickness",
                "thickness",
                "8 mm",
            ))
            .expect("thickness");

        apply_parameters(&mut model, &params).expect("apply");
        let node = model.nodes.get("feature:extrude_base").expect("extrude");
        let FeatureDefinition::Extrude(extrude) = &node.definition else {
            panic!("extrude");
        };
        let ExtrudeExtent::Distance { length } = extrude.extent else {
            panic!("distance extent");
        };
        assert!((length.meters() - 0.008).abs() < 1e-9);
    }

    #[test]
    fn applies_hole_diameter_to_boss_sketch() {
        let mut model = crate::regenerate::bracket_boss_join().expect("model");
        let mut params = opencad_graph::bracket_parameters();
        params
            .set_expr("param:hole_diameter", "16 mm")
            .expect("hole_diameter");

        apply_parameters(&mut model, &params).expect("apply");
        let sketch = model.sketches.get("sketch:boss").expect("sketch");
        let radius_expr = match &sketch.constraints[0] {
            Constraint::Radius { expr, .. } => expr.as_str(),
            _ => panic!("radius"),
        };
        assert_eq!(radius_expr, "hole_diameter / 2");
        let circle = sketch
            .find_entity("ent:boss_circle")
            .and_then(|e| match e {
                opencad_sketch::SketchEntity::Circle(c) => Some(c),
                _ => None,
            })
            .expect("boss circle");
        let radius_m = match circle.radius {
            opencad_sketch::Coord::Literal(v) => v,
            _ => panic!("literal radius"),
        };
        assert!((radius_m - 0.008).abs() < 1e-4);
    }

    #[test]
    fn applies_boss_height_to_boss_join_extrude() {
        let mut model = crate::regenerate::bracket_boss_join().expect("model");
        let mut params = opencad_graph::bracket_parameters();
        params
            .set_expr("param:boss_height", "16 mm")
            .expect("boss_height");

        apply_parameters(&mut model, &params).expect("apply");
        let node = model.nodes.get("feature:boss_join").expect("boss");
        let FeatureDefinition::Extrude(extrude) = &node.definition else {
            panic!("extrude");
        };
        assert_eq!(extrude.length_expr.as_deref(), Some("boss_height"));
        let ExtrudeExtent::Distance { length } = extrude.extent else {
            panic!("distance");
        };
        assert!((length.meters() - 0.016).abs() < 1e-9);
    }

    #[test]
    fn applies_robot_joint_bolt_circle_radius_to_tool_position() {
        let mut model = crate::regenerate::robot_joint_actuator_housing().expect("model");
        let mut params = opencad_graph::robot_joint_housing_parameters();
        params
            .set_expr("param:bolt_circle_radius", "50 mm")
            .expect("bolt circle radius");

        apply_parameters(&mut model, &params).expect("apply");
        let sketch = model.sketches.get("sketch:pcd_hole").expect("PCD sketch");
        let center_x = sketch
            .find_entity("ent:pcd_hole_center")
            .and_then(|entity| match entity {
                opencad_sketch::SketchEntity::Point(point) => match point.x {
                    opencad_sketch::Coord::Literal(value) => Some(value),
                    _ => None,
                },
                _ => None,
            })
            .expect("PCD hole center x");
        let axis_x = sketch
            .find_entity("ent:pcd_axis_center")
            .and_then(|entity| match entity {
                opencad_sketch::SketchEntity::Point(point) => match point.x {
                    opencad_sketch::Coord::Literal(value) => Some(value),
                    _ => None,
                },
                _ => None,
            })
            .expect("PCD axis center x");
        assert!(((center_x - axis_x).abs() - 0.050).abs() < 1e-5);
        assert!((axis_x - 0.070).abs() < 1e-5, "axis center x {axis_x}");
    }
}
