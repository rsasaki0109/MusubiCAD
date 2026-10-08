# ADR-032: Robot motion, reach, and a web viewer from the Design Graph

Status: Accepted  
Date: 2026-10-08  
Roadmap: MCAD-P9-001 to MCAD-P9-006

## Context

Phase 8 made MusubiCAD a CAD plugin for coding agents and exported robot
descriptions (URDF, ADR-028) with joints and limits (ADR-030). What a robot
designer, or someone watching, wants to see next is the robot moving, its
dimensions changing, and whether it reaches what it must reach. Before Phase 9
that meant a simulator (MuJoCo) or a GPU (`animate`), and no tool answered
"does the gripper reach this point?".

## Decision

1. **One kinematic chain.** `KinematicTree::pose` is the forward kinematics
   of the URDF tree: each link is its parent's frame composed with the joint
   origin and the joint motion. Positions past a declared limit are rejected
   with the limits in degrees or millimetres, never clamped. Every new
   surface uses it, after solving the assembly's mates the way regeneration
   does, so connector and link edits are reflected and the motion shown is
   the motion a simulator gets.
2. **GPU-free animation.** `animate-joints` and `animate-sweep` render with
   the ADR-031 CPU renderer. A `PreviewFraming` fitted to every frame keeps
   the camera still; a 5 × 7 bitmap font captions frames with the values that
   produced them; `PreviewStyle` adds `studio` and `dark` looks (gradient and
   a blurred ground shadow) while `plain` stays byte-identical to the agent
   preview. Sweeps apply each value as a validated `set_parameter` patch to an
   in-memory copy and regenerate it; the file is never written, and a value
   that does not regenerate stops the sweep and is named.
3. **Kernel-free reach.** `workspace` and `solve_reach` sample joint space on
   a deterministic grid inside the limits (at most 20 000 samples) and refine
   the six best samples by damped least squares with a finite-difference
   Jacobian and a backtracking step, clamped to the limits. No randomness,
   no new dependency, milliseconds per query. Reachable targets converge to
   about a micrometre; out-of-reach gaps are accurate to about 0.1 mm because
   the stretched pose is singular.
4. **Self-contained web viewer.** `export *.html` writes one page with the
   regenerated meshes (base64 `f32`/`u32`), an inline WebGL renderer, and a
   slider per joint bounded by its limits. No external scripts (no three.js),
   no network access, deterministic bytes. The page re-implements the same
   chain in JavaScript; a headless Chromium check agrees with the Rust pose to
   1e-10 m.
5. **Three surfaces.** Each capability is a CLI command, an Agent API method
   (`opencad.reach_document`, `opencad.animate_joints_document`,
   `opencad.animate_sweep_document`, `opencad.export` for HTML), and an MCP
   tool. The Agent API builds the CLI's flags from its params, so validation
   and units live in one parser.

## Consequences

- An agent can check reach before regenerating geometry, change a link with
  verified patches, and show the result as a GIF or a page without a GPU.
- Example sketches must be fully constrained for the motion story to hold:
  the robot arm link bars only constrained one edge's length and came out a
  wedge when lengthened, which verification did not catch. They now follow
  the MCP authoring guide (every rectangle edge horizontal or vertical), and
  the checked-in reach patch declares a mass expectation that would refuse a
  wedge.
- Moving a connector far (40 mm) needs the downstream instances moved with it
  for the mate solver to converge; the reach example patch does both.
- The CPU styles and the viewer are presentation only; the Design Graph stays
  the source of truth, and every image or page is regenerated from it.
