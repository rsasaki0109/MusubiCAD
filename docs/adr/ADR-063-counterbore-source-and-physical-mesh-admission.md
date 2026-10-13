# ADR-063: Counterbore source and physical mesh admission

Status: Accepted design for MCAD-P16-009; implementation remains MCAD-P16-010
and MCAD-P16-003 after their dependencies pass.
Date: 2026-10-13.

## Context and scope

ADR-062 / P16-002 proves real rectangle/circle/concentric-cut measurements in an
explicit analytic Mock mode. It deliberately refuses meshes and topology.
Current `MeshSet::box_prism` is legacy synthetic plumbing: it contains only
four top/bottom triangles and is not a closed physical box. Feature face
discovery tessellates bodies, and existing regeneration can turn discovery
errors into an empty list. Neither path is sufficient counterbore evidence.

Admit one counterbored standard-hole family on unmodified rectangular stock
created by a right-rectangle, distance `NewBody` extrusion. Entry is one of that
extrusion's two end faces, explicitly bound by an owned semantic face reference.
The circle sketch is on the selected entry plane, with its normal outward;
both cuts run inward. Bore and larger seat are strictly radially contained.
Depths are explicit distances, bounded by stock thickness. Include arbitrary
admitted rigid placement and either entry end; refuse side entry, already-cut
targets, imports, patterned/fused/nonrectangular stock, oblique/lateral cuts,
through-all, symmetric depths and multiple nonconcentric chains initially.

This is a feature admission, not a claim of general hole topology, manufacturing
tolerance classes, automatic fastener head sizing, countersinks or threads.
The fixed maturity rubric and existing earned scores remain unchanged here.

## Source representation and compatibility

P16-003 adds a distinct `FeatureDefinition::SeatedStandardHole` source tag
`seated_standard_hole`. Its own struct and seat wire variants deny unknown
fields; do not flatten the legacy `HoleFeature` or add optional seat fields to
`standard_hole`. Required fields are owning sketch/profile, target feature,
semantic `face_ref`, explicit `HoleStandard`, `bore_depth: Length` and seat.
The closed seat enum initially admits only `counterbore`, with required
`diameter: Length` and `depth: Length`. Optional expressions are
`bore_depth_expr`, `seat.diameter_expr` and `seat.depth_expr`.

Length wire values use the existing `{ "value_si": ... }` representation in
metres. Expressions retain original source strings, evaluate with explicit
length units and enter dependency/cache invalidation. Bore diameter comes from
the existing ISO273/ISO metric coarse table and must match the exact sketch
circle within 1e-9 m. Tap-drill remains a drill cylinder, with no physical thread.
Seat dimensions are explicit, not inferred from designation or a new standards
table. A future countersink must have a different strict seat discriminant and
its own accepted angle/frustum admission.

After evaluation, require finite positive depths, `seat_diameter` greater than
cut diameter by more than 1e-9 m, and `0 < seat_depth < bore_depth <= thickness`,
with distinct depth/radial material margins exceeding 1e-9 m. A full-depth
larger cylinder is not admitted as a counterbore. Preserve ADR-062 dimension,
coordinate, coincidence and budget bounds. No clamping of authoring input.

The additive feature/seat tags use the existing schema version policy. Older
readers must refuse the unknown feature tag; a later seat type must be rejected
by a counterbore-only reader. P16-003 updates both document/patch schemas,
migration/admission policy and expanded/packed deterministic round trips as one
change. Migrations never infer seats, standards, faces or new depths in old
`hole`/`standard_hole` records. Test legacy source identity and genuine old-form
decoder refusal, not just current schema validation.

## Owned face and rectangular-stock witness

Source `face_ref` is mandatory and nonempty. Resolve the reference on the
specified target body, checking creator/target ownership, missing/consumed/
ambiguous/stale bindings and body-specific discoveries. No stored numeric ID
alone, nearest face, bounding-box approximation or current active-body fallback
can establish ownership. Coarse role matching is insufficient without the
actual unique target-body physical witness.

Extend `RegenContext` with read-only source-node access for this feature; default
unsupported implementations refuse. Verify the target is the admitted original
rectangle/distance/NewBody extrusion and its resolved profile/extent match the
actual geometric witness. No graph ownership or source lookup moves into a
geometry kernel. Both a stale source-node binding and unsupported target source
must fail before cutting.

P16-010 introduces a geometry-neutral query:

```text
rectangular_stock_geometry(body, entry_pick: FacePick)
    -> Result<RectangularStockGeometry>
```

The transient DTO reports entry face ID, origin in metres, unit U/V axes,
outward unit normal, two face spans and inward stock thickness in metres.
Require one actual rectangular solid: six planar rectangular faces, twelve
straight edges, eight corners and three orthogonal dimensions, with no inner
wires or missing faces. Match the pick to exactly one real face by finite plane
coincidence (1e-9 m), positive outward-normal agreement (cross-product norm
at most 1e-9 dimensionless and positive dot product)
and in-face position. Never select the closest candidate. Mock derives this
only from an uncut tracked stock record; Native checks its actual B-Rep privately.
Concrete OCCT types and native conversion remain in `modules/kernel-occt`.
Use cadrum's existing face/edge access where sufficient; any additional private
FFI must remain bounded, justified and independently tested in P16-010.

