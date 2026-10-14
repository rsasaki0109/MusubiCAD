# ADR-064: Bounded countersink physical cuts

Status: Accepted for MCAD-P16-011 and MCAD-P16-004.
Date: 2026-10-14.

## Decision and admission

Extend ADR-063 with one countersink on unmodified rectangular stock. The
geometry-neutral runtime operation receives an outward entry pick, bore and
seat radii in metres, bore depth and frustum depth in metres. Both kernels must
obtain their actual unique rectangular-stock witness first. Require finite
dimensions in (1e-9, 1000] m, seat radius greater than bore radius by more than
1e-9 m, 1e-9 < seat depth < bore depth - 1e-9, bore depth within stock thickness
(only 64 EPS arithmetic roundoff), and seat disk clearance greater than 1e-9 m.
Native can cut any admitted cuboid face; analytic Mock initially requires
one of its tracked extrusion end faces. P16-004 uses their common admission
and additionally enforces original extrusion end-face/source/semantic ownership
from ADR-063. Unsupported analytic side entry explicitly refuses.
Already-cut, curved, compound, imported or untracked stock refuses. No nearest
face or bounds approximation establishes stock admission. Default Mock refuses.

OCCT privately subtracts a cylinder and a real `Solid::cone(R, r, direction*d)`
frustum, preserving derivation from the original target and publishing only the
final result. No OCCT type escapes. Analytic Mock stores the same two concentric
cuts with linear endpoint radii. The union envelope has exact axial breaks at
the frustum tip, bore end and stock ends; no sampled volume or cylinder seat
approximation. Subsequent generic cylindrical subtraction on tapered stock
refuses until a general intersecting-radius envelope is separately admitted.
Rigid transforms, bounds, mass, face membership and tessellation remain valid.

For a segment of length L with endpoint radii a,b, removed volume is
pi*L*(a*a+a*b+b*b)/3 and first moment relative to its start is
pi*L*L*(a*a+2*a*b+3*b*b)/12. Integrate the union once; the bore inside the
frustum is not counted twice. Lateral area is pi*(a+b)*sqrt(L*L+(b-a)^2).
Axial caps use endpoint radius jumps; subtract entry/exit disks from stock area.
Independent tests derive the full countersink union and world first moments.

## Mesh and topology

Keep ADR-063 angular grid, exact corner anchors, face-bearing output, f32
coordinate/error checks, closed opposite edge pairing and all budgets. For a
conical wall, pair circle rings at its two endpoint radii with straight
generators. These generators are exact; circumferential error is bounded by the
largest radius sagitta plus coordinate error. The void-facing wall normal is
proportional to minus radial plus radius slope times the stock axial direction.
Frustum endpoint radii are part of a distinct runtime wall identity. Preserve
original six outer-face IDs; new cone/cylinder/blind faces have new body-owned
IDs. Do not invent a planar shoulder at the continuous cone/bore junction.
No new triangulation dependency, relaxed quality, retry or general cone API.

## P16-004 source contract and compatibility

Add the closed `countersink` seat discriminant to `seated_standard_hole`, with
required diameter Length in metres and included_angle Angle in radians, plus
optional diameter_expr and included_angle_expr. Derive frustum depth as
(seat_radius - bore_radius)/tan(included_angle/2). Require finite 0 < angle < pi
and all geometry margins above. Explicit angle expressions use angle evaluation;
length fields never accept angle units. No implicit fastener-head sizing.
Counterbore fields and behaviour remain unchanged. A counterbore-only decoder
must reject the countersink discriminant rather than silently model a bore.
Update both schemas, additive no-inference migration, packed/expanded roundtrip,
patch/dependency/cache handling and reader-refusal tests together in P16-004.
Source admission, physical postconditions, retained-face ownership, mandatory
host verification and protected candidate/history behaviour reuse ADR-063.

## Validation and limitations

P16-011 proves independent volume/area/COM/bounds and mesh closure/winding/face
membership in both kernels, blind/through/both entry ends, rigid placements and
invalid dimensions/ownership/quality/unsupported stock. Existing cylindrical
chains retain their tests. It changes no .ocad schema and earns no feature or
host score. P16-004 supplies the complete source/feature, example, guide, skill
and independently graded MCP eval before countersink feature credit.
