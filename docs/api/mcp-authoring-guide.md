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

## Variable and semantic face fillets (MCAD-P11-005)

A `fillet` may carry `radius_law` in addition to its existing `radius` and
`radius_expr`. The start radius is `radius`; `end_radius` is the final radius:

```json
{
  "type":"fillet", "target_feature":"feature:plate",
  "edge_ref":"ref:edge:plate_front",
  "radius":{"value_si":0.0005}, "radius_expr":"start_radius",
  "radius_law":{"type":"linear","end_radius":{"value_si":0.0015},
                "end_radius_expr":"end_radius"}
}
```

Endpoint order is lexicographic in world coordinates (x, then y, then z). For a
front edge parallel to X, the start is its lower-X end. Both radii must be finite
and at least 1e-9 m. Edit named parameters through DesignPatch; changing the
end expression also works through `replace_feature_definition`.

Variable fillets require a semantic `edge_ref` resolving to one straight edge.
Curved edges, tangent chains, ambiguous/missing references and invalid kernel
results fail closed. `face_ref` and `edge_ref` cannot be supplied together. For
a constant-radius face perimeter, supply only `face_ref` and omit `radius_law`.
For example, a `ref:face:plate_top` referencing the plate's `top` face rounds its
perimeter with the existing constant-radius operation.

The explicit linear law is approximated with 65 fixed, uniformly spaced radius
points and OCCT interpolation. Endpoint easing is confined to the end intervals;
it is not an arbitrary editable spline. The tested prism volume agrees with the
circular-corner reference to 1e-3 of removed volume; arbitrary-body accuracy is
kernel-dependent. BREP transfer does not preserve exact pointer-based face
history, so subsequent semantic references must resolve from verified geometry.
Use verified dry-run/apply and review its provenance after changes. See
`examples/agent/create_variable_fillet_plate_patch.json` and
[ADR-038](../adr/ADR-038-linear-variable-fillets.md).

### Operations between existing bodies (MCAD-P11-006)

Create a `body_boolean` feature through `add_feature`:

```json
{"type":"body_boolean","source_feature":"feature:tool",
 "target_feature":"feature:plate","operation":"subtract"}
```

`union`, `subtract` and `intersect` are supported. Subtraction is target minus
source; union/intersection use the same explicit operand order for provenance.
Both inputs must be distinct solid-producing feature IDs in this document.
Their dependency edges are derived automatically. Sketches, datums, missing
inputs, self-inputs and cycles are refused; suppressed inputs fail regeneration.
The result is a new body output. Input feature outputs remain available in the
Design Graph; editing either upstream branch invalidates the downstream Boolean.

OCCT refuses results with non-finite or non-positive volume, including empty
intersection or complete subtraction. Ordinary low-level `boolean` retains its
existing empty-result semantics for interference queries. Mock implements the
operation contract with deterministic IDs, without geometric emptiness evidence.
Use verified dry-run/apply: a failed operation must not be persisted. Dimensions
and mass use metres, m³ and explicit density as usual. Exact face history covers
the target operand; source-derived references still require verified geometric
matching. Disconnected nonempty results follow the existing kernel compound
representation. See `examples/agent/create_body_boolean_patch.json` and
`examples/body_boolean.ocad.d` (80 × 60 × 6 mm plate, 30 × 20 × 8 mm tool at
(20,10,0) mm; union 30000 mm³, subtraction 25200 mm³, intersection 3600 mm³).

### Angular drawing dimensions (MCAD-P12-001)

Create an annotation through DesignPatch, on an existing view of its own sheet:

```json
{"type":"add_angular_dimension","sheet_id":"sheet:a4","dimension":{
 "id":"dim:plate_corner","view_id":"view:front",
 "edge_refs":["ref:edge:plate_front","ref:edge:plate_left"],
 "ray_ends":["end","end"],"arc_radius_m":0.01}}
```

The two references belong to the view's referenced part document. Each must
resolve uniquely to an exact native straight edge. Current semantic discovery
covers the top-Z perimeter; other edges, curves, missing/ambiguous bindings and
assembly-instance bindings are refused. Canonically sort each edge's endpoints
by world x/y/z; `start` selects the lower endpoint, `end` the higher. Selected
endpoints must lie away from the intersection of the supporting lines. Parallel,
skew and degenerate rays fail; intersections of extended lines are allowed.
The measured minor angle is derived in radians and shown in SVG as degrees.
Only Front/Top/Right orthographic views parallel to the angle plane are accepted;
isometric and foreshortened layouts fail. `arc_radius_m` is a positive finite
length on the sheet, independent of view scale.

For explicit non-associative construction rays, replace `edge_refs`/`ray_ends`
with `vertex_model_m`, `first_model_m`, `second_model_m` (three model-space
metre vectors). Never mix the two forms or store a label override. Semantic
annotations persist references, with resolved point rays kept transient.
`regen_document` returns `drawing_angular_dimensions` with `measured_angle_rad`
and both reference provenances. Text-mode regeneration prints the same SI angle.

