# Assembly modeling

Static assembly support (Phase 3, M3.1) lives in `opencad-assembly` and integrates
with the existing `.ocad` document pipeline.

## Document model

Assembly documents set `DocumentMetadata.kind = assembly` and populate
`OcadDocument.assembly`. Part fields (`sketches`, `feature_nodes`) remain empty.

On disk (expanded `.ocad.d` format), the model is stored in
`graph/assemblies.json`. Part-only documents omit the `assembly` field; legacy
`{ "assemblies": [] }` files deserialize as `None`.

```
AssemblyModel
├─ components  — child part references (relative path + DocumentId)
├─ instances   — placed copies with RigidTransform
└─ mates       — reserved for M3.2 (empty in MVP)
```

## Placement

`Placement.transform` is a `RigidTransform`:

- `translation_m: [f64; 3]` — meters
- `rotation: [[f64; 3]; 3]` — orthonormal 3×3 matrix (row basis vectors)

Applied through `GeometryKernel::transform_body`.

## Regeneration

1. Validate references (no direct self-reference via `Component.source_doc`).
2. Resolve each `Component.source_path` relative to the assembly directory.
3. Regenerate the child part through `PartModel::regenerate`.
4. Apply each instance `placement.transform` via `transform_body`.
5. Aggregate successful instance bodies into a compound scene (mesh merge for export).

Failed child regeneration is reported per instance; the assembly document is not modified.

## Mate solving (M3.2)

Mates reference `(InstanceId, TopoRef)` attachment entities with optional local frames,
or named `connector` anchors on instances. Each movable instance carries 6 DOF
(translation + rotation vector). `Ground` mates and `Instance.fixed` remove DOF before solving.

Supported mate kinds: `coincident`, `concentric`, `distance`, `angle`, `parallel`, `ground`.

Regeneration runs `solve_assembly_mates` when `mates` is non-empty, then places instances.

## Connectors and patterns (M3.3)

- `connectors` — named `RigidTransform` frames on instances for reusable mate anchors.
- `patterns` — linear instance expansion along `direction_m` with `spacing_m`.
- `Component.source_kind` — `part` (default) or `assembly` for nested sub-assemblies.
- Agent API: `list_assembly_instances`, `list_assembly_mates`, `list_connectors` queries;
  `set_instance_placement`, `set_mate_distance`, `add_connector` patch operations.
- Desktop preview tessellates each instance separately with distinct colors.

## CLI

```bash
opencad new assembly.ocad.d assembly
opencad regen assembly.ocad.d      # reports instances: N
opencad export assembly.ocad.d out.stl
```

See `examples/assembly_two_brackets.ocad.d`.

## Related

- [ADR-003](../adr/ADR-003-assembly-document-model.md)
- [Geometry kernel](geometry-kernel.md)
- [Feature modeling](feature-modeling.md)
