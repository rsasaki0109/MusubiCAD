# ADR-057: Closed cubic sketch spline admission

Status: Accepted

Task: MCAD-P13-008; admission under ADR-012.

## 1. User story and scope

Create and edit a closed non-rational cubic Bezier sketch curve through stable
control-point IDs, then extrude it normally into an exact solid. This is one
clamped degree-three B-spline span, not general editable NURBS. First and last
control references must be the same authored point ID; the other two are distinct.
The three distinct controls form a counterclockwise nondegenerate triangle.
This restricted convex family is simple without sampled self-intersection repair.
Construction copies may exist but do not become solid profiles.

Open/mixed spline loops, multiple spans, rational/periodic curves, knot/weight
editing, trim/offset/projection of splines, surface NURBS, loft/sweep/revolve,
through-all and spline-specific cut/join are outside this admission. Do not
reinterpret the existing reserved NurbsSurface DTO or advertise it as editable.

## 2. Source ownership, IDs and wire/API sketches

SketchEntity gains `cubic_bezier`. The graph owns three PointEntity nodes and the
curve's ordered four point references. A shared endpoint appears twice because
it is one topological vertex; controls are identified by stable point IDs, never
array index alone. Coordinates and constraints stay on the existing PointEntity.
No BREP, resolved coordinates or render samples are persisted as curve truth.

```json
{"type":"cubic_bezier","id":"ent:spline","construction":false,
 "control_points":["ent:p0","ent:p1","ent:p2","ent:p0"]}
```

Use existing AddSketchEntity and ReplaceSketchEntity/point edits through verified
DesignPatch/history. Retain IDs; changing references is explicit. Reuse the
existing CLI/Agent/MCP patch tools; no GUI-only model mutation. Source schema
allows exactly the declared fields. Unknown degree/knot/weight/rational fields
refuse, including rational input, rather than silently producing a non-rational
curve. Historical sketch enums reject the new entity tag.

## 3. Mathematical convention and units

Degree is exactly 3, knots are implicitly [0,0,0,0,1,1,1,1] (dimensionless),
weights exactly [1,1,1,1] (positive dimensionless), non-periodic clamped endpoints.
This subtype fixes those values; none is silently edited or migrated.
Control coordinates use existing SI-metre Coord values/expressions. Resolve
length expressions with strict unit-kind admission, then solve point constraints.
Require finite coordinate magnitudes <=1e6 m, distinct control separations >1 nm,
positive signed control-triangle cross product >1e-18 m² and shortest/longest
triangle-edge ratio >=1e-3 and sine of the control-vector angle >=1e-3. The bound limits conditioning and metric cost.
Closure requires explicit identical endpoint IDs, not a nearest-coordinate guess.
Plane axes are proper orthonormal within 1e-9 dimensionless; normal extrusion
requires finite length >1 nm and direction parallel to the plane normal within
1e-9 dimensionless. Rigid placements must be finite proper transforms, with
orthonormal rotation rows and determinant +1 within 1e-9 dimensionless.

## 4. Neutral kernel contract and geometry

SolvedSketch carries an optional exact closed cubic control tuple in SI metres;
its neutral/world frame stores three distinct controls with the fourth implicitly
the first; the private native bridge receives all four. Kernels admit only one exact closed
curve, no competing polygon/circle/ellipse/mixed segments. Distance and centred
symmetric NewBody normal extrusion are supported. Unsupported contexts refuse
before producing a surrogate polygon body. Native conversion constructs an OCCT
Geom_BezierCurve edge/wire/planar face/prism privately and returns opaque BREP
bytes through the existing adapter. No OCCT type escapes kernel-occt.

Mock stores analytic prism metadata. For P0=P3, vectors A=P1-P0, B=P2-P0,
area is 3|A×B|/20; centre is P0+3(A+B)/14. Volume is area times normal height.
Bounds use extrema of the cubic coordinate polynomials and quadratic derivative
roots, not tessellated vertices. Native volume must agree with the analytic
oracle before admission. Transforms preserve controls, volume and source state.

