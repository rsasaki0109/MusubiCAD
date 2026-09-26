# Agent API

MusubiCAD exposes a JSON-RPC 2.0 API for AI agents and automation tools.

Transport: **stdio** via `opencad agent`. No network server is started by default.

## Invocation

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"opencad.inspect","params":{"path":"bracket.ocad.d"}}' \
  | opencad agent
```

## Methods

### In-memory (no file I/O)

| Method | Params | Result |
|---|---|---|
| `opencad.patch_dry_run` | `{ parameters, feature_nodes, semantic_refs?, assembly?, drawing?, patch }` | `{ validation, diff, impact }` |
| `opencad.patch_apply` | `{ parameters, feature_nodes, semantic_refs?, assembly?, drawing?, patch }` | `{ parameters, feature_nodes, semantic_refs, assembly?, drawing?, diff, impact }` |
| `opencad.diff` | `{ before, after }` (each may include `semantic_refs`, `assembly`, `drawing`) | `DesignDiff` |
| `opencad.regen` | `{ parameters, sketches, feature_graph, feature_nodes }` | `RegenResult` |
| `opencad.query` | `{ parameters, feature_nodes, feature_graph?, assembly?, drawing?, query }` | `QueryResult` |
| `opencad.explain` | `{ parameters, feature_nodes, feature_graph?, sketch_count?, document_name? }` | `DesignExplanation` |

### Document (`.ocad` / `.ocad.d`)

| Method | Params | Result |
|---|---|---|
| `opencad.inspect` | `{ path }` | document summary |
| `opencad.validate` | `{ path }` | `{ valid, path }` |
| `opencad.patch_dry_run_document` | `{ path, patch }` | `{ validation, diff, impact }` |
| `opencad.patch_apply_document` | `{ path, patch, history? }` | `{ patched, history, can_undo, can_redo }` |
| `opencad.history_undo_document` | `{ path, history }` | `{ history, can_undo, can_redo }` |
| `opencad.history_redo_document` | `{ path, history }` | `{ history, can_undo, can_redo }` |
| `opencad.regen_document` | `{ path }` | `RegenResult` |
| `opencad.export` | `{ path, output }` | `ExportSummary` |
| `opencad.diff_document` | `{ before, after? \| patch?, geometry? }` | `DesignDiff` |
| `opencad.query_document` | `{ path, query }` | `QueryResult` |
| `opencad.pick_document` | `{ path, x, y, width?, height? }` | `PickSummary` |
| `opencad.explain_document` | `{ path }` | `DesignExplanation` |

Patch application is atomic across parameters, feature nodes, semantic
references, assembly state, and drawing state. The in-memory `patch_apply`
path and the document patch path stage a candidate state and commit it only
after every operation validates; a later operation failure cannot retain an
earlier operation's mutation.

For optimistic concurrency, a patch may include a complete-state revision
precondition. The digest covers the canonical serialized patchable state, not
B-Rep or mesh caches:

```json
{
  "type": "revision_equals",
  "algorithm": "sha256",
  "version": "musubicad.design-state.v2",
  "digest": "<64 lowercase hexadecimal characters>"
}
```

| `version` | Covers | Accepted for |
|---|---|---|
| `musubicad.design-state.v2` (current) | parameters, feature nodes, semantic references, optional assembly/drawing, sketches, and design assertions | every patch |
| `musubicad.design-state.v1` (legacy, ADR-008) | parameters, feature nodes, semantic references, optional assembly/drawing | value-edit patches only; a structural patch guarded by v1 is rejected as too weak |

`algorithm` and `version` are validated explicitly. A stale digest is rejected
before mutation with the same deterministic validation error from dry-run,
in-memory apply, file apply, and Agent API paths. Rust callers can calculate
the current value with `opencad_ai::design_state_revision`, a specific version
with `design_state_revision_for_version`, or attach one with
`PatchPrecondition::revision_equals`. Document-backed paths build the state
with `opencad_file::document_design_state`, so v2 digests include the
document's sketches and assertions. In-memory `opencad.patch_*` requests do not
transport sketches or assertions, so their v2 digests cover empty collections.

The history methods accept and return the serialized `history` value produced
by a prior backend edit. Clients must treat it as opaque and pass it back
unchanged; `can_undo` and `can_redo` are the only capability values needed for
toolbar state. History snapshots are held outside the `.ocad` schema and
contain only complete `OcadDocument` source states, never viewport, camera,
selection, B-Rep, or mesh state. A stale document is rejected before either
the document or history value is changed.

Dry-run and apply share the validated candidate builder. Rust callers that
need the same contract can use
`opencad_ai::build_patch_candidate(&DesignState, &DesignPatch)`; it clones the
Design Graph, applies all operations, validates the final structural state,
and evaluates the resulting parameter graph before returning the candidate. Assembly operations require an assembly
model and drawing operations require a drawing model; missing context is a
deterministic validation error in both paths.

`impact` uses the shared `ChangeImpact` DTO documented in
[Change impact and regeneration trace](change-impact-and-regeneration-trace.md).
Document dry-run predicts the exact dependency-propagated Feature Graph suffix.
`RegenResult.trace` uses the same serializable `RegenerationTrace` returned by
Desktop regeneration.

Rust callers that must validate a part regeneration in the same boundary can
use `opencad_desktop::apply_patch_and_regenerate`. The command-layer helper runs the patch and
regeneration against a cloned candidate and swaps the candidate into the
document only after successful regeneration. B-Rep and mesh outputs remain
disposable and are never serialized. The API accepts part documents; assembly
and drawing regeneration continue through their specialized pipelines.

### Query kinds (`query.kind`)

| kind | Description |
|---|---|
| `list_parameters` | All parameters with evaluated values |
| `get_parameter` | Single parameter (`id`) |
| `list_features` | All features (id, name, type) |
| `get_feature` | Single feature with full definition (`id`) |
| `feature_order` | Topological regeneration order |
| `list_sketches` | All sketches (id, name, entity/constraint counts) |
| `get_sketch` | Full sketch definition (`id`) |
| `list_sketch_constraints` | Constraints in a sketch (`sketch_id`) |
| `list_sketch_entities` | Entities in a sketch (`sketch_id`) |
| `feature_dependencies` | All feature dependency edges |
| `get_feature_dependencies` | Upstream/downstream features (`id`) |
| `parameter_dependencies` | All parameter dependency edges |
| `get_parameter_dependencies` | Upstream/downstream parameters (`id`) |
| `list_overlay_lines` | Pickable sketch overlay segments (`line_index`, `sketch_id`, `entity_id`) |
| `list_face_groups` | Tessellated solid face groups with inferred feature/topo refs |
| `list_semantic_refs` | Persisted `TopoRef` entries from `semantic_refs.json` |
| `get_semantic_ref` | Single persisted `TopoRef` (`ref_id`) |
| `list_assembly_instances` | Placed assembly instances (requires `assembly` context) |
| `get_assembly_instance` | Single assembly instance (`id`) |
| `list_assembly_mates` | Assembly mate constraints |
| `list_connectors` | Named connector frames on instances |
| `list_drawing_sheets` | Drawing sheets (requires `drawing` context) |
| `get_drawing_sheet` | Single drawing sheet (`id`) |
| `list_drawing_views` | Views on a sheet (`sheet_id`) |
| `get_drawing_view` | Single drawing view (`sheet_id`, `view_id`) |

Semantic-reference query results preserve the persisted `TopoRef.ref_id` as the
identity key. Kernel face/edge IDs are regeneration hints; fallback matching
uses the unit-labelled `TopoRefTolerancePolicy` documented in
[`docs/api/topo-ref.md`](topo-ref.md).

Assembly query kinds require an `assembly` field in in-memory `opencad.query`, or an assembly document path in `opencad.query_document`.

Drawing query kinds require a `drawing` field in in-memory `opencad.query`, or a drawing document path in `opencad.query_document`. Drawing patches support `set_drawing_view_scale` and `set_drawing_view_origin`; origins use meters through `origin_on_sheet_m`.

`list_overlay_lines` and `list_face_groups` require document tessellation. Use `opencad.query_document` (or pass a `scene` context to in-memory `opencad.query`).

### `PickSummary`

Headless GPU pick at viewport pixel coordinates (same default camera as `opencad mesh --render`).

```json
{
  "x": 256.0,
  "y": 256.0,
  "width": 512,
  "height": 512,
  "overlay_line_count": 8,
  "triangle_count": 248,
  "selection": {
    "kind": "solid_triangle",
    "triangle_index": 42,
    "vertices_m": [[0.04, 0.003, 0.02], [0.04, 0.003, -0.02], [-0.04, 0.003, -0.02]],
    "face_group_index": 3,
    "face_role": "top",
    "face_normal_m": [0.0, 0.0, 1.0],
    "face_centroid_m": [0.04, 0.03, 0.006],
    "inferred_feature_id": "feature:extrude_base",
    "inferred_topo_ref_id": "ref:face:extrude_base_top"
  }
}
```

Selection kinds: `none`, `sketch_line`, `solid_triangle`.

`sketch_line` includes `sketch_id`, `entity_id`, and optional `segment_index` (circle tessellation chords).

`solid_triangle` includes `face_group_index`, `face_role`, `face_normal_m`, `face_centroid_m`, `kernel_face_id` (OCCT B-Rep face ID when tessellated via OCCT), and inferred `inferred_feature_id` / `inferred_topo_ref_id`. When `kernel_face_id` is present, `inferred_topo_ref_id` uses `ref:face:kernel_{id}`.

### `list_overlay_lines` / `list_face_groups`

Enumerate pick targets without a pixel coordinate (same tessellation as `opencad pick`).

Persisted face references live in `graph/semantic_refs.json`. Sync them after regeneration:

```bash
opencad regen bracket.ocad.d --sync-topo-refs
```

```json
{ "kind": "overlay_lines", "items": [
  { "line_index": 0, "sketch_id": "sketch:base", "entity_id": "ent:e0", "entity_kind": "line",
    "construction": false, "start_m": [0.0, 0.0, 0.0], "end_m": [0.08, 0.0, 0.0] }
]}
```

```json
{ "kind": "face_groups", "items": [
  { "face_group_index": 3, "face_role": "top", "triangle_count": 48,
    "face_normal_m": [0.0, 0.0, 1.0], "face_centroid_m": [0.0, 0.0, 0.006],
    "kernel_face_id": 18446744073709551615,
    "inferred_feature_id": "feature:extrude_base", "inferred_topo_ref_id": "ref:face:kernel_18446744073709551615" }
]}
```

### `DesignExplanation`

```json
{
  "summary": "Bracket with Hole: 7 parameters, 4 features, 2 sketches. ...",
  "document_name": "Bracket with Hole",
  "parameter_count": 7,
  "feature_count": 4,
  "sketch_count": 2,
  "parameters": [{ "id": "param:width", "name": "width", "expr": "80 mm", "value_m": 0.08 }],
  "features": [{ "id": "feature:extrude_base", "name": "Extrude Base", "feature_type": "extrude", "suppressed": false }],
  "feature_order": ["feature:sketch_base", "feature:extrude_base", "feature:sketch_hole", "feature:hole_mount"]
}
```

### `ExportSummary`

```json
{
  "format": "stl",
  "triangles": 248,
  "output": "bracket.stl"
}
```

The output extension selects the format:

| Extension | Format | Source |
|---|---|---|
| `.stl` | Binary STL mesh | Part or assembly |
| `.step` / `.stp` | STEP (ISO 10303-21, AP214) B-rep in millimetres | Part (active body) or assembly (every placed instance, with mates solved) |
| `.svg` | Drawing sheet | Drawing |

STEP output is deterministic: the header time stamp is fixed at
`1970-01-01T00:00:00` and the product is named `MusubiCAD`. For STEP,
`triangles` is `0` and `bytes` reports the file size:

```json
{ "format": "step", "triangles": 0, "output": "bracket.step", "bytes": 137102 }
```

### `RegenResult`

```json
{
  "kernel": "OCCT 8.0.0 (cadrum static)",
  "regenerated": ["feature:sketch_base", "feature:extrude_base"],
  "skipped_suppressed": [],
  "volume_m3": 2.833178323652379e-5,
  "mass_kg": 0.07649581473861423,
  "density_kg_per_m3": 2700.0,
  "trace": {
    "executed_nodes": ["feature:sketch_base", "feature:extrude_base"],
    "skipped_nodes": [],
    "solver_call_count": 1,
    "geometry_kernel_call_count": 3,
    "elapsed_time_ms": 2,
    "output_hashes_sha256": {},
    "trace_hash_sha256": "<sha256>"
  }
}
```

## Patch format

`DesignPatch` uses the same JSON shape as the CLI `patch` command:

```json
{
  "operations": [
    { "type": "set_parameter", "id": "param:width", "expr": "100 mm" },
    {
      "type": "set_feature_expr",
      "feature_id": "feature:extrude_base",
      "field": "length_expr",
      "expr": "thickness * 2"
    },
    {
      "type": "set_feature_expr",
      "feature_id": "feature:fillet_top",
      "field": "radius_expr",
      "expr": "fillet_radius * 2"
    },
    {
      "type": "set_feature_expr",
      "feature_id": "feature:hole_row",
      "field": "spacing_expr",
      "expr": "hole_pitch"
    },
    {
      "type": "assign_face_ref",
      "ref_id": "ref:face:bracket_top",
      "kernel_face_id": 0,
      "created_by": "feature:extrude_base",
      "role": "top",
      "normal_m": [0.0, 0.0, 1.0]
    }
  ]
}
```

`assign_face_ref` adds or updates an entry in `semantic_refs.json`. When `kernel_face_id` is `0`, the OCCT backend resolves the face by `role` and `created_by` after regeneration. Semantic diffs report `topo_ref_added`, `topo_ref_removed`, or `topo_ref_modified` changes.

### `set_feature_expr` fields

| field | Feature type | Resolved field |
|---|---|---|
| `length_expr` | `extrude` | `extent.length` |
| `depth_expr` | `hole` | `depth` |
| `radius_expr` | `fillet` | `radius` |
| `distance_expr` | `chamfer` | `distance` |
| `spacing_expr` | `linear_pattern` | `spacing` |
| `thickness_expr` | `shell` | `thickness` |

### `set_feature_ref` fields

| field | Feature type | Patched field |
|---|---|---|
| `plane_face_ref` | `mirror_pattern` | mirror plane face ref |
| `face_ref` | `hole` | target face ref |

```json
{
  "operations": [
    {
      "type": "assign_face_ref",
      "ref_id": "ref:face:bracket_top",
      "created_by": "feature:extrude_base",
      "role": "top"
    },
    {
      "type": "set_feature_ref",
      "feature_id": "feature:pin_mirror",
      "field": "plane_face_ref",
      "ref_id": "ref:face:bracket_top"
    }
  ]
}
```

See `examples/agent/plane_face_ref_patch.json`.

### Structural operations (ADR-013)

Structural operations create or remove Design Graph objects instead of editing
existing values. MCAD-P7-001 delivers parameters, design assertions,
sketches, features, semantic references, and assembly and drawing structure, so
a complete part, assembly, or drawing can be authored from an empty document of
its kind.

| type | fields | effect |
|---|---|---|
| `add_parameter` | `id`, `name`, `expr` | Create a parameter; dependency edges are derived from `expr` |
| `remove_parameter` | `id` | Remove a parameter and its dependency edges |
| `add_assertion` | `assertion` (the `graph/assertions.json` entry shape) | Create a design assertion |
| `remove_assertion` | `id` | Remove a design assertion |
| `add_sketch` | `id`, `name`, `workplane` | Create an empty sketch |
| `remove_sketch` | `id` | Remove a sketch no sketch feature uses |
| `add_sketch_entity` | `sketch_id`, `entity` (the `graph/sketches.json` entity shape) | Add a point, line, circle, arc, or rectangle |
| `remove_sketch_entity` | `sketch_id`, `entity_id` | Remove an entity nothing references |
| `add_sketch_constraint` | `sketch_id`, `constraint` (the `graph/sketches.json` constraint shape) | Add a sketch constraint |
| `remove_sketch_constraint` | `sketch_id`, `constraint_id` | Remove a sketch constraint |
| `add_feature` | `node` (the `graph/features.json` node shape), `position` | Create a feature in the display order |
| `remove_feature` | `id` | Remove a feature nothing consumes |
| `move_feature` | `id`, `position` | Move a feature in the display order |
| `set_feature_suppressed` | `id`, `suppressed` | Suppress or unsuppress a feature |
| `replace_feature_definition` | `id`, `definition` | Replace a definition with one of the same feature type |
| `add_semantic_ref` | `topo_ref` (the `graph/semantic_refs.json` entry shape) | Create a semantic topology reference |
| `remove_semantic_ref` | `ref_id` | Remove a semantic reference nothing consumes |
| `add_component` / `remove_component` | `component` / `id` | Add an assembly component, or remove one no instance or pattern uses |
| `add_instance` / `remove_instance` | `instance` / `id` | Add a placed instance, or remove one no mate or connector references |
| `add_mate` / `remove_mate` | `mate` / `id` | Add or remove an assembly mate |
| `remove_connector` | `id` | Remove a connector no mate references by name (`add_connector` already existed) |
| `add_assembly_pattern` / `remove_assembly_pattern` | `pattern` / `id` | Add or remove an assembly pattern |
| `add_sheet` / `remove_sheet` | `sheet` / `id` | Add an empty sheet, or remove a sheet with the views and dimensions it owns |
| `add_drawing_view` / `remove_drawing_view` | `sheet_id`, `view` / `view_id` | Add a view, or remove one no dimension uses |
| `add_drawing_dimension` / `remove_drawing_dimension` | `sheet_id`, `dimension` / `id` | Add or remove a linear dimension |
| `add_attachment` / `remove_attachment` | `path`, `sha256`, `content_base64` / `path` | Add a STEP file under `imports/` (decoded bytes must match `sha256`), or remove one no imported solid uses |

An `imported_solid` feature places a STEP attachment as a fixed solid
([ADR-016](../adr/ADR-016-imported-step-solids.md)):

```json
{ "type": "imported_solid", "source": "imports/motor.step", "sha256": "<64 hex>",
  "transform": { "translation_m": [0.1, 0.0, 0.0], "rotation": [[1,0,0],[0,1,0],[0,0,1]] },
  "operation": "cut", "target_feature": "feature:plate" }
