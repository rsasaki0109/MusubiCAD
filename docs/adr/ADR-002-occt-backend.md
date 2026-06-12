# ADR-002: OpenCASCADE backend

## Status

Accepted

## Context

ForgeCAD needs a practical B-Rep kernel for extrude, boolean, tessellation, and mass properties.
Building a custom kernel from scratch is out of scope for MVP.

## Decision

1. Use **OpenCASCADE (OCCT)** as the initial geometry kernel.
2. Keep a **kernel-neutral trait** in `opencad-geometry` (`GeometryKernel`).
3. Implement the backend in `opencad-kernel-occt`.
4. MVP links OCCT via **cadrum** (static OCCT 8.0.0 prebuilt) to avoid system install friction.
5. A direct C++ `cxx` bridge may replace cadrum later (Task-076).

## Boundaries

| Allowed in `kernel-occt` | Forbidden |
|---|---|
| OCCT / cadrum calls | Owning Design Graph |
| Handle storage | `.ocad` serialization |
| B-Rep execution | UI logic |

OCCT types must not leak outside `opencad-kernel-occt`.

## Consequences

### Positive

- Real solids on day one without sudo/apt
- CI-friendly static linking
- Trait boundary preserved for future kernels

### Negative

- cadrum API is not identical to raw OCCT
- Static binary download on first build (~tens of MB)
- LGPL OCCT license applies (with exception)

## Alternatives considered

| Alternative | Rejected because |
|---|---|
| System OCCT only | Version skew across distros; requires sudo |
| opencascade-rs builtin | Long compile from source |
| CGAL as main kernel | Not a history B-Rep CAD kernel |
| Parasolid | Commercial license |
