//! Convert solved sketch profiles into kernel-neutral wire input.

use indexmap::IndexMap;

use opencad_core::{EntityId, OpenCadError, Result};
use opencad_geometry::{
    ExtrudeOperation, ProfileSegment, SketchPlacement, SolvedCircle, SolvedSketch,
};
use opencad_sketch::{
    entity::{expand_rectangle, Coord, LineEntity, SketchEntity},
    workplane::{GlobalPlane, Workplane},
    Profile, ProfileKind, Sketch,
};

use crate::feature::RegenContext;
use crate::topo_resolve::workplane_for_face_ref;

const CIRCLE_SEGMENTS: usize = 32;

/// Expand rectangle helpers, then refresh profile detection.
pub fn prepare_sketch(sketch: &mut Sketch) -> Result<()> {
    let rectangles: Vec<_> = sketch
        .entities
        .iter()
        .filter_map(|e| match e {
            SketchEntity::Rectangle(r) if !r.corner_ids.is_empty() => Some(r.clone()),
            _ => None,
        })
        .collect();

    if !rectangles.is_empty() {
        let mut expanded = Vec::new();
        for rect in rectangles {
            expanded.extend(expand_rectangle(&rect)?);
        }
        sketch
            .entities
            .retain(|e| !matches!(e, SketchEntity::Rectangle(_)));
        for entity in expanded {
            sketch.add_entity(entity)?;
        }
    }

    sketch.update_profiles()?;
    Ok(())
}

/// Build a `SolvedSketch` from a closed profile reference.
pub fn profile_to_solved(sketch: &Sketch, profile_ref: &str) -> Result<SolvedSketch> {
    let mut solved = profile_to_solved_local(sketch, profile_ref)?;
    solved.placement = Some(placement_from_workplane(&sketch.workplane)?);
    Ok(solved)
}

/// Build a `SolvedSketch` with workplane resolved from face refs during regeneration.
pub fn profile_to_solved_with_context(
    sketch: &Sketch,
    profile_ref: &str,
    ctx: &dyn RegenContext,
) -> Result<SolvedSketch> {
    let mut solved = profile_to_solved_local(sketch, profile_ref)?;
    let workplane = resolve_workplane(sketch, ctx)?;
    solved.placement = Some(placement_from_workplane(&workplane)?);
    Ok(solved)
}

pub fn extrude_direction_for_operation(
    sketch: &Sketch,
    ctx: &dyn RegenContext,
    operation: ExtrudeOperation,
) -> Result<[f64; 3]> {
    let mut direction = extrude_direction_for_sketch(sketch, ctx)?;
    if matches!(operation, ExtrudeOperation::Cut) && uses_face_local_workplane(sketch) {
        direction = [-direction[0], -direction[1], -direction[2]];
    }
    Ok(direction)
}

fn uses_face_local_workplane(sketch: &Sketch) -> bool {
    matches!(
        sketch.workplane,
        Workplane::FaceRef { .. } | Workplane::Custom { .. }
    )
}

pub fn extrude_direction_for_sketch(sketch: &Sketch, ctx: &dyn RegenContext) -> Result<[f64; 3]> {
    let workplane = resolve_workplane(sketch, ctx)?;
    Ok(placement_from_workplane(&workplane)?.extrude_direction_m())
}

fn resolve_workplane(sketch: &Sketch, ctx: &dyn RegenContext) -> Result<Workplane> {
    match &sketch.workplane {
        Workplane::FaceRef { face_ref } => workplane_for_face_ref(ctx, face_ref),
        other => Ok(other.clone()),
    }
}

pub fn placement_from_workplane(workplane: &Workplane) -> Result<SketchPlacement> {
    match workplane {
        Workplane::Global { plane } => Ok(match plane {
            GlobalPlane::XY => SketchPlacement::global_xy(),
            GlobalPlane::YZ => SketchPlacement {
                origin_m: [0.0, 0.0, 0.0],
                x_axis_m: [0.0, 1.0, 0.0],
                y_axis_m: [0.0, 0.0, 1.0],
            },
            GlobalPlane::XZ => SketchPlacement {
                origin_m: [0.0, 0.0, 0.0],
                x_axis_m: [1.0, 0.0, 0.0],
                y_axis_m: [0.0, 0.0, 1.0],
            },
        }),
        Workplane::Custom {
            origin,
            normal,
            x_axis,
        } => {
            let y_axis = y_axis_from_custom(normal, x_axis);
            Ok(SketchPlacement {
                origin_m: *origin,
                x_axis_m: *x_axis,
                y_axis_m: y_axis,
            })
        }
        Workplane::FaceRef { .. } => Err(OpenCadError::validation(
            "face_ref workplane must be resolved before building sketch placement",
        )),
    }
}

