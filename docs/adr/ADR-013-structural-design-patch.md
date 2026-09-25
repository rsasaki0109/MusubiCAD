# ADR-013: Structural DesignPatch operations

Status: Proposed  
Date: 2026-09-25  
Roadmap: MCAD-P7-001 (proposed)

## Context

`DesignPatch` is the only sanctioned mutation path for AI and automation
(AGENTS.md §7), and ADR-009 makes it the shared contract for Desktop, CLI, and
Agent API. Every current `PatchOperation` edits a value that already exists:
`SetParameter`, `SetFeatureExpr`, `SetFeatureRef`, `AssignFaceRef`,
`SetInstancePlacement`, `SetMateDistance`, `AddConnector`,
`SetDrawingViewScale`, and `SetDrawingViewOrigin`
(`modules/ai/src/patch.rs`).

As a result, no supported surface can author a new design:

- an agent cannot add a parameter, sketch, sketch entity, constraint, or
  feature, and cannot remove or reorder one;
- the Desktop creates documents only from the sixteen Rust-coded
  `DocumentTemplate` variants and then edits parameters;
- hand-writing `.ocad.d` JSON bypasses the transaction, validation, history,
  and review contracts and must also reproduce `checksums.json`.

The review, provenance, assertion, trace, and merge machinery built in Phase 6
therefore only ever sees parameter variations of built-in models. Structural
editing must be added to `DesignPatch` rather than to a parallel API, so that
dry-run, apply, history, rebase, review, and future semantic merge (P6-005)
inherit it without a second mutation path.

Two further facts constrain the design:

- `DesignState` (`modules/ai/src/state.rs`) covers parameters, feature nodes,
  semantic references, and optional assembly/drawing models. It does **not**
  include sketches, the persisted `feature_graph` (`order`, `edges`), or
  design assertions. The `musubicad.design-state.v1` revision digest therefore
  cannot detect a concurrent sketch edit.
- `graph/features.json` persists both `feature_nodes` and a `feature_graph`
  with explicit `order` and `depends_on` edges. If both are authorable, they can
  disagree.

## Decision

### 1. New operations

`PatchOperation` gains the following variants. All IDs are strings with the
existing namespace prefixes (`param:`, `sketch:`, `ent:`, `con:`, `feature:`,
`ref:`, `assertion:`). All lengths inside embedded definitions follow the existing
DTO conventions (meters, radians, explicitly labelled dimensionless values);
this ADR does not introduce new numeric fields.

| Group | Operation | Payload |
|---|---|---|
| Parameters | `add_parameter` | `id`, `name`, `expr` |
| | `remove_parameter` | `id` |
| Sketches | `add_sketch` | `id`, `name`, `workplane` |
| | `remove_sketch` | `id` |
| | `add_sketch_entity` | `sketch_id`, `entity` (existing `SketchEntity` JSON) |
| | `remove_sketch_entity` | `sketch_id`, `entity_id` |
| | `add_sketch_constraint` | `sketch_id`, `constraint` (existing `Constraint` JSON) |
| | `remove_sketch_constraint` | `sketch_id`, `constraint_id` |
| Features | `add_feature` | `node` (existing `FeatureNode` JSON), `position` |
| | `remove_feature` | `id` |
| | `move_feature` | `id`, `position` |
| | `set_feature_suppressed` | `id`, `suppressed` |
| | `replace_feature_definition` | `id`, `definition` (same `feature_type` only) |
| References | `remove_semantic_ref` | `ref_id` |
| Assertions | `add_assertion` | `assertion` (existing `Assertion` JSON) |
| | `remove_assertion` | `id` |

`position` is one of `{"at": "end"}`, `{"after": "<feature id>"}`, or
`{"before": "<feature id>"}`.

Structural assembly operations (add/remove component instance and mate) and
drawing operations (add/remove view) use the same rules and are specified in a
follow-up slice of this ADR's roadmap task rather than a new ADR.

Rename operations are intentionally absent. IDs are identities; a display
`name` can change through a future `set_*_name` operation without touching
references.

### 2. IDs are chosen by the patch author

Every created object carries an explicit, author-chosen ID. The host never
allocates IDs and there are no placeholder or temporary IDs.

- Creating an object whose ID already exists in the staged candidate is a
  validation error, including an ID removed earlier in the same patch. Reusing
  an ID for a different object inside one patch is therefore impossible.
- The ID grammar is `<prefix>:[a-z0-9_]+(\.[a-z0-9_]+)*`, at most 128 bytes.
- Later operations in the same patch refer to the new object by that same ID.

Rationale: host allocation would make the resulting document depend on the
allocator, would let two independent branches produce the same generated ID for
different objects, and would force the review to show IDs that the patch author
never saw. Author-chosen IDs keep dry-run, apply, rebase, and merge
deterministic and make the review read exactly like the patch. Agents and the
CLI can use a pure helper that suggests a free ID derived from the `name`; the
helper is not part of patch semantics.

