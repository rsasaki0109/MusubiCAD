# Bounded physical Mock kernel example

Developer SDK example for MCAD-P16-002 / ADR-062:

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

The default Mock still uses legacy synthetic plumbing for ordinary features.
Analytic mode supports only admitted primitives, bounded concentric subtraction,
mass/area/centre/bounds and proper rigid transforms/translation. It refuses
through-all, oblique/laterally intersecting/out-of-stock cuts, nonconcentric
chains, unions/intersections, topology, tessellation, STEP, compounds, inertia,
and other shape edits. It cannot supply viewport/semantic-face output yet.
MCAD-P16-003 needs its own feature/schema/semantic/tessellation gate before
implementing document-level counterbores; do not substitute a plain hole.

Native integration evidence is in
`modules/kernel-occt/tests/analytic_primitives.rs`: both kernels are independently
checked for stepped removal in either order, blind cuts, first moments, surface
area, original input preservation, placed geometry, centered symmetric rectangle/
circle extrusions and tilted cylinder bounds. No new curve sampling is used.
