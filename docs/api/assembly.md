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

## Related

- [Assembly architecture](../architecture/assembly.md)
- [Cross-artifact golden evidence](golden-evidence.md)
- [ADR-003](../adr/ADR-003-assembly-document-model.md)