Angular annotations live in `sheet.angular_dimensions`; old linear annotations
keep their existing wire format and commands. IDs share the `dim:` namespace.
Use `remove_drawing_dimension` for either kind; a view cannot be removed while
an annotation uses it. To replace an annotation, remove it and create a new
stable ID. Always use verified dry-run/apply and export SVG to check placement.
See `examples/agent/create_angular_drawing_patch.json`, the companion part patch,
`examples/angular_plate_drawing.ocad.d` and ADR-040. The 80 × 60 × 6 mm plate
has a derived corner angle of 90°, with a 10 mm sheet arc at view scale 0.5.

### Semantic radius and diameter dimensions (MCAD-P12-002)

Use `add_radial_dimension` through DesignPatch on a drawing document:

```json
{"type":"add_radial_dimension","sheet_id":"sheet:a4","dimension":{
  "id":"dim:radius","view_id":"view:front","edge_ref":"ref:edge:circle",
  "kind":"radius","leader_direction":[1,1],"leader_extension_m":0.01
}}
```

`kind` is `radius` or `diameter`. Direction is in sheet coordinates and normalized
at layout; extension is positive sheet metres independent of view scale. Choose a
view parallel to the circle plane (`front` projects XY in the current convention).
The exact complete circle must belong to the referenced part's current active
body and resolve uniquely from semantic refs; current discovery covers top-Z
perimeter edges. Inspect discovery rather than guessing roles or edge indices.
Missing/ambiguous refs, arcs, ellipses, noncircular curves, assembly bindings and
foreshortened/isometric views fail closed. Never supply radius/center/label caches.

Verified dry-run/apply regenerates before persisting. Results expose
`drawing_radial_dimensions` with `measured_length_m`, `radius_m`, `kind` and
reference provenance, in SI units. SVG labels show model millimetres (`R 10.00 mm`,
`⌀ 20.00 mm`). Edit by removing a dimension and adding a fresh stable ID; generic
`remove_drawing_dimension` works across all dimension kinds. IDs are globally
unique, and removing a still-dimensioned view fails atomically.

See `examples/agent/create_radial_cylinder_patch.json`,
`examples/agent/create_radial_drawing_patch.json` and the self-contained
`examples/radial_plate_drawing.ocad.d`. Eval `drawing_semantic_radial_layout`
requires fresh regenerated measurements alongside semantic bindings.

### Exact polygonal section views (MCAD-P12-003)

Add a view through `add_drawing_view` with an optional section:

```json
{"type":"add_drawing_view","sheet_id":"sheet:a4","view":{
 "id":"view:section","name":"Section A-A",
 "model":{"source_path":"parts/plate.ocad.d","source_doc":"doc:plate"},
 "projection":"front","scale":0.5,"origin_on_sheet_m":[0.05,0.05],
 "section":{"plane":{"origin_m":[0,0,0.003],"normal":[0,0,1]},"hatch_spacing_m":0.003}
}}
```

The plane is explicit in model metres with a unit normal, and must strictly cross
the current part. The view must face the plane (`front` = XY, `top` = XZ, `right`
= YZ). Hatch perpendicular spacing is positive sheet metres (>1e-6 m), independent
of view scale. Exact line contours only: curves, outside/coplanar-face cuts,
open/branched/touching boundaries, assemblies and foreshortened/isometric views
fail closed. The 4096-edge limit bounds validation work.

Dry-run/apply regenerates before persistence. Results expose `drawing_sections`
with `area_m2`, `loop_count`, `edge_count`, sheet/view IDs; CLI text provides the
same fresh evidence. Export SVG for slice outlines and even-odd material hatching,
with holes left clear. Never author area/contour caches. Edit by remove/add view
with a fresh stable ID. Section dimensions and geometry projected beyond the cut
are not supported yet; annotations bound to original part edges are refused on
section views. Parent-view cutting arrows are also a future capability.

See `examples/agent/create_section_plate_patch.json`,
`examples/agent/create_section_drawing_patch.json` and
`examples/section_plate_drawing.ocad.d`, a self-contained square-hole plate.
Eval `drawing_polygonal_section` checks the authored plane and fresh area/counts.

### Parent-linked detail views (MCAD-P12-004)

Use `add_drawing_view` with optional `detail`:

```json
{"type":"add_drawing_view","sheet_id":"sheet:a4","view":{
 "id":"view:detail","name":"Corner detail",
 "model":{"source_path":"parts/plate.ocad.d","source_doc":"doc:plate"},
 "projection":"front","scale":1.0,"origin_on_sheet_m":[0.15,0.10],
 "detail":{"parent_view_id":"view:front","region_min_m":[0.06,0.04],
           "region_max_m":[0.09,0.07],"magnification":2.0}
}}
```

The parent must be an ordinary view on the same sheet with the same model and
orthographic projection. Crop bounds are projected model metres (not sheet
coordinates), each extent >1e-9 m; magnification >1. Keep child `scale` equal to
parent scale × magnification. In this example parent scale 0.5 yields child scale
1.0. The child sheet origin centers the crop. SVG marks the region on the parent
and prints the derived magnification. No segment/viewport caches may be authored.

