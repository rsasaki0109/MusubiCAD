//! ADR-105: constant centre/major/through-point intent, no new source family.
use super::*;
use opencad_geometry::{SketchPlacement, SolvedEllipse, POSITION_TOLERANCE_M};
use std::f64::consts::TAU;

fn constant(doc: &OcadDocument, text: &str) -> Result<(Coord, f64)> {
    if !opencad_graph::parameter_names_in_expr(text).is_empty() {
        return Err(OpenCadError::validation(
            "ellipse creation requires constant unit-bearing coordinates; associative axis input is not admitted",
        ));
    }
    Ok((
        coord(doc, text, false)?,
        opencad_graph::eval_length_expr_strict(text, &Default::default())?,
    ))
}

pub(super) fn entities(
    doc: &OcadDocument,
    sketch: &Sketch,
    form: &SketchForm,
) -> Result<Vec<SketchEntity>> {
    let SketchForm::EllipseAxes {
        id,
        center_x,
        center_y,
        major_x,
        major_y,
        through_x,
        through_y,
        construction,
    } = form
    else {
        return Err(OpenCadError::validation(
            "expected ellipse axes creation intent",
        ));
    };
    base(id, false)?;
    if sketch.find_entity(id).is_some() {
        return Err(OpenCadError::validation("ellipse source ID already exists"));
    }
    let (cx_source, cx) = constant(doc, center_x)?;
    let (cy_source, cy) = constant(doc, center_y)?;
    let (_, mx) = constant(doc, major_x)?;
    let (_, my) = constant(doc, major_y)?;
    let (_, qx) = constant(doc, through_x)?;
    let (_, qy) = constant(doc, through_y)?;
    let major = (mx - cx).hypot(my - cy);
    if major <= POSITION_TOLERANCE_M || !major.is_finite() {
        return Err(OpenCadError::validation(
            "ellipse major radius must exceed 1 nm",
        ));
    }
    let ux = (mx - cx) / major;
    let uy = (my - cy) / major;
    let x = (qx - cx) * ux + (qy - cy) * uy;
    let y = (qy - cy) * ux - (qx - cx) * uy;
    let k = 1. - (x / major).powi(2);
    if !k.is_finite() || k <= 1e-9 {
        return Err(OpenCadError::validation(
            "ellipse through-point must lie unambiguously inside the major-axis span",
        ));
    }
    let minor = y.abs() / k.sqrt();
    let angle = (my - cy).atan2(mx - cx).rem_euclid(TAU);
    // Reuse the existing analytic admission; never repair or swap the axes.
    SolvedEllipse {
        center_m: [cx, cy],
        major_radius_m: major,
        minor_radius_m: minor,
        axis_angle_rad: angle,
    }
    .frame(SketchPlacement::global_xy())?;
    let center_id = EntityId::new(format!("{id}_center"))?;
    Ok(vec![
        SketchEntity::Point(PointEntity {
            base: base(center_id.as_str(), false)?,
            x: cx_source,
            y: cy_source,
        }),
        SketchEntity::Ellipse(EllipseEntity {
            base: base(id, *construction)?,
            center: center_id,
            major_radius: Coord::expr(format!("{:.12} mm", major * 1000.))?,
            minor_radius: Coord::expr(format!("{:.12} mm", minor * 1000.))?,
            axis_angle: Coord::expr(format!("{angle:.17} rad"))?,
            resolved_parameters: None,
        }),
    ])
}
