# Developer Guide

Welcome to ForgeCAD development.

## Prerequisites

- Rust stable (see `rust-toolchain.toml`)
- `rustfmt` and `clippy` components
- OCCT: auto-installed via `cadrum` on first build (see [occt-install.md](occt-install.md))

## Getting started

```bash
git clone <repo-url> opencad
cd opencad
cargo test --workspace
cargo run -p opencad-cli -- help
cargo run -p opencad-cli -- new bracket.ocad.d
cargo run -p opencad-cli -- regen bracket.ocad.d
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

Foundation (completed in bootstrap):

- Task-001 … Task-015

Up next:

- Task-016: DesignGraph type
- Task-026: Sketch type
- Task-046: Solver variable model
