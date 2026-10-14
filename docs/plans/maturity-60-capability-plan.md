# Proposal for the user's approximately 60% objective

The original fixed rubric, FreeCAD = 100 comparison, weights and historical
50% target remain unchanged. This is an additional capability plan, not a
replacement rubric or a score award. Baseline after PR #97 remains:
solid 55, sketch 60, assembly 70, drawing 50, I/O 60, GUI 45, ecosystem 14;
weighted general CAD 50.10; agent operation 90.

A working allocation for additional capabilities is solid 70, sketch 70,
assembly 75, drawing 65, I/O 60, GUI 65, ecosystem 14: weighted 60.10.
These are estimates, not guaranteed increments. Actual credit requires a
main merge, explicit implemented behavior, independently verified acceptance
and a ledger entry explaining the comparison. Reduce/withhold credit for a
partial capability; add further work if the evidence does not warrant 60%.
No extra host/signing credentials are assumed. Do not credit existing tangent,
symmetry, helix sweep, shell/loft, circle/ellipse, or basic feature editing twice.

Keep one task per PR and do not stack. Register canonical IDs in roadmap.md
before task implementation. Each new phase must have objective, dependencies,
ID table, completion definition, tests and risks. P15-009 is complete in PR #101;
v0.3.1 was published and independently verified.
MCAD-P16-001 registers this plan and ADR-062; the next implementation task is
MCAD-P16-002. Only Phase 16 IDs are accepted here. Following phases remain
candidates until their purpose/dependencies/IDs/done/tests/risks are registered
in the roadmap before implementation.

## Next phase: broader manufacturing modeling

Start by planning and accepting the admission/compatibility contract. Current
ordinary Mock profiles/extrusions/Booleans use synthetic body IDs and synthetic
mass; ellipse/cubic/imported-mesh paths have bounded genuine analytic data.
Do not label ordinary Mock ID arithmetic as a physical volume proof. Decide an
explicit, tested analytic primitive/Boolean admission extension before requiring
new seats to pass genuine Mock volume/dimension checks; native measurements stay
independently graded. This prerequisite earns no CAD feature credit by itself.
Then:

- Counterbored standard holes: explicit larger seat diameter/depth, parametric
  edits, arbitrary admitted sketch plane, analytic stepped-cylinder removal,
  target source/face ownership and fully protected failure/history. Distinct
  closed feature/seat tags must make older readers refuse new solid semantics.
- Countersunk seats: explicit diameter/included angle with derived frustum
  depth/volume, independently checked contact dimensions and unit/range refusal.
- Curved/native draft: analytic cylinder-to-frustum cases, exact owned face
  admission and neutral-plane semantics; no lifting planar guards without
  genuine Mock/native support and volume/contact evidence.
- Tapered ribs: explicit thickness/height/draft/end choices and independently
  computed cross-section integrals. Unsupported curved intersections refuse.
- Standard physical external threads: accepted representation/profile/handedness/
  end-run contract, actual native geometry, independent pitch/crest/root/volume
  checks. Existing generic helix sweep is not sufficient new evidence.
- Broader variable fillets: accepted radius-law/contour contract, analytic
  volume/contact tests for genuinely expanded support. Do not award general
  curved-edge support for only another straight-edge special case.

All CAD features require both Mock and OCCT, independent analytic tests,
examples, MCP guide and skill, feasible actual MCP eval, schema/migration/
round trips when the document changes, source/undo/refusal isolation and all
required pre-push/native CI. Architecture/numeric/representation choices get ADRs.

## Following phase: a usable desktop creation workflow

Preserve ADR-058 backend ownership and shared DesignPatch gateways. Proposed
increments come from actual native controls, not additional JSON fields:

- New sketch creation on explicit global and admitted datum workplanes.
- Pointer-driven circle/arc/ellipse/rectangle drafting with view-only overlays,
  unit-bearing source commands, reviewed solve/physical preview and atomic apply.
- Dimension/constraint interaction, backend DOF/contradiction diagnostics and
  construction/trim/offset operations. Existing line/point forms earn no repeat credit.
- Broader feature creation/edit controls for hole, revolve, loft/sweep, shell,
  fillet/chamfer and patterns as admitted by existing native backends.

