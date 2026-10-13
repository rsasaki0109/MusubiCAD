# ADR-056: Semantic coupled joint displacements

Status: Accepted

Task: MCAD-P13-007

## Purpose and source contract

Couple declared assembly joints using stable semantic IDs. A gear relationship
uses `follower_rad = ratio * driver_rad`; ratio is finite signed dimensionless
and nonzero. A rack relationship uses `follower_m = metres_per_radian * driver_rad`;
coefficient is finite signed SI metres per radian and nonzero. These are changes
from authored zero, not absolute angles or geometric tooth-contact inference.
There is no automatically inferred phase offset. The zero displacement preserves
the authored mechanical assembly.

Persist a separate sorted `AssemblyModel.couplings` collection. Each record owns
an explicit `coupling:` ID, driver/follower `JointId`s and a tagged relationship.
Use DesignPatch add/set/remove operations through normal transactions, dry runs,
verification and history. Never write derived joint positions into instances or
mates. Schemas require the explicit relationship/unit fields. Absent collections
migrate to empty, serialize absent and preserve old meanings.

Example proposed source forms:

```json
{"id":"coupling:gears","driver_joint":"joint:drive","follower_joint":"joint:driven","type":"gear","ratio":-2.0}
```

```json
{"id":"coupling:rack","driver_joint":"joint:pinion","follower_joint":"joint:rack","type":"rack","metres_per_radian":0.02}
```

## Admission and evaluation

At most 256 relationships. IDs and follower owners are unique. Both references
resolve to declared unsuppressed active joints and their actual moving link in
the kinematic tree. Gear joints must both rotate; rack driver must rotate and
follower translate. Fixed/ground, duplicate, unknown, suppressed, incorrectly
typed or cyclic dependencies refuse. Preserve exact named semantics; do not
select another mate/instance heuristically.

Resolve an acyclic dependency graph in deterministic ID order. Chains multiply
signed coefficients with finite-result checks. Explicit driver positions carry
the existing radian/metre units. Derive all followers; a supplied follower must
agree within the appropriate explicit tolerance, otherwise refuse without
clamping or silently overriding. Unknown joint inputs remain failures.

Finite driver ranges must map entirely within follower ranges. For the bounded
initial scope, require zero within all participating finite ranges; an unlimited
angular driver cannot feed a limited follower. Reject range-incompatible sources
rather than guessing a smaller usable range. Continuous/continuous gear pairs are admitted as unwrapped displacements;
continuous drivers cannot feed finite followers. No angle wrapping is inferred.

Use the same resolution in `KinematicTree::pose`, tool-point evaluation,
workspace/reach and animation. Enumerate only independent driver coordinates
for optimization, keyframes and sampling; expose derived coordinates separately
for truthful diagnostics/captions. Do not grant followers independent sliders.

## Interference and sampling

Keep the current per-independent-driver evenly spaced sample grid. Derive all
coupled followers for each pose. This is not a continuous proof or a sweep of all
combinations of independent drivers.

The current interference optimization reuses pairs within one rigid moving
subtree. Coupled subtrees may move relative to one another, so that optimization
is invalid for them. Use a complete posed-body pair evaluation for coupled
assemblies, with existing explicit broad-phase and exact-solid volume tolerances.
Keep the unchanged legacy path for uncoupled assemblies. Add a regression where
coupled siblings interfere only after motion; checking just the union boundary
must fail the regression. No new stochastic/adaptive sampling.

## Exports and frontend consumers

URDF must emit faithful mimic relations with actual exported joint names and SI
multipliers, including signed ratios. Verify target format semantics for rack
cross-unit mimic before accepting it; otherwise explicitly refuse that export.
Never silently export independent joints from a coupled source.

The self-contained web viewer must either derive follower coordinates using the
same admitted relation data and offer only independent controls or refuse coupled
sources before writing output. Do not mutate mechanical poses to fake coupling.
Animation/IK unsupported cases must likewise fail before producing artifacts.
Static STEP/STL/BOM and exploded presentations use the authored zero configuration
unless a separate explicit mechanical pose request is supported.

## Kernel ownership and evidence

No OCCT types escape the kernel adapter. Coupling is graph and neutral pose math;
both Mock and OCCT execute resulting placements and interference through existing
interfaces. Use validated triangle solids for independent analytic placement and
volume probes; generic legacy Mock extrusion/translation is synthetic and cannot
prove true physical motion. At this ADR’s introduction Mock multi-imported-solid compounds were refused.
ADR-060 subsequently adds bounded physical mesh member compounds for P13-011;
exact imported-solid Boolean/intersection and inertia remain unsupported.

## Acceptance evidence

- gear_signed_ratio_and_zero_pose_match_analytic_rotation
- rack_si_displacement_matches_analytic_translation
- coupling_chains_are_order_independent_and_keep_source_placements
- duplicate_follower_cycles_missing_suppressed_and_wrong_types_refuse
- incompatible_ranges_nonfinite_coefficients_and_conflicting_inputs_refuse
- both_kernels_coupled_pose_bounds_and_invariant_volume
- coupled_siblings_collision_is_found_inside_moving_union
- independent_driver_sampling_reports_derived_follower_motion
- coupled_source_old_new_expanded_packed_authoring_history_roundtrip
- coupled_patch_dry_apply_undo_redo_and_failed_removal_are_atomic
- urdf_mimic_names_and_coefficients_match_source_or_export_refuses
- animation_and_web_controls_cannot_drive_followers_independently
- actual_mcp_coupled_pose_query_matches_native_and_independent_grader

