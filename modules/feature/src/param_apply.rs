//! Apply parametric graph values to sketches before regeneration.

use indexmap::IndexMap;

use opencad_core::{Expression, Length, OpenCadError, Result};
use opencad_geometry::ExtrudeExtent;
use opencad_graph::{eval_angle_expr, eval_length_expr, evaluate_param_graph, ParamGraph};
use opencad_sketch::{solve_sketch, Constraint, Sketch};
use opencad_solver::SolverOptions;

use crate::feature::{FeatureDefinition, FeatureNode};
use crate::regenerate::PartModel;

/// A unit-bearing circle form needs solving even with an empty parameter graph.
/// Only the disposable regeneration model is resolved; authored source stays intact.
pub(crate) fn has_circle_form_source(model: &PartModel) -> bool {
    use opencad_sketch::{Coord, SketchEntity};
    model.sketches.values().any(|sketch| {
        sketch.entities.iter().any(|entity| {
            let SketchEntity::Circle(circle) = entity else { return false; };
            matches!(circle.radius, Coord::Expr(_))
                || matches!(sketch.find_entity(circle.center.as_str()), Some(SketchEntity::Point(point))
                    if matches!(point.x, Coord::Expr(_)) || matches!(point.y, Coord::Expr(_)))
        })
    })
}

