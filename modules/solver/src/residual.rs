use crate::variables::{VarId, VarSet};

/// Single scalar residual equation.
pub trait ResidualEquation: std::fmt::Debug + Send + Sync {
    fn involved_vars(&self) -> Vec<VarId>;
    fn residual(&self, vars: &VarSet) -> f64;
}

/// Built-in 2D constraint residuals.
#[derive(Debug, Clone)]
pub enum ConstraintResidual {
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
    FixedX {
        x: VarId,
        value: f64,
    },
    FixedY {
        y: VarId,
        value: f64,
    },
}

impl ConstraintResidual {
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
            Self::Distance { x1, y1, x2, y2, .. } => vec![*x1, *y1, *x2, *y2],
            Self::Radius { radius, .. } => vec![*radius],
            Self::FixedX { x, .. } => vec![*x],
            Self::FixedY { y, .. } => vec![*y],
        }
    }

    fn residual(&self, vars: &VarSet) -> f64 {
        match self {
            Self::CoincidentX { a, b } => vars.get(*a) - vars.get(*b),
            Self::CoincidentY { a, b } => vars.get(*a) - vars.get(*b),
            Self::Horizontal { y1, y2, .. } => vars.get(*y1) - vars.get(*y2),
            Self::Vertical { x1, x2, .. } => vars.get(*x1) - vars.get(*x2),
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
            Self::FixedX { x, value } => vars.get(*x) - value,
            Self::FixedY { y, value } => vars.get(*y) - value,
        }
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
}
