# ADR-030: Assembly joints with limits

Status: Accepted  
Date: 2026-10-06  
Roadmap: MCAD-P8-009

## Context

URDF export (ADR-028) derived every joint from mates. A concentric mate
became an unlimited `continuous` joint, because a mate says how parts sit
together, not how far they may move. Real robots have joint limits, actuator
effort, and speed limits. Simulators and planners need them, and they belong
to the design, not to a hand-edited copy of the URDF.

## Decision

1. **Model.** `AssemblyModel.joints: Vec<AssemblyJoint>`. Each joint has an
   `id` (`joint:` prefix, new `JointId` in `modules/core`), the `mate` it
   refines, and a flattened `type`:
   - `revolute` with `lower_rad`, `upper_rad`, `effort_n_m`,
     `velocity_rad_s`;
   - `continuous`;
   - `prismatic` with `lower_m`, `upper_m`, `effort_n`, `velocity_m_s`;
   - `fixed`.

   Limits are SI and measured from the current pose, which is the joint's
   zero. Effort and velocity are required for limited joints because URDF
   requires them; there is no silent default.
2. **Joints do not move parts.** Mates and placements still position
   instances. A joint adds motion semantics for robot descriptions only.
3. **Validation.**
   - IDs are unique, the mate exists, and each mate has at most one joint.
   - Moving joints (`revolute`, `continuous`, `prismatic`) require a
     concentric mate, whose axis they use. Joints never use ground mates.
   - Limits are finite with `lower <= upper`, and effort and velocity are
     positive.
4. **Patches.** `add_joint` and `remove_joint` are structural assembly
   operations, and `set_joint` replaces a joint by ID. Removing a mate that a
   joint uses fails closed and names the joint (ADR-013). `authoring_patch`
   emits `add_joint` after `add_mate`. Semantic diff reports
   `assembly_joint_added`, `assembly_joint_removed`, and
   `assembly_joint_changed`. Merge and rebase treat joint IDs as assembly
   targets.
5. **Kinematics.** `kinematic_tree` uses a declared joint over a pair's mate
   in preference to the ADR-028 inference. URDF export writes `revolute` or
   `prismatic` with `<axis>` and
   `<limit lower upper effort velocity>`, plus `continuous` or `fixed`.
6. **Schema compatibility.** An empty `joints` list is not serialized. Every
   existing document, its checksums, and its design-state revision digest
   stay byte-identical, so there is no format version bump and no migration
   step. `schemas/ocad.assembly.schema.json` and
   `schemas/ocad.patch.schema.json` describe the new field and operations.

## Verification

- Unit tests cover serialization, valid and invalid joints (reversed limits,
  zero effort, ground or unknown mate, a second joint on one mate), and the
  kinematic tree with declared revolute and fixed joints and with none.
- Integration tests cover `set_joint` with diff and persistence, removal of
  a guarded mate, `remove_joint` followed by `remove_mate`, and the
  byte-for-byte rebuild of the robot arm from `authoring_patch`.
- The robot arm example gained its joints through
  `musubicad patch … add_joint ×3`, which verified the change before
  writing. Only `graph/assemblies.json` and the checksums changed.
- MuJoCo 3.15 imports the exported URDF as three limited hinges: shoulder
  ±150°, elbow −100° to 140°, wrist ±90°. Driving the elbow at 20 rad/s
  stops at 143°, within MuJoCo's soft-limit compliance.

## Consequences and follow-ups

- Connector frames are still fixed numbers. Lengthening a part does not
  move joint origins. Driving connectors from part parameters needs a
  cross-document reference model, which is a later ADR.
- No joint-angle parameter yet: posing the arm is still a
  `set_instance_placement`.
- MCP has no dedicated joint tool. Agents use the patch operations above.
