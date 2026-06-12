# Architecture Overview

ForgeCAD is an AI-native, open-source parametric 3D CAD engine.

## Core equation

```
CAD = Geometry Kernel + Parametric Program + Design Intent Graph + Validated AI Patch System
```

## Layered architecture

```
Human UI / AI Agents / CLI / Plugins
        │
        ▼
Command Layer (transactions, undo, dry-run, design patch)
        │
        ▼
ForgeCAD Design Graph
        │
        ▼
Regeneration Engine
        │
        ├── Sketch Solver
        └── Geometry Kernel Interface → OCCT backend
                │
                ▼
        Shape Snapshot / B-Rep Cache / Tessellation Cache
                │
                ▼
        Rendering / Export / Mass Properties
                │
                ▼
        .ocad Native Format
```

## Source of truth

The **Design Graph** is authoritative. B-Rep and meshes are disposable caches regenerated from the graph.

## Principles

| Principle | Meaning |
|---|---|
| Design Graph First | Graph before UI |
| AI Editable | Stable IDs, semantic tags, explicit units |
| Kernel Abstracted | OCCT behind a trait; internal IR is kernel-neutral |
| Deterministic Regeneration | Same `.ocad` + kernel version → same result |
| Semantic Topological Naming | Faces referenced by intent, not raw indices |
| Headless First | CLI/API before GUI |
| Local-first Collaboration | `.ocad` zip or git-friendly directory |
| Testable CAD | Volume, mass, constraints, regen are testable |

## Technology stack

- **Core:** Rust
- **Kernel:** OpenCASCADE (initial backend)
- **UI:** Tauri + Web (MVP)
- **Rendering:** wgpu
- **Format:** `.ocad` (JSON-based, git-friendly)

## Further reading

- [ADR-001: Rust-first](../adr/ADR-001-rust-first.md)
- [Developer guide](../developer-guide/index.md)
- [AGENTS.md](../../AGENTS.md)