/// The in-plane y axis of a custom workplane: `normal × x_axis`, so that
/// `x × y` points along the workplane normal.
///
/// It used to ignore the normal and cross a fixed +y helper with the x axis,
/// which put sketches on a top face (normal +z) into a vertical plane: a
/// pin sketched on the bracket's top face stood on its side.
fn y_axis_from_custom(normal: &[f64; 3], x_axis: &[f64; 3]) -> [f64; 3] {
    let cross = [
        normal[1] * x_axis[2] - normal[2] * x_axis[1],
        normal[2] * x_axis[0] - normal[0] * x_axis[2],
        normal[0] * x_axis[1] - normal[1] * x_axis[0],
    ];
    let len = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    if len <= 1e-12 {
        return [0.0, 0.0, 1.0];
    }
    [cross[0] / len, cross[1] / len, cross[2] / len]
}

fn profile_to_solved_local(sketch: &Sketch, profile_ref: &str) -> Result<SolvedSketch> {
    let profile = find_profile(sketch, profile_ref)?;
    if profile.kind != ProfileKind::Closed {
        return Err(OpenCadError::validation(format!(
            "profile '{profile_ref}' is not closed"
        )));
    }

    let (points, circle, segments) = if profile.entity_ids.len() == 1 {
        let circle = circle_profile(sketch, &profile.entity_ids[0])?;
        (inscribed_polygon(circle), Some(circle), Vec::new())
    } else if let Some((points, segments)) = arc_loop(sketch, profile)? {
        (points, None, segments)
    } else {
        (line_loop_points(sketch, profile)?, None, Vec::new())
    };

    if points.len() < 3 {
        return Err(OpenCadError::validation(format!(
            "profile '{profile_ref}' needs at least three points"
        )));
    }

    Ok(SolvedSketch {
        profile_ref: profile_ref.into(),
        points,
        closed: true,
        circle,
        segments,
        placement: None,
    })
}

fn find_profile<'a>(sketch: &'a Sketch, profile_ref: &str) -> Result<&'a Profile> {
    resolve_sketch_profile(sketch, profile_ref)
        .ok_or_else(|| OpenCadError::not_found(format!("profile '{profile_ref}'")))
}

/// Resolve a feature `profile_ref` against a prepared sketch's profiles.
///
/// This is the lookup used by extrude, hole, and revolve regeneration, so
/// patch validation can prove a reference resolves before any kernel call.
pub fn resolve_sketch_profile<'a>(sketch: &'a Sketch, profile_ref: &str) -> Option<&'a Profile> {
    sketch.profiles.iter().find(|p| {
        p.profile_ref.as_deref() == Some(profile_ref)
            || p.id == profile_ref
            || format!("{}/profile:outer", sketch.id) == profile_ref
    })
}

fn point_coord(sketch: &Sketch, point_id: &EntityId) -> Result<[f64; 2]> {
    let entity = sketch
        .find_entity(point_id.as_str())
        .ok_or_else(|| OpenCadError::not_found(format!("point '{}'", point_id.as_str())))?;
    let SketchEntity::Point(point) = entity else {
        return Err(OpenCadError::validation(format!(
            "entity '{}' is not a point",
            point_id.as_str()
        )));
    };
    match (&point.x, &point.y) {
        (Coord::Literal(x), Coord::Literal(y)) => Ok([*x, *y]),
        _ => Err(OpenCadError::validation(format!(
            "point '{}' must have literal coordinates; solve the sketch first",
            point_id.as_str()
        ))),
    }
}

