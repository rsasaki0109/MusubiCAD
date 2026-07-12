use crate::jacobian::finite_difference_jacobian_generic;
use crate::residual::{ConstraintResidual, ResidualEquation};
use crate::variables::VarSet;

const RANK_TOL: f64 = 1e-6;

/// Estimate remaining degrees of freedom.
///
/// `dof = n_vars - rank(J)`
pub fn estimate_dof(equations: &[ConstraintResidual], vars: &VarSet) -> i32 {
    estimate_dof_generic(equations, vars)
}

/// Estimate DOF for any equation type implementing [`ResidualEquation`].
pub fn estimate_dof_generic<E: ResidualEquation>(equations: &[E], vars: &VarSet) -> i32 {
    let n_vars = vars.len();
    if n_vars == 0 {
        return 0;
    }
    let jac = finite_difference_jacobian_generic(equations, vars);
    let rank = matrix_rank(&jac);
    (n_vars as i32) - (rank as i32)
}

fn matrix_rank(jacobian: &crate::jacobian::Jacobian) -> usize {
    let rows = jacobian.rows;
    let cols = jacobian.cols;
    if rows == 0 || cols == 0 {
        return 0;
    }

    let mut a = jacobian.data.clone();
    let mut rank = 0_usize;
    let row_limit = rows.min(cols);

    for col in 0..cols {
        if rank == row_limit {
            break;
        }

        let mut pivot_row = rank;
        let mut max_val = a[pivot_row * cols + col].abs();
        for row in (rank + 1)..rows {
            let val = a[row * cols + col].abs();
            if val > max_val {
                max_val = val;
                pivot_row = row;
            }
        }

        if max_val < RANK_TOL {
            continue;
        }

        if pivot_row != rank {
            for c in 0..cols {
                a.swap(pivot_row * cols + c, rank * cols + c);
            }
        }

        let pivot = a[rank * cols + col];
        for row in (rank + 1)..rows {
            let factor = a[row * cols + col] / pivot;
            if factor.abs() < RANK_TOL {
                continue;
            }
            for c in col..cols {
                a[row * cols + c] -= factor * a[rank * cols + c];
            }
        }

        rank += 1;
    }

    rank
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::residual::ConstraintResidual;
    use crate::variables::{VarId, VarSet};

    #[test]
    fn fully_constrained_rectangle_has_zero_dof() {
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
                target: 10.0,
            },
            ConstraintResidual::Distance {
                x1: VarId(0),
                y1: VarId(1),
                x2: VarId(6),
                y2: VarId(7),
                target: 5.0,
            },
        ];
        // Use a non-degenerate configuration; rank at all-zero is unreliable for distance constraints.
        let vars = VarSet::new(vec![0.0, 0.0, 10.0, 0.0, 10.0, 5.0, 0.0, 5.0]);
        let dof = estimate_dof(&eqs, &vars);
        assert_eq!(dof, 0);
    }

    #[test]
    fn under_constrained_point_has_positive_dof() {
        let eqs = vec![ConstraintResidual::FixedX {
            x: VarId(0),
            value: 0.0,
        }];
        let vars = VarSet::new(vec![0.0, 0.0]);
        assert!(estimate_dof(&eqs, &vars) > 0);
    }
}
