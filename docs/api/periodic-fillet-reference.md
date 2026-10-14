# Bounded ideal periodic fillet reference

P16-019 implements [ADR-072](../adr/ADR-072-bounded-periodic-fillet-reference.md)
as transient Rust arithmetic. This is an ideal surface reference, not a geometry
kernel body, native approximation, source feature or MCP command.

`CircularFilletReference::new(R, H, r_min, r_max)` takes four Length values.
All must be finite, above 1nm and at most 1000m; require r_min<=r_max<=R/4 and
H>=4*r_max. Getters return these explicit Length values. The local reference
cylinder has base z=0 and axis +Z; its radius is maximal at +X. Real source
placement and phase admission remain P16-008 work.

`radius_at(theta)` returns the cosine radius as Length. `sample(theta,alpha)`
takes Angles with theta in [0,2*pi], alpha in [0,pi/2], and returns:

- `point_m`: the ideal rolling-sphere envelope point in metres.
- `normal`: its dimensionless outward unit sphere/surface normal.
- `theta_tangent_m`, `alpha_tangent_m`: analytic parameter derivatives in metres
  per radian, calculated by private first-order chain rules.

Only the declared theta=2*pi endpoint is canonicalized to zero, retaining the
same values/derivatives at the seam. Other out-of-range angles refuse. The radius
expression is evaluated as r_min+(r_max-r_min)*(1+cos(theta))/2 to avoid cancelling
the small minimum against the mean. No positive amplitude is inferred constant.

`exposure_margin_m2()` returns `(2-7*q+q*q/2)*R^2`, q=r_max/R. ADR-072 proves
that it is at least 9*R^2/32 in real arithmetic and bounds the positive factor in
the moving-sphere residual. This establishes global exposure and regularity of
the ideal surface over the admitted range. Floating-point evaluation is not
an outward-rounded numerical certificate, a mesh error bound or native proof.

`constant_limit_volume_m3()` gives the ideal constant-radius rounded-cylinder
volume only for identically authored minimum/maximum values. An explicitly
unequal pair, even one floating-point step apart, refuses. This original constant-only method retains its refusal semantics. No area/COM
result or backend mass-property substitution is supplied.

See the [executable example](../../examples/periodic_fillet_reference.md).
Kernel solids, bounded meshes/sampling, native parameter/contact/periodicity
admission and guarded source/reader/cache/history/eval remain separate work.

## Bounded nonconstant volume (P16-020)

[ADR-073](../adr/ADR-073-periodic-reference-volume-bounds.md) adds
`volume_bounds_m3(max_width_m3, max_cells)`. The first argument is a finite positive
absolute **full enclosure width in cubic metres**, not a relative tolerance;
the second is an explicit leaf budget in 4..=262144. For example 1e-12 m³ is
0.001 mm³. There is no implicit sampling quality or precision default.

The result `CircularFilletVolumeBounds` contains `body_lower_m3`, `body_upper_m3`,
`removed_lower_m3`, `removed_upper_m3`, and `cells`. It bounds the ideal reference
volume; it does not measure an OCCT solid. The characteristic-circle arc is
integrated analytically, preserving tangential displacement, then one angular
integral is enclosed by directed binary64 interval arithmetic on rational charts.
IEEE-754 nearest-rounded operations and correctly rounded sqrt are assumed;
private adjacent-float helpers preserve Rust 1.78 compatibility. The integrator
uses neither libm trigonometry nor a finite-sample convergence estimate as a bound.

Largest-width cells are bisected deterministically. Invalid domains, insufficient
budgets, stalled subdivision or unattainable precision return an error; no coarse
midpoint is returned as a precise answer. The final width includes stock-volume
and roundoff bounds. Identical authored radii use the enclosed constant formula
with `cells=0`; an explicit positive amplitude retains the nonconstant path even
when tiny. The returned enclosure need not have the same width at different
scales, and the API promises no native fidelity or approximation admission.
