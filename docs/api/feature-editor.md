# Verified feature-tree editor

MCAD-P14-002 implements the first part editor under ADR-058. Open a **copy** of
`examples/cubic_plate.ocad.d` in the Tauri desktop. Feature rows show source IDs,
names, suppression and admitted dimension expressions. The creation form selects
an existing authored closed sketch profile and creates a NewBody distance
extrusion. Enter explicit units, for example `10 mm`, or length parameter names.
Review dimension, creation, move or suppression, inspect the exact requested
change and resulting volume, then Apply. Cancel and preview do not write source.
Undo/redo share opaque backend history with parameter edits.

## Shared transports

| Tauri command | Agent method | MCP tool |
|---|---|---|
| `list_document_features_cmd` | `opencad.list_features` | `list_features` |
| `plan_feature_edit_cmd` | `opencad.plan_feature_edit` | `plan_feature_edit` |
| `apply_feature_edit_cmd` | `opencad.apply_feature_edit` | `apply_feature_edit` |

Listing takes `{path}` and returns `{revision, rows}`. Plan/apply take
`{path, revision, edit}`; apply optionally accepts `history` and returns
`{tree, history, evidence}`. `history.history` is the opaque value for the next
command; clients consume only `can_undo`/`can_redo`. Do not modify snapshots.

`edit` supports these bounded forms, with unknown fields refused:

```json
{"operation":"create_extrude","id":"feature:second","name":"Second prism","sketch_feature":"feature:cubic_sketch","profile_ref":"profile:bezier:ent:cubic","length_expr":"12 mm"}
```

```json
{"operation":"set_expression","id":"feature:second","field":"length_expr","expr":"10 mm"}
```

```json
{"operation":"move","id":"feature:second","position":{"before":"feature:cubic_body"}}
```

```json
{"operation":"suppress","id":"feature:cubic_body","suppressed":true}
```

Query the revision before **each** operation. Review the returned concrete
DesignPatch and evidence; apply submits the admitted form intent and revision,
which the backend replans and verifies. Stale revisions refuse. SI evidence has
`volume_m3`, `bounds_size_m`, regenerated feature IDs and `no_op`. No-op preserves
history and skips kernel execution; missing metrics then mean unexecuted, not
an empty result. New candidates regenerate, evaluate required assertions and
prove native tessellation before publication. Source/history remain unchanged
on dependency, units, geometry, assertion or file errors. Files use the existing
atomic writer, with a revision recheck before publication; no cross-process lock
or network collaboration is introduced.

## Scope and limitations

- Creation: existing **authored** closed sketch, normal positive Distance
  extrusion, NewBody only. Stable profile IDs come from backend discovery.
- Editing: distance extrusion length; distance hole/standard-hole depth; fillet
  radius; chamfer distance; linear pattern spacing; shell thickness. Fields with
  stored numbers show an explicit mm expression. Other definitions remain visible
  but do not expose inferred editing forms. No delete/rename/general feature
  definition editor or sketch drawing is included.
- Moves use authored order; dependency admission and the existing deterministic
  regeneration traversal still determine execution. Suppressing any input of an
  active consumer refuses, including sketch/workplane/semantic graph edges.
- Empty part results are admitted if assertions permit; their volume/bounds are
  absent. A body handle may represent a compound: physical solid count is not
  available from the shared kernel interface, so required BodyCount assertions
  on nonempty editor candidates refuse rather than assuming one solid.
- Parameter/history desktop commands now verify part regeneration and display
  mesh before writes. Assembly editing belongs to its separate backend; this
  gateway explicitly accepts parts only. GPU image rasterization happens after
  geometry/mesh verification and remains a separate display operation.
- Mock provides exact cubic analytic volume/bounds but refuses cubic rendering;
  production publication uses OCCT. No `.ocad` schema, migration, dependency or
  kernel feature is added. External Windows/Codex/Cursor/Gemini host checks remain
  separate ecosystem work.

## Evidence

`examples/agent/feature_tree_edit.json` gives a reproducible four-operation
workflow. With a=b=40 mm, the closed cubic area is `3ab/20 = 240 mm²` and the
10 mm prism has volume **2400 mm³**. Profile extrema give XY spans
`4a/9 = 17.777777778 mm`; Z is 10 mm. Tests compare volume at `1e-12 m³` and
conservative native bounding boxes at `3e-7 m` (OCCT includes 0.1 µm shape
tolerance at each bound).

Backend tests exercise both kernels, preview/no-op/stale refusal, dependency
failure, byte-identical file/history/preview preservation after a staged invalid
control edit, and undo/redo. The real MCP stdio test calls the same helpers used
by Tauri and compares source/history after every operation. The MCP eval task
`feature_tree_edit` independently grades source order, suppression, exact profile
identity/expression and analytic volume; no paid host run is implied.

Actual Linux Tauri interactions passed using Xvfb, unmodified WebKit/Tauri and
PRoot mapping of the workspace helper path. The checked-in
`tools/test_tauri_feature_editor.mjs` drives the real WebView controls via its
inspector and verifies review/no-write, create/edit/reorder/suppress, 2400 mm³,
undo/redo canonical source identity and dependency/unit refusal. It is not an
IPC harness. The resulting saved document also passed the independent eval
grader against fresh native regeneration. See the
[interaction test instructions](../developer-guide/desktop-ui.md).
