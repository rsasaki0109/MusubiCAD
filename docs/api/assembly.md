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
  order; a joint declared over one of the pair's mates
  ([ADR-030](../adr/ADR-030-assembly-joints.md)) sets the type and limits
  (`revolute`, `continuous`, `prismatic`, `fixed`); without one, a pair
  with a `concentric` mate is a continuous joint about that mate's axis on
  the child (connector or local frame), and any other mated pair is a fixed
  joint;
- a pair that reaches an already placed instance is a loop and an error that
  names the mate; instances no mate reaches are fixed to the root and listed
  in `warnings`;
- joint zero positions are the current assembly pose, and each child link
  frame is its part frame translated to the joint axis origin.

`KinematicTree::pose(&positions)` is the forward kinematics over that tree.
`positions` maps a moving instance ID to its joint position (radians for
revolute and continuous joints, metres for prismatic ones); unnamed joints stay
at zero, which is the current assembly pose. It returns every instance's part
frame in world coordinates, in link order: each link is its parent's world
frame composed with the joint origin and the joint motion
(`JointKind::motion`: a rotation about the joint axis, or a translation along
it). Positions past a declared limit (1e-9 tolerance) are rejected with the
limits in degrees or millimetres, never clamped, and a non-zero position on a
fixed joint is an error.

`musubicad animate-joints <assembly> <out.gif>` uses it to draw the assembly
moving through its joints with the CPU preview renderer, so it needs no GPU.
Each `--pose shoulder=60deg,elbow=-45deg` adds a keyframe; positions carry a
unit (`deg`, `rad`, `mm`, `m`) and a joint is named by its joint ID, its short
name, its mate, or the instance it moves. The clip runs from the zero pose
through every keyframe and back with eased motion (`--frames-per-move`, default
14; `--fps`, default 15; at most 600 frames). Without `--pose`, every movable
joint swings to 60 % of its upper limits, then 60 % of its lower ones
(continuous joints ±90°). Parts are regenerated once; every keyframe is checked
against the limits before that. The command prints the frames written and the
range each joint covered, in degrees or millimetres. Frames are captioned with
every moving joint's position (`--no-caption` turns that off).

### Reach and workspace

`opencad_assembly::workspace(&tree, &tool)` and
`solve_reach(&tree, &tool, target_m, tolerance_m)` answer where a `ToolPoint`
(an instance and a point in its part frame, metres) can go, without the
geometry kernel. Joint space is sampled on a deterministic grid inside the
declared limits (continuous joints over −π to π, at most 20 000 samples); the
six best samples are refined by damped least squares on a finite-difference
Jacobian with a backtracking step, clamped to the limits. `Workspace` reports
the sampled tool bounds and the closest and farthest tool distance from the
first moving joint. `ReachSolution` gives the joint positions, the tool point,
the remaining distance, and `reachable` when that distance is within the
tolerance. Reachable targets converge to about a micrometre; for targets out
of reach the gap is accurate to about 0.1 mm, because the stretched pose is
singular.

`musubicad reach <assembly> --tool gripper --point 0,40,7mm --target 0,330,63mm`
prints the workspace (millimetres), whether the target is reachable, the gap,
each joint's value in degrees or millimetres, and the pose as an
`animate-joints --pose` argument. `--tolerance` defaults to 0.5 mm. Mates are
solved first, as assembly regeneration does, so the answer follows connector
and link-length edits; `animate-joints` uses the same solved placements.
`--gif out.gif [animate-joints options]` also draws the arm moving to that
pose with the target marked and the result in the caption.

[`examples/agent/reach_upper_arm_length_patch.json`](../../examples/agent/reach_upper_arm_length_patch.json)
and [`reach_elbow_connector_patch.json`](../../examples/agent/reach_elbow_connector_patch.json)
are the design loop the README shows: the arm falls 20 mm short of
`(0, 330, 63) mm`; the first patch lengthens the upper arm from 160 mm to 200 mm
and requires its mass to rise by 30–50 g; the second moves the elbow connector
to the new link end and carries the forearm and gripper 40 mm with it (the mate
solver needs a nearby start for a move that large); after both, the target is
reached. `docs/assets/generate-reach-demo.sh` replays it on a temporary copy.

### Web joint viewer

`musubicad export <assembly> <name>.html` (Agent `opencad.export`, MCP
`export_document`) writes one self-contained page: the regenerated part
meshes, a small inline WebGL renderer, orbit and zoom (mouse, touch, pinch),
and one slider per movable joint, bounded by the declared limits (continuous
joints by one turn), plus Animate and Reset. The sliders run the same chain as
`KinematicTree::pose` (after solving mates): each link is its parent's frame
composed with the joint origin and motion; a check against the Rust pose
agrees to 1e-10 m. Meshes are stored as base64 little-endian `f32` positions
and `u32` indices and shaded with flat face normals and the preview palette;
the page loads nothing from the network, follows the system light or dark
theme, and is byte-identical for the same document.
`docs/assets/robot-arm-viewer.html` and `docs/assets/six-axis-viewer.html`
are the two example arms, and
`docs/assets/generate-viewer-demo.sh` regenerates it with a screenshot.

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
