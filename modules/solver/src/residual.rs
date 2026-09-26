use crate::variables::{VarId, VarSet};

/// Minimum line length accepted by normalized direction residuals, in meters.
///
/// Parallel and perpendicular residuals divide by the product of the two line
/// lengths.  A line at or below this tolerance is therefore degenerate rather
/// than a valid direction.  The sketch layer validates this condition before
/// constructing a direction residual; the numeric residual also clamps the
/// denominator so a directly-constructed equation cannot divide by zero.
pub const DIRECTION_DEGENERACY_TOLERANCE_M: f64 = 1e-12;

/// Single scalar residual equation.
pub trait ResidualEquation: std::fmt::Debug + Send + Sync {
    fn involved_vars(&self) -> Vec<VarId>;
    fn residual(&self, vars: &VarSet) -> f64;
}

/// A scalar length used by an equal-length residual.
///
/// Segment coordinates and scalar values are both expressed in internal SI
/// units (meters).  The sketch layer is responsible for resolving semantic
/// entity references to one of these terms.
#[derive(Debug, Clone, Copy)]
pub enum LengthTerm {
    Segment {
        x1: VarId,
        y1: VarId,
        x2: VarId,
        y2: VarId,
    },
    Scalar {
        value: VarId,
    },
}

/// Built-in 2D constraint residuals.
#[derive(Debug, Clone)]
pub enum ConstraintResidual {
    /// A point lies at a fixed angle on an arc: `px - (cx + r cos(angle))`,
    /// in metres (ADR-021).  `cos` is the cosine of the arc end angle.
    ArcEndpointX {
        px: VarId,
        cx: VarId,
        radius: VarId,
        cos: f64,
    },
    /// `py - (cy + r sin(angle))`, in metres (ADR-021).
    ArcEndpointY {
        py: VarId,
        cy: VarId,
        radius: VarId,
        sin: f64,
    },
    CoincidentX {
        a: VarId,
        b: VarId,
    },
    CoincidentY {
        a: VarId,
        b: VarId,
    },
    Horizontal {
        x1: VarId,
        y1: VarId,
        x2: VarId,
        y2: VarId,
    },
    Vertical {
        x1: VarId,
        y1: VarId,
        x2: VarId,
        y2: VarId,
    },
    /// Direction alignment of two line segments.
    ///
    /// The residual is the signed 2D cross product divided by the product of
    /// the segment lengths.  It is dimensionless (`sin(angle)`), so its scale
    /// does not depend on the units or absolute size of either line.
    Parallel {
        ax1: VarId,
        ay1: VarId,
        ax2: VarId,
        ay2: VarId,
        bx1: VarId,
        by1: VarId,
        bx2: VarId,
        by2: VarId,
    },
    /// Orthogonality of two line segments.
    ///
    /// The residual is the 2D dot product divided by the product of the
    /// segment lengths.  It is dimensionless (`cos(angle)`), so its scale does
    /// not depend on the units or absolute size of either line.
    Perpendicular {
        ax1: VarId,
        ay1: VarId,
        ax2: VarId,
        ay2: VarId,
        bx1: VarId,
        by1: VarId,
        bx2: VarId,
        by2: VarId,
    },
    Distance {
        x1: VarId,
        y1: VarId,
        x2: VarId,
        y2: VarId,
        target: f64,
    },
    Radius {
        radius: VarId,
        target: f64,
    },
    /// Equality between any two length-valued terms (line length or radius).
    EqualLength {
        a: LengthTerm,
        b: LengthTerm,
    },
    FixedX {
        x: VarId,
        value: f64,
    },
    FixedY {
        y: VarId,
        value: f64,
    },
    /// Directed angle from segment `a` to segment `b` equals `target_rad`.
    ///
    /// The residual is `sin(angle - target)`: the cross/dot form divided by
    /// both segment lengths, so it is dimensionless and continuous.  It also
    /// vanishes at `target + pi` (opposite direction); solving from a nearby
    /// initial sketch selects the intended branch.
    Angle {
        ax1: VarId,
        ay1: VarId,
        ax2: VarId,
        ay2: VarId,
        bx1: VarId,
        by1: VarId,
        bx2: VarId,
        by2: VarId,
        target_rad: f64,
    },
    /// One coordinate of a point equals the mean of two coordinates
    /// (a midpoint uses one equation per axis), in meters.
    Midpoint {
        point: VarId,
        a: VarId,
        b: VarId,
    },
    /// The midpoint of `p`–`q` lies on the line `l1`–`l2`: signed distance in
    /// meters.
    SymmetricMidpointOnLine {
        px: VarId,
        py: VarId,
        qx: VarId,
        qy: VarId,
        lx1: VarId,
        ly1: VarId,
        lx2: VarId,
        ly2: VarId,
    },
    /// `p`–`q` is perpendicular to the line `l1`–`l2`: projection of `q - p`
    /// onto the line direction, in meters.
    SymmetricPerpendicular {
        px: VarId,
        py: VarId,
        qx: VarId,
        qy: VarId,
        lx1: VarId,
        ly1: VarId,
        lx2: VarId,
        ly2: VarId,
    },
    /// Distance from a circle center to an infinite line equals the radius,
    /// in meters.
    TangentLineCircle {
        cx: VarId,
        cy: VarId,
        radius: VarId,
        lx1: VarId,
        ly1: VarId,
        lx2: VarId,
        ly2: VarId,
    },
}

