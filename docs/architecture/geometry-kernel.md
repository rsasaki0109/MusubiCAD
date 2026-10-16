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

P16-033 / [ADR-086](../adr/ADR-086-native-rational-surface-certificates.md)
recomputes complete position/normal/regularity/orientation bounds from current
post-sewing native surface definitions. A fixed immutable descriptive snapshot
converts original millimetre controls outward into SI homogeneous intervals;
common weight gauge is normalized with its uncertainty retained. The shared
complete-cover proof gates both position and normal requests. An explicit Rust
example reads bounded diagnostic input and echoes its exact data; the live probe
binds certificates to current native capture, canonical domain and identity
Location. No core I/O, OCCT types or editing/body/source authority is introduced.
Floating evaluation, trims, contact coincidence, exact G1 and whole-BREP/global
embedding remain separate gates.


P16-032 / [ADR-085](../adr/ADR-085-rounded-rational-normal-certificates.md)
certifies the actual immutable rational controls' derivatives, positive Jacobian,
outward orientation and unit-normal error on each entire closed chart. Private
reference binding, outward homogeneous differences, an analytic ideal Jacobian
lower bound and deterministic Bernstein restriction provide complete bounded
coverage; invalid/unattainable requests refuse without geometry mutation. Shared
private de Casteljau restriction preserves previous area-integration semantics.
Support/quarter seam normal implications concern supplied mathematical geometry;
actual native D1/Jacobian/boundary witnesses remain separately gated numerical
evidence, not global native G1/embedding or source admission.


P16-031 / [ADR-084](../adr/ADR-084-rational-rolling-sphere-surfaces.md) supplies
immutable degree-(8,2) rational characteristic-circle charts, a direct
reparameterization of the ideal envelope with outward actual-control position
bounds. A separate bounded 256-span analytic Hermite cylinder pcurve includes
fourth-derivative interpolation and stored-control rounding errors. Native
construction retains supplied contact edges/linear blend pcurves and unchanged
analytic supports, yielding closed diagnostic solids. Actual current controls,
paired pcurves/normals, topology, independent volume and transformed physical
phase are checked. Native numerical evidence does not certify global rounded
G1/embedding or admit physical Mock, production geometry, mesh or guarded source.
Old polynomial-band certificates are not transferred; no editable NURBS or OCCT
types enter the graph/source API.

P16-030 / [ADR-083](../adr/ADR-083-native-contact-support-trimming.md) constructs
separate diagnostic retained faces on the unchanged analytic cylinder and top
plane using rational contacts and native projected pcurves. Current support,
contact representation, ranges, tolerances, topology, region classification and
independent area/phase controls are measured. Approximate cylindrical pcurves
and native integration have explicit numerical gates, not global certificates.
Wire builders may copy edge identities; no semantic ownership is inferred.
No compatible blend, shell, solid, physical Mock, source or MCP admission follows.

P16-029 / [ADR-082](../adr/ADR-082-rational-support-contact-curves.md) supplies
immutable fixed-degree rational side/end contact curves with positive weights,
encoded support identities and outward positional error bounds. Native SDK
edges/wires and measured phase/D1 are separate diagnostic outputs. No OCCT
curve types enter the geometry API; no source/editor/body admission is added.
Existing polynomial surfaces must be rebuilt/re-certified before using new edges.

P16-028 / [ADR-081](../adr/ADR-081-curved-reference-area-enclosures.md)
reuses private outward Bernstein arithmetic for band/skirt parametric-area
bounds. Deterministic cross-product subdivision changes integration partitions,
not geometry. Native per-face curved areas are independent measured outputs;
planar caps, embedding and native error guarantees remain outside the certificate.

P16-027 / [ADR-080](../adr/ADR-080-closed-reference-boundary-integrals.md)
independently integrates actual mathematical band/skirt/cap boundary polynomials
with outward Bernstein arithmetic for signed volume/first moments. Native GProp
COM/moments are separate measured outputs. Numerical native comparisons do not
certify sewing/conversion/integration error, embedding or physical Mock admission.

P16-026 / [ADR-079](../adr/ADR-079-closed-native-reference-diagnostic.md) closes
diagnostic bands using ruled polynomial skirts and planar caps, and measures
actual native solids/refinement against bounded ideal volume. Skirts approximate
the cylinder side; they do not implement exact support trimming. This separate
pinned-SDK investigation introduces no production body, Mock record or source
feature. Native mass is measured independently, never replaced by ideal mass.