### 3. Sequential staging, whole-candidate validation

Operations apply in order to the single staged candidate already used by
`build_patch_candidate`. Each operation performs only **local** checks: the
payload deserializes and validates on its own, the object it mutates or removes
exists, and a created ID is free.

**Referential and structural validation runs once, on the final candidate,**
before commit:

- every expression names only existing parameters, with no parameter cycles;
- every sketch entity reference, constraint target, and `profile_ref`
  resolves inside its sketch;
- every feature input (`sketch_feature`, `target_feature`, pattern sources,
  `face_ref`/`plane_face_ref`, and so on) resolves;
- every semantic reference's `created_by` names an existing feature;
- every assertion target exists;
- feature order is a valid topological order of the derived dependency edges
  (section 4).

Intermediate states may be invalid; the final state may not. Any failure rejects
the whole patch and leaves the document and history byte-for-byte unchanged, as
MCAD-P3-001 already requires. Errors are reported in deterministic order (by
check category, then stable ID).

A patch is limited to 10,000 operations and each embedded definition to the
existing file-level size limits. The limit protects hosts from unbounded agent
input and is part of validation, not transport.

### 4. The feature graph is derived, not authored

`feature_nodes` order is the authored construction order. `feature_graph.edges`
and `feature_graph.order` become a **derived projection** computed from
`feature_nodes` by one pure function in `modules/graph`:

- edges are produced from each definition's declared inputs, in feature order,
  then input-field order, with duplicates removed;
- `order` equals the `feature_nodes` order;
- the function is the only writer of `feature_graph`; patches cannot set it.

`add_feature` and `move_feature` place the node at `position`. If the result
places a feature before any of its inputs, validation fails and names the first
violating edge. There is no automatic reordering.

Before this derivation is switched on, the implementation must prove that it
reproduces the persisted `feature_graph` of every checked-in example and fixture
byte-for-byte. A disagreement is a bug in either the derivation or the fixture
and must be resolved explicitly, not by regenerating goldens.

### 5. Removal fails closed and never cascades

A removal succeeds only if nothing in the final candidate still depends on the
removed object. The host never deletes dependents implicitly. To remove a
feature together with its consumers, the patch removes each consumer explicitly.
The same patch may do this in any order, because validation runs on the final
state.

When a removal leaves dangling dependents, the error lists every dependent ID in
sorted order, grouped by kind (feature, semantic reference, assertion,
constraint, drawing view, assembly mate). Agents can therefore produce the
complete, reviewable follow-up patch without guessing.

This extends the MCAD-P6-003 principle to structure: the system refuses an edit
it cannot explain rather than repairing it silently.

### 6. `DesignState` v2 and revision preconditions

`DesignState` gains `sketches` and `assertions`. It does not carry
`feature_graph`: section 4 makes it a pure function of `feature_nodes`, so
hashing it would add no information and would couple the digest to the
derivation code. The canonical revision representation becomes
`musubicad.design-state.v2`, using the same envelope, key canonicalization,
and SHA-256 rules as ADR-008. v2 always serializes both new arrays, even when
empty; v1 bytes remain byte-identical to ADR-008, so recorded v1 digests keep
verifying.

- `RevisionEquals` with version `v2` is verified against the v2 canonical bytes.
- `RevisionEquals` with version `v1` is still accepted, but **only** for patches
  that contain no operation introduced by this ADR. A structural patch guarded
  by v1 is rejected with a deterministic "revision version too weak for
  structural operations" error, because v1 cannot observe sketch edits.
- New precondition variants `sketch_exists`, `sketch_entity_exists`, and
  `assertion_exists` complete the existing `feature_exists` and
  `topo_ref_exists` set.

### 7. Diff, review, rebase, and merge

- `DesignDiff` reports `added`, `removed`, and `moved` entries per kind with
  stable IDs, in sorted order, in addition to today's value changes. The review
  HTML and GitHub summary render them. `ChangeImpact` treats an added or moved
  feature, and every feature after it, as dirty; a removed feature dirties its
  former downstream suffix.
- `rebase_patch` extends conflict detection:
  - an add whose ID now exists with different canonical content produces an
    `add_add` conflict; identical content rebases to a no-op;
  - removing or replacing an object the new base changed produces a
    `remove_modify` conflict;
  - a `position` anchor missing from the new base produces an `anchor_missing`
    conflict.
- For P6-005 three-way merge, two sides that insert different features at the
  same anchor are ordered by ascending feature ID after the anchor. This makes
  the merge result independent of which side is "ours" and yields identical
  canonical bytes in both merge orders, as P6-005 requires. If that order
  violates a dependency, the merge reports an `order` conflict instead of
  choosing.

### 8. File format and schemas

The `.ocad`/`.ocad.d` document schema does not change. Every object created by
these operations already has a persisted representation, so no migration is
required. `feature_graph` stays in `graph/features.json`; only its writer
changes.

