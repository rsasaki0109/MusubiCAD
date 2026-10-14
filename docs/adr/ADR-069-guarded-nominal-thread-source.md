# ADR-069: Guarded nominal metric external-thread source

Status: Accepted for MCAD-P16-016 design; P16-007 implementation remains planned.
Date: 2026-10-14.

## Context and decision

ADR-067 supplies the nominal basic flat-root profile. ADR-068 / PR #114 supplies
real Mock and OCCT bodies, physical meshes and independent phase measurements.
Neither supplies an authored feature, document compatibility, source provenance
or transaction admission. Runtime metadata cannot substitute for those proofs.

Implement a distinct closed `nominal_metric_thread` feature in P16-007. Its
admission is the original authored exact-circle/distance/NewBody extrusion and
its owned full cylindrical side. The entire stock length becomes 1–32 complete
coarse pitches with square ends. This is a nominal basic flat-root envelope,
not a tolerance-class production thread, rounded external root, tap-drill hole,
cosmetic annotation or generic helical sweep. No new sampling method is accepted
here; retain ADR-068's bounded physical construction and mesh quality contract.

This decision is design only. None of the proposed wire tags, fields, output
roles, source checks or MCP capabilities below exists until P16-007 implements
and validates them. Existing runtime APIs and persisted files are unchanged.

## Closed source and typed edits

The planned definition contains these required fields and no implicit defaults:

| Field | Closed meaning |
|---|---|
| `target_feature` | Stable ID of the original, unsuppressed cylinder extrusion |
| `face_ref` | Mandatory semantic reference owned by that extrusion's full cylindrical side |
| `representation` | Exactly `nominal_metric_basic_flat_root` |
| `size` | One of `m3`, `m4`, `m5`, `m6`, `m8`, `m10`, `m12` |
| `handedness` | Exactly `right` or `left` |
| `turns` | Integer in [1,32], never a fractional or coerced numeric string |
| `ends` | Exactly `square` |

Both document and authoring schemas require the same fields, enum values,
integer limits and unknown-field refusal as Rust decoding. No separate authored
diameter, pitch, depth, length, phase or axis introduces conflicting authorities.
Size selects ADR-067's SI diameter/pitch table; length is turns times pitch.
Size and turns are explicit categorical/integer values, not dimensional
expressions. No expression fields are accepted on this new feature initially.
The original sketch radius and extrusion distance retain their existing typed
Length/strict-expression paths. Validate static source fallbacks even when an
expression is present; a good expression must not conceal malformed source.
Evaluate upstream expressions with the strict parameter scope before comparing
the current solved circle, extrusion length and actual stock witness.

Use existing typed `add_feature` / `replace_feature_definition` operations for
creation and size/hand/turn/end edits. Add the feature to `set_feature_ref`'s
closed `face_ref` admission; unsupported expression fields must refuse. A
coordinated turns edit changes stock distance and thread turns in one reviewed
DesignPatch. A size edit also changes the authored circle radius. Validate the
final candidate graph; do not publish an intermediate mismatched stock.
Never automatically resize stock, infer missing source fields or change refs
while reading/regenerating. Unsupported explicit values must remain errors.

## Authored stock, phase and consumed reference

Require exactly one unsuppressed target Extrude with Distance extent, NewBody
operation and no target body, driven by an original unsuppressed authored Sketch
and a solved exact Circle profile. Reject import, projection, approximation,
pattern, cut/fusion/modified stock, ellipse, mixed profile and competing active
consumption. Source graph ordering/dependencies must include the target and
semantic owner. Suppressed or removed dependencies must not admit cached data.

Check the authored/resolved Workplane axes before any normalization: finite unit
orthogonal axes and proper orientation. The origin is the solved circle centre
in its checked sketch placement. The positive extrusion axis is x cross y;
the radial phase-zero direction is that same placement's positive x axis.
Do not use mean face normal, OCCT parameter axes, global X or a stale cached
placement. Proper rigid rotation of the source rotates the whole phase frame.
Mirrored/improper frames do not silently swap handedness.

