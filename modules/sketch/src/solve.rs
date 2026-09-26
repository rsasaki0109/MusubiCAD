use indexmap::IndexMap;

use opencad_core::{OpenCadError, Result};
use opencad_solver::{
    point_x, point_y, radius_var, solve_with_diagnostics, ConstraintResidual, LengthTerm,
    SolveStatus, SolverOptions, VarSet, VariableRegistry, DIRECTION_DEGENERACY_TOLERANCE_M,
};

use crate::constraint::{Constraint, DistanceTarget, EntityRef, EqualTarget, LineEnd};
use crate::entity::{Coord, LineEntity, PointEntity, SketchEntity};
use crate::solve_state::SolveState;
use crate::Sketch;

type SketchProblem = (
    Vec<ConstraintResidual>,
    VariableRegistry,
    IndexMap<String, (f64, f64)>,
);

/// Solve sketch constraints and write coordinates back into point entities.
pub fn solve_sketch(sketch: &mut Sketch, options: &SolverOptions) -> Result<SolveStatus> {
    let (mut equations, registry, point_coords) = build_problem(sketch)?;

    // Anchor the first point to remove translation DOF.
    if let Some((id, (x, y))) = point_coords.iter().next() {
        if let (Some(x_id), Some(y_id)) = (
            registry.get(&format!("{id}.x")),
            registry.get(&format!("{id}.y")),
        ) {
            equations.push(ConstraintResidual::FixedX { x: x_id, value: *x });
            equations.push(ConstraintResidual::FixedY { y: y_id, value: *y });
        }
    }

    if equations.is_empty() {
        sketch.solve_state = SolveState::UnderConstrained {
            dof: registry.len() as i32,
        };
        return Ok(SolveStatus::UnderConstrained {
            dof: registry.len() as i32,
            iterations: 0,
            max_error: 0.0,
        });
    }

    let mut values = registry.initial_values();
    for (key, (x, y)) in &point_coords {
        if let Some(x_id) = registry.get(&format!("{key}.x")) {
            values[x_id.index()] = *x;
        }
        if let Some(y_id) = registry.get(&format!("{key}.y")) {
            values[y_id.index()] = *y;
        }
    }
    seed_radius_values(&registry, &mut values, sketch);

    let vars = VarSet::new(values);
    let (output, status) = solve_with_diagnostics(&equations, vars, options);
    if matches!(
        status,
        SolveStatus::Solved { .. }
            | SolveStatus::UnderConstrained { .. }
            | SolveStatus::OverConstrained { .. }
    ) {
        validate_solved_direction_lines(sketch, &registry, &output.vars)?;
        apply_solution(sketch, &registry, &output.vars)?;
    }
    sketch.solve_state = map_status(&status);
    Ok(status)
}

fn seed_radius_values(registry: &VariableRegistry, values: &mut [f64], sketch: &Sketch) {
    for entity in &sketch.entities {
        let (id, radius) = match entity {
            SketchEntity::Circle(circle) => (&circle.base.id, &circle.radius),
            SketchEntity::Arc(arc) => (&arc.base.id, &arc.radius),
            _ => continue,
        };
        let Some(r_id) = registry.get(&format!("{}.radius", id.as_str())) else {
            continue;
        };
        // Radius expressions may be simple unit-bearing literals (for example,
        // `80 mm`).  Symbolic expressions are evaluated by a higher-level
        // parameter context and retain the existing zero seed here.
        if let Ok(r) = coord_literal(radius) {
            values[r_id.index()] = r;
        }
    }
}

fn map_status(status: &SolveStatus) -> SolveState {
    match status {
        SolveStatus::Solved { .. } => SolveState::FullyConstrained,
        SolveStatus::UnderConstrained { dof, .. } => SolveState::UnderConstrained { dof: *dof },
        SolveStatus::OverConstrained { redundant, .. } => SolveState::OverConstrained {
            redundant: *redundant,
        },
        SolveStatus::Contradictory { message, .. } => SolveState::Failed {
            message: message.clone(),
        },
        SolveStatus::NonConvergent { message, .. } => SolveState::Failed {
            message: message.clone(),
        },
        SolveStatus::Failed { message, .. } => SolveState::Failed {
            message: message.clone(),
        },
    }
}

fn build_problem(sketch: &Sketch) -> Result<SketchProblem> {
    let mut registry = VariableRegistry::new();
    let mut point_coords = IndexMap::new();
    let lines: IndexMap<String, &LineEntity> = sketch
        .entities
        .iter()
        .filter_map(|e| match e {
            SketchEntity::Line(l) => Some((l.base.id.as_str().to_string(), l)),
            _ => None,
        })
        .collect();

    for entity in &sketch.entities {
        match entity {
            SketchEntity::Point(p) => {
                register_point(&mut registry, &mut point_coords, p)?;
            }
            SketchEntity::Line(l) => {
                register_point_ref(&mut registry, &mut point_coords, sketch, &l.start)?;
                register_point_ref(&mut registry, &mut point_coords, sketch, &l.end)?;
            }
            SketchEntity::Circle(c) => {
                register_point_ref(&mut registry, &mut point_coords, sketch, &c.center)?;
                radius_var(&mut registry, c.base.id.as_str());
            }
            SketchEntity::Arc(a) => {
                register_point_ref(&mut registry, &mut point_coords, sketch, &a.center)?;
                radius_var(&mut registry, a.base.id.as_str());
                for point in [&a.start_point, &a.end_point].into_iter().flatten() {
                    register_point_ref(&mut registry, &mut point_coords, sketch, point)?;
                }
            }
            SketchEntity::Rectangle(_) => {}
        }
    }

    let mut equations = Vec::new();
    for constraint in &sketch.constraints {
        build_constraint(
            constraint,
            &mut equations,
            &registry,
            &lines,
            &point_coords,
            sketch,
        )?;
    }
    arc_endpoint_equations(sketch, &registry, &mut equations)?;

    Ok((equations, registry, point_coords))
}

fn register_point(
    registry: &mut VariableRegistry,
    coords: &mut IndexMap<String, (f64, f64)>,
    point: &PointEntity,
) -> Result<()> {
    let id = point.base.id.as_str();
    point_x(registry, id);
    point_y(registry, id);
    let x = coord_literal(&point.x)?;
    let y = coord_literal(&point.y)?;
    coords.insert(id.to_string(), (x, y));
    Ok(())
}

fn register_point_ref(
    registry: &mut VariableRegistry,
    coords: &mut IndexMap<String, (f64, f64)>,
    sketch: &Sketch,
    point_id: &opencad_core::EntityId,
) -> Result<()> {
    let id = point_id.as_str();
    point_x(registry, id);
    point_y(registry, id);
    if !coords.contains_key(id) {
        if let Some(p) = find_point(sketch, id) {
            let x = coord_literal(&p.x)?;
            let y = coord_literal(&p.y)?;
            coords.insert(id.to_string(), (x, y));
        } else {
            coords.insert(id.to_string(), (0.0, 0.0));
        }
    }
    Ok(())
}

