# Assembly API

The `opencad-assembly` crate exposes kernel-neutral assembly models,
regeneration reports, and interference queries. Assembly documents remain the
source of truth; `AssemblyScene` and its bodies are disposable results.

## Child document contract

`Component` records a relative `source_path`, expected `source_doc:
DocumentId`, and `source_kind` (`part` or `assembly`).
`Component::validate_source_path` rejects empty, absolute, drive-prefixed, and
parent-traversing paths. `validate_component_path` additionally canonicalizes
an existing path and requires it to stay below the canonical assembly root,
including through symbolic links.

`ResolvedChild::Part` carries `ChildPart.doc_id`; both part and nested-assembly
loads must match `Component.source_doc`. Drawing documents are not valid
assembly children. Regeneration detects recursion by both document ID and
canonical document path. A child failure becomes
`InstanceRegenStatus::Failed` and does not mutate `AssemblyModel`; another
regeneration may retry it.

## Interference tolerance

`AssemblyInterferenceTolerance` makes both units explicit:

- `bounds_tolerance_m`: broad-phase contact tolerance in meters; default
  `1e-9 m`.
- `volume_tolerance_m3`: exact common-volume threshold in cubic meters;
  default `1e-12 m³`.

Both values must be finite and strictly positive. Use
`detect_interferences_with_tolerance` to set both. The compatibility helper
`detect_interferences` accepts a volume tolerance and uses the default bounds
tolerance. Common volume is computed exactly through the kernel-neutral
`GeometryKernel::intersection_volume` primitive, which treats an empty or
disjoint intersection as zero volume instead of failing; the OCCT backend sums
the exact volumes of every resulting piece. Common volume must be strictly
greater than the threshold; output pairs are sorted by `InstanceId` regardless
of scene input order.

## Kinematic tree and URDF export

`opencad_assembly::kinematic_tree(&AssemblyModel)` derives a robot kinematic
tree from the Design Graph ([ADR-028](../adr/ADR-028-urdf-export.md)):

- the single grounded instance (`ground` mate) is the root link; zero or
  several grounded instances are an error;
- mated instance pairs are tree edges, visited breadth-first in mate-ID
  order; a pair with a `concentric` mate is a continuous joint about that
  mate's axis on the child (connector or local frame), any other mated pair
  is a fixed joint;
- a pair that reaches an already placed instance is a loop and an error that
  names the mate; instances no mate reaches are fixed to the root and listed
  in `warnings`;
- joint zero positions are the current assembly pose, and each child link
  frame is its part frame translated to the joint axis origin.

`musubicad export <assembly> <name>.urdf` (Agent `opencad.export`, MCP
`export_document`) writes the URDF and one binary STL per component next to
it (`<robot>_<component>.stl`, metres, referenced by file name). Inertials use
the kernel's mass, centre of mass, and inertia tensor about the centre of mass
(`GeometryKernel::inertia_about_com`) at the default density of
2700 kg/m³. Output is deterministic: numbers carry 12 significant digits and
kernel noise below 1e-12 m (or 1e-9 of the inertia trace) is written as 0.

## Related

- [Assembly architecture](../architecture/assembly.md)
- [Cross-artifact golden evidence](golden-evidence.md)
- [ADR-003](../adr/ADR-003-assembly-document-model.md)
