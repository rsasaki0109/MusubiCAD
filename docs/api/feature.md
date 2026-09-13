# Feature API

`opencad-feature` owns serializable feature definitions and deterministic
regeneration through the kernel-neutral `GeometryKernel` interface.

## Representative multi-feature part

`robot_joint_actuator_housing()` constructs the checked-in
`examples/robot_joint_actuator.ocad.d` model. Its 22 nodes expose nine visible
body milestones:

1. base plate;
2. lower and upper stepped hubs;
3. output-shaft bore and bearing counterbore;
4. eight-hole circular fastener pattern;
5. six-instance circular rib union;
6. mirrored mounting-ear union and mirrored mounting-hole cut.

Use `opencad_graph::robot_joint_housing_parameters()` for its 19 explicit-unit
parameters. Call `PartModel::regenerate()` with a `FeatureRegistry` and a
`GeometryKernel`; generated bodies remain disposable outputs of the Design
Graph.

```rust
let mut model = opencad_feature::robot_joint_actuator_housing()?;
let parameters = opencad_graph::robot_joint_housing_parameters();
let registry = opencad_feature::FeatureRegistry::with_defaults();
let report = model.regenerate(&kernel, &registry, Some(&parameters), None)?;
println!("{}", report.trace.trace_hash_sha256);
```

The constructor performs no file-system or network I/O. The desktop template
layer owns `.ocad` persistence through the `robot-joint` template.
`RegenReport.trace` records deterministic execution evidence; see
[Change impact and regeneration trace](change-impact-and-regeneration-trace.md).

## Robot-arm assembly parts

`robot_arm_base()`, `robot_arm_upper_arm()`, `robot_arm_forearm()`, and
`robot_arm_gripper()` build the four parametric parts behind
`examples/robot_arm_assembly.ocad.d`. Each part constructor applies its own
explicit-unit parameter graph (`robot_arm_base_parameters()` and friends) and
performs no I/O. The assembly model itself is built by
`opencad_assembly::robot_arm_assembly_model()`, which places the four parts,
declares six named connectors, and adds three concentric revolute joints at
the shoulder, elbow, and wrist. The authored -45° elbow/wrist pose satisfies
every mate with zero initial residual, and links are stacked along `+Z` so the
joint hubs touch with zero interference; regeneration is deterministic and
leaves the pose unchanged.

```rust
let mut model = opencad_feature::robot_arm_upper_arm()?;
let parameters = opencad_graph::robot_arm_upper_arm_parameters();
let report = model.regenerate(&kernel, &registry, Some(&parameters), None)?;
assert_eq!(report.regenerated.len(), 10);
```

## Incremental content-addressed regeneration

`PartModel::regenerate` performs a cold full regeneration. For repeated edits,
`PartModel::regenerate_with_cache` reuses unchanged outputs through an
in-memory, disposable `RegenerationCache` (MCAD-P6-002):

```rust
let kernel = OcctGeometryKernel::new();
let registry = opencad_feature::FeatureRegistry::with_defaults();
let mut model = opencad_feature::robot_joint_actuator_housing()?;
let mut cache = opencad_feature::RegenerationCache::with_backend("occt");
let mut parameters = opencad_graph::robot_joint_housing_parameters();

model.regenerate_with_cache(&kernel, &registry, Some(&parameters), None, &mut cache)?;
parameters.set_expr("param:upper_hub_height", "42 mm")?;
let report = model.regenerate_with_cache(&kernel, &registry, Some(&parameters), None, &mut cache)?;
assert!(report.cached_nodes.iter().any(|id| id == "feature:joint_base"));
```

- Each feature output has a versioned content key over its definition, solved
  source sketch, upstream output identity, and the kernel backend tag; a cached
  output is reused only when its whole derivation is unchanged.
- Cached nodes are reported in `RegenReport.cached_nodes` and make zero
  geometry-kernel calls; `RegenerationTrace.output_hashes_sha256` is identical
  to a cold run, so mass, bounds, and content hashes match.
- The cache and kernel must be reused together across calls; cache data is
  disposable and never written into `.ocad`.
- A failed incremental regeneration restores the previous document outputs.
- Checked-in 22/100/250-node chain benchmarks
  (`modules/feature/tests/incremental_benchmarks.rs`) gate deterministic call
  counts and cold/incremental equivalence.

## Feature-build animation

`opencad animate-features` regenerates a part, omits standalone pattern-tool
bodies, and renders the remaining body-producing milestones in deterministic
Feature Graph order. Every frame uses a camera fitted to the final body so
geometry growth is directly comparable:

```bash
opencad animate-features examples/robot_joint_actuator.ocad.d build.gif \
  --frames 54 --fps 9 --orbit-deg 35 --pitch-deg 30
```
