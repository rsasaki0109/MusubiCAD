# ADR-101: Guarded numerical faceted circular-fillet source

Status: Accepted; implemented by MCAD-P16-008, completion merge-gated.
Date: 2026-10-16.

## Decision and representation

Expose ADR-100 through a distinct closed `faceted_circular_fillet` definition.
Require `target_feature`, `edge_ref`, `representation`, `verification`,
`minimum_radius`, `maximum_radius` and `quality`; unknown fields refuse.
Representation is exactly `grid_faceted_periodic_cosine`; verification is
exactly `canonical_grid_with_numerical_backend_checks`. This explicitly admits
ADR-070's separately named approximation path, not an exact smooth fillet.
All supports, caps and blend are planar facets. There is no exact G1 claim,
unchanged analytic support, general curved contour or editable NURBS admission.

Both radii are explicit Length values; initially edits replace the typed
definition, without optional radius expressions. Upstream sketch/extrusion
expressions retain strict dimensional evaluation and static fallback checks.
The positive original sketch X fixes the cosine maximum; X cross Y fixes the
positive extrusion axis. Native seams, edge orientation and body-local indices
never define phase. Check raw unit orthogonal directions before normalization.

Quality requires explicit final position Length, unit-normal chord, reference
volume/area/first-moment/centroid interval widths, permitted absolute grid
differences, vertex/triangle limits and integration work limit. Fields ending
in `_m2`, `_m3`, `_m4` use those SI units; centroid/position use Length. One
position/normal request applies to both initial shell and final stored grid.
No implicit defaults, coarsening or increased runtime caps are allowed.
Canonical outward bounds apply ONLY to the unplaced stored input grid.
Native mass, placed geometry and f32 tessellation are separate numerical checks,
never substituted with ideal integrals or called certified by those bounds.

## Source, reference and output admission

Require one active original exact-circle Sketch / Distance / NewBody extrusion,
with no projected/imported/modified stock or competing active solid consumer.
Validate raw authored data before parameter substitution, solved geometry and
the actual complete cylindrical stock/rims in the checked authored frame.
Consume one mandatory uniquely owned semantic Edge with role `extrusion_end`
at positive z=H. Require unique role ownership, current full-circle identity,
and agreement of any optional hints/fingerprint; stale/foreign/start-rim,
ambiguous aliases and conflicting consumers refuse, without nearest selection.
An absent old body-local binding may rebind only to that uniquely proved rim;
a binding to another present physical edge refuses. No global-ID claim is made.

Build through ADR-100. Independently reconstruct the expected placed canonical
facet shell for output/cache admission. Require finite positive actual mass,
numerical volume/area/COM agreement, closed face-bearing physical tessellation,
one actual face per expected planar triangle, every expected face exactly once,
and matching ordered triangle corners/winding within a separately derived f32
coordinate allowance. Refuse if coordinate precision cannot resolve the radius
law or distinct expected vertices/facets. Match geometry, not native numbering;
wrong-phase equal-volume cache bodies, cylinder/constant/straight substitutes,
missing/duplicate faces and borrowed producer lists must fail. Numerical mesh
comparison is a bounded source/output consistency check, not an arbitrary BREP
equivalence or exact embedding certificate.

Native display normals for marked grid member faces are derived from their
actual oriented f32 triangles, with independent per-facet display vertices;
finite nondegenerate orientation must survive conversion. Keep physical triangle
coordinates/connectivity/IDs and non-faceted members intact. This fixes the
legacy +Z display placeholder without claiming a native/f32 normal certificate.

Every physical output facet is produced by the new feature. Discovery exposes
all actual IDs as `faceted_surface`; a role-only selector is ambiguous. Do not
retain stock cap/side/edge bindings, invent derivation history, or promise stable
subface numbering. Re-admit source, frame, rim and actual output on cache hits.
Keep default legacy Mock unsupported; analytic Mock and OCCT execute physically.

## Transactions, compatibility and acceptance

Creation/replacement/ref/coordinated stock edits use DesignPatch and the existing
transaction/history backend. Mandatory native candidate verification applies
even with caller `verify_geometry: false` / CLI `--no-verify`. Preview/review
remain read-only; any failed candidate preserves source, output, history and
artifacts. Canonical Undo/Redo uses the same source definition.

Update both schemas with the same closed required source and quality fields.
Migration never infers this intent from legacy fillets/cylinders; genuine older
wire readers reject the new tag. Test deterministic packed, expanded and source
authoring round trips and existing files. No golden changes are required.

Acceptance includes constant and genuinely variable physical source tests in
both kernels against independent analytic/integer-grid volume/dimensions,
proper/reversed placement and phase, cold/warm cache, equal-volume wrong-phase
and stock/reference/unit/quality/work refusals. Include actual reviewed MCP
creation/edit/refusal/history, source-aware grading plus independent physical
surface evidence, a runnable DesignPatch-authored example, guide and skill.
All required local checks and exact-head applicable CI precede merge.
Any maturity credit requires the complete actual feature main merge and a
fixed-rubric reason; accepting this ADR alone earns none.

## Limits and risks

Only one complete convex positive end rim and one periodic cosine are admitted.
Huge placements or tiny law amplitudes may be refused when mesh precision cannot
distinguish them. Native tessellation that splits or merges the expected physical
facets refuses; no normal-average fallback is accepted. Strict optional source
hints may require explicit patch edits when stock changes. Broader contours,
smooth supports, phase/frequency editing and downstream facet editing remain
separate work.
