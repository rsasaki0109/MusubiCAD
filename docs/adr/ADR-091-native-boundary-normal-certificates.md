# ADR-091: Bound current native paired boundary normals

Status: Accepted for read-only paired-normal diagnostics only.
Date: 2026-10-15.

## Decision and scope

Register P16-038 before implementation. After P16-035/036/037's position bounds,
certify corresponding-parameter unit-normal CHORD gaps on the current native
blend's four side contacts, four end contacts and four quarter seams per law.
Use original captured definitions, never externally supplied certificate metadata.
This is a finite error bound, not exact G1, native floating evaluation, global
embedding, untouched support seams or whole-BREP/production admission. P16-008
remains planned; no Mock/runtime/mesh/source/schema/editable NURBS/MCP operation
or maturity credit is admitted.

## Complete bound and fixed work contract

A case method consumes a validated reference, four immutable current surface
snapshots in chart order, four actual cylindrical-pcurve/support snapshots and
the current end plane's dimensionless X/Y directions. Recompute each full surface
certificate ONCE using ADR-086 and each whole cylindrical lift using ADR-089.
Explicit precision includes finite positive SI maximum_position_error, finite
normal chord maximums strictly in (0,1), and 1..=16384 leaves per surface. Require
all twelve boundary gaps below maximum_normal_gap; no partial case result.
The native fixture requests 1e-12 m, surface normal error 2.5e-10, pair gap 1e-9
and 4096 leaves per surface. This resolves the old 1e-9-per-face certificates'
insufficient budget for pairing. No new sampling/refinement policy is introduced.

ADR-085 proves ideal normals radial at v=0, axial at v=1, and identical at adjacent
quarter seams in the same original transverse parameter. For actual support
axes, let dx=norm(X-ex), dy=norm(Y-ey), dz=norm(Z-ez), evaluated outward from raw
dimensionless binary64 data. Plane X cross Y differs from ez by at most

    dp = dx + dy + dx*dy.

For cylinder normal proportional to (-sin(phi)*X+cos(phi)*Y) cross Z, its
unnormalized direction differs from the canonical radial unit normal by at most

    dc = (dx+dy)*(1+dz) + dz.

Require finite d<1. Its norm is at least 1-d>0, and normalization changes its
unit-normal chord distance from the canonical normal by at most 2*d, by the
reverse triangle inequality. This proves actual support orientation/regularity
within this conservative near-canonical frame contract; arbitrary frames can
refuse even when geometrically equivalent. Plane origin and positive cylinder
radius do not affect normal direction. Cylinder position/radius validity is still
rechecked by ADR-089, and the existing position-boundary proofs remain mandatory.

Let N[c] be the recomputed current surface normal bound and ea[c] the recomputed
current cylindrical pcurve's whole angle error. The global unit-circle Lipschitz
bound yields outward support/boundary bounds

    cylinder_normal[c] <= 2*dc[c] + ea[c]
    side_gap[c] <= N[c] + cylinder_normal[c]
    end_gap[c] <= N[c] + 2*dp
    seam_gap[c] <= N[c] + N[(c+1)%4].

All operations, norms, conversions and sums are outward. Refuse invalid chart/data,
nonfinite/overflow arithmetic, unresolved orientation, precision or exhausted
work. No externally passed bound or finite normal sample is proof evidence.

## Capture, transport and validation

Reuse current post-sewing raw surface/plane/cylinder/pcurve capture without native
construction changes. Require forward owning faces and identity Locations.
Existing fixed types/degrees/bases/unit ranges and positive/unit weights remain
qualified. Bind side/end blend lines as (t,0)/(t,1), seam lines as (1,t)/(0,t)
with chart c to (c+1)%4; original corresponding parameters cannot be inferred from
samples. A bounded 1 MiB read-only Rust example consumes exactly two case requests,
qualifies charts/weights/lines, echoes every raw input and precision/work budget,
and emits only after BOTH complete case proofs succeed. Python binds binary64
echoes including signed zero and validates inventory, components and outward
sums. Existing surface/edge/position/topology/volume/phase/numerical-normal gates
remain mandatory. Actual paired native normal witnesses must also lie within
these new bounds plus the existing explicit floating-evaluation allowances.

Independent tests compare actual rational derivative normals and analytic support
normals across laws/charts/scales, frame changes and angle deformations; include
paired budget failure, bad units/chart/precision/work/data and orientation/overflow
refusal. Actual native proof and real subprocess refusal controls must succeed;
synthetic report tests validate parsing only. Update API, architecture, example,
MCP guide, skill and plans; run all four mandatory local checks before push and
all exact-head applicable workflows/full job inventories before authorized merge.
No schema/migration/golden or runnable MCP eval applies to a read-only diagnostic.
