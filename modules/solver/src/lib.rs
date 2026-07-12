//! Numeric constraint solver for 2D sketches.
//!
//! # Assumptions
//!
//! - All lengths are in meters internally (SI) when parsed from expressions.
//! - Residual tolerance default: `1e-9`.
//! - Finite-difference step: `1e-8`.
//! - Distance constraints use Euclidean distance.
//! - Horizontal means equal Y; vertical means equal X.
//! - Damping uses a diagonal Levenberg-Marquardt-style term on `J^T J`.
//! - DOF estimate uses numeric rank of the Jacobian.
//!
//! See `docs/architecture/solver.md` for full details.

pub mod diagnostics;
pub mod dof;
pub mod jacobian;
pub mod numeric;
pub mod residual;
pub mod variables;

pub use diagnostics::{
    count_redundant_equations, solve_with_diagnostics, solve_with_diagnostics_generic, SolveStatus,
};
pub use dof::{estimate_dof, estimate_dof_generic};
pub use jacobian::{finite_difference_jacobian, finite_difference_jacobian_generic, Jacobian};
pub use numeric::{gauss_newton_solve, gauss_newton_solve_generic, SolveOutput, SolverOptions};
pub use residual::{
    evaluate_residuals, evaluate_residuals_generic, ConstraintResidual, ResidualEquation,
};
pub use variables::{point_x, point_y, radius_var, VarId, VarSet, VariableRegistry};
