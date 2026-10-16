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
MCAD-P16-001 registered this plan and ADR-062 before MCAD-P16-002.
Phase 16's registered tasks are now merged. Phase 17 is registered
in roadmap.md with purpose/dependencies/IDs/done/tests/risks; P17-001 accepts
ADR-102 before P17-002 implementation. Later phases remain candidates until
their corresponding roadmap registration, with no advance maturity credit.

## Phase 16: broader manufacturing modeling (merged)

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

## Phase 17: a usable desktop creation workflow

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

Phase 17's first task, P17-001 / ADR-102, defines creation/workplane admission
without adding controls or points. P17-002 adds atomic global-plane sketch and
owner creation; P17-003 adds source-bound existing datum planes and their
resolved view frames. Pointer, constraint and feature-form tasks follow in the
registered roadmap order. Blank-document file creation and arbitrary custom/
face-plane editing are not implicitly included. Acceptance requires actual new
Linux Tauri/Windows WebView2 interactions and independent both-kernel physical
results, including absolute placement and equal-volume wrong-placement controls.
Existing Phase 14 operations receive no duplicate credit. After P17-005 /
PR #153, actual weighted CAD is 53.50 and agent operation 95. Further feature
credit requires its own main merge and explicit justified evidence ledger.

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

PR #112 merged P16-006 to main as 2c8d8a8 after exact-head workflow/job success.
Its conditional credit is earned: solid 63, weighted CAD 52.10, agent operation
94. P16-014's nominal thread profile contract/calculation earns no increment;
physical P16-007 construction and source deliverables remain pending. The
original weights, comparison and additional approximately 60% goal are unchanged.

## Current merged baseline and P16-008 scope

P16-007 merged in PR #116 (`508e7c8`), as recorded in implementation-status.md:
solid 63→65, weighted CAD 52.10→52.60, agent operation 94→95. Its partial credit
is for physical nominal metric external threads with source/phase/ref/reader/
transaction/native MCP acceptance; do not credit the prerequisite diagnostics
again. The fixed rubric, weights and approximately 60% objective are unchanged.

P16-008 supplies an explicitly named numerical faceted circular-rim feature under
ADR-101, with actual persistence/edit/cache/MCP use. All retained supports are
also facets; exact smooth/G1 support and general variable contours remain
outside this bounded admission. This PR assigns no additional fixed-rubric or
agent-operation points. Keep 52.60 / 95, including after its source merge, until
further independently accepted capabilities justify an increment.

## P17-002 merged evidence ledger

P17-002 merged in PR #150 with required Linux/Windows native interactions
and all applicable exact-head CI passed: GUI 45→47, weighted CAD
52.60→52.90 (about 53%). Scores at that merge were GUI 47, weighted CAD 52.90
and agent operation 95.
Partial credit is for the new usable name/plane sketch creation lifecycle,
backend source/owner atomicity, reviewed apply and canonical history, actual
sketch-only behavior and independent placed physical results. Existing geometry
and old editing forms earn no repeat credit. No additional agent-operation or
ecosystem credit: generic authoring already existed, and protocol/UI automation
does not establish a new external model-host session. At that merge, datums,
new pointer tools, blank-document creation and the full GUI 65 target remained further work. Fixed
rubric weights and the additional approximately 60% objective are unchanged.

## P17-003 merged evidence ledger

P17-003 merged in PR #151 with all applicable exact-head CI and its new Linux
Tauri / Windows WebView2 interaction acceptance: GUI 47→49, weighted CAD
52.90→53.20 (about 53%); agent operation remains 95. Partial credit is the new
usable datum choice/local-edit lifecycle with source association, regenerated
frame and upstream parameter motion/refusal/history. Existing datum modeling,
ellipse geometry, global sketch creation and prior basic forms earn no repeat
credit. Independent both-kernel absolute placement and actual native controls
are required; protocol automation does not establish a new external model-host
session. Fixed weights and comparison targets remain unchanged.

## P17-004 merged evidence ledger

P17-004 merged in PR #152 with all applicable exact-head CI and its new Linux
Tauri / Windows WebView2 pointer acceptance: GUI 49→50, weighted CAD
53.20→53.35 (about 53%); agent operation remains 95. Partial credit is the
usable center/radius pointer gesture with disposable preview, atomic reviewed
source publication, cancellation and protected history. Existing circle geometry,
numeric circle forms, workplane creation and prior pointer line editing earn
no repeat credit. Independent placed Mock/OCCT physical results and actual new
native pointer interactions are required. No external model-host increment.

## P17-005 merged evidence ledger

P17-005 merged in PR #153 with complete exact-head CI and actual new Linux
Tauri / Windows WebView2 rectangle interactions: GUI 50→51, weighted CAD
53.35→53.50 (about 54%), agent operation 95 unchanged. Partial credit covers
the usable opposite-corner gesture, reviewed backend four-point/four-line
expansion, cancellation and atomic source/history. Existing rectangle geometry,
point/line forms, workplanes and prior pointer tools receive no repeat credit.
Both-kernel placed physical measurements and independent actual MCP grading
passed. No external model-host increment; original weights remain unchanged.

## P17-020 prerequisite ledger

P17-020 closes physical analytic Mock arc-sector parity needed by P17-006.
OCCT already supports exact line/arc extrusion, so this prerequisite receives
no CAD or agent-operation increment. Current scores remain weighted CAD 53.50,
GUI 51 and agent operation 95 until a later qualifying feature main merge.
