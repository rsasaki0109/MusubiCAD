# Portable physical rectangular revolves (ADR-112)

`RectangularRevolveGeometry::from_input(&RevolveInput)` is a read-only,
kernel-neutral admission/evidence API. Its output is derived SI geometry, not
`.ocad` source or a substitute for the profile. `GeometryKernel::revolve` in
`MockGeometryKernel::with_analytic_primitives()` now creates a genuine analytic
annular-sector body with actual closed tessellation, mass, surface area,
centroid and absolute bounds. Legacy synthetic Mock is not physical evidence.
Native OCCT retains exact line/circular sweep geometry. For portable profiles,
its spine begins on the actual radial side: this corrects the former
negative-side half-turn choosing the opposite angular branch despite equal
volume. Actual oriented triangle display normals also survive rigid placement.

The common subset is a closed right rectangle with four ordered straight edges
on global XY/YZ/XZ, with no curve channels. An optional solved placement must
match the declared global frame. The finite normalized axis is coplanar and
parallel to one rectangle edge; moving the axis origin along itself has no
physical effect. Either axis direction, radial side and source winding is
supported. Require positive inner radius, radial width and height above 1 nm,
at most 1000 m; positions remain within 1e6 m. Crossing/touching axes, skew or
noncoplanar axes, wrong placement, curves and tiny ambiguous intervals refuse.
Only NewBody without a target is portable; this does not narrow broader native
OCCT admission or add custom/face/datum sketch-plane editing.

Angles are positive, finite and at most 2*pi rad. A partial turn within 1e-6 rad
of a full turn refuses the common admission because existing native sweep
classifies it as a circle; arithmetic-roundoff full turns remain full turns.
Full turns have four physical faces; partial turns have six. No seam wall is
added to a full turn. Exact analytic measurements and angular coordinate extrema
are independent of tessellation. Display reuses the existing deflection rule,
8192 angular-sample budget and oriented manifold builder; unrepresentable f32
coordinates, degenerate output or budget overflow refuse.

The source format, feature schema and kernel trait are unchanged. Use ordinary
`RevolveFeature` with explicit `angle_expr` and optional `axis_feature`, authored
only through DesignPatch. `axis_feature` preserves a live datum dependency;
inline cached coordinates cannot replace it. The runnable example is
`examples/physical_rectangular_revolve.ocad.d`; its complete structural recipe
is `examples/agent/physical_rectangular_revolve_patch.json`.

The recipe has a global XY rectangle x=15..25 mm, y=-7..13 mm, and associated
axis x=`axis_u`=5 mm along +Y. `turn_angle`=90 deg gives radii 10/20 mm,
V=1500*pi mm3, bounds (5,-7,-20)..(25,13,0) mm and centroid
(5+280/(9*pi),3,-280/(9*pi)) mm. Setting axis_u=10 mm and turn_angle=180 deg
in one verified patch gives radii 5/15 mm, V=2000*pi mm3, bounds
(-5,-7,-15)..(25,13,0) mm, centroid (10,3,-65/(3*pi)) mm. Authored source
expressions remain unchanged when regeneration resolves them.

Actual MCP/Agent coverage uses six placements/branches, independent fresh native
SI STL, angular/radial/end contacts, exact source, byte-preserving review and
refusal, and canonical history. Eval `edit_physical_rectangular_revolve` checks
source and fresh physical export, rejecting equal-volume wrong placement and
inline axis/angle substitutions. No external model turns or new GUI controls
are claimed. The native creation form remains P17-013 after this prerequisite.