```

Regeneration verifies the attachment digest and fails closed on a mismatch.
The rotation must be proper and orthonormal within `1e-9`. `join` and `cut`
need `target_feature`. `opencad import-step <doc> <file.step> --id <feature:id>
[--operation new_body|join|cut] [--target <feature:id>] [--translate-mm x,y,z]`
builds and applies this patch; it reuses an existing identical attachment.

A `shell` feature hollows a body to a uniform inward wall and removes the
listed faces to form openings
([ADR-017](../adr/ADR-017-shell-feature.md)):

```json
{ "type": "shell", "target_feature": "feature:box",
  "thickness": { "value_si": 0.002 }, "thickness_expr": "wall",
  "open_face_refs": ["ref:face:box_top"] }
```

- The thickness must be greater than `1e-6 m`.
- At least one open face is required. Each must be a unique, existing
  `ref:face:` reference.
- Every open face is resolved on the target body itself. If one does not
  resolve, regeneration fails; there is no role fallback.
- A wall that does not fit the part fails regeneration.
- Known limitation: OCCT cannot shell a body whose open face is pierced by a
  through hole. Shell before cutting the hole.
- After a shell, regeneration reports each opened face reference as
  `Ambiguous` in `references:`. For example: `ref:face:box_top Ambiguous 2
  candidates tie`.
  - The cause: references are checked against the final body, where the
    opened face is gone and both the rim and the inner floor match its
    description.
  - The shell geometry is correct.
  - Do not put a `required_reference` assertion on an opened face; it
    would fail.

  See ADR-017 §4.

`examples/agent/add_shell_patch.json` authors a 60 × 40 × 20 mm enclosure
with 2 mm walls from an empty document.

Assembly and drawing objects use the stored JSON shapes of
`graph/assemblies.json` and `graph/drawings.json`, with IDs under the
`component:`, `instance:`, `mate:`, `pattern:`, `sheet:`, `view:`, and `dim:`
prefixes. Instances must name existing components, dimensions must stay on a
view of their own sheet, views need a finite scale above zero, and the
existing mate, connector, and pattern validators run on the final model. The
document layer also runs the document-level validators (component
self-reference, view sources). An assembly or drawing document without a model
yet is patched as an empty model of its kind.

`position` is `{"at": "start"}`, `{"at": "end"}`, `{"after": "<feature id>"}`,
or `{"before": "<feature id>"}`, resolved against the order at that point in
the patch.

Rules shared by every structural operation:

- **Author-chosen IDs.** The patch names every new object. IDs match
  `<prefix>:[a-z0-9_]+(.[a-z0-9_]+)*`, at most 128 bytes (`param:`,
  `assertion:`, `sketch:`, `ent:`, `con:`, `feature:`). Semantic reference IDs
  keep their existing `ref:` form. Rectangle `corner_ids` and
  `edge_ids` follow the same rule and share the sketch's entity namespace. An
  existing ID, or one removed earlier in the same patch, is rejected.
  Parameter names must be unique expression identifiers.
- **Final-state validation.** Operations apply to one staged candidate.
  Structural sketch, feature, and reference operations are staged first, in
  patch order, followed by value edits, so a value edit may target an object
  the same patch creates. References are checked once, on the final state, so
  a patch may add objects that refer to each other in any order, or remove an
  object together with its consumers.
- **No cascading removal.** Removing a parameter whose name is still used fails
  and lists every dependent in sorted order (`assertion …`, `feature …`,
  `parameter …`, `sketch …`). Added `parameter_range` and `required_reference`
  assertions must name an existing parameter or semantic reference.
- **Sketch integrity.** In every sketch the patch touches, line endpoints and
  circle/arc centers must name point entities (or rectangle corners),
  constraints must name existing entities, dimensions must keep their
  constraint, every coordinate and constraint expression must use existing
  parameters, and a `face_ref` workplane must name an existing semantic
  reference. A sketch still used by a sketch feature cannot be removed.
- **Profiles stay consumable.** Every extrude, hole, and revolve whose sketch
  was touched must still resolve its `profile_ref` to a closed profile, using
  the same lookup as regeneration. Removing an edge of an extruded outline is
  therefore rejected at dry-run, before any kernel call.
- **Derived sketch data.** A touched sketch's `profiles` are re-detected from
  its entities and its `solve_state` is reset to `unknown`; regeneration solves
  it again. Custom workplanes must be finite, with normal and x axis at least
  `1e-9` long and perpendicular within `|cos| <= 1e-6` (dimensionless).
- **Derived Feature Graph.** `graph/features.json` edges and entries are
  never authored. They are derived from each definition's
  `sketch_feature`/`source_feature`/`target_feature` inputs, plus the creator
  of every consumed semantic reference (including a consumed sketch's
  `face_ref` workplane) when that creator is not already upstream. A patch is
  rejected when an input names an unknown feature, when dependencies form a
  cycle, or when the display order places a feature before one of its inputs.
  The persisted graph is re-derived only when a patch adds, removes, moves,
  suppresses, or replaces a feature, or adds or removes a semantic reference;
  value edits never rewrite it.
- **Features stay regenerable.** Added or replaced features must reference an
  existing sketch, existing semantic references, and existing parameters, and
  every extrude, hole, and revolve must resolve its `profile_ref` to a closed
  profile. Removing a feature or semantic reference that anything still
  consumes fails and lists every consumer. Feature operations need the
  authored display order, so in-memory `opencad.patch_*` requests without it
  are rejected.
- **Feature values.** Every patch, including value edits, rejects feature
  values regeneration cannot use, evaluated the way regeneration will
  (expression first, stored value otherwise):
  - extrude lengths, hole depths, fillet radii, chamfer distances, and
    pattern spacings below `1e-9 m`;
  - pattern counts of `0`;
  - zero or non-finite axes, directions, and plane normals;
  - revolve angles outside `(0, 2π]`;
  - invalid imported-solid placements.

  For example, setting `thickness` to `0 mm` fails at dry-run and names every
  affected feature. Suppressed features are skipped.
- **Under-constrained sketches.** Dry-run adds a `sketch_under_constrained`
  warning, targeting the sketch ID, for a sketch that still has degrees of
  freedom after solving with the new parameter values. It checks only
  sketches the patch adds or edits, and sketches that use a parameter whose
  value changes.

  Warnings do not reject the patch. They flag geometry that can move or skew
  on a later edit: for example, the bracket's base sketch, which only its
  side lengths constrain, warns when `width` changes.
- **Limits.** A patch holds at most 10,000 operations.
- **Complete state required.** Structural operations run only through
  `build_patch_candidate` / `DesignPatch::apply_to_state` and the document,
  CLI, and Agent paths built on them. The legacy
  `DesignPatch::apply_to_document` rejects them.

```json
{
  "operations": [
    { "type": "add_parameter", "id": "param:rib_depth", "name": "rib_depth", "expr": "thickness * 2" },
    {
      "type": "add_assertion",
      "assertion": {
        "id": "assertion:rib_depth_range",
        "name": "Rib depth range",
        "severity": "required",
        "type": "parameter_range",
        "parameter_name": "rib_depth",
        "min_m": 0.004,
        "max_m": 0.02
      }
    }
  ]
}
```

See `examples/agent/add_rib_depth_patch.json`,
`examples/agent/add_boss_sketch_patch.json`, and
`examples/agent/add_top_fillet_feature_patch.json`. Parameter additions and
removals appear in semantic diffs as `parameter_changed` with an empty
`before` or `after`. Assertion changes appear as `assertion_added`,
`assertion_removed`, or `assertion_changed`, and in change impact as an
`assertion` input with no dirty Feature Graph nodes. Sketch changes appear as
`sketch_added`, `sketch_removed`, `sketch_entity_added`,
`sketch_entity_removed`, `sketch_constraint_added`, or
`sketch_constraint_removed`; change impact reports a `sketch` input and dirties
the consuming sketch feature and its downstream suffix. Feature additions,
removals, and replacements appear as `feature_added`, `feature_removed`, or
`feature_modified` and dirty the feature and its suffix in the derived
candidate graph; `feature_moved` reports a display-order change and dirties
nothing, because regeneration order depends only on dependencies. Assembly
component and pattern changes appear as `assembly_component_*` and
`assembly_pattern_*`, and drawing dimension changes as `drawing_dimension_*`.

### Rebase and semantic merge

`opencad rebase-patch` and `opencad_ai::rebase_patch` compare each target by
stable ID across the old base, the patch result, and the new base.
Structural conflicts carry a `reason`:

| `reason` | Meaning |
|---|---|
| `add_add` | Both sides created the same ID with different content |
| `remove_modify` | One side removed an object the other side changed |
| `anchor_missing` | A feature `position` anchor no longer exists in the new base |
| `order` | Both sides reordered features differently, or the merged order breaks a dependency |
| `invalid_result` | The combined result fails structural validation |

An addition that the new base already contains with identical content is
dropped from the rebased patch, and the rebased patch must still apply to the
new base.

`opencad merge` and `opencad_ai::semantic_three_way_merge` merge every
collection of the complete design state by stable ID: parameters, features,
sketches, semantic references, assertions, assembly components, instances,
mates, connectors, patterns, and drawing sheets. A side equal to base yields to
the other side's change, addition, or removal; retained objects keep base
order and additions follow in ID order. Feature display order keeps the
reordering side's relative order; features that both sides insert at the same
anchor follow it in order of their first feature ID. The merged result
therefore does not depend on which side is "ours". The combined state must
pass `opencad_ai::validate_design_state`. `opencad merge` writes back every
merged collection and re-derives `graph/features.json` only when features,
their order, sketches, or semantic references changed.

Rust callers can express an existing design as one structural patch with
`opencad_ai::authoring_patch(&DesignState)`. Applying it to an empty document
of the same kind rebuilds every checked-in example. Source data matches
exactly, and derived sketch profiles and solve state are recomputed.

The preconditions `sketch_exists` (`id`), `sketch_entity_exists`
(`sketch_id`, `entity_id`), and `assertion_exists` (`id`) guard patches that
depend on those objects. They need the complete design state, so they are
evaluated by `build_patch_candidate` and the paths built on it.

### Assembly patch operations

Assembly documents accept these additional operation types (alongside part patches):

| type | fields | effect |
|---|---|---|
| `set_instance_placement` | `instance_id`, `translation_m`, `rotation` | Update instance rigid transform |
| `set_mate_distance` | `mate_id`, `distance_m` | Update a distance mate target |
| `add_connector` | `id`, `name`, `instance_id`, `transform` | Add a named connector frame |

```json
{
  "operations": [
    {
      "type": "set_instance_placement",
      "instance_id": "instance:right",
      "translation_m": [0.12, 0.0, 0.0],
      "rotation": [[1,0,0],[0,1,0],[0,0,1]]
    },
    {
      "type": "set_mate_distance",
      "mate_id": "mate:spacing",
      "distance_m": 0.15
    }
  ]
}
```

Semantic diffs report `assembly_instance_*`, `assembly_mate_*`, and `assembly_connector_*` changes.

### Pattern features

Linear and circular patterns support `operation`: `union` (default) or `cut`. Cut patterns require `target_feature`. Union patterns may set `target_feature` to fuse copies onto an existing body.

```json
{
  "id": "feature:hole_row",
  "name": "Hole Row",
  "definition": {
    "type": "linear_pattern",
    "source_feature": "feature:hole_mount",
    "target_feature": "feature:extrude_base",
    "operation": "cut",
    "direction_m": [1.0, 0.0, 0.0],
    "spacing": { "type": "distance", "length": { "m": 0.02 } },
    "spacing_expr": "hole_pitch",
    "count": 3
  }
}
```

```json
{
  "id": "feature:boss_ring",
  "name": "Boss Ring",
  "definition": {
    "type": "circular_pattern",
    "source_feature": "feature:boss",
    "axis_origin_m": [0.04, 0.03, 0.0],
    "axis_direction_m": [0.0, 0.0, 1.0],
    "count": 4,
    "operation": "union"
  }
}
```

```json
{
  "id": "feature:boss_pair",
  "name": "Boss Pair",
  "definition": {
    "type": "mirror_pattern",
    "source_feature": "feature:boss",
    "plane_origin_m": [0.04, 0.03, 0.0],
    "plane_normal_m": [1.0, 0.0, 0.0],
    "operation": "union"
  }
}
```

Mirror patterns may also use `plane_face_ref` to derive the plane from a persisted face ref (see `bracket_pin_mirror()` in `opencad-feature`):

```json
{
  "id": "feature:pin_mirror",
  "name": "Pin Mirror",
  "definition": {
    "type": "mirror_pattern",
    "source_feature": "feature:pin_tool",
    "plane_face_ref": "ref:face:bracket_top",
    "target_feature": "feature:extrude_base",
    "operation": "union"
  }
}
```

Holes accept `face_ref` for semantic targeting; pass `semantic_refs` during regen so the face resolves from discovery data:

```json
{
  "id": "feature:hole_mount",
  "definition": {
    "type": "hole",
    "face_ref": "ref:face:bracket_top",
    "target_feature": "feature:extrude_base",
    "sketch_feature": "feature:sketch_hole",
    "profile_ref": "sketch:hole/profile:outer"
  }
}
```

Fillet and chamfer `face_ref` select the perimeter edges of the referenced
face, on any face (ADR-018). Before kernel IDs became deterministic
enumeration indices, only top faces worked.

`spacing_expr` is evaluated during regeneration (same timing as `length_expr` on extrude). Use `set_feature_expr` with `field: "spacing_expr"` to patch it parametrically.

## Linked plugin discovery and invocation

`opencad.plugin_list` accepts `{}` and returns manifests in stable lexical ID
order. `opencad.plugin_invoke` accepts `plugin_id`, document `path`, a typed
`request`, optional `dry_run`, optional exporter `output`, and optional opaque
backend `history`. Feature/importer results use the same validated
DesignPatch/history boundary as `opencad.patch_apply_document`; exporter output
is persisted by the host, not by plugin code.

Use [`plugin_list_request.json`](../../examples/agent/plugin_list_request.json)
and [`plugin_invoke_request.json`](../../examples/agent/plugin_invoke_request.json)
as transport examples. The invoke example is a dry run and assumes the caller
has copied `examples/bracket.ocad.d` to `work/bracket.ocad.d`.

## Errors

Standard JSON-RPC error codes:

| Code | Meaning |
|---|---|
| `-32700` | Parse error |
| `-32600` | Invalid request |
| `-32601` | Method not found |
| `-32602` | Invalid params |
| `-32000` | Application error (validation, regen, I/O) |

## Example

See `examples/agent/inspect_request.json`, `examples/agent/query_request.json`,
`examples/agent/plugin_list_request.json`,
`examples/agent/plugin_invoke_request.json`, and the other checked-in requests.
