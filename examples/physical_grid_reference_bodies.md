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
