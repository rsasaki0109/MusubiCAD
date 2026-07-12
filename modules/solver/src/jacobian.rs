use crate::residual::{ConstraintResidual, ResidualEquation};
use crate::variables::{VarId, VarSet};

const FD_STEP: f64 = 1e-8;

/// Dense Jacobian matrix: rows = equations, cols = variables.
#[derive(Debug, Clone, PartialEq)]
pub struct Jacobian {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f64>,
}

impl Jacobian {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
    }

    pub fn get(&self, row: usize, col: usize) -> f64 {
        self.data[row * self.cols + col]
    }

    pub fn set(&mut self, row: usize, col: usize, value: f64) {
        self.data[row * self.cols + col] = value;
    }
}

/// Compute Jacobian by central finite differences.
pub fn finite_difference_jacobian(equations: &[ConstraintResidual], vars: &VarSet) -> Jacobian {
    finite_difference_jacobian_generic(equations, vars)
}

/// Compute Jacobian for any equation type implementing [`ResidualEquation`].
pub fn finite_difference_jacobian_generic<E: ResidualEquation>(
    equations: &[E],
    vars: &VarSet,
) -> Jacobian {
    let cols = vars.len();
    let rows = equations.len();
    let mut jac = Jacobian::new(rows, cols);

    for (row, eq) in equations.iter().enumerate() {
        let involved = eq.involved_vars();
        for var_idx in 0..cols {
            let var_id = VarId(var_idx as u32);
            if !involved.contains(&var_id) {
                continue;
            }
            let mut v_plus = vars.clone();
            let mut v_minus = vars.clone();
            v_plus.set(var_id, vars.get(var_id) + FD_STEP);
            v_minus.set(var_id, vars.get(var_id) - FD_STEP);
            let r_plus = eq.residual(&v_plus);
            let r_minus = eq.residual(&v_minus);
            jac.set(row, var_idx, (r_plus - r_minus) / (2.0 * FD_STEP));
        }
    }

    jac
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::residual::ConstraintResidual;

    #[test]
    fn jacobian_for_fixed_x_is_one() {
        let vars = VarSet::new(vec![3.0, 0.0]);
        let eqs = vec![ConstraintResidual::FixedX {
            x: VarId(0),
            value: 3.0,
        }];
        let jac = finite_difference_jacobian(&eqs, &vars);
        assert!((jac.get(0, 0) - 1.0).abs() < 1e-5);
        assert!(jac.get(0, 1).abs() < 1e-5);
    }
}
