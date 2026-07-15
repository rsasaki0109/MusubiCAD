# AGENTS.md

This repository is the MusubiCAD project.

MusubiCAD is an AI-native, open-source, parametric 3D CAD system.
The source of truth is the Design Graph, not the GUI and not a cached B-Rep shape.

All AI agents must follow this document.

---

## 1. Coding Standards

### General

- Prefer small, deterministic, testable changes.
- One task per PR.
- Do not mix refactoring with feature implementation unless the task explicitly asks for it.
- Do not introduce global mutable state.
- Do not introduce hidden I/O in core modules.
- Do not make network calls from core modules.
- All public data structures must be serializable if they are part of `.ocad`.
- All user-visible numeric values must carry explicit units.
- All geometry comparisons must use tolerances, never exact floating-point equality.

### Rust

- Use stable Rust.
- Run `cargo fmt` before committing.
- Run `cargo clippy --all-targets -- -D warnings`.
- Use `Result<T, OpenCadError>` for fallible operations.
- Do not use `unwrap()` or `expect()` outside tests unless the invariant is documented in the same function.
- Prefer newtypes for units and IDs.
- Use deterministic ordering for serialized maps and graph traversal.
- Avoid large dependencies unless justified in an ADR.

### TypeScript / UI

- UI must not mutate the model directly.
- UI must send commands or patches to the MusubiCAD backend.
- Keep viewport state separate from document state.
- Use generated schema types where available.
- Every command exposed in UI must also be available through CLI or Agent API.

---

## 2. Module Rules

### `modules/core`

Allowed:
- IDs
- units
- errors
- document metadata
- transactions
- validation primitives

Forbidden:
- OCCT types
- rendering types
- UI types
- file system access except through explicit traits

### `modules/graph`

Allowed:
- Design Graph
- Parametric Graph
- Feature Graph
- dependency analysis
- semantic diff

Forbidden:
- direct geometry kernel calls
- direct UI calls

### `modules/geometry`

Allowed:
- kernel-neutral B-Rep abstractions
- topology references
- NURBS definitions
- tessellation data structures
- mass property interfaces

Forbidden:
- concrete OCCT API leakage
- UI state

### `modules/kernel-occt`

Allowed:
- C++ bridge
- OCCT conversion
- OCCT backend implementation

Forbidden:
- MusubiCAD document ownership
- UI logic
- direct `.ocad` serialization

### `modules/sketch`

Allowed:
- sketch entities
- sketch constraints
- profiles
- workplanes
- solve state

Forbidden:
- direct OCCT calls
- direct rendering calls

### `modules/solver`

Allowed:
- numeric solver
- residuals
- Jacobian
- DOF analysis
- diagnostics

Forbidden:
- CAD feature semantics
- file format logic

### `modules/feature`

Allowed:
- feature definitions
- feature registry
- regeneration pipeline
- feature execution through `GeometryKernel`

Forbidden:
- concrete OCCT types
- UI logic

### `modules/file`

Allowed:
- `.ocad` serialization
- schema validation
- migrations
- checksums
- expanded directory format

Forbidden:
- feature execution
- geometry mutation

### `modules/ai`

Allowed:
- DesignPatch
- Agent API
- natural-language intent wrappers
- design queries
- explanations
- dry-run orchestration

Forbidden:
- calling external LLM APIs by default
- bypassing validation
- mutating documents without transactions

### `modules/render`

Allowed:
- scene graph
- mesh upload
- selection buffer
- overlays
- camera

Forbidden:
- owning the Design Graph
- changing CAD model parameters directly

---

## 3. Testing Rules

Every PR must include tests unless it is documentation-only.

Required test types:

1. Pure data model tests
2. Sketch tests
3. Geometry tests
4. File format tests
5. AI patch tests
6. Regression tests

Commands:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

If OCCT is required for a test, mark it as an integration test.

---

## 4. Documentation Rules

Any public API change must update `docs/api/`.
Any architecture change must update `docs/architecture/`.
Any major decision must add an ADR under `docs/adr/`.
Every feature must have at least one example under `examples/`.
Every `.ocad` schema change must update `schemas/`, migration code, and round-trip tests.

---

## 5. PR Rules

PR title format: `Task-XXX: Short imperative title`

Each PR must include: linked task ID, summary, implementation notes, tests added, docs updated, known limitations.

Do not submit a PR that:
- changes unrelated modules
- disables tests
- changes golden files without explanation
- introduces non-deterministic serialization
- exposes OCCT types outside `modules/kernel-occt`
- bypasses transaction or validation systems

---

## 6. AI Agent Workflow

When implementing a task:

1. Read the task description.
2. Inspect relevant modules only.
3. Make the smallest correct change.
4. Add tests.
5. Run formatting and tests.
6. Update docs if needed.
7. Summarize the change and limitations.

When uncertain:
- Prefer adding an interface and a test over guessing hidden behavior.
- Do not invent geometry behavior without documenting tolerance assumptions.
- Do not silently change serialized schema.
- Ask for a new ADR if the change affects architecture.

---

## 7. Design Invariants

These invariants must never be broken:

- The Design Graph is the source of truth.
- Cached B-Rep and meshes are disposable.
- `.ocad` files must be deterministic.
- Model changes must go through transactions.
- AI changes must go through DesignPatch.
- Failed regeneration must not corrupt the document.
- Units must be explicit.
- Topological references must be semantic where possible.
