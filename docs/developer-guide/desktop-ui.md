# Desktop UI

MusubiCAD desktop preview uses **Tauri 2** for the shell and **`opencad-desktop`** for regeneration + PNG preview.

## Quick start

See [apps/desktop/README.md](../../apps/desktop/README.md) and the
[desktop backend/command parity API](../api/desktop.md).

## Shared API (`opencad-desktop`)

| Function | Purpose |
|---|---|
| `preview_document(path)` | Regenerate, tessellate, render PNG (base64) |
| `inspect_document(path)` | Document metadata summary |
| `regenerate_document(path)` | Read-only OCCT regeneration report for a part |
| `list_document_parameters(path)` | Parameter expressions + evaluated mm values |
| `set_document_parameter(path, id, expr)` | Compatibility update through a validated DesignPatch (without retained history) |
| `set_document_parameter_with_history(path, id, expr, history?)` | Update one parameter and return opaque backend history state |
| `undo_document_with_history(path, history)` | Restore the validated previous full-document snapshot |
| `redo_document_with_history(path, history)` | Restore the validated next full-document snapshot |
| `pick_document(path, options)` | Headless GPU pick at preview pixel coordinates |
| `export_stl_document(path, output)` | Regenerate and export a part as binary STL |
| `create_document(path, template)` | Built-in sample templates |
| `load_view_data(path)` | Scene + sketch overlay for advanced viewers |

CLI commands (`musubicad mesh`, `musubicad new`, and `musubicad params`) reuse the
same crate. The shared regeneration/export helpers are also used by the
headless `run_desktop_smoke` contract; they are intentionally not exposed as
Tauri UI commands. The complete UI/Tauri/CLI/Agent mapping and headless smoke
command are documented in [api/desktop.md](../api/desktop.md).

The Tauri/UI toolbar passes the returned `history` value back to the backend
without interpreting snapshot semantics. It uses only `can_undo` and
`can_redo` to enable controls; no browser-local inverse stack is maintained.

## Actual feature editor interaction test

P17-002 extends both the Linux script below and the Windows native editor
script with `tools/sketch_creation_ui_checks.mjs`. On the disposable cubic
fixture, the real forms review/cancel and discard a late IPC result, reject a
foreign document identity and owner collision without changing source/history/
preview, create/select each global-plane sketch, check canonical Undo/Redo and
create in a sketch-only state with null physical evidence. These are additional
native interactions, separate from the actual MCP protocol/analytic fixture.

The part feature editor contract and intentional scope are in
[feature-editor.md](../api/feature-editor.md). Linux Tauri interaction evidence is
provided by `tools/test_tauri_feature_editor.mjs`, using Node's built-in WebSocket
and the **actual** WebKit WebView inspector. It is not a browser/IPC harness.

Build the locked Tauri crate with GTK/WebKit development libraries. Copy
`examples/cubic_plate.ocad.d` to a disposable working directory under
`examples/bracket.ocad.d` (the startup sample path). Start Xvfb or a local desktop,
and launch the app there with `WEBKIT_INSPECTOR_HTTP_SERVER=127.0.0.1:9223`.
Then run:

```sh
node tools/test_tauri_feature_editor.mjs http://127.0.0.1:9223 /tmp/editor/examples/bracket.ocad.d
```

On the managed cloud setup, source `/workspace/.musubicad-setup/gui-env.sh` and
map the workspace WebKit helper directory to its compiled `/usr/lib` path using
PRoot; the environment's saved startup instructions provide the exact command.
No system library or repository source is modified by this runtime override.
Local X/inspector sockets and PRoot require suitable tool execution permission.
The test drives product create/edit/reorder/suppress forms, checks the 2400 mm³
analytic preview, verifies review has no writes, exercises undo/redo, and checks
byte-identical source after invalid dependency/unit refusal. Undo/redo compares
canonical JSON content: the existing opaque-history serde transport can reorder
object members and therefore update checksums without changing source identity.
Stop the test app and inspector afterward. This is local Linux evidence, not
Windows or external agent-host verification; no paid evaluation is performed.

## Actual semantic selection interaction test

Prepare a disposable ellipse_plate copy under `examples/bracket.ocad.d` using the verified `semantic_ref_patch` in `examples/agent/semantic_selection_edit.json`, then launch the actual Tauri app there with the same Xvfb/PRoot/loopback inspector setup. Run `node tools/test_tauri_selection_edit.mjs <inspector-http-endpoint> <disposable-part-path>`. The test dispatches a pointer click on the displayed image, exercises the selected-dimension product form and backend, checks analytic 18849.555922 mm³, preview/no-write, wrong name hints, delayed response refusal and canonical undo/redo. It uses the actual complete pick response; it does not fabricate a model or IPC harness. The resulting source is independently graded by `semantic_selection_edit`. Stop the test app/inspector afterward; external host checks remain separate.