fn find_point<'a>(sketch: &'a Sketch, id: &str) -> Option<&'a PointEntity> {
    sketch.entities.iter().find_map(|e| match e {
        SketchEntity::Point(p) if p.base.id.as_str() == id => Some(p),
        _ => None,
    })
}

fn build_constraint(
    constraint: &Constraint,
    equations: &mut Vec<ConstraintResidual>,
    registry: &VariableRegistry,
    lines: &IndexMap<String, &LineEntity>,
    point_coords: &IndexMap<String, (f64, f64)>,
    sketch: &Sketch,
) -> Result<()> {
    match constraint {
        Constraint::Coincident { a, b, .. } => {
            let (ax, ay) = entity_ref_xy(registry, lines, sketch, a)?;
            let (bx, by) = entity_ref_xy(registry, lines, sketch, b)?;
            equations.extend(ConstraintResidual::coincident(ax, ay, bx, by));
        }
        Constraint::Horizontal { line, .. } => {
            let (x1, y1, x2, y2) = line_endpoints(registry, lines, line.as_str())?;
            equations.push(ConstraintResidual::Horizontal { x1, y1, x2, y2 });
        }
        Constraint::Vertical { line, .. } => {
            let (x1, y1, x2, y2) = line_endpoints(registry, lines, line.as_str())?;
            equations.push(ConstraintResidual::Vertical { x1, y1, x2, y2 });
        }
        Constraint::Distance { target, expr, .. } => match target {
            DistanceTarget::PointToPoint { a, b } => {
                let (x1, y1) = point_xy(registry, a.as_str())?;
                let (x2, y2) = point_xy(registry, b.as_str())?;
                equations.push(ConstraintResidual::Distance {
                    x1,
                    y1,
                    x2,
                    y2,
                    target: parse_length_expr(expr.as_str())?,
                });
            }
            DistanceTarget::LineLength { line } => {
                let (x1, y1, x2, y2) = line_endpoints(registry, lines, line.as_str())?;
                equations.push(ConstraintResidual::Distance {
                    x1,
                    y1,
                    x2,
                    y2,
                    target: parse_length_expr(expr.as_str())?,
                });
            }
            DistanceTarget::RectangleDimension { rectangle, edge } => {
                let rect = sketch
                    .entities
                    .iter()
                    .find(|e| e.id().as_str() == rectangle.as_str())
                    .ok_or_else(|| OpenCadError::not_found(format!("rectangle '{rectangle}'")))?;
                if let SketchEntity::Rectangle(r) = rect {
                    let target = parse_length_expr(expr.as_str())?;
                    match edge {
                        crate::constraint::RectangleEdge::Width => {
                            let (x1, y1) = point_xy(registry, r.corner_ids[0].as_str())?;
                            let (x2, y2) = point_xy(registry, r.corner_ids[1].as_str())?;
                            equations.push(ConstraintResidual::Distance {
                                x1,
                                y1,
                                x2,
                                y2,
                                target,
                            });
                        }
                        crate::constraint::RectangleEdge::Height => {
                            let (x1, y1) = point_xy(registry, r.corner_ids[0].as_str())?;
                            let (x2, y2) = point_xy(registry, r.corner_ids[3].as_str())?;
                            equations.push(ConstraintResidual::Distance {
                                x1,
                                y1,
                                x2,
                                y2,
                                target,
                            });
                        }
                    }
                }
            }
        },
        Constraint::Radius { target, expr, .. } => {
            let radius = registry
                .get(&format!("{}.radius", target.as_str()))
                .ok_or_else(|| {
                    OpenCadError::not_found(format!("radius for '{}'", target.as_str()))
                })?;
            equations.push(ConstraintResidual::Radius {
                radius,
                target: parse_length_expr(expr.as_str())?,
            });
        }
        Constraint::Diameter { target, expr, .. } => {
            let radius = registry
                .get(&format!("{}.radius", target.as_str()))
                .ok_or_else(|| {
                    OpenCadError::not_found(format!("radius for '{}'", target.as_str()))
                })?;
            equations.push(ConstraintResidual::Radius {
                radius,
                target: parse_length_expr(expr.as_str())? / 2.0,
            });
        }
        Constraint::Equal { a, b, .. } => {
            let a = equal_target_length(a, registry, lines, sketch)?;
            let b = equal_target_length(b, registry, lines, sketch)?;
            equations.push(ConstraintResidual::EqualLength { a, b });
        }
        Constraint::Parallel { line_a, line_b, .. } => {
            let a = line_endpoints(registry, lines, line_a.as_str())?;
            let b = line_endpoints(registry, lines, line_b.as_str())?;
            validate_direction_line(line_a.as_str(), lines, point_coords)?;
            validate_direction_line(line_b.as_str(), lines, point_coords)?;
            equations.push(ConstraintResidual::Parallel {
                ax1: a.0,
                ay1: a.1,
                ax2: a.2,
                ay2: a.3,
                bx1: b.0,
                by1: b.1,
                bx2: b.2,
                by2: b.3,
            });
        }
        Constraint::Perpendicular { line_a, line_b, .. } => {
            let a = line_endpoints(registry, lines, line_a.as_str())?;
            let b = line_endpoints(registry, lines, line_b.as_str())?;
            validate_direction_line(line_a.as_str(), lines, point_coords)?;
            validate_direction_line(line_b.as_str(), lines, point_coords)?;
            equations.push(ConstraintResidual::Perpendicular {
                ax1: a.0,
                ay1: a.1,
                ax2: a.2,
                ay2: a.3,
                bx1: b.0,
                by1: b.1,
                bx2: b.2,
                by2: b.3,
            });
        }
        Constraint::Angle {
            line_a,
            line_b,
            expr,
            ..
        } => {
            let a = line_endpoints(registry, lines, line_a.as_str())?;
            let b = line_endpoints(registry, lines, line_b.as_str())?;
            validate_direction_line(line_a.as_str(), lines, point_coords)?;
            validate_direction_line(line_b.as_str(), lines, point_coords)?;
            equations.push(ConstraintResidual::Angle {
                ax1: a.0,
                ay1: a.1,
                ax2: a.2,
                ay2: a.3,
                bx1: b.0,
                by1: b.1,
                bx2: b.2,
                by2: b.3,
                target_rad: parse_angle_expr(expr.as_str())?,
            });
        }
        Constraint::Midpoint { point, line, .. } => {
            let (px, py) = point_xy(registry, point.as_str())?;
            let endpoints = line_endpoints(registry, lines, line.as_str())?;
            equations.extend(ConstraintResidual::midpoint(px, py, endpoints));
        }
        Constraint::Symmetric { a, b, line, .. } => {
            let (px, py) = point_xy(registry, a.as_str())?;
            let (qx, qy) = point_xy(registry, b.as_str())?;
            let (lx1, ly1, lx2, ly2) = line_endpoints(registry, lines, line.as_str())?;
            validate_direction_line(line.as_str(), lines, point_coords)?;
            equations.push(ConstraintResidual::SymmetricMidpointOnLine {
                px,
                py,
                qx,
                qy,
                lx1,
                ly1,
                lx2,
                ly2,
            });
            equations.push(ConstraintResidual::SymmetricPerpendicular {
                px,
                py,
                qx,
                qy,
                lx1,
                ly1,
                lx2,
                ly2,
            });
        }
        Constraint::Tangent { line, curve, .. } => {
            let (lx1, ly1, lx2, ly2) = line_endpoints(registry, lines, line.as_str())?;
            validate_direction_line(line.as_str(), lines, point_coords)?;
            let center = sketch
                .entities
                .iter()
                .find_map(|entity| match entity {
                    SketchEntity::Circle(circle) if circle.base.id == *curve => {
                        Some(&circle.center)
                    }
                    SketchEntity::Arc(arc) if arc.base.id == *curve => Some(&arc.center),
                    _ => None,
                })
                .ok_or_else(|| {
                    OpenCadError::validation(format!(
                        "tangent target '{curve}' must be a circle or arc"
                    ))
                })?;
            let (cx, cy) = point_xy(registry, center.as_str())?;
            let radius = registry
                .get(&format!("{}.radius", curve.as_str()))
                .ok_or_else(|| OpenCadError::not_found(format!("radius for '{curve}'")))?;
            equations.push(ConstraintResidual::TangentLineCircle {
                cx,
                cy,
                radius,
                lx1,
                ly1,
                lx2,
                ly2,
            });
        }
    }
    Ok(())
}

