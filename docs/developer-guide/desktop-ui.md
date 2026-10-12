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
