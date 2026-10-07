# Assembly modeling

Static assembly support (historical implementation Phase 3, M3.1–M3.3) lives in `opencad-assembly` and integrates
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
└─ mates       — solver constraints (implemented in M3.2)
```

## Placement

`Placement.transform` is a `RigidTransform`:

- `translation_m: [f64; 3]` — meters
- `rotation: [[f64; 3]; 3]` — orthonormal 3×3 matrix (row basis vectors)

Applied through `GeometryKernel::transform_body`.

## Regeneration

1. Validate IDs, source kinds, relative path syntax, and direct self-reference.
2. Canonicalize each existing child path and require it to remain inside the
   canonical assembly directory. Distinct components may not alias one
   canonical child document; multiple instances may reuse one component.
3. Load only part or assembly documents and verify the loaded `DocumentId`
   against `Component.source_doc`.
4. Detect nested cycles using both the active `DocumentId` and canonical path.
5. Regenerate the child through its appropriate pipeline and apply each
   instance `placement.transform` via `transform_body`.
6. Aggregate successful instance bodies into a compound scene (mesh merge for export).

Failed child loading or regeneration is reported per instance; sibling results
remain usable and the assembly document is not modified. A later regeneration
re-resolves the child, so transient failures are recoverable.

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
- `detect_interferences_with_tolerance` validates an
  `AssemblyInterferenceTolerance` containing a positive linear bounds
  tolerance in meters and common-volume threshold in cubic meters. Defaults
  are `1e-9 m` and `1e-12 m³`. It then measures candidate pairs with exact OCCT
  Boolean Intersect. Contact within tolerance is not reported, and results are
  ordered by `InstanceId` rather than scene input order.

## Joints (ADR-030)

`AssemblyModel.joints` refines mates for robot descriptions. Each
`AssemblyJoint` names one mate and a motion: `revolute` or `prismatic` with
SI limits, effort, and velocity, `continuous`, or `fixed`. Joints do not move
parts; mates and placements still do. They feed the kinematic tree that URDF
export uses. Validation: unique IDs, a known mate, at most one joint per mate,
moving joints only on concentric mates, finite `lower <= upper`, and
positive effort and velocity. Removing a mate a joint uses fails and names
the joint. An empty list is not serialized, so existing documents and their
revision digests are unchanged.

## CLI

```bash
musubicad new assembly.ocad.d assembly
musubicad regen assembly.ocad.d      # reports instances: N
musubicad export assembly.ocad.d out.stl
```

See `examples/assembly_two_brackets.ocad.d`.

## Cross-artifact evidence

The OCCT assembly result is fixed together with the source part, semantic
references, drawing SVG, and CLI review artifacts by the MCAD-P5-005
[deterministic evidence contract](golden-evidence.md). The manifest records
instance counts, SI-unit mass/volume, and bounding-box tolerances; it does not
replace the assembly Design Graph or persisted document.

## Related

- [ADR-003](../adr/ADR-003-assembly-document-model.md)
- [Assembly API](../api/assembly.md)
- [Geometry kernel](geometry-kernel.md)
- [Feature modeling](feature-modeling.md)
