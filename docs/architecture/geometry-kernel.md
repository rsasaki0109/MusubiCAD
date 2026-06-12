# Geometry Kernel Boundary

ForgeCAD separates **design intent** from **kernel B-Rep cache**.

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
}
```

## Handles

- `KernelBody` — opaque solid
- `KernelWire` — closed sketch profile
- `TopoRef` — semantic face/edge reference (not raw OCCT indices)

## Mock backend

`MockGeometryKernel` enables tests and headless CI without OCCT installed.

## Tolerances

All geometry comparisons use explicit tolerances. Never compare raw `f64` with `==`.

## Further reading

- [ADR-002 OCCT backend](../adr/ADR-002-occt-backend.md) (planned)
- [Topological naming](./topological-naming.md) (planned)
