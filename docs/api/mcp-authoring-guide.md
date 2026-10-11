# MusubiCAD structural patch authoring guide

This guide is served to MCP clients as the resource
`musubicad://guide/structural-patch`. The exact JSON shapes are in
`musubicad://schema/design-patch`.

## Workflow

1. `new_document` creates an empty part, assembly, or drawing, or you start
   from an existing document.
2. `inspect_document`, `query_document`, and `authoring_patch` show what
   exists. `authoring_patch` prints any document as the patch that rebuilds
   it; it is the best source of correct examples.
3. Write one `DesignPatch` and send it to `patch_dry_run`. Fix every reported
   problem until `verification.passed` is `true`; a failing
   `verification.error` names the feature that stopped regenerating or the
   expected effect that did not hold. Errors list every dependent or missing reference by stable ID.
   A `sketch_under_constrained` warning means that sketch can skew on a
   later parameter edit. Add horizontal/vertical and dimension constraints
   until the warning disappears.
4. Optionally, `review_patch` writes a before/after HTML review with images
   and mass and bounds changes.
5. `patch_apply` validates and verifies the patch again and writes the
   document. A failed or unverified patch leaves the document unchanged.

## Rules

- **Units.** Lengths are meters and angles are radians in stored values
  (`0.005`, `{"value_si": 0.005}`). Expressions carry units: `"5 mm"`,
  `"thickness * 2"`.
- **IDs.** You choose every ID. Use `<prefix>:<lowercase_words>`, for example
  `param:wall`, `sketch:base`, `ent:c0`, `con:width`, `feature:plate`,
  `component:bracket`, `instance:left`, `mate:ground`, `sheet:a4`,
  `view:front`, or `dim:width`. Semantic references use `ref:face:<name>` or
  `ref:edge:<name>`. An existing ID, or one removed earlier in the same patch,
  is rejected.
- **Order within a patch is free.** References are checked once, on the final
  state. Structural operations are applied before value edits
  (`set_parameter`, `set_feature_expr`).
- **Nothing is deleted implicitly.** Removing something that is still used
  fails and names every user. Remove the users in the same patch.
- **Features.** A feature's inputs (`sketch_feature`, `source_feature`,
  `target_feature`) must come earlier in the display order. Use `position`
  `{"at": "end"}`, `{"after": "feature:x"}`, `{"before": "feature:x"}`, or
  `{"at": "start"}`. Dependency edges are derived automatically.
- **Profiles.** A closed loop of lines, or a circle, forms a profile. The
  first closed profile of a sketch is `"<sketch id>/profile:outer"`.
  Extrudes, holes, and revolves must reference a closed profile.

## Building a part from nothing

A complete mounting plate with a through hole, applied to an empty part
document (`examples/agent/author_plate_from_empty_patch.json`), uses:

1. **Parameters.**
   `{"type": "add_parameter", "id": "param:width", "name": "width", "expr": "60 mm"}`
2. **A sketch.**
   `{"type": "add_sketch", "id": "sketch:base", "name": "Base Sketch", "workplane": {"type": "global", "plane": "XY"}}`
3. **Sketch entities.** Points first, then lines, circles, or arcs that name
   them:
   - `{"type": "add_sketch_entity", "sketch_id": "sketch:base", "entity": {"type": "point", "id": "ent:c0", "x": 0.0, "y": 0.0}}`
   - `{"type": "add_sketch_entity", "sketch_id": "sketch:base", "entity": {"type": "line", "id": "ent:e0", "start": "ent:c0", "end": "ent:c1"}}`
   - `{"type": "add_sketch_entity", "sketch_id": "sketch:hole", "entity": {"type": "circle", "id": "ent:hole_circle", "center": "ent:hole_center", "radius": 0.005}}`
   - Arcs join line loops when they name the points at their ends; the arc
     runs counterclockwise from `start_angle` to `end_angle` (ADR-021):
     `{"type": "arc", "id": "ent:right", "center": "ent:c_right", "radius": 0.004, "start_angle": "-90 deg", "end_angle": "90 deg", "start_point": "ent:s1", "end_point": "ent:s2"}`.
     `examples/agent/author_slot_plate_patch.json` authors a slot this way.
   - Arc angles may name parameters (ADR-024), so a shape can turn with a
     design angle: `"start_angle": "slot_angle - 90 deg"`. Add `tangent`
     constraints between the straight sides and the arcs to record the
     intent: `{"type": "tangent", "id": "con:bottom_tangent", "line": "ent:bottom", "curve": "ent:right"}`.
     `examples/agent/author_slanted_slot_patch.json` authors a slot at a
     `slot_angle` this way.
