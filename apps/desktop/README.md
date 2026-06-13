# ForgeCAD Desktop (Tauri)

Minimal desktop shell for previewing `.ocad.d` documents with OCCT regeneration and wgpu rendering.

## Prerequisites (Linux)

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev build-essential curl wget file \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Install the Tauri CLI once:

```bash
cargo install tauri-cli --version "^2.0.0"
```

## Run (dev)

From the repository root:

```bash
cd apps/desktop/src-tauri
cargo tauri dev
```

The app loads `examples/bracket.ocad.d` automatically when launched from the workspace.

## Features (MVP)

- Open `.ocad.d` directory
- Regenerate + PNG preview (sketch overlay included)
- Create built-in sample templates
- Document inspect panel (features, sketches, bounds)

## Architecture

| Layer | Crate / path |
|---|---|
| Preview API | `modules/desktop` (`opencad-desktop`) |
| Desktop shell | `apps/desktop/src-tauri` |
| Web UI | `apps/desktop/ui` |

The shared `opencad-desktop` crate is tested in CI; the Tauri shell is built locally.
