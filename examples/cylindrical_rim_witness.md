# Body-owned complete-cylinder rims

This is P16-018's executable kernel test fixture, not a new authored fillet.
The actual fixture constructor is
[cylinder](../modules/geometry/tests/support/cylindrical_rim_cases.rs).
It extrudes an exact circle centred at (12,9)mm with radius 20mm for 30mm along Z.
Independent expected circle centres are (12,9,0)mm and (12,9,30)mm;
circumference is 40*pi mm and cylinder volume is 12000*pi mm^3.
The tests also check area, COM, closed mesh and a proper rotated/translated copy.

Given this fixture's `kernel` and `body`, query both physical rims:

```rust
let rims = kernel.cylindrical_stock_rims(&body, [0.0, 0.0, 1.0])?;
let start = kernel.circular_edge_geometry(&body, rims.start_edge_id)?;
let end = kernel.circular_edge_geometry(&body, rims.end_edge_id)?;
rims.validate_circles(&start, &end)?;
```

Run the actual fixture and failure tests in both physical backends:

```sh
cargo test -p opencad-geometry -p opencad-kernel-occt --test cylindrical_rims
```

The same suite tests reversed hints, current native iterator IDs, equal-volume
stock with different radius/height, wrong Mock body/legacy/stale IDs, and
frustum/thread/cuboid/compound/invalid-hint refusals without changing source stock.
In the native circular extrusion the caps can share a TShape at different
Locations; pointer-only incidence fails this fixture. Actual IsSame incidence
and full-circle queries distinguish them.

No source tag, MCP command, schema or fillet shape is introduced. Do not turn this
read-only witness into a claim of circular variable-filleting support. See the
[API contract](../docs/api/cylindrical-rim-witnesses.md) and ADR-071.
