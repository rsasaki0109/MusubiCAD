# ADR-100: Guarded grid-faceted periodic cylinder fillet runtime

Status: Accepted for MCAD-P16-047's named numerical runtime representation.
Date: 2026-10-16.
Depends on: ADR-070/071/092–099. P16-008 remains planned; no feature score.

## Decision and representation

Introduce the explicitly named `grid_faceted_periodic_cosine` kernel operation,
separate from legacy fillet and smooth native rolling-sphere representations.
It replaces a full physical cylinder by the complete faceted reference body:
ALL supports, caps and blend are rebuilt as owned planar facets. It promises
neither unchanged analytic supports nor exact G1 contact. This is the separately
named approximation path allowed by ADR-070, not a silent radius-law change.

Require an actual current full-cylinder/rim witness and its body-local end edge
ID, expected SI radius/height/origin, and explicit phase X and positive axis.
Validate finite unit/orthogonal directions BEFORE normalizing; derive a proper
right-handed frame. Check actual origin/axis/radius/height within 1e-9 SI
length / 1e-12 direction tolerance. End means positive frame Z, never BREP seam
or edge enumeration. Numeric native edge indices remain body-local (owner,id).
Source originality and semantic aliases cannot be inferred here; the future
feature must enforce ADR-070's exact authored circle/NewBody/Distance/ownership
and positive authored phase before calling this runtime.

Privately construct the original cosine rolling-sphere reference, its bounded
raw mesh, admitted stored grid and ADR-095/097/099 comparisons. Require explicit
raw/grid position and normal quality, vertex/triangle limits, ideal integral
widths/work and absolute volume/area/per-axis moment/centroid differences. Keep
ALL existing caps unchanged. Invalid input, unreachable quality, work or any
error budget refuses BEFORE inserting an output body. No caller grid/certificate
or ideal mass substitution. Preserve the stock and allocate fresh output facets.
Default legacy Mock refuses; analytic Mock and OCCT construct real geometry.
Counting forwards the same operation. Validate placement before allocation and
insert only the final complete body, with the native proper-placement marker.
Native placement applies the full proper 3x4 SI matrix with gp_Trsf::SetValues,
then rechecks the placed solid validity, self-intersection and actual grid-volume
agreement before insertion. Avoid axis-angle/identity thresholds that discard
a small but explicitly authored phase. Existing general placement is unchanged.

## Precision meaning and native admission

Existing rigorous quality/discrepancy bounds apply to the canonical stored input
grid and its ideal reference. Return an immutable preparation record with the
actual grid/comparisons and proper placement, so callers can inspect that scope.
These bounds do NOT become native, placed or f32 certificates. The result is a
numerical physical kernel solid: existing OCCT one-shell/one-solid/BRepCheck/
BOP self-intersection/volume gates remain mandatory; placement and actual
volume/area/centroid/contact/bounds/facet ownership are checked independently
with explicit numerical allowances in tests. This ADR accepts that numerical
runtime, not an exact global embedding or native floating-point proof.

No source feature is enabled by this ADR. P16-008 must separately fix its closed
serialized tag, approximation and verification semantics, quality persistence,
cache contact/phase/ownership, all-new face groups, competing consumers, strict
old-reader refusal, DesignPatch/transactions/history and independent actual MCP
evaluation. It must not claim the canonical input bounds certify native output.

## Validation and limits

Test constant and genuinely variable laws against independent volume/first-
moment/area formulas or integrals and actual contact points, including rotation,
translation, authored phase reversal, scales and repeated deterministic plans.
Test wrong stock/rim/axis/frame/dimensions, zero/negative/nonfinite parameters,
all precision/work/error failures, unsupported legacy Mock, counting and stock
preservation. Both kernels return actual facets/mass; unsupported cylinder or
analytic support queries on the rebuilt output refuse. Reuse the physical
example/CI report, update API/architecture/guide/skill/plans, and pass mandatory
local and all exact-head CI before merge. No schema, golden, MCP operation/eval
or maturity change. Source admission and native conversion certificates remain
explicit future decisions.
