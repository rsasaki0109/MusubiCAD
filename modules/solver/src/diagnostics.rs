use crate::dof::estimate_dof_generic;
use crate::numeric::{gauss_newton_solve_generic, SolveOutput, SolverOptions};
use crate::residual::{ConstraintResidual, ResidualEquation};
use crate::variables::VarSet;

/// Solver outcome with DOF and redundancy diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub enum SolveStatus {
    Solved {
        iterations: usize,
        max_error: f64,
    },
    UnderConstrained {
        dof: i32,
        iterations: usize,
        max_error: f64,
    },
    OverConstrained {
        redundant: usize,
        iterations: usize,
        max_error: f64,
    },
    Failed {
        message: String,
        iterations: usize,
        max_error: f64,
    },
}

impl SolveStatus {
    pub fn is_solved(&self) -> bool {
        matches!(self, Self::Solved { .. })
    }
}

/// Solve and classify the result.
pub fn solve_with_diagnostics(
    equations: &[ConstraintResidual],
    vars: VarSet,
    options: &SolverOptions,
) -> (SolveOutput, SolveStatus) {
    solve_with_diagnostics_generic(equations, vars, options)
}

/// Solve and classify the result for any [`ResidualEquation`] type.
pub fn solve_with_diagnostics_generic<E: ResidualEquation>(
    equations: &[E],
    vars: VarSet,
    options: &SolverOptions,
) -> (SolveOutput, SolveStatus) {
    let n_vars = vars.len();
    let n_eqs = equations.len();

    if n_vars == 0 {
        return (
            SolveOutput {
                vars: vars.clone(),
                iterations: 0,
                max_error: 0.0,
                converged: true,
            },
            SolveStatus::Solved {
                iterations: 0,
                max_error: 0.0,
            },
        );
    }

    let output = gauss_newton_solve_generic(equations, vars, options);
    let dof = estimate_dof_generic(equations, &output.vars);

    let redundant = if n_eqs > n_vars {
        n_eqs.saturating_sub(n_vars)
    } else {
        0
    };

    let status = if output.converged {
        if dof > 0 {
            SolveStatus::UnderConstrained {
                dof,
                iterations: output.iterations,
                max_error: output.max_error,
            }
        } else if redundant > 0 && output.max_error > options.tolerance * 10.0 {
            SolveStatus::OverConstrained {
                redundant,
                iterations: output.iterations,
                max_error: output.max_error,
            }
        } else {
            SolveStatus::Solved {
                iterations: output.iterations,
                max_error: output.max_error,
            }
        }
    } else if dof > 0 {
        SolveStatus::UnderConstrained {
            dof,
            iterations: output.iterations,
            max_error: output.max_error,
        }
    } else if redundant > 0 {
        SolveStatus::OverConstrained {
            redundant,
            iterations: output.iterations,
            max_error: output.max_error,
        }
    } else {
        SolveStatus::Failed {
            message: "solver did not converge".into(),
            iterations: output.iterations,
            max_error: output.max_error,
        }
    };

    (output, status)
}

/// Detect obviously duplicate equations (same type and variables).
pub fn count_redundant_equations(equations: &[ConstraintResidual]) -> usize {
    let mut keys = Vec::new();
    let mut redundant = 0_usize;
    for eq in equations {
        let key = format!("{eq:?}");
        if keys.contains(&key) {
            redundant += 1;
        } else {
            keys.push(key);
        }
    }
    redundant
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::residual::ConstraintResidual;
    use crate::variables::{VarId, VarSet};

    #[test]
    fn classifies_under_constrained_system() {
        let eqs = vec![ConstraintResidual::FixedX {
            x: VarId(0),
            value: 0.0,
        }];
        let vars = VarSet::new(vec![0.0, 0.0]);
        let (_, status) = solve_with_diagnostics(&eqs, vars, &SolverOptions::default());
        assert!(matches!(status, SolveStatus::UnderConstrained { .. }));
    }

    #[test]
    fn detects_duplicate_equations() {
        let eqs = vec![
            ConstraintResidual::FixedX {
                x: VarId(0),
                value: 0.0,
            },
            ConstraintResidual::FixedX {
                x: VarId(0),
                value: 0.0,
            },
        ];
        assert_eq!(count_redundant_equations(&eqs), 1);
    }
}
