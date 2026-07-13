# ForgeCAD

<p align="center">
  <strong>AI-native parametric CAD, built on a deterministic Design Graph.</strong>
</p>

<p align="center">
  <img src="docs/assets/forgecad-showcase.gif" alt="ForgeCAD deterministic presentation orbit of a two-component assembly with feature edges and floor grid" width="960">
</p>

<p align="center">
  <sub>Deterministic Design Graph → OCCT regeneration → presentation viewport → reproducible GIF export.</sub>
</p>

<p align="center">
  <a href="https://github.com/rsasaki0109/ForgeCAD/actions/workflows/ci.yml"><img src="https://github.com/rsasaki0109/ForgeCAD/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <img src="https://img.shields.io/badge/Rust-stable-dea584?logo=rust" alt="Rust stable">
  <img src="https://img.shields.io/badge/kernel-OCCT%208.0-3b82f6" alt="OCCT 8.0">
  <img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-22c55e" alt="MIT OR Apache-2.0">
</p>

ForgeCAD is an open-source parametric 3D CAD engine designed for humans,
agents, and CI pipelines to edit the same model safely.

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
| Assemblies | Components, instances, connectors, mates, patterns, deterministic regeneration |
| Drawings | Orthographic SVG, mesh-based hidden lines, model-driven linear dimensions |
| TopoRef | Semantic face refs, `plane_face_ref` / hole `face_ref`, fingerprint fallback |
| Agent API | JSON-RPC over stdio — patch, query, diff, dry-run, regen, pick, assembly and drawing operations |
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

# Sample documents (also in examples/)
cargo run -p opencad-cli -- new examples/bracket.ocad.d
cargo run -p opencad-cli -- new examples/bracket_hole_row.ocad.d hole-row
cargo run -p opencad-cli -- new examples/bracket_hole_ring.ocad.d hole-ring
cargo run -p opencad-cli -- new examples/bracket_pin_row.ocad.d pin-row
cargo run -p opencad-cli -- new examples/bracket_pin_ring.ocad.d pin-ring
cargo run -p opencad-cli -- new examples/bracket_pin_mirror.ocad.d pin-mirror
cargo run -p opencad-cli -- regen examples/bracket.ocad.d
cargo run -p opencad-cli -- animate examples/assembly_two_brackets.ocad.d showcase.gif

# Agent API (JSON-RPC on stdio)
echo '{"jsonrpc":"2.0","id":1,"method":"opencad.inspect","params":{"path":"examples/bracket.ocad.d"}}' \
  | opencad agent
```

## Examples

| Path | Description |
|---|---|
| `examples/bracket.ocad.d` | Bracket plate with centered mounting hole (`face_ref`) |
| `examples/bracket_hole_row.ocad.d` | Linear cut pattern with `spacing_expr: hole_pitch` |
| `examples/bracket_hole_ring.ocad.d` | Circular cut pattern (`hole-ring`) |
| `examples/bracket_pin_row.ocad.d` | Linear union pattern fused onto plate (`pin-row`) |
| `examples/bracket_pin_ring.ocad.d` | Circular union pattern fused onto plate (`pin-ring`) |
| `examples/bracket_pin_mirror.ocad.d` | Mirror pattern via `plane_face_ref`, fused onto plate |
| `examples/assembly_two_brackets.ocad.d` | Two-instance assembly with connectors and mates |
| `examples/bracket_front_view.ocad.d` | Drawing document with hidden-line classification and an 80 mm model-driven dimension |
| `examples/agent/` | JSON-RPC request samples for `opencad agent` |

Pattern comparison: [docs/examples/patterns.md](docs/examples/patterns.md).

Regenerate and export:

```bash
cargo run -p opencad-cli -- regen examples/bracket_hole_row.ocad.d
cargo run -p opencad-cli -- export examples/bracket.ocad.d bracket.stl
```

## Agent API

See [docs/api/agent.md](docs/api/agent.md) and `examples/agent/` for JSON-RPC request samples (`patch`, `query`, `diff`, `regen`, `pick`, `assign_face_ref`).

## Repository layout

```
modules/     Rust crates (core, graph, sketch, feature, …)
apps/        Desktop shell (`apps/desktop` — Tauri + Web UI)
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