Interactive sketch forms (P14-004): `inspect_sketches`, `plan_sketch_edit`,
`apply_sketch_edit` share Tauri/Agent/MCP backend solve, DesignPatch verification
and atomic history publication. Use explicit units, stable authored IDs and the
current revision; review never writes. Authored global sketches and the
ADR-102 admitted datum-plane sketches are supported; datum edits require
current document identity and a regenerated display frame;
unsupported/ambiguous/stale/failed-solve candidates refuse. See
`docs/api/sketch-editing.md` and `examples/agent/interactive_sketch_edit.json`.

P17-003's shared `tools/datum_sketch_ui_checks.mjs` runs from both actual OS
native editor scripts, after the global checks. It copies the canonical datum
plate into the disposable fixture, loads it through the actual backend and uses
product controls to review/cancel/create a source-bound datum sketch, author a
local point/ellipse, create its physical prism, change the upstream offset,
refuse a stale real command and restore parameter/source association with
Undo/Redo. It fakes no IPC/backend result or frontend model. The helper path is
part of Desktop PR triggers and the release-policy contract; the required
Windows gate executes these newly introduced interactions, separate from Linux,
protocol or external-host evidence. See the API's analytic and mesh tolerances.

Actual Linux Tauri sketch evidence (disposable ellipse fixture, Inspector runtime
as documented above):
```text
node tools/test_tauri_sketch_editor.mjs http://127.0.0.1:9223 /tmp/editor/examples/bracket.ocad.d
```
This drives the real WebView and backend, not an IPC replacement harness.


## Actual Windows WebView2 editing checks

P15-005 runs `tools/test_windows_desktop_ui.ps1 -BinaryPath <native-exe>` on the
Windows desktop CI target with Node 24. The wrapper copies the cubic-plate
fixture to a disposable directory, launches the actual native app and points
WebView2's profile and loopback debugger at the test process only.
`tools/test_windows_tauri_editor.mjs` connects to the actual `tauri.localhost`
WebView2 page, checks real Tauri IPC presence and operates existing product
forms/buttons. It reviews and applies create/length/reorder/suppress edits,
checks an independent 2400 mm³ preview, verifies opaque Undo/Redo source identity
and confirms dependency/unit refusals preserve source/checksum bytes. It does
not install browser mocks or replace the backend invoke function. The original
fixture is hash-checked and the owned app/profile is removed afterwards.

Windows is now included alongside Linux in desktop PR builds so the test must
pass before merge; main/tag/manual builds still cover all four platforms.
Inspector arguments are runtime-only test configuration; production configuration
and unsigned desktop trust remain unchanged. No manual user session, installation
wizard, code signing, Cursor or authenticated Gemini session is inferred from
this automated native GUI check. Record the exact CI/head before claiming
Windows evidence. The cloud Linux environment cannot execute this Windows test.

## Actual pointer circle checks (P17-004)

Both native editor scripts unconditionally run
`tools/pointer_circle_ui_checks.mjs` after their existing creation/datum checks.
The helper loads the canonical datum fixture into the disposable copy, creates
a sketch through product controls and dispatches coordinate-bearing click/move
events in the actual WebView. It does not call the product radius/draft helper,
replace IPC, fake backend results or claim a physical mouse session. The
prescribed centre (2,-3) mm and radius point (5,1) mm define an independent
3-4-5 triangle. The test aligns only the SVG viewport origin to CSS pixels at
the product's 120 mm span: WebKit truncates mouse coordinates to integer pixels.
No event coordinate property, draft helper or model result is replaced.
Checks cover moving preview, half-review refusal, Escape/Discard,
canceling a real pending IPC, zero radius, actual view/tool/sketch changes and
reload, construction flags, free IDs, one-entry Undo/Redo, a native 150*pi mm³
prism and upstream-parameter/stale-command refusal. The helper path is enforced
by Desktop PR triggers and the policy test. Successful Linux execution does not
establish Windows completion; record its exact-head required native step too.

## Actual pointer rectangle checks (P17-005)

Both native editor scripts also unconditionally run
[the rectangle helper](../../tools/pointer_rectangle_ui_checks.mjs). It creates
a source-bound datum sketch through real controls, dispatches opposite-corner
events and checks the independent 20x10 mm preview plus a native 1200 mm³ prism.
The same integer-pixel presentation setup is used; input properties, product
helpers, IPC responses and model results are never replaced. Checks cover
half-review refusal, Escape/Discard and late real IPC cancellation, zero width/
height, view/tool/sketch/reload invalidation, construction-only source, fresh
derived IDs, one-entry eight-entity Undo/Redo and upstream/stale-command refusal.
The stored geometry is four ordinary authored points and four lines, without
a Rectangle helper or implied rectangle constraints. Linux execution and the
required exact-head Windows WebView2 step remain separate evidence.