/// Reject a line whose initial direction cannot be normalized reliably.
///
/// Direction residuals are dimensionless, but their normalization uses the
/// product of both line lengths.  The line-length tolerance is expressed in
/// internal SI meters and is intentionally shared with the numeric solver.
fn validate_direction_line(
    line_id: &str,
    lines: &IndexMap<String, &LineEntity>,
    point_coords: &IndexMap<String, (f64, f64)>,
) -> Result<()> {
    let line = lines
        .get(line_id)
        .ok_or_else(|| OpenCadError::not_found(format!("line '{line_id}'")))?;
    let (start_x, start_y) = point_coords.get(line.start.as_str()).ok_or_else(|| {
        OpenCadError::not_found(format!("line '{line_id}' start point '{}'", line.start))
    })?;
    let (end_x, end_y) = point_coords.get(line.end.as_str()).ok_or_else(|| {
        OpenCadError::not_found(format!("line '{line_id}' end point '{}'", line.end))
    })?;
    let length = (end_x - start_x).hypot(end_y - start_y);
    validate_direction_length(line_id, length)
}

fn validate_solved_direction_lines(
    sketch: &Sketch,
    registry: &VariableRegistry,
    vars: &VarSet,
) -> Result<()> {
    for constraint in &sketch.constraints {
        let (line_a, line_b) = match constraint {
            Constraint::Parallel { line_a, line_b, .. }
            | Constraint::Perpendicular { line_a, line_b, .. }
            | Constraint::Angle { line_a, line_b, .. } => (line_a, line_b),
            _ => continue,
        };
        for line_id in [line_a, line_b] {
            let line = match sketch.find_entity(line_id.as_str()) {
                Some(SketchEntity::Line(line)) => line,
                _ => {
                    return Err(OpenCadError::validation(format!(
                        "direction constraint target '{}' must reference a line",
                        line_id.as_str()
                    )))
                }
            };
            let (x1, y1) = point_xy(registry, line.start.as_str())?;
            let (x2, y2) = point_xy(registry, line.end.as_str())?;
            let length = (vars.get(x2) - vars.get(x1)).hypot(vars.get(y2) - vars.get(y1));
            validate_direction_length(line_id.as_str(), length)?;
        }
    }
    Ok(())
}

fn validate_direction_length(line_id: &str, length: f64) -> Result<()> {
    if !length.is_finite() || length <= DIRECTION_DEGENERACY_TOLERANCE_M {
        return Err(OpenCadError::validation(format!(
            "direction constraint line '{line_id}' is zero-length or below the direction degeneracy tolerance ({DIRECTION_DEGENERACY_TOLERANCE_M:.1e} m); got {length:.6e} m"
        )));
    }
    Ok(())
}

/// Resolve an equal target to a length-valued solver term.
///
/// Both line lengths and circle/arc radii are lengths in internal SI units,
/// so mixed equal constraints are intentionally supported.  The target kind
/// is still checked against the referenced sketch entity to avoid silently
/// treating (for example) a circle as a line or a point as a radius.
fn equal_target_length(
    target: &EqualTarget,
    registry: &VariableRegistry,
    lines: &IndexMap<String, &LineEntity>,
    sketch: &Sketch,
) -> Result<LengthTerm> {
    match target {
        EqualTarget::LineLength(line_id) => {
            let entity = sketch.find_entity(line_id.as_str()).ok_or_else(|| {
                OpenCadError::not_found(format!("equal line-length target '{}'", line_id.as_str()))
            })?;
            if !matches!(entity, SketchEntity::Line(_)) {
                return Err(OpenCadError::validation(format!(
                    "equal line-length target '{}' must reference a line",
                    line_id.as_str()
                )));
            }
            let (x1, y1, x2, y2) = line_endpoints(registry, lines, line_id.as_str())?;
            Ok(LengthTerm::Segment { x1, y1, x2, y2 })
        }
        EqualTarget::Radius(entity_id) => {
            let entity = sketch.find_entity(entity_id.as_str()).ok_or_else(|| {
                OpenCadError::not_found(format!("equal radius target '{}'", entity_id.as_str()))
            })?;
            if !matches!(entity, SketchEntity::Circle(_) | SketchEntity::Arc(_)) {
                return Err(OpenCadError::validation(format!(
                    "equal radius target '{}' must reference a circle or arc",
                    entity_id.as_str()
                )));
            }
            let radius = registry
                .get(&format!("{}.radius", entity_id.as_str()))
                .ok_or_else(|| {
                    OpenCadError::not_found(format!(
                        "radius variable for equal target '{}'",
                        entity_id.as_str()
                    ))
                })?;
            Ok(LengthTerm::Scalar { value: radius })
        }
    }
}

fn entity_ref_xy(
    registry: &VariableRegistry,
    lines: &IndexMap<String, &LineEntity>,
    _sketch: &Sketch,
    reference: &EntityRef,
) -> Result<(opencad_solver::VarId, opencad_solver::VarId)> {
    match reference {
        EntityRef::Entity(id) => point_xy(registry, id.as_str()),
        EntityRef::PointOnLine { line, end } => {
            let line_ent = lines
                .get(line.as_str())
                .ok_or_else(|| OpenCadError::not_found(format!("line '{}'", line.as_str())))?;
            let point_id = match end {
                LineEnd::Start => line_ent.start.as_str(),
                LineEnd::End => line_ent.end.as_str(),
            };
            point_xy(registry, point_id)
        }
    }
}

fn point_xy(
    registry: &VariableRegistry,
    point_id: &str,
) -> Result<(opencad_solver::VarId, opencad_solver::VarId)> {
    let x = registry
        .get(&format!("{point_id}.x"))
        .ok_or_else(|| OpenCadError::not_found(format!("point x '{point_id}'")))?;
    let y = registry
        .get(&format!("{point_id}.y"))
        .ok_or_else(|| OpenCadError::not_found(format!("point y '{point_id}'")))?;
    Ok((x, y))
}

