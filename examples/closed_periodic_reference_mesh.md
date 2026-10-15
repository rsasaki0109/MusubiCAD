# Closed periodic reference triangle shells

P16-039 / ADR-092 creates a fully connected f64 SI mesh of the ideal fillet,
original cylindrical region and both caps. This is a read-only diagnostic,
not an authored feature or physical Mock runtime.

```sh
cargo run -p opencad-geometry --example periodic_fillet_closed_mesh > target/closed-reference-mesh.json
cargo test -p opencad-geometry --test circular_fillet_closed_mesh
python tools/test_closed_reference_mesh_probe.py
python tools/run_closed_reference_mesh_probe.py \
  --occt-root target/cadrum-occt-v800-x86_64-unknown-linux-gnu \
  --output target/closed-reference-mesh-diagnostic.json
```

Both constant and variable radius cases include coarse/fine closed shells with
explicitly shared vertex IDs, all ten reference face groups and complete
position/facet-normal bounds. The fine fixture has 2114 vertices and 4224
triangles. Position bounds are approximately 39/43 micrometres; vertex and
triangle budgets and requested normal chord bounds are explicit. Tighter
requests refine deterministically or refuse without a partial mesh.

The actual native probe constructs triangular faces and measures closed sewn
solids, checking current face connectivity/normals and independently measured
volume/area/COM against actual triangle integrals. These are faceted native
solids; the existing rational diagnostic retains its separate analytic supports.
Four actual input/winding refusals emit empty stdout. Independent constant-limit
polygon-frustum tests check volume, area and z moment; variable refinement is
compared to the independently bounded ideal volume. Mesh and ideal masses are
kept distinct.

No global embedding, exact G1, native floating-kernel proof or production
Mock/source/MCP admission follows. See [API](../docs/api/closed-periodic-reference-mesh.md).