Perimeter is numerical: deterministic composite eight-point Gauss-Legendre,
dyadic subdivision, convergence tolerance max(1e-10 m, relative 1e-10), capped
at 4096 panels. Non-convergence refuses mass properties. This is a convergence
criterion, not a certified bound. Surface area uses two analytic caps plus
perimeter times height. Independently compare high-precision reference metrics
and native results; never claim perimeter is analytic. No polygon sampling for
physical metrics. Native tessellation remains presentation.

## 5. Determinism, topology, cache and failure

Detect a stable `profile:bezier:<curve_id>` and preserve explicit consumer refs.
Adding/removing curves cannot silently steal `profile:outer` from an existing
consumer. Control edits invalidate resolved profile/cache inputs while authored
expressions remain unchanged. Duplicate/missing control refs, bad orientation,
unsupported constraints and degenerate inputs fail closed. Failed edits and
regeneration preserve previous document, history and cached outputs.
Source endpoint/curve IDs are semantic identities. Generated BREP references use
existing derivation/discovery policy; do not invent a source-curve association for
an unmatched kernel edge. Required ambiguous/missing refs retain current refusal.
Mock unsupported shape/topology/inertia/tessellation operations explicitly refuse.

## 6. Format, dependencies and budget

Update document and patch entity schemas, migration/refusal policy, exact
source/expanded/packed/history round trips, and legacy golden invariance. Absent
solved-curve fields remain None; no legacy circle/ellipse is inferred as a spline.
No new Rust dependency. Extend the private numeric C++ bridge, not its public
surface. Four controls bound source work; metric evaluation is capped and must
record representative runtime. Admission failure at a cap is a supported result.

## 7. Named acceptance evidence

Before completion provide these tests and execute the new example generator:

- cubic_source_ids_units_weights_and_degenerate_inputs_refuse
- cubic_control_constraints_and_construction_preserve_profiles
- both_kernels_cubic_prism_match_analytic_volume_bounds_and_centre
- cubic_native_and_reference_perimeter_metrics_agree
- cubic_source_control_edit_invalidates_warm_cache_and_keeps_expressions
- cubic_failed_edit_preserves_source_history_and_previous_outputs
- cubic_old_new_expanded_packed_patch_history_roundtrip
- actual_mcp_cubic_control_edit_matches_independent_analytic_grader

The independent reference P0=(0,0), P1=(a,0), P2=(0,b), P3=P0 has area
3ab/20, volume 3ab*h/20, coordinate bounds [0,4a/9] and [0,4b/9], centre
(3a/14,3b/14,h/2). Include invalid rational/periodic/malformed-knot refusal fixtures
alongside accepted non-rational evidence. Add examples, API/architecture docs,
MCP authoring guide, skill and feasible eval. All mandatory local checks and all
GitHub CI are required before merge. No maturity credit until main merge.

## 8. Reproducible local evidence

`cargo test -p opencad-file --test cubic_bezier` compares independent analytic
volume/bounds/centroid in XY/XZ, native versus GL8 surface area, proper rigid
placement, centred symmetric extrusion, source units/unsupported fields,
constraint/construction identity, warm-cache edits and rollback, and file/history
round trips. `cargo test -p opencad-geometry cubic` includes an independent
65,536-panel Simpson perimeter reference. `cargo test -p opencad-cli --test mcp
actual_mcp_cubic` exercises real stdio patch dry-run/apply/native regeneration
and refusal; returned history round trips through the backend. Execute the
ignored `generate_cubic_example` generator explicitly. No golden is changed.

Five independent debug CLI processes regenerating the edited 60×40 mm control
triangle took 0.0198, 0.0146, 0.0100, 0.0087 and 0.0102 seconds in the configured
cloud Linux environment (`CARGO_PROFILE_DEV_DEBUG=0`). Reproduce by applying
`param:a=60 mm` to a temporary copy of `cubic_plate.ocad.d`, then timing five
`musubicad regen` subprocesses and grading `cubic_edit_control`. These are local
representative observations, not performance guarantees or real external-host
verification. Metric quadrature remains capped at 4096 GL8 panels.
