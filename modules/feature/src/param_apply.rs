//! Apply parametric graph values to sketches before regeneration.

use indexmap::IndexMap;

use opencad_core::{Expression, Length, OpenCadError, Result};
use opencad_graph::{eval_length_expr, evaluate_param_graph, ParamGraph};
use opencad_geometry::ExtrudeExtent;
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
        FeatureDefinition::LinearPattern(pattern) => {
            if let Some(expr) = &pattern.spacing_expr {
                let meters = eval_length_expr(expr, values)?;
                pattern.spacing = Length::from_meters(meters);
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
    for constraint in &mut sketch.constraints {
        match constraint {
            Constraint::Distance { expr, .. }
            | Constraint::Radius { expr, .. }
            | Constraint::Diameter { expr, .. } => {
                *expr = resolve_expression(expr, scope)?;
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
            | Constraint::Diameter { expr, .. } => Some((index, expr.clone())),
            _ => None,
        })
        .collect()
}

fn restore_dimension_exprs(sketch: &mut Sketch, originals: &[(usize, Expression)]) {
    for (index, expr) in originals {
        if let Some(
            Constraint::Distance { expr: target, .. }
            | Constraint::Radius { expr: target, .. }
            | Constraint::Diameter { expr: target, .. },
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
}