4. **Driving constraints.**
   - `{"type": "add_sketch_constraint", "sketch_id": "sketch:base", "constraint": {"type": "distance", "id": "con:width", "target": {"line": "ent:e0"}, "expr": "width"}}`
   - `{"type": "radius", "id": "con:hole_radius", "target": "ent:hole_circle", "expr": "hole_diameter / 2"}`
   - **Shape constraints.** Constrain every rectangle edge horizontal or
     vertical:
     `{"type": "horizontal", "id": "con:e0_horizontal", "line": "ent:e0"}`
     and `{"type": "vertical", "id": "con:e1_vertical", "line": "ent:e1"}`.
     The solver anchors a sketch's first point, so four edges, two
     lengths, and four horizontal/vertical constraints leave no freedom.
     With lengths alone the rectangle regenerates correctly while its
     starting coordinates match the parameters. Once a length parameter
     changes, it skews: a 60 mm plate edited to 90 mm came out 88.9 mm
     wide.
5. **A sketch feature per sketch.**
   `{"type": "add_feature", "position": {"at": "end"}, "node": {"id": "feature:sketch_base", "name": "Base Sketch", "definition": {"type": "sketch", "sketch_id": "sketch:base"}}}`
6. **Solid features.**
   - Extrude: `{"type": "extrude", "sketch_feature": "feature:sketch_base", "profile_ref": "sketch:base/profile:outer", "extent": {"type": "distance", "length": {"value_si": 0.005}}, "operation": "new_body", "length_expr": "thickness"}`
   - Hole: `{"type": "hole", "sketch_feature": "feature:sketch_hole", "profile_ref": "sketch:hole/profile:outer", "depth": {"type": "distance", "length": {"value_si": 0.005}}, "target_feature": "feature:plate", "depth_expr": "thickness"}`

Other feature types are `revolve`, `fillet`, `chamfer`, `linear_pattern`,
`circular_pattern`, `mirror_pattern`, `imported_solid`, `shell`, `loft`,
`sweep`, and `helix_sweep`. A helix sweep moves a closed section along a
helix through its centre, around `axis_origin_m`/`axis_direction_m`, with
`pitch_expr` and `height_expr` (see
`examples/agent/author_coil_spring_patch.json`). A sweep moves a closed
section along a separate path sketch of lines and endpoint arcs. Place the section at one end of the path, perpendicular to
it (see `examples/agent/author_sweep_elbow_patch.json`).
A loft skins two or more closed sections, each on its own sketch workplane,
for example a raised one:
`{"type": "custom", "origin": [0, 0, 0.03], "normal": [0, 0, 1], "x_axis": [1, 0, 0]}`
(see `examples/agent/author_loft_frustum_patch.json`). A shell
needs an `add_semantic_ref` for each face it opens, for example
`{"type": "add_semantic_ref", "topo_ref": {"ref_id": "ref:face:box_top", "kind": "face", "semantic": {"created_by": "feature:box", "role": "top"}}}`,
then
`{"type": "shell", "target_feature": "feature:box", "thickness": {"value_si": 0.002}, "open_face_refs": ["ref:face:box_top"]}`.
To place a
purchased part from a STEP file, use the `import_step` tool rather than
writing its base64 attachment by hand. Call `authoring_patch` on the
examples to see each one. Extrude and revolve `operation` is `new_body`,
`cut`, or `join`; pattern `operation` is `union` or `cut`.

