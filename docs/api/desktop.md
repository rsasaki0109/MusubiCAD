# Desktop backend and command parity

`opencad-desktop` is the backend boundary used by the Tauri shell and by
headless desktop tests. It reads the Design Graph from `.ocad`/`.ocad.d`,
regenerates disposable B-Rep/mesh data, and returns serializable summaries.
The web UI does not own or mutate model structures; it invokes a Tauri command
for every document operation. Viewport camera and selection state remain in the
render/viewport layer and are not written to the document.

## Shared backend API

P17-002/003 add the shared [global/datum sketch creation API](sketch-creation.md):
`inspect_document_sketch_creation`, `plan_document_sketch_create` and
`apply_document_sketch_create` (in `sketch_creation`). Tauri delegates through
`inspect_sketch_creation_cmd`, `plan_sketch_create_cmd`, `apply_sketch_create_cmd`;
Agent routes are `opencad.inspect_sketch_creation`, `opencad.plan_sketch_create`,
`opencad.apply_sketch_create`, exposed as identically named MCP tools without
the prefix. No frontend graph mutation or persisted schema is introduced.
P17-003 lists source-bound datum choices with regenerated SI frames; creation
selects exactly one global/datum identity. Datum local editing requires current
document identity/revision, retains the source association and tracks upstream
offset edits. Returned display frames never become mutation authority.

Phase 14 follows [ADR-058](../adr/ADR-058-backend-only-desktop-editing.md) and
[desktop editing ownership](../architecture/desktop-editing.md). P14-001 is
architecture-only; feature-tree/selection/sketch editors remain planned.
New editor commands must use an authoritative verified DesignPatch gateway,
admit explicit units and source/scene revisions, regenerate a candidate and
publish source/history/scene atomically, with CLI or Agent parity. Existing
parameter/history file helpers validate/persist source; their geometry-before-
publication gap is an explicit P14-002 implementation requirement. No new
runtime command or `.ocad` field is introduced by this ADR.

| Function | Contract |
|---|---|
| `inspect_document(path)` | Open and summarize document metadata, graph counts, and semantic references |
| `preview_document(path)` | Regenerate, tessellate, and render a deterministic offscreen PNG preview |
| `regenerate_document(path)` | Read-only OCCT regeneration report and triangle count for a part document |
| `inspect_document_regeneration(path)` | Read-only `RegenInspectionResult`: where a part stops regenerating, which features the failure blocks, and the body it was building on (MCAD-P6-006) |
| `inspect_parameter_intent(path, id)` | `ParameterIntent`: what drives a parameter and which features an edit to it regenerates |
| `inspect_reference_intent(path, ref_id)` | `ReferenceIntent`: a semantic reference's creator, consumers, and the features a change to it regenerates |
| `apply_patch_and_regenerate_with_trace(doc, patch, kernel, registry)` | Atomically apply a patch and return shared `ChangeImpact` plus `RegenerationTrace`; no-op performs zero kernel calls |
| `list_document_parameters(path)` | Return parameters in deterministic evaluation order with explicit unit values |
| `set_document_parameter(path, id, expr)` | Compatibility wrapper: apply one validated `DesignPatch` and persist without retaining history |
| `set_document_parameter_with_history(path, id, expr, history?)` | Apply one validated parameter patch and return an opaque `DocumentHistoryState` |
| `undo_document_with_history(path, history)` | Validate and persist the previous full-document snapshot; return updated history state |
| `redo_document_with_history(path, history)` | Validate and persist the next full-document snapshot; return updated history state |
| `pick_document(path, options)` | Headless offscreen selection query with semantic/topological context |
| `load_assembly_evidence_from_document(path, doc)` | Regenerate an assembly once and return `AssemblyEvidence { scene, report, interference_count }`: the viewport scene, the `AssemblyRegenReport` (DOF, mass, bounds, per-instance status), and the exact interference count under the default tolerances; `load_assembly_scene_from_document` returns only the scene and count (MCAD-P10-001); `load_assembly_evidence_with_assertions(path, doc, assertions)` also sweeps joint motion for the motion assertions in `assertions` (MCAD-P10-002) |
| `export_stl_document(path, output)` | Regenerate a part and write a disposable binary STL |
| `run_desktop_smoke(source, work_dir)` | Copy a part fixture into a new work directory and return serializable open/preview/edit/regenerate/pick/export evidence |

The regeneration and export result types contain only serializable strings,
counts, and paths. They do not expose OCCT handles or cached geometry
ownership. Assembly and drawing export continue to use their specialized
CLI/Agent API paths until the corresponding Phase 5 backend contracts are
completed. `regenerate_document` and `export_stl_document` are shared backend
functions, not Tauri/UI commands.

`DocumentRegeneration.trace` is the same DTO returned by CLI/Agent regeneration.
The richer patch helper returns `PatchRegenerationResult { impact,
regeneration }`; the compatibility helper retains its `RegenReport` return.

## Tauri/UI/CLI/Agent parity

