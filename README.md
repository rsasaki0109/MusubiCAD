# ForgeCAD

AI-native, open-source, parametric 3D CAD engine.

ForgeCAD treats the **Design Graph** as the source of truth — not the GUI and not a cached B-Rep shape. Human operators, AI agents, and CI pipelines all work against the same deterministic, Git-friendly design data.

> **Note:** The CLI binary and Rust crates still use the `opencad` prefix (`opencad agent`, `opencad-cli`, etc.) while the project is branded ForgeCAD.

**Repository:** [github.com/rsasaki0109/ForgeCAD](https://github.com/rsasaki0109/ForgeCAD)

## Vision

- Operate like SOLIDWORKS for humans
- Editable by AI agents via semantic patches
- Testable, reviewable design data in `.ocad` format

## Current capabilities

| Area | Status |
|---|---|
| Parametric sketches | Distance, radius, horizontal/vertical constraints |
| Features | Extrude, hole, fillet, chamfer |
| Patterns | Linear, circular, mirror (`union` / `cut`, `spacing_expr`) |
| TopoRef | Semantic face refs, fingerprint fallback, `assign_face_ref` patch |
| Agent API | JSON-RPC over stdio (`opencad agent`) — patch, query, diff, regen, pick |
| Kernel | OCCT 8.0 via cadrum (auto-download on first build) |
| Headless | CLI regen, mesh render, STL export, golden regression tests |

## Stack

| Layer | Technology |
|---|---|
| Core | Rust |
| Geometry kernel | OpenCASCADE 8.0 (static via cadrum) |
| Desktop UI | Tauri + Web |
| Rendering | wgpu |
| Scripting | Python (plugins) |

## OCCT (no apt required)

First build downloads a prebuilt OCCT binary automatically:

```bash
cargo build -p opencad-kernel-occt
cargo test -p opencad-kernel-occt
```

Optional system install: see [docs/developer-guide/occt-install.md](docs/developer-guide/occt-install.md).

## Quick start

```bash
cargo test --workspace
cargo run -p opencad-cli -- --help

# Agent API (JSON-RPC on stdio)
echo '{"jsonrpc":"2.0","id":1,"method":"opencad.inspect","params":{"path":"bracket.ocad.d"}}' \
  | opencad agent
```

## Agent API

See [docs/api/agent.md](docs/api/agent.md) and `examples/agent/` for JSON-RPC request samples (`patch`, `query`, `diff`, `regen`, `pick`, `assign_face_ref`).

## Repository layout

```
modules/     Rust crates (core, graph, sketch, feature, …)
apps/        Desktop and web applications (future)
schemas/     .ocad JSON schemas
docs/        Architecture, ADRs, API reference
examples/    Parametric model examples
tests/       Integration and regression tests
```

## Documentation

- [Architecture overview](docs/architecture/overview.md)
- [Developer guide](docs/developer-guide/index.md)
- [AGENTS.md](AGENTS.md) — rules for AI agents working in this repo

## License

MIT OR Apache-2.0