Query the actual body's full-cylinder witness and compare source origin, axis,
radius and height within 1nm, then compare the nominal size/turn dimensions
within 1nm. Validate finite coordinates and ADR-068 precision/world limits.
Bounds, a triangle silhouette, scalar mass or a body handle alone cannot prove
that the body is the admitted original cylinder.

Require exactly one Face ref with `created_by = target_feature` and role
`cylindrical`. It is consumed by the new thread. Ref normal hints, when present,
must be finite unit radial vectors. Optional fingerprints retain their meaning:
cylinder surface, no edge identity, actual side area interval and centroid hint
must agree. Reject stale annotations; do not widen or discard them to admit an
edit. Examples intended for stock-length edits may deliberately omit optional
area/centroid annotations while keeping mandatory semantic ownership.

A kernel face ID presently belonging to a cap or another physical face is an
error. An absent/stale ID may rebind only after unique typed authored ownership
and the actual full-side witness are proved. Do not search the nearest face.
Bind the real lateral face before cutting, and preserve the original body.
Another active feature consuming that side must refuse. Downstream requests to
consume the old full-cylinder side on the threaded output must also refuse.
Detect competing active solid modifiers of the target body even when they use
another semantic alias of the same side; matching the ref string alone is not
sufficient. Read-only measurements and rendering do not consume the stock.

## Physical output and provenance

Use the existing `cut_nominal_metric_thread` runtime with the admitted frame.
Require actual finite positive body properties, bounded face-bearing mesh and
ADR-068's closed/oppositely paired mesh and representability/budget checks.
Measure mass/area/COM from the actual backend; native values must come from its
BREP integration, never from substituting the ideal profile's result.
Compare volume and area with independent profile integration using ADR-068's
2e-5 native relative bound; COM uses 20nm plus coordinate roundoff. Tests also
check dimensions, actual screw phase and bounds. Equal mass cannot prove hand.

The old stock cap disks become noncircular cross-sections, and the original full
cylindrical side no longer survives as a complete face. **Every output face is
produced by the new thread feature**, including both end faces and any remaining
crest-cylinder patches. Do not retain original producer IDs, fabricate face
history or carry original cap/side bindings into the output.

Feature-specific discovery groups triangles by actual nonzero kernel face ID,
not by averaged normal. Verify finite indices/coordinates and inspect all
vertices in the admitted local frame. End-face vertices lie on z=0 or z=L,
within that end's actual noncircular radial contour, with outward winding.
Side vertices lie within the square ends and on ADR-067's screw envelope within
the declared native boundary/coordinate precision; their triangle winding is
outward. Cap interiors are inside the contour, not forced onto the outer radius.
Boundary-vertex phase checks must not borrow a potentially large linear
tessellation deflection to admit a wrong hand. That deflection bounds interpolated
facets, not displacement of boundary vertices from the physical surface. Derive
the radial check budget from ADR-068's construction and measured coordinate
precision, propagate phase-coordinate error through the profile slope and
refuse if it cannot distinguish the required phase. Test a wrong-hand cached
body explicitly; volume, area and axial COM cannot distinguish the two hands.
Runtime tests independently validate boundary phase, so this is a source/output
consistency gate rather than a proof for arbitrary untrusted surfaces.
Insufficient precision or a face that mixes end and lateral surfaces refuses.

Proposed discovery roles are `thread_start`, `thread_end`, `thread_surface`.
These describe physical groups and do not promise stable subface numbering or
individual flank identity. Mock's six physical classes and OCCT's split faces
need not have equal cardinality. Preserve every actual face ID once, with the
new feature as producer; do not synthesize IDs or collapse distinct faces into
one invented face. No exactly-three-face requirement from cylindrical draft,
generic normal-based producer inference or synthetic Mock fallback is allowed.

Multiple real faces may share `thread_surface`; that role alone cannot select
one. Downstream resolution must refuse ambiguity instead of choosing the first
face or a transient native index. End refs also require unique proved discovery;
otherwise refuse. General thread-face/edge selection, generic downstream edits
and reconstruction history remain outside this initial admission. Add no edge
reference stability claim. Existing runtime unsupported-consumer refusals stay.

