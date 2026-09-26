# ADR-018: Deterministic kernel topology IDs

Status: Accepted  
Date: 2026-09-26  
Roadmap: MCAD-P7-010

## Context

Kernel face and edge IDs connect regeneration to semantic references. They
come from tessellation triangle face IDs, face discoveries, fillet/chamfer
selectors, face derivation history, and `assign_face_ref`. `opencad regen
--sync-topo-refs` persists them in `graph/semantic_refs.json` as
`kernel_face_id` and `kernel_edge_id`, and it embeds them in generated IDs
such as `ref:face:kernel_<id>`.

The OCCT backend defines these IDs as cadrum's `Face::id()` and `Edge::id()`,
which is the address of the underlying `TShape`. Measurements on the current
backend show three defects.

1. **The IDs are not deterministic.** Running `opencad regen
   --sync-topo-refs` twice on copies of `examples/bracket.ocad.d` wrote
   different `kernel_face_id` values (for example `2268474746992` vs
   `1724170044832`), different generated reference IDs, and a different
   reference order. This violates the invariant that `.ocad` files are
   deterministic.
2. **The stored solid shares topology.** A prism's top and bottom faces share
   one `TShape` and differ only by location. A stored 60 × 40 × 6 mm box has
   6 faces but only 5 distinct face IDs, and 12 edges but only 8 distinct
   edge IDs. An ID cannot name "the top face".
3. **Every read uses a different copy.** The backend clones the stored solid
   before tessellating, discovering edges, or filleting. cadrum's
   `Solid::clone` is a deep copy (`BRepBuilderAPI_Copy`) with new `TShape`s.
   So IDs from tessellation never name faces of the stored solid, nor of the
   copy a later fillet uses. Face-reference fillets and chamfers therefore
   select no edges on any face except the top. The top face works only
   because it falls back to the top-perimeter selector.

The same measurement shows what a deep copy preserves: it has the same face
and edge enumeration order as the original (6 of 6 and 12 of 12 distinct IDs,
in order).

## Decision

### 1. IDs are enumeration indices

A kernel face ID is the 1-based position of the face in the body's canonical
face enumeration (`Solid::iter_face`, OCCT's `TopExp` map order). Edge IDs
use the edge enumeration the same way. A compound body numbers its members
in member order, with running offsets.

- IDs are local to a body.
- IDs are deterministic for identical inputs and OCCT version.
- IDs survive deep copies, because a copy keeps the enumeration order.
- `0` stays "no ID".

### 2. Every conversion goes through the enumeration

- **Tessellation.** Tessellation still meshes a deep copy. It maps each copy
  face's `TShape` address to its enumeration index. A deep copy does not
  share topology, so this map is one-to-one. If it ever is not, tessellation
  fails instead of guessing.
- **Selectors.** Edge discovery, `FacePerimeter`, and `KernelEdges` enumerate
  the solid they run on. That solid is always a deep copy with the same
  order, so an index selects the same face or edge as the stored body.
- **Face history.** A modifying operation records its face history as index
  pairs `(result face, input face)` when it runs, and the store keeps the
  pairs with the result body. The input is the deep copy the operation
  consumed, so its addresses map one-to-one to indices.
  - Result solids may share topology, so a pair is emitted for every result
    face that carries the recorded address.
  - Booleans keep only the pairs whose source is the target body.
  - The pairs describe one operation. Indices are local to a body, so pairs
    from different operations must not be merged into one flat map: the same
    index names different faces in different bodies.

### 3. Stored IDs are matched directly, verified by role

A persisted `kernel_face_id` or `kernel_edge_id` is an index into the final
body at the time it was synced. Resolution and reference provenance match it
directly against the discoveries of the current body. They no longer remap
it through the regeneration's face history. The flat history composition
(`build_src_to_post_map` over every operation) chained addresses that were
globally unique. With body-local indices it would conflate faces of
different bodies, and a test showed it relabelling a reference to another
feature's face.

An index names a different face once topology changes; for example, cutting
a hole inserts faces into the enumeration. A pointer ID that went stale only
failed to match, but a stale index can match the wrong face. So a stored ID
is trusted only when the discovered face or edge it names has the
reference's role. This check applies in:

- face resolution;
- face and edge provenance;
- edge-reference fillet selection;
- `sync_semantic_refs`, which previously overwrote a reference's creating
  feature and role with whatever face now had its ID.

Otherwise resolution falls back to fingerprint matching, exactly as for a
missing ID.

As a consequence, provenance no longer reports `Derived`. The variant is
kept for compatibility. `rebind_kernel_face_ids` and the history argument of
`sync_semantic_refs_with_history` are kept as no-ops.

### 4. No schema change

The fields stay `u64`. Pointer values already stored in documents lie far
outside any index range, so they resolve through the existing
stale-ID fallback. Documents stay readable, and a later `--sync-topo-refs`
rewrites them deterministically.

## Consequences

- Kernel IDs, generated `ref:face:kernel_<n>` IDs, and reference order become
  deterministic, so `--sync-topo-refs` is reproducible.
- Face-reference fillets and chamfers select the referenced face's edges on
  every face.
  - Earlier, with geometric picking on address IDs, side-face fillets
    succeeded in only 3–8 of 10 identical runs.
  - With index IDs, the +x, -y, and bottom faces succeeded 10 of 10 times
    each.
  - The removed volumes match the analytic difference
    `(1 − π/4)·r²·40 mm` for fillets and `r²/2·40 mm` for chamfers.

  The earlier failures came from address-keyed discoveries selecting
  different faces between runs, not from OCCT.
- Every modifying operation site records index history for observers such as
  regeneration reports. Stored IDs no longer depend on it, so an operation
  that records none loses nothing in resolution.
- Rebinding a reference across a topology change relies on role checks and
  fingerprints instead of history. A face whose role and fingerprint both
  change is reported missing rather than silently followed.

## Evidence plan

- **Determinism.** Two `--sync-topo-refs` runs on bracket copies write
  byte-identical `semantic_refs.json`.
- **Distinct faces.** Tessellating a prism reports six distinct face IDs,
  `1..=6`.
- **Selectors.** A face-reference fillet selects exactly the four edges of
  the referenced side face.
- **History.** Recorded history pairs are valid indices, and history no
  longer remaps stored IDs.
- **Stale IDs.** A stored ID whose face no longer has the reference's role
  falls back to fingerprint matching.