impl ConstraintResidual {
    /// Midpoint equations for both axes.
    pub fn midpoint(
        px: VarId,
        py: VarId,
        (x1, y1, x2, y2): (VarId, VarId, VarId, VarId),
    ) -> [Self; 2] {
        [
            Self::Midpoint {
                point: px,
                a: x1,
                b: x2,
            },
            Self::Midpoint {
                point: py,
                a: y1,
                b: y2,
            },
        ]
    }

    pub fn coincident(a_x: VarId, a_y: VarId, b_x: VarId, b_y: VarId) -> [Self; 2] {
        [
            Self::CoincidentX { a: a_x, b: b_x },
            Self::CoincidentY { a: a_y, b: b_y },
        ]
    }
}

impl ResidualEquation for ConstraintResidual {
    fn involved_vars(&self) -> Vec<VarId> {
        match self {
            Self::CoincidentX { a, b } => vec![*a, *b],
            Self::CoincidentY { a, b } => vec![*a, *b],
            Self::Horizontal { x1, y1, x2, y2 } | Self::Vertical { x1, y1, x2, y2 } => {
                vec![*x1, *y1, *x2, *y2]
            }
            Self::Parallel {
                ax1,
                ay1,
                ax2,
                ay2,
                bx1,
                by1,
                bx2,
                by2,
            }
            | Self::Perpendicular {
                ax1,
                ay1,
                ax2,
                ay2,
                bx1,
                by1,
                bx2,
                by2,
            } => vec![*ax1, *ay1, *ax2, *ay2, *bx1, *by1, *bx2, *by2],
            Self::Distance { x1, y1, x2, y2, .. } => vec![*x1, *y1, *x2, *y2],
            Self::Radius { radius, .. } => vec![*radius],
            Self::EqualLength { a, b } => {
                let mut vars = length_term_vars(*a);
                vars.extend(length_term_vars(*b));
                vars
            }
            Self::FixedX { x, .. } => vec![*x],
            Self::FixedY { y, .. } => vec![*y],
            Self::Angle {
                ax1,
                ay1,
                ax2,
                ay2,
                bx1,
                by1,
                bx2,
                by2,
                ..
            } => vec![*ax1, *ay1, *ax2, *ay2, *bx1, *by1, *bx2, *by2],
            Self::Midpoint { point, a, b } => vec![*point, *a, *b],
            Self::ArcEndpointX { px, cx, radius, .. } => vec![*px, *cx, *radius],
            Self::ArcEndpointY { py, cy, radius, .. } => vec![*py, *cy, *radius],
            Self::SymmetricMidpointOnLine {
                px,
                py,
                qx,
                qy,
                lx1,
                ly1,
                lx2,
                ly2,
            }
            | Self::SymmetricPerpendicular {
                px,
                py,
                qx,
                qy,
                lx1,
                ly1,
                lx2,
                ly2,
            } => vec![*px, *py, *qx, *qy, *lx1, *ly1, *lx2, *ly2],
            Self::TangentLineCircle {
                cx,
                cy,
                radius,
                lx1,
                ly1,
                lx2,
                ly2,
            } => vec![*cx, *cy, *radius, *lx1, *ly1, *lx2, *ly2],
        }
    }