## Editing

- **Change a value:**
  `{"type": "set_parameter", "id": "param:width", "expr": "80 mm"}`
- **Replace a definition** (same feature type):
  `{"type": "replace_feature_definition", "id": "feature:plate", "definition": {...}}`
- **Suppress:**
  `{"type": "set_feature_suppressed", "id": "feature:through_hole", "suppressed": true}`
- **Remove:** `remove_parameter`, `remove_sketch`, `remove_sketch_entity`,
  `remove_sketch_constraint`, `remove_feature`, `remove_semantic_ref`, and
  `remove_assertion` each take the object's ID.
- **Guard against concurrent edits:** add a precondition
  `{"type": "revision_equals", "algorithm": "sha256", "version": "musubicad.design-state.v2", "digest": "<hex>"}`.

## Assemblies and drawings

- **Assemblies:** `add_component`, `add_instance`, `add_mate`,
  `add_connector`, `add_assembly_pattern`, and their `remove_*` forms
  (`remove_connector` included).
- **Robot joints:** `add_joint`, `set_joint`, `remove_joint`. A joint names a
  mate and how the child moves about it, with SI limits:
  `{"type": "add_joint", "joint": {"id": "joint:elbow", "mate": "mate:elbow", "type": "revolute", "lower_rad": -1.745, "upper_rad": 2.443, "effort_n_m": 1.0, "velocity_rad_s": 3.0}}`.
  Moving joints need a `concentric` mate; zero is the current pose. URDF
  export writes these limits. After adding or changing a robot joint, export
  `.urdf` and run `reach` on the same assembly so the kinematic tree, joint
  limits, and target residual agree with the Design Graph.
- **Drawings:** `add_sheet` (added empty), `add_drawing_view`,
  `add_drawing_dimension`, and their `remove_*` forms.

Object shapes match `graph/assemblies.json` and `graph/drawings.json`; run
`authoring_patch` on `examples/robot_arm_assembly.ocad.d` or
`examples/bracket_front_view.ocad.d` for complete examples.

### Semantic linear drawing dimensions

Prefer a semantic binding when a dimension must follow model edits:

```json
{
  "type": "add_drawing_dimension",
  "sheet_id": "sheet:a4",
  "dimension": {
    "id": "dim:width",
    "view_id": "view:front",
    "edge_ref": "ref:edge:bracket_top_front",
    "offset_m": -0.01
  }
}
```

The reference must exist in the part named by the view, rather than in the
drawing. Inspect the part's semantic references first; never invent a kernel
index or use a projected mesh segment as identity. The dimension measures the
distance between current end vertices (model meters), not curved arc length.
Do not include `start_model_m`/`end_model_m` with `edge_ref`.

`patch_dry_run` verifies all sheets and names the dimension/ref when a binding
is missing, ambiguous or consumed. Repair the binding explicitly; do not replace
a failed semantic measurement with fixed coordinates. `regen_document` returns
`drawing_dimensions` with resolved length and provenance. Inspect the SVG after
verified apply. Assembly instance bindings and radius/diameter dimensions are
separate future capabilities.

[`semantic_bracket_dimension_patch.json`](../../examples/agent/semantic_bracket_dimension_patch.json)
converts the current `musubicad new <path> drawing` template using a new dimension
ID (removed IDs cannot be reused in the same patch).
[`semantic_bracket_drawing.ocad.d`](../../examples/semantic_bracket_drawing.ocad.d)
is the complete example; its width follows changes to the nested bracket.

### Six-axis arm clearance example

`examples/six_axis_arm.ocad.d` includes a required motion-clearance assertion
at 13 samples for each of its six joints. The shoulder range is −60° to +80°,
the elbow −130° to +30°, and wrist pitch −110° to +70° (serialized in radians).
After changing joint limits, verify the patch and regenerate exported evidence
from the same Design Graph. `bash docs/assets/generate-six-axis.sh` refreshes
the example's URDF/STL, GIF, HTML viewer and screenshot. ADR-033 samples one
joint at a time with others at rest; combined poses and continuous motion
require additional checks.