Each increment needs backend/CLI-Agent/native Tauri parity, analytic Mock/OCCT
physical results, meaningful failure and history tests, actual Windows form/
pointer evidence, examples/guide/skill/eval. Keep implementation details out of
product flows and never write graph/solver state from frontend JavaScript.

## Following phase: associative sketch and drawing coverage

Sketch candidates: circle-circle/internal-external tangency with explicit branch,
ellipse axes/orientation/tangency constraints, and regeneration-owned parametric
offsets with stable source/profile identity. Existing line-circle tangency,
symmetry and literal convex offset planning are already implemented. NURBS editing
stays behind ADR-012; no new sampling method without an ADR and acceptance evidence.

Drawing candidates: exact curved sections (arcs/circles and analytic area/hatch),
semantic section/detail dimensions and assembly views/dimensions using scoped
physical leaf ownership. Full SVG/PDF/DXF and refusal/source tests must agree;
do not rebind a projected position heuristically or treat mesh wireframes as
analytic CAD geometry.

## Following phase: flexible mechanism consumers

Shared scoped child-driver pose/motion API and bounded hierarchical moving-pair
interference checks; transparent sampled coverage, zero-DOF context binding and
whole dependency snapshot verification. Then unit-correct rack/coupled export and
interactive viewer controls where mathematically representable, with explicit
refusal elsewhere. Do not claim continuous collision proof or unsupported URDF
mimic relationships. This phase earns credit only for expanded usable assembly
workflows, not reusing the existing parent-context solve results.

## Ecosystem work

v0.3.1 official release and downloaded dual tutorial verification are complete
(PR #101), with no maturity increment.
Continue tutorials/releases and analytic evals throughout, but keep ecosystem 14
until real additional host evidence warrants an increment. Cursor and authenticated
Gemini sessions remain unavailable/unverified; do not simulate success or weaken
protected desktop signing/publication policies. No maturity increment for plans,
packaging alone or unavailable hosts.


## P16-003 merge-conditioned evidence ledger

After the P16-003 implementation main merge: solid 55 → 57; fixed weights and
all other scores stay unchanged. Weighted general CAD 50.10 → 50.60 (about 51%);
agent operation 90 → 91. Credit is for the distinct parametric counterbore source,
original-stock/end-face ownership, both-kernel physical measurements and closed
output faces, strict schema/reader admission, source/history/refusal isolation,
example/guide/skill and actual source-aware MCP workflow. The constrained stock
scope gets partial credit; no general arbitrary-face hole, host or release claim.
P16-001/002/009/010 infrastructure receives no feature credit. Before the feature
merge the actual baseline remains 50.10/90; the 60.10 working allocation is still
an estimate that must be justified by later merged capabilities.

P16-011 is merged in PR #107 with no additional score. P16-004's countersink
implementation may earn solid 57 → 59 (weighted 50.60 → 51.10), agent 91 → 92
only after its main merge: real conical seat/bore union, typed parametric angle,
independent contact/volume/COM/closed meshes in both kernels and protected
source/MCP/history. No arbitrary-face machining, inferred head standard or
additional host/release support is credited. Until then use 50.60 / 91.


## P16-005 merge-conditioned evidence ledger

Only after P16-005 merges to main: solid 59 → 61, weighted CAD 51.10 → 51.60
(about 52%) and agent operation 92 → 93. Credit is partial for actual admitted
full-cylinder curved draft with signed SI angle/neutral plane, owned source and
physical output, independent Mock/native contact/integral evidence, closed
schema/old-reader refusal and protected actual MCP/Agent history. No general
curved/concave/NURBS, host, release or P16-012 infrastructure credit. The fixed
rubric/weights and other scores remain unchanged; 60% remains an additional goal
requiring further merged capabilities.


## P16-006 merge-conditioned evidence ledger

After P16-006 main merge only: solid 61→63, weighted CAD 51.60→52.10
(about 52%) and agent operation 93→94. Partial credit for square-ended true tapered
stock addition, owned original source/end/line, strict parametric SI dimensions,
independent both-kernel contact/integrals and source/schema/refusal/cache/history/
actual MCP evidence. No arbitrary curved/multiple rib, existing constant-rib,
P16-013 runtime, host, release or planning credit. Fixed rubric/weights and
other scores stay unchanged; the approximately 60% objective remains active.
