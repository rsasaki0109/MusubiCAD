# ADR-083: Investigate native trimming on unchanged contact supports

Status: Accepted for diagnostic construction only.
Date: 2026-10-15.

## Decision

Use ADR-082's actual rational contact edges to construct four retained cylinder
quadrants and one retained top-plane face per constant/nonconstant law. Keep the
original analytic cylinder radius/frame and top-plane height. Do not replace
the cylinder by ADR-079's polynomial skirt. These are separate diagnostic faces,
not a sewn shell, blend, physical Mock/product body or source feature. P16-008
remains planned, with no maturity credit.

## Native projection and units

The rational quarter parameter t and cylindrical azimuth u obey
u=chart*pi/2+2*atan(t); thus a rational 3D contact does not supply an exact
rational cylindrical pcurve. Use pinned OCCT 8.0.0 GeomProjLib::Curve2d on
explicit quadrant UV bounds, attach the actual returned pcurve to the original
3D edge, then force BRepLib::SameParameter. Null projection, native failure,
invalid face or inflated tolerances refuse. All native lengths are millimetres;
reports and comparisons use metres and square metres, and UV angles are radians.

Request 1e-7 mm projection tolerance. Require actual returned projection and
all edge/vertex tolerances at most 1e-6 mm (1e-9 m). Check current attached
pcurve/3D correspondence at fixed witnesses with a separate 1e-9 m allowance;
check actual poles/weights remain ADR-082's within 1e-14 m / 1e-14 relative
weight allowance. These are native numerical observations, not an outward
whole-curve projection or trimming certificate. Do not reinterpret OCCT's
achieved-tolerance output or SameParameter flag as such a certificate.

## Retained regions and independent oracles

Bound each cylinder quadrant by the actual side contact, bottom circular arc
and two axial generators. Bound the top face by the actual closed end contact
wire. Require actual native cylinder/plane support types and dimensions/frame,
valid forward faces, one closed four-edge/four-vertex boundary per face and no
shell or solid. Inspect every boundary's pcurve/range/flags and tolerances,
including endpoints. Classify interior and removed-region points on each
support using the current face; measure current contact point and support
normal at fixed witnesses. The top-plane retained region contains the axis.
Wire construction may copy edges while merging vertices. Identify current
contact geometry by its actual curve handle, then verify its current poles,
weights, degree, knots, ranges and positions against the input. This does not
assert retained edge TShape identity or establish semantic ownership.

For r=m+a*cos(theta), independently calculate retained areas:

    cylinder quadrant = R*((H-m)*pi/2-a*(sin(u1)-sin(u0)))
    top plane = pi*((R-m)^2+a^2/2).

Compare actual native surface integration to those values with an explicit
1e-10 m² numerical allowance; report the observed residual. This is not a
certified native area bound. Preserve the authored maximum-radius +X phase;
classify fixed physical witnesses on actual quarter-turned faces to distinguish
constant control from a variable-law phase decoy.

## Validation and limits

Run live native construction in CI and save its report. Synthetic report tests
exercise refusals for missing/invalid inventories, supports, pcurves, tolerances,
regions, areas and phase controls; they are not native geometry evidence.
Update API/architecture/example/MCP guide/skill/plans and run all mandatory
local and exact-head CI gates. No .ocad schema, golden, MCP operation/eval or
editable NURBS is added. A compatible blend must be rebuilt against these
contacts and establish new position/normal/G1, embedding, physical Mock/runtime,
mesh and guarded ownership/cache/history/reader/source proofs before admission.