Verified regeneration provides `drawing_details` with parent/view IDs, fresh
segment count, scale and magnification. Missing parent, mismatched source or
projection/scale, empty crop, nested or section parents fail before persistence.
Parent scale changes require coordinated child scale changes in the same patch;
existing `set_drawing_view_scale` supports this. Removing a still-referenced parent
fails; remove children first. For a crop/magnification edit, remove/add the child
with a fresh stable ID. Dimensions on detail geometry need separate semantic refs
and are refused in this initial capability.

See `examples/agent/create_detail_plate_patch.json`,
`examples/agent/create_detail_drawing_patch.json` and self-contained
`examples/detail_plate_drawing.ocad.d`. Eval `drawing_bounded_detail` verifies
fresh clipping evidence and authored bounds/parent/scale.

### Explicit title blocks (MCAD-P12-005)

Create or update a sheet's title block through DesignPatch:

```json
{"type":"set_title_block","sheet_id":"sheet:a4","title_block":{
 "title":"Inspection plate","drawing_number":"MUSUBI-001","revision":"A",
 "date":"2026-10-12","author":"MusubiCAD example","material":"Steel",
 "scale_note":"FRONT 1:2"
}}
```

Required fields are title (1–50 characters), drawing number (1–24) and revision
(1–8). Optional date (≤16), author/material (≤24) and scale note (≤50) are authored
single-line text. No automatic date, author, material or scale inference occurs.
Scale note is metadata rather than a measured model value. SVG escapes XML markup.
Control characters and XML-forbidden BMP noncharacters are refused.

The initial frame is 180 × 48 mm at 5 mm bottom/right margins, with 12 mm rows and
explicit 2/3 mm label/value font sizes. A sheet too small for the frame fails
before persistence. Existing views/dimensions/geometry remain unchanged. Remove
with explicit `"title_block":null`; omitting the update field fails rather than
implicitly deleting metadata. Creation through `add_sheet` may also include
`sheet.title_block`. Clear metadata before deleting nothing else; removing a sheet
removes its owned title block. Export SVG to verify the fields and layout.

See `examples/agent/create_title_drawing_patch.json` and self-contained
`examples/title_plate_drawing.ocad.d`; eval `drawing_title_block_revision`.
The fixed template has no automatic wrapping, overlap avoidance or template
library; place drawing views above the block and use bounded field text.

### Multi-page vector PDF (MCAD-P12-006)

Export a drawing with `musubicad export drawing.ocad.d drawing.pdf` or the existing
`opencad.export` Agent method (`path`, `output`). No model patch is needed for a
read-only export. Every sheet, source model and semantic dimension is regenerated
before conversion. PDF pages follow canonical sheet-ID order, preserving each
sheet's authored dimensions in metres, view scale and drawing annotations.

Results include `format:"pdf"`, `pages` and `bytes` (generic `triangles` is zero).
The output embeds a fixed DejaVu Sans subset and preserves vector lines/arcs,
text, source/detail markers, even-odd section hatches and title blocks. There is
no system-font search, external image/resource access or inferred timestamp.
Unsupported glyphs (including CJK not covered by the bundled font), invalid
references/layouts and numeric overflow fail before replacing an existing PDF.
Source `.ocad` files remain unchanged. Fix the authored model/text through
DesignPatch, verify regeneration, then retry export.

Paper dimensions use 72 PDF points per inch, 25.4 mm per inch; PDF-specific precise
SVG root sizing avoids legacy SVG's rounded physical dimensions. Supported bounds
are 1–128 sheets, at most one million PDF objects and finite paper dimensions
≤10⁷ points. The strict converter mode detects missing glyphs; the output is not
claimed to be PDF/A-certified. No arbitrary images/filters are accepted.

`examples/printable_plate_drawing.ocad.d` contains A4 portrait and landscape
sheets, created with `examples/agent/create_printable_drawing_patch.json`; the
embedded part uses the existing detail-plate creation patch.
`examples/printable_plate_drawing.pdf` is a deterministic printable artifact.
Eval `drawing_pdf_revision` checks authored metadata and required PDF evidence.

### Millimetre drawing DXF (MCAD-P12-007)

Use `musubicad export drawing.ocad.d drawing.dxf`, Agent `opencad.export` or MCP
`export_document` with a `.dxf` output. The document must have exactly one sheet;
multiple sheets fail rather than losing pages. All views and semantic dimensions
regenerate before output. Results include `format:"dxf"` and `bytes`.

The AC1027 DXF contains UTF-8 text and right-handed **paper coordinates in mm**,
with `$INSUNITS=4`, `$MEASUREMENT=1` and an explicit paper frame. Authored view scale
is already applied: an 80 mm model edge at 1:2 spans 40 mm in this drawing file,
while its dimension label still reports 80 mm. Use STEP for exact model geometry;
this drawing output does not imply DXF input support or a 1:1 machining profile.

Visible/hidden edges, dimensions, section contours/hatching, detail markers and
title blocks use named layers and LINE/LWPOLYLINE/ARC/TEXT primitives. Section
hatches are analytically clipped at 45 degrees using even-odd material membership,
so holes stay clear. Dimensions are rendered labels/geometry, not associative
DXF DIMENSION records. Wireframe approximation and existing drawing limitations
remain; consumer fonts determine text appearance.