### Parametric datum planes and axes

Add `datum_plane`/`datum_axis` through the existing feature patch operations;
place construction features before consumers. A datum plane uses `base`,
signed `offset.value_si` metres and optional `offset_expr`; a sketch binds it
with `{"type":"datum_plane","feature_id":"feature:plane"}`. A datum axis
uses `origin_m`, a nonzero `direction` and optional three-entry `origin_expr`
(length expressions or null); a revolve consumes it via `axis_feature`.
Verify every edit: missing/suppressed/wrong-kind datums fail closed, and changing
an upstream datum invalidates dependent cached geometry. See [datums.md](datums.md),
[ADR-035](../adr/ADR-035-parametric-datums.md), and
`examples/agent/create_datum_plate_patch.json`. Revolve profiles currently require
global workplanes; datum-plane sketches are supported by extrusions.

### Neutral-plane draft (MCAD-P11-002)

Use `type: "draft"` with a signed SI `angle` (or `angle_expr`, e.g. `5 deg`),
`target_feature`, `neutral_plane` and unique `face_refs`. Positive angles narrow
in the neutral normal direction; negative angles widen. Create semantic face
refs before the draft, and use a datum workplane for a parametric neutral
plane. The OCCT path accepts only convex planar solids and initially
perpendicular side faces; holes, cavities, curved faces and collapsed results
fail closed. Never substitute a different face when resolution fails.
See [draft API](draft.md), ADR-036 and
`examples/agent/create_draft_plate_patch.json`; dry-run/apply through the
backend, regenerate, then check volume and dimensions.

### Centerline ribs (MCAD-P11-003)

Use `type: "rib"` when a reinforcing web can be defined by one open sketch
line. Add the sketch first, usually on the face or datum plane where the rib
starts. The rib feature consumes `sketch_feature`, `centerline_ref`,
`target_feature`, positive `thickness` in metres, optional `thickness_expr`,
positive distance `extent`, and optional `length_expr` for the rib height. The
feature expands the centerline into a rectangular closed profile in the sketch
plane, extrudes it along the workplane normal, and unions it into the target
body.

Keep the centreline explicit and unit-bearing; do not pre-bake the rib as a
plain rectangle extrude when the user asked for a rib. Missing centreline
entities, zero thickness, zero height and missing target bodies fail closed.
The current contract supports straight-line ribs with distance extent.

## Standard holes and simplified internal threads (MCAD-P11-004)

Use `type: standard_hole` with the same `sketch_feature`, circular `profile_ref`,
`target_feature`, `depth`, optional `depth_expr` and `face_ref` as a hole, plus a
required `standard`:

```json
{"type":"iso273","size":"M6","fit":"normal"}
```

This requires a 6.6 mm sketch diameter. For a simplified internal coarse thread:

```json
{"type":"iso_metric_coarse","size":"M6","representation":"tap_drill"}
```

This requires a 5 mm sketch diameter (6 mm nominal, 1 mm pitch). Supported sizes
are M3/M4/M5/M6/M8/M10/M12; clearance fits are `close`, `normal`, `loose`.
`StandardHoleDimensions` derives unit-explicit nominal, pitch and cut diameters.
Tap-drill sizes use nominal minus pitch, not commercial drill rounding or thread
tolerance limits. `tap_drill` exports a cylinder only, with no physical thread
helix, drill point or tolerance class. The simplified thread spans the hole depth.

Keep the circle diameter and designation/fit consistent in one DesignPatch.
Regeneration and verified dry-run/apply reject diameter discrepancies above
1e-9 m and non-circular profiles. Unsupported standards/sizes/representations
and extra standard fields fail closed. Structural staged authoring alone does
not verify diameter agreement. Use verified application for completed designs.
See `examples/agent/create_standard_hole_plate_patch.json` and
[ADR-037](../adr/ADR-037-standard-holes-and-simplified-threads.md).