## Regeneration, caching and transactions

Feature dependencies, consumed refs, values and strict parameter evaluation
must include the new variant. Cache identity must incorporate the full authored
definition, upstream source and backend as for existing features. On a cache
hit, re-admit the current authored cylinder/ref/frame and revalidate the actual
cached output and face discoveries; never trust cached producer lists alone.
Missing bodies, stale refs, wrong hand/frame/dimensions, mismatched output or
ambiguous identities refuse. Cold/warm regeneration must agree semantically;
raw native handle equality across regenerations is not required.

Add this feature to mandatory native candidate verification in dry-run/apply
and editor/MCP paths, even with `verify_geometry: false` or CLI `--no-verify`.
Geometry validation must execute before publishing source, history, cache or
artifacts. Test failure after earlier successful operations in the same patch,
not only a failure on the first operation. Rejecting source/ref/unit/precision/
kernel/output checks preserves prior document bytes, output, history and export
artifacts. Successful edits and canonical Undo/Redo go through transactions and
the same DesignPatch backend; no GUI/MCP-only geometry path.

## Compatibility and independently graded acceptance

Use the new closed `nominal_metric_thread` FeatureDefinition tag so genuine
older readers refuse it. Update both schemas, migration policy and packed/
expanded/source-authoring round trips in the source implementation PR. Migration
does not infer a thread from a cylinder, standard-hole designation or helix.
Keep existing files unchanged. Test old-reader refusal with the pre-extension
wire enum/parser rather than a manually returned error or schema-only check.

P16-007 is complete only with all of the following actual evidence:

- Both kernels execute the source feature against independent coarse dimensions,
  piece-integrated volume/area/first moments, actual radial phase and closed mesh
  checks. Include both hands, 1/8/32 pitches, all seven sizes in source validation
  and representative physical cases, tilted/reversed source workplanes and
  proper source rotations. Do not repeat the whole runtime matrix needlessly.
- Missing/foreign/ambiguous/consumed/cap-bound refs, stale fingerprints, unsupported
  source kinds, mismatched diameter/length, invalid discriminants/counts/units,
  bad static fallbacks/expressions and precision/quality failures refuse. Protect
  source and previous output; exercise warm cache and failed cached admission.
- Actual DesignPatch add/edit/ref/coordinate changes, even with verification
  disabled by the caller, cold/warm regeneration, canonical Undo/Redo and failed
  candidate publication are tested. Structural admission must not bypass native
  verification on replacement or parameter edits.
- Deterministic packed/expanded and source-authoring round trips, strict schemas,
  no-inference migration, genuine old-reader refusal and legacy fixtures pass.
- Add a runnable source example under examples/, authoring guide and skill, API/
  architecture docs, and a real source-aware MCP eval. Its grader independently
  requires the requested representation/size/turns/hand and retained authored
  source/ref ownership. Include a right-to-left edit and volume-matching wrong-
  hand/cylinder/helix decoys: scalar volume alone must not pass the evaluation.
  Physical handedness is independently checked by backend geometry tests; an
  eval that only inspects fields must not claim to measure the surface itself.

All four local pre-push checks and exact-head applicable CI workflows/jobs must
pass. Planning and runtime work earn no extra score. P16-007 feature credit can
be assessed only after that complete source feature merges to main.

## Alternatives and risks

Adding optional thread fields to a generic helix/cylinder risks older readers
silently dropping manufacturing intent. A distinct mandatory tag avoids that.
Automatically changing stock or clearing stale annotations hides an authored
design change; require an explicit coordinated patch instead. Reusing cylinder
face ownership or equating six Mock classes with native face counts fabricates
provenance. Preserve real identities and refuse ambiguous selectors.

This narrow admission excludes partial/threaded shoulders, runout/chamfer,
rounded roots, arbitrary pitches, fit/production classes, multi-start/internal
threads, phase editing and general downstream thread-face editing. Extending
any of those requires its own geometric/source admission and independent proof;
the word nominal must remain visible in authoring documentation and examples.
