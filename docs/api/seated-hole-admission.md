# Seated standard-hole admission sketch

MCAD-P16-009 / [ADR-063](../adr/ADR-063-counterbore-source-and-physical-mesh-admission.md)
is a design gate. **This source tag is not implemented or accepted by current
readers.** P16-010 supplies physical meshes/stock-face witnesses first; P16-003
then implements the document feature and its schema, patches, examples and eval.
Do not paste this design sketch into a current document as a working feature.

Proposed feature definition (all `value_si` lengths are metres):

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
canonical source and their evaluated values are transient. Proposed patch
authoring uses existing `add_feature` / `replace_feature_definition` operations,
with `set_feature_expr` support for `bore_depth`, `seat_diameter`, `seat_depth`
added by P16-003. These new fields are not valid operations on current readers.
Normal unit-bearing parameter edits dirty/rebuild the feature. Shared backend
review/apply/Undo/Redo and revision guards protect files, history and artifacts.

The additive feature tag follows the existing schema-version policy; both
document/patch schemas and migration/admission/round-trip tests ship together.
Old readers reject the unknown feature. Future seat variants must be rejected
by counterbore-only readers. Legacy `hole`/`standard_hole` wire meanings stay
unchanged and migration does not infer new data.

P16-010's proposed transient kernel query takes a body and `FacePick`, returning
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
callers lacking a real stock witness refuse this feature. Native CLI/MCP will
exercise actual document authoring when P16-003 is implemented. No new MCP tool,
source kernel setting, host validation or maturity increment is introduced by
this design task.
