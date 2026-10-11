# ADR-038: Linear variable fillets on semantic straight edges

Status: Accepted  
Date: 2026-10-11  
Roadmap: MCAD-P11-005

## User story and bounded geometry

A designer rounds a straight mechanical-part edge with different start/end
radii, or rounds the perimeter of a selected semantic face at constant radius.
Extend the existing `fillet` definition with optional `radius_law`:
`{"type":"linear","end_radius":{"value_si":0.0015},"end_radius_expr":"end_radius"}`.
The existing `radius`/`radius_expr` defines the start radius. Endpoints follow
lexicographically increasing world coordinates (x, then y, then z), independent
of kernel enumeration and edge orientation. Both radii are SI lengths >= 1e-9 m.

Variable laws require exactly one nonempty semantic `edge_ref`; a face and edge
cannot be selected together. Constant-radius face-perimeter fillets retain the
existing semantic face input. An absent law retains legacy constant behavior.
Curved edges, tangent chains, arbitrary sampled laws, face-wide variable laws,
zero radii and collapsed/invalid topology are refused. No editable NURBS is
introduced. The fixed interpolation scheme below is the sampling decision.

## Native kernel boundary

cadrum 0.8.5 exposes constant fillets only. OCCT 8.0 supplies
`BRepFilletAPI_MakeFillet` for variable evolution. Its two-radius helper uses
zero-slope endpoint interpolation, which did not match the desired linear law.
The pinned version's direct Law_Function overload is unsuitable. A private cxx
bridge uses the supported parameter/radius array API with **64 equal intervals**
(65 points) from `r(t) = r_start + t (r_end - r_start)`, t in [0,1]. Native OCCT
interpolates those values, with endpoint easing limited to the two end intervals.
The number of intervals is fixed and bounded; it is not a user-editable sample
count. This is an approximation of the explicit linear law, not a claim of an
exact surface. Its radius samples are deterministic; conservative radius error
in an endpoint interval is bounded by the total radius change divided by 64.
The tested shallow-slope prism fixture matches the integrated circular-corner
reference volume within 1e-3 of removed volume. This is fixture evidence, not a
universal volume guarantee for arbitrary solids.

The private cxx bridge in `modules/kernel-occt` calls that operation. Its input/output is an
in-memory text BREP plus endpoint coordinates/radii; no files, OCCT pointers,
types, or geometry mutation escape the kernel. The bridge catches OCCT errors
and cxx translates exceptions into a Rust Result. It checks unique straight
endpoint matching (1e-8 m), one-edge contour and BRepCheck validity.

The bridge works in millimetres for OCCT numerical tolerances and converts all
inputs/outputs back to the kernel's metres. Surface approximation parameters
use 1e-6 mm 3D tolerance, and BREP endpoint matching uses 1e-5 mm (1e-8 m).

The bridge shares cadrum's pinned OCCT headers and static libraries. A cadrum
build dependency prepares that root before the bridge compiles; build.rs derives
the same target cache path or honors OCCT_ROOT. cxx/cxx-build are already locked
transitive dependencies and become direct bridge dependencies. Linux, Windows
and macOS compilation uses the target's existing OCCT root and C++17 compiler.

The workspace forbids unsafe Rust. This kernel package uses `deny` instead, with
one explicit allowance on the cxx-generated FFI module only; handwritten kernel
Rust remains denied and all other packages retain `forbid`. This exception is
required for cxx's generated foreign calls, not permission for general unsafe
code. The public GeometryKernel method has an unsupported default and explicit
Mock validation; counting kernels forward it.

## Provenance, transaction and compatibility

Selectors must identify one edge on the current target. Missing or ambiguous
role matches fail rather than using a longest-edge heuristic. The native bridge
uses a temporary BREP snapshot and preserves source/output validity. Pointer
history from cadrum cannot survive this transfer, so no exact derived face
history is fabricated. Subsequent references use the existing verified geometry
provenance; ambiguous/missing geometry references must still fail closed.

End-radius expressions participate in graph queries and cache keys. Changes use
DesignPatch and normal transaction/verified-regeneration paths. Failed execution
restores prior model outputs and verified Agent application does not persist it.
The schema extension is additive. Missing radius_law defaults to None, and
migration never infers a variable law for old fillets. Both schemas and legacy/
current round trips cover this contract.

## Validation

Mock and OCCT contracts, analytic volumes/dimensions, parameter/cache edits,
orientation checks, topology/selection refusal, file/patch round trips, an example,
authoring guide/skill and eval task accompany this change. For an 80 × 60 × 6 mm prism and one 80 mm right-angle edge, the circular-corner
reference removal is `(1 - π/4) L (r0² + r0 r1 + r1²)/3` (all SI lengths).
Equal radii, 0.5→1.5 mm and 1.5→0.5 mm cases are tested with removed-volume
relative tolerance 1e-3 and endpoint contact tolerance 2e-6 m. The variable surface
is approximated, while the equal-radius limit has an exact cylinder reference.
Physical host testing beyond this Linux environment remains separate evidence.
