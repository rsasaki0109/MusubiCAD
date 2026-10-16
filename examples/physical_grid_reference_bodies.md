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


ADR-096 adds `ideal_area` to that report with explicit 1e-7 m² total interval
width and a 262144-leaf budget. Complete ideal blend/support/total bounds remain
separate from `bodies[*].area_m2`, which is still measured from each physical
faceted body. The report does not certify their area discrepancy or native
conversion. See [ideal area](../docs/api/ideal-periodic-reference-area.md).


P16-044 / ADR-097 also compares the stored-grid area with its bound ideal:

```rust
let comparison = g.area_comparison(CircularFilletGridAreaPrecision {
    maximum_reference_interval_width_m2: 1e-7,
    maximum_absolute_difference_m2: 1e-5,
    max_cells: 262144,
})?;
assert!(comparison.absolute_difference_upper_m2() <= 1e-5);
```

The existing `circular_grid_reference` example/CI JSON adds `area_comparison`
with unsigned grid/ideal/signed-difference intervals, absolute/relative bounds
and cell count. Actual `bodies[*].area_m2` stays a separate kernel query.
The mathematical bound counts triangle multiplicity; native/placed/f32 conversion
and exact embedding are excluded. See [API](../docs/api/grid-reference-area-difference.md).


P16-045 / ADR-098 independently bounds ideal first moments and centroid:

```rust
let ideal = r.moment_bounds(CircularFilletMomentPrecision {
    maximum_volume_interval_width_m3: 1e-11,
    maximum_first_moment_interval_width_m4: 1e-12,
    maximum_centroid_interval_width: Length::from_meters(1e-6),
    max_cells: 262144, // separate volume and moment leaf caps
})?;
assert!(ideal.centroid_m[2][1] - ideal.centroid_m[2][0] <= 1e-6);
```

The existing example/CI JSON adds `ideal_moments` with all volume/moment/centroid
intervals, requested widths and separate leaf counts. `bodies[*].volume_m3` and
`center_of_mass_m` remain ACTUAL kernel measurements. Canonical y symmetry is
exact; constant radii also give exact x symmetry and analytic zero-cell results.
No midpoint/native mass substitution or grid-moment discrepancy certificate.
See [API](../docs/api/ideal-periodic-reference-moments.md).
