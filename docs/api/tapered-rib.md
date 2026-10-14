# Square-ended tapered rib source (MCAD-P16-006)

ADR-066 admits an original authored right-rectangle Distance/NewBody stock and
one complete straight Line in another authored Sketch, on an original extrusion
end. A distinct closed `tapered_rib` tag makes legacy constant-width rib readers
refuse the new solid semantics. Released v0.3.1 predates this tag.

```json
{"type":"tapered_rib","target_feature":"feature:extrude_base",
 "sketch_feature":"feature:sketch_rib","centerline_ref":"ent:rib_line",
 "face_ref":"ref:face:rib_base","base_thickness":{"value_si":0.004},
 "height":{"value_si":0.02},"angle":{"value_si":0.049958395721942765},
 "base_thickness_expr":"rib_thickness","height_expr":"rib_height",
 "angle_expr":"rib_angle_rad","ends":"square"}
```

Lengths are SI metres; angle is signed SI radians. Static stored fallback fields
must satisfy their ranges even with an expression. Resolve current expressions
strictly before deriving top thickness and physical admission. Length expressions
refuse angle units/parameters; angle expressions refuse length units/parameters.
Angle names follow existing `_rad`/`_deg`/`angle` convention. Values are finite:
base/height in (1nm,1000m], angle magnitude 1e-8..pi/4. Top thickness must remain
positive/resolvable, all world vertices stay within 1e6m, and added volume must
exceed arithmetic resolution relative to stock. No clamp or loose fallback.

## Source and owned end

Require original authored Sketch nodes, right rectangle/distance/NewBody stock,
complete straight line/point identities, solved finite coordinates and unit
orthogonal placements. The line may be construction geometry. Only an explicitly
referenced complete noncollapsed construction centreline may omit a material
profile; legacy/unconsumed empty-profile sketches still refuse.

The line plane must equal an actual original extrusion start/end, with outward
normal parallel/antiparallel to the original extrusion axis. Compare all original
placed end corners, span lengths and inward thickness with the actual body-owned
rectangular-stock witness. Line direction is parallel to a stock in-plane axis;
its complete base rectangle stays strictly inside the end with >1nm margins.
Imports, patterns, fusions, already-modified targets, curves, oblique/disconnected
or partial footprints refuse. Runtime box geometry cannot establish these
source facts; source admission independently does.

`face_ref` identifies exactly one `kind: face` reference created by the target,
with stable `role: extrusion_start` or `extrusion_end` matching the source end.
Global top/bottom naming is not used to guess an end after rigid rotations.
Optional fingerprint surface is `plane` or `brep_face`, with exact area range,
centroid bounds/plane and normal annotations checked against actual stock.
Picking normal annotations tolerate f32 error 1e-6; actual source/query axes use
1e-9. A binding to another present target face refuses. Absent old IDs can rebind
only through this owned typed end/source proof, never a nearest or mean face.
Missing/ambiguous/foreign/stale/otherwise-consumed ends refuse.

## Geometry, ownership and cache

Base is neutral: T(z)=T0-2*z*tan(angle), square ends, fixed centreline. Positive
angle narrows outward; negative widens, including actual overhang. Existing
physical Mock/OCCT runtime creates one connected stock/trapezoid union and
removes the shared internal interface. Independently integrate cross-sections
for union volume/area/first moments; verify a closed outward face-bearing mesh.

Physically classify all six surviving stock planes and five rib surfaces.
Remaining stock, including the annular entry plane and opposite cap, keeps
its target creator and stable named end roles; new surfaces belong to the rib
with `rib_start`, `rib_end`, `rib_top`, `rib_side_negative`/`positive` roles.
The entire entry reference is not consumed. No reconstruction ancestry is
fabricated. Cache hits repeat source/reference/volume/area/COM/closed-contact
checks and rediscover actual owners, rather than trusting stale cached evidence.

## Patches and verification

Create via `add_feature`, replace only with the same closed type. Typed
`set_feature_expr` fields `base_thickness`, `rib_height`, `draft_angle` write the
corresponding expressions; typed `face_ref` edits the mandatory end binding.
Parameter edits regenerate. Shared DesignPatch/transactions protect source and
history. Native candidate verification is mandatory for an existing or introduced
tag, including MCP `verify:false` and CLI `--no-verify`. Preview/review is read-only;
failed geometry/ref/unit/range edits preserve source/history/previous outputs.

Both schemas, no-inference migration, deterministic expanded/packed/authoring
round trips and genuine legacy-reader refusal ship. No optional taper/end fields
on legacy `rib`, implicit source planes, generic picker named-role support or
arbitrary curved/intersecting/multiple-rib family is claimed. Default Mock
refuses; SDK uses explicit analytic Mock, CLI/MCP native OCCT.

See [the patch-authored example](../../examples/tapered_rib_plate.md), source-aware
`tapered_rib_height` eval and actual stdio regression
`tools/test_tapered_rib_mcp.py`. Tests include signed angles, original ends and
oblique source placement, independent contact/integrals, protected cache/refusal,
strict static fallback units and canonical Undo/Redo, with zero model calls.
