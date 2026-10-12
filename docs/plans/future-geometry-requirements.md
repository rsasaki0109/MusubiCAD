# Future geometry admission requirements

MCAD-P5-006 defines the evidence required before MusubiCAD starts NURBS
editing or adds another geometry-kernel feature. It deliberately does not
authorize an implementation. A candidate must satisfy every gate below in its
own roadmap task and ADR.

## Current baseline

`opencad_geometry::NurbsSurface` is a serializable, kernel-neutral data
structure with degrees, knots, weights, and control points. It is not wired to
the Design Graph, `.ocad` document schema, feature registry, `GeometryKernel`,
DesignPatch, CLI, Agent API, or Desktop commands. It therefore does not
constitute an editing feature. Cached B-Rep and meshes remain disposable.

## Admission gates for every candidate

Before implementation, the proposal must provide:

1. A bounded user story and explicit non-goals. One proposal covers one
   feature family, such as NURBS surface editing, loft, sweep, shell, or draft.
2. A kernel-neutral Design Graph representation that owns all authoring input.
   OCCT handles, cached B-Rep, tessellation, and GUI state may not become source
   data.
3. Unit and validation rules for every public number. Lengths use meters in
   runtime DTOs, angles use radians, and dimensionless values are labelled as
   such. Every geometric comparison names a finite, positive tolerance and its
   unit.
4. Deterministic ordering, semantic TopoRef production/rebinding rules, and
   regeneration failure behavior. A failed operation must leave the document
   and history unchanged.
5. A transaction and DesignPatch command contract. Any Desktop command must
   have CLI or Agent API parity and must not mutate the model directly.
6. A file-format decision. If persisted data changes, the same task updates
   `schemas/`, migration code, canonical serialization, checksums, and
   round-trip tests. An ADR is required before changing the schema.
7. A kernel-neutral `GeometryKernel` input/output contract plus Mock behavior.
   Concrete OCCT conversion stays in `modules/kernel-occt`; no OCCT type may
   leak into another crate.
8. An evidence plan covering pure model validation, geometry and OCCT
   integration, DesignPatch atomicity, file migration/round trip, regression
   goldens, a checked-in example, and API/architecture documentation.
9. Dependency and performance budgets. A large dependency, new FFI surface,
   or material regeneration-cost change requires justification in the feature
   ADR and reproducible measurements.

## Additional NURBS editing requirements

A NURBS proposal must resolve these questions before code changes:

- Define whether the first scope is surface creation, control-point editing,
  knot/weight editing, trimming, continuity constraints, or a smaller subset.
- Specify control-grid dimensions and indexing order; permitted degrees;
  complete knot-vector convention and multiplicities; clamped, periodic, and
  rational behavior; and finite positive weight rules.
- Label control-point coordinates in meters and knots/weights as dimensionless.
  Define tolerances for knot monotonicity, coincident control points, weight
  validation, closure, and continuity checks.
- Define stable IDs for the surface, control points, trims, and generated
  semantic faces/edges. Array position alone is not a stable identity.
- Decide whether the existing `NurbsSurface` DTO is migrated, wrapped by a
  feature definition, or replaced. Its current field layout must not be
  silently reinterpreted.
- Define round-trip and regeneration goldens for both rational and
  non-rational cases, including invalid and degenerate inputs.

## Additional kernel-feature requirements

A non-NURBS kernel operation must define its kernel-neutral feature inputs,
operation semantics, output-body behavior, supported failure modes, and
semantic topology roles. It must provide Mock and OCCT tests using the same
contract, document all tolerances, and show that a failed regeneration cannot
replace the last valid Design Graph state.

## Exit criterion

The gate is passed only when a follow-up roadmap task has an accepted feature
ADR answering the applicable questions above, reviewable API/schema sketches,
and named tests. Until then, NURBS editing and new kernel features remain
deferred; the existing `NurbsSurface` type must not be advertised as editable.

See [ADR-012](../adr/ADR-012-future-geometry-admission-gate.md).

## Scoped closed cubic admission

P13-008 uses [ADR-057](../adr/ADR-057-closed-cubic-sketch-spline-admission.md),
accepted before implementation, for a closed single-span non-rational cubic
sketch curve and normal NewBody extrusion. It resolves the gate's source IDs,
fixed knot/weight conventions, units, neutral DTO, Mock/native oracle, budget,
wire/refusal and named acceptance tests. This admission does not authorize
surface NURBS, general spline loops or editable knots/weights. See
[the scoped API](../api/cubic-spline.md).