P16-025 / [ADR-078](../adr/ADR-078-bounded-periodic-bezier-bands.md) assembles a
conforming complete four-chart reference band with explicit position/normal and
patch-count budgets. Uniform dyadic refinement preserves shared control nets;
failed quality never returns a partial cover. An isolated native diagnostic
sews actual faces and validates connected open-shell/boundary topology. Native
coordinates convert SI to millimetres explicitly. No GeometryKernel body or
solid/mesh/source admission follows from this transient reference cover.

P16-024 / [ADR-077](../adr/ADR-077-bezier-normal-and-orientation-bounds.md)
adds current-control-net derivative-hull certificates: positive local Jacobian
area, ideal-oriented unit normals and bounded normal discrepancy. Native D1
normal/area and seam witnesses extend the existing isolated SDK diagnostic.
Position metadata is not trusted by this recomputation. Local immersion and
bounded normal gaps do not establish G1, global embedding or a closed body.

P16-023 / [ADR-076](../adr/ADR-076-bounded-bezier-reference-patches.md) constructs
kernel-neutral bicubic reference poles and a Bernstein convex-hull positional
error bound. An isolated pinned-SDK diagnostic creates actual native surfaces
from those metre-valued poles; OCCT types stay inside the native module. Shared
boundary curves do not imply a tangent-continuous solid. No GeometryKernel body,
source feature or cached mass is supplied; physical construction remains pending.

P16-022 / [ADR-075](../adr/ADR-075-bounded-periodic-reference-patches.md) supplies
rational-chart patch bounds as a prerequisite for investigating direct controlled
reference approximation. Private outward interval jets enclose position, normal,
first derivatives and second-derivative norms. A Taylor/barycentric inequality
bounds positional triangle-interpolation error including rounded shared corners.
The shared numerical helper is private, preserving ADR-073 volume semantics.
This does not admit a mesh, tangent native surface, solid, source or editable NURBS.

P16-021 / [ADR-074](../adr/ADR-074-native-periodic-fillet-counterexamples.md)
adds an isolated diagnostic executable under `modules/kernel-occt/diagnostics`.
Its concrete OCCT types stay inside the native module; only SI measurements leave
it. The Python runner invokes the actual Rust reference-volume example and
checks controls/counterexamples, not product admission. No production bridge,
GeometryKernel operation, body construction or source tag is enabled.

P16-020 / [ADR-073](../adr/ADR-073-periodic-reference-volume-bounds.md) adds a
bounded ideal reference volume method. An analytic characteristic-circle reduction
leaves one integral; private outward-rounded rational-chart intervals enclose it
under explicit absolute SI width and cell budgets. It refuses unmet precision,
retains the constant-only API semantics and never replaces native mass. This
admits no new solid, mesh, source or native approximation.

P16-019 / [ADR-072](../adr/ADR-072-bounded-periodic-fillet-reference.md) adds the
pure `CircularFilletReference`, with explicit bounded dimensions/angles, ideal
sphere-envelope point/normal and analytic two-parameter tangents. A factorization
of the moving-sphere residual proves global exposure/injectivity and regularity
for r_max<=R/4, H>=4*r_max in real arithmetic. It admits no kernel solid, native
approximation or sampling. Constant-limit reference volume is independent of any
backend; nonconstant volume calls refuse. See the
[reference API](../api/periodic-fillet-reference.md).

P16-018 / [ADR-071](../adr/ADR-071-physical-cylindrical-rim-witnesses.md) adds
`cylindrical_stock_rims` and `CylindricalRimsGeometry` as a read-only prerequisite.
Analytic Mock uses cylinder/cap/side records and a separate deterministic physical
rim namespace, with exact current circle queries. Native incidence uses IsSame,
including Location; a circular extrusion may reuse a TShape for its two caps.
Private BREP integer indices follow the pinned cadrum unique-edge iterator and
are validated by actual public circle queries. IDs remain body-scoped; no source
phase, general edge selector or fillet body is inferred. See the
[API](../api/cylindrical-rim-witnesses.md).

