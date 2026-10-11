# ADR-034: Semantic linear drawing dimensions

## Status

Accepted for MCAD-P10-003.

## Context

Linear dimensions store fixed model-space point pairs. A resized or topologically
changed model can leave a plausible, stale label in an otherwise regenerated
drawing. Projected mesh segments and OCCT enumeration positions cannot serve as
authored identities. Phase 10 requires unresolved dimensions to fail closed.

## Decision

Add a semantic linear measurement to `LinearDimension`: `edge_ref: TopoRefId`
names a reference in the part referenced by that dimension's view. It measures
the straight-line distance between the edge's two end vertices, in model meters;
it does not measure curve arc length, radius or diameter. Closed/degenerate edge
chords and collapsed projections are rejected at `1e-9 m` model tolerance.

The wire format has two exclusive variants: legacy `start_model_m` plus
`end_model_m`, or `edge_ref`. There are no cached coordinates or editable labels
in a semantic dimension. Untagged deserialization migrates legacy point pairs
without inventing a geometric association; their canonical wire representation
is preserved. This additive change retains document version v0.1 and updates
both drawing and DesignPatch schemas with migration/round-trip tests.

Regeneration verifies referenced document identity, regenerates its final active
body, and uses P6-003 edge provenance with `required=true`. Exact and unique
fingerprint matches are accepted; missing, ambiguous, consumed, duplicated or
wrong-kind bindings are refused, naming the dimension and reference. It never
uses a projected segment index or falls back to stored measurement points.

`GeometryKernel::edge_endpoints` exposes only two `[f64; 3]` positions in meters.
OCCT conversion and its 1-based final-body edge enumeration remain inside
`kernel-occt`. Mock exposes the top edges of its existing deterministic cube.
Endpoint ordering is lexicographic so reversing kernel edge orientation does
not change annotation placement. No dependency or sampling algorithm is added.

The file-backed drawing regenerator validates all sheets/views/dimensions before
CLI/Agent/MCP regeneration, dry-run, apply, review or SVG export succeeds. SVG
still exports one sheet but cannot hide a bad later sheet. Derived point-resolved
sheets are transient and never replace the authored drawing in the document.
Failed verified patches preserve document/history/revision and failed SVG
exports preserve existing output. Authors add/remove dimensions through existing
DesignPatch commands; replacing a measurement uses a new dimension ID under the
existing no-ID-reuse rule. Backend transactions remain unchanged.

## Scope and limitations

Semantic edge bindings initially address part models and their active body.
Legacy point dimensions on assemblies remain supported; an assembly semantic
binding is refused until instance-qualified references have a separate design.
Face-to-face, angle, radius and diameter dimensions belong to later roadmap
tasks. Existing P6-003 discovery/role/fingerprint limits still apply: a changed
topology that has no unique supported match produces an error requiring an
explicit new binding. Kernel enumeration is a runtime lookup after resolution,
not a persisted drawing identity.

Each view regenerates its referenced model once per drawing pass. Cost is bounded
by the existing model regeneration, edge discovery and one endpoint query per
semantic dimension; no pairwise mesh search or new cache invalidation scheme is
introduced. Cross-view caching is deferred.

## Evidence

- `dimension.rs` tests: Mock chord, canonical orientation, stale ID fallback,
  missing/ambiguous candidates, legacy and semantic wire forms, mixed rejection.
- `semantic_drawing_migration.rs`: expanded-file persistence and reconstruction
  through structural DesignPatch authoring.
- `semantic_drawing.rs`: OCCT bracket width 80 -> 100 mm, real MCP dry-run/apply,
  missing later-sheet references, ambiguity, document identity, atomic refusal
  review refusal, closed OCCT circle rejection and deterministic SVG output.
  Width agrees with the analytic model dimension
  within `1e-9 m`; existing bracket volume tests cover solid regeneration.
- `examples/semantic_bracket_drawing.ocad.d` and its authoring patch demonstrate
  the binding. API/architecture docs, MCP guide and agent skill explain it.

## Alternatives

Persist projected edge indices or measurement coordinates: permits stale or
arbitrarily rebound measurements. Infer semantic references while migrating
legacy points: invents design intent and can choose among equally near edges.
Introduce face/vertex/assembly addressing together: expands one task into several
new contracts without focused acceptance evidence.
