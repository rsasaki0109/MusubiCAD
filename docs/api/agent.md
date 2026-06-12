# Agent API

ForgeCAD exposes a JSON-RPC 2.0 API for AI agents and automation tools.

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
| `opencad.patch_dry_run` | `{ parameters, feature_nodes, semantic_refs?, patch }` | `{ validation, diff }` |
| `opencad.patch_apply` | `{ parameters, feature_nodes, semantic_refs?, patch }` | `{ parameters, feature_nodes, semantic_refs, diff }` |
| `opencad.diff` | `{ before, after }` (each may include `semantic_refs`) | `DesignDiff` |
| `opencad.regen` | `{ parameters, sketches, feature_graph, feature_nodes }` | `RegenResult` |
| `opencad.query` | `{ parameters, feature_nodes, feature_graph?, query }` | `QueryResult` |
| `opencad.explain` | `{ parameters, feature_nodes, feature_graph?, sketch_count?, document_name? }` | `DesignExplanation` |

### Document (`.ocad` / `.ocad.d`)

| Method | Params | Result |
|---|---|---|
| `opencad.inspect` | `{ path }` | document summary |
| `opencad.validate` | `{ path }` | `{ valid, path }` |
| `opencad.patch_dry_run_document` | `{ path, patch }` | `{ validation, diff }` |
| `opencad.patch_apply_document` | `{ path, patch }` | `{ patched }` |
| `opencad.regen_document` | `{ path }` | `RegenResult` |
| `opencad.export` | `{ path, output }` | `ExportSummary` |
| `opencad.diff_document` | `{ before, after? \| patch?, geometry? }` | `DesignDiff` |
| `opencad.query_document` | `{ path, query }` | `QueryResult` |
| `opencad.pick_document` | `{ path, x, y, width?, height? }` | `PickSummary` |
| `opencad.explain_document` | `{ path }` | `DesignExplanation` |

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

### `RegenResult`

```json
{
  "kernel": "OCCT 8.0.0 (cadrum static)",
  "regenerated": ["feature:sketch_base", "feature:extrude_base"],
  "skipped_suppressed": [],
  "volume_m3": 2.833178323652379e-5,
  "mass_kg": 0.07649581473861423,
  "density_kg_per_m3": 2700.0
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

### Pattern features

Linear and circular patterns support `operation`: `union` (default) or `cut`. Cut patterns require `target_feature`.

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

`spacing_expr` is evaluated during regeneration (same timing as `length_expr` on extrude). Use `set_feature_expr` with `field: "spacing_expr"` to patch it parametrically.

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

See `examples/agent/inspect_request.json`, `examples/agent/query_request.json`, `examples/agent/query_sketch_constraints_request.json`, `examples/agent/query_overlay_lines_request.json`, `examples/agent/query_face_groups_request.json`, `examples/agent/query_semantic_ref_request.json`, `examples/agent/pick_document_request.json`, `examples/agent/explain_request.json`, `examples/agent/export_request.json`, `examples/agent/diff_document_request.json`, `examples/agent/assign_face_ref_patch.json`, and `examples/agent/spacing_expr_patch.json`.
