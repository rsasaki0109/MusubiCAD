# Numerical faceted periodic cylinder fillet

The authored example `faceted_circular_fillet.ocad.d` retains the original exact
circle sketch, 10 mm stock radius, 20 mm Distance/NewBody extrusion and an owned
complete positive end rim. A genuinely variable cosine fillet has radii 0.5 to
1.5 mm, maximum at positive authored X. All surfaces are planar facets.

Use a CLI built from this checkout; published v0.3.1 predates this feature.

```sh
cargo build --locked -p opencad-cli --bin musubicad
target/debug/musubicad regen examples/faceted_circular_fillet.ocad.d
target/debug/musubicad patch examples/faceted_circular_fillet.ocad.d examples/agent/faceted_circular_fillet_radius_patch.json --dry-run
target/debug/musubicad export examples/faceted_circular_fillet.ocad.d fillet.stl
```

The committed example and creation patch are generated through DesignPatch;
use `new_document` plus `patch_dry_run`, `review_patch`, `patch_apply` in MCP.
Native verification remains mandatory when a caller requests `verify: false`.
Edit `maximum_radius` by replacing the typed feature definition; do not alter
its representation/verification tags or remove the original stock/ref/phase.

The explicit 0.4 mm position and 0.8 normal-chord requests are canonical grid
bounds. SI reference interval widths and volume/area/moment/centroid discrepancy
budgets are persisted separately. They do not certify native conversion, f32
export or placement. The actual initial native volume is approximately
6222.97048807090 mm³, with 1088 all-new physical facets. Ideal analytic mass is
never substituted for this numerical geometry.

The MCP eval `faceted_circular_fillet_radius` changes the maximum to 1 mm,
preserves authored phase/stock/ref/quality and checks fresh physical STL contacts
and independently integrated mesh volume. Cylinder, constant-radius, wrong-phase
and changed-source substitutes fail. Rust tests separately exercise both kernels,
constant-limit analytic volume, placed phase, cache decoys and protected refusal.

[Source contract](../docs/api/faceted-circular-fillet-source.md),
[canonical runtime](../docs/api/faceted-circular-fillet-runtime.md).
