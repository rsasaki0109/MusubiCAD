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

## Native variable-fillet bridge

MCAD-P11-005 adds `GeometryKernel::variable_fillet_edge`, a bounded linear-radius
operation on one semantic straight edge. Mock validates the contract; OCCT uses
a private cxx bridge sharing cadrum's pinned OCCT 8.0 installation. The bridge
exchanges in-memory BREP bytes, works in millimetres internally and restores the
public metre units. It uses 65 deterministic parameter/radius points, validates
one unique straight edge/contour and checks the output BREP. No OCCT types or
pointers escape `modules/kernel-occt`.

Only the cxx-generated module has a scoped unsafe allowance. Other handwritten
kernel Rust remains denied and other workspace modules retain their forbid rule.
This transfer loses pointer-based exact face history; it does not invent a relay.
Subsequent references require the existing geometry-provenance checks. The
unsupported default preserves custom kernel compatibility, and counting kernels
forward this call. [ADR-038](../adr/ADR-038-linear-variable-fillets.md) records the
sampling, unit, precision, failure and file-compatibility decisions.

### Existing-body Boolean output contract

MCAD-P11-006 adds `GeometryKernel::body_boolean(target, source, operation)` for
solid-producing Feature Graph nodes. It reuses the existing Boolean operation;
OCCT additionally requires finite positive result volume. Low-level `boolean`
continues to allow empty results for intersection-volume/interference queries.
Mock returns deterministic operation IDs and counting kernels forward the full
contract. Input and output handles remain kernel-neutral. The body feature
records both operand feature IDs; dependency/cycle/type validation and verified
DesignPatch persistence own document changes. Exact derivation history is kept
for the target, with existing geometry provenance for source-derived references.

### Read-only straight-edge queries for angular dimensions

MCAD-P12-001 adds `straight_edge_endpoints` to the kernel-neutral boundary.
Mock supplies its box-edge witnesses; counting kernels forward the query. OCCT
serializes the current body to an in-memory BREP and uses the existing private
bridge to classify a uniquely endpoint-matched edge as `GeomAbs_Line` (1e-8 m
matching tolerance). Curves and missing/ambiguous matches are refused. The query
does not modify a body, invent history, expose OCCT types or fit sampled lines.
Drawing resolution derives the supporting-line intersection and explicit rays;
labels and orthographic layouts are transient. Semantic discovery retains its
existing top-Z perimeter scope. See ADR-040 for units, degeneracy, canonical
endpoint choices, view restrictions and additive file compatibility.

### Analytic circular-edge witness (ADR-041)

`GeometryKernel::circular_edge_geometry` returns neutral `CircularEdgeGeometry`
(center in model metres, unit normal, radius in metres) for an exact complete
circle. The OCCT backend reads its current BREP, matches one unique edge by its
current endpoints (1e-8 m), requires native circle type and full 2π domain
(1e-9 rad). It performs no sampled curve fit or document mutation. Mock edge 5
provides a deterministic XY circle of radius half the body's side for contract
tests; Mock box edges 1–4 refuse circle queries. Counting kernels forward queries.
Drawing regeneration passes witnesses/layouts separately from authored Sheet
data; unsupported curves and ambiguous topology fail closed.

### Exact polygonal sections (ADR-042)

`section_body` returns neutral, transient `SectionGeometry` with canonical model
loops and SI material area. Native OCCT BRepAlgoAPI_Section disables approximation
and accepts only exact line edges. Cuts coincident with a planar face are refused.
The neutral stitcher uses 1e-8 m tolerance, rejects open/branched/intersecting or
touching boundaries, and derives area by even-odd loop nesting. Work is bounded to
4096 edges with positive areas >1e-16 m². Mock supports analytic axis-aligned box
slices. No new sampling is introduced. Drawing section metadata stores only the
plane and hatch spacing; regenerated layouts pass separately to sheet renderers.

### Bounded detail wireframe layout (ADR-043)

Detail views reuse both Mock and OCCT tessellation/projection paths and clip their
line segments analytically with Liang–Barsky. No new curve sampling or fitting is
introduced; visibility classifications survive clipping. Parent/model/projection
and explicit magnification are validated in drawing state, and child scale must
match parent.scale × magnification. Crop data is authored in projected model
metres; derived centered sheet segments remain disposable. The existing mesh
wireframe accuracy applies, including its 1 µm projected endpoint quantization.

### Title-block metadata and layout (ADR-044)

Title blocks are explicit per-sheet authored metadata, not kernel geometry.
`set_title_block` stages edits/clears through DesignPatch and sheet snapshots;
missing update fields are refused. Shared SVG export renders the fixed SI frame
on both Mock and OCCT drawing paths without changing their bodies or projection.
Layout validates sheet fit and single-line text; SVG escapes markup. No current
clock, inferred model metadata, new sampling or kernel dependencies are involved.
