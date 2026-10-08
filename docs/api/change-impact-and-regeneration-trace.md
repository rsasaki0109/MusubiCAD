# Change impact and regeneration trace

MCAD-P6-001 adds two serializable, kernel-neutral evidence contracts. They are
observations of the Design Graph; neither DTO owns document state, B-Rep, mesh,
or a cache.

## `ChangeImpact`

Patch dry-run returns `impact` beside validation and semantic diff:

```json
{
  "version": "opencad.change-impact.v1",
  "no_op": false,
  "changed_inputs": [{ "kind": "parameter", "id": "param:upper_hub_height" }],
  "directly_affected_nodes": ["feature:upper_hub", "feature:shaft_bore"],
  "predicted_dirty_nodes": [
    "feature:upper_hub", "feature:shaft_bore", "feature:counterbore",
    "feature:pcd_fasteners", "feature:radial_ribs",
    "feature:mounting_ears", "feature:mounting_holes"
  ]
}
```

Document-backed dry-run has the complete Feature Graph and sketches, so it
returns dependency-propagated nodes in deterministic topological order.
In-memory Agent calls without that authoring context return the directly
affected feature nodes. A patch that produces no semantic diff sets `no_op` and
returns empty node lists.

## `RegenerationTrace`

Every part regeneration returns:

```json
{
  "executed_nodes": ["feature:sketch_base", "feature:extrude_base"],
  "skipped_nodes": [],
  "solver_call_count": 1,
  "geometry_kernel_call_count": 3,
  "elapsed_time_ms": 2,
  "output_hashes_sha256": { "feature:extrude_base": "<sha256>" },
  "trace_hash_sha256": "<sha256>"
}
```

`geometry_kernel_call_count` is collected by a borrowing adapter around the
kernel-neutral `GeometryKernel`. `output_hashes_sha256` identify logical feature
outputs from canonical feature inputs and ordered upstream hashes; they do not
serialize kernel handles. The trace hash includes execution order, skipped
nodes, call counts, and output hashes. It deliberately excludes
`elapsed_time_ms`, because elapsed time is explicit millisecond evidence but is
not deterministic identity.

`opencad_desktop::apply_patch_and_regenerate_with_trace` returns prediction and
execution evidence together and commits only after regeneration succeeds. A
no-op patch returns `RegenerationTrace::no_op()`, performs zero solver/kernel
calls, and leaves the document byte-for-byte unchanged.

## `RegenerationFailure`

`PartModel::inspect_regeneration` (MCAD-P6-006) regenerates a copy of the
model and returns a `RegenerationInspection`: the copy, holding every output
that was produced, and either the `RegenReport` or a `RegenerationFailure`.
The inspected model is never changed, so it is safe on a document whose
regeneration fails.

```json
{
  "stage": "feature",
  "node": "feature:shaft_bore",
  "error": "feature 'feature:shaft_bore': OCCT error: Invalid edge: circle: invalid params (...)",
  "completed_features": ["feature:sketch_joint_base", "feature:joint_base", "...", "feature:upper_hub"],
  "blocked_features": ["feature:counterbore", "feature:pcd_fasteners", "feature:radial_ribs", "feature:mounting_ears", "feature:mounting_holes"],
  "not_reached_features": [],
  "skipped_suppressed": [],
  "upstream_body_feature": "feature:upper_hub"
}
```

- `stage` is `parameters` (evaluating the parameter graph), `sketch` (solving
  one sketch), `feature_graph` (ordering features, for example a cycle), or
  `feature` (executing one feature). `node` names the failing sketch or
  feature and is absent for the two whole-document stages.
- Every unsuppressed feature other than `node` lands in exactly one list, in
  recompute order. `completed_features` produced an output, which stays in
  the returned copy. `blocked_features` depend on the failing node: the
  feature's downstream, the sketch features using a failing sketch and their
  downstream, or every feature for a parameter or graph failure.
  `not_reached_features` do not depend on the failure but come after it.
- `upstream_body_feature` is the last completed feature upstream of a failing
  feature that has a body: the solid the failing feature was building on.
