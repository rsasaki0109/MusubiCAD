# ADR-065: Bounded cylindrical-stock draft

Status: Accepted for MCAD-P16-012 and MCAD-P16-005.
Date: 2026-10-14.

## Context and decision

ADR-036 admits convex planar half-space reconstruction. A curved side has no
single face centroid on its surface or constant normal; mesh discoveries split
one native side ID across directional roles. Removing the planar guard cannot
supply correct curved draft. ADR-064's conical hole removal does not provide a
standalone solid frustum or cylinder stock witness.

P16-012 adds a kernel-neutral full-cylinder witness, oriented by a finite unit
axis hint. Verify an actual unmodified closed right circular cylinder, not a
bounds or mesh approximation: one valid solid, one complete cylindrical lateral
surface, two circular planar caps, two circle edges and one seam, no extra
wires/holes/cavities/compounds. Outward orientation uses actual analytic surface derivatives plus face
orientation, allowing indirect cylindrical parameter frames. Native exact
surface/curve/topology checks stay
in the private BREP bridge; no OCCT types or graph ownership escape. Analytic
Mock queries only its actual tracked Cylinder record. Default Mock refuses.
The witness returns SI origin, oriented unit axis, radius, height and body-owned
start/end/lateral face IDs. A hint parallel or antiparallel to the actual axis
must agree within 1e-9 dimensionless; it selects orientation, never nearest shape.

The transient draft operation receives that axis hint, a neutral plane origin
in metres, unit normal and signed angle in radians. Require plane normal
parallel or antiparallel to cylinder axis (1e-9), neutral axial coordinate z0
inside [0,H] with arithmetic roundoff only, and angle magnitude 1e-8..pi/4.
Positive angle narrows toward the neutral normal. With sign = dot(normal,axis),
k = sign*tan(angle), radius r(z) = R-k*(z-z0), so the circle on the neutral
plane stays fixed. Both endpoint radii and all dimensions must be finite in
(1e-9,1000] m, within the established world bound of 1e6 m. Endpoint radii must differ by more than 1 nm; unresolvable taper refuses.
No clamping input,
partial side, oblique plane, curved neutral surface or already-tapered stock.

Native constructs an actual private OCCT cone at the witnessed origin,
through an exception-catching bridge with BREP exchange. This is exact for the admitted full lateral-face family, not a sampled
or planar approximation. Analytic Mock records a solid frustum separately from
cylindrical cutters; generic Booleans/shape edits on this new record refuse.
Inputs remain immutable and no output handle is published after failed admission.
Reconstruction gives new body-owned face IDs and no invented ancestry. The
source feature later preserves semantic neutral-cap ownership only where actual
unchanged geometry proves it; changed side/caps belong to the draft producer.

## Exact measurements and mesh admission

For H and endpoint radii a,b, volume = pi*H*(a*a+a*b+b*b)/3;
axial COM = H*(a*a+2*a*b+3*b*b)/(4*(a*a+a*b+b*b)).
Area = pi*(a*a+b*b) + pi*(a+b)*sqrt(H*H+(b-a)^2).
Bounds use endpoint-circle support in each world coordinate. Independent tests
also integrate removed/added annular slices and exported triangles; neither
kernel matching the other nor a mass-only assertion is sufficient.

Extend ADR-063's existing deterministic angular grid and largest-radius sagitta
plus actual f32 coordinate error checks to solid frustums. Endpoint circles are
paired by straight exact generators; outward wall normals are proportional to
radial minus radius slope times axial direction. Disk caps have opposite axial
winding. Keep 8192 samples / 100000 vertices / 200000 triangles, strict angular/
linear settings, degeneracy/closed opposite-edge/positive-volume checks and
no looser retry. Circle contact and conical slope must match actual vertices.
Rigid placements and either axis-hint orientation are supported and tested.

## P16-005 source and failure contract

Use a distinct closed `cylindrical_draft` feature tag; do not add an ignored
optional curved flag to legacy `draft`. Require target, owned lateral semantic
reference, signed SI Angle/optional angle expression and explicit neutral plane.
The target must be original exact-circle distance/NewBody extrusion; compare
source circle/origin/axis/height with actual witness. Imports, fusions, patterns,
cut/tapered/mixed-profile stock refuse even if a shape looks cylindrical.

The witness establishes the actual lateral ID on the target body. Validate
creator, missing/foreign/consumed/ambiguous/stale reference and optional physical
fingerprint/normal annotations against the actual side, not a mesh average or
nearest face. The changed side is consumed; output discoveries identify actual
conical side and caps with explicit producers, including any unchanged neutral
cap by physical plane/radius proof. Cached outputs repeat source/physical checks.
Strict angle units/reference kinds, schema/migration/roundtrip and genuine
old-reader refusal accompany source introduction. Mandatory candidate geometry
verification, source/history/artifact rollback and canonical Undo/Redo use the
existing backend DesignPatch/transaction gateways. No default kernel switching.

## Acceptance, alternatives and limitations

P16-012 tests independent volume/COM/area/bounds, endpoint/contact/neutral radius,
closed oriented meshes/face membership, signed angles, endpoint/interior planes,
axis reversals, rigid placements, invalid units/ranges/axes/planes/quality,
unsupported shapes and original-stock preservation in both kernels. It changes
no .ocad schema and earns no feature/host credit. P16-005 additionally supplies
source/patch/semantic/history, example, guide/skill and independent MCP eval.

Retain ADR-036's planar operation unchanged. Do not approximate a cylinder by
half-spaces, use average normals, introduce general NURBS draft, relax sampling
quality, or claim arbitrary curved/concave-face support from this one family.
