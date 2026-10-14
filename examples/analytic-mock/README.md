# Bounded physical Mock kernel example

Developer SDK example for MCAD-P16-002/010 / ADR-062/063:

```sh
cargo run -p opencad-geometry --example analytic_stock_and_cuts
```

The source is `modules/geometry/examples/analytic_stock_and_cuts.rs`. It selects
the explicit analytic Mock mode and builds 40 × 30 × 10 mm rectangular stock,
a 6.6 mm through bore at (12, 9) mm, and an 11 mm top seat 4 mm deep. All runtime
coordinates/lengths are metres. Density is 2700 kg/m³. This is a kernel example,
not an `.ocad` document, DesignPatch authoring command or completed seated-hole
feature.

Independent expectations:

- Removed volume: `π × 3.3² × 10 + π × (5.5² − 3.3²) × 4` mm³.
- Remaining volume: `11414.595624930078` mm³.
- Centre: `(20.410284792773, 15.307713594580, 4.936059512815)` mm.
- Surface area: `3994.213257844921` mm².
- Bounds: `(0, 0, 0)` to `(40, 30, 10)` mm.

The example now prints the original cuboid entry witness, exact measurements,
and an actual closed mesh (10 µm linear deflection, 8° angular deflection), with
body-owned triangle face IDs. It performs no hidden file export. Repeat runs
produce the same mesh/IDs. Default Mock remains synthetic for ordinary features.

Analytic mode also supports physical meshes and face membership/history. It
refuses through-all, oblique/laterally intersecting/out-of-stock cuts,
nonconcentric chains, unions/intersections, edge/circular-edge discovery, STEP,
compounds, inertia, other shape edits and unrepresentable/out-of-budget meshes.
The cuboid witness refuses cut stock and unsupported picks. MCAD-P16-003
implements document counterbores with source/schema/semantic and DesignPatch
gates (PR #106); see `examples/counterbored_plate.md`. Countersink authoring
remains P16-004; do not substitute a plain hole or invent seat fields.

Independent evidence:

```sh
cargo test -p opencad-geometry --test analytic_mock
cargo test -p opencad-geometry --test physical_mesh
cargo test -p opencad-kernel-occt --test analytic_primitives
cargo test -p opencad-kernel-occt --test physical_mesh
```

Tests integrate the exported triangles independently, check closed opposite-edge
pairing, winding, volume, centre, true ring contact/radii, actual gap/deflection,
face coverage, owned picks and failure preservation. Native and Mock are checked
against physical expectations; matching each other's output is not the oracle.

## Physical countersink runtime example (P16-011)

```sh
cargo run -p opencad-geometry --example analytic_countersink
cargo test -p opencad-geometry --test countersink
cargo test -p opencad-kernel-occt --test countersink
```

This developer example cuts the union of a 6.6 mm bore, 10 mm deep, and a
12 mm entry diameter frustum, 2.7 mm deep (90 degree included angle), on the
same 40 × 30 × 10 mm original stock. The frustum meets the bore continuously;
there is no annular flat shoulder. Volume is
`12000 - pi*(3.3^2*10 + 2.7*(6^2+6*3.3-2*3.3^2)/3)` mm³.
Tests use several frustum depths, independent area/first moments, both entry
ends, blind/through bores and actual closed conical meshes. This runtime API
is not a document feature or MCP command; authoring countersinks requires
P16-004's strict source/schema/patch implementation.

## Physical circular-stock draft (P16-012, ADR-065)

```sh
cargo run -p opencad-geometry --example analytic_cylindrical_draft
cargo test -p opencad-geometry --test cylindrical_draft
cargo test -p opencad-kernel-occt --test cylindrical_draft
```

A 20 mm radius, 30 mm high actual cylinder is drafted by +7° about the neutral
plane at 15 mm, keeping the neutral circle fixed. The affine law is
`r(z) = 20 - tan(7°)*(z-15)` mm. The resulting solid has conical walls and circular
disk ends, not a faceted set of planar half-space drafts. Independent tests
integrate this law, then verify volume/area/COM/bounds, actual contact rings,
closed triangles/face membership, sign/plane/axis reversals, rigid placements
and protected failure. Native checks exact cylinder topology/types privately;
Mock uses explicit tracked records. Default Mock refuses this runtime API.

This example changes no .ocad document. P16-005 supplies the guarded curved-draft
source, side-reference ownership, DesignPatch/history and MCP eval afterward.
No feature/host maturity credit is awarded for P16-012 infrastructure.
