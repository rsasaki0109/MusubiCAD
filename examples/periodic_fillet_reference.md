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


For P16-021's separate native counterexample experiment, see the
[Linux diagnostic instructions](../docs/api/native-periodic-fillet-diagnostic.md).
It compiles an isolated actual OCCT fixture, invokes the
[reference-volume example](../modules/geometry/examples/periodic_fillet_volume_bounds.rs)
and records contact/interior/volume measurements plus a wrong-phase control.
Passing the diagnostic keeps curved authoring unsupported; it does not create an
`.ocad` feature or replace native mass with the ideal interval.


P16-022 supplies a positional error bound for a reference rectangle:

```rust
// Dimensionless rational chart coordinates; this covers a small ideal patch.
let patch = reference.patch_bounds(0, 0.25, 0.255, 0.30, 0.305)?;
assert!(patch.linear_interpolation_error_upper_m < 1e-6);
// The two triangles use corner indices (0,1,2) and (0,2,3).
// The bound includes corner rounding, and does not admit a mesh or native solid.
```

The [patch tests](../modules/geometry/tests/circular_fillet_patch.rs) compare
analytic constant-limit derivatives, independent angular derivatives/numerical
Hessians, barycentric interpolation, refinement/scales and all shared seams.
Run `cargo test -p opencad-geometry --test circular_fillet_patch`.
[ADR-075](../docs/adr/ADR-075-bounded-periodic-reference-patches.md) proves the
error inequality; a finite sampling grid is only a test oracle. Actual native
fitting, closed mesh construction, normal/volume error and source admission
remain separate work.


P16-023 constructs an explicit bounded bicubic reference surface:

```rust
let patch = reference.bezier_patch(0, 0.25, 0.255, 0.30, 0.305)?;
assert!(patch.position_error_upper_m < 1e-6);
// control_points_m[u][v][xyz]: 4x4 poles in metres, local surface domain [0,1]².
// Use the returned poles unchanged; their certificate does not survive editing.
```