fn line_loop_points(sketch: &Sketch, profile: &Profile) -> Result<Vec<[f64; 2]>> {
    let lines: IndexMap<String, &LineEntity> = sketch
        .entities
        .iter()
        .filter_map(|e| match e {
            SketchEntity::Line(l) => Some((l.base.id.as_str().to_string(), l)),
            _ => None,
        })
        .collect();

    let profile_lines: Vec<&LineEntity> = profile
        .entity_ids
        .iter()
        .map(|id| {
            lines
                .get(id.as_str())
                .copied()
                .ok_or_else(|| OpenCadError::not_found(format!("line '{}'", id.as_str())))
        })
        .collect::<Result<Vec<_>>>()?;

    if profile_lines.is_empty() {
        return Err(OpenCadError::validation("profile has no line entities"));
    }

    let first_line = profile_lines[0];
    let start = first_line.start.clone();
    let mut ordered = vec![point_coord(sketch, &first_line.start)?];
    let mut current_end = first_line.end.clone();
    ordered.push(point_coord(sketch, &current_end)?);
    let mut remaining: Vec<&LineEntity> = profile_lines[1..].to_vec();

    while current_end != start {
        let Some(idx) = remaining
            .iter()
            .position(|line| line.start == current_end || line.end == current_end)
        else {
            break;
        };
        let line = remaining.remove(idx);
        if line.start == current_end {
            current_end = line.end.clone();
        } else {
            current_end = line.start.clone();
        }
        ordered.push(point_coord(sketch, &current_end)?);
        if ordered.len() > profile_lines.len() + 1 {
            return Err(OpenCadError::validation(
                "profile loop traversal did not close cleanly",
            ));
        }
    }

    if current_end != start {
        return Err(OpenCadError::validation(
            "profile loop does not close on the first point",
        ));
    }

    if ordered.len() > 1 && ordered.last() == ordered.first() {
        ordered.pop();
    }

    Ok(ordered)
}

/// A sampled polygon and the exact segments of a loop with arcs.
type ArcLoop = (Vec<[f64; 2]>, Vec<ProfileSegment>);

/// Samples per arc in the polygon kept for kernels without curves.
const ARC_POLYGON_SAMPLES: usize = 16;

