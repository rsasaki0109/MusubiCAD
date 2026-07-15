# Developer Guide

Welcome to MusubiCAD development.

## Prerequisites

- Rust stable (see `rust-toolchain.toml`)
- `rustfmt` and `clippy` components
- OCCT: auto-installed via `cadrum` on first build (see [occt-install.md](occt-install.md))

## Getting started

```bash
git clone https://github.com/rsasaki0109/MusubiCAD.git
cd MusubiCAD
cargo test --workspace
cargo run -p opencad-cli -- help

# Use committed samples
cargo run -p opencad-cli -- regen examples/bracket.ocad.d
cargo run -p opencad-cli -- new my_part.ocad.d hole-row
cargo run -p opencad-cli -- new my_holes.ocad.d hole-ring
cargo run -p opencad-cli -- new my_bosses.ocad.d pin-row
cargo run -p opencad-cli -- new my_ring.ocad.d pin-ring
cargo run -p opencad-cli -- new my_mirror.ocad.d pin-mirror
cargo run -p opencad-cli -- animate examples/assembly_two_brackets.ocad.d showcase.gif \
  --frames 36 --fps 12 --orbit-deg 220 --pitch-deg 26
```

## Workspace layout

| Crate | Responsibility |
|---|---|
| `opencad-core` | IDs, units, errors, transactions |
| `opencad-graph` | Design graph, parametric graph, diff |
| `opencad-sketch` | 2D sketch entities and constraints |
| `opencad-solver` | Numeric constraint solver |
| `opencad-geometry` | Kernel-neutral geometry IR |
| `opencad-kernel-occt` | OCCT geometry backend (cadrum) |
| `opencad-feature` | Feature tree and regeneration |
| `opencad-file` | `.ocad` serialization |
| `opencad-ai` | DesignPatch and Agent API |
| `opencad-cli` | Command-line interface |
| `opencad-desktop` | Shared preview + template helpers for desktop UI |
| `opencad-render` | wgpu viewport |
| `opencad-assembly` | Assembly model (Phase 3) |
| `opencad-drawing` | Drawing model (Phase 4) |
| `opencad-plugin-api` | Plugin extension points |

See [ocad-format.md](../architecture/ocad-format.md) for the native file layout.

## Development workflow

1. Pick a task from the roadmap (e.g. `Task-026`).
2. Read `AGENTS.md` for module boundaries.
3. Implement the smallest correct change in the relevant crate.
4. Add unit tests.
5. Run:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

6. Update docs if the public API or architecture changes.

## README review demo

The README hero is generated from `examples/bracket.ocad.d` and
`examples/agent/review_width_patch.json`; do not edit its images or reports by hand. Regenerate the
complete bundle on Linux or macOS with:

```bash
./docs/assets/generate-review-demo.sh
```

On Windows PowerShell, use `./docs/assets/generate-review-demo.ps1`. The Design Review workflow
compares reports byte-for-byte and raster output with a 1% normalized mean-absolute-error tolerance.
The tolerance absorbs GPU rasterization differences while still detecting a material visual change,
so Design Graph, flagship patch, or renderer changes may require updating the bundle in the same PR.

## PR checklist

- [ ] Task ID in PR title (`Task-XXX: …`)
- [ ] Tests added
- [ ] `cargo fmt` and `clippy` pass
- [ ] No OCCT types outside `kernel-occt`
- [ ] Serialized data remains deterministic
- [ ] Docs/ADR updated if needed

## Key invariants

See `AGENTS.md` section 7. The Design Graph is always the source of truth.

## Next tasks

Recent milestones (post-bootstrap):

- Linear / circular / mirror patterns with union/cut
- Semantic `TopoRef` patches and fingerprint fallback
- Agent API query/pick/explain over documents
- OCCT integration tests for patterns and topo sync

Up next (recommended):

- Face-ref-driven hole placement in sample documents
- Mirror plane from semantic face refs
- CI green + committed `examples/*.ocad.d` fixtures
- Optional MusubiCAD CLI/crate rename from the current `opencad` prefix
- Desktop UI: [desktop-ui.md](desktop-ui.md)
