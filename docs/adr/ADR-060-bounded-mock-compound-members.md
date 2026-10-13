# ADR-060: Bounded exact Mock compound members

Status: Accepted for MCAD-P13-011; no new source representation

ADR-059 requires real Mock/OCCT placement and analytic mass/bounds evidence for
independently articulated copies. The legacy Mock kernel uses synthetic IDs for
ordinary feature plumbing; those IDs are not physical transformed geometry.
The imported closed triangle-solid path already has exact rigid transforms,
mass/bounds and tessellation, but multi-member compounds previously refused.

Preserve legacy synthetic behavior for existing tests and add an explicit neutral
`has_exact_rigid_placement` capability. Native kernels honor the existing rigid
transform contract; Mock returns true only for its tracked analytic/mesh bodies
and supported tracked compounds. Articulated scenes reject synthetic leaves
instead of claiming that synthetic cube IDs prove rotation, dimensions or mass.

Store bounded runtime-only Mock compound member handles for closed imported mesh
solids and recursively supported compounds. Members remain independent bodies;
compound mass/area/volume is additive, centre is volume-weighted, bounds are the
union of member bounds, tessellation merges actual member meshes, and rigid
transforms apply to every member. This is an assembly compound, not a Boolean
union or a claim that overlapping parts form one valid manufactured solid.

Admit at most 100000 members and depth 64. No source/schema import admission is
relaxed: each imported STL remains one validated connected outward shell.
Unknown/synthetic mixed members, unsupported analytic compound edits, inertia,
Boolean/shape/topology operations and exact mesh intersections remain explicit
refusals. In particular an AABB overlap is never substituted for common solid
volume. Keep legacy untracked compounds on their existing synthetic path.

Required evidence: independent 10 x 20 x 30 mm box mesh transforms, per-member
and additive volumes/centres/bounds, recursive transforms/tessellation, plus
explicit unsupported intersection/inertia/topology refusal. P13-011 verifies
both kernels on these physical fixtures and uses native OCCT for colliding
leaf pairs. The change supplies the Mock implementation needed by that task;
it grants no separate CAD maturity credit and exports no OCCT types.
