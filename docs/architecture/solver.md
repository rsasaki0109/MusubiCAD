# Constraint Solver

MusubiCAD uses a numeric 2D geometric constraint solver in `opencad-solver`.

## Algorithm

1. Build residual equations from sketch constraints.
2. Compute Jacobian by central finite differences (`step = 1e-8`).
3. Iterate Gauss-Newton with Levenberg-Marquardt-style damping on `J^T J`.
4. Stop when `max(|residual|) < 1e-9` or `50` iterations.

## Supported constraints (MVP)

| Constraint | Residual |
|---|---|
| Coincident | `xa - xb`, `ya - yb` |
| Horizontal | `y1 - y2` |
| Vertical | `x1 - x2` |
| Distance | `‖p2 - p1‖ - target` |
| Radius / Diameter | `r - target` (diameter uses `target/2`) |
| Fixed anchor | `x - x0`, `y - y0` (first point) |

## Units

- Internal SI: meters.
- Expression parser accepts `mm`, `cm`, `m`, `in`, or bare numbers (interpreted as meters).

## DOF diagnostics

```
dof = n_variables - rank(J)
```

| Status | Meaning |
|---|---|
| `Solved` | Converged, `dof <= 0` |
| `UnderConstrained` | Converged or not, `dof > 0` |
| `OverConstrained` | More equations than variables, possible redundancy |
| `Failed` | Did not converge |

## Tolerances

| Parameter | Default |
|---|---|
| Residual tolerance | `1e-9` |
| FD step | `1e-8` |
| Damping λ | `1e-4` (adaptive) |
| Rank tolerance | `1e-6` |

## Limitations (MVP)

- No parallel / perpendicular sketch constraints yet.
- Equal constraint not wired to solver.
- Rectangle must be expanded to points + lines before solving.
- No drag solving or redundancy decomposition (Phase 2).

## Crate boundaries

- `opencad-solver`: numeric engine only, no sketch/file semantics.
- `opencad-sketch::solve`: maps `Sketch` → residuals → writes back point coords.

## Tests

```bash
cargo test -p opencad-solver
cargo test -p opencad-sketch solve::
```