Run `cargo run -p opencad-geometry --example periodic_fillet_bezier_patches`
for forty actual fixtures, and
`cargo test -p opencad-geometry --test circular_fillet_bezier`
for independent contact derivatives, Bernstein/reference comparisons, refinement,
scale, domain refusal and shared control-net edges. The
[native diagnostic](../docs/api/periodic-fillet-reference.md#bounded-native-bezier-reference-patches-p16-023)
constructs and measures those surfaces in OCCT; it does not create a closed
fillet body. Position error, native sampled agreement and shared curves alone
cannot establish tangent continuity, support trimming, mass or feature support.


P16-024 checks the current poles' whole-patch local orientation and normals:

```rust
let normals = patch.normal_bounds(reference, 0, 0.25, 0.255, 0.30, 0.305)?;
assert!(normals.jacobian_area_lower_m2 > 0.0);
assert!(normals.ideal_normal_alignment_lower > 0.0);
// normal_error_upper is dimensionless unit-vector distance, not radians.
// This recomputation ignores stored position bounds and cannot validate them.
```

Run `cargo test -p opencad-geometry --test circular_fillet_normals` for independent
planar area/normals, Bernstein and angular comparisons, scale and protected
refusals (collapsed/reversed/interior-corrupted nets and underflow). The existing
native diagnostic also checks actual unit normals/area and shared seam gaps.
Whole-patch bounds prove local immersion/orientation only; support trimming,
global embedding, tangent-continuous assembly and a closed solid remain pending.


P16-025 covers the entire periodic reference with conforming bounded patches:

```rust
let band = reference.bezier_band(Length::from_meters(1e-4), 0.8, 4096)?;
assert_eq!(band.cells.len(), 4 * band.divisions_per_chart_axis.pow(2));
assert!(band.position_error_upper_m <= 1e-4);
assert!(band.normal_error_upper <= 0.8);
// Explicit coarse demonstration requests; no manufacturing defaults or solid.
```

Run `cargo run -p opencad-geometry --example periodic_fillet_bezier_bands` for
constant/nonconstant covers, and
`cargo test -p opencad-geometry --test circular_fillet_band` for exact coverage,
shared edge poles, independent contact dimensions, refinement, scale, determinism
and budget refusal. The [native sewing diagnostic](../docs/api/periodic-fillet-reference.md#complete-periodic-reference-bands-p16-025)
constructs one open shell per fixture and verifies two closed boundary loops,
actual face/edge incidence, face connectivity and independent surface values.
Support trimming, exact tangent continuity, closed solids and mass remain pending.


P16-026 closes diagnostic bands with explicit polynomial skirts and planar caps:

```sh
cargo run -p opencad-geometry --example closed_periodic_reference
python tools/run_closed_reference_probe.py \
  --occt-root target/cadrum-occt-v800-x86_64-unknown-linux-gnu \
  --output target/closed-reference-diagnostic.json
```

Both constant/nonconstant laws are built at two explicit refinement levels. The
native diagnostic checks real closed positive-volume solids, connected two-face
edge incidence and outward caps, compares actual surfaces to independent
polynomials, and compares native volume estimates to the matching bounded ideal
reference. It records the skirt's cylinder deviation rather than treating the
skirt as an exact cylindrical support. Successful results never enable a product
fillet, physical Mock body or `.ocad` source; support/G1/embedding and independently
bounded mass properties remain admission gates. See
[API and measured limits](../docs/api/periodic-fillet-reference.md#closed-native-diagnostic-approximations-p16-026).

P16-027 extends the same command with independent polynomial boundary integrals.
`boundary_integrals` in each Rust fixture contains outward volume (m³) and first
moment (m⁴) intervals for the current control points, ruled skirts and caps.
The native report contains actual GProp COM (m) and first moments, compared to
those intervals using separately named numerical allowances. Its interval COM
comes from dividing moment intervals by a positive volume interval. These are
bounds for the mathematical polynomial boundary, not ideal stock or all native
sewing/integration error. No product feature is enabled. See
[independent integral contract](../docs/api/periodic-fillet-reference.md#independent-closed-boundary-integrals-p16-027).

P16-028 adds `curved_area_bounds` to the same Rust fixture and diagnostic command.
It encloses only band/skirt parametric area, excluding planar caps, with a 1e-7 m²
absolute-width request and 32768-leaf budget. Integration refinement preserves
the original poles and native topology. Native reports contain actual per-face,
curved, cap and total areas; only the curved sum has the independent mathematical
area enclosure. Reported 1e-12 m² native comparison allowance is separate from
that certificate. See [area scope and work limits](../docs/api/periodic-fillet-reference.md#curved-parametric-area-bounds-p16-028).

P16-029 constructs separate immutable rational support contacts:

```sh
cargo run -p opencad-geometry --example periodic_fillet_contacts
python tools/run_rational_contact_probe.py \
  --occt-root target/cadrum-occt-v800-x86_64-unknown-linux-gnu \
  --output target/rational-contact-diagnostic.json
```

The encoded side curves preserve their circle identity; end curves have a
constant encoded plane height. Positive weights, shared endpoint tables and
outward whole-curve position bounds accompany the actual stored data. The native
diagnostic constructs real closed wires and compares actual positions/D1 and
physical phase, including a quarter-turn decoy that keeps scalar wire length.
These are not trimmed supports or fillet bodies and cannot replace the old band
boundaries without rebuilding/re-certifying surfaces. See
[rational contact scope](../docs/api/periodic-fillet-reference.md#rational-support-contact-curves-p16-029).

P16-030 trims retained regions on the unchanged native cylinder and top plane:

```sh
python tools/run_trimmed_support_probe.py \
  --occt-root target/cadrum-occt-v800-x86_64-unknown-linux-gnu \
  --output target/trimmed-support-diagnostic.json
```

The same rational-contact fixture creates ten actual analytic faces, with current
pcurve/3D/tolerance checks, independent retained-area expressions, native region
classification and a quarter-turn phase decoy. Report UV units are rad/m on
cylinders and m/m on planes. Projection and integration allowances are numerical
gates, not whole-curve certificates. Wire construction may copy edge identities;
current contact geometry is checked explicitly. These faces are not a sewn shell,
compatible blend or product body. See
[trimming scope](../docs/api/periodic-fillet-reference.md#native-analytic-support-trimming-p16-030).

P16-031 constructs direct rational characteristic-circle surfaces and sews them
with unchanged cylinder/top/bottom supports:

```sh
cargo run -p opencad-geometry --example periodic_fillet_rational
python tools/run_rational_reference_probe.py \
  --occt-root target/cadrum-occt-v800-x86_64-unknown-linux-gnu \
  --output target/rational-reference-diagnostic.json
```

Four degree-(8,2) charts cover each law. Transverse v is stereographic, not
normalized alpha. New whole-patch position bounds accompany actual immutable
controls; a bounded 256-span cylinder pcurve replaces the native projection
recipe that failed the existing normal gate. Inspect actual sewn controls,
off-knot paired pcurves/normals, 10-face closed topology, ideal-volume bounds and
quarter-turned solid phase controls. These are two actual diagnostic solids;
`candidate_admitted` and `product_body_created` stay false. Physical Mock,
production source and global native G1/embedding are still unadmitted. See
[rational surface scope](../docs/api/periodic-fillet-reference.md#rational-characteristic-circle-surfaces-p16-031).


P16-032 also emits `normal_bounds` for every unchanged rational chart, requesting
1e-9 dimensionless normal error and 4096 complete-cover leaves per chart. Positive
Jacobian bounds use m² and derivative errors metres per original u/v parameter.
The interval proof covers all parameters, including radial/axial support and
quarter seam boundaries. Tight precision/work requests refuse without changing
the controls. Run the same example and native probe above: the latter checks
current native D1/Jacobians/normals against the mathematical certificates with
separate numerical allowances. Global native G1/embedding, production Mock/source
and MCP admission remain excluded; see
[normal certificate API](../docs/api/periodic-fillet-reference.md#rounded-rational-normal-certificates-p16-032).


P16-033 extends the same live native command to capture post-sewing native
Bezier controls in their original millimetres. It passes those current data to
`periodic_fillet_native_snapshot`, a read-only Rust example with 256 KiB bounded
stdin and eight requests. Each complete proof requests position 1e-12 m and
normal 1e-9 with 4096 leaves, includes outward native-to-SI conversion and common
weight-gauge normalization, and echoes its capture/reference/request for grading.
The report certifies underlying mathematical native surface definitions, not
floating evaluation, trims, contact coincidence or whole-BREP fidelity. Missing
or mismatched captures, changed domain/Location, invalid budgets and unresolved
certificates refuse. No new source/MCP operation is introduced; see
[native definition certificates](../docs/api/periodic-fillet-reference.md#current-native-surface-definition-certificates-p16-033).

The live snapshot example also rejects inputs over 256 KiB, an invalid quarter
and an unattainable precision on the last of eight requests. All three must
return failure with no partial certificate stdout, after the same real native
capture; synthetic parser tests do not substitute for these executions.

P16-034 additionally certifies all sixteen CURRENT native side/end 3D contact
curve definitions through `periodic_fillet_native_contacts`. The existing live
command above captures original millimetre poles, current weights and fixed
representation metadata after sewing, requests a complete 1e-12 m position bound
against each ideal contact and binds the echoed request to the capture. The
bounded 64 KiB input contains sixteen requests; even a last-request failure
emits no partial certificate. Report `native_contact_definition_certificates`
summarizes whole-curve position errors. Attached pcurve lifts, full edge-to-face
gaps, native floating evaluation and whole-BREP fidelity remain unproved; this
adds no authored operation. See
[contact snapshot API](../docs/api/periodic-fillet-reference.md#current-native-3d-contact-definition-certificates-p16-034).

P16-035 extends that same live command with whole END/PLANE boundary certificates.
Current attached degree-4 pcurves and raw plane origins/axes are lifted outward
to SI, and current attached blend lines must be canonical (t,1) on [0,1]. The
read-only `periodic_fillet_end_boundary` example recomputes edge/lift/surface
certificates for eight requests within 256 KiB; all three pairwise gap bounds
must meet 1e-12 m. Echoed definitions and budgets bind each proof to its actual
post-sewing capture. `native_end_boundary_certificates` summarizes complete-span
gaps. All eight malformed/precision/work refusal processes emit no partial stdout.
These end-contact definition proofs do not certify cylindrical side contacts,
native floating evaluation, paired normal agreement, exact G1 or whole-BREP
fidelity. See [end boundary API](../docs/api/periodic-fillet-reference.md#current-native-end-boundary-certificates-p16-035).