Invalid references/layouts, nonfinite coordinates, budgets or interpreted text
control syntax (`%%`, backslashes and caret codes) fail before replacing output.
Normal Unicode and single `%` characters are preserved. Correct authored values
through DesignPatch, verify dry-run/apply, regenerate and export again. Limits:
±1e9 mm coordinates, 200,000 entities, 100,000 hatch lines, 10 million edge/line
checks. No clock/GUID metadata is generated and source documents are unchanged.

Examples: `examples/title_plate_drawing.dxf` and `section_plate_drawing.dxf`,
from the existing patch-authored drawing examples. Independent verification:
`python -m pip install ezdxf==1.4.4`, then `python tools/test_drawing_dxf.py`.
Eval `drawing_dxf_revision` checks authored revision plus required mm DXF output.
See ADR-046.

### Verified assembly bill of materials (MCAD-P12-008)

Call MCP `bom_document` with `{ "path": "assembly.ocad.d" }`, Agent
`opencad.bom_document`, or `musubicad bom assembly.ocad.d`. Optional `output`
(`--output` in CLI) writes `.json` or `.csv` outside every source document.
The same structured BOM is returned whether or not a table is written.

`declared_component_count` counts root declarations; `top_level_instance_count`
includes expanded root patterns. `leaf_part_count` counts verified leaf source
files and `leaf_instance_count` counts all placed leaf occurrences. Each row has
`source_doc`, `name`, canonical root-relative `source_path`, dimensionless
`quantity`, and structured root-to-leaf `instance_paths`. Fixed and movable
instances both count; unused declarations add no quantity. Repeated subassemblies
multiply their internal leaf occurrences, not their declaration counts.

Rows combine repeated references to the same verified file/document. Copies at
different resolved paths remain separate even when their UUIDs match. No guessed
part number, material, cost or exclusion rule is introduced. CSV uses quoted
strings and a `quantity_count` column; JSON preserves full instance provenance.
Counts and ordering are deterministic; names come from authored metadata.

Missing/wrong-kind/identity references, cycles, excessive expansion or failed
regeneration at **any** assembly level refuse the BOM. Root compounds alone can
hide localized child failures, so the backend verifies each snapshotted assembly.
OCCT now transforms compound members exactly and keeps them as separate solids;
it does not Boolean-union repeated parts. Queries do not modify input graphs.
Outputs are written by same-directory atomic replacement after verification;
model paths are refused and external hardlinks cannot corrupt source files.
Bounds: 64 instance levels, 10,000 sources, 100,000 occurrence visits.

Use DesignPatch to author/edit components, instances and patterns, then verify
and query again. `examples/bom_nested_assembly.ocad.d` contains two copies of a
three-plate patterned subassembly plus one direct plate: seven leaf plates in
one row, three top-level instances and two declared components. Its JSON/CSV
artifacts and root/subassembly authoring patches are included. Eval
`assembly_bom_add_subassembly` adds another subassembly and checks ten plates.
See ADR-047.

### Planar DXF sketch input (MCAD-P12-009)

Inspect first with `inspect_dxf` / Agent `opencad.inspect_dxf` using `dxf_path`,
or `musubicad inspect-dxf source.dxf`. Inspect version, declared units and entity
counts by layer, then explicitly select geometry layers for `import_dxf`:

```json
{"path":"part.ocad.d","dxf_path":"source.dxf","sketch_id":"sketch:dxf","feature_id":"feature:sketch_dxf","layers":["OUTLINE"],"dry_run":true}
```

Verify, then set `dry_run:false` to apply through DesignPatch/history. Return the
opaque `history` for existing undo/redo methods. CLI equivalent:
`musubicad import-dxf part.ocad.d source.dxf --sketch-id sketch:dxf --id feature:sketch_dxf --layers OUTLINE`.
Optional `source_unit` (`--unit`) supports `m`, `mm`, `cm`, `in` when INSUNITS is
missing/unassigned; it must agree with declared supported units. Never guess units
from destination metadata. Summary includes source SHA-256, selected layers,
source unit, primitive/entity counts, verification and history.

Support planar model-space LINE, CIRCLE, ARC and straight LWPOLYLINE. Circles/arcs
remain analytic; degrees convert to radians and CCW arcs can cross zero degrees.
Coordinates become authored SI literals on XY, with fixed 1 nm point-grid identity.
Planarity/thickness/width tolerance is 1 nm; OCS tolerance 1e-12, bulge tolerance
1e-15 dimensionless. Collapsed geometry, malformed/ambiguous tags, conflicting or
unsupported units, selected unsupported entities and non-default OCS refuse before
changes. Binary DXF, INSERT/spline/ellipse expansion, styles and semantic drawing
annotations are unsupported. Layers not selected are deliberately excluded.

The imported feature is an ordinary sketch. Bare sketches are valid staged parts;
extrude/hole/revolve consumers enforce existing profile rules. Coordinates are
editable design inputs, not a linked DXF cache or auto-constrained dimensions.
Drawing DXF output contains view-scaled paper coordinates: importing it preserves
those coordinates and cannot recover the original model scale or semantic refs.
Limits: 16 MiB, 500,000 tag pairs, 50,000 primitives, finite coordinates/radii ≤1e6 m.
Example input `examples/import/plate_outline.dxf`; verified extruded result
`examples/dxf_import_plate.ocad.d` is an 80×60×6 mm plate with an authoring patch.
See ADR-048; eval `dxf_import_copy_sketch` exercises the same existing APIs.