Add an example and feasible MCP eval; update guide, skill, API, architecture,
roadmap/status and migration. All four mandatory local checks and all GitHub CI
must pass before the task PR is merged. No maturity credit before main merge.

## Concrete analytic probes and Mock limitation

For a signed gear ratio -2, driver displacement π/2 rad implies follower -π rad.
For a rack coefficient 0.02 m/rad, the same driver displacement implies
0.03141592653589793 m translation. Apply these poses to validated closed boxes;
compare exact neutral transforms and per-body Mock/OCCT bounds, centres and
volume with independent numeric tolerances.

Imported triangle-solid Boolean intersection is also refused by the current
Mock kernel. Do not turn that refusal into a passing physical collision claim.
The true coupled sibling-collision regression must execute native OCCT; Mock
must retain an explicit unsupported refusal at exact intersection, while neutral
pair-enumeration/posed-placement tests prove it visits internal moving pairs.
Any expansion of Mock exact intersection requires a separately justified kernel
scope and analytic tests, rather than a fabricated AABB intersection oracle.

## Read-only pose API

Expose the same backend through CLI `inspect-joint-pose` and Agent/MCP
`inspect_joint_pose`. A request contains assembly `path` and positions keyed by
stable declared joint IDs with explicit unit-bearing strings (`90deg`, `0.5rad`,
`20mm`, `0.02m`). Reuse the existing `JointValue` admission/parser. Reject a length
for an angular joint and an angle for a linear joint; no unit inference. Resolve
only exact IDs, not short-name guesses. Return derived driver/follower positions
with explicit `position_rad` or `position_m`, association IDs and world rigid
transforms (`translation_m`, dimensionless proper rotation). No document writes.
Bind requests to solved mechanical placements before posing, like animation.

The probe with gear ratio -2, driver 90deg and a sufficiently wide follower range
must report -π radians for the follower. Rack coefficient 0.02 m/rad must report
0.03141592653589793 metres. The MCP eval independently inspects saved coupling
records and queries the backend pose; neither source placements nor followers
may be edited to approximate the result. Add this selector to guides/skill and
real local MCP smoke evidence. The public name is `inspect_joint_pose` (CLI `inspect-joint-pose`).

## Compatibility review notes

Existing kinematic joints are keyed by child instance, with source `mate` IDs.
Bind each declared source joint to exactly one derived movable child through its
mate. A joint omitted or constrained to fixed by tree derivation must refuse a
coupling, even if its raw motion tag says revolute. Do not reinterpret input
joint directions based on mesh shape.

Reach/workspace and animation currently enumerate every movable link. Filter
followers out of independent coordinate lists, and resolve them in the shared
pose function. Keep the uncoupled code path/output byte-compatible. The existing
URDF exporter names joints by shortened mate IDs, not source `JointId`s; mimic
references must use that actual naming rule and be checked for uniqueness.
The HTML viewer implements its own forward kinematics in embedded JavaScript;
its existence alone does not provide parity for coupling. Initially refuse
coupled HTML export unless the shared relation semantics are explicitly added
and independently exercised. Refuse before creating any output artifacts.

Initial coefficient admission: absolute dimensionless gear ratio >1e-12,
rack coefficient >1e-9 m/rad; both bounded to 1e6 in their respective units.
URDF gear mimic is admitted; rack URDF and coupled HTML export explicitly refuse
before writing artifacts. No new dependencies or FFI. The bounded dependency DAG
has at most 256 edges; coupled collision cost is quadratic in posed body count
per existing independent-driver sample. Uncoupled execution stays unchanged.

## Forward-reader refusal on the wire

An additive `couplings` object alone is insufficient: pre-feature serde readers
ignore unknown assembly fields and could export independent joints. On the wire,
a follower joint therefore uses the new `type:"coupled"` record with explicit
`coupling_id` and nested physical `motion` (the existing SI motion definition).
Older JointMotion enums reject the unknown tag. The current assembly codec
unwraps it into the unchanged physical AssemblyJoint and checks a one-to-one
match with the owning coupling/follower ID. Missing, orphan, duplicate or mismatched
guards refuse; no phase or relationship is inferred. Uncoupled assembly records
and canonical ordering remain byte-compatible. The wire wrapper is a format
admission marker; the Design Graph still owns one coupling relationship and one
physical joint motion, not two independent editable definitions. Patch AddJoint
uses physical motion; coupling operations own the source association. Verify
legacy deserialization rejects guarded joints and new/old packed/expanded/history
round trips retain exact logical models. No manifest-version fiction: historical
readers did not validate the free-form manifest schema string.

Executed evidence resides in `modules/file/tests/coupled_joints.rs` and
`modules/cli/tests/coupled_joints.rs`: analytic signed gear/chains and rack motion,
Mock/native rigid placement bounds/centres/volume, native coupled sibling
collision plus explicit Mock refusal, legacy decoder/guard refusal, source/
expanded/packed/authoring/history atomicity, real Agent transactions and unit
refusal, URDF mimic, animation and coupled HTML/follower-control refusals.