P16-017's [ADR-070](../adr/ADR-070-closed-circular-variable-fillet-acceptance.md)
defines a future closed circular candidate and independent sphere-envelope,
contact and constant-radius volume reference. It does not enable a curved-edge
runtime. Existing Mock variable fillets change handles rather than construct
physical bodies. Native radius coordinates/periodicity, regularity/exposure and
physical Mock construction/mesh sampling require separate evidence and a further
ADR before P16-008. A meridional quarter-circle revolution is insufficient when
the radius derivative is nonzero; authored phase must not follow a BREP seam.

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

### Deterministic printable vector output (ADR-045)

PDF conversion shares regenerated Mock/OCCT drawing geometry and SVG primitives;
no kernel mutation or new sampling occurs. Pinned svg2pdf/text and pdf-writer
convert in-memory pages with a bundled font, disabled resource resolvers and
strict missing-glyph failure. Local numeric checks prevent invalid shapes from
being dropped by the SVG parser. Precise PDF-only SVG root sizing and explicit
72 DPI preserve paper metres → points; normalized Form XObjects are scaled to
that page size and reference IDs are deterministically renumbered. All page
buffers are assembled before file output; no font lookup, timestamp or external
I/O occurs in the drawing module. Root/Desktop lockfiles include the new deps.

## Fixed faceted solids (ADR-049)

`TriangleSolid` privately owns f64 metre coordinates and indexed triangles.
Bounded explicit-unit STL parsing admits closed connected oriented manifold
shells; vertex fans and a bounded deterministic triangle-intersection sweep
reject ambiguous/self-intersecting inputs. Mass integrals use a local origin.
`GeometryKernel::import_triangle_solid` accepts this neutral value, with opaque
`KernelBody` output. OCCT constructs triangle faces, sews at 1e-8 m, validates
and checks self-interference/volume before private BREP transfer to cadrum.
Mock state is owned per instance; mesh measurements and rigid transforms are
accurate. Unsupported mesh edits, compounds and inertia fail explicitly.
No OCCT types or document ownership cross the geometry interface.

## Closed cubic admission

[ADR-057](../adr/ADR-057-closed-cubic-sketch-spline-admission.md) admits one closed
non-rational cubic span after the ADR-012 gate. Three neutral SI controls imply
the fourth is the first; the private OCCT bridge builds exact Bezier BREP and
checks analytic volume. Mock stores analytic prism metadata, with true volume,
centroid, polynomial bounds and capped GL8 perimeter; unsupported shape queries
refuse. OCCT cubic bounds use shape-based AddOptimal without triangulation;
legacy bounds and cache identities are unchanged. Solved control coordinates
are transient, source point expressions survive warm regeneration, and actual
resolved controls enter cubic-only cache identities. No NURBS type escapes the
kernel or becomes editable by inference. See [the API limits](../api/cubic-spline.md).


## Bounded analytic Mock admission

[ADR-062](../adr/ADR-062-analytic-seated-hole-admission.md), registered by
MCAD-P16-001, admits an explicit analytic-primitives Mock configuration for
MCAD-P16-002. MCAD-P16-002 implements this explicit mode; default ordinary Mock IDs/mass
remain synthetic, while existing tracked ellipse/cubic/mesh/compound paths retain
their documented physical scope.

The new mode must retain SI primitive/cut provenance, independently calculate
contained concentric removal without overlap double-counting, and refuse
unsupported operations/queries instead of synthesizing a cube. Default legacy
Mock behavior and geometry-neutral OCCT boundaries stay intact. Runtime records
are disposable; no kernel handle or new source representation is serialized.
Seated-hole source/schema and semantic/source ownership still require their
feature-specific gate before MCAD-P16-003/004 implementation; the physical
mesh/stock witness prerequisite is implemented below.

Analytic mode stores bounded SI rectangle/circle profiles and stock/cylinder
records. Coaxial subtraction decomposes the exact axial endpoints and integrates
the largest removed radius on each interval; overlapping tools are counted once.
First moments, cylindrical walls, internal annular caps and changed outer faces
produce physical centre and area. Bounds use stock corners or cylindrical support
radii; proper rigid placement preserves source records. P16-010 adds bounded mesh sampling without a triangulation dependency; no OCCT
dependency, global state or serialized source DTO enters the geometry module. Unsupported
consumers refuse; see [the API contract](../api/analytic-mock.md).


## Counterbore mesh and face gate (P16-010)