`FacePick` supplements the resolved semantic identity; it does not replace it.
Require the returned face to be the uniquely resolved semantic face, with its
outward normal and source extrusion axis checked. This handles native faces
that share a surface ID without pretending numeric IDs are universal. Stock
shape/source disagreement, shared/copied ambiguous candidates, nonplanarity,
imprecise or unsupported picks refuse. Verify the circle centre lies on the
entry plane and its full seat disk has more than 1e-9 m lateral material.
Tests must cover foreign target, opposite end, side face, plane mismatch and
nearly oblique input; checking only `abs(dot(n1,n2))` is insufficient.

## P16-010 physical mesh and face identities

Analytic mode emits actual closed rectangle/cylinder/concentric-cut meshes;
never call the legacy synthetic box helper. Keep exact mass/area/centre/bounds
on the analytic path independently of the faceted output. Support stock with
no cuts and a single concentric chain of at most 64 cuts, including blind and
through intervals. Exact endpoint integration from ADR-062 remains unchanged.

Admit deterministic circle sampling using the existing quality settings:
positive finite linear deflection in metres and angular deflection in degrees,
the latter at most 180 degrees. The largest relevant radius determines a
uniform angular grid with at least eight segments. Every actual angular gap
`a` must satisfy `a <= angular_deflection_rad` and
`R * (1 - cos(a/2)) <= linear_deflection_m`. Use a grid with half the allowed
gap to leave margin for adding/replacing mandatory rectangle-corner rays.

For perforated rectangle caps, add rays through all four true rectangle corners
from the strictly contained hole centre. Pair the inner circular ring and its
ray intersections with the true outer rectangle boundary; connect successive
pairs with consistently wound quads split into triangles. Never omit a corner
and replace stock with a clipped polygon. Keep corner anchors when a grid ray
coincides; recheck actual gaps after deterministic coalescing. If near-coincident
rays or f32 conversion cannot produce valid triangles within the requested
bound, refuse instead of changing quality silently. Require sagitta plus measured
f32/arithmetic coordinate error to remain within linear deflection; a requested
quality finer than representable coordinates must refuse. Circle vertices lie
on the intended analytic radius within coordinate error, not merely within the
coarser chord deflection. Validate frame orthonormality before meshing and
refuse accumulated non-rigid drift.

Connect outer boundary walls across the stock depth. Inner cylindrical walls
follow active exact axial intervals; radius changes have correctly oriented
annular caps, and blind cavities include disk caps. Standalone cylinders have
both end caps. Use source frame handedness to ensure outward winding after
positive/negative extrusion and proper rigid placement. Every geometric edge
of the final mesh is paired twice with opposite orientation, there are no
degenerate/self-crossing triangles, and signed tetrahedral volume is positive.
Duplicated vertices for normals may be welded by exact f32 position solely for
validation; collision/area/volume still use the actual exported mesh.

For analytic Mock output, budget at most 8192 angular samples (including anchors), 100000 vertices and
200000 triangles per mesh. Invalid settings, non-finite/error-bound failure,
overflow, ambiguous tiny steps, budget excess or f32 collapse refuse before
publishing a partial mesh. There is no automatic retry with looser settings.
Native tessellation retains its existing contract; independent native evidence
checks its actual outputs and quality limits rather than claiming that Mock
sample counts apply globally to all OCCT geometry.
No new triangulation dependency is needed: admitted caps use the ray-paired
rectangle/ring construction and disk/annulus fans, not general polygon holes.

Analytic Mock faces have actual runtime IDs and triangle-face metadata. Allocate
deterministic IDs without collision between unrelated Mock bodies; preserve logical outer faces
through admitted cuts, and report modifying/transform history where IDs change.
Do not reuse global face IDs 1..6 for unrelated physical boxes. A body's face
lookup validates membership. Native IDs retain their existing backend convention,
including shared surface IDs: validate them on the specified body with the
normal/plane witness, without globally rewriting Native IDs or assuming they are
universal. Curved wall and blind/annular cap identity must be
distinct from an outer entry face. Tested known-owner planar face bindings can
rebind across parameter changes; unsupported edge/circular-edge queries, exact
intersection, inertia and other shape edits still explicitly refuse.

