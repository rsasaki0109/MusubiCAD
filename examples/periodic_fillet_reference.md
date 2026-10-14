# Ideal periodic fillet reference

This P16-019 example exercises a mathematical reference, not a new `.ocad`
feature or a generated solid. Dimensions are a 10mm radius, 20mm tall cylinder
with a fillet radius varying from 0.5mm to 1.5mm once around its end rim.
The maximum occurs at local +X. Maximum radius is below R/4; H>=4*r_max.

```rust
use opencad_core::{Angle, Length};
use opencad_geometry::CircularFilletReference;
let reference = CircularFilletReference::new(
    Length::from_meters(0.010), Length::from_meters(0.020),
    Length::from_meters(0.0005), Length::from_meters(0.0015),
)?;
let sample = reference.sample(
    Angle::from_radians(std::f64::consts::FRAC_PI_2),
    Angle::from_radians(std::f64::consts::FRAC_PI_4),
)?;
assert!(reference.exposure_margin_m2() > 0.0);
// sample contains actual reference point/normal and analytic parameter tangents.
// A nonconstant body-volume request explicitly refuses.
assert!(reference.constant_limit_volume_m3().is_err());
```

The [actual test fixture](../modules/geometry/tests/circular_fillet_reference.rs)
checks this case and the admitted radius-ratio boundary, other scales, both
contacts, seam derivatives, direct sphere distances/factorization and numerical
tangent differences. Constant-limit volume is checked against independent
meridional-corner and closed-surface integrals. A meridional quarter-circle
revolution fails the nonconstant tangential-displacement assertion.

```sh
cargo test -p opencad-geometry --test circular_fillet_reference
```

The [API](../docs/api/periodic-fillet-reference.md) and
[ADR-072](../docs/adr/ADR-072-bounded-periodic-fillet-reference.md) distinguish
the all-angle mathematical proof from floating-point/numerical approximation
and actual Mock/OCCT solids. No source tag, native sampling, mesh, MCP command
or fillet construction is enabled by this example.

P16-020 additionally supplies ideal volume bounds with an explicit work budget:

```rust
let bounds = reference.volume_bounds_m3(1e-12, 262_144)?;
assert!(bounds.body_upper_m3 - bounds.body_lower_m3 <= 1e-12);
// Width is at most 0.001 mm³. Bounds are ideal reference values, not OCCT mass.
// A tiny budget explicitly refuses this requested precision.
assert!(reference.volume_bounds_m3(1e-12, 4).is_err());
```

The [volume tests](../modules/geometry/tests/circular_fillet_volume.rs) independently
integrate the original two-parameter surface using its analytic tangents, exercise
constant limits/scales, determinism, refinement and budget/precision refusal.
Run `cargo test -p opencad-geometry --test circular_fillet_volume`.
The existing constant-only method still refuses nonconstant radii; bounded volume
uses a separately named method under
[ADR-073](../docs/adr/ADR-073-periodic-reference-volume-bounds.md).