    fn residual(&self, vars: &VarSet) -> f64 {
        match self {
            Self::ArcEndpointX {
                px,
                cx,
                radius,
                cos,
            } => vars.get(*px) - (vars.get(*cx) + vars.get(*radius) * cos),
            Self::ArcEndpointY {
                py,
                cy,
                radius,
                sin,
            } => vars.get(*py) - (vars.get(*cy) + vars.get(*radius) * sin),
            Self::CoincidentX { a, b } => vars.get(*a) - vars.get(*b),
            Self::CoincidentY { a, b } => vars.get(*a) - vars.get(*b),
            Self::Horizontal { y1, y2, .. } => vars.get(*y1) - vars.get(*y2),
            Self::Vertical { x1, x2, .. } => vars.get(*x1) - vars.get(*x2),
            Self::Parallel {
                ax1,
                ay1,
                ax2,
                ay2,
                bx1,
                by1,
                bx2,
                by2,
            } => normalized_direction_residual(
                [*ax1, *ay1, *ax2, *ay2],
                [*bx1, *by1, *bx2, *by2],
                vars,
                false,
            ),
            Self::Perpendicular {
                ax1,
                ay1,
                ax2,
                ay2,
                bx1,
                by1,
                bx2,
                by2,
            } => normalized_direction_residual(
                [*ax1, *ay1, *ax2, *ay2],
                [*bx1, *by1, *bx2, *by2],
                vars,
                true,
            ),
            Self::Distance {
                x1,
                y1,
                x2,
                y2,
                target,
            } => {
                let dx = vars.get(*x2) - vars.get(*x1);
                let dy = vars.get(*y2) - vars.get(*y1);
                (dx * dx + dy * dy).sqrt() - target
            }
            Self::Radius { radius, target } => vars.get(*radius) - target,
            Self::EqualLength { a, b } => length_term_value(*a, vars) - length_term_value(*b, vars),
            Self::FixedX { x, value } => vars.get(*x) - value,
            Self::FixedY { y, value } => vars.get(*y) - value,
            Self::Angle {
                ax1,
                ay1,
                ax2,
                ay2,
                bx1,
                by1,
                bx2,
                by2,
                target_rad,
            } => {
                let sin = normalized_direction_residual(
                    [*ax1, *ay1, *ax2, *ay2],
                    [*bx1, *by1, *bx2, *by2],
                    vars,
                    false,
                );
                let cos = normalized_direction_residual(
                    [*ax1, *ay1, *ax2, *ay2],
                    [*bx1, *by1, *bx2, *by2],
                    vars,
                    true,
                );
                // sin(angle - target) = sin*cos(t) - cos*sin(t)
                sin * target_rad.cos() - cos * target_rad.sin()
            }
            Self::Midpoint { point, a, b } => {
                vars.get(*point) - 0.5 * (vars.get(*a) + vars.get(*b))
            }
            Self::SymmetricMidpointOnLine {
                px,
                py,
                qx,
                qy,
                lx1,
                ly1,
                lx2,
                ly2,
            } => {
                let (dx, dy, length) = line_direction(*lx1, *ly1, *lx2, *ly2, vars);
                let mx = 0.5 * (vars.get(*px) + vars.get(*qx)) - vars.get(*lx1);
                let my = 0.5 * (vars.get(*py) + vars.get(*qy)) - vars.get(*ly1);
                (dx * my - dy * mx) / length
            }
            Self::SymmetricPerpendicular {
                px,
                py,
                qx,
                qy,
                lx1,
                ly1,
                lx2,
                ly2,
            } => {
                let (dx, dy, length) = line_direction(*lx1, *ly1, *lx2, *ly2, vars);
                let vx = vars.get(*qx) - vars.get(*px);
                let vy = vars.get(*qy) - vars.get(*py);
                (vx * dx + vy * dy) / length
            }
            Self::TangentLineCircle {
                cx,
                cy,
                radius,
                lx1,
                ly1,
                lx2,
                ly2,
            } => {
                let (dx, dy, length) = line_direction(*lx1, *ly1, *lx2, *ly2, vars);
                let ox = vars.get(*cx) - vars.get(*lx1);
                let oy = vars.get(*cy) - vars.get(*ly1);
                ((dx * oy - dy * ox) / length).abs() - vars.get(*radius)
            }
        }
    }
}

