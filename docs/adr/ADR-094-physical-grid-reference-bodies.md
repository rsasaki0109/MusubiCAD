# ADR-094: Physical grid reference bodies in both kernels

- Status: Accepted
- Task: MCAD-P16-041
- Depends on: ADR-062, ADR-092, ADR-093; prerequisite to P16-008

## Context

ADR-093 supplies an immutable actual TriangleSolid and its unplaced grid error
certificate. Generic mesh import remains deliberately unsupported by the
analytic Mock mode. Using legacy synthetic plumbing is insufficient evidence
for a physical analytic Mock body. Native diagnostic construction alone does
not exercise GeometryKernel storage, queries or placement.

## Decision

Add GeometryKernel::import_circular_fillet_grid with the private-field
CircularFilletGridSolid as its only input. The default trait implementation and
default legacy Mock refuse. The counting adapter forwards/counts the call.
Analytic Mock explicitly stores a new physical facet record containing a clone
of the actual admitted TriangleSolid. OCCT uses its existing actual triangle
BREP construction and one-solid validation. No caller-supplied arrays, ideal
mass, error metadata, fake stock or fallback body can bypass the typed gate.

Each analytic Mock triangle receives a new body-owned facet ID. The fixed input
triangle order defines local facet identity; neither diagnostic chart groups nor
original cylinder/plane support identities are claimed. OCCT retains its existing
body-local face indices: ownership is the pair (body handle, local index), not
a globally unique number. Indices outside the queried body must refuse. Mass, area, centroid and
bounds derive from the actual stored grid geometry. Ordinary rigid placement
transforms the actual facets through existing body/face derivation storage; the input error
certificate does NOT extend to newly transformed coordinates. Analytic Mock shape edits and stock/cylinder/rim/support queries outside the
facet record's capabilities refuse. OCCT retains its ordinary native facet
operations, which do not propagate any reference certificate. A native body
marker requires proper rigid placement, including compound members and repeated
placements; invalid placement must not insert an output body. Generic mesh import remains unsupported in analytic mode.
Existing triangle and intersection budgets remain unchanged.

Tessellation represents this faceted body, independent of the ideal reference.
It validates ordinary finite positive tessellation quality before conversion to
f32, but no ideal-reference f32/placement/native-conversion error certificate is
introduced. Native import is numerical physical evidence with existing native
sewing and validation, not an exact global embedding or native floating proof.
The smooth rational-BREP diagnostic and its analytic supports remain separate.

## Validation

Both kernels must store/query real constant and variable reference bodies.
Compare independently calculated integer-grid determinant volume, actual facet
area/centroid/bounds and transformed coordinates. Check owned facet IDs, closed
mesh, proper placement, source-body preservation, deterministic fresh imports,
unknown IDs and default-Mock isolation, unsupported stock/support/shape edits,
and counting-adapter forwarding. Add a runnable example, API/architecture,
MCP authoring guide and skill. Required workspace/native CI must pass.

## Consequences and risks

This is an explicit physical reference-body construction prerequisite. It is
not a fillet applied to an authored cylinder: current stock/rim consumption,
smooth unchanged supports, source/history/cache/schema/old-reader admission and
independent MCP editing remain separate requirements before P16-008. No source
feature/MCP operation or eval is added because there is no authorable operation.
No maturity credit. Native numerical mass/bounds/mesh comparisons use explicit
SI tolerances; ideal mass is never substituted for actual faceted geometry.
