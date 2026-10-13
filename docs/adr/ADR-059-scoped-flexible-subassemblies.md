# ADR-059: Scoped flexible subassemblies

Status: Accepted; P13-011 implemented, P13-012 admission specified by ADR-061

Tasks: MCAD-P13-010 (design), MCAD-P13-011 (articulated copies),
MCAD-P13-012 (parent-context constraints)

## Problem and decision

ADR-003 regenerates a child assembly and transforms its compound as one parent
instance. Child instance/joint IDs are local to that source. Two copies of the
same mechanism currently have no independent parent-owned internal coordinates;
parent mates address only root instances. Rigid nesting, BOM recursion and
exploded offsets do not establish flexible mechanical behavior.

Implement flexibility through **explicit scoped source references and a shared
neutral posed-leaf scene**, not by changing external child source, baking motion
into leaf placements or inferring joints from shapes. Separate the prerequisite
of independently posed copies from parent-context constraints. Only the latter
completes the original flexible-subassembly rubric. This ADR adds no runnable
command, source field or CAD capability by itself.

## Source ownership and scoped identity

The parent graph owns an explicit per-instance articulation configuration.
Each coordinate identifies a declared child joint using an ordered path of
ancestor `InstanceId`s plus the owning source `DocumentId` and `JointId`.
References are structured arrays/fields, never guessed slash-separated names;
identical local IDs under separate copies remain distinct. Runtime resolution
also binds each hop to the validated component/document source snapshot.

For P13-011, admit immediate declared child assembly instances and independent
child joint drivers. Keep nested flexible scopes and pattern-generated targets
unsupported until their scope/ownership semantics are implemented and tested.
Repeated ordinary declared copies of the same child are admitted. A rigid copy
uses the authored child zero configuration; a declared articulated copy stores
only its explicit driver displacements. Omitted coordinates use authored zero,
not a hidden previous evaluation or a guessed mate solution. Zero must satisfy
existing finite joint ranges.

Angular inputs require unit-bearing angle expressions, linear inputs length
expressions. Resolve through existing joint value admission; source normalization
uses explicit `position_rad`/`position_m` or equally explicit tagged SI fields.
Positions are displacements from authored zero, as in ADR-056. Reject unknown,
wrong-owner, suppressed, fixed, nonfinite, wrong-unit and out-of-range inputs.
Derive coupled followers once within their actual owning child scope. Explicit
follower inputs cannot override coupling, and must either be forbidden or checked
for agreement with the same published tolerance. Do not clamp or wrap angles.

Parent articulation edits use typed DesignPatch add/set/remove operations,
transactional validation, review/apply and opaque history. Removing articulation
must explicitly restore rigid/authored-zero behavior. Child source bytes never
change from editing a parent copy. Metadata, existing source expressions and
local IDs remain intact. Cross-scope constraint requests added in P13-012 must
also be graph-owned and use DesignPatch, not a UI-only pose override.

## Neutral evaluation and cache boundaries

1. Validate canonical source containment, exact child document identity, cycles
   and budgets using the existing recursive assembly rules. Capture source
   revisions/checksums used by review and revalidate dependencies before commit;
   a stale child must refuse the commit even if the parent revision is unchanged.
2. Resolve child mechanical rest placements and source joint ownership without
   mutating the child graph. Solve/coupling operations run on evaluation copies.
3. Apply admitted coordinates in the child frame, then compose every ancestor
   rigid placement in order. Parent translations/rotations are not reapplied
   inside the child mechanism and motion is not applied twice.
4. Return a kernel-neutral world-space leaf scene with structured instance paths,
   source document/revision, local instance/component ownership and explicit
   transforms. Preserve root compound results for compatibility, but never use
   a compound as a substitute for mechanical leaf identity.
5. Cache mechanical results by dependency source snapshots, complete scoped
   driver/coupling inputs and ancestor placements. Two instances sharing one
   component cannot reuse a body in the wrong internal pose. Presentation offsets
   remain separate and do not enter mechanical source or physical assertions.

Use the existing `GeometryKernel` transform/compound/mass/intersection boundary.
No concrete OCCT types or new hidden filesystem I/O enter core/graph/geometry.
Both Mock and OCCT must evaluate admitted neutral placements/analytic solids.
ADR-060 adds bounded Mock mesh member compounds. Unsupported exact intersections remain
explicit refusals; an AABB overlap must not be substituted for exact solid volume.

## Parent-context constraints (P13-012)

A parent mate identifies exact scoped leaf connectors, preserving their local
source definition. Connectors transform through the shared mechanical leaf scene.
Admit only a documented subset of existing mate kinds and kinematic child trees,
with explicit ownership and budgets; unsupported types or ambiguous solutions
refuse. Never fall back to treating the child as rigid while claiming flexibility.

