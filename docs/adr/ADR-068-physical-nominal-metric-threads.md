# ADR-068: Physical nominal metric threads and bounded screw meshes

Status: Accepted for MCAD-P16-015 runtime; source P16-007 remains pending.
Date: 2026-10-14.

P16-016 / [ADR-069](ADR-069-guarded-nominal-thread-source.md) now defines the
separate source admission before implementation. It does not expose a source
feature or expand this runtime's physical admission.

## Admission and representation

Implement ADR-067's nominal metric basic flat-root screw envelope, not rounded
production roots, fit classes or a generic spring. Runtime consumes an actual
uncut full cylinder witness whose major diameter/height match the supported
coarse size and 1–32 whole pitches within 1nm. An explicit unit axis hint selects
the actual axis sign, and a unit perpendicular radial axis fixes the phase-zero
crest. Use the actual cylinder origin/axis and a proper frame. No stock inferred
from bounds, mesh or cached IDs. Runtime geometry cannot prove authored provenance;
P16-007 separately requires original circle/distance/NewBody source and owned side.
Reject nonfinite/nonunit/improper frames, unsupported bodies, dimensions, world
coordinates beyond 1e6m and unresolved precision. Source bodies remain immutable.
All new output surfaces have new identities; no fabricated reconstruction history.

## Native construction

Construct in local millimetres with an explicit SI conversion, avoiding OCCT's
absolute default tolerances at millimetre-sized SI geometry. Build one helix edge
per full revolution, connected at exact phase/height; one long multi-turn edge
failed independent Boolean/phase tests despite valid topology. Cylindrical
support/2D line gives a right-handed spine. BuildCurve3d tolerance 1e-10mm,
C1, degree14, max2048 segments; raw Frenet pipe with no profile contact/correction,
3D/bound tolerances1e-9mm, angular1e-8, degree14/max2048. Valley trapezoid has
root flat P/4 and major intersection width7P/8; extrapolate slopes 2um past the
major cylinder, so the tool's artificial outer face cannot become thread material.
Overrun at least a pitch on each end, subtract from the major cylinder, clip to
z=0/L. Non-destructive deterministic operations, consistent edge parameters,
oriented valid single solid and independent properties are required before a
handle is published. Catch OCCT exceptions; no cylinder/sweep/looser retry fallback.

Left hand is a reflection of the proved local right-hand solid across Y=0,
then checked orientation/validity; direct negative-angle Frenet construction
matched volume but failed the actual phase. Reflection constructs the local
shape; subsequent source/output placements are always proper rigid transforms.
Convert back to SI and place in the explicit frame. Native volume/area tolerance
2e-5 relative accounts for its default integration, COM tolerance20nm plus
coordinate roundoff. Independently grade actual radial boundary/mesh/phase;
volume agreement alone cannot admit the construction. No OCCT type escapes.

## Analytic Mock and physical mesh

Store a separate immutable physical thread record/frame, exact ideal volume,
area and COM, not body-ID arithmetic or measured volume of an unrelated primitive.
For a=P/(2pi), F(r)=r*sqrt(4r²+3a²)/2+3a²/4*asinh(2r/(sqrt(3)*a)),
the side area is 2pi*L/P*[P*(root/4+major/8)+2/sqrt(3)*(F(major)-F(root))].
Add twice ADR-067's section area. Unsupported downstream queries/edits refuse.

Parametrize each linear-profile band as
S(u,q,k)=(r(q)cos(2pi*u),r(q)sin(2pi*u),P*(q+s*u+k)),
s=±1, 0≤u≤1, q knots 0,1/16,3/8,5/8,15/16,1. Angular count N is divisible16.
Clip the exact parameter rectangles to 0≤q+s*u+k≤turns. All intersections have
integer coordinates on the 1/N grid: cap lines have slope±1 and integer intercept.
Canonical key (angle_index modulo N, axial_index) merges only exact mathematical
identities. Reconstruct position from that key; no proximity welding. Positive
UV winding gives outward sides. Opposite cap fans follow the actual noncircular
contour. No internal faces, duplicate seam or T-junction. Six Mock physical
classes (two caps, crest, root, two flanks) do not invent native face cardinality.