/// Ordered exact segments of a loop that contains arcs, with a sampled
/// polygon (ADR-021).  Returns `None` for line-only loops, which keep the
/// polygon path.
fn arc_loop(sketch: &Sketch, profile: &Profile) -> Result<Option<ArcLoop>> {
    enum Piece<'a> {
        Line(&'a LineEntity),
        Arc(&'a opencad_sketch::entity::ArcEntity),
    }
    let pieces: Vec<Piece> = profile
        .entity_ids
        .iter()
        .map(|id| match sketch.find_entity(id.as_str()) {
            Some(SketchEntity::Line(line)) => Ok(Piece::Line(line)),
            Some(SketchEntity::Arc(arc)) => Ok(Piece::Arc(arc)),
            _ => Err(OpenCadError::validation(format!(
                "profile entity '{}' is not a line or arc",
                id.as_str()
            ))),
        })
        .collect::<Result<_>>()?;
    if !pieces.iter().any(|piece| matches!(piece, Piece::Arc(_))) {
        return Ok(None);
    }
    let ends = |piece: &Piece| -> Result<(EntityId, EntityId)> {
        Ok(match piece {
            Piece::Line(line) => (line.start.clone(), line.end.clone()),
            Piece::Arc(arc) => match (&arc.start_point, &arc.end_point) {
                (Some(start), Some(end)) => (start.clone(), end.clone()),
                _ => {
                    return Err(OpenCadError::validation(format!(
                        "arc '{}' needs start_point and end_point to join a loop",
                        arc.base.id.as_str()
                    )))
                }
            },
        })
    };

    let mut remaining: Vec<&Piece> = pieces.iter().collect();
    let first = remaining.remove(0);
    let (loop_start, mut current) = ends(first)?;
    let mut ordered = vec![(first, false)];
    while current != loop_start {
        let Some(index) = remaining
            .iter()
            .position(|piece| ends(piece).is_ok_and(|(a, b)| a == current || b == current))
        else {
            return Err(OpenCadError::validation(
                "profile loop does not close on the first point",
            ));
        };
        let piece = remaining.remove(index);
        let (a, b) = ends(piece)?;
        let reversed = b == current;
        current = if reversed { a } else { b };
        ordered.push((piece, reversed));
    }

    let mut segments = Vec::with_capacity(ordered.len());
    let mut points = Vec::new();
    for (piece, reversed) in ordered {
        let (a, b) = ends(piece)?;
        let (from, to) = if reversed { (b, a) } else { (a, b) };
        let (start_m, end_m) = (point_coord(sketch, &from)?, point_coord(sketch, &to)?);
        points.push(start_m);
        match piece {
            Piece::Line(_) => segments.push(ProfileSegment::Line { start_m, end_m }),
            Piece::Arc(arc) => {
                let center = point_coord(sketch, &arc.center)?;
                let radius = match &arc.radius {
                    Coord::Literal(r) => *r,
                    _ => {
                        return Err(OpenCadError::validation(
                            "arc radius must be a literal after solving",
                        ))
                    }
                };
                let start_angle = opencad_sketch::solve::arc_angle(arc, &arc.start_angle)?;
                let end_angle = opencad_sketch::solve::arc_angle(arc, &arc.end_angle)?;
                let mut sweep = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
                if sweep == 0.0 {
                    sweep = std::f64::consts::TAU;
                }
                let at = |fraction: f64| {
                    let angle = start_angle + sweep * fraction;
                    [
                        center[0] + radius * angle.cos(),
                        center[1] + radius * angle.sin(),
                    ]
                };
                let mid_m = at(0.5);
                segments.push(ProfileSegment::Arc {
                    start_m,
                    mid_m,
                    end_m,
                });
                // Interior samples, in loop direction.
                for step in 1..ARC_POLYGON_SAMPLES {
                    let fraction = step as f64 / ARC_POLYGON_SAMPLES as f64;
                    points.push(at(if reversed { 1.0 - fraction } else { fraction }));
                }
            }
        }
    }
    Ok(Some((points, segments)))
}

fn circle_profile(sketch: &Sketch, circle_id: &EntityId) -> Result<SolvedCircle> {
    let entity = sketch
        .find_entity(circle_id.as_str())
        .ok_or_else(|| OpenCadError::not_found(format!("circle '{}'", circle_id.as_str())))?;
    let SketchEntity::Circle(circle) = entity else {
        return Err(OpenCadError::validation(format!(
            "entity '{}' is not a circle",
            circle_id.as_str()
        )));
    };
    let center = point_coord(sketch, &circle.center)?;
    let radius = match &circle.radius {
        Coord::Literal(r) => *r,
        _ => {
            return Err(OpenCadError::validation(
                "circle radius must be a literal after solving",
            ))
        }
    };

    Ok(SolvedCircle {
        center_m: center,
        radius_m: radius,
    })
}

/// Inscribed polygon of a circle, for kernels without exact curves.
fn inscribed_polygon(circle: SolvedCircle) -> Vec<[f64; 2]> {
    (0..CIRCLE_SEGMENTS)
        .map(|i| {
            let angle = std::f64::consts::TAU * i as f64 / CIRCLE_SEGMENTS as f64;
            [
                circle.center_m[0] + circle.radius_m * angle.cos(),
                circle.center_m[1] + circle.radius_m * angle.sin(),
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::{Constraint, DistanceTarget},
        entity::{EntityBase, LineEntity, PointEntity},
        workplane::Workplane,
        Sketch,
    };

    fn solved_rectangle_sketch() -> Sketch {
        let mut sketch = Sketch::new(
            SketchId::new("sketch:base").expect("id"),
            "Base",
            Workplane::xy(),
        );

        let corners = ["ent:c0", "ent:c1", "ent:c2", "ent:c3"];
        let edges = ["ent:e0", "ent:e1", "ent:e2", "ent:e3"];
        for (id, x, y) in [
            (corners[0], 0.0, 0.0),
            (corners[1], 0.08, 0.0),
            (corners[2], 0.08, 0.06),
            (corners[3], 0.0, 0.06),
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
            .add_constraint(Constraint::Distance {
                id: ConstraintId::new("con:w").expect("id"),
                target: DistanceTarget::LineLength {
                    line: EntityId::new(edges[0]).expect("id"),
                },
                expr: Expression::new("80 mm").expect("expr"),
            })
            .expect("constraint");
        sketch.update_profiles().expect("profiles");
        sketch
    }

    #[test]
    fn profile_to_solved_rectangle() {
        let sketch = solved_rectangle_sketch();
        let solved = profile_to_solved(&sketch, "sketch:base/profile:outer").expect("solved");
        assert_eq!(solved.points.len(), 4);
        assert!(solved.closed);
    }
}