### Explicit-unit STL solid input (MCAD-P12-010)

Use `import_stl` with `path`, `stl_path`, a new `feature_id` and mandatory
`source_unit` (`m`, `mm`, `cm`, `in`). STL has no trustworthy unit; destination
metadata and header text never supply it. Optional `transform` is a proper rigid
placement: `translation_m` and a dimensionless 3×3 `rotation`. Verify with
`dry_run: true` before applying. The returned `history.history` can be passed to
history undo/redo; a returned top-level history state is not the history itself.

```json
{"path":"part.ocad.d","stl_path":"examples/import/closed_plate_mm.stl","feature_id":"feature:stl_plate","source_unit":"mm","dry_run":true}
```

ASCII/binary input is bounded to 4 MiB and 10,000 triangles. Admission requires
one connected closed outward-oriented manifold shell with no self-intersection.
Coordinates convert once to metres and weld on a 1 nm grid. A deterministic
intersection sweep refuses after 1,000,000 candidate pairs. OCCT sews at 10 nm,
checks B-rep validity/self-interference and compares mesh/solid volume. Invalid
or unresolved candidates leave the document unchanged; no automatic repairs.

An `imported_mesh` feature stores an immutable `imports/mesh_<sha256>.stl` asset,
SHA-256, explicit unit and placement. Identical bytes share the asset; differing
units still remain explicit per feature. AddAttachment/AddFeature DesignPatch
and backend transaction/history perform all changes. Open scans, disconnected
solids, cavities, repair and surface editing are outside initial admission.
Mock reports genuine mesh bounds/mass/area and placement; imported-mesh CSG,
shape edits, compounds and inertia refuse rather than invent geometry. OCCT
is the production solid path. Existing STEP inputs retain their original tag.

The example `stl_import_plate.ocad.d` and its authoring patch reconstruct an
80×60×6 mm plate, volume 28,800 mm³. Eval `stl_import_explicit_units` independently
checks volume, the explicit-unit feature and source byte digest. Checker tests
are run locally; a paid agent-host run is separate evidence.

### Construction geometry edits (MCAD-P13-001)

Use `set_sketch_construction` through the existing verified patch tools:

```json
{"operations":[{"type":"set_sketch_construction","sketch_id":"sketch:base","entity_id":"ent:guide_line","construction":true}]}
```

The boolean is explicit and dimensionless; repeated requests do not invert it.
Stored Point/Line/Circle/Arc/Rectangle IDs, coordinates and constraints remain.
Rectangle helpers use their stored helper ID, not derived corner/edge IDs.
Construction endpoints still participate in constraints. Construction curves
stay in the solver and are excluded from solid profiles. Ordinary open guides
also do not become closed solid boundaries.

Dry-run before applying with history. Unknown entities, lost consumed profiles,
or unchanged consumer aliases selecting a different loop refuse atomically.
Explicitly rebind/remove consumers in the same patch when changing a boundary;
never silently substitute another loop for `profile:outer`. Semantic diff reports
`sketch_entity_modified`, and impact marks the sketch and downstream features.
No-op flags have an empty semantic diff. Source defaults for old files remain
false. Example: `construction_plate.ocad.d` and its authoring patch; eval
`construction_geometry_edit` checks stable entity types/flags and plate volume.

### Analytic trim/extend plans (MCAD-P13-002)

`plan_curve_edit` reads a part and returns a DesignPatch; it does not write.
Pass `path` and nested `edit` with `sketch_id`, stored `target_id` (line/arc),
`cutter_id` (finite line, circle or arc), `mode` (`trim`/`extend`), `end`
(`start`/`end`) and a fresh `new_point_id`. Multiple eligible intersections
require `intersection_index`, sorted by target parameter. Tangent, coincident,
degenerate or out-of-extent cuts refuse. Coordinates/radii use metres, angles
radians; 1 nm contact tolerance and bounded positive sub-full-turn arc sweeps.

```json
{"path":"part.ocad.d","edit":{"sketch_id":"sketch:base","target_id":"ent:guide_line","cutter_id":"ent:cutter","mode":"trim","end":"end","new_point_id":"ent:edited_end","remove_constraints":["con:guide_length"]}}
```

Review and dry-run the returned patch before `patch_apply`. Shared old endpoint
IDs are preserved; only the target endpoint reference/angle changes. Constraints
are removed only by an explicit `remove_constraints` list. A retained constraint
can move the endpoint away from the intersection; the included
`sketch_point_position` expected effect checks a solved copy after parameter and
constraint evaluation, refusing wrong results even with `verify:false` or CLI
`--no-verify`. Keep the revision guard and expected effect. Closed consumers
may need explicit closing/rebinding operations in that same patch.

