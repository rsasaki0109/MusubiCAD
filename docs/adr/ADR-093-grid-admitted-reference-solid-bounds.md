# ADR-093: Preserve reference bounds through grid solid admission

- Status: Accepted
- Task: MCAD-P16-040
- Depends on: ADR-049, ADR-092; prerequisite to still-planned P16-008

## Context

ADR-092 certifies an immutable SI triangle shell before subsequent conversions.
TriangleSolid admission uses an existing 1e-9 m grid and numeric closed-shell,
vertex-fan, intersection and positive-volume checks. Those checks and their
10000-triangle/1000000-candidate budgets must remain unchanged. Passing the
unquantized certificate alone cannot certify the stored grid coordinates.

## Decision

Add a read-only conversion from the trusted immutable reference mesh to a
private-field certificate carrying the actually admitted TriangleSolid. Require
finite positive final position precision in metres and final unit-normal chord
precision strictly between zero and one. Check size and original bounds before
admission; return no partial certificate or relaxed quality on any failure.

Use the existing TriangleSolid constructor, then require equal face/vertex
counts and a bijection between original and admitted vertex IDs derived from
ordered face/corner correspondence. Each original vertex maps to exactly one
stored vertex, and each stored vertex has exactly one original preimage. No
new tolerance weld, correspondence search or repair is permitted.

For actual binary64 original p_i and admitted q_i, outward interval arithmetic
bounds D = max_i ||q_i-p_i||. For each corresponding pair of oriented triangles,
compute both exact mathematical unit normals from outward cross-product,
squared-norm and square-root/division enclosures; require strictly positive
lower squared norm. Bound E = max_faces ||n_q-n_p|| outward. Then the final
whole-triangle position bound is old_position + D and the final complete
normal chord bound is old_normal + E, each added outward. Barycentric
interpolation proves the position displacement bound over the entire facet;
the facet normal is constant. The original parameter correspondence and ideal
reference qualification remain as in ADR-092. Require both final bounds within
the caller's precision. A final normal bound less than one also preserves
positive alignment to the ideal unit normal. Never authorize from witnesses.

The certificate retains read-only access to the admitted solid and displacement,
normal-change and final error bounds. Mass properties describe that actual grid
solid, not the ideal reference. The existing numeric intersection validator is
not promoted to an exact global embedding proof. Rigid transforms, f32/STL
serialization, native floating evaluation and all subsequent conversions are
outside these bounds.

The pinned OCCT diagnostic additionally constructs the actual admitted grid
facets, checking the existing topology, current vertices/planes/orientation and
mass measurements. An explicit grid transport mode allows at most 1e-9 m
stock-bound expansion at input (raw mode retains 1e-12 m); it does not relax
native matching/sewing or comparison tolerances. Run the four real refusal
controls on both modes, with no partial stdout. Retain original raw diagnostics.

## Validation

Independent integer-grid determinant volume, actual-triangle area/centroid,
original-versus-grid vertex and facet-normal differences, unchanged topology,
determinism, scales and final-quality/work-budget refusal. Synthetic Python
mutation controls are separate from actual native construction and must reject
unbound counts, metadata, maps, quality, units and mass. Run required workspace
checks and pinned native probe; update API, architecture, example, MCP guide and
skill. This is a conversion prerequisite, not a new authorable feature, so no
Mock kernel operation, source schema, migration, golden or MCP eval is added.

## Consequences and risks

Admission is bounded by unchanged STL limits: n=32 (16640 triangles) must refuse,
even if the reference mesh fits its independent larger work budget. The caller
may generate a finer reference separately but this conversion never silently
coarsens or retries. Grid movement can merge/degenerate/intersect facets and must
refuse. Error sums can exceed a previously adequate raw precision. No new
source/history/cache/Mock/runtime or editable NURBS admission; no maturity score.
