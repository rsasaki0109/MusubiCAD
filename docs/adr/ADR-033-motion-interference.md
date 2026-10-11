# ADR-033: Interference across joint motion

Status: Accepted  
Date: 2026-10-10  
Roadmap: MCAD-P10-002

## Context

MCAD-P10-001 made assembly assertions run on every verified patch, but
`interference_at_most` only sees the authored pose. A robot that clears itself
at rest can still fold into itself inside its declared joint limits. The
checked-in six-axis arm shows this: sweeping the elbow through its limits
(−130° to +100°) drives the hand and gripper into the base and turntable
from about +45° to +80°. The shoulder at −80° hits the turntable, and wrist
pitch above +80° drives the gripper into the wrist. No existing check
reported any of it.

An exhaustive check of every combination of joint positions is exponential
in the number of joints: a 6-axis arm at 13 positions per joint is 4.8 million
poses, each needing exact Booleans between instance pairs.

## Decision

1. **A typed assertion.** `motion_interference_at_most { max_count,
   samples_per_joint }` is an `AssertionKind` like the others. It is stored in
   `graph/assertions.json`, evaluated by `musubicad regen` and `verify_patch`,
   and fails closed when the sweep cannot run (no grounded instance, an
   instance that did not regenerate). `samples_per_joint` must be 2–360.
2. **Per-joint sampling.** Each movable joint of the kinematic tree
   (ADR-028/030) sweeps `samples_per_joint` evenly spaced positions. Limited
   joints include both limits, and continuous joints sample one full turn from
   −180°. All other joints stay at zero, the authored pose. Cost is
   joints × samples poses. Combined positions of several joints are not
   sampled.
3. **Only straddling pairs are re-checked.** Moving one joint moves its
   subtree rigidly. Pairs entirely inside or entirely outside the subtree keep
   their authored-pose result, so only pairs that straddle the subtree boundary
   get an exact Boolean. Bodies are regenerated once and moved with
   `GeometryKernel::transform_body`, using the same forward kinematics as
   URDF export and `animate-joints` (`KinematicTree::pose` after solving
   mates).
4. **Same contact rule as the static check.** Pairs use the explicit
   `AssemblyInterferenceTolerance` defaults (`1e-9 m` bounds, `1e-12 m³`
   common volume) through the shared `interference_volume` helper, so
   touching hubs of connected links are contact, not interference.
5. **Actionable evidence.** The report names the first sampled pose with the
   most interfering pairs, the joint's instance, and its position in degrees
   or millimetres, together with every pair. Pose and pair order are
   deterministic.

## Consequences

- An agent can guard a robot with one assertion and get refusals such as
  `worst instance:forearm at 61.7 deg: instance:base × instance:gripper, …`,
  then fix the design (narrow a limit, shorten a link) with a verified patch.
- A collision window narrower than the sample spacing can be missed, and so
  can collisions that need two joints away from zero at once. Both are
  documented limits. Raising `samples_per_joint` or adding grid sampling are
  follow-ups.
- Intentional volumetric overlap between connected links (for example a pin
  modelled inside its bore) counts as interference. Such assemblies need
  clearance in the model or a future pair-exclusion list.
- A brute-force cross-check test re-poses every instance and confirms that
  the shortcut in decision 3 finds the same pairs. The six-axis arm sweep at
  13 samples (78 poses) takes about 2 s in release builds.
