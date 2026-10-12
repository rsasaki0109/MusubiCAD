# ADR-053: Analytic ellipse sketches and bounded exact extrusion

- Status: Accepted
- Date: 2026-10-12
- Task: MCAD-P13-004

## Context

An ellipse is an analytic curve, not a sampled polygon or spline edit. cadrum
0.8.5 exposes no ellipse Edge constructor and imports opaque BREP solids only.
Its private FFI types must not escape or be patched in registry sources.

## Decision

Add an authored `ellipse` entity with stable ID, centre point ID, positive
`major_radius`/`minor_radius` (metres for literals) and `axis_angle` (radians for
literals). Major >= minor, including the explicit circular special case.
Lengths/angle may be unit-bearing parameter expressions. Parts with ellipses
use strict literal-unit evaluation (length: m/mm/cm/in, angle: rad/deg and
spelled variants). Named angle parameters follow the existing `_rad`/`_deg` or
`angle` naming contract; do not name a degree parameter as an ordinary length.
Initial expressions admit same-kind length/angle parameter references and
literal factors; cross-kind parameter formulas refuse rather than guess a
dimensional interpretation. Legacy parts without ellipses retain their previous evaluator. Resolved values are
separate transient solver input, never document truth. Feature cache keys include
resolved ellipse dimensions/angle separately from source serialization; warm
cache and angle-only edits must regenerate downstream geometry. The centre participates
in ordinary point constraints; axis lengths and orientation are defined by
source parameters, not new free solver DOFs. Radius/diameter/equal-radius
constraints refuse ellipse targets. Construction ellipses are excluded from
profiles. Full closed ellipses only; no elliptical arcs or mixed ellipse loops.

Carry a neutral SolvedEllipse to GeometryKernel. Native kernel stores an opaque
wire recipe and privately constructs gp_Elips -> edge -> closed wire -> face ->
prism, returning BREP solid bytes to the existing cadrum reader. Distance and centred symmetric extents are admitted; through-all refuses
until a bounded target extent contract exists. New-body and existing native
cut/join feature paths consume this exact solid. Elliptical loft/sweep,
helix/revolve and trim/offset remain unsupported and refuse without polygon
fallback. Ordinary native BREP tessellation provides rendering.

Mock stores analytic ellipse prism metadata for normal new-body extrusions,
with exact volume, bounds/COM and rigid placement. Perimeter/surface area uses
the complete elliptic integral via a bounded deterministic AGM recurrence
(maximum 32 steps), checked against circles and native mass properties. No new
geometric sampling is introduced. Unsupported Mock shape edits/booleans and
tessellation refuse explicitly, rather than inventing an accurate BREP.

Add/remove manufacturing entities must preserve the entity-set identity of an
unchanged profile consumer. New closed ellipses must not silently steal a
profile:outer alias; explicit consumer rebind/removal is required.

The persisted entity tag is additive; update sketch and patch schemas,
migration policy and exact old/new source round trips together. All writes use
existing AddSketchEntity/ReplaceSketchEntity/DesignPatch and backend history.
No OCCT public types or new dependencies are introduced. ADR-012 spline/NURBS
gates are unaffected.

## Validation and limits

Named tests: 30/20 mm ellipse extruded 6 mm has pi*.03*.02*.006 m3 volume;
rotated bounds use hypot(a*cos(phi), b*sin(phi)) and the corresponding y term.
Mock area is checked against the elliptic-integral oracle to 1e-12 m2; native
surface integration comparison uses 5e-9 m2 for the named probes (OCCT default
quadrature differs by about 1.8e-9 m2). Volume/bounds use independent oracles.
Verify both kernels, circles, plane placement, axis parameter edits, invalid
axes/angles/missing expressions, construction and alias refusal, unsupported
operations, source/patch/schema/expanded/packed/history round trips. Add an
example, guide/skill and independent MCP eval. Native prototype in /tmp proved
volume at 0 and 30 degrees; it is not production integration evidence.

This initial ellipse tool does not provide ellipse tangency/axis constraints,
elliptical arcs, or arbitrary ellipse-based solid operations. Document these
limits clearly and refuse unsupported combinations.