Build one deterministic variable/constraint system containing admitted outer
placements and independent internal joint coordinates. Grounded coordinates,
coupled followers and repeated references do not get duplicate degrees of
freedom. Existing child mates remain internal constraints; parent constraints
must not be silently dropped or solved against stale pre-articulation transforms.
Resolve scopes in canonical structured-ID order. Explicit authored seed/branch
selection must be sufficient to distinguish admissible solutions; refuse
unresolved branch ambiguity, contradictory constraints and unconstrained free
coordinates rather than publish whichever local minimum happens to converge.

The required example has a parent mate driving a child link inside one copy,
while a second copy of the same child remains independent. Changing only the
parent constraint or its declared coordinate changes that internal pose and
verified physical results; parent and child source identities survive Undo/Redo.
An implementation limited to a saved static pose is P13-011, not P13-012.

## Physical reports and consumers

Every physical consumer must use the same resolved leaf scene or refuse the
articulated source before writing artifacts. This includes static bounds/volume,
tessellation, selection, pose inspection, assertions, motion checks, BOM,
exploded presentations, STEP/STL, URDF, animation, reach and the HTML viewer.
BOM quantities/provenance remain source-based while validating admitted sources;
pose changes do not create extra purchased parts. Root exploded offsets move a
child's posed leaves together without changing mechanical results.

Interference must include leaf pairs inside an articulated child, between its
leaves and parent siblings, and between copies. Use existing explicit broad-phase
metre and exact common-volume cubic-metre tolerances. Return scoped pair identity
in deterministic order; equal threshold means contact. A compound-level shortcut
cannot certify an internally colliding child as clear. Missing child geometry
fails a required report rather than disappearing from its candidate pairs.

The existing per-independent-root-joint sampling contract does not automatically
cover child coordinates or parent/internal combinations. For P13-011, unsupported
hierarchical motion, reach, URDF mechanics or viewer controls must refuse before
output, even when static geometry export is admitted. P13-012 may extend these
consumers only with faithful ownership and explicit sampling coverage. Do not
introduce adaptive/random sampling without a separate ADR, and do not claim a
continuous collision proof from the existing finite grid.

## Wire compatibility and rollback

Any source/schema change ships with `schemas/`, a versioned migration, expanded
and packed round-trip tests, history and explicit legacy-reader refusal. Merely
adding an optional collection to a serde struct is unsafe: old readers could
ignore articulation and display a plausible rigid assembly.

Implementation must introduce a mandatory wire discriminator on an existing
strictly parsed boundary (for example a new component source-kind variant) for
all articulation-bearing sources, and test it with a frozen old-reader model.
The exact wire representation belongs to P13-011's schema change; this document
is not a supported payload specification. Missing/orphan/mismatched guards
refuse in the new reader. Legacy rigid files migrate without changed geometry,
IDs or semantics, and an empty articulation collection does not invent motion.

Failed edits/reviews, dependency invalidation, missing references, failed solving,
regeneration or consumer admission preserve parent/child source, opaque history
and previously published outputs. Verify everything before atomic publication.
Do not claim multi-process file locking or multi-document atomic writes beyond
the repository's existing guarantees.

## Analytic acceptance and completion gates

Use independently specified box mechanisms, not expected output computed by the
implementation's pose helper. A 10 x 20 x 30 mm box has volume `6e-6 m³`;
two rigidly transformed copies have summed volume `12e-6 m³`. A 90 degree Z
rotation swaps X/Y extents, preserving Z and volume. A 25 mm prismatic
coordinate moves its centre by exactly `0.025 m` along the declared world axis.
Test nonidentity parent transforms with a separately derived matrix product.
Use explicit SI tolerances suitable for existing OCCT bounds and mass evidence.

Required implementation evidence:

- Both kernels: independent volumes, centres, dimensions and transforms for
  different copies, zero/rest preservation, coordinate changes and child hashes.
- Parent-context example: a scoped parent constraint moves a child link;
  independent copy isolation and correct DOF/coupled follower ownership.
- Native exact collision: child-internal, copy/copy and parent/child collisions
  whose rest configuration is clear. Mock refusal for unsupported exact geometry
  is explicit and separate from neutral candidate-pair/placement evidence.
- Missing/wrong-owner references, units/ranges, cycles, duplicate coordinates,
  conflicting/ambiguous scopes and stale child dependency refusal; no outputs
  replaced on failure. Warm/cold caches and input permutations agree.
- Expanded/packed migration, strict schema, old-reader refusal, DesignPatch
  authoring, semantic diff/change impact and opaque Undo/Redo canonical identity.
- Runnable nested example under `examples/`, MCP authoring guide and MusubiCAD
  skill updates, plus independently graded real-MCP eval when feasible.
- Every public consumer explicitly supports the scoped behavior or refuses it;
  no root-only report or export may silently discard flexibility.

P13-010 merges only design and plans, earning no maturity credit. P13-011 does
not by itself satisfy flexible parent-context behavior. Assembly 68 -> 70 can
be credited only after the complete admitted P13-012 workflow and evidence merge
main, without altering the fixed rubric or crediting unimplemented behavior.