pub fn apply_parameters(model: &mut PartModel, parameters: &ParamGraph) -> Result<()> {
    let guarded_scope = if model.nodes.values().any(|n| {
        !n.suppressed
            && matches!(
                n.definition,
                FeatureDefinition::NominalMetricThread(_)
                    | FeatureDefinition::FacetedCircularFillet(_)
            )
    }) {
        opencad_graph::evaluate_param_graph_strict(parameters)?
    } else {
        IndexMap::new()
    };
    let nodes: Vec<_> = model.nodes.values().collect();
    let sketches: Vec<_> = model.sketches.values().collect();
    for node in &nodes {
        if !node.suppressed {
            if let FeatureDefinition::FacetedCircularFillet(def) = &node.definition {
                def.validate_source(&nodes, &sketches, &guarded_scope)?;
            }
            if let FeatureDefinition::NominalMetricThread(def) = &node.definition {
                def.validate_source(&nodes, &sketches, &guarded_scope)?;
            }
        }
    }

    let contains_cubic = model.sketches.values().any(|s| {
        s.entities
            .iter()
            .any(|e| matches!(e, opencad_sketch::SketchEntity::CubicBezier(_)))
    });
    if parameters.evaluation_order()?.is_empty()
        && !contains_cubic
        && !has_circle_form_source(model)
        && !model.nodes.values().any(|n| {
            matches!(
                n.definition,
                FeatureDefinition::NominalMetricThread(_)
                    | FeatureDefinition::FacetedCircularFillet(_)
            )
        })
    {
        for node in model.nodes.values_mut() {
            if matches!(
                node.definition,
                FeatureDefinition::DatumPlane(_)
                    | FeatureDefinition::DatumAxis(_)
                    | FeatureDefinition::Draft(_)
                    | FeatureDefinition::CylindricalDraft(_)
                    | FeatureDefinition::NominalMetricThread(_)
                    | FeatureDefinition::FacetedCircularFillet(_)
                    | FeatureDefinition::TaperedRib(_)
                    | FeatureDefinition::Rib(_)
                    | FeatureDefinition::SeatedStandardHole(_)
            ) {
                resolve_feature_node(node, &IndexMap::new())?;
            }
        }
        return Ok(());
    }
    let values = if model.nodes.values().any(|n| {
        matches!(
            n.definition,
            FeatureDefinition::SeatedStandardHole(_)
                | FeatureDefinition::CylindricalDraft(_)
                | FeatureDefinition::NominalMetricThread(_)
                | FeatureDefinition::FacetedCircularFillet(_)
                | FeatureDefinition::TaperedRib(_)
        )
    }) || model.sketches.values().any(|s| {
        s.entities.iter().any(|e| {
            matches!(
                e,
                opencad_sketch::SketchEntity::Ellipse(_)
                    | opencad_sketch::SketchEntity::CubicBezier(_)
            )
        })
    }) {
        opencad_graph::evaluate_param_graph_strict(parameters)?
    } else {
        evaluate_param_graph(parameters)?
    };
    for sketch in model.sketches.values_mut() {
        let originals = snapshot_dimension_exprs(sketch);
        let cubic_sources = snapshot_cubic_point_sources(sketch);
        let result = resolve_sketch_constraints(sketch, &values)
            .and_then(|()| solve_sketch(sketch, &SolverOptions::default()).map(|_| ()))
            .and_then(|()| retain_cubic_solution(sketch));
        // A rejected resolved curve must not literalize authored point sources.
        restore_cubic_point_sources(sketch, &cubic_sources);
        restore_dimension_exprs(sketch, &originals);
        result?;
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

/// Resolve a copy to current sketch-local SI geometry without changing source.
pub fn solved_sketch_copy(sketch: &Sketch, values: &IndexMap<String, f64>) -> Result<Sketch> {
    let mut copy = sketch.clone();
    crate::prepare_sketch(&mut copy)?;
    resolve_sketch_constraints(&mut copy, values)?;
    solve_sketch(&mut copy, &SolverOptions::default())?;
    retain_cubic_solution(&mut copy)?;
    match copy.solve_state {
        opencad_sketch::SolveState::FullyConstrained
        | opencad_sketch::SolveState::UnderConstrained { .. } => Ok(copy),
        _ => Err(opencad_core::OpenCadError::validation(
            "sketch edit requires a successfully solved sketch",
        )),
    }
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
        FeatureDefinition::DatumPlane(def) => {
            if let Some(expr) = &def.offset_expr {
                def.offset = Length::from_meters(eval_length_expr(expr, values)?);
            }
        }
        FeatureDefinition::DatumAxis(def) => {
            if let Some(exprs) = &def.origin_expr {
                for (i, expr) in exprs.iter().enumerate() {
                    if let Some(expr) = expr {
                        def.origin_m[i] = eval_length_expr(expr, values)?;
                    }
                }
            }
        }
        FeatureDefinition::Extrude(extrude) => {
            if let Some(expr) = &extrude.length_expr {
                let meters = eval_length_expr(expr, values)?;
                extrude.extent = ExtrudeExtent::Distance {
                    length: Length::from_meters(meters),
                };
            }
        }
        FeatureDefinition::SeatedStandardHole(def) => {
            if let Some(expr) = &def.bore_depth_expr {
                def.bore_depth = Length::from_meters(crate::seated_standard_hole::evaluate_length(
                    expr, values,
                )?);
            }
            def.seat.resolve_expressions(values)?;
            def.validate_dimensions()?;
        }
        FeatureDefinition::StandardHole(def) => {
            if let Some(expr) = &def.hole.depth_expr {
                def.hole.depth = ExtrudeExtent::Distance {
                    length: Length::from_meters(eval_length_expr(expr, values)?),
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
            if let Some(crate::FilletRadiusLaw::Linear {
                end_radius,
                end_radius_expr: Some(expr),
            }) = &mut fillet.radius_law
            {
                *end_radius = Length::from_meters(eval_length_expr(expr, values)?);
            }

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
        FeatureDefinition::FacetedCircularFillet(def) => {
            def.validate()?;
        }
        FeatureDefinition::NominalMetricThread(def) => {
            def.validate()?;
        }
        FeatureDefinition::CylindricalDraft(def) => {
            def.resolve_expressions(values)?;
        }
        FeatureDefinition::Draft(def) => {
            if let Some(expr) = &def.angle_expr {
                def.angle = opencad_core::Angle::from_radians(eval_angle_expr(expr, values)?);
            }
        }
        FeatureDefinition::TaperedRib(def) => {
            def.resolve_expressions(values)?;
        }
        FeatureDefinition::Rib(rib) => {
            if let Some(expr) = &rib.thickness_expr {
                let meters = eval_length_expr(expr, values)?;
                rib.thickness = Length::from_meters(meters);
            }
            if let Some(expr) = &rib.length_expr {
                let meters = eval_length_expr(expr, values)?;
                rib.extent = ExtrudeExtent::Distance {
                    length: Length::from_meters(meters),
                };
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

type CubicPointSource = (String, opencad_sketch::Coord, opencad_sketch::Coord);
fn snapshot_cubic_point_sources(sketch: &Sketch) -> Vec<CubicPointSource> {
    let ids: std::collections::BTreeSet<_> = sketch
        .entities
        .iter()
        .filter_map(|e| match e {
            opencad_sketch::SketchEntity::CubicBezier(curve) => {
                Some(curve.control_points[..3].iter().map(|id| id.as_str()))
            }
            _ => None,
        })
        .flatten()
        .collect();
    sketch
        .entities
        .iter()
        .filter_map(|e| match e {
            opencad_sketch::SketchEntity::Point(p) if ids.contains(p.base.id.as_str()) => {
                Some((p.base.id.as_str().into(), p.x.clone(), p.y.clone()))
            }
            _ => None,
        })
        .collect()
}
fn retain_cubic_solution(sketch: &mut Sketch) -> Result<()> {
    let controls: Vec<_> = sketch
        .entities
        .iter()
        .filter_map(|e| match e {
            opencad_sketch::SketchEntity::CubicBezier(curve) => Some((
                curve.base.id.as_str().to_owned(),
                opencad_sketch::solve::cubic_control_points(sketch, curve),
            )),
            _ => None,
        })
        .collect();
    for (id, controls) in controls {
        let controls = controls?;
        opencad_geometry::SolvedCubicBezier {
            controls_m: controls,
        }
        .validate()?;
        if let Some(opencad_sketch::SketchEntity::CubicBezier(curve)) =
            sketch.entities.iter_mut().find(|e| e.id().as_str() == id)
        {
            curve.resolved_controls_m = Some(controls);
        }
    }
    Ok(())
}
fn restore_cubic_point_sources(sketch: &mut Sketch, sources: &[CubicPointSource]) {
    for (id, x, y) in sources {
        if let Some(opencad_sketch::SketchEntity::Point(p)) =
            sketch.entities.iter_mut().find(|e| e.id().as_str() == id)
        {
            p.x = x.clone();
            p.y = y.clone();
        }
    }
}

pub(crate) fn resolve_sketch_constraints(
    sketch: &mut Sketch,
    scope: &IndexMap<String, f64>,
) -> Result<()> {
    for entity in &mut sketch.entities {
        if let opencad_sketch::SketchEntity::CubicBezier(curve) = entity {
            curve.resolved_controls_m = None;
        }
    }
    let cubic_points: std::collections::BTreeSet<_> = sketch
        .entities
        .iter()
        .filter_map(|entity| match entity {
            opencad_sketch::SketchEntity::CubicBezier(e) => Some(
                e.control_points[..3]
                    .iter()
                    .map(|id| id.as_str().to_owned()),
            ),
            _ => None,
        })
        .flatten()
        .collect();
    for entity in &mut sketch.entities {
        if let opencad_sketch::SketchEntity::Point(point) = entity {
            if cubic_points.contains(point.base.id.as_str()) {
                for coord in [&mut point.x, &mut point.y] {
                    if let opencad_sketch::Coord::Expr(expr) = coord {
                        if opencad_graph::parameter_names_in_expr(expr.as_str())
                            .iter()
                            .any(|name| opencad_graph::param_eval::is_angle_parameter(name))
                        {
                            return Err(OpenCadError::validation(
                                "cubic coordinates cannot reference angle parameters",
                            ));
                        }
                        *coord = opencad_sketch::Coord::Literal(
                            opencad_graph::eval_length_expr_strict(expr.as_str(), scope)?,
                        );
                    }
                }
            }
        }
    }
    for entity in &mut sketch.entities {
        if let opencad_sketch::SketchEntity::Ellipse(e) = entity {
            e.resolved_parameters = Some(resolve_ellipse_parameters(e, scope)?);
        }
    }
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

/// Resolve analytic source dimensions without mutating the authored ellipse.
pub fn resolve_ellipse_parameters(
    e: &opencad_sketch::EllipseEntity,
    scope: &IndexMap<String, f64>,
) -> Result<[f64; 3]> {
    for (coord, angle) in [
        (&e.major_radius, false),
        (&e.minor_radius, false),
        (&e.axis_angle, true),
    ] {
        if let opencad_sketch::Coord::Expr(expr) = coord {
            for name in opencad_graph::parameter_names_in_expr(expr.as_str()) {
                if opencad_graph::param_eval::is_angle_parameter(&name) != angle {
                    return Err(OpenCadError::InvalidExpression(format!(
                        "ellipse field parameter kind mismatch: {name}"
                    )));
                }
            }
        }
    }
    let length = |coord: &opencad_sketch::Coord| match coord {
        opencad_sketch::Coord::Literal(v) => Ok(*v),
        opencad_sketch::Coord::Expr(expr) => {
            opencad_graph::eval_length_expr_strict(expr.as_str(), scope)
        }
    };
    let angle = match &e.axis_angle {
        opencad_sketch::Coord::Literal(v) => *v,
        opencad_sketch::Coord::Expr(expr) => {
            opencad_graph::eval_angle_expr_strict(expr.as_str(), scope)?
        }
    };
    let values = [length(&e.major_radius)?, length(&e.minor_radius)?, angle];
    let mut resolved = e.clone();
    resolved.resolved_parameters = Some(values);
    opencad_sketch::solve::ellipse_parameters(&resolved)
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
