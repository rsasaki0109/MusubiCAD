use crate::jacobian::{finite_difference_jacobian_generic, Jacobian};
use crate::residual::{evaluate_residuals_generic, ConstraintResidual, ResidualEquation};
use crate::variables::VarSet;

/// Solver configuration.
#[derive(Debug, Clone)]
pub struct SolverOptions {
    pub max_iterations: usize,
    pub tolerance: f64,
    pub damping: f64,
    pub damping_growth: f64,
    pub max_damping: f64,
}

impl Default for SolverOptions {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            tolerance: 1e-9,
            damping: 1e-4,
            damping_growth: 10.0,
            max_damping: 1e6,
        }
    }
}

/// Result of a numeric solve attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct SolveOutput {
    pub vars: VarSet,
    pub iterations: usize,
    pub max_error: f64,
    pub converged: bool,
}

/// Gauss-Newton with optional Levenberg-Marquardt-style damping.
pub fn gauss_newton_solve(
    equations: &[ConstraintResidual],
    vars: VarSet,
    options: &SolverOptions,
) -> SolveOutput {
    gauss_newton_solve_generic(equations, vars, options)
}

/// Gauss-Newton solve for any equation type implementing [`ResidualEquation`].
pub fn gauss_newton_solve_generic<E: ResidualEquation>(
    equations: &[E],
    mut vars: VarSet,
    options: &SolverOptions,
) -> SolveOutput {
    let mut lambda = options.damping;
    let mut iterations = 0_usize;
    let mut max_error = f64::INFINITY;

    while iterations < options.max_iterations {
        let residuals = evaluate_residuals_generic(equations, &vars);
        max_error = residuals.iter().map(|r| r.abs()).fold(0.0, f64::max);

        if max_error < options.tolerance {
            return SolveOutput {
                vars,
                iterations,
                max_error,
                converged: true,
            };
        }

        let jacobian = finite_difference_jacobian_generic(equations, &vars);
        let step = match damped_normal_equations_step(&jacobian, &residuals, lambda) {
            Some(step) => step,
            None => break,
        };

        let mut trial = vars.clone();
        for (i, delta) in step.iter().enumerate() {
            trial.set(crate::variables::VarId(i as u32), vars.values()[i] - delta);
        }

        let trial_error = evaluate_residuals_generic(equations, &trial)
            .iter()
            .map(|r| r.abs())
            .fold(0.0, f64::max);

        if trial_error < max_error {
            vars = trial;
            lambda = (lambda / options.damping_growth).max(options.damping);
        } else {
            lambda = (lambda * options.damping_growth).min(options.max_damping);
        }

        iterations += 1;
    }

    SolveOutput {
        vars,
        iterations,
        max_error,
        converged: max_error < options.tolerance,
    }
}

/// Solve `(J^T J + lambda I) delta = J^T r` for the update `delta`.
fn damped_normal_equations_step(
    jacobian: &Jacobian,
    residuals: &[f64],
    lambda: f64,
) -> Option<Vec<f64>> {
    let n = jacobian.cols;
    if n == 0 {
        return Some(Vec::new());
    }

    let mut jt_j = vec![0.0; n * n];
    let mut jt_r = vec![0.0; n];

    for (row, r) in residuals.iter().enumerate().take(jacobian.rows) {
        let r = *r;
        for i in 0..n {
            let ji = jacobian.get(row, i);
            jt_r[i] += ji * r;
            for j in 0..n {
                jt_j[i * n + j] += ji * jacobian.get(row, j);
            }
        }
    }

    for i in 0..n {
        jt_j[i * n + i] += lambda;
    }

    solve_symmetric_positive_definite(&jt_j, &jt_r, n)
}

/// Cholesky-based solver for small dense systems.
fn solve_symmetric_positive_definite(a: &[f64], b: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; n * n];

    for i in 0..n {
        for j in 0..=i {
            let mut sum = a[i * n + j];
            for k in 0..j {
                sum -= l[i * n + k] * l[j * n + k];
            }
            if i == j {
                if sum <= 1e-14 {
                    return None;
                }
                l[i * n + j] = sum.sqrt();
            } else {
                l[i * n + j] = sum / l[j * n + j];
            }
        }
    }

    let mut y = vec![0.0; n];
    for i in 0..n {
        let mut sum = b[i];
        for k in 0..i {
            sum -= l[i * n + k] * y[k];
        }
        y[i] = sum / l[i * n + i];
    }

    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = y[i];
        for k in (i + 1)..n {
            sum -= l[k * n + i] * x[k];
        }
        x[i] = sum / l[i * n + i];
    }

    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::residual::ConstraintResidual;
    use crate::variables::{VarId, VarSet};

    #[test]
    fn solves_simple_rectangle() {
        // c0(0,1) c1(2,3) c2(4,5) c3(6,7) — 80 x 60 rectangle at origin
        let eqs = vec![
            ConstraintResidual::FixedX {
                x: VarId(0),
                value: 0.0,
            },
            ConstraintResidual::FixedY {
                y: VarId(1),
                value: 0.0,
            },
            ConstraintResidual::Horizontal {
                x1: VarId(0),
                y1: VarId(1),
                x2: VarId(2),
                y2: VarId(3),
            },
            ConstraintResidual::Horizontal {
                x1: VarId(6),
                y1: VarId(7),
                x2: VarId(4),
                y2: VarId(5),
            },
            ConstraintResidual::Vertical {
                x1: VarId(0),
                y1: VarId(1),
                x2: VarId(6),
                y2: VarId(7),
            },
            ConstraintResidual::Vertical {
                x1: VarId(2),
                y1: VarId(3),
                x2: VarId(4),
                y2: VarId(5),
            },
            ConstraintResidual::Distance {
                x1: VarId(0),
                y1: VarId(1),
                x2: VarId(2),
                y2: VarId(3),
                target: 80.0,
            },
            ConstraintResidual::Distance {
                x1: VarId(0),
                y1: VarId(1),
                x2: VarId(6),
                y2: VarId(7),
                target: 60.0,
            },
        ];

        let vars = VarSet::new(vec![0.0, 0.0, 70.0, 5.0, 75.0, 58.0, 5.0, 55.0]);
        let out = gauss_newton_solve(&eqs, vars, &SolverOptions::default());
        assert!(out.converged, "max_error={}", out.max_error);
        assert!((out.vars.get(VarId(2)) - 80.0).abs() < 1e-4);
        assert!((out.vars.get(VarId(7)) - 60.0).abs() < 1e-4);
    }
}
