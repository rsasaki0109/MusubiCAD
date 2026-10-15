# ADR-095: Bound actual grid reference volume differences

- Status: Accepted
- Task: MCAD-P16-042
- Depends on: ADR-073, ADR-093, ADR-094; prerequisite to P16-008

## Context

Whole-facet coordinate/normal bounds do not by themselves certify volume error.
The actual grid body has a different volume from its ideal rolling-sphere
reference. Native measurements must never be replaced with ideal mass. ADR-073
already encloses the ideal volume; actual stored grid coordinates allow an
independent exact signed boundary-volume flux enclosure.

## Decision

Privately retain the original CircularFilletReference in the immutable raw mesh
and resulting grid solid. No caller-supplied reference, mass or external error
metadata can be paired with an unrelated mesh. This changes no serialized source.

Add CircularFilletGridSolid::volume_comparison with explicit finite positive
maximum_reference_interval_width_m3 and maximum_absolute_difference_m3 plus the
existing ADR-073 max_cells budget. Invalid precision or cell count refuses
before integration. Use ADR-073 without changing its accuracy/work rules.

For each stored oriented grid triangle (a,b,c), outward interval arithmetic
computes det(a,b,c)/6 and sums all terms to bound its exact mathematical signed
volume flux F (m³). Require a finite positive lower flux bound. The privately
bound ideal reference produces V=[v_lo,v_hi] with v_lo>0. Outward subtraction
D=F-V encloses the signed difference, and max(abs(D.lo),abs(D.hi)) bounds its
absolute value. Outward division by v_lo gives a relative difference bound.
Return an immutable certificate with getters for F, V, D, absolute/relative
bounds and reference cell count only if the requested absolute discrepancy is
met. No midpoint, sample refinement or native mass authorizes a result. No
partial comparison, mesh mutation, coarsening or alternate reference fallback.

The result certifies a signed boundary-integral difference. It does not upgrade
TriangleSolid's numeric intersection admission to exact global embedding, certify
native/f32/transformed geometry, ideal area/first moments or production source
fillets. Physical volume interpretation retains existing oriented embedded-shell
qualifications. Backend mass remains its actual measured grid/BREP value.

## Validation

Independent integer-grid determinants, analytic constant-limit ideal volume,
variable-law reference interval/refinement and scale-cubed units. Check finite
complete intervals, conservative subtraction/absolute/relative bounds,
reference binding, invalid budgets/precision and insufficient discrepancy refusal
without changing geometry. Exercise both physical kernels on the actual input
grid certificate and compare their actual volume to independently computed grid
mass, retaining numeric backend tolerances separately from the integral proof.
Update example, API, architecture, MCP guide and skill; run all required checks.

## Consequences and risks

A tighter reference interval cannot repair a genuinely excessive grid discrepancy.
The raw 10000-triangle/intersection caps stay unchanged; no quality coarsening or
new sampling rule is introduced. Interval dependency can conservatively refuse.
Input reference binding and exact stored binary64 vertices are essential; no
certificate is inherited by a placed or edited body. No source/schema/golden,
new MCP operation/eval or maturity credit; P16-008 remains planned.
