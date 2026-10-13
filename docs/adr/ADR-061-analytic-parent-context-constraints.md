# ADR-061: Analytic parent-context connector constraints

Status: Accepted scoped implementation decision for MCAD-P13-012

ADR-059/060 and P13-011 provide independent coordinates and physical leaves.
P13-012 adds parent-owned constraints which solve those coordinates against
named source connectors. The source child and authored placements never change.

## Admitted system

Outer instances must be explicitly fixed, without patterns or root coupling
relations. Their existing rest mates must solve first. Each context driver is a
unique declared independent child joint immediately below the child's grounded
kinematic root. Its moving named connector must belong to that exact child link,
which references a part. The target is a named parent connector on a different
fixed direct part instance. Structured instance paths, owning assembly document
IDs and exact ConnectorIds bind both ends. No inferred geometric anchors.

Child joints can retain coupled followers; these add no independent variables.
Every independent movable child driver is either explicitly grounded by a
unit-bearing saved coordinate or solved by exactly one parent constraint. Reject
missing/duplicate drivers, follower inputs, nondeclared motion, contradictory
saved/solved coordinates, unknown owners/links/connectors, unfixed outer links,
nested context, patterns and unsupported constraint kinds. Ordinary rigid copies
remain at authored zero and independently articulated copies remain independent.

Use the existing `set_instance_articulation` transaction with an optional strict
`context_constraints` array (maximum 64). Any constrained component requires a
new `flexible_assembly` source-kind discriminator: P13-011 readers must refuse,
not ignore parent constraints. Guard selection is component-wide and automatic.
Legacy migration leaves the array absent/empty and does not introduce behavior.

## Deterministic analytic solve

Signed distance projects the moving connector origin minus the parent target
origin onto the target frame's +Z normal. For a prismatic driver the residual is
linear in metres: solve once by dividing the requested residual by the projected
world motion axis. Reject an absolute projected slope <= 1e-9. This includes
coincident planes as zero signed distance; full 3D coincidence is not claimed.

Signed angle measures from the target +Z normal to the moving +Z normal about
the revolute world axis. Both normals must lie in the axis-normal plane within
1e-9. Inputs are within [-pi, pi] radians, with an explicit integer `turns`
(-32..32) selecting the displacement branch. Compute the rest angle with atan2,
then q = requested angle - rest angle + 2*pi*turns. Do not clamp, wrap or guess a
branch. The finite declared limits, omitted zero admission and coupled limits
remain authoritative. Continuous joints are refused for context drivers.

Grounded ancestor displacements participate in evaluating connector frames.
Context drivers are siblings beneath the fixed root, so they cannot change one
another's connector equations. This is one deterministic diagonal variable /
constraint system; it uses no sampling, random seeds or nonlinear local minima.
Re-evaluate every residual after applying the solution (1e-9 m/rad tolerance).
Angle residuals compare periodic orientation; the selected displacement stays
unwrapped. An admitted completely bound system reports DOF zero; unsupported full hierarchy
must not receive a root-only DOF certificate. Bounds/mass/interference/export
use the same solved physical leaf scene from P13-011. Motion/reach/URDF/HTML
mechanics remain explicit refusals, as permitted by ADR-059's consumer gate.

## Acceptance and limits

Both Mock and OCCT: independent box volumes/centres/bounds, parent transforms,
25 mm displacement, 90 degree rotation, parent-only constraint edits moving one
copy, second-copy isolation and child-byte invariance. Native internal/external
collision pairs remain complete; Mock exact mesh intersection refuses.
Strict source/schema/old-reader refusal, migration/expanded/packed round trips,
semantic diff, verified review/apply, stale dependencies, Undo/Redo and refusal
preservation must accompany a runnable example, guide, skill and independent
real-MCP/eval evidence. No general six-DOF nonlinear mate or closed-loop system
is claimed. Maturity can change only after this complete admitted workflow merges.
