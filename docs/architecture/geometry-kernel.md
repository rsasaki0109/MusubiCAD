# Geometry Kernel Boundary

MusubiCAD separates **design intent** from **kernel B-Rep cache**.

## Rule

> OCCT types must not appear outside `modules/kernel-occt`.

## Layers

| Layer | Crate | Role |
|---|---|---|
| Design Graph | `opencad-graph` | Source of truth |
| Geometry IR | `opencad-geometry` | Kernel-neutral handles and traits |
| Kernel backend | `opencad-kernel-occt` | OCCT FFI (Phase 2) |

## `GeometryKernel` trait

```rust
pub trait GeometryKernel {
    fn make_wire_from_sketch(&self, sketch: &SolvedSketch) -> Result<KernelWire>;
    fn extrude(&self, profile: KernelWire, extent: ExtrudeExtent, ...) -> Result<KernelBody>;
    fn boolean(&self, lhs: KernelBody, rhs: KernelBody, op: BooleanOp) -> Result<KernelBody>;
    fn tessellate(&self, body: &KernelBody, settings: &TessellationSettings) -> Result<MeshSet>;
    fn mass_properties(&self, body: &KernelBody, density: f64) -> Result<MassProperties>;
    fn bounding_box(&self, body: &KernelBody) -> Result<BoundingBox>;
    fn export_step(&self, body: &KernelBody) -> Result<Vec<u8>>;   // default: unsupported
    fn import_step(&self, step: &[u8]) -> Result<KernelBody>;      // default: unsupported
}
```

## STEP exchange

STEP crosses the kernel boundary as bytes, so no OCCT type leaves
`modules/kernel-occt`. Kernel lengths are metres and STEP files are written in
millimetres: the OCCT backend scales by 1000 on export and by 1/1000 on
import. `opencad_kernel_occt::step::normalize_step_header` replaces the
wall-clock header time stamp and OCCT's per-process product counter, so
identical geometry exports identical bytes. The Mock backend reports STEP as
unsupported. A round-trip integration test verifies the millimetre
coordinates, determinism, volume within `1e-12 m³`, and bounds within
`1e-6 m`.

## Handles

- `KernelBody` — opaque solid
- `KernelWire` — closed sketch profile
- `TopoRef` — semantic face/edge reference (not raw OCCT indices)

### Semantic TopoRef contract

`TopoRef.ref_id` is the stable persisted identity key. Its identity record also
contains `(kind, semantic.created_by, semantic.role, semantic.intent)` so a
consumer can verify the intended semantic meaning; these fields are metadata
for the same key, not alternate kernel IDs. Kernel face/edge IDs, normal hints,
bounding-box hints, and other `geometric_fingerprint` values are deliberately
excluded: they are regeneration hints, not identity. This lets a reference
survive a kernel renumbering while still preserving the authoring intent that
explains what should be rebound.

Resolution uses this order:

1. persisted kernel ID (a deterministic enumeration index, ADR-018), trusted
   only while the discovered face or edge still has the reference's role;
2. semantic producer/role match;
3. fingerprint fallback using centroid or midpoint distance and direction
   hints.

Fingerprint fallback accepts an explicit `TopoRefTolerancePolicy`. Distances
are in meters (`face_centroid_tolerance_m` and
`edge_midpoint_tolerance_m`); normal and tangent thresholds are dimensionless
absolute dot products in `[0, 1]`; all distance and vector epsilon values are
strictly positive. The default policy is available from
`opencad_geometry::TopoRefTolerancePolicy::default()` and keeps the shipped
the existing edge `2 mm`, `0.99 dot`, and `1e-9` direction-epsilon behavior;
face centroid fallback adopts the same `2 mm` default. Candidate ties are
resolved by the smallest kernel ID, independent of discovery order.

The policy is runtime configuration and is not serialized into `.ocad` data.
Existing TopoRef JSON remains readable and byte-compatible. Migration guidance
for older documents is to preserve `ref_id` and semantic fields, treat stale
kernel IDs as hints, and run the history/fingerprint sync path during
regeneration; no schema rewrite is required solely to adopt this contract.

## Mock backend

`MockGeometryKernel` enables tests and headless CI without OCCT installed.

## Tolerances

All geometry comparisons use explicit tolerances. Never compare raw `f64` with `==`.
TopoRef fallback tolerances are documented and unit-labelled in
[`docs/api/topo-ref.md`](../api/topo-ref.md).

## Further reading

- [ADR-002 OCCT backend](../adr/ADR-002-occt-backend.md)
- [ADR-011 semantic TopoRef identity](../adr/ADR-011-semantic-toporef-identity.md)
- [ADR-012 future geometry admission gate](../adr/ADR-012-future-geometry-admission-gate.md)
- [Future geometry requirements](../plans/future-geometry-requirements.md)
- [Semantic TopoRef API](../api/topo-ref.md)
- Topological reference specification: [MCAD-P5-001 in the roadmap](../plans/roadmap.md)
