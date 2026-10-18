//! Bounded analytic trim/extend selection in sketch-local SI coordinates.
use opencad_core::{OpenCadError, Result};
use serde::{Deserialize, Serialize};

const CONTACT_M: f64 = 1e-9;
const PARALLEL_SIN: f64 = 1e-12;
const TAU: f64 = std::f64::consts::TAU;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EditCurve {
    Line {
        start_m: [f64; 2],
        end_m: [f64; 2],
    },
    Circle {
        center_m: [f64; 2],
        radius_m: f64,
    },
    Arc {
        center_m: [f64; 2],
        radius_m: f64,
        start_rad: f64,
        sweep_rad: f64,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveEditMode {
    Trim,
    Extend,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveEditEnd {
    Start,
    End,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CurveEditIntersection {
    /// Line fraction, or arc angle relative to its original start in radians.
    pub target_parameter: f64,
    pub position_m: [f64; 2],
}

impl EditCurve {
    pub fn validate(&self) -> Result<()> {
        let valid_point = |p: &[f64; 2]| p.iter().all(|v| v.is_finite() && v.abs() <= 1e6);
        match self {
            Self::Line { start_m, end_m }
                if valid_point(start_m)
                    && valid_point(end_m)
                    && norm(sub(*end_m, *start_m)) > CONTACT_M =>
            {
                Ok(())
            }
            Self::Circle { center_m, radius_m }
                if valid_point(center_m)
                    && radius_m.is_finite()
                    && *radius_m > CONTACT_M
                    && *radius_m <= 1e6 =>
            {
                Ok(())
            }
            Self::Arc {
                center_m,
                radius_m,
                start_rad,
                sweep_rad,
            } if valid_point(center_m)
                && radius_m.is_finite()
                && *radius_m > CONTACT_M
                && *radius_m <= 1e6
                && start_rad.is_finite()
                && sweep_rad.is_finite()
                && *sweep_rad > CONTACT_M / radius_m
                && *sweep_rad < TAU - CONTACT_M / radius_m =>
            {
                Ok(())
            }
            _ => Err(invalid("invalid, degenerate or out-of-budget curve")),
        }
    }
    fn parameter(&self, p: [f64; 2]) -> f64 {
        match self {
            Self::Line { start_m, end_m } => {
                let d = sub(*end_m, *start_m);
                dot(sub(p, *start_m), d) / dot(d, d)
            }
            Self::Circle { center_m, .. } => {
                let d = sub(p, *center_m);
                d[1].atan2(d[0]).rem_euclid(TAU)
            }
            Self::Arc {
                center_m,
                start_rad,
                ..
            } => {
                let d = sub(p, *center_m);
                (d[1].atan2(d[0]) - start_rad.rem_euclid(TAU)).rem_euclid(TAU)
            }
        }
    }
    fn parameter_tolerance(&self) -> f64 {
        CONTACT_M
            / match self {
                Self::Line { start_m, end_m } => norm(sub(*end_m, *start_m)),
                Self::Circle { radius_m, .. } | Self::Arc { radius_m, .. } => *radius_m,
            }
    }
    fn contains(&self, p: [f64; 2]) -> bool {
        let t = self.parameter(p);
        let eps = self.parameter_tolerance();
        match self {
            Self::Line { .. } => t >= -eps && t <= 1. + eps,
            Self::Circle { .. } => true,
            Self::Arc { sweep_rad, .. } => t <= sweep_rad + eps || TAU - t <= eps,
        }
    }
}

/// Eligible intersections sorted by target parameter. Multiple results require
/// a caller-selected index; callers must never infer a nearest click silently.
pub fn curve_edit_intersections(
    target: EditCurve,
    cutter: EditCurve,
    mode: CurveEditMode,
    end: CurveEditEnd,
) -> Result<Vec<CurveEditIntersection>> {
    target.validate()?;
    cutter.validate()?;
    if matches!(target, EditCurve::Circle { .. }) {
        return Err(invalid("target must be a stored line or arc"));
    }
    let points = intersections(target, cutter)?;
    let limit = match target {
        EditCurve::Line { .. } => 1.,
        EditCurve::Arc { sweep_rad, .. } => sweep_rad,
        _ => unreachable!("circle refused above"),
    };
    let eps = target.parameter_tolerance();
    let mut selected = Vec::new();
    for point in points {
        if !cutter.contains(point) {
            continue;
        }
        if point.iter().any(|v| !v.is_finite() || v.abs() > 1e6) {
            return Err(invalid("intersection exceeds coordinate budget"));
        }
        let mut t = target.parameter(point);
        if matches!(
            (target, mode, end),
            (
                EditCurve::Arc { .. },
                CurveEditMode::Extend,
                CurveEditEnd::Start
            )
        ) {
            t -= TAU;
        }
        let eligible = match (mode, end) {
            (CurveEditMode::Trim, _) => t > eps && t < limit - eps,
            (CurveEditMode::Extend, CurveEditEnd::Start) => t < -eps,
            (CurveEditMode::Extend, CurveEditEnd::End) => t > limit + eps,
        };
        if eligible
            && (!matches!(target, EditCurve::Arc { .. })
                || match end {
                    CurveEditEnd::Start => limit - t < TAU - eps,
                    CurveEditEnd::End => t < TAU - eps,
                })
        {
            selected.push(CurveEditIntersection {
                target_parameter: t,
                position_m: point,
            });
        }
    }
    selected.sort_by(|a, b| a.target_parameter.total_cmp(&b.target_parameter));
    if selected
        .windows(2)
        .any(|p| norm(sub(p[0].position_m, p[1].position_m)) <= CONTACT_M)
    {
        return Err(invalid("ambiguous near-coincident intersections"));
    }
    if selected.is_empty() {
        return Err(invalid("no eligible intersection for selected mode/end"));
    }
    Ok(selected)
}

fn intersections(a: EditCurve, b: EditCurve) -> Result<Vec<[f64; 2]>> {
    match (a, b) {
        (
            EditCurve::Line {
                start_m: p,
                end_m: q,
            },
            EditCurve::Line {
                start_m: r,
                end_m: s,
            },
        ) => {
            let u = sub(q, p);
            let v = sub(s, r);
            let det = cross(u, v);
            if det.abs() <= PARALLEL_SIN * norm(u) * norm(v) {
                if cross(sub(r, p), u).abs() / norm(u) <= CONTACT_M {
                    return Err(invalid("coincident cutters are ambiguous"));
                }
                return Ok(Vec::new());
            }
            Ok(vec![add(p, mul(u, cross(sub(r, p), v) / det))])
        }
        (
            EditCurve::Line {
                start_m: p,
                end_m: q,
            },
            EditCurve::Circle {
                center_m: c,
                radius_m: r,
            }
            | EditCurve::Arc {
                center_m: c,
                radius_m: r,
                ..
            },
        )
        | (
            EditCurve::Circle {
                center_m: c,
                radius_m: r,
            }
            | EditCurve::Arc {
                center_m: c,
                radius_m: r,
                ..
            },
            EditCurve::Line {
                start_m: p,
                end_m: q,
            },
        ) => {
            let u = sub(q, p);
            let length = norm(u);
            let middle = dot(sub(c, p), u) / dot(u, u);
            let distance = cross(sub(c, p), u).abs() / length;
            let h2 = r * r - distance * distance;
            let tolerance = 2. * r * CONTACT_M + CONTACT_M * CONTACT_M;
            if h2.abs() <= tolerance {
                return Err(invalid("tangent cuts are unsupported"));
            }
            if h2 < -tolerance {
                return Ok(Vec::new());
            }
            let delta = h2.sqrt() / length;
            Ok(vec![
                add(p, mul(u, middle - delta)),
                add(p, mul(u, middle + delta)),
            ])
        }
        (
            EditCurve::Circle {
                center_m: p,
                radius_m: r,
            }
            | EditCurve::Arc {
                center_m: p,
                radius_m: r,
                ..
            },
            EditCurve::Circle {
                center_m: q,
                radius_m: s,
            }
            | EditCurve::Arc {
                center_m: q,
                radius_m: s,
                ..
            },
        ) => {
            let u = sub(q, p);
            let d = norm(u);
            if d <= CONTACT_M {
                return if (r - s).abs() <= CONTACT_M {
                    Err(invalid("coincident circles are ambiguous"))
                } else {
                    Ok(Vec::new())
                };
            }
            let along = (r * r - s * s + d * d) / (2. * d);
            let h2 = r * r - along * along;
            let tolerance = 2. * r * CONTACT_M + CONTACT_M * CONTACT_M;
            if h2.abs() <= tolerance {
                return Err(invalid("tangent cuts are unsupported"));
            }
            if h2 < -tolerance {
                return Ok(Vec::new());
            }
            let middle = add(p, mul(u, along / d));
            let offset = mul([-u[1], u[0]], h2.sqrt() / d);
            Ok(vec![add(middle, offset), sub(middle, offset)])
        }
    }
}
fn invalid(s: &str) -> OpenCadError {
    OpenCadError::validation(format!("sketch curve edit: {s}"))
}
fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}
fn mul(a: [f64; 2], s: f64) -> [f64; 2] {
    [a[0] * s, a[1] * s]
}
fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn norm(a: [f64; 2]) -> f64 {
    a[0].hypot(a[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analytic_trim_extend_intersections_and_selection() {
        let target = EditCurve::Line {
            start_m: [0.02, 0.03],
            end_m: [0.06, 0.03],
        };
        let cutter = EditCurve::Line {
            start_m: [0.04, 0.],
            end_m: [0.04, 0.06],
        };
        let p = curve_edit_intersections(target, cutter, CurveEditMode::Trim, CurveEditEnd::End)
            .expect("trim");
        assert_eq!(p.len(), 1);
        assert!((p[0].target_parameter - 0.5).abs() < 1e-12);
        assert!(norm(sub(p[0].position_m, [0.04, 0.03])) < 1e-12);
        let cutter = EditCurve::Line {
            start_m: [0.07, 0.],
            end_m: [0.07, 0.06],
        };
        let p = curve_edit_intersections(target, cutter, CurveEditMode::Extend, CurveEditEnd::End)
            .expect("extend");
        assert!((p[0].target_parameter - 1.25).abs() < 1e-12);
        let arc = EditCurve::Arc {
            center_m: [0., 0.],
            radius_m: 0.05,
            start_rad: 0.,
            sweep_rad: std::f64::consts::FRAC_PI_2,
        };
        let cutter = EditCurve::Line {
            start_m: [0.03, -0.1],
            end_m: [0.03, 0.1],
        };
        let p = curve_edit_intersections(arc, cutter, CurveEditMode::Trim, CurveEditEnd::End)
            .expect("arc");
        assert_eq!(p.len(), 1);
        assert!(norm(sub(p[0].position_m, [0.03, 0.04])) < 1e-12);
        assert!((p[0].target_parameter - 4_f64.atan2(3.)).abs() < 1e-12);
    }
    #[test]
    fn finite_cutter_and_tangent_coincident_refusals() {
        let target = EditCurve::Line {
            start_m: [0., 0.],
            end_m: [1., 0.],
        };
        assert!(curve_edit_intersections(
            target,
            EditCurve::Line {
                start_m: [0.5, 1.],
                end_m: [0.5, 2.]
            },
            CurveEditMode::Trim,
            CurveEditEnd::End
        )
        .is_err());
        assert!(
            curve_edit_intersections(target, target, CurveEditMode::Trim, CurveEditEnd::End)
                .is_err()
        );
        assert!(curve_edit_intersections(
            target,
            EditCurve::Circle {
                center_m: [0.5, 1.],
                radius_m: 1.
            },
            CurveEditMode::Trim,
            CurveEditEnd::End
        )
        .is_err());
    }

    #[test]
    fn multiple_intersections_are_sorted_and_arc_extension_is_bounded() {
        let target = EditCurve::Line {
            start_m: [0.02, 0.03],
            end_m: [0.06, 0.03],
        };
        let cutter = EditCurve::Circle {
            center_m: [0.04, 0.03],
            radius_m: 0.005,
        };
        let results =
            curve_edit_intersections(target, cutter, CurveEditMode::Trim, CurveEditEnd::End)
                .expect("two intersections");
        assert_eq!(results.len(), 2);
        assert!((results[0].position_m[0] - 0.035).abs() < 1e-12);
        assert!((results[1].position_m[0] - 0.045).abs() < 1e-12);
        let arc = EditCurve::Arc {
            center_m: [0., 0.],
            radius_m: 0.05,
            start_rad: 0.,
            sweep_rad: std::f64::consts::FRAC_PI_2,
        };
        let cutter = EditCurve::Line {
            start_m: [-0.03, 0.],
            end_m: [-0.03, 0.1],
        };
        let results =
            curve_edit_intersections(arc, cutter, CurveEditMode::Extend, CurveEditEnd::End)
                .expect("arc extension");
        assert_eq!(results.len(), 1);
        assert!(
            results[0].target_parameter > std::f64::consts::FRAC_PI_2
                && results[0].target_parameter < TAU
        );
    }
}
