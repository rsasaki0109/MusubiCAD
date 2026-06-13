# Desktop UI

ForgeCAD desktop preview uses **Tauri 2** for the shell and **`opencad-desktop`** for regeneration + PNG preview.

## Quick start

See [apps/desktop/README.md](../../apps/desktop/README.md).

## Shared API (`opencad-desktop`)

| Function | Purpose |
|---|---|
| `preview_document(path)` | Regenerate, tessellate, render PNG (base64) |
| `inspect_document(path)` | Document metadata summary |
| `create_document(path, template)` | Built-in sample templates |
| `load_view_data(path)` | Scene + sketch overlay for advanced viewers |

CLI commands (`opencad mesh`, `opencad new`, `opencad export`) reuse the same crate.