For an inscribed ring, maximum gap `a` bounds relative disk area error by
`1 - sin(a)/a <= a*a/6`. For fixed rectangular stock, mesh remaining-volume
error is nonnegative and bounded by removed analytic volume times `a*a/6`,
plus a separately measured f32 coordinate-error budget. For a fixed local
integration origin, maximum vertex error delta and maximum vertex distance D,
a conservative per-triangle signed-volume perturbation bound is
`(3*delta*D*D + 3*delta*delta*D + delta*delta*delta)/6`; sum over actual triangles
and state the additional f64 arithmetic tolerance. Tests independently check
ring/contact positions within coordinate error so a loose conservative volume
bound cannot excuse a wrong radius or pose. Independent tests use
actual triangle/tetrahedral integration, centroid, outer bounds, closedness,
winding, face coverage and measured gaps; a volume-only or AABB-only check is
not sufficient. Native meshes need not have identical sampling or face IDs;
each must satisfy its documented quality/physical bounds independently.

## Feature execution and transaction contract

P16-003 evaluates/validates source, stock and face witnesses before either cut.
Resolve the original circle in its declared frame. Cut the explicit bore, then
the coaxial larger seat through `GeometryKernel`, preserving owned target
inputs. The analytic removed volume is
`pi*r*r*bore_depth + pi*(R*R-r*r)*seat_depth`; derive first moments and contact
dimensions independently. Do not subtract the seat's full cylinder twice or
alter source sketch geometry to get the seat radius.

Existing `add_feature` and `replace_feature_definition` author the new tag.
Extend `set_feature_expr` for named `bore_depth`, `seat_diameter`, `seat_depth`
fields with their explicit-unit expression validation; ordinary parameter edits
must invalidate the feature and regenerate. Verified geometry is mandatory for changes introducing or affecting this new
feature; an unverified-apply flag cannot bypass its source/face/mesh gates.
Discovery errors or empty results cannot establish resolved physical topology.
In particular the feature must verify actual output mesh/face support before
returning a successful geometric result. DesignPatch/transaction review/apply and opaque Agent Undo/Redo preserve canonical source, parameters,
other features/child files, previous geometry and existing artifacts on failure.
No direct GUI writes or new external-model call is introduced.

Default Mock factories remain unchanged. Rust callers explicitly select the
analytic mode for complete admitted document tests, keeping the same selection
through review/apply/Undo/Redo. Native CLI/MCP exercises the actual authoring
workflow. A legacy/no-OCCT Mock caller unable to supply a physical stock witness
must refuse the new feature, never report a synthetic hole. Do not add a source
kernel setting or silently switch all old Mock documents to new geometry.

## Named acceptance evidence and implementation order

1. MCAD-P16-009 merges this design, API/schema sketch and dependencies. No
   capability, feature or maturity credit is awarded for that planning task.
2. MCAD-P16-010 implements real closed meshes, body-owned rectangular-stock
   queries and face identities in both kernels, with independent physical and
   failure tests. It adds no persisted feature and receives no separate credit.
3. MCAD-P16-003 implements the source/patch/schema/feature workflow after the
   prerequisite passes, with all five required feature deliverables. Only then
   can merged behavior justify a separately explained rubric update.

Required test families:

- `physical_mesh_quality_and_closure`: stock, standalone cylinder, bore/seat in
  either order, blind/internal cavity, either extrusion sign, tilted/translated
  frames, actual gaps/error bounds, all faces and unchanged source measurements.
- `stock_face_witness_is_owned_and_unique`: both kernels, positive/opposite/side
  picks, foreign/unknown/curved/cut stock, body membership, duplicate candidates,
  f32 collapse/invalid quality/budget refusals, default Mock isolation.
- `counterbore_matches_independent_manufactured_geometry`: both kernels on
  40 x 30 x 10 mm stock with M6 normal 6.6 mm bore at (12,9) mm and explicit
  11 mm seat of depth 4 mm; before/after parameter edits, full first moments,
  analytic volume/area/contact dimensions and independently integrated exports.
- `counterbore_source_and_reader_roundtrips`: document/patch schemas, strict
  unknown feature/seat fields, current expanded/packed round trips, actual
  old-form refusal and byte-preserved legacy hole/standard-hole source.
- `counterbore_verified_patch_history_and_mcp`: actual native MCP/Agent create,
  review/apply/Undo/Redo, independent grading after seat edits, stale/wrong-unit/
  missing/ambiguous/foreign/consumed-reference/unsupported-target refusal and
  previous source/artifact preservation. Add a complete example, guide/skill and
  `tools/mcp_eval_tasks.json` task when implemented; no placeholder passing eval.

All implementation tasks are separate main-target PRs cut after their
predecessor merges, with the four mandatory pre-push commands and all applicable
native CI. A new FFI dependency/scope/sampling policy requires an ADR amendment
before code. NURBS and broader curved stock remain outside this admission.

## Alternatives

Optional cosmetic fields on `standard_hole` were rejected because old readers
can ignore them and cut the wrong solid. Native-only feature validation or
synthetic Mock volume was rejected because both kernels are required. Rendering
an unperforated box was rejected because it hides holes and produces invalid
physical exports. General polygon triangulation/CSG mesh support is unnecessary
for this bounded family. Heuristic nearest-face selection or silent quality
relaxation would hide ambiguity; explicit refusal preserves authored intent.