`schemas/ocad.patch.schema.json` gains the new operation and precondition
variants. Older binaries reject unknown operation `type` values during
deserialization, which is the required fail-closed behavior. No patch version
field is added.

### 9. Surface parity and boundaries

- CLI `opencad patch`/`review` and the Agent API `patch_*` methods accept the
  new operations with no new methods, because they already transport a
  `DesignPatch`. Desktop commands that create or remove objects must build a
  `DesignPatch` (ADR-009).
- Feature definitions in `add_feature` are limited to the closed
  `FeatureDefinition` enum. Plugin features keep the ADR-010 invocation path.
  No new geometry operation is introduced, so the ADR-012 admission gate is not
  triggered.
- The work stays within the existing module rules: operation types and
  validation live in `modules/ai`; graph derivation lives in `modules/graph`;
  `modules/file` persists the result; no OCCT types are involved.

## Alternatives considered

- **Host-allocated or `$temporary` IDs.** Friendlier for one-shot agent output,
  but they make results depend on allocation state, break merge commutativity,
  and hide the final IDs from the review. Rejected in favour of author-chosen
  IDs plus a suggestion helper.
- **Whole-document replacement (`set_document`).** Trivial to implement, but it
  turns every diff into a file diff and makes rebase and merge meaningless.
  Rejected.
- **Cascading removal.** Convenient, but silently deletes intent the author did
  not name and hides the scope of the change from reviewers. Rejected; the error
  lists the dependents instead.
- **Per-operation validation.** Simpler errors, but forces authors to order
  operations so that every intermediate state is valid, which is impossible for
  some edits, such as swapping a feature's sketch. Rejected in favour of
  final-state validation.
- **A separate authoring API outside `DesignPatch`.** It would duplicate
  transactions, history, and review, and violate AGENTS.md §7. Rejected.

## Delivery slices

Each slice is one PR with its own tests and docs:

1. `DesignState` v2, revision rules, and parameter/assertion add/remove.
2. Sketch, entity, and constraint add/remove with final-state validation.
3. Feature graph derivation, proven against every fixture.
4. Feature add/remove/move/suppress/replace, `DesignDiff`/`ChangeImpact`/review
   rendering.
5. Rebase conflicts (`add_add`, `remove_modify`, `anchor_missing`).
6. Structural assembly and drawing operations.

## Acceptance evidence

- **Rebuild from nothing.** Starting from an empty part document, one patch
  per slice-4 capability rebuilds `examples/bracket.ocad.d`. The resulting
  `graph/*.json` must equal the checked-in files as canonical bytes.
- **The flagship through patches.** The 22-node `robot_joint_actuator` Feature
  Graph is reproduced by a checked-in patch sequence, and its regenerated mass
  and bounds match the P5-005 golden within the existing unit-labelled
  tolerances (OCCT integration test).
- **Atomicity.** For every new operation, a failure-injection test proves that
  a failing final validation leaves the document, history, and revision digest
  unchanged.
- **Dry-run/apply parity.** Dry-run and apply return identical validation
  errors and diffs for every structural fixture.
- **Removal.** Removing a feature with consumers fails and lists all
  dependents in sorted order. Removing the same feature together with its
  consumers in one patch succeeds.
- **Determinism.** Applying the same patch twice yields identical canonical
  bytes. The P6-005 merge fixtures with same-anchor inserts produce identical
  bytes in both merge orders.
- **Revision.** A v1-guarded structural patch is rejected, and a v2 guard
  detects a concurrent sketch edit.
- **Schema.** `ocad.patch.schema.json` validates every new example patch under
  `examples/agent/`, and an unknown operation is rejected.

## Consequences

- Agents, the CLI, and a future text or Python authoring layer can create
  designs through the same validated, reviewable, undoable path as parameter
  edits.
- Reviews and merges gain structural diffs without a second mutation system.
- Author-chosen IDs push naming responsibility onto patch authors; the
  suggestion helper and clear collision errors mitigate this.
- Fail-closed removal makes deletions verbose, but every one is explicit and
  reviewable.
- Revision v2 enlarges the canonical state and its digest cost, roughly in
  proportion to sketch size.
- `feature_graph` becomes derived data; any future feature input field must be
  registered with the derivation function, or validation will not see the
  dependency. A unit test enumerates every `FeatureDefinition` input field to
  guard this.

## Open questions

- Whether `replace_feature_definition` should also allow changing
  `feature_type`. Currently it does not: a type change is a remove plus an add,
  which keeps semantic reference ownership explicit.
- Whether `add_sketch` should accept a whole sketch, with entities and
  constraints, in one operation to shorten agent patches. It could be added
  later as sugar that expands to the primitive operations before validation.
- Where the empty-part template lives: a `DocumentTemplate::EmptyPart` variant
  or `opencad new --empty`.
