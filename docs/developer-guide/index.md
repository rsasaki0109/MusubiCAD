# Developer Guide

Welcome to ForgeCAD development.

## Prerequisites

- Rust stable (see `rust-toolchain.toml`)
- `rustfmt` and `clippy` components
- OCCT: auto-installed via `cadrum` on first build (see [occt-install.md](occt-install.md))

## Getting started

```bash
git clone https://github.com/rsasaki0109/ForgeCAD.git
cd ForgeCAD
cargo test --workspace
cargo run -p opencad-cli -- help

# Use committed samples
cargo run -p opencad-cli -- regen examples/bracket.ocad.d
cargo run -p opencad-cli -- new my_part.ocad.d hole-row
cargo run -p opencad-cli -- new my_mirror.ocad.d pin-mirror
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
- Optional `forgecad` CLI crate rename
