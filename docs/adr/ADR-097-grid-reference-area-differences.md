# ADR-097: Bound grid reference surface-area differences

Status: Accepted for MCAD-P16-044's immutable reference comparison only.
Date: 2026-10-16.

## Decision and proof

Add `CircularFilletGridSolid::area_comparison(precision)` using its privately
bound original reference. The caller supplies a finite positive ideal total-area
full width in m², a finite positive maximum absolute area difference in m² and
4..=262144 reference leaves. No default, alternate reference or coarsening.

For every ACTUAL stored binary64 triangle (a,b,c), enclose
`|cross(b-a,c-a)|/2` with the existing private outward arithmetic, square and
correctly rounded sqrt. Sum in stored triangle order. This gives A, the unsigned
parametric triangle area, counted with multiplicity. Use ADR-096 to obtain the
ideal complete-body area interval S, including retained supports. Compute
`D=A-S`, `E=max(abs(D.lo),abs(D.hi))` and an outward upper bound `E/S.lo`.
Require finite valid strictly positive A/S lower endpoints and E within the
requested absolute budget. Invalid/unresolved facets, arithmetic, precision,
work or discrepancy refuse without a partial comparison or any mutation.

This bounds mathematical stored-grid area minus ideal area. It does NOT establish
exact global embedding or area of a geometric union if facets overlap. Existing
TriangleSolid admission is unchanged. No native/placed/f32 conversion, kernel
floating evaluation or independent ideal first-moment certificate follows.
Kernel mass properties remain actual; never replace native area with S or A.

## Validation and deliverables

Test an exact dyadic rectangular box area using integer endpoint comparisons,
constant analytic reference areas and SI scales, independent grid triangle areas,
variable reference binding, finer-grid convergence, deterministic results,
invalid precision/work and real excess discrepancy, inclusive error-budget
acceptance and preservation on failure. Exercise actual Mock/OCCT area queries
with a separate explicit numerical conversion allowance, not a native proof.

Extend the existing both-kernel example/CI JSON with area comparison bounds and
scope flags. Update API/architecture, examples, MCP guide, skill and canonical
plans. No source/schema/migration/golden or MCP-operation/eval changes. P16-008
remains planned; source ownership/phase/history/readers, native approximation
admission and ideal first moments remain separate. This prerequisite earns no
score. All mandatory local checks and exact-head applicable CI gate merge.
