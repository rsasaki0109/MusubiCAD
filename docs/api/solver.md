# Sketch solver API

`opencad-sketch::solve::solve_sketch` maps serialized sketch constraints to
`opencad-solver` residual equations and writes the solved point and radius
values back to the sketch.

## Direction constraints

`Constraint::Parallel` and `Constraint::Perpendicular` reference two line
entities by `line_a` and `line_b`.  The numeric engine uses normalized
direction residuals: the 2D cross product divided by both line lengths for
parallel, and the dot product divided by both line lengths for perpendicular.
Both residuals are dimensionless (`sin(angle)` and `cos(angle)`), so their
magnitude is independent of the lines' absolute scale.

Direction constraints validate both referenced lines before solving.  A line
whose initial length is non-finite or at/below
`opencad_solver::DIRECTION_DEGENERACY_TOLERANCE_M` (`1e-12 m`) returns a
validation error rather than constructing an undefined direction.  The
same tolerance is checked again before solved coordinates are written back, so
a numeric iteration cannot satisfy the relation by collapsing a line. The
serialized sketch constraint shape is unchanged; only the transient solver
residual adds direction equations.

## Angle, midpoint, symmetric, and tangent

| Constraint | Fields | Meaning |
|---|---|---|
| `angle` | `line_a`, `line_b`, `expr` | Directed angle from `line_a` to `line_b`; `expr` is an angle such as `30 deg` or a parameter |
| `midpoint` | `point`, `line` | `point` is the midpoint of `line` |
| `symmetric` | `a`, `b`, `line` | Points `a` and `b` mirror each other across `line` |
| `tangent` | `line`, `curve` | `line` is tangent to the circle or arc `curve` |

The angle residual is `sin(angle - target)`, which is dimensionless and
continuous. It also vanishes at `target + π`, the reversed direction, so the
solver keeps the branch nearest the current sketch. Angle, symmetric, and
tangent constraints validate their lines against the same `1e-12 m`
degeneracy tolerance as direction constraints. A tangent `curve` that is not
a circle or arc is a validation error.

```json
{ "type": "angle", "id": "con:tilt", "line_a": "ent:e0", "line_b": "ent:e1", "expr": "30 deg" }
```

## Arc endpoints

An arc with `start_point` and `end_point` holds those points on the arc
(ADR-021). For each endpoint, the solver adds the residuals
`x − (cx + r·cos θ)` and `y − (cy + r·sin θ)`, in metres, where θ is
`start_angle` or `end_angle`.

- Angles are literals in radians or constant angle expressions such as
  `90 deg`.
- The radius stays a solver variable, so radius constraints move the
  endpoints.
- Lines, and arcs with both endpoints, form closed profiles through shared
  point IDs.

## Equal targets

`EqualTarget` uses an explicit canonical wire representation:

- `LineLength(entity_id)` serializes as `{ "line": "ent:..." }` and
  references a line entity's Euclidean length.
- `Radius(entity_id)` serializes as `{ "radius": "ent:..." }` and references
  a circle or arc entity's radius.

The deserializer also accepts the pre-disambiguation bare string form (for
example, `"ent:line_1"`) and interprets it as `LineLength`. This preserves old
`.ocad` files while ensuring a newly written document cannot change a radius
target into a line target during a round trip.

Both targets are lengths and are compared in internal SI meters. Therefore a
line-length target and a radius target may be mixed in one `Equal` constraint.
The solver rejects a target whose entity kind does not match its variant, and
reports a missing entity as an error before solving.

The numeric engine represents these terms with
`ConstraintResidual::EqualLength` and `LengthTerm`; the latter is an internal
solver API and is not serialized into `.ocad` documents.

## Solver diagnostics

`solve_with_diagnostics` returns the existing `SolveOutput` together with a
deterministic `SolveStatus`:

- `Solved` means the final maximum residual is at or below
  `SolverOptions::tolerance` and the Jacobian has zero remaining DOF and no
  redundant rows.
- `UnderConstrained { dof, .. }` means the Jacobian rank leaves `dof` free
  variables.  This status is also returned for a converged system with no
  equations.
- `OverConstrained { redundant, .. }` means the residual converged, there are
  no remaining DOF, and `equation_count - rank(J)` rows are redundant.
  Redundancy is rank-based, rather than inferred from the equation/variable
  count.  A converged system can have both free DOF and redundant rows; in
  that case the status remains `UnderConstrained`, while
  `count_redundant_equations_generic` reports the redundant-row count.
- `Contradictory { redundant, message, .. }` means the residual is outside the
  contradiction threshold and `rank([J | residual]) > rank(J)`, so an
  over-constraining row conflicts with the independent rows.
- `NonConvergent { dof, message, .. }` means the iteration budget ended (or a
  non-finite residual was encountered) without convergence or a proven
  contradiction.  The former `Failed` variant remains available for source
  compatibility but is no longer emitted by the diagnostic solver.

The shared rank tolerance is
`opencad_solver::RANK_TOLERANCE` (`1e-6`, dimensionless).  The contradiction
threshold is `SolverOptions::tolerance` multiplied by
`opencad_solver::CONTRADICTION_ERROR_MULTIPLIER` (`10`).  Numeric convergence
uses `SolverOptions::tolerance` with an inclusive `<=` comparison, and the
returned `SolveOutput::max_error` is always recomputed for the returned
variables.  The finite-difference step and normal-equation pivot tolerance are
also named public constants: `FINITE_DIFFERENCE_STEP` (`1e-8`) and
`NORMAL_EQUATION_PIVOT_TOLERANCE` (`1e-14`).

A zero-rank Jacobian is reported as non-convergent rather than contradictory:
a singular nonlinear linearization cannot by itself prove that no solution
exists.

For generic residual equations, use
`estimate_rank`, `estimate_dof_generic`, or
`count_redundant_equations_generic(equations, &vars)` to inspect the same rank
calculation used by diagnostics.  The one-argument
`count_redundant_equations` helper is retained as a legacy duplicate-equation
check; it does not replace the rank-based count.

Sketch and assembly consumers do not commit the numeric trial variables for
`Contradictory`, `NonConvergent`, or legacy `Failed` results. Sketches retain
their prior coordinates and store the diagnostic message in `SolveState`;
assemblies return a validation error containing that message.

## Regression fixture

[`examples/sketch_constraints_regression.ocad.d`](../../examples/sketch_constraints_regression.ocad.d)
is the deterministic serialized regression fixture for the supported
constraint combinations. Its `sketch:mixed` case combines `Equal`, `Parallel`,
and `Perpendicular` with coincident, horizontal, and unit-bearing distance
constraints. Its `sketch:radius` case covers an explicit circle/arc radius
`Equal` target. The same document also fixes under-constrained, rank-redundant
over-constrained, and contradictory outcomes.

`modules/file/tests/sketch_regression.rs` validates the expanded directory,
round-trips every canonical file, checks mm/cm/m/in conversion to SI meters,
and solves every case twice. Coordinates, DOF, iteration/error diagnostics,
and the persisted solve state must match exactly across the repeated runs.

The fixture's `checksums.json` is intentionally part of the golden contract.
It is regenerated with the normal `write_expanded_dir`/`ChecksumManifest`
canonical JSON procedure whenever a fixture file changes; the golden update is
therefore limited to recording the new serialized regression evidence. No
`.ocad` schema version bump is required for the equal-target wire fix: legacy
strings remain readable and canonical writes use the explicit object form.