The patch uses `replace_sketch_entity`, retaining the stored primitive ID/type;
rectangle helpers require explicit conversion before derived edges are edited.
Arc centre/radius expressions and the unaffected angle/endpoint remain source;
the edited angle is explicit fixed geometry, with no inferred cutter association.
Examples: `trim_extend_plate.ocad.d` and its authoring patch. Eval
`sketch_extend_guide` checks source SI endpoints, old point preservation and
unchanged plate volume; production verification checks the solved endpoint.

### Independent analytic sketch offsets (P13-003, ADR-052)

Use `plan_offset` (CLI `plan-offset`, Agent `opencad.plan_offset`) with
`{"path":"part.ocad.d","edit":{...}}`. Required edit fields:

```json
{
  "source_sketch_id": "sketch:base",
  "curve_ids": ["ent:e0", "ent:e1", "ent:e2", "ent:e3"],
  "distance_m": 0.003,
  "association": "independent_copy",
  "join": "mitre_tangent",
  "destination_sketch_id": "sketch:offset",
  "name": "Offset copy",
  "destination_curve_ids": ["ent:o0", "ent:o1", "ent:o2", "ent:o3"],
  "destination_point_ids": ["ent:p0", "ent:p1", "ent:p2", "ent:p3"]
}
```

Supply explicit ordered counterclockwise line/arc IDs (rectangle-derived edges
are supported), or one circle. Consecutive source curves must share endpoint
IDs; coincident coordinates alone do not repair topology. Arcs require explicit
endpoint IDs. The closed contour must be convex; arc joins
must be tangent. Positive distance expands, negative distance insets. Collapse,
concavity, non-tangent joins and duplicate/found destination IDs refuse. Point
IDs name each curve's start vertex, followed by arc centres in curve order; a
circle requires one centre ID. All destination entity IDs must be globally
fresh. The same source workplane is used. Source entities/constraints remain
unchanged; source constraints are not copied. **This is an independent copy:
source coordinate/parameter edits do not update it.** Semantic workplane
references retain their ordinary plane dependency.

Review the returned revision-guarded DesignPatch. It authors a complete sketch
using existing operations, without implicitly adding features. If needed,
append a sketch feature and extrusion with explicit new IDs and profile ref,
then `patch_dry_run` and verified `patch_apply` with history. Undo/redo uses the
existing history tools. The 80 x 60 mm +3 mm example is
`examples/offset_plate.ocad.d`; its new 86 x 66 x 6 mm body is 34056 mm3.

### Analytic closed ellipses (P13-004, ADR-053)

Author an ellipse via `add_sketch_entity` (or edit the same ID/type with
`replace_sketch_entity`), then use a sketch feature and a normal extrusion:

```json
{
  "type": "add_sketch_entity",
  "sketch_id": "sketch:ellipse",
  "entity": {
    "type": "ellipse",
    "id": "ent:ellipse",
    "construction": false,
    "center": "ent:center",
    "major_radius": "major_radius",
    "minor_radius": "minor_radius",
    "axis_angle": "axis_angle_rad"
  }
}
```

Create `ent:center` first. Radii are **semiaxes**, not diameters: literal values
are metres, and literal angles are radians. For example, parameters
`major_radius: 30 mm`, `minor_radius: 20 mm`, `axis_angle_rad: 30 deg` and
`thickness: 6 mm` produce volume `pi*0.03*0.02*0.006 m3` (11309.73355 mm3).
Named angle parameters must use `_rad`, `_deg` or `angle` in their name, per the
existing evaluator contract. Ellipse parts use strict inline length/angle units:
length m/mm/cm/in; angle rad/deg and spelled variants. Incompatible or unknown
literal units refuse; do not treat `1 mm` as an angle. Initial ellipse-part
expressions require same-kind length/angle parameter references and literal
factors; cross-kind formulas require a future dimensional-expression contract. Source expressions remain
persisted; resolved axes/angles are transient and absent from the document.

Major must be >= minor > 1 nm (equal axes explicitly admit a circle). The centre
point participates in existing Coincident/Distance/etc. constraints; axes and
orientation are source parameters, not new free sketch DOFs. Construction
ellipses stay out of profiles. Radius/diameter/equal-radius/tangent constraints,
elliptical arcs and mixed ellipse loops are not supported. Adding/removing a
closed curve cannot implicitly rebind an unchanged feature's profile alias.

Distance and centred symmetric extents are supported; through-all refuses
until target bounds can be resolved explicitly. Native OCCT supports exact
normal new-body extrusion and the existing feature
cut/join paths; ellipse-based loft/sweep/helix/revolve/trim/offset refuse. Mock
supports analytic normal new-body volume, bounds, COM/surface area and rigid
matrix placement; Mock topology/shape edits, booleans and tessellation refuse.
Native BREP rendering works; ellipse sketch-overlay picking remains deferred.
Always dry-run and use verified `patch_apply` with history. Example:
`examples/ellipse_plate.ocad.d` and `agent/create_ellipse_plate_patch.json`.

### Associated cross-sketch projection (P13-005, ADR-054)

Create a `projected_sketch` feature with a new destination sketch ID, explicit
Global/Custom plane and semantic upstream curve bindings. Do not create a raw
sketch with the destination ID or copy generated coordinates into the document:

