# Physical grid reference bodies (ADR-094)

From the repository root, with the documented OCCT environment active:

```bash
cargo run --locked -p opencad-kernel-occt --example circular_grid_reference
cargo test --locked -p opencad-kernel-occt --test circular_grid_reference
```

The example creates constant and variable radius reference meshes for a 0.01 m
radius / 0.02 m high cylinder, certifies the actual 1e-9 m grid coordinates, then
imports the same certificate through analytic Mock and OCCT. JSON reports actual
volume (m³), area (m²), density-derived mass (kg), centroid/bounds (m), triangle
and facet counts for both backends. Quality requests are 4e-4 m position and
0.8 unit-normal chord; these input bounds do not certify subsequent native/f32
conversions. No source document is created or modified.

The integration test uses independent integer-grid volume and actual facet
area/centroid, constant/coarse and variable/fine cases, physical mesh closure,
proper placement, owned face queries, unsupported analytic Mock edits and
protected invalid inputs. This is a faceted reference-body prerequisite, not an
authored variable fillet. See [the API](../docs/api/physical-grid-reference-bodies.md).

ADR-095 also calls `g.volume_comparison(CircularFilletGridVolumePrecision {
maximum_reference_interval_width_m3: 1e-12,
maximum_absolute_difference_m3: 1e-7, max_cells: 262144 })`. Each case reports
actual grid signed-volume flux, its privately bound ideal-volume interval,
signed/absolute/relative discrepancy bounds and reference cell count under
`volume_comparison`. Both backend masses remain separate actual measurements.
These comparison bounds do not certify native/placed/f32 geometry or exact
global embedding. CI retains the example JSON as
`grid-reference-volume-comparison.json` in its existing diagnostics artifact.