fn line_endpoints(
    registry: &VariableRegistry,
    lines: &IndexMap<String, &LineEntity>,
    line_id: &str,
) -> Result<(
    opencad_solver::VarId,
    opencad_solver::VarId,
    opencad_solver::VarId,
    opencad_solver::VarId,
)> {
    let line = lines
        .get(line_id)
        .ok_or_else(|| OpenCadError::not_found(format!("line '{line_id}'")))?;
    let (x1, y1) = point_xy(registry, line.start.as_str())?;
    let (x2, y2) = point_xy(registry, line.end.as_str())?;
    Ok((x1, y1, x2, y2))
}

/// Hold each arc's endpoint points on the arc (ADR-021).
fn arc_endpoint_equations(
    sketch: &Sketch,
    registry: &VariableRegistry,
    equations: &mut Vec<ConstraintResidual>,
) -> Result<()> {
    for entity in &sketch.entities {
        let SketchEntity::Arc(arc) = entity else {
            continue;
        };
        let var = |name: String| {
            registry.get(&name).ok_or_else(|| {
                OpenCadError::validation(format!("unknown sketch variable '{name}'"))
            })
        };
        let (cx, cy) = (
            var(format!("{}.x", arc.center.as_str()))?,
            var(format!("{}.y", arc.center.as_str()))?,
        );
        let radius = var(format!("{}.radius", arc.base.id.as_str()))?;
        for (point, angle) in [
            (&arc.start_point, &arc.start_angle),
            (&arc.end_point, &arc.end_angle),
        ] {
            let Some(point) = point else {
                continue;
            };
            let angle = resolved_or(arc, angle)?;
            equations.push(ConstraintResidual::ArcEndpointX {
                px: var(format!("{}.x", point.as_str()))?,
                cx,
                radius,
                cos: angle.cos(),
            });
            equations.push(ConstraintResidual::ArcEndpointY {
                py: var(format!("{}.y", point.as_str()))?,
                cy,
                radius,
                sin: angle.sin(),
            });
        }
    }
    Ok(())
}

/// Start and end angles of an arc in radians: the values resolved from
/// parameter expressions before solving (ADR-024), otherwise literals or
/// constant angle expressions.
pub fn arc_angles(arc: &crate::entity::ArcEntity) -> Result<(f64, f64)> {
    if let Some([start, end]) = arc.resolved_angles_rad {
        return Ok((start, end));
    }
    Ok((
        arc_angle(arc, &arc.start_angle)?,
        arc_angle(arc, &arc.end_angle)?,
    ))
}

/// One of `arc`'s angles, preferring the parameter-resolved value.
fn resolved_or(arc: &crate::entity::ArcEntity, angle: &Coord) -> Result<f64> {
    match arc.resolved_angles_rad {
        Some([start, end]) => Ok(if std::ptr::eq(angle, &arc.start_angle) {
            start
        } else {
            end
        }),
        None => arc_angle(arc, angle),
    }
}

/// An arc angle in radians: a literal, or a constant angle expression such
/// as `90 deg`.  Parameter-driven angles are resolved beforehand into
/// `resolved_angles_rad` (ADR-024).
pub fn arc_angle(arc: &crate::entity::ArcEntity, angle: &Coord) -> Result<f64> {
    match angle {
        Coord::Literal(value) => Ok(*value),
        Coord::Expr(expr) => parse_angle_expr(expr.as_str()).map_err(|_| {
            OpenCadError::validation(format!(
                "arc '{}' angle '{}' must be a literal or constant angle",
                arc.base.id.as_str(),
                expr.as_str()
            ))
        }),
    }
}

fn coord_literal(coord: &Coord) -> Result<f64> {
    match coord {
        Coord::Literal(v) => Ok(*v),
        Coord::Expr(expr) => parse_length_expr(expr.as_str()),
    }
}

/// Parse simple angle literals in radians: `0.5`, `0.5 rad`, `30 deg`.
pub fn parse_angle_expr(expr: &str) -> Result<f64> {
    let trimmed = expr.trim();
    let (value, unit) = trimmed
        .split_once(char::is_whitespace)
        .map(|(value, unit)| (value.trim(), unit.trim()))
        .unwrap_or((trimmed, "rad"));
    let value: f64 = value
        .parse()
        .map_err(|_| OpenCadError::InvalidExpression(expr.into()))?;
    match unit {
        "rad" => Ok(value),
        "deg" => Ok(value.to_radians()),
        _ => Err(OpenCadError::InvalidExpression(expr.into())),
    }
}

/// Parse simple length literals: `80`, `80 mm`, `0.08 m`.
pub fn parse_length_expr(expr: &str) -> Result<f64> {
    let trimmed = expr.trim();
    if let Some((value, unit)) = trimmed.split_once(char::is_whitespace) {
        let value: f64 = value
            .trim()
            .parse()
            .map_err(|_| OpenCadError::InvalidExpression(expr.into()))?;
        return Ok(convert_length(value, unit.trim()));
    }
    trimmed
        .parse::<f64>()
        .map_err(|_| OpenCadError::InvalidExpression(expr.into()))
}

fn convert_length(value: f64, unit: &str) -> f64 {
    match unit {
        "m" => value,
        "mm" => value * 0.001,
        "cm" => value * 0.01,
        "in" => value * 0.0254,
        _ => value,
    }
}

