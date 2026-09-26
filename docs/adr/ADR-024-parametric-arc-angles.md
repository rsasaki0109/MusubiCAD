# ADR-024: Parametric arc angles

Status: Accepted  
Date: 2026-09-26  
Roadmap: MCAD-P7-023

## Context

ADR-021 let arcs join profiles through endpoint points, but their start and
end angles had to be literals or constant angle expressions. A sector,
slot, or rounded corner whose opening angle is a design parameter could not
follow that parameter. Arc angles are entity coordinates, not constraints,
so the parameter pass that resolves dimension expressions before solving
never reached them.

## Decision

1. **Expressions allowed.** An arc's `start_angle` and `end_angle` may be
   angle expressions that name parameters (`sweep_angle`, `90 deg`,
   `sweep_angle / 2`), in the same syntax as angle constraints.
2. **Resolved before solving.** The pass that resolves sketch constraints
   (`resolve_sketch_constraints`, used by regeneration and by dry-run
   freedom checks) evaluates each parametric arc angle in radians. It
   stores the result in `ArcEntity::resolved_angles_rad`.
3. **Not persisted.** The resolved radians are never serialized. The
   expressions stay the source of truth in the document.
4. **Consumers.** The solver's endpoint residuals and the kernel profile
   segments use the resolved radians when present (`arc_angles`).
   Otherwise they parse literal or constant angles as before.

## Consequences

- An angle edit moves the arc's endpoint points, and every line joined to
  them follows, so profiles stay closed.
- Unknown parameter names in arc angle expressions are rejected by the
  existing sketch-expression validation.

## Evidence

A slanted slot (`examples/agent/author_slanted_slot_patch.json`) turns with
a `slot_angle` parameter. Its arc angles are `slot_angle ± 90 deg`, and
tangent constraints join the straight sides to the end arcs. It is fully
constrained. At 30° and again after an edit to 60°, the volume matches
`(L·w + πr²)·h` within 1e-12 m³. The tessellated extents match
`L·cos θ + w` and `L·sin θ + w` within 0.1 mm.

A sector test builds two radii and an arc whose end angle is
`sweep_angle`. The sketch is fully constrained, with no dry-run warnings.
OCCT matches `θ/2·r²·h` within 1e-12 m³ at 90°, and again after an edit to
120°. The serialized sketch still holds `"end_angle": "sweep_angle"`.
