# ADR-079: Closed native reference approximation diagnostic

Status: Accepted for MCAD-P16-026's isolated construction diagnostic only.
Date: 2026-10-15.

## Decision and distinction

Close ADR-078's actual native band with explicit polynomial side skirts and
planar top/bottom caps, then measure valid closed native solids at two refinement
levels. This is a separately named diagnostic approximation. The skirt's radial
curve is the band's polynomial side-contact curve, not an exact cylindrical
support. It changes the side's representation and cannot satisfy ADR-070's
unchanged-stock/support-trimming gate. Do not replace product cylinder geometry,
enable the fillet source, redefine the radius law or claim physical Mock support.
P16-008 remains planned; no feature or maturity credit is earned.

## Construction

Use actual Rust-generated constant/nonconstant periodic bands. At each u cell,
the alpha=0 boundary is a cubic curve A(u). Construct a bicubic ruled skirt with
poles (A_x(i), A_y(i), j*A_z(i)/3), j=0..3. Its top edge is exactly A; its bottom
lies in z=0. Shared skirt vertical edges agree at neighbouring u endpoints.
This finite polynomial construction neither projects to the cylinder nor heals
an off-support curve into an exact analytic support.

Build ordered cubic native boundary edges from the band top and skirt bottom.
Use those closed wires for planar caps at H and zero. Set the bottom cap's
orientation explicitly outward. Convert SI poles to millimetres explicitly;
keep the diagnostic sewing tolerance 1e-6 mm (1e-9 m). Sew all band/skirt/cap
faces, require a single shell, then construct one solid. Never silently reverse
an invalid/negative-volume result or remove missing faces to make tests pass.

## Native measurements

For n dyadic divisions per chart axis require:

```
faces = 4*n*n + 4*n + 2
vertices = 4*n*(n+2)
edges = 4*n*(2*n+3)
Euler characteristic = 2
```

Check actual native shell/solid validity and closure, one connected face graph,
exactly two planar caps, zero free edges and exactly two distinct incident faces
per edge. Require positive finite native volume. Measure native volume using
two integration epsilons and preserve the refinement delta separately; this
delta is evidence of numerical stability, not a certified native mass bound.
Check actual band/skirt surface values against independent polynomial evaluators
and measure the support deviation introduced by the skirt at interior points.
Do not inspect only stored pole metadata or reuse ideal mass as native output.

Use two explicit position requests producing successively finer complete bands
and the same declared normal/work budgets. Compute each constant/nonconstant
ideal volume interval through the actual ADR-073 API with matching radii.
Report signed native-minus-ideal volume difference intervals. These intervals
locate the native estimate relative to a bounded ideal reference; they are not
bounds on all native integration error or solid fidelity. Require numerical
stability and refinement toward the ideal reference for these diagnostic
fixtures, without choosing a manufacturing tolerance or asserting general
convergence from two samples. Persist measured results rather than assuming a
native approximation equals its reference.

## Tests and limits

Run the actual pinned OCCT diagnostic in CI. Add report-contract negative tests
for missing/unclosed topology, bad orientation/volume, altered surfaces,
inconsistent ideal reference and absent/refinement failures. Synthetic reports
are parser tests only. Update API/architecture/example/guide/skill/plans and all
mandatory local/exact-head CI checks. No production GeometryKernel operation,
Mock analytic record, .ocad source/schema, migration, golden or MCP eval applies.

Remaining admission gates include exact or explicitly accepted support fidelity,
G1/normal continuity at supports and seams, global embedding, independently
bounded solid area/volume/first moments, physical Mock/native runtime/meshes and
the guarded source/cache/history/reader/eval contract. A valid closed diagnostic
solid alone cannot admit the circular variable-fillet feature.
