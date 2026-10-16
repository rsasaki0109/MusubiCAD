//! ADR-103 kernel-neutral admission for one-arc/two-radii sectors.
use crate::{
    ProfileSegment, SketchPlacement, SolvedSketch, POSITION_TOLERANCE_M, ROTATION_TOLERANCE,
};
use opencad_core::{OpenCadError, Result};
use std::f64::consts::PI;
const MAX_POSITION_M: f64 = 1_000_000.;
const MAX_LENGTH_M: f64 = 1_000.;
fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(format!("arc sector: {message}"))
}
fn roundoff(length: f64) -> f64 {
    64. * f64::EPSILON * length.max(1.)
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    a.map(|v| v * s)
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: [f64; 3]) -> Result<[f64; 3]> {
    let n = dot(a, a).sqrt();
    if !n.is_finite() || n <= 1e-12 {
        return Err(invalid("invalid axis"));
    }
    Ok(scale(a, 1. / n))
}
fn dimension(value: f64) -> Result<()> {
    if !value.is_finite() || value <= POSITION_TOLERANCE_M || value > MAX_LENGTH_M {
        return Err(invalid("radius must be above 1 nm and at most 1000 m"));
    }
    Ok(())
}
fn position(p: [f64; 3]) -> Result<()> {
    if p.into_iter()
        .any(|v| !v.is_finite() || v.abs() > MAX_POSITION_M)
    {
        return Err(invalid("position must be finite within 1e6 m"));
    }
    Ok(())
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn ends(segment: &ProfileSegment) -> ([f64; 2], [f64; 2]) {
    match segment {
        ProfileSegment::Line { start_m, end_m } | ProfileSegment::Arc { start_m, end_m, .. } => {
            (*start_m, *end_m)
        }
    }
}
pub(crate) struct SectorProfile {
    pub(crate) origin_m: [f64; 3],
    pub(crate) axes: [[f64; 3]; 3],
    pub(crate) radius_m: f64,
    pub(crate) sweep_rad: f64,
}
/// Validate the bounded portable closed arc-sector profile (ADR-103).
/// All coordinates are metres; no source, profile, polygon or placement is changed.
pub fn validate_arc_sector_profile(sketch: &SolvedSketch) -> Result<()> {
    profile(sketch).map(|_| ())
}
pub(crate) fn profile(sketch: &SolvedSketch) -> Result<SectorProfile> {
    if !sketch.closed
        || sketch.circle.is_some()
        || sketch.ellipse.is_some()
        || sketch.cubic_bezier.is_some()
    {
        return Err(invalid("requires one exact closed arc-sector profile"));
    }
    let plane = sketch.placement.unwrap_or(SketchPlacement::global_xy());
    position(plane.origin_m)?;
    for axis in [plane.x_axis_m, plane.y_axis_m] {
        if axis.into_iter().any(|v| !v.is_finite())
            || (dot(axis, axis) - 1.).abs() > ROTATION_TOLERANCE
        {
            return Err(invalid("sketch frame must be orthonormal"));
        }
    }
    if dot(plane.x_axis_m, plane.y_axis_m).abs() > ROTATION_TOLERANCE {
        return Err(invalid("sketch frame must be orthonormal"));
    }
    if sketch.segments.len() != 3
        || sketch
            .segments
            .iter()
            .filter(|s| matches!(s, ProfileSegment::Arc { .. }))
            .count()
            != 1
    {
        return Err(invalid("sector needs exactly one arc and two radial lines"));
    }
    for (i, segment) in sketch.segments.iter().enumerate() {
        let (start, end) = ends(segment);
        if start
            .into_iter()
            .chain(end)
            .any(|v| !v.is_finite() || v.abs() > MAX_POSITION_M)
        {
            return Err(invalid("sector coordinates must be finite and bounded"));
        }
        let (next, _) = ends(&sketch.segments[(i + 1) % 3]);
        let magnitude = start
            .into_iter()
            .chain(end)
            .chain(next)
            .map(f64::abs)
            .fold(1., f64::max);
        if distance(end, next) > roundoff(magnitude) {
            return Err(invalid("sector loop must close without snapping a gap"));
        }
    }
    let i = sketch
        .segments
        .iter()
        .position(|s| matches!(s, ProfileSegment::Arc { .. }))
        .ok_or_else(|| invalid("sector arc missing"))?;
    let ProfileSegment::Arc {
        start_m,
        mid_m,
        end_m,
    } = sketch.segments[i]
    else {
        return Err(invalid("sector arc missing"));
    };
    if mid_m
        .into_iter()
        .any(|v| !v.is_finite() || v.abs() > MAX_POSITION_M)
    {
        return Err(invalid("sector through-point must be finite and bounded"));
    }
    let (_, center) = ends(&sketch.segments[(i + 1) % 3]);
    let radius = distance(center, start_m);
    dimension(radius)?;
    let epsilon = POSITION_TOLERANCE_M.min(roundoff(radius));
    for p in [mid_m, end_m] {
        dimension(distance(center, p))?;
        if (distance(center, p) - radius).abs() > epsilon {
            return Err(invalid("radial lines must meet at the arc's circle centre"));
        }
    }
    for (a, b) in [(start_m, mid_m), (mid_m, end_m), (start_m, end_m)] {
        if distance(a, b) <= POSITION_TOLERANCE_M {
            return Err(invalid("sector arc points must be distinct above 1 nm"));
        }
    }
    let u = [
        (start_m[0] - center[0]) / radius,
        (start_m[1] - center[1]) / radius,
    ];
    let angle = |p: [f64; 2]| {
        let v = [p[0] - center[0], p[1] - center[1]];
        (u[0] * v[1] - u[1] * v[0])
            .atan2(u[0] * v[0] + u[1] * v[1])
            .rem_euclid(2. * PI)
    };
    let end = angle(end_m);
    let mid = angle(mid_m);
    let tolerance = ROTATION_TOLERANCE;
    if end <= tolerance
        || end >= 2. * PI - tolerance
        || mid <= tolerance
        || mid >= 2. * PI - tolerance
        || (mid - end).abs() <= tolerance
    {
        return Err(invalid("sector zero/full turn or ambiguous through-point"));
    }
    let (sweep, sign) = if mid < end {
        (end, 1.)
    } else {
        (2. * PI - end, -1.)
    };
    let x = unit(add(
        scale(plane.x_axis_m, u[0]),
        scale(plane.y_axis_m, u[1]),
    ))?;
    let y = unit(add(
        scale(plane.x_axis_m, -sign * u[1]),
        scale(plane.y_axis_m, sign * u[0]),
    ))?;
    if dot(x, y).abs() > ROTATION_TOLERANCE {
        return Err(invalid("rotated sketch frame must remain orthonormal"));
    }
    let origin_m = plane.map_point(center[0], center[1]);
    position(origin_m)?;
    Ok(SectorProfile {
        origin_m,
        axes: [x, y, unit(cross(plane.x_axis_m, plane.y_axis_m))?],
        radius_m: radius,
        sweep_rad: sweep,
    })
}