/// Direction of a line and its length clamped to the degeneracy tolerance.
fn line_direction(x1: VarId, y1: VarId, x2: VarId, y2: VarId, vars: &VarSet) -> (f64, f64, f64) {
    let dx = vars.get(x2) - vars.get(x1);
    let dy = vars.get(y2) - vars.get(y1);
    (dx, dy, dx.hypot(dy).max(DIRECTION_DEGENERACY_TOLERANCE_M))
}

/// Evaluate a normalized direction relation between two 2D line segments.
///
/// `perpendicular` selects the dot-product (`cos(angle)`) residual; otherwise
/// the cross-product (`sin(angle)`) residual is returned.  The denominator is
/// clamped only for direct numeric use with a degenerate line.  Sketch solving
/// rejects such lines up front using [`DIRECTION_DEGENERACY_TOLERANCE_M`].
fn normalized_direction_residual(
    a: [VarId; 4],
    b: [VarId; 4],
    vars: &VarSet,
    perpendicular: bool,
) -> f64 {
    let avx = vars.get(a[2]) - vars.get(a[0]);
    let avy = vars.get(a[3]) - vars.get(a[1]);
    let bvx = vars.get(b[2]) - vars.get(b[0]);
    let bvy = vars.get(b[3]) - vars.get(b[1]);
    let a_length = avx.hypot(avy);
    let b_length = bvx.hypot(bvy);
    let denominator = a_length.max(DIRECTION_DEGENERACY_TOLERANCE_M)
        * b_length.max(DIRECTION_DEGENERACY_TOLERANCE_M);

    let numerator = if perpendicular {
        avx * bvx + avy * bvy
    } else {
        avx * bvy - avy * bvx
    };
    numerator / denominator
}

fn length_term_vars(term: LengthTerm) -> Vec<VarId> {
    match term {
        LengthTerm::Segment { x1, y1, x2, y2 } => vec![x1, y1, x2, y2],
        LengthTerm::Scalar { value } => vec![value],
    }
}

fn length_term_value(term: LengthTerm, vars: &VarSet) -> f64 {
    match term {
        LengthTerm::Segment { x1, y1, x2, y2 } => {
            let dx = vars.get(x2) - vars.get(x1);
            let dy = vars.get(y2) - vars.get(y1);
            (dx * dx + dy * dy).sqrt()
        }
        LengthTerm::Scalar { value } => vars.get(value),
    }
}

/// Evaluate all residuals into a vector.
pub fn evaluate_residuals(equations: &[ConstraintResidual], vars: &VarSet) -> Vec<f64> {
    evaluate_residuals_generic(equations, vars)
}

