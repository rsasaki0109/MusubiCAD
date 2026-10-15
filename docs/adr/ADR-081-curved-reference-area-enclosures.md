# ADR-081: Curved reference area enclosures

Status: Accepted for MCAD-P16-028's mathematical diagnostic only.
Date: 2026-10-15.

## Decision and scope

Enclose the parametric area of ADR-079's actual mathematical polynomial band
and ruled skirts. Exclude the two planar caps explicitly. This is a transient
reference calculation and actual native diagnostic, not a physical Mock/body,
mesh or source feature; P16-008 remains planned and earns no maturity credit.
Area with multiplicity does not prove global embedding or an admitted solid.

## Bound and refinement

For a polynomial patch S, form the tensor Bernstein coefficients C_ij of
C=S_u cross S_v using outward operations. Each component has degree (5,5).
The triangle inequality and nonnegative partition of unity give

    norm(integral C) <= integral norm(C)
                    <= sum(norm(C_ij))/((p+1)*(q+1)).

Enclose all coefficient arithmetic and Euclidean norms outward. The lower
bound uses the norm of the integral vector, not the integral of a lower hull
coordinate. No sampled quadrature or native area supplies these bounds.
Reuse ADR-080's private Bernstein arithmetic and exact shared-band validation;
mutable positional/normal metadata never serves as the area certificate.

If the summed interval is too wide, subdivide the cross-product polynomial by
outward de Casteljau at exact halves in both coordinates. Its four children
carry one-quarter parameter-area weights. These are restrictions of C, not
new geometry or a mesh. Deterministically refine the leaf with largest interval
width, breaking ties by insertion order. Maintain outward endpoint sums without
subtracting whole uncertainty intervals. Explicit positive width (m²) and bounded
leaf budget (at most 262144) are mandatory; no partial/estimated acceptance when
budget or numerical precision cannot certify the requested width. Initial
leaves cover all actual band/skirt patches; input validation remains bounded.

The skirt's actual mathematical poles are (A_x,A_y,j*A_z/3), j=0..3, preserving
the shared upper poles. Planar caps are outside this API. Reflection changes
orientation but not unsigned parametric area. Degenerate patches may contribute
zero area; the API does not assert their regularity or native admissibility.

## Actual native comparison and validation

Measure actual GProp area on every recognized sewn band/skirt face; also report
native planar-cap areas and total native area separately. Compare only the
curved-face sum to the independent mathematical enclosure with a separately
named 1e-12 m² numerical allowance. That allowance does not certify all native
SI/mm conversion, sewing or integration error. Native validity and surface
witnesses remain separate prerequisites; passing area alone never admits geometry.

Independent tests use analytic prism side areas, scaling/reflection/current-pole
changes, numerical quadrature of real bands, refinement and invalid/budget
refusals. Live CI runs actual native construction/comparison and retains the
report; synthetic tests reject corrupted area reports but prove no geometry.
Update API/architecture/example, authoring guide/skill and plans. Schema,
migration, MCP eval and golden changes do not apply because no document
operation is added. All mandatory local and exact-head CI gates apply.

Unchanged cylindrical support, G1, global embedding, independent whole-solid
area/fidelity guarantees, physical Mock/runtime/meshes and guarded source/history/
reader admission remain unresolved. No editable NURBS is introduced.
