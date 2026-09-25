# Constraint Solver

MusubiCAD uses a numeric 2D geometric constraint solver in `opencad-solver`.

## Algorithm

1. Build residual equations from sketch constraints.
2. Compute Jacobian by central finite differences (`FINITE_DIFFERENCE_STEP = 1e-8`).
3. Iterate Gauss-Newton with Levenberg-Marquardt-style damping on `J^T J`.
4. Stop when `max(|residual|) <= SolverOptions::tolerance` (`1e-9` by
   default) or `DEFAULT_MAX_ITERATIONS` (`50`) iterations.  The final error is
   evaluated again at the returned variables, including after the last trial.

## Supported constraints (MVP)

| Constraint | Residual |
|---|---|
| Coincident | `xa - xb`, `ya - yb` |
| Horizontal | `y1 - y2` |
| Vertical | `x1 - x2` |
| Parallel | `cross(da, db) / (‖da‖‖db‖)` (`sin(angle)`, dimensionless) |
| Perpendicular | `dot(da, db) / (‖da‖‖db‖)` (`cos(angle)`, dimensionless) |
| Distance | `‖p2 - p1‖ - target` |
| Radius / Diameter | `r - target` (diameter uses `target/2`) |
| Equal | `length(a) - length(b)` for line lengths and circle/arc radii |
| Angle | `sin(angle - target)` from the normalized cross/dot products (dimensionless) |
| Midpoint | `p - (a + b) / 2` per axis (meters) |
| Symmetric | signed distance of the midpoint of `a`, `b` to the line, and projection of `b - a` onto the line direction (meters) |
| Tangent | `abs(distance(center, line)) - r` for a line and a circle or arc (meters) |
| Fixed anchor | `x - x0`, `y - y0` (first point) |

## Units

- Internal SI: meters.
- Expression parser accepts `mm`, `cm`, `m`, `in`, or bare numbers (interpreted as meters).
- Angle expressions accept `deg`, `rad`, or bare numbers (interpreted as radians);
  parameter references resolve through the angle evaluator before solving.
- Equal line-length and radius targets are all compared in internal meters; mixed
  line/radius targets are valid because both target kinds represent lengths.
- Parallel and perpendicular directions normalize the 2D cross or dot product by
  both line lengths.  Their residuals are dimensionless and therefore remain
  comparable when line lengths differ by scale.  A line at or below
  `1e-12 m` is degenerate for direction constraints and is rejected before
  solving and again before solved coordinates are committed to the sketch.

## DOF diagnostics

```
dof = n_variables - rank(J)
```

`rank(J)` uses deterministic row-normalized elimination with the shared
`RANK_TOLERANCE` (`1e-6`) threshold.  Redundant equations are counted as
`equation_count - rank(J)`, not as `equation_count - variable_count` and not
only by comparing duplicate equation values.  A non-converged system is
checked with the augmented rank test `rank([J | residual]) > rank(J)`; when it
passes and the error exceeds `tolerance * CONTRADICTION_ERROR_MULTIPLIER`
(`10`), the diagnostics report a contradictory over-constraint.  Otherwise
the result is explicitly non-convergent.  A zero-rank Jacobian is always kept
in the non-convergent category because a singular nonlinear linearization does
not prove global inconsistency.

| Status | Meaning |
|---|---|
| `Solved` | Converged, `dof <= 0` |
| `UnderConstrained` | Converged with `dof > 0` |
| `OverConstrained` | Converged with zero DOF and one or more rank-redundant equations |
| `Contradictory` | Over-constraining residual is outside tolerance and the augmented rank is larger |
| `NonConvergent` | Did not converge and no contradiction was proven |
| `Failed` | Legacy compatibility variant; new diagnostics do not emit it |

## Tolerances

| Parameter | Default |
|---|---|
| Residual tolerance | `SolverOptions::tolerance` (`1e-9`) |
| Max iterations | `SolverOptions::max_iterations` (`50`) |
| FD step | `FINITE_DIFFERENCE_STEP` (`1e-8`) |
| Damping λ | `SolverOptions::damping` (`1e-4`, adaptive) |
| Damping growth | `SolverOptions::damping_growth` (`10`) |
| Max damping | `SolverOptions::max_damping` (`1e6`) |
| Normal-equation pivot | `NORMAL_EQUATION_PIVOT_TOLERANCE` (`1e-14`) |
| Rank tolerance | `RANK_TOLERANCE` (`1e-6`) |
| Contradiction multiplier | `CONTRADICTION_ERROR_MULTIPLIER` (`10`) |
| Direction degeneracy | `1e-12 m` |

## Limitations (MVP)

- Rectangle must be expanded to points + lines before solving.
- No drag solving or redundancy decomposition (future solver work).

## Crate boundaries

- `opencad-solver`: numeric engine only, no sketch/file semantics.
- `opencad-sketch::solve`: maps `Sketch` → residuals → writes back point coords.

## Tests

```bash
cargo test -p opencad-solver
cargo test -p opencad-sketch solve::
```