| UI action | Tauri command | CLI route | Agent API route |
|---|---|---|---|
| Open | `inspect_document_cmd` + `preview_document_cmd` | `inspect`, `screenshot` | `opencad.inspect` for document metadata |
| Preview | `preview_document_cmd` | `screenshot` | — (CLI provides parity) |
| Refresh | `inspect_document_cmd` + `preview_document_cmd` | `inspect`, `screenshot` | — (CLI provides parity) |
| List parameters | `list_document_parameters_cmd` | `params [--json]` | `opencad.query_document` / document inspection |
| Edit parameter | `set_document_parameter_cmd` | `patch` with `set_parameter` | `opencad.patch_apply_document` |
| Undo document edit | `undo_document_cmd` | — | `opencad.history_undo_document` |
| Redo document edit | `redo_document_cmd` | — | `opencad.history_redo_document` |
| Pick | `pick_document_cmd` | `pick` | `opencad.pick_document` |
| Regeneration panel (on Open/Refresh) | `inspect_regeneration_cmd` | `intent <path> regen` | `opencad.inspect_regeneration_document` |
| Parameter Intent button | `inspect_parameter_intent_cmd` | `intent <path> param:...` | `opencad.query_document` (`inspect_parameter`) |
| Pick a face with a topo ref | `inspect_reference_intent_cmd` | `intent <path> ref:...` | `opencad.query_document` (`inspect_reference`) |
| Create sample | `create_template_document` | `new` | — (CLI provides parity) |
| Open viewport | `open_viewport_cmd` | `view` | — (CLI provides parity) |

The command parity integration test includes the UI and Tauri command surfaces
as compile-time fixtures and checks the actual `generate_handler!` registration.
It fails if a document/viewport model operation loses its Tauri handler or its
CLI/Agent route. The `default_example_path` call is
intentionally classified as a shell-only bootstrap path resolver, not a model
command. Refresh deliberately calls the same inspect/preview load path as Open;
there is no separate UI regenerate or export command. The test also checks that
the UI keeps document path state separate from viewport preview synchronization
and does not contain direct Design Graph mutation expressions.

### Intent Inspector

The info panel's Regeneration and Intent sections are the desktop Intent
Inspector (MCAD-P6-006). They call the same backend functions as
`musubicad intent` and the Agent API, and none of them writes the document.
Open and Refresh load the document summary, preview, parameters, and
regeneration inspection independently. A document that no longer regenerates
still shows its parameters and the Regeneration section (failing node, error,
blocked features, and the upstream body's mass) without a preview, so it can
be repaired by editing a parameter. Each parameter row's Intent button shows
what drives it and which features an edit regenerates. Picking a face whose
topo ref is inferred shows that reference's creator and consumers.

The parity test also checks the implementation boundary: the Tauri parameter
handler delegates to the desktop history helper, the desktop helper constructs
the same validated `DesignPatch` used by the CLI/Agent routes, and template
creation is shared with `musubicad new`. A runtime case applies the toolbar
parameter command and a direct `DesignPatch` transaction to equivalent
documents and compares their serialized results.

Parameter edits cross the same validated `DesignPatch`/file boundary as other
backend changes. The Tauri commands return a serializable
`DocumentHistoryState`; the UI passes its `history` field back opaquely to
parameter, undo, and redo commands and uses only `can_undo`/`can_redo` for
button state. `DocumentHistory` stores deterministic complete document
snapshots and descriptions outside the `.ocad` schema. It excludes viewport,
camera, selection, B-Rep, and mesh state. A failed patch or stale undo/redo
check leaves both the document and the caller's history value unchanged.

## Headless smoke test

Run the real checked-in example through the backend on a machine with OCCT and
a headless Vulkan adapter:

```powershell
./tools/desktop-smoke.ps1
```

The test calls `run_desktop_smoke(source, work_dir)`, which copies
`examples/bracket.ocad.d` to a new temporary directory and returns a JSON-safe
summary after verifying open/validate, PNG preview, a `100 mm` width edit, OCCT
regeneration, offscreen pick/highlight, and binary STL export. It accepts only
`DocumentKind::Part`, rejects an existing work directory, validates the edited
copy, and compares every source fixture file as a `BTreeMap<String, Vec<u8>>`
so the committed example remains byte-for-byte unchanged.

The packaged Tauri binary exposes the same check without opening a GUI:

```text
musubicad-desktop --version
musubicad-desktop --smoke-test <source.ocad.d> <new-work-dir.ocad.d>
```

The Linux workflow downloads its AppImage and runs this mode with
`APPIMAGE_EXTRACT_AND_RUN=1` under Mesa Vulkan before accepting the artifact.

## Part feature editor (MCAD-P14-002)

See [feature-editor.md](feature-editor.md) for admitted create/edit/reorder/suppress forms, shared Tauri/Agent/MCP commands, explicit-unit review/apply, verified parameter/history publication and intentional part-only limitations.

## Selection to reviewed dimension patch (MCAD-P14-003)

Part previews/picks carry backend source/scene revisions and uniquely admitted source bindings. `plan_selection_edit_cmd` and `apply_selection_edit_cmd` rebind declared face refs or authored overlay IDs before the verified feature gateway. Read [selection-editing.md](selection-editing.md); name/role guesses remain display hints and do not authorize mutation.

Interactive sketch forms (P14-004): `inspect_sketches`, `plan_sketch_edit`,
`apply_sketch_edit` share Tauri/Agent/MCP backend solve, DesignPatch verification
and atomic history publication. Use explicit units, stable authored IDs and the
current revision; review never writes. Global and ADR-102 admitted datum sketches
are supported, with separate source associations and regenerated frames;
unsupported/ambiguous/stale/failed-solve candidates refuse. See
[sketch editing](sketch-editing.md) and `examples/agent/interactive_sketch_edit.json`.

## Pointer circle drafting (MCAD-P17-004)

Choose Draw circle, click its centre and then a radius point in local mm. Pointer
motion displays a disposable circle; Review solves/verifies and Apply commits
one typed point/circle batch. A half gesture cannot submit the numeric form.
Escape/Discard and tool/sketch/view/file/upstream changes cancel drafts/reviews;
late IPC replies cannot revive them. Existing source IDs are avoided when
drafting, and the backend rejects collisions or stale intent without an orphan
point. Construction circles use the same checkbox and gateway. See
[sketch editing](sketch-editing.md#pointer-circle-intent-mcad-p17-004).
