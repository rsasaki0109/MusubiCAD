//! ADR-052: analytic convex offsets, with no sampling or topology repair.
use crate::curve_edit::EditCurve;
use opencad_core::{OpenCadError, Result};
use std::f64::consts::TAU;
const EPS: f64 = 1e-9;
fn err() -> OpenCadError {
    OpenCadError::validation("offset requires a finite convex CCW closed contour, mitred lines and tangent arc joins; inset must preserve edges and radii")
}
fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn scale(a: [f64; 2], s: f64) -> [f64; 2] {
    [a[0] * s, a[1] * s]
}
fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn norm(a: [f64; 2]) -> f64 {
    a[0].hypot(a[1])
}
fn radial(angle: f64) -> [f64; 2] {
    [angle.cos(), angle.sin()]
}
fn position(c: EditCurve, end: bool) -> Result<[f64; 2]> {
    match c {
        EditCurve::Line { start_m, end_m } => Ok(if end { end_m } else { start_m }),
        EditCurve::Arc {
            center_m,
            radius_m,
            start_rad,
            sweep_rad,
        } => Ok(add(
            center_m,
            scale(
                radial(start_rad + if end { sweep_rad } else { 0. }),
                radius_m,
            ),
        )),
        _ => Err(err()),
    }
}
fn tangent(c: EditCurve, end: bool) -> Result<[f64; 2]> {
    match c {
        EditCurve::Line { start_m, end_m } => {
            let v = sub(end_m, start_m);
            let n = norm(v);
            if n <= EPS {
                return Err(err());
            }
            Ok(scale(v, 1. / n))
        }
        EditCurve::Arc {
            start_rad,
            sweep_rad,
            ..
        } => {
            let r = radial(start_rad + if end { sweep_rad } else { 0. });
            Ok([-r[1], r[0]])
        }
        _ => Err(err()),
    }
}
fn finite(c: EditCurve) -> bool {
    let point = |p: [f64; 2]| p.iter().all(|v| v.is_finite() && v.abs() <= 1e6);
    let radius = |r: f64| r.is_finite() && r > EPS && r <= 1e6;
    match c {
        EditCurve::Line { start_m, end_m } => {
            point(start_m) && point(end_m) && norm(sub(end_m, start_m)) > EPS
        }
        EditCurve::Circle { center_m, radius_m } => point(center_m) && radius(radius_m),
        EditCurve::Arc {
            center_m,
            radius_m,
            start_rad,
            sweep_rad,
        } => {
            point(center_m)
                && radius(radius_m)
                && start_rad.is_finite()
                && start_rad.abs() <= TAU * 2.
                && sweep_rad.is_finite()
                && sweep_rad > 1e-12
                && sweep_rad < TAU - 1e-12
        }
    }
}
fn validate_loop(curves: &[EditCurve]) -> Result<()> {
    let mut turn = 0.;
    for (i, &a) in curves.iter().enumerate() {
        let b = curves[(i + 1) % curves.len()];
        if !finite(a) || norm(sub(position(a, true)?, position(b, false)?)) > EPS {
            return Err(err());
        }
        let ta = tangent(a, true)?;
        let tb = tangent(b, false)?;
        let angle = cross(ta, tb).atan2(dot(ta, tb));
        if angle < -1e-10 || angle >= std::f64::consts::PI - 1e-10 {
            return Err(err());
        }
        if !matches!((a, b), (EditCurve::Line { .. }, EditCurve::Line { .. }))
            && angle.abs() > 1e-10
        {
            return Err(err());
        }
        turn += angle.max(0.);
        if let EditCurve::Arc { sweep_rad, .. } = a {
            turn += sweep_rad;
        }
    }
    if (turn - TAU).abs() > 1e-8 {
        return Err(err());
    }
    Ok(())
}
/// Positive distance expands a convex CCW contour; negative distance insets it.
/// This operation never changes curve count or repairs geometry.
pub fn offset_loop(curves: &[EditCurve], distance_m: f64) -> Result<Vec<EditCurve>> {
    if curves.is_empty()
        || curves.len() > 256
        || !distance_m.is_finite()
        || distance_m.abs() <= EPS
        || distance_m.abs() > 1e6
    {
        return Err(err());
    }
    if let [EditCurve::Circle { center_m, radius_m }] = curves {
        let c = EditCurve::Circle {
            center_m: *center_m,
            radius_m: radius_m + distance_m,
        };
        return if finite(curves[0]) && finite(c) {
            Ok(vec![c])
        } else {
            Err(err())
        };
    }
    if curves.len() < 2 {
        return Err(err());
    }
    validate_loop(curves)?;
    let mut shifted = Vec::with_capacity(curves.len());
    for &c in curves {
        shifted.push(match c {
            EditCurve::Line { start_m, end_m } => {
                let t = tangent(c, false)?;
                let d = scale([t[1], -t[0]], distance_m);
                EditCurve::Line {
                    start_m: add(start_m, d),
                    end_m: add(end_m, d),
                }
            }
            EditCurve::Arc {
                center_m,
                radius_m,
                start_rad,
                sweep_rad,
            } => EditCurve::Arc {
                center_m,
                radius_m: radius_m + distance_m,
                start_rad,
                sweep_rad,
            },
            _ => return Err(err()),
        });
    }
    let mut joins = Vec::with_capacity(curves.len());
    for i in 0..curves.len() {
        let previous = (i + curves.len() - 1) % curves.len();
        let a = shifted[previous];
        let b = shifted[i];
        let pa = position(a, true)?;
        let pb = position(b, false)?;
        let ta = tangent(a, true)?;
        let tb = tangent(b, false)?;
        let det = cross(ta, tb);
        let join = if matches!((a, b), (EditCurve::Line { .. }, EditCurve::Line { .. }))
            && det.abs() > 1e-12
        {
            add(pa, scale(ta, cross(sub(pb, pa), tb) / det))
        } else {
            if norm(sub(pa, pb)) > EPS {
                return Err(err());
            }
            pa
        };
        joins.push(join);
    }
    let count = shifted.len();
    for (i, c) in shifted.iter_mut().enumerate() {
        if let EditCurve::Line { start_m, end_m } = c {
            *start_m = joins[i];
            *end_m = joins[(i + 1) % count];
            // Insets must not reverse a surviving edge or jump past collapse.
            if dot(sub(*end_m, *start_m), tangent(curves[i], false)?) <= EPS {
                return Err(err());
            }
        }
    }
    validate_loop(&shifted)?;
    Ok(shifted)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rectangle() -> Vec<EditCurve> {
        let p = [[0., 0.], [0.08, 0.], [0.08, 0.06], [0., 0.06]];
        (0..4)
            .map(|i| EditCurve::Line {
                start_m: p[i],
                end_m: p[(i + 1) % 4],
            })
            .collect()
    }
    #[test]
    fn rectangle_offset_has_analytic_dimensions_and_refuses_collapse() {
        let out = offset_loop(&rectangle(), 0.003).expect("offset");
        assert!(
            norm(sub(
                position(out[0], false).expect("point"),
                [-0.003, -0.003]
            )) < 1e-12
        );
        assert!(norm(sub(position(out[2], false).expect("point"), [0.083, 0.063])) < 1e-12);
        assert_eq!(
            serde_json::to_string(&out).expect("wire"),
            serde_json::to_string(&offset_loop(&rectangle(), 0.003).expect("repeat"))
                .expect("wire")
        );
        for d in [-0.03, -0.05, 0., f64::NAN] {
            assert!(offset_loop(&rectangle(), d).is_err());
        }
        assert!(offset_loop(&rectangle(), -0.01).is_ok());
    }
    pub fn stadium() -> Vec<EditCurve> {
        vec![
            EditCurve::Line {
                start_m: [0., -0.02],
                end_m: [0.04, -0.02],
            },
            EditCurve::Arc {
                center_m: [0.04, 0.],
                radius_m: 0.02,
                start_rad: -std::f64::consts::FRAC_PI_2,
                sweep_rad: std::f64::consts::PI,
            },
            EditCurve::Line {
                start_m: [0.04, 0.02],
                end_m: [0., 0.02],
            },
            EditCurve::Arc {
                center_m: [0., 0.],
                radius_m: 0.02,
                start_rad: std::f64::consts::FRAC_PI_2,
                sweep_rad: std::f64::consts::PI,
            },
        ]
    }
    #[test]
    fn tangent_arc_offsets_and_circle_radius_are_exact() {
        let out = offset_loop(&stadium(), 0.005).expect("stadium");
        assert!(matches!(out[1],EditCurve::Arc{radius_m,..} if (radius_m-0.025).abs()<1e-12));
        assert!(norm(sub(position(out[0], false).expect("point"), [0., -0.025])) < 1e-12);
        assert!(offset_loop(&stadium(), -0.02).is_err());
        let circle = [EditCurve::Circle {
            center_m: [0., 0.],
            radius_m: 0.02,
        }];
        assert!(
            matches!(offset_loop(&circle,0.005).expect("circle")[0],EditCurve::Circle{radius_m,..} if (radius_m-0.025).abs()<1e-12)
        );
        assert!(offset_loop(&circle, -0.02).is_err());
    }
    #[test]
    fn nonconvex_and_nontangent_inputs_refuse() {
        let p = [[0., 0.], [0.08, 0.], [0.04, 0.02], [0.08, 0.06], [0., 0.06]];
        let curves: Vec<_> = (0..5)
            .map(|i| EditCurve::Line {
                start_m: p[i],
                end_m: p[(i + 1) % 5],
            })
            .collect();
        assert!(offset_loop(&curves, 0.003).is_err());
        let mut bad = stadium();
        if let EditCurve::Line { start_m, .. } = &mut bad[0] {
            *start_m = [0., -0.019];
        }
        assert!(offset_loop(&bad, 0.003).is_err());
    }
}
