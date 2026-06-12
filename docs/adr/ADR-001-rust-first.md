# ADR-001: Rust-first core architecture

## Status

Accepted

## Context

ForgeCAD needs a core language that supports:

- Deterministic regeneration
- Safe concurrent access patterns
- Long-lived CAD data structures
- AI-generated code that is auditable and testable
- A hard boundary around the OCCT C++ backend

## Decision

Use **Rust** for all core modules: design graph, sketch model, constraint solver, feature pipeline, file format, agent API, and rendering.

Use **C++** only inside `modules/kernel-occt` as an isolated OCCT shim.

Use **Python** later for plugins, scripting, and test automation — not for core ownership.

## Consequences

### Positive

- Memory safety and predictable performance in the hot path
- Strong module boundaries via Cargo workspace crates
- Easier CI with `cargo test`, `clippy`, and `rustfmt`
- OCCT complexity is contained behind a trait boundary

### Negative

- OCCT integration requires a C++ FFI bridge
- Desktop UI will use Tauri (Rust + Web) rather than Qt in MVP
- Steeper onboarding for contributors unfamiliar with Rust

## Alternatives considered

| Alternative | Rejected because |
|---|---|
| C++ everywhere | OCCT already forces C++; expanding C++ to graph/AI layers increases coupling |
| Python core | Too slow and unsafe for deterministic geometry regeneration |
| Go core | Weak ecosystem for CAD kernels and numeric solvers |