/// Evaluate residuals for any equation type implementing [`ResidualEquation`].
pub fn evaluate_residuals_generic<E: ResidualEquation>(equations: &[E], vars: &VarSet) -> Vec<f64> {
    equations.iter().map(|eq| eq.residual(vars)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::variables::VarSet;

    #[test]
    fn coincident_residual_is_zero_when_equal() {
        let vars = VarSet::new(vec![1.0, 2.0, 1.0, 2.0]);
        let eq = ConstraintResidual::CoincidentX {
            a: VarId(0),
            b: VarId(2),
        };
        assert!((eq.residual(&vars)).abs() < 1e-12);
    }

    #[test]
    fn horizontal_residual_for_level_line() {
        let vars = VarSet::new(vec![0.0, 5.0, 10.0, 5.0]);
        let eq = ConstraintResidual::Horizontal {
            x1: VarId(0),
            y1: VarId(1),
            x2: VarId(2),
            y2: VarId(3),
        };
        assert!((eq.residual(&vars)).abs() < 1e-12);
    }

    #[test]
    fn vertical_residual_for_vertical_line() {
        let vars = VarSet::new(vec![3.0, 0.0, 3.0, 8.0]);
        let eq = ConstraintResidual::Vertical {
            x1: VarId(0),
            y1: VarId(1),
            x2: VarId(2),
            y2: VarId(3),
        };
        assert!((eq.residual(&vars)).abs() < 1e-12);
    }

    #[test]
    fn distance_residual_matches_target() {
        let vars = VarSet::new(vec![0.0, 0.0, 3.0, 4.0]);
        let eq = ConstraintResidual::Distance {
            x1: VarId(0),
            y1: VarId(1),
            x2: VarId(2),
            y2: VarId(3),
            target: 5.0,
        };
        assert!((eq.residual(&vars)).abs() < 1e-12);
    }

    #[test]
    fn radius_residual_matches_target() {
        let vars = VarSet::new(vec![10.0]);
        let eq = ConstraintResidual::Radius {
            radius: VarId(0),
            target: 10.0,
        };
        assert!((eq.residual(&vars)).abs() < 1e-12);
    }

    #[test]
    fn equal_length_residual_matches_line_length_and_radius() {
        let vars = VarSet::new(vec![0.0, 0.0, 0.08, 0.0, 0.08]);
        let eq = ConstraintResidual::EqualLength {
            a: LengthTerm::Segment {
                x1: VarId(0),
                y1: VarId(1),
                x2: VarId(2),
                y2: VarId(3),
            },
            b: LengthTerm::Scalar { value: VarId(4) },
        };
        assert!(eq.residual(&vars).abs() < 1e-12);
    }

    #[test]
    fn equal_length_residual_preserves_si_tolerance() {
        let vars = VarSet::new(vec![0.0, 0.0, 0.08, 0.0, 0.0800000005]);
        let eq = ConstraintResidual::EqualLength {
            a: LengthTerm::Segment {
                x1: VarId(0),
                y1: VarId(1),
                x2: VarId(2),
                y2: VarId(3),
            },
            b: LengthTerm::Scalar { value: VarId(4) },
        };
        assert!(eq.residual(&vars).abs() < 1e-9);
        assert!(eq.residual(&vars).abs() > 1e-12);
    }

    #[test]
    fn parallel_residual_is_zero_for_same_or_opposite_direction() {
        let vars = VarSet::new(vec![
            0.0, 0.0, 2.0, 0.0, // a: +x
            4.0, 3.0, 1.0, 3.0, // b: -x
        ]);
        let eq = ConstraintResidual::Parallel {
            ax1: VarId(0),
            ay1: VarId(1),
            ax2: VarId(2),
            ay2: VarId(3),
            bx1: VarId(4),
            by1: VarId(5),
            bx2: VarId(6),
            by2: VarId(7),
        };
        assert!(eq.residual(&vars).abs() < 1e-12);
    }

    #[test]
    fn perpendicular_residual_is_zero_for_orthogonal_lines() {
        let vars = VarSet::new(vec![
            0.0, 0.0, 2.0, 0.0, // a: +x
            4.0, 3.0, 4.0, 8.0, // b: +y
        ]);
        let eq = ConstraintResidual::Perpendicular {
            ax1: VarId(0),
            ay1: VarId(1),
            ax2: VarId(2),
            ay2: VarId(3),
            bx1: VarId(4),
            by1: VarId(5),
            bx2: VarId(6),
            by2: VarId(7),
        };
        assert!(eq.residual(&vars).abs() < 1e-12);
    }

    #[test]
    fn direction_residual_is_scale_aware() {
        let unit = VarSet::new(vec![
            0.0, 0.0, 1.0, 0.0, // a: +x
            0.0, 0.0, 1.0, 1.0, // b: 45 degrees
        ]);
        let scaled = VarSet::new(vec![
            0.0, 0.0, 1.0e-6, 0.0, // a scaled by 1e-6
            0.0, 0.0, 1.0e6, 1.0e6, // b scaled by 1e6
        ]);
        let eq = ConstraintResidual::Parallel {
            ax1: VarId(0),
            ay1: VarId(1),
            ax2: VarId(2),
            ay2: VarId(3),
            bx1: VarId(4),
            by1: VarId(5),
            bx2: VarId(6),
            by2: VarId(7),
        };
        assert!((eq.residual(&unit) - eq.residual(&scaled)).abs() < 1e-12);
        assert!((eq.residual(&unit) - (0.5_f64).sqrt()).abs() < 1e-12);
    }

    #[test]
    fn direction_residual_does_not_divide_by_zero_for_degenerate_input() {
        let vars = VarSet::new(vec![0.0; 8]);
        let eq = ConstraintResidual::Perpendicular {
            ax1: VarId(0),
            ay1: VarId(1),
            ax2: VarId(2),
            ay2: VarId(3),
            bx1: VarId(4),
            by1: VarId(5),
            bx2: VarId(6),
            by2: VarId(7),
        };
        assert!(eq.residual(&vars).is_finite());
    }
}
