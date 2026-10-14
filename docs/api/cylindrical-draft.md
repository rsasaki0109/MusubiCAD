# Cylindrical neutral-plane draft (MCAD-P16-005)

ADR-065 admits one full curved-face family: an original exact-circle
Distance/NewBody extrusion becomes an actual solid frustum. `cylindrical_draft`
is a distinct closed feature tag. Legacy planar `draft` retains its behavior;
readers predating this tag refuse it. Released v0.3.1 predates the tag.

```json
{"type":"cylindrical_draft","target_feature":"feature:stock",
 "face_ref":"ref:face:stock_side","angle":{"value_si":0.12217304763960307},
 "angle_expr":"draft_angle_rad",
 "neutral_plane":{"type":"custom","origin":[0,0,0.015],
                  "normal":[0,0,1],"x_axis":[1,0,0]}}
```

Angles carry SI radians; positions/radii/heights carry metres. An optional
expression uses strict angle units and angle parameter names (`_rad`, `_deg` or
`angle`), such as `draft_angle_rad = 7 deg`. Length units/references refuse.
Magnitude is 1e-8 through pi/4 radians; endpoint radii must differ by more than
1 nm. Both radii and height must be finite in (1 nm,1000 m], with the established
1e6 m world limit. Custom workplanes require finite unit orthogonal axes.
Global, custom, admitted face-reference and datum workplanes resolve through the
existing backend; graph dependencies include target and neutral datum/reference.

The neutral plane normal must be parallel or antiparallel to the actual source
axis. Its axial coordinate z0 lies within [0,H]; only arithmetic endpoint
roundoff is accepted. Positive angle narrows toward the neutral normal.
With k = sign(dot(normal,axis))*tan(angle), r(z)=R-k*(z-z0). The neutral circle
stays fixed; an endpoint neutral cap remains physically unchanged. Radius
collapse, unresolvable taper, oblique/outside planes and nonfinite data refuse.

## Source and semantic ownership

Require an original authored circle sketch and distance/NewBody target. Compare
its placed circle centre, radius, axis and height with the actual kernel-owned
full-cylinder witness. Imports, booleans, patterns, previously tapered stock,
mixed profiles and approximated circles refuse. Default Mock is unsupported;
SDK physical tests explicitly use `MockGeometryKernel::with_analytic_primitives()`.
CLI/MCP use native OCCT without switching kernels.

`face_ref` identifies exactly one `kind: face` reference, created by the target,
with `role: cylindrical`. If a fingerprint is supplied, `surface_type` must be
`cylinder`; the actual witness proves the unique full side. A cap ID present on
the target refuses. An absent old ID can rebind only through this owned typed
witness, never nearest-face search. Optional normal annotations are finite unit
radial directions (f32 picking tolerance 1e-6), not an average curved-face normal.
Optional area ranges must contain the exact lateral area. Legacy bbox hints
retain their centroid-bound meaning; their midpoint must lie within the actual
cylinder's occupied box. No annotation establishes geometric admission.

Missing/duplicate/foreign references and sides consumed by another active
feature refuse. This feature's own input consumption remains valid on subsequent
regeneration and cache hits. Provenance reports the original changed side as
`consumed`; consumers requiring a surviving side must fail closed. The output
has three actual face IDs. Its conical side and changed caps belong to the draft;
only an endpoint cap with proved unchanged plane/radius keeps source ownership.
No reconstruction face ancestry is fabricated. Cached output repeats source,
ref, volume/area/first-moment/closed-contact checks and rediscovers actual owners.

## Patches, verification and evidence

Create via `add_feature`; replace only with the same source type. Typed
`set_feature_expr` field `draft_angle` changes `angle_expr`, and `set_feature_ref`
field `face_ref` changes the full-side binding. Parameter edits also regenerate.
References and angle kinds/ranges are validated before candidate publication.
Use reviewed DesignPatch/transactions. Native geometry verification is mandatory
for an existing or introduced tag, including MCP `verify:false` and CLI
`--no-verify`. Preview/review is read-only; refused candidates preserve files,
history and prior outputs. Packed/expanded round trips and canonical Undo/Redo
retain authored source and feature order.

[The runnable example](../../examples/cylindrical_draft.md) and creation patch
show the complete workflow. `cylindrical_draft_angle` independently checks fresh
volume and unchanged source plus signed -5 degree expression: centred-plane
volume alone cannot distinguish positive from negative angles.
`tools/test_cylindrical_draft_mcp.py` exercises actual MCP and Agent history with
zero model calls. Mock/native tests independently integrate radial slices and
closed triangles, check contact/COM, signed and reversed planes, oblique source
placement, physical cap ownership, cache corruption and protected refusals.
General curved/concave-face or NURBS draft is outside this admitted family.
