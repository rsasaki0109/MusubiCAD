//! ADR-104: constant three-click intent expands to existing exact source.
use super::*;
use opencad_geometry::{ProfileSegment, SolvedSketch, POSITION_TOLERANCE_M, ROTATION_TOLERANCE};
use opencad_sketch::ArcEntity;
use std::f64::consts::TAU;

fn constant(doc: &OcadDocument, text: &str) -> Result<(Coord, f64)> {
    if !opencad_graph::parameter_names_in_expr(text).is_empty() {
        return Err(OpenCadError::validation(
            "arc creation requires constant unit-bearing coordinates; associative parameter input is not admitted",
        ));
    }
    Ok((
        coord(doc, text, false)?,
        opencad_graph::eval_length_expr_strict(text, &Default::default())?,
    ))
}
fn length(value: f64) -> Result<Coord> {
    Coord::expr(format!("{:.12} mm", value * 1000.))
}

pub(super) fn entities(
    doc: &OcadDocument,
    sketch: &Sketch,
    form: &SketchForm,
) -> Result<Vec<SketchEntity>> {
    let SketchForm::Arc {
        id,
        center_x,
        center_y,
        start_x,
        start_y,
        end_direction_x,
        end_direction_y,
        clockwise,
        close_sector,
        construction,
    } = form
    else {
        return Err(OpenCadError::validation("expected arc creation intent"));
    };
    base(id, false)?;
    if sketch.find_entity(id).is_some() {
        return Err(OpenCadError::validation("arc source ID already exists"));
    }
    let (cx_source, cx) = constant(doc, center_x)?;
    let (cy_source, cy) = constant(doc, center_y)?;
    let (sx_source, sx) = constant(doc, start_x)?;
    let (sy_source, sy) = constant(doc, start_y)?;
    let (_, dx) = constant(doc, end_direction_x)?;
    let (_, dy) = constant(doc, end_direction_y)?;
    let r = (sx - cx).hypot(sy - cy);
    if (dx - cx).hypot(dy - cy) <= POSITION_TOLERANCE_M {
        return Err(OpenCadError::validation(
            "arc end direction must differ from centre above 1 nm",
        ));
    }
    let a = (sy - cy).atan2(sx - cx).rem_euclid(TAU);
    let b = (dy - cy).atan2(dx - cx).rem_euclid(TAU);
    let sweep = if *clockwise {
        (a - b).rem_euclid(TAU)
    } else {
        (b - a).rem_euclid(TAU)
    };
    if !sweep.is_finite() || sweep <= ROTATION_TOLERANCE || sweep >= TAU - ROTATION_TOLERANCE {
        return Err(OpenCadError::validation(
            "arc requires an unambiguous positive sub-full-turn sweep",
        ));
    }
    let canonical_start = if *clockwise { b } else { a };
    let canonical_end = canonical_start + sweep;
    let center = [cx, cy];
    let clicked_start = [sx, sy];
    let projected_end = [cx + r * b.cos(), cy + r * b.sin()];
    let (start, end) = if *clockwise {
        (projected_end, clicked_start)
    } else {
        (clicked_start, projected_end)
    };
    let mid = canonical_start + sweep / 2.;
    // This transient fan validates the existing portable subset, even for an
    // open draft. Its radial lines are persisted only when closure was requested.
    opencad_geometry::validate_arc_sector_profile(&SolvedSketch {
        profile_ref: "editor arc admission".into(),
        points: Vec::new(),
        closed: true,
        circle: None,
        ellipse: None,
        cubic_bezier: None,
        placement: None,
        segments: vec![
            ProfileSegment::Arc {
                start_m: start,
                mid_m: [cx + r * mid.cos(), cy + r * mid.sin()],
                end_m: end,
            },
            ProfileSegment::Line {
                start_m: end,
                end_m: center,
            },
            ProfileSegment::Line {
                start_m: center,
                end_m: start,
            },
        ],
    })?;
    let center_id = EntityId::new(format!("{id}_center"))?;
    let start_id = EntityId::new(format!("{id}_start"))?;
    let end_id = EntityId::new(format!("{id}_end"))?;
    let (arc_start, arc_end) = if *clockwise {
        (end_id.clone(), start_id.clone())
    } else {
        (start_id.clone(), end_id.clone())
    };
    let mut result = vec![
        SketchEntity::Point(PointEntity {
            base: base(center_id.as_str(), false)?,
            x: cx_source,
            y: cy_source,
        }),
        SketchEntity::Point(PointEntity {
            base: base(start_id.as_str(), false)?,
            x: sx_source,
            y: sy_source,
        }),
        SketchEntity::Point(PointEntity {
            base: base(end_id.as_str(), false)?,
            x: length(projected_end[0])?,
            y: length(projected_end[1])?,
        }),
        SketchEntity::Arc(ArcEntity {
            base: base(id, *construction)?,
            center: center_id.clone(),
            radius: length(r)?,
            start_angle: Coord::expr(format!("{canonical_start:.17} rad"))?,
            end_angle: Coord::expr(format!("{canonical_end:.17} rad"))?,
            start_point: Some(arc_start.clone()),
            end_point: Some(arc_end.clone()),
            resolved_angles_rad: None,
        }),
    ];
    if *close_sector {
        for (suffix, start, end) in [
            ("start_ray", center_id.clone(), arc_start),
            ("end_ray", arc_end, center_id),
        ] {
            result.push(SketchEntity::Line(LineEntity {
                base: base(&format!("{id}_{suffix}"), *construction)?,
                start,
                end,
            }));
        }
    }
    Ok(result)
}
