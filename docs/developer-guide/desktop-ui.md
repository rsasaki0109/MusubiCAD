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