fn apply_solution(sketch: &mut Sketch, registry: &VariableRegistry, vars: &VarSet) -> Result<()> {
    for entity in &mut sketch.entities {
        match entity {
            SketchEntity::Point(point) => {
                let id = point.base.id.as_str();
                if let Some(x_id) = registry.get(&format!("{id}.x")) {
                    point.x = Coord::Literal(vars.get(x_id));
                }
                if let Some(y_id) = registry.get(&format!("{id}.y")) {
                    point.y = Coord::Literal(vars.get(y_id));
                }
            }
            SketchEntity::Circle(circle) => {
                if let Some(r_id) = registry.get(&format!("{}.radius", circle.base.id.as_str())) {
                    circle.radius = Coord::Literal(vars.get(r_id));
                }
            }
            SketchEntity::Arc(arc) => {
                if let Some(r_id) = registry.get(&format!("{}.radius", arc.base.id.as_str())) {
                    arc.radius = Coord::Literal(vars.get(r_id));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::{Constraint, DistanceTarget, EntityRef, EqualTarget};
    use crate::entity::{ArcEntity, CircleEntity, EntityBase, LineEntity, PointEntity};
    use crate::workplane::Workplane;
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};

    fn rectangle_sketch() -> Sketch {
        let mut sketch = Sketch::new(
            SketchId::new("sketch:rect").expect("id"),
            "Rectangle",
            Workplane::xy(),
        );

        let corners = ["ent:c0", "ent:c1", "ent:c2", "ent:c3"];
        let edges = ["ent:e0", "ent:e1", "ent:e2", "ent:e3"];

        for (id, x, y) in [
            (corners[0], 0.0, 0.0),
            (corners[1], 70.0, 0.0),
            (corners[2], 70.0, 50.0),
            (corners[3], 0.0, 50.0),
        ] {
            sketch
                .add_entity(SketchEntity::Point(PointEntity {
                    base: EntityBase {
                        id: EntityId::new(id).expect("id"),
                        construction: false,
                    },
                    x: Coord::literal(x),
                    y: Coord::literal(y),
                }))
                .expect("point");
        }

        for (id, start, end) in [
            (edges[0], corners[0], corners[1]),
            (edges[1], corners[1], corners[2]),
            (edges[2], corners[2], corners[3]),
            (edges[3], corners[3], corners[0]),
        ] {
            sketch
                .add_entity(SketchEntity::Line(LineEntity {
                    base: EntityBase {
                        id: EntityId::new(id).expect("id"),
                        construction: false,
                    },
                    start: EntityId::new(start).expect("id"),
                    end: EntityId::new(end).expect("id"),
                }))
                .expect("line");
        }

        sketch
            .add_constraint(Constraint::Horizontal {
                id: ConstraintId::new("con:h0").expect("id"),
                line: EntityId::new(edges[0]).expect("id"),
            })
            .expect("h");
        sketch
            .add_constraint(Constraint::Horizontal {
                id: ConstraintId::new("con:h1").expect("id"),
                line: EntityId::new(edges[2]).expect("id"),
            })
            .expect("h");
        sketch
            .add_constraint(Constraint::Vertical {
                id: ConstraintId::new("con:v0").expect("id"),
                line: EntityId::new(edges[1]).expect("id"),
            })
            .expect("v");
        sketch
            .add_constraint(Constraint::Vertical {
                id: ConstraintId::new("con:v1").expect("id"),
                line: EntityId::new(edges[3]).expect("id"),
            })
            .expect("v");
        sketch
            .add_constraint(Constraint::Distance {
                id: ConstraintId::new("con:w").expect("id"),
                target: DistanceTarget::LineLength {
                    line: EntityId::new(edges[0]).expect("id"),
                },
                expr: Expression::new("80 mm").expect("expr"),
            })
            .expect("w");
        sketch
            .add_constraint(Constraint::Distance {
                id: ConstraintId::new("con:h").expect("id"),
                target: DistanceTarget::LineLength {
                    line: EntityId::new(edges[1]).expect("id"),
                },
                expr: Expression::new("60 mm").expect("expr"),
            })
            .expect("h");
        sketch
    }

    #[test]
    fn solves_rectangle_sketch() {
        let mut sketch = rectangle_sketch();
        let status = solve_sketch(&mut sketch, &SolverOptions::default()).expect("solve");
        assert!(
            status.is_solved() || matches!(status, SolveStatus::UnderConstrained { dof: 0, .. })
        );

        let c1 = sketch
            .find_entity("ent:c1")
            .and_then(|e| match e {
                SketchEntity::Point(p) => Some(p),
                _ => None,
            })
            .expect("c1");
        let x = match c1.x {
            Coord::Literal(v) => v,
            _ => panic!("expected literal"),
        };
        assert!((x - 0.08).abs() < 1e-4);
    }

    #[test]
    fn failed_solve_does_not_commit_trial_coordinates() {
        let mut sketch = rectangle_sketch();
        let before = point_coords(&sketch, "ent:c1");
        let options = SolverOptions {
            max_iterations: 0,
            ..SolverOptions::default()
        };

        let status = solve_sketch(&mut sketch, &options).expect("diagnostic result");
        assert!(matches!(
            status,
            SolveStatus::Contradictory { .. }
                | SolveStatus::NonConvergent { .. }
                | SolveStatus::Failed { .. }
        ));
        let after = point_coords(&sketch, "ent:c1");
        assert!((after[0] - before[0]).abs() < 1e-12);
        assert!((after[1] - before[1]).abs() < 1e-12);
        assert!(matches!(sketch.solve_state, SolveState::Failed { .. }));
    }

    #[test]
    fn parses_mm_expression() {
        assert!((parse_length_expr("80 mm").expect("parse") - 0.08).abs() < 1e-9);
    }

    #[test]
    fn equal_line_lengths_use_si_units() {
        let mut sketch = Sketch::new(
            SketchId::new("sketch:equal_lines").expect("id"),
            "Equal lines",
            Workplane::xy(),
        );
        for (id, x, y) in [
            ("ent:p0", 0.0, 0.0),
            ("ent:p1", 0.08, 0.0),
            ("ent:p2", 0.0, 0.02),
            ("ent:p3", 0.04, 0.02),
        ] {
            sketch
                .add_entity(SketchEntity::Point(PointEntity {
                    base: EntityBase {
                        id: EntityId::new(id).expect("id"),
                        construction: false,
                    },
                    x: Coord::literal(x),
                    y: Coord::literal(y),
                }))
                .expect("point");
        }
        for (id, start, end) in [
            ("ent:line_a", "ent:p0", "ent:p1"),
            ("ent:line_b", "ent:p2", "ent:p3"),
        ] {
            sketch
                .add_entity(SketchEntity::Line(LineEntity {
                    base: EntityBase {
                        id: EntityId::new(id).expect("id"),
                        construction: false,
                    },
                    start: EntityId::new(start).expect("id"),
                    end: EntityId::new(end).expect("id"),
                }))
                .expect("line");
        }
        sketch
            .add_constraint(Constraint::Equal {
                id: ConstraintId::new("con:equal_lines").expect("id"),
                a: EqualTarget::LineLength(EntityId::new("ent:line_a").expect("id")),
                b: EqualTarget::LineLength(EntityId::new("ent:line_b").expect("id")),
            })
            .expect("equal");
        sketch
            .add_constraint(Constraint::Distance {
                id: ConstraintId::new("con:line_a_length").expect("id"),
                target: DistanceTarget::LineLength {
                    line: EntityId::new("ent:line_a").expect("id"),
                },
                expr: Expression::new("80 mm").expect("expr"),
            })
            .expect("distance");

        solve_sketch(&mut sketch, &SolverOptions::default()).expect("solve");
        let point = |id: &str| {
            let entity = sketch.find_entity(id).expect("point entity");
            let SketchEntity::Point(point) = entity else {
                panic!("expected point")
            };
            let Coord::Literal(x) = point.x else {
                panic!("expected literal x")
            };
            let Coord::Literal(y) = point.y else {
                panic!("expected literal y")
            };
            [x, y]
        };
        let a0 = point("ent:p0");
        let a1 = point("ent:p1");
        let b0 = point("ent:p2");
        let b1 = point("ent:p3");
        let length =
            |a: [f64; 2], b: [f64; 2]| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        assert!((length(a0, a1) - length(b0, b1)).abs() < 1e-8);
        assert!((length(a0, a1) - 0.08).abs() < 1e-8);
    }

    #[test]
    fn equal_circle_and_arc_radii_use_unit_bearing_values() {
        let mut sketch = Sketch::new(
            SketchId::new("sketch:equal_radii").expect("id"),
            "Equal radii",
            Workplane::xy(),
        );
        for id in ["ent:center_circle", "ent:center_arc"] {
            sketch
                .add_entity(SketchEntity::Point(PointEntity {
                    base: EntityBase {
                        id: EntityId::new(id).expect("id"),
                        construction: false,
                    },
                    x: Coord::literal(0.0),
                    y: Coord::literal(0.0),
                }))
                .expect("center");
        }
        sketch
            .add_entity(SketchEntity::Circle(CircleEntity {
                base: EntityBase {
                    id: EntityId::new("ent:circle").expect("id"),
                    construction: false,
                },
                center: EntityId::new("ent:center_circle").expect("id"),
                radius: Coord::expr("80 mm").expect("radius"),
            }))
            .expect("circle");
        sketch
            .add_entity(SketchEntity::Arc(ArcEntity {
                base: EntityBase {
                    id: EntityId::new("ent:arc").expect("id"),
                    construction: false,
                },
                center: EntityId::new("ent:center_arc").expect("id"),
                radius: Coord::literal(0.04),
                start_angle: Coord::literal(0.0),
                end_angle: Coord::literal(1.0),
                start_point: None,
                end_point: None,
                resolved_angles_rad: None,
            }))
            .expect("arc");
        sketch
            .add_constraint(Constraint::Radius {
                id: ConstraintId::new("con:circle_radius").expect("id"),
                target: EntityId::new("ent:circle").expect("id"),
                expr: Expression::new("80 mm").expect("expr"),
            })
            .expect("radius");
        sketch
            .add_constraint(Constraint::Equal {
                id: ConstraintId::new("con:equal_radii").expect("id"),
                a: EqualTarget::Radius(EntityId::new("ent:circle").expect("id")),
                b: EqualTarget::Radius(EntityId::new("ent:arc").expect("id")),
            })
            .expect("equal");

        solve_sketch(&mut sketch, &SolverOptions::default()).expect("solve");
        let radius = |id: &str| {
            let entity = sketch.find_entity(id).expect("radius entity");
            let radius = match entity {
                SketchEntity::Circle(circle) => &circle.radius,
                SketchEntity::Arc(arc) => &arc.radius,
                _ => panic!("expected circular entity"),
            };
            let Coord::Literal(radius) = radius else {
                panic!("expected literal radius")
            };
            *radius
        };
        assert!((radius("ent:circle") - 0.08).abs() < 1e-8);
        assert!((radius("ent:arc") - 0.08).abs() < 1e-8);
    }

    #[test]
    fn seeds_arc_radius_from_existing_si_value() {
        let mut sketch = Sketch::new(
            SketchId::new("sketch:arc_seed").expect("id"),
            "Arc seed",
            Workplane::xy(),
        );
        sketch
            .add_entity(SketchEntity::Point(PointEntity {
                base: EntityBase {
                    id: EntityId::new("ent:center").expect("id"),
                    construction: false,
                },
                x: Coord::literal(0.0),
                y: Coord::literal(0.0),
            }))
            .expect("center");
        sketch
            .add_entity(SketchEntity::Arc(ArcEntity {
                base: EntityBase {
                    id: EntityId::new("ent:arc").expect("id"),
                    construction: false,
                },
                center: EntityId::new("ent:center").expect("id"),
                radius: Coord::expr("40 mm").expect("radius"),
                start_angle: Coord::literal(0.0),
                end_angle: Coord::literal(1.0),
                start_point: None,
                end_point: None,
                resolved_angles_rad: None,
            }))
            .expect("arc");

        let (_, registry, _) = build_problem(&sketch).expect("problem");
        let mut values = registry.initial_values();
        seed_radius_values(&registry, &mut values, &sketch);
        let radius = registry.get("ent:arc.radius").expect("radius variable");
        assert!((values[radius.index()] - 0.04).abs() < 1e-12);
    }

    #[test]
    fn equal_target_kind_validation_is_explicit() {
        let mut sketch = Sketch::new(
            SketchId::new("sketch:equal_validation").expect("id"),
            "Equal validation",
            Workplane::xy(),
        );
        sketch
            .add_entity(SketchEntity::Point(PointEntity {
                base: EntityBase {
                    id: EntityId::new("ent:point").expect("id"),
                    construction: false,
                },
                x: Coord::literal(0.0),
                y: Coord::literal(0.0),
            }))
            .expect("point");
        sketch
            .add_constraint(Constraint::Equal {
                id: ConstraintId::new("con:invalid_line_target").expect("id"),
                a: EqualTarget::LineLength(EntityId::new("ent:point").expect("id")),
                b: EqualTarget::LineLength(EntityId::new("ent:point").expect("id")),
            })
            .expect("constraint");

        let error = solve_sketch(&mut sketch, &SolverOptions::default()).expect_err("validation");
        assert!(error
            .to_string()
            .contains("equal line-length target 'ent:point' must reference a line"));

        sketch.constraints.clear();
        sketch
            .add_constraint(Constraint::Equal {
                id: ConstraintId::new("con:missing_radius_target").expect("id"),
                a: EqualTarget::Radius(EntityId::new("ent:missing").expect("id")),
                b: EqualTarget::Radius(EntityId::new("ent:missing").expect("id")),
            })
            .expect("constraint");
        let error = solve_sketch(&mut sketch, &SolverOptions::default()).expect_err("not found");
        assert!(error
            .to_string()
            .contains("equal radius target 'ent:missing'"));
    }

    #[test]
    fn solves_parallel_direction_constraint() {
        let mut sketch = two_line_sketch((0.5, 0.5));
        add_direction_support_constraints(&mut sketch);
        sketch
            .add_constraint(Constraint::Parallel {
                id: ConstraintId::new("con:parallel").expect("id"),
                line_a: EntityId::new("ent:line_a").expect("id"),
                line_b: EntityId::new("ent:line_b").expect("id"),
            })
            .expect("parallel");

        let status = solve_sketch(&mut sketch, &SolverOptions::default()).expect("solve");
        assert!(status.is_solved());
        let [x, y] = point_coords(&sketch, "ent:p3");
        assert!((x - 1.0).abs() < 1e-8);
        assert!(y.abs() < 1e-8);
    }

    /// Tolerance on solved coordinates, in meters.
    const SOLVED_TOLERANCE_M: f64 = 1e-8;

    /// Converged, possibly with remaining degrees of freedom (free points
    /// added by these tests); correctness is asserted on coordinates.
    fn converged(status: &SolveStatus) -> bool {
        matches!(
            status,
            SolveStatus::Solved { .. } | SolveStatus::UnderConstrained { .. }
        )
    }

    fn add_point(sketch: &mut Sketch, id: &str, x: f64, y: f64) {
        sketch
            .add_entity(SketchEntity::Point(PointEntity {
                base: EntityBase {
                    id: EntityId::new(id).expect("id"),
                    construction: false,
                },
                x: Coord::literal(x),
                y: Coord::literal(y),
            }))
            .expect("point");
    }

    #[test]
    fn solves_angle_constraint_in_degrees() {
        let mut sketch = two_line_sketch((0.5, 0.5));
        add_direction_support_constraints(&mut sketch);
        sketch
            .add_constraint(Constraint::Angle {
                id: ConstraintId::new("con:angle").expect("id"),
                line_a: EntityId::new("ent:line_a").expect("id"),
                line_b: EntityId::new("ent:line_b").expect("id"),
                expr: Expression::new("30 deg").expect("expression"),
            })
            .expect("angle");

        let status = solve_sketch(&mut sketch, &SolverOptions::default()).expect("solve");
        assert!(status.is_solved());
        let [x, y] = point_coords(&sketch, "ent:p3");
        let expected = 30_f64.to_radians();
        assert!((x - expected.cos()).abs() < SOLVED_TOLERANCE_M, "x {x}");
        assert!((y - expected.sin()).abs() < SOLVED_TOLERANCE_M, "y {y}");
    }

    #[test]
    fn angle_expressions_accept_radians_and_degrees() {
        assert!((parse_angle_expr("30 deg").unwrap() - 30_f64.to_radians()).abs() < 1e-15);
        assert!((parse_angle_expr("0.5 rad").unwrap() - 0.5).abs() < 1e-15);
        assert!((parse_angle_expr("0.5").unwrap() - 0.5).abs() < 1e-15);
        assert!(parse_angle_expr("30 mm").is_err());
    }

    #[test]
    fn solves_midpoint_constraint() {
        let mut sketch = two_line_sketch((0.5, 0.5));
        add_direction_support_constraints(&mut sketch);
        add_point(&mut sketch, "ent:m", 0.3, 0.2);
        sketch
            .add_constraint(Constraint::Midpoint {
                id: ConstraintId::new("con:mid").expect("id"),
                point: EntityId::new("ent:m").expect("id"),
                line: EntityId::new("ent:line_a").expect("id"),
            })
            .expect("midpoint");

        assert!(converged(
            &solve_sketch(&mut sketch, &SolverOptions::default()).expect("solve")
        ));
        let [p0x, p0y] = point_coords(&sketch, "ent:p0");
        let [p1x, p1y] = point_coords(&sketch, "ent:p1");
        let [mx, my] = point_coords(&sketch, "ent:m");
        assert!((mx - 0.5 * (p0x + p1x)).abs() < SOLVED_TOLERANCE_M);
        assert!((my - 0.5 * (p0y + p1y)).abs() < SOLVED_TOLERANCE_M);
    }

    #[test]
    fn solves_symmetric_constraint() {
        let mut sketch = two_line_sketch((0.5, 0.5));
        add_direction_support_constraints(&mut sketch);
        add_point(&mut sketch, "ent:p", 0.3, 0.4);
        add_point(&mut sketch, "ent:q", 0.5, -0.1);
        sketch
            .add_constraint(Constraint::Symmetric {
                id: ConstraintId::new("con:sym").expect("id"),
                a: EntityId::new("ent:p").expect("id"),
                b: EntityId::new("ent:q").expect("id"),
                line: EntityId::new("ent:line_a").expect("id"),
            })
            .expect("symmetric");

        assert!(converged(
            &solve_sketch(&mut sketch, &SolverOptions::default()).expect("solve")
        ));
        // line_a stays on the x axis, so symmetry mirrors y and keeps x.
        let [_, line_y] = point_coords(&sketch, "ent:p1");
        let [px, py] = point_coords(&sketch, "ent:p");
        let [qx, qy] = point_coords(&sketch, "ent:q");
        assert!(line_y.abs() < SOLVED_TOLERANCE_M);
        assert!((px - qx).abs() < SOLVED_TOLERANCE_M, "{px} vs {qx}");
        assert!((py + qy).abs() < SOLVED_TOLERANCE_M, "{py} vs {qy}");
    }

    #[test]
    fn solves_tangent_line_circle_constraint() {
        let mut sketch = two_line_sketch((0.5, 0.5));
        add_direction_support_constraints(&mut sketch);
        add_point(&mut sketch, "ent:c", 0.5, 0.3);
        sketch
            .add_entity(SketchEntity::Circle(CircleEntity {
                base: EntityBase {
                    id: EntityId::new("ent:circle").expect("id"),
                    construction: false,
                },
                center: EntityId::new("ent:c").expect("id"),
                radius: Coord::literal(0.2),
            }))
            .expect("circle");
        sketch
            .add_constraint(Constraint::Radius {
                id: ConstraintId::new("con:r").expect("id"),
                target: EntityId::new("ent:circle").expect("id"),
                expr: Expression::new("0.2 m").expect("expression"),
            })
            .expect("radius");
        sketch
            .add_constraint(Constraint::Tangent {
                id: ConstraintId::new("con:tangent").expect("id"),
                line: EntityId::new("ent:line_a").expect("id"),
                curve: EntityId::new("ent:circle").expect("id"),
            })
            .expect("tangent");

        assert!(converged(
            &solve_sketch(&mut sketch, &SolverOptions::default()).expect("solve")
        ));
        let [_, line_y] = point_coords(&sketch, "ent:p1");
        let [_, cy] = point_coords(&sketch, "ent:c");
        assert!(
            ((cy - line_y).abs() - 0.2).abs() < SOLVED_TOLERANCE_M,
            "cy {cy}"
        );

        let mut wrong = two_line_sketch((0.5, 0.5));
        wrong
            .add_constraint(Constraint::Tangent {
                id: ConstraintId::new("con:tangent").expect("id"),
                line: EntityId::new("ent:line_a").expect("id"),
                curve: EntityId::new("ent:line_b").expect("id"),
            })
            .expect("tangent");
        let error = solve_sketch(&mut wrong, &SolverOptions::default()).expect_err("not a curve");
        assert!(error.to_string().contains("must be a circle or arc"));
    }

    #[test]
    fn solves_perpendicular_direction_constraint() {
        let mut sketch = two_line_sketch((0.5, 0.5));
        add_direction_support_constraints(&mut sketch);
        sketch
            .add_constraint(Constraint::Perpendicular {
                id: ConstraintId::new("con:perpendicular").expect("id"),
                line_a: EntityId::new("ent:line_a").expect("id"),
                line_b: EntityId::new("ent:line_b").expect("id"),
            })
            .expect("perpendicular");

        let status = solve_sketch(&mut sketch, &SolverOptions::default()).expect("solve");
        assert!(status.is_solved());
        let [x, y] = point_coords(&sketch, "ent:p3");
        assert!(x.abs() < 1e-8);
        assert!((y - 1.0).abs() < 1e-8);
    }

    #[test]
    fn rejects_direction_constraint_for_line_below_degeneracy_tolerance() {
        let mut sketch = two_line_sketch((0.5, 0.5));
        if let Some(SketchEntity::Point(point)) = sketch
            .entities
            .iter_mut()
            .find(|entity| entity.id().as_str() == "ent:p1")
        {
            point.x = Coord::literal(DIRECTION_DEGENERACY_TOLERANCE_M * 0.5);
        }
        sketch
            .add_constraint(Constraint::Parallel {
                id: ConstraintId::new("con:parallel_degenerate").expect("id"),
                line_a: EntityId::new("ent:line_a").expect("id"),
                line_b: EntityId::new("ent:line_b").expect("id"),
            })
            .expect("parallel");

        let error = solve_sketch(&mut sketch, &SolverOptions::default()).expect_err("validation");
        assert!(error.to_string().contains("zero-length"));
        assert!(error.to_string().contains("1.0e-12 m"));
    }

    #[test]
    fn accepts_direction_line_above_degeneracy_tolerance() {
        let mut sketch = two_line_sketch((0.5, 0.5));
        if let Some(SketchEntity::Point(point)) = sketch
            .entities
            .iter_mut()
            .find(|entity| entity.id().as_str() == "ent:p1")
        {
            point.x = Coord::literal(DIRECTION_DEGENERACY_TOLERANCE_M * 2.0);
        }
        sketch
            .add_constraint(Constraint::Parallel {
                id: ConstraintId::new("con:parallel_small").expect("id"),
                line_a: EntityId::new("ent:line_a").expect("id"),
                line_b: EntityId::new("ent:line_b").expect("id"),
            })
            .expect("parallel");

        assert!(solve_sketch(&mut sketch, &SolverOptions::default()).is_ok());
    }

    #[test]
    fn rejects_a_direction_line_that_collapses_during_solving() {
        let mut sketch = two_line_sketch((0.5, 0.5));
        sketch
            .add_constraint(Constraint::Parallel {
                id: ConstraintId::new("con:parallel_collapse").expect("id"),
                line_a: EntityId::new("ent:line_a").expect("id"),
                line_b: EntityId::new("ent:line_b").expect("id"),
            })
            .expect("parallel");
        let (_, registry, _) = build_problem(&sketch).expect("valid initial problem");
        let mut vars = VarSet::new(registry.initial_values());
        let p0x = registry.get("ent:p0.x").expect("p0 x");
        let p0y = registry.get("ent:p0.y").expect("p0 y");
        let p1x = registry.get("ent:p1.x").expect("p1 x");
        let p1y = registry.get("ent:p1.y").expect("p1 y");
        vars.set(p1x, vars.get(p0x));
        vars.set(p1y, vars.get(p0y));

        let error = validate_solved_direction_lines(&sketch, &registry, &vars)
            .expect_err("collapsed line must be rejected");
        assert!(error
            .to_string()
            .contains("direction constraint line 'ent:line_a'"));
    }

    fn two_line_sketch(line_b_end: (f64, f64)) -> Sketch {
        let mut sketch = Sketch::new(
            SketchId::new("sketch:directions").expect("id"),
            "Direction constraints",
            Workplane::xy(),
        );
        for (id, x, y) in [
            ("ent:p0", 0.0, 0.0),
            ("ent:p1", 1.0, 0.0),
            ("ent:p2", 0.0, 0.0),
            ("ent:p3", line_b_end.0, line_b_end.1),
        ] {
            sketch
                .add_entity(SketchEntity::Point(PointEntity {
                    base: EntityBase {
                        id: EntityId::new(id).expect("id"),
                        construction: false,
                    },
                    x: Coord::literal(x),
                    y: Coord::literal(y),
                }))
                .expect("point");
        }
        for (id, start, end) in [
            ("ent:line_a", "ent:p0", "ent:p1"),
            ("ent:line_b", "ent:p2", "ent:p3"),
        ] {
            sketch
                .add_entity(SketchEntity::Line(LineEntity {
                    base: EntityBase {
                        id: EntityId::new(id).expect("id"),
                        construction: false,
                    },
                    start: EntityId::new(start).expect("id"),
                    end: EntityId::new(end).expect("id"),
                }))
                .expect("line");
        }
        sketch
    }

    fn add_direction_support_constraints(sketch: &mut Sketch) {
        sketch
            .add_constraint(Constraint::Coincident {
                id: ConstraintId::new("con:coincident").expect("id"),
                a: EntityRef::Entity(EntityId::new("ent:p0").expect("id")),
                b: EntityRef::Entity(EntityId::new("ent:p2").expect("id")),
            })
            .expect("coincident");
        sketch
            .add_constraint(Constraint::Horizontal {
                id: ConstraintId::new("con:horizontal").expect("id"),
                line: EntityId::new("ent:line_a").expect("id"),
            })
            .expect("horizontal");
        for (id, line) in [
            ("con:length_a", "ent:line_a"),
            ("con:length_b", "ent:line_b"),
        ] {
            sketch
                .add_constraint(Constraint::Distance {
                    id: ConstraintId::new(id).expect("id"),
                    target: DistanceTarget::LineLength {
                        line: EntityId::new(line).expect("id"),
                    },
                    expr: Expression::new("1 m").expect("expression"),
                })
                .expect("distance");
        }
    }

    /// ADR-021: arc endpoint points are pulled onto the arc at its angles.
    #[test]
    fn arc_endpoint_points_lie_on_the_arc() {
        let mut sketch = Sketch::new(
            SketchId::new("sketch:arc_ends").expect("id"),
            "Arc ends",
            Workplane::xy(),
        );
        for (id, x, y) in [
            ("ent:c", 0.0, 0.0),
            ("ent:p", 0.03, 0.01),
            ("ent:q", -0.01, 0.02),
        ] {
            sketch
                .add_entity(SketchEntity::Point(PointEntity {
                    base: EntityBase {
                        id: EntityId::new(id).expect("id"),
                        construction: false,
                    },
                    x: Coord::literal(x),
                    y: Coord::literal(y),
                }))
                .expect("point");
        }
        sketch
            .add_entity(SketchEntity::Arc(ArcEntity {
                base: EntityBase {
                    id: EntityId::new("ent:arc").expect("id"),
                    construction: false,
                },
                center: EntityId::new("ent:c").expect("id"),
                radius: Coord::literal(0.02),
                start_angle: Coord::expr("0 deg").expect("angle"),
                end_angle: Coord::expr("90 deg").expect("angle"),
                start_point: Some(EntityId::new("ent:p").expect("id")),
                end_point: Some(EntityId::new("ent:q").expect("id")),
                resolved_angles_rad: None,
            }))
            .expect("arc");
        sketch
            .add_constraint(Constraint::Radius {
                id: ConstraintId::new("con:r").expect("id"),
                target: EntityId::new("ent:arc").expect("id"),
                expr: Expression::new("0.02 m").expect("expression"),
            })
            .expect("radius");
        solve_sketch(&mut sketch, &SolverOptions::default()).expect("solve");
        let [px, py] = point_coords(&sketch, "ent:p");
        let [qx, qy] = point_coords(&sketch, "ent:q");
        assert!(
            (px - 0.02).abs() < SOLVED_TOLERANCE_M && py.abs() < SOLVED_TOLERANCE_M,
            "start ({px}, {py})"
        );
        assert!(
            qx.abs() < SOLVED_TOLERANCE_M && (qy - 0.02).abs() < SOLVED_TOLERANCE_M,
            "end ({qx}, {qy})"
        );
    }

    fn point_coords(sketch: &Sketch, id: &str) -> [f64; 2] {
        let SketchEntity::Point(point) = sketch.find_entity(id).expect("point") else {
            panic!("expected point")
        };
        let Coord::Literal(x) = point.x else {
            panic!("expected literal x")
        };
        let Coord::Literal(y) = point.y else {
            panic!("expected literal y")
        };
        [x, y]
    }
}
