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
   problem. Errors list every dependent or missing reference by stable ID.
4. Optionally, `review_patch` writes a before/after HTML review with images
   and mass and bounds changes.
5. `patch_apply` validates the patch again and writes the document. A failed
   patch leaves the document unchanged.

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
4. **Driving constraints.**
   - `{"type": "add_sketch_constraint", "sketch_id": "sketch:base", "constraint": {"type": "distance", "id": "con:width", "target": {"line": "ent:e0"}, "expr": "width"}}`
   - `{"type": "radius", "id": "con:hole_radius", "target": "ent:hole_circle", "expr": "hole_diameter / 2"}`
5. **A sketch feature per sketch.**
   `{"type": "add_feature", "position": {"at": "end"}, "node": {"id": "feature:sketch_base", "name": "Base Sketch", "definition": {"type": "sketch", "sketch_id": "sketch:base"}}}`
6. **Solid features.**
   - Extrude: `{"type": "extrude", "sketch_feature": "feature:sketch_base", "profile_ref": "sketch:base/profile:outer", "extent": {"type": "distance", "length": {"value_si": 0.005}}, "operation": "new_body", "length_expr": "thickness"}`
   - Hole: `{"type": "hole", "sketch_feature": "feature:sketch_hole", "profile_ref": "sketch:hole/profile:outer", "depth": {"type": "distance", "length": {"value_si": 0.005}}, "target_feature": "feature:plate", "depth_expr": "thickness"}`

Other feature types are `revolve`, `fillet`, `chamfer`, `linear_pattern`,
`circular_pattern`, and `mirror_pattern`. Call `authoring_patch` on the
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
- **Drawings:** `add_sheet` (added empty), `add_drawing_view`,
  `add_drawing_dimension`, and their `remove_*` forms.

Object shapes match `graph/assemblies.json` and `graph/drawings.json`; run
`authoring_patch` on `examples/robot_arm_assembly.ocad.d` or
`examples/bracket_front_view.ocad.d` for complete examples.