```json
{
  "type": "add_feature",
  "node": {
    "id": "feature:projection",
    "name": "Projected sketch",
    "definition": {
      "type": "projected_sketch",
      "sketch_id": "sketch:projection",
      "workplane": {"type":"global","plane":"XY"},
      "projections": [{
        "source_feature": "feature:source_sketch",
        "source_entity": "ent:source_circle",
        "curve_id": "ent:projected_curve",
        "point_ids": ["ent:projected_center"],
        "construction": false
      }]
    }
  },
  "position": {"at":"end"}
}
```

Sources are upstream sketch features (including projected sketches), addressed
by stable stored/prepared line/circle/ellipse entity IDs. Rectangle-derived
edges use their declared IDs. Lines require two destination endpoint IDs;
full circles/ellipses require one centre ID. Shared destination points must
refer to the same semantic source vertex; coincident different vertices do not
merge. Source dependencies are graph-derived; missing/ambiguous/suppressed
sources, cycles, invalid planes and collapsed projections refuse.

Orthogonal world-to-plane projection uses metres. A circle projected to an
oblique plane becomes an exact ellipse. Source parameter changes invalidate
downstream outputs and preserve destination IDs. Construction curves stay out
of profiles; normal consumer closure rules apply. Append a normal extrusion
with `sketch_feature: feature:projection` and profile
`sketch:projection/profile:outer`, then dry-run and verified apply/history.

Use `inspect_projected_sketch` / Agent `opencad.inspect_projected_sketch` / CLI
`inspect-projected-sketch <part> <feature_id>` to inspect current derived SI
geometry and bindings without writing. Edit the upstream source or the
projection's definition; generated entities are not authored sketch objects.
Initial scope excludes body-edge picks, Datum/FaceRef planes, arcs and splines.
Example: `projected_circle_plate.ocad.d` and its authoring patch. A 30 mm circle
projected to a 30-degree tilted plane and extruded 6 mm has volume
`pi*.03^2*cos(pi/6)*.006 m3` in both native and analytic Mock ellipse paths.
Legacy Mock line/parallel-circle solids remain synthetic.

## Explicit exploded assembly views (MCAD-P13-006 / ADR-055)

Persist a named presentation with `add_exploded_view`; update it with
`set_exploded_view` (retaining its ID) or delete it with `remove_exploded_view`.
Use the normal dry-run/apply/history commands. Example operation:

```json
{"type":"add_exploded_view","view":{"id":"view:exploded","name":"Exploded",
 "offsets":[{"instance_id":"instance:upper_arm","translation_m":[0.1,0,0]}]}}
```

Offsets are **world-space SI metres**, applied after mechanical regeneration.
Target declared top-level instance IDs exactly; each child assembly moves as one
rigid compound. Unlisted instances stay in place. A view needs a nonempty name
and 1..256 unique offsets; missing/ambiguous targets, nested paths, generated
pattern IDs, non-finite coordinates and values above one million metres refuse.
Remove dependent views or replace their offsets before removing an instance.

Select the saved view with MCP `preview_document` or Agent
`opencad.preview_document` parameters `{"path":"...","exploded_view":"view:exploded"}`.
The result labels the selected presentation and includes a deterministic PNG.
CLI export uses `musubicad preview <assembly> <out.png> --exploded-view view:exploded`.
Omit the selector for the mechanical view. Plain STEP/STL/GLB, drawings, BOM,
interference and URDF retain mechanical positions. Exploded drawing projections,
rotations, internal child offsets and automatic spacing are deferred.
The desktop backend exposes `load_assembly_presentation` for the same contract;
its reports remain mechanical while its render scene uses cloned meshes.

See `examples/agent/add_arm_exploded_view_patch.json` and the saved view in
`examples/robot_arm_assembly.ocad.d`. Do not apply that add operation twice to
an already configured example; edit it with `set_exploded_view` instead.
Presentation tessellations use existing f32 SI coordinates; tests compare bounds
within 1 µm and volume within 1e-10 m³. Analytic Mock/OCCT evidence uses a closed
80×60×6 mm triangle box (2.88e-5 m³), avoiding legacy synthetic Mock extrusions.

## Coupled gear/rack joints (MCAD-P13-007 / ADR-056)

Author `add_joint_coupling`, `set_joint_coupling` or `remove_joint_coupling`
through DesignPatch, dry-run and verified apply/history. Bind **declared joint
IDs**, not guessed instance names. Example operation:

```json
{"type":"add_joint_coupling","coupling":{"type":"gear","id":"coupling:gear",
 "driver_joint":"joint:shoulder","follower_joint":"joint:elbow","ratio":-0.5}}
```

Gear displacement is `follower_rad = ratio * driver_rad`; signed ratio is
explicitly dimensionless. Rack displacement is
`follower_m = metres_per_radian * driver_rad`, with signed SI m/rad coefficient.
Both refer to authored zero; there is no inferred phase or tooth contact.
Drivers must rotate; gear followers rotate, rack followers translate. One owner
per follower, known active source joints, acyclic dependencies and at most 256
relations are required. The entire declared finite driver range must map within
the follower range, and finite ranges must contain zero. Unlimited drivers cannot
feed finite followers. Continuous/continuous gear pairs use unwrapped radians.
Absolute ratios must exceed 1e-12, rack coefficients 1e-9 m/rad; both are at most
1e6 in their respective units. Non-finite values, wrong types, dangling references,
cycles, duplicate owners and explicit follower conflicts refuse atomically.

