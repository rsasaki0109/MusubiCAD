# Semantic selection to reviewed dimension patch

MCAD-P14-003 implements the selection path specified by ADR-058. Click an
admitted face or authored sketch line in the Tauri preview/viewport, choose a
dimension in the **selected dimension** form, enter explicit units, Review and
Apply. The backend resolves source identity; frontend feature-name/role hints
cannot authorize a mutation. Selecting geometry no longer focuses an editable
parameter using those guesses. Tree editing and the explicit parameter pane
remain separate commands.

## Source and scene evidence

Part `DocumentPreview` and `PickSummary` now include a disposable
`selection_frame`: exact source revision, scene revision and admitted bindings.
The source revision is captured on the document used to prepare the scene, not
on a later file read. Scene revision SHA-256 covers source revision, actual
document identity, positions/normals/indices, overlay source bindings and admitted binding/capability/hit mapping with a versioned domain;
camera, style and disposable kernel IDs do not establish source ownership. The
frame is not persisted in `.ocad`. Selection mutation APIs accept an intent,
never a client-provided authoritative frame or provenance report.

Face targets use **declared** face semantic refs and current regeneration
provenance. Exact/Derived/Fingerprint must resolve uniquely to a current scene
face and its declared source owner. Missing, ambiguous, consumed, aliased,
suppressed or unsupported bindings refuse. A role-first match, guessed feature
name, triangle index or kernel handle cannot choose the source feature.

Authored sketch lines use exact stored sketch/entity IDs from overlay bindings.
The entity must belong to a resolved profile of exactly one active consumer
with an admitted dimension field. Multiple consumers refuse. Projected/generated
coordinates and rectangle-generated helper lines absent from authored source
are not editable source. Missing source/profile/owner or unsupported consumer
forms refuse rather than choosing a convenient feature.

## Transports

| Query/command | Agent method | MCP tool |
|---|---|---|
| Native source/scene binding query | `opencad.inspect_selection_frame` | `inspect_selection_frame` |
| Pixel pick (`PickSummary`) | `opencad.pick_document` | `pick_document` |
| `plan_selection_edit_cmd` | `opencad.plan_selection_edit` | `plan_selection_edit` |
| `apply_selection_edit_cmd` | `opencad.apply_selection_edit` | `apply_selection_edit` |

`inspect_selection_frame` takes `{path}` and rebuilds/tessellates the native part
without GPU rasterization or writes. `pick_document` also provides the current
binding for an actual picked hit. The GPU-free, styled CLI/MCP `preview_document`
is a separate image API; its image is not a selection authorization token.

Plan/apply take `{path, edit}`, with optional opaque `history` on apply:

```json
{
  "source_revision":"<current frame source revision>",
  "scene_revision":"<current frame scene revision>",
  "target":{"kind":"face_reference","ref_id":"ref:face:ellipse_top"},
  "field":"length_expr",
  "expr":"10 mm"
}
```

An authored overlay target is
`{"kind":"sketch_entity","sketch_id":"sketch:base","entity_id":"ent:line"}`.
Unknown edit/target fields refuse. Plan returns the concrete revision-guarded
DesignPatch and candidate evidence. Apply re-resolves current native source and
scene, verifies the exact owner/field, and uses the existing verified feature
gateway for regeneration, required assertions, display tessellation, history
and atomic file publication. Return values match the feature editor contract.
Old source or scene revisions refuse before publication; stale asynchronous
pick/review results cannot replace newer UI state. Failures retain source,
opaque history and published outputs. No-op retains existing no-op semantics.

## Scope, example and evidence

The initial selection form edits the six length fields already admitted by
[feature-editor.md](feature-editor.md). It changes the feature expression, not
an inferred named parameter. The review states the actual feature/field/effect.
No face deletion, arbitrary direct modeling, edge/vertex editing, assembly
selection editing or interactive sketch manipulation is introduced.
Authored line/circle/arc overlay bindings are available where the existing
renderer supports them. Ellipse/cubic overlay strokes remain unimplemented;
declared native faces support the ellipse example. This adds no sampler,
NURBS edit, kernel feature, dependency or source schema.

`examples/agent/semantic_selection_edit.json` prepares an ellipse top-face ref
through verified DesignPatch, then changes only its extrusion expression to
10 mm. With axes 30/20 mm and unchanged 30° rotation, volume is
`π * 30 * 20 * 10 = 18849.555922 mm³`. Sketch/entity/profile/ref IDs and original
parameter expressions (including thickness 6 mm) remain intact.

Source binding/plan tests compare analytic volume in Mock and OCCT at
`1e-12 m³`, stale/missing/ambiguous/consumed/alias refusal and source/history
retention. Mock ellipse rendering/topology remain refused; its source-only frame
fixture proves neutral binding plus exact geometry verification, not a fake
render. Production transports reconstruct actual native frame evidence.
Native scene hashes are tested for repeatability. Real MCP stdio compares
Tauri helpers with MCP plans/results and source/history. Real Linux Tauri
pointer selection, review/apply, wrong name hints, delayed response refusal and
undo/redo passed via `tools/test_tauri_selection_edit.mjs`; the saved part also
passes fresh native regeneration and the independent source/volume eval grader.
This is actual Tauri WebView evidence, not a browser/IPC harness or paid host run.

The feature gateway's intentional limits remain: nonempty physical BodyCount
is unavailable and required assertions refuse; the file writer plus revision
recheck does not add cross-process locking; GPU image display follows verified
geometry/mesh publication. External Windows/Codex/Cursor/Gemini host validation
remains separate ecosystem work.