Linear error for a side cell is at most
E_side=major*pi²/(2N²)+2pi*(5sqrt(3)P/16)/(4N).
Taylor's remainder uses |S_uu|≤(2pi)²major, |S_uq|≤2pi|r'|, S_qq=0,
weighted variance≤interval²/4 and mixed absolute covariance≤product/4.
For the cap contour, r is linear in angle within every anchored band;
E_cap≤[(2pi)²major+4pi*sqrt(3)P]/(8N²). Fan interior scales this boundary
interpolation by a factor≤1. Add actual f32 position error to max(E_side,E_cap).
Select N monotonically in steps of16, minimum64; use at most half the linear
budget for interpolation, reserving the rest for actual representability.
Normal change across a flank's radial range is bounded by
B=sqrt(3)*a*(1/root-1/major)/2. Require 2*(2pi/N+B)≤angular deflection;
requests below this unsubdivided-band angular floor refuse explicitly.
No author-input clamping. Bound N≤8192, vertices≤100000, triangles≤200000;
preflight and actual output checks, finite nondegenerate correctly wound f32
triangles, paired opposite geometric edges and positive integrated volume.
Insufficient quality/coordinate resolution/budget refuses with no looser retry.
Mesh integrals are approximation checks with their declared error, not ideal
record mass; tests independently integrate actual triangles and verify convergence.

## Bounds, placement and tests

A tilted thread's exact bounds differ from full-cylinder bounds. At fixed angle,
support is linear in q within each band, so extrema lie on knot helices or end
contours. Helix support derivative is an elementary sinus plus axial constant;
include its roots and clipped endpoints. Cap support derivative is
r'(angle)*(dx*cos+dy*sin)+r*(-dx*sin+dy*cos). Each supported band has positive
curvature numerator r²+2r'² and tangent rotation less than pi (coarse ratios
bound the radial-slope correction), hence at most one critical support direction.
Include endpoints and any bracketed derivative root with fixed64 bisections.
No unregistered general sampling scheme or stock-box surrogate. Validate proper
frames again after transformations; compare independent meshes/native tight
bounds, including tilted placements and both hands.

Independent tests use meridional pitch/width/radii, handed phase, section/piece
integrals, exact area/COM, actual mesh integrals/edge pairing, body-owned IDs,
stock immutability and invalid body/unit/axis/precision/quality refusal. Examples,
API/architecture/guide/skill document runtime only. No source/schema/MCP operation
or eval yet: P16-007 must supply closed source, semantic/cache checks, schemas/
migration/old-reader refusal/roundtrips, DesignPatch/history and actual eval.
This runtime infrastructure earns no maturity increment.

Native thread tessellation uses the same bounded quality preflight, half linear/
angular budgets for actual OCCT meshing and the remaining budgets for measured
f32 position/normal errors; reject actual output budget/degeneracy failures.
Mock reserves the analytic normal-cone remainder for actual f32 facet rotation.
Global proper rotations use transform_body; legacy direct rotate/mirror entry
points refuse on these runtime records instead of losing physical metadata.

## Native integration precision refinement (accepted before implementation)

The 32-pitch M6 test measured a 31.675nm COM error with default native quadrature,
outside the declared20nm bound, while volume/area were within2ppm. Keep the
acceptance bounds. Use OCCT's adaptive BRepGProp volume/surface integration with
explicit relative epsilon1e-9 for thread construction and thread-containing mass
queries. These measure the actual BREP, not the ideal-profile helper; other
legacy bodies retain their existing path. No native integration precision is
silently loosened or replaced by an analytic cached value.
