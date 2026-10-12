# Closed cubic sketch spline

MCAD-P13-008 / [ADR-057](../adr/ADR-057-closed-cubic-sketch-spline-admission.md)
admits one closed, non-rational cubic Bezier span under ADR-012. It is a
clamped degree-three B-spline with implicit knots `[0,0,0,0,1,1,1,1]` and
unit weights. General NURBS editing remains deferred.

Author three ordinary `point` entities and a `cubic_bezier` entity through
`DesignPatch`. Control coordinates are SI metres or length expressions.

```json
{"type":"cubic_bezier","id":"ent:cubic","construction":false,
 "control_points":["ent:p0","ent:p1","ent:p2","ent:p0"]}
```

Closure is the same endpoint **ID**, not a coordinate proximity inference.
The other controls are distinct existing points. Ordered controls must form a
counterclockwise triangle: finite coordinates bounded to 1e6 m, separations
greater than 1 nm, cross product greater than 1e-18 m², shortest/longest edge
ratio and control-angle sine at least 1e-3. Unknown degree, knot, weight,
periodic or resolved-coordinate fields refuse. Control-point constraints work;
constraints targeting the curve itself refuse. Construction curves are excluded
from profiles. Adding/removing geometry cannot silently rebind a consumed alias.

The exact profile ID is `profile:bezier:ent:cubic`. Supported solid creation is
normal `NewBody` extrusion with distance or centred symmetric extent, on a
resolved proper orthonormal plane. Open/mixed spans, multiple spans, rational
curves, knot/weight edits, trim, offset, projection, loft, sweep, revolve,
through-all and cubic cut/join refuse. Sketch curve overlay/picking is deferred.

`SolvedSketch.cubic_bezier` contains three SI control coordinates; the final
control is implicitly the first. Its default is absent, preserving legacy
profile wire and cache keys. Solved controls are transient and never authored
as document truth. Parameter edits retain point expressions and append actual
resolved cubic inputs to the cache identity. Failed edits retain source/history;
failed regeneration retains previous outputs.

Native OCCT builds an exact private Bezier edge, face and prism, checks validity
and analytic volume, then returns opaque BREP bytes. Native cubic bounds use
`BRepBndLib::AddOptimal` without triangulation, with shape tolerances retained;
ordinary OCCT conservative control-hull bounds are too loose for this family.
Closed native Bezier edges remain unbound because endpoint-chord discovery
cannot assign honest curve roles/lengths; required semantic edge refs refuse.
Native tessellation is presentation. Mock computes analytic volume, centroid
and polynomial-extrema bounds; it refuses shape/topology/inertia/tessellation
operations it cannot reproduce.

For controls `(0,0)`, `(a,0)`, `(0,b)`, `(0,0)` and height `h`, volume is
`3*a*b*h/20`, XY bounds are `[0,4*a/9] × [0,4*b/9]`, and centre of mass is
`(3*a/14,3*b/14,h/2)`. Perimeter is capped deterministic GL8 quadrature,
not an analytic formula or certified error bound; surface area is twice cap
area plus perimeter times height. Non-convergence refuses the metric.

Example: `examples/cubic_plate.ocad.d` and
`examples/agent/create_cubic_plate_patch.json`. With `a=b=40 mm`, `h=6 mm`,
volume is 1440 mm³; editing `a` to `60 mm` gives 2160 mm³ while retaining
control IDs and source expressions. Use MCP `patch_dry_run`, `patch_apply` and
`regen_document`; history undo/redo is available through the Agent API/backend.
Eval `cubic_edit_control` checks source identities and independent analytic volume.
