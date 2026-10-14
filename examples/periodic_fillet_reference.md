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
