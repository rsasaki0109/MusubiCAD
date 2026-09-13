# Semantic TopoRef API

`opencad-geometry` exposes semantic topology references without exposing OCCT
handles. A `TopoRef` keeps a stable `ref_id`, a semantic producer/role, and
optional regeneration hints. The identity is derived without adding fields to
the existing serialized TopoRef shape:

```rust
use opencad_geometry::TopoRef;

let identity = topo_ref.identity();
assert_eq!(identity.ref_id, topo_ref.ref_id);
```

`TopoRefIdentity` contains the stable `ref_id` key plus `kind`, `created_by`,
`role`, and `intent` metadata for semantic meaning. `kernel_face_id`,
`kernel_edge_id`, normal hints, and geometric fingerprints are deliberately
excluded because kernel regeneration may change them.

## Resolution and fallback

The existing resolution functions retain the default policy for compatibility.
Call the explicit-policy variants when a workflow has a documented tolerance
budget:

```rust
use opencad_geometry::{
    match_face_discovery_for_topo_ref_with_policy, TopoRefTolerancePolicy,
};

let policy = TopoRefTolerancePolicy {
    face_centroid_tolerance_m: 0.001,
    edge_midpoint_tolerance_m: 0.002,
    normal_alignment_min_dot: 0.99,
    tangent_alignment_min_dot: 0.99,
    vector_norm_epsilon: 1e-9,
};
let kernel_face_id = match_face_discovery_for_topo_ref_with_policy(
    &topo_ref,
    &discoveries,
    policy,
);
```

The policy fields are explicit: centroid and midpoint distances are meters and
strictly positive; normal/tangent thresholds are dimensionless absolute dot
products in `[0, 1]`; the vector epsilon is dimensionless and strictly
positive. Invalid policies are rejected by resolution and
produce no fallback match from the direct matcher. When candidates have the
same score, the smallest kernel ID wins, so the result does not depend on
tessellation discovery order.

When current discoveries are supplied, a stored or history-remapped kernel ID
is accepted only if it is present in those discoveries. This matters across
separate regeneration runs, where kernel-local IDs can all change and the
current run's derivation history cannot bridge the prior process-local value.
An absent stored ID therefore continues through semantic/fingerprint fallback;
without discoveries, the legacy stored-ID behavior is retained.

## Reference provenance (fail-closed, MCAD-P6-003)

Every reference resolution can be classified and reported through
`opencad_geometry::ReferenceProvenance`:

```rust
use opencad_geometry::{
    resolve_face_ref_with_provenance, ReferenceStatus, TopoRefTolerancePolicy,
};

let resolution = resolve_face_ref_with_provenance(
    semantic_refs,
    face_history,
    "ref:face:bracket_top",
    Some(&discoveries),
    TopoRefTolerancePolicy::default(),
    /* required = */ true,
)?;
match resolution.provenance.status {
    ReferenceStatus::Exact => { /* stored kernel id is present */ }
    ReferenceStatus::Derived => { /* remapped through derivation history */ }
    ReferenceStatus::Fingerprint => { /* role/geometric fallback */ }
    ReferenceStatus::Ambiguous => { /* equal best scores; never picked */ }
    ReferenceStatus::Missing => { /* no candidate satisfied the reference */ }
}
```

- `exact` accepts the stored (or history-remapped) kernel id present in the
  regenerated body; `derived` distinguishes the remapped case.
- `fingerprint` is a role/geometric fallback pick.
- When two or more distinct candidates tie for the best score, the resolution
  is `ambiguous` and reports no chosen kernel id. Ties are detected from the
  scored candidate set, so the outcome is independent of discovery order.
- `missing` means no candidate satisfied the reference.
- Setting `required = true` makes `ambiguous` and `missing` return an error,
  so a required reference blocks the commit instead of silently choosing.
- Each provenance records the source feature, intended role, candidate set
  with scores, the tolerance policy, and a human-readable reason.

`resolve_all_reference_provenance` classifies every document reference against
the final regenerated discoveries as observability data; `RegenReport` carries
the per-reference provenance, the CLI `opencad regen` prints a status summary,
and the design-review artifact includes the same provenance table for part
documents.

For the legacy serialized fingerprint, `area_range` is measured in square
meters. Face `bbox_hint` values are centroid bounds in meters, while edge
`bbox_hint` stores `[midpoint_m, unit_tangent]`. P5-001 uses face centroids and
edge midpoint/tangent hints; `surface_type` and `area_range` are retained for
compatibility but are not scoring inputs yet.

## Migration guidance

No `.ocad` schema migration is required for P5-001. Existing readers continue
to deserialize the same `TopoRef` fields. For a legacy document or expanded
directory:

1. Keep each existing `ref_id`, `kind`, `semantic.created_by`, `role`, and
   `intent` unchanged.
2. Do not promote a kernel face/edge ID into identity; it is a cache hint.
3. Pass derivation history to the resolver first. If the stored ID is stale or
   absent, provide current discoveries and an explicit tolerance policy.
4. Persist refreshed fingerprint/kernel hints only through the existing sync
   path. Do not rewrite every legacy file merely to record the runtime policy.

The checked-in example [`examples/topo-ref-semantic.json`](../../examples/topo-ref-semantic.json)
shows a compatible persisted reference with both semantic identity fields and
optional fallback hints.