[ADR-063](../adr/ADR-063-counterbore-source-and-physical-mesh-admission.md)
registers P16-010 before P16-003. The legacy synthetic box helper is not a closed
solid and cannot supply physical counterbore evidence. The accepted design uses
actual analytic stock/cylinder/step records, a bounded deterministic circular
grid with true rectangle-corner anchors, closed correctly wound caps/walls,
owned face identities and independently integrated mesh/error evidence.

A transient rectangular-stock query verifies actual geometry and one unique
entry pick; the feature layer separately owns source-node/semantic bindings.
Native checks stay private and IDs remain body scoped; no OCCT types or source
ownership cross the interface. P16-010 implements these runtime meshes and the
stock-face query; default Mock factories remain unchanged. The private native
bridge is necessary because cadrum does not expose exact surface/curve types or
complete cuboid topology admission. It exchanges BREP bytes and finite SI face
frames only. Native face enumeration follows the existing TopExp_Explorer order,
verified against actual tessellation metadata. Source/seat/schema and document
ownership gates are implemented by P16-003 (PR #106), described below. See the [runtime API](../api/analytic-mock.md)
for quality, budgets, history and unsupported query boundaries.


## Seated-hole source and output ownership (P16-003)

ADR-063's feature executor obtains source nodes through a default-refusing
read-only `RegenContext` method, independently compares the original rectangle
and distance extent with the kernel's physical stock witness, then performs two
inward cuts. Exact physical volume/first moments and actual closed output mesh
are mandatory; cached outputs repeat admission instead of trusting prior refs.

`Feature::discover_output_faces` optionally supplies transient owned discoveries
through the registry, without changing serialized `FeatureOutput` or model
source. The seated-hole implementation tests actual triangle planes and outward
normals against the admitted stock frame with projected f32 coordinate bounds.
Original outer faces retain their producer; new cylinder/step/blind faces name
the seat feature. Ambiguity/discovery errors propagate, so an annular floor is
not classified as the source stock's entry. Other feature discovery defaults
remain unchanged. Geometry kernels contain no Design Graph ownership; concrete
OCCT data stays private. CLI/Agent gates cannot disable verification for these
source tags; default Mock factories and document version policy stay unchanged.

## Bounded countersink geometry (P16-011)

ADR-064 adds a transient SI countersink cut operation. The kernel validates an
actual uncut cuboid and contained entry disk. Native privately constructs a
cylinder plus cadrum conical frustum and derives the final result from original
stock faces. Analytic Mock records the bore and linear-radius frustum, computes
their exact union envelope/volume/first moments/area and meshes the real cone
with paired endpoint rings. Existing cylindrical chains retain their admission;
general cuts on tapered stock refuse. No source graph/schema/standard sizing
moves into the kernel. P16-004 separately implements the closed angle-bearing
seat source, semantic ownership, DesignPatch/transaction and history gates.

## Physical cylindrical-stock draft (P16-012)

ADR-065 separates exact full-cylinder witnessing and solid-frustum generation
from ADR-036's convex planar draft. Geometry receives no source graph or semantic
ownership. A private native bridge checks BREP surface/curve/full-boundary types
and body-specific cap/side indices. It uses actual outward derivatives and face
orientation, so a valid indirect cylinder frame is not mistaken for an inward
face. The validated neutral-plane radial law produces a real cone via a bridge
that catches OCCT construction failures before publishing a handle.

Analytic Mock has a separate solid Frustum record, exact mass/bounds and real
closed conical disk/wall meshes. No synthetic IDs establish physical proof;
new face identities have no fabricated reconstruction ancestry. Default Mock
and legacy planar draft stay unchanged. P16-005 adds source ownership, consumed
side/retained neutral-cap semantics, patches/schemas/history and MCP evidence.

P16-005 exposes ADR-065's physical family through the distinct closed
`cylindrical_draft` source tag. Feature admission compares authored circle/
placement/distance with the exact body-owned witness and requires its owned full
side. Geometry stays transient; SI angle/neutral plane stay graph source.
Changed side consumption and physically classified cap/conical producers replace
fabricated ancestry. Native hosts force candidate verification through existing
DesignPatch/transaction gateways; cached outputs repeat source/physical proof
and rediscover owners. General curved or already-modified stock still refuses.

P16-013 / ADR-066 adds transient `TaperedRibInput` and an explicit physical
stock/trapezoidal-prism union. Runtime proves actual uncut rectangular stock,
unit witnessed axes, contained base and finite resolvable taper; only later
P16-006 graph admission can prove original extrusion-end/source ownership.
Analytic Mock's RibbedStock records exact added volume/first moments and exterior
area, with an exact planar contact partition, no internal interface and no fake
reconstruction ancestry. Native exception-catching BREP construction performs
actual prism/fuse, requires one valid solid, 11 planar faces/24 edges/16 vertices,
exact stock/contact/top vertices and independent expected volume/area. Unsupported
records/inputs/precision refuse; no source tags or implicit kernel switches ship.

P16-006 exposes ADR-066's runtime through closed `tapered_rib`. Original authored
rectangle/distance/NewBody and complete open-line source independently prove an
owned original extrusion end; typed end roles stay stable under placement.
Source compares actual stock geometry, strict SI expressions and square-end
admission before calling the transient operation. Exact physical output planes
prove six retained stock/end producers and five new rib producers; no full-end
consumption or fake ancestry. Cached owners are rediscovered and host candidate
verification is mandatory through existing DesignPatch/transactions. Only an
explicit declared construction-line consumer admits a profile-free sketch;
legacy/unconsumed profile validation remains unchanged.

P16-014 / ADR-067 introduces immutable nominal metric basic-profile arithmetic
in geometry, separate from physical kernel records and document source. Explicit
coarse size/handedness/whole turns define a flat-root 60-degree meridional screw
envelope, square clips, SI radii and ideal section/volume targets. The helper
creates no body and proves no native sweep; sharing it cannot substitute for
independent measurements in both kernels. Rounded-root production/tolerance
claims and physical construction/sampling remain behind further admission.

P16-015 / ADR-068 supplies actual nominal basic-flat-root thread runtime.
The operation uses a real full-cylinder witness, explicit phase axis and coarse
whole-pitch profile. Mock stores an immutable analytic screw envelope with exact
ideal properties/support bounds and bounded integer-partition closed triangles;
unsupported consumers refuse. Native builds a precision-controlled per-turn
valley cut in local mm, square clips and converts/places back in SI. Left hand
is a validated local shape reflection; global placements remain proper rotations.
Native properties measure the BREP with adaptive epsilon1e-9 and tight bounds;
transient metadata selects query/quality/placement guards, never analytic mass
substitution or document ownership. New output IDs have no fabricated ancestry.
P16-007 supplies the authored source, semantic/cache/schema/patch/history/eval
contract below.

P16-016 / [ADR-069](../adr/ADR-069-guarded-nominal-thread-source.md) defines that
source admission before implementation. Phase comes from the checked
authored sketch placement, with original exact-circle/distance/NewBody stock
and its owned full side. All actual output faces are newly produced by the
thread; noncircular ends and native split surfaces cannot retain cylinder
producers or use cylindrical draft's three-face classifier. Proposed discovery
roles describe physical ends/surfaces, with ambiguity refusal and no stable
native subface numbering. Cached outputs must undergo current source and
physical discovery checks. P16-016 was design only; P16-007 now supplies the implementation below.

P16-007 implements that boundary as closed `nominal_metric_thread`. Source values
and raw fallbacks/strict expressions validate before parameter resolution,
including empty/absent parameter graphs. The original authored circle extrusion
and owned full side must agree with the actual body witness; authored positive
sketch X fixes phase. Coordinated stock/thread edits use existing DesignPatch.
All physical output faces have the thread producer, with current start/end/surface
roles and actual backend IDs. Thread-only reference admission requires unique
owned current ends and refuses transient surface indexing, including stored IDs.
The shared matcher, resolver and provenance paths apply the same restriction;
legacy roles retain their existing behavior. No output edge discoveries or fake
ancestry are retained. Cache hits redo source/phase/mesh/producer admission.
Both schemas and no-inference migration preserve intent via a distinct tag that
old readers refuse. CLI/MCP force native candidate validation; Tauri retains its
verified shared backend path. Independent physical source tests, file/history
round trips and actual stdio evaluation protect failures and canonical Undo/Redo.
No sampling scheme, OCCT type leakage or legacy golden change is introduced.

P16-034 / [ADR-087](../adr/ADR-087-native-contact-curve-certificates.md) adds
immutable descriptive native 3D contact snapshots in geometry, with original
millimetre controls converted outward to SI homogeneous intervals. Whole-curve
Bernstein differences and common positive gauge bound all corresponding side/end
contact positions. The kernel diagnostic captures the CURRENT post-sewing curve
with explicit identity Location and fixed representation metadata; a bounded
read-only Rust process recomputes sixteen proofs with exact transport binding.
No OCCT types leave kernel-occt. This is a definition certificate, not attached
pcurve lift/contact-gap, G1 or whole-BREP admission. Existing sampled pcurve and
surface/topology/volume gates remain independent; P16-008 is still planned.

P16-035 / [ADR-088](../adr/ADR-088-native-end-boundary-certificates.md) adds
immutable affine plane-pcurve lift snapshots using original native frame/UV data
and outward SI arithmetic. Recomputed current edge, lifted pcurve and blend
definition certificates share the ideal end contact; outward pairwise sums bound
the complete corresponding-parameter end boundary. Actual attached blend lines
are separately qualified as canonical (t,1), not inferred from samples. A bounded
Rust diagnostic binds all eight proofs to current captures. No OCCT types or
ownership enter geometry, and no external certificate can authorize a body.
Side contacts, quarter seams, exact G1, floating native evaluation, global embedding
and whole-BREP/production admission remain separate obligations.

P16-036 / [ADR-089](../adr/ADR-089-native-side-boundary-certificates.md) supplies
immutable cylindrical pcurve snapshots of original [angle_rad,height_mm] controls
and their actual owning cylinder's raw frame/radius. Fixed-basis convex hulls
add native control differences to the reference Hermite bounds; global circle
Lipschitz and frame/radius inequalities give a whole lifted-position bound without
sampling. Recomputed edge/lift/blend certificates then bound all pairwise side
gaps. Actual unit weights/basis and attached blend line (t,0) are separate capture
qualifications. All eight proofs bind to raw definitions and budgets. Geometry
contains no OCCT types, source ownership or body construction; floating native
evaluation, quarter seams, paired normals/G1, embedding and whole-BREP/production
admission remain separate gates before P16-008.


P16-037 / [ADR-090](../adr/ADR-090-native-seam-boundary-certificates.md) adds a
fixed immutable quadratic current-edge snapshot and direct outward homogeneous
comparisons with both actual surface boundary rows. Current native Bezier type,
Locations, unit spans, chart adjacency and attached (1,t)/(0,t) lines are qualified
from captures, never inferred from samples. All three whole-span pairwise
position bounds must meet explicit SI precision. No ideal-chart identity,
external certificate, sampling or native mutation enters the proof. Bounded
transport emits all eight proofs together and binds every raw input/budget.
Surface/end/side proofs remain intact. Paired normals/G1, floating evaluation,
global embedding and whole-BREP/production admission remain separate; no score.


P16-038 / [ADR-091](../adr/ADR-091-native-boundary-normal-certificates.md) recomputes
four actual surface certificates once per case, then bounds all side/end/seam
unit-normal chord gaps using actual support axes, whole actual pcurve angle
bounds and ideal boundary identities. Immutable cylindrical snapshots expose
axis intervals only within geometry; no OCCT types or mutable native authority
enter the core. Outward frame cross-product/normalization inequalities include
orientation and refuse unresolved frames. All twelve gaps per case must meet
explicit precision/work requirements. Bounded transport binds actual raw data
and canonical attached maps; existing position proofs remain mandatory. Finite
normal agreement is separate from exact G1, native floating evaluation, embedding
and whole-BREP/production fidelity; no maturity credit follows.


P16-039 / [ADR-092](../adr/ADR-092-closed-periodic-reference-mesh.md) adds an
immutable indexed f64 reference triangle shell with explicit periodic/contact/cap
vertex sharing. Whole-cell Taylor and facet-normal interval bounds govern
complete dyadic refinement under vertex/triangle budgets. The old corner
interval evaluator is shared within geometry; no new writable source/NURBS or
native types enter the module. Connected opposed-edge sphere topology and signed
boundary integrals are separate from global embedding/physical Mock admission.
An isolated kernel-occt diagnostic builds actual faceted native solids and
measures current topology/geometry/mass. The existing analytic-support rational
BREP and certificates remain unchanged; no product/schema/MCP admission or score.

MCAD-P16-040 / ADR-093 connects immutable bounded reference meshes to existing
TriangleSolid admission. A bijective ordered vertex/facet map and outward actual
vertex/normal changes preserve final requested quality through its 1e-9 m grid.
Existing numeric intersection/triangle budgets remain unchanged; no exact global
embedding, product kernel body, source/history/cache or editable feature admission.
The pinned native diagnostic checks separate actual grid facets and mass.
See [the API](../api/grid-admitted-reference-solid.md).

MCAD-P16-041 / ADR-094 adds typed `import_circular_fillet_grid` to GeometryKernel.
Analytic Mock opts into actual facet records with physical mass/bounds and owned
face IDs; its generic mesh import and legacy Mock fallback remain unsupported.
OCCT constructs a genuine faceted BREP using the existing triangle bridge and
retains body-local face indices. Native reference markers require proper rigid
placement, including compound members and repeated placement. No input error
certificate extends to placed/native/f32 geometry. See
[physical reference bodies](../api/physical-grid-reference-bodies.md); this is
not authored stock/rim consumption or P16-008 feature admission.

MCAD-P16-042 / ADR-095 privately binds each raw/grid reference mesh to its original
CircularFilletReference. The grid solid can compute outward exact signed-volume
flux and subtract the independent ideal volume interval, refusing unmet SI
precision/work/absolute-difference budgets. This comparison does not modify
backend mass or promote numeric intersection admission to exact embedding.
See [scope](../api/grid-reference-volume-difference.md). Native/placement/f32
fidelity and source fillet admission remain separate.


MCAD-P16-043 / ADR-096 adds pure ideal rolling-sphere boundary area enclosures.
The analytic characteristic-arc Jacobian reduction leaves one rational-chart
interval integral; retained cylinder/end/bottom areas are enclosed analytically.
The explicit SI total-width and leaf budgets fail closed. See
[area API](../api/ideal-periodic-reference-area.md). Actual physical kernel areas
remain separate and unchanged; this does not admit production fillets, native
conversion, ideal first moments, source or serialization. No score.


MCAD-P16-044 / ADR-097 adds immutable stored-grid unsigned area versus ideal
area comparisons. Existing outward facet arithmetic sums actual stored binary64
triangles in order; ADR-096 supplies the privately bound ideal total. Explicit
SI precision/work/discrepancy requests fail closed. See
[area difference](../api/grid-reference-area-difference.md). Multiplicity is
retained; this does not certify a geometric union, exact embedding or later
native/placed/f32 conversion. Both kernels retain actual mass properties.
No source/schema/first-moment/production-fillet admission or score.


MCAD-P16-045 / ADR-098 adds pure canonical ideal first-moment/centroid enclosures.
Degree-four characteristic-arc integration reduces removed Mx and a positive
axial correction to deterministic outward rational-chart integrals. Independent
volume, stock and moment bounds produce centroid intervals through positive
volume division. Each SI full-width request and each pass's leaf cap must fit;
otherwise refuse without mutation or partial output. Existing volume/area
algorithms remain unchanged. See [API](../api/ideal-periodic-reference-moments.md).
Actual kernel mass/centroid stays separate; no actual-grid/native/placed/f32 or
production/source certificate and no score.

P16-046 / ADR-099 keeps actual stored-grid signed moment flux and positive-volume
centroid quotient arithmetic in geometry. Immutable comparisons bind the original
ideal privately and reject per-coordinate SI error excess; kernel mass stays
actual. This adds no embedding/native/placed/f32/source admission. See
`docs/api/grid-reference-moment-difference.md`.

P16-047 / ADR-100 adds `grid_faceted_periodic_cosine`: the geometry module
validates an explicit proper phase frame, actual cylinder/end-rim witness and
all canonical mesh/integral discrepancy budgets before body allocation. Analytic
Mock stores/transforms actual facets; OCCT places a validated BREP with the full
SI matrix and rechecks its solid/self-intersection/actual volume before insertion.
All output facets are new, stock remains immutable, counting forwards and legacy
Mock refuses. Canonical certificates do not propagate to native/placed/f32;
source/schema/cache/history/MCP admission remains separate.

ADR-101 exposes ADR-100 as the separately named numerical
`faceted_circular_fillet` source. The feature layer proves original authored
cylinder/end-ref/phase and validates actual output facets on cold and warm
regeneration. Mandatory CLI/MCP candidate verification, strict wire schemas and
DesignPatch/history preserve the graph as truth. All physical facets have the
new producer and ambiguous facet roles refuse. Canonical input-grid bounds stay
separate from native/placed/f32 numerical checks; see
[the source API](../api/faceted-circular-fillet-source.md).
