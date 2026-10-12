# ADR-051: Analytic sketch trim and extend plans

- Status: Accepted
- Date: 2026-10-12
- Task: MCAD-P13-002

Plan an explicit start/end trim or extension of a stored line or arc against a
line/circle/arc cutter in the same sketch. Solve a copy using current parameters
and constraints first, then use analytic intersections. Return a reviewable
DesignPatch; CLI/Agent/MCP application uses existing verification/transactions.

Use SI metre coordinates and radian angles, a 1e-9 m contact tolerance and
1e-12 dimensionless parallel-direction tolerance. Bound coordinates/radii to
1e6 m, reject degenerate, coincident and tangent cuts, and preserve a positive
arc sweep smaller than one full turn. Cutters use their finite line/arc extent;
only the selected target endpoint can extend. Sort eligible intersections by
target parameter and require explicit selection if more than one remains.
There is no curve sampling or implicit unit/association inference.

Preserve the target curve ID, centre/radius source expressions and unaffected
endpoint/angle. A caller supplies a new endpoint ID rather than moving a shared
point; the old point remains available to neighbouring curves/constraints.
Rectangle-derived edges require explicit source conversion. Only line and arc
targets are initially supported; circles can be cutters. Arc angle edits are
explicit fixed source values, not a persistent link to the cutter.

Add a stable-ID `replace_sketch_entity` patch operation with reference/type and
collision checks, semantic modification/change impact and rebase snapshots.
Constraint removal is explicit and opt-in. Consumers of a trimmed closed loop
may require explicit closing/rebinding operations in the same patch.

Add a unit-bearing expected sketch point position, measured after actual
parameter resolution and constraint solving. A constraint that moves the new
endpoint away from the planned intersection must cause verified refusal, even
if the resulting body happens to regenerate. The planner includes a revision
guard, preventing stale application. No backend changes bypass DesignPatch.

Named tests: `analytic_trim_extend_intersections_and_selection`,
`trim_preserves_shared_endpoints_and_explicit_constraints`,
`solver_displaced_trim_endpoint_refuses_verified_application`, and
`trim_extend_round_trip_history_and_native_geometry`. Include source/patch
schema updates, additive migration policy, old/new round trips, Mock/native
profile/dimension/volume evidence, examples, guide/skill and feasible eval.
Kernel execution reuses existing line/arc wires; no new OCCT type or operation.
