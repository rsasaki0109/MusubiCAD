# Seated standard-hole admission sketch

MCAD-P16-009 / [ADR-063](../adr/ADR-063-counterbore-source-and-physical-mesh-admission.md)
defines the source and physical gate. MCAD-P16-003 implements the initial
`seated_standard_hole` / `counterbore` form on the P16-010 physical mesh/stock
prerequisite. Source readers support this tag after the implementation merge;
released v0.3.1 predates it. This is bounded original-stock machining, not a
claim of general face-based hole support.

Feature definition (all `value_si` lengths are metres):

```json
{
  "type": "seated_standard_hole",
  "sketch_feature": "feature:sketch_clearance",
  "profile_ref": "sketch:clearance/profile:outer",
  "target_feature": "feature:plate",
  "face_ref": "ref:face:plate_entry",
  "bore_depth": { "value_si": 0.010 },
  "bore_depth_expr": "plate_thickness",
  "standard": { "type": "iso273", "size": "M6", "fit": "normal" },
  "seat": {
    "type": "counterbore",
    "diameter": { "value_si": 0.011 },
    "depth": { "value_si": 0.004 },
    "diameter_expr": "seat_diameter",
    "depth_expr": "seat_depth"
  }
}
```

The owning sketch supplies the exact bore circle. ISO273 M6 normal derives a
6.6 mm diameter; mismatching source circles fail. The explicit larger seat is
independent of fastener designation. Seat diameter must exceed bore diameter;
seat depth must be strictly smaller than bore depth, which cannot exceed stock
thickness. ADR-063 defines finite/range/material-margin rules. No automatic
head dimensions, countersinks, thread helix or tolerance class are inferred.

The new struct and closed seat variants reject unknown fields. `face_ref` is
required and nonempty. The admitted target is the original right-rectangle,
distance/NewBody stock extrusion. Only its owned end faces are admitted; source
node, actual six-face stock geometry, uniquely resolved semantic face, outward
normal, entry plane and full seat-disk containment must agree. Side entry,
already-cut/imported/patterned/fused/nonrectangular targets and ambiguous/stale/
foreign/consumed references refuse. No nearest-face or current-body fallback.

Source uses the existing Length wire (`value_si`); expression strings remain
canonical source and their evaluated values are transient. Patch
authoring uses existing `add_feature` / `replace_feature_definition` operations,
with `set_feature_expr` support for `bore_depth`, `seat_diameter`, `seat_depth`
implemented by P16-003. They address `bore_depth_expr`, `seat.diameter_expr`
and `seat.depth_expr` respectively; legacy hole field names stay unchanged.
Normal unit-bearing parameter edits dirty/rebuild the feature. Shared backend
review/apply/Undo/Redo and revision guards protect files, history and artifacts.

The additive feature tag follows the existing schema-version policy; both
document/patch schemas and migration/admission/round-trip tests ship together.
Old readers reject the unknown feature. Future seat variants must be rejected
by counterbore-only readers. Legacy `hole`/`standard_hole` wire meanings stay
unchanged and migration does not infer new data.

P16-010's transient kernel query takes a body and `FacePick`, returning
actual selected rectangular-stock face ID, entry origin/unit axes/outward normal,
spans and inward thickness. It proves a unique complete rectangular solid;
the feature layer additionally proves source/semantic ownership and the end-face
rule. Numeric IDs are backend specific and never establish ownership alone.

Analytic meshes have closed caps/walls, actual triangle face IDs, deterministic
bounded chord/angular error and f32/degenerate refusal. A rendered unperforated
box is not sufficient. Independent triangle/tetrahedral checks verify actual
exports, volume/centroid/bounds/winding and all face coverage, alongside exact
analytic mass measurements in both kernels.

Default Mock selection remains unchanged. Complete bounded document tests
explicitly use analytic Mock throughout their history session; default/no-OCCT
callers lacking a real stock witness refuse this feature. Native CLI/MCP exercises actual document authoring through the P16-003 gateway. No new MCP tool, source kernel setting or host validation is introduced;
feature maturity credit requires the implementation main merge.

## Implemented execution and evidence

Regeneration contexts must expose read-only source nodes; default unsupported
contexts refuse. Require a distance/NewBody original right-rectangle extrusion,
actual cuboid witness and source corner/dimension agreement, an exact circle
matching the existing ISO table within 1e-9 m in diameter, and unique owned entry
reference. The bore and full seat disk must remain in stock; side/oblique entry,
cut/imported/fused targets, missing/foreign/consumed/ambiguous/stale references
and numeric/quality failures refuse. Source/sketch/kernel normal and plane
checks retain 1e-9 dimensionless / 1e-9 m tolerance. Optional semantic normal
annotations may originate in f32 picking and use 1e-6 dimensionless agreement;
actual geometric admission is not relaxed. Planar fingerprint bounds are checked
by their midpoint with a projected f32 coordinate-error allowance, including
rotated bounds whose min/max corners are not on the plane.

Strict length expressions reject angle literals and angle-parameter references;
parts containing seated holes use the existing strict parameter evaluator.
Lengths are finite in (1e-9, 1000] m, with seat/bore separation above 1e-9.
Bore depth cannot exceed stock height except arithmetic roundoff. Two inward
cuts remove the union once. Runtime volume/first-moment checks use the analytic
bore-plus-annulus formula (1e-8 relative volume, 1e-9 m centroid tolerance), and
refuse removal below measurable f64 resolution. Actual output requires nonzero
face IDs, nondegenerate closed oppositely paired triangles and positive volume.

New output face ownership is supplied by the feature's optional read-only
`discover_output_faces` query. Match actual triangle planes/outward normals to
the already admitted source-stock frame, with projected f32 rounding bounds;
retained outer faces belong to the stock producer, new walls/step/blind caps to
the seated-hole producer. Ambiguous ownership refuses. No nearest-face or AABB
shape inference is used. Discovery failures propagate for this feature, and
warm-cache reuse rechecks source/entry/kernel ownership and physical output.
This prevents the internal annular floor being confused with the outer entry.

CLI/Agent/MCP review/apply cannot bypass geometry for introduced or existing
seated holes using `verify=false` or `--no-verify`. Review is read-only; candidate
failures preserve files/history/artifacts. Opaque history follows the existing
canonical JSON-value/array-order contract; JSON object member order may change
through transport, while imported bytes remain exact. No default Mock factory
is switched; default/no-OCCT callers without a stock witness refuse.

See [the patch-authored example](../../examples/counterbored_plate.md),
`cargo test -p opencad-feature --test seated_standard_hole`,
`cargo test -p opencad-file --test seated_standard_hole`, and
`python tools/test_counterbore_mcp.py --musubicad target/debug/musubicad`.
`counterbore_seat_depth` has an independent MCP eval checker that checks actual
fresh native volume and preserved explicit source/standard/seat/sketch/reference
records. The stdio regression makes zero model calls; no paid-host result is claimed.