MCP `inspect_joint_pose`, Agent `opencad.inspect_joint_pose`, or
`musubicad inspect-joint-pose <assembly> joint:shoulder=90deg` inspect the same
read-only backend. Request `positions` is a map from exact `joint:` IDs to strings
with `deg`, `rad`, `mm` or `m`. Response positions use explicit `position_rad` or
`position_m`, `derived`, joint/instance IDs and SI world placements. Bare numbers,
mismatched units, unknown/non-moving joints and out-of-range input refuse.
A -0.5 gear ratio at shoulder 90deg yields derived elbow -45deg; editing the ratio
to -0.25 yields -22.5deg. Do not move source placements to fake the result.

Followers are excluded from independent animation/reach/workspace controls;
shared pose evaluation derives them. Motion interference reuses the existing
sample grid for independent drivers and checks all posed body pairs for coupled
assemblies, including collisions within two moving subtrees. It is sampled
motion evidence, not continuous/all-combinations clearance. The legacy assembly
DOF field remains a mate-solver estimate; derived-coordinate flags identify which
joint coordinates are independently controlled.

Gear URDF emits named zero-offset mimic relationships. Rack URDF and coupled
HTML viewer export refuse before producing artifacts; cross-unit URDF semantics
and JS coupling controls are deferred. Native OCCT executes exact coupled solid
intersections. Mock imported-solid placements have true dimensions/volume, but
exact imported-solid Boolean intersection and multi-solid compounds still refuse.
No fake Mock collision result is accepted. Example: `coupled_arm.ocad.d` with
`examples/agent/add_arm_gear_coupling_patch.json`. The example already contains
that association; use set instead of adding its ID again.

Wire compatibility: coupled followers serialize as `type:"coupled"` with their
owning `coupling_id` and nested physical `motion`. Historical motion enums reject
that tag instead of silently losing the relationship. Current decoding validates
one-to-one guards and returns normal physical joints plus source couplings.
Absent collections remain empty; legacy joint bytes retain their meaning. Do not
strip the wire guard or add a relationship by editing JSON directly.

## Closed cubic sketch spline (P13-008)

See [the exact source and admission contract](cubic-spline.md). Author three
stable point IDs with length coordinates/expressions, then `cubic_bezier` with
four ordered references whose final ID equals the first. Degree-three clamped
knots and unit weights are fixed; unsupported fields refuse. The distinct
controls must form a well-conditioned CCW triangle. Use existing control-point
constraints and `profile:bezier:<curve_id>` for normal distance/centred symmetric
NewBody extrusion. Construction curves do not supply profiles. Other spline
operations and general NURBS editing remain deferred; do not approximate them
with polygons. Preserve point expressions on edits; dry-run then apply a
DesignPatch, inspect returned history, and call `regen_document`.

`cubic_plate.ocad.d` uses `a=b=40 mm`, height 6 mm, volume 1440 mm³. Changing
`param:a` to `60 mm` yields 2160 mm³, bounds X `[0,26.666667]` mm and Y
`[0,17.777778]` mm, without replacing `ent:p1.x="a"` or `ent:p2.y="b"`.
Eval `cubic_edit_control` grades source IDs/expressions and analytic volume.

## Phase 14 desktop command admission

[ADR-058](../adr/ADR-058-backend-only-desktop-editing.md) defines the backend-only
GUI contract. P14-001 adds architecture and acceptance evidence requirements;
feature-tree, selection patch and interactive sketch editors are planned.
Continue using existing CLI/Agent/MCP DesignPatch paths. Future GUI model actions
must use those same verified backend semantics: explicit units, stable graph/
semantic IDs, source/scene revision admission, concrete patch/effect preview,
candidate regeneration and atomic source/history/scene publication. Ambiguous,
missing, stale or unsupported selection bindings refuse; frontend feature-name
hints cannot bind mutations. UI sketch overlays are drafts until a backend
source/constraint patch is validated and committed. Backend history remains
opaque to the frontend. Static handler audits and headless tests do not alone
prove an interactive editor, and local MCP is separate from real host evidence.

## Part feature-tree editing (MCAD-P14-002)

Query `list_features` for stable row IDs, admitted fields/profiles and revision. Submit `plan_feature_edit` with `{path, revision, edit}`, review the concrete patch and SI volume/bounds, then submit the same edit to `apply_feature_edit`. Use the new revision for each operation and pass returned `history.history` unchanged. Creation is NewBody distance extrusion from an authored closed sketch; dimension forms expose the six supported length fields. Moves/suppression enforce dependencies; stale/invalid edits do not change source/history. Parameter and history desktop commands now regenerate and verify native mesh before publication. BodyCount assertions on nonempty candidates fail closed because physical solid count is unavailable. See [feature-editor.md](feature-editor.md) and `examples/agent/feature_tree_edit.json`; the `feature_tree_edit` eval checks volume 2400 mm³, source order, identity and suppression.
