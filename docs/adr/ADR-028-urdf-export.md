# ADR-028: URDF export from the assembly Design Graph

Status: Accepted  
Date: 2026-10-06  
Roadmap: MCAD-P8-006

## Context

Robot simulators (MuJoCo, Gazebo, Isaac Sim, PyBullet) and ROS consume URDF:
a tree of rigid links with inertials and meshes, joined by typed joints. A
hand-written URDF duplicates the CAD model and drifts from it: when a link
gets longer, the joint origins, meshes, masses, and inertia tensors all have
to be edited again by hand.

MusubiCAD assemblies already hold what a URDF needs. Instances are links,
mates say which parts are joined and about which axis, and the kernel
computes mass properties. There is no joint type, no joint limit, and no
joint-angle parameter in the Design Graph yet; the robot arm example poses
its joints by instance placements.

## Decision

1. **Derive, do not store.** `kinematic_tree` (in `modules/assembly`, pure and
   kernel-free) builds the tree from existing mates. The schema is unchanged.
   - The single grounded instance is the root.
   - A mated pair with a `concentric` mate becomes a `continuous` joint about
     the child's mate axis (connector or local frame, the same frame the mate
     solver uses). Any other mated pair becomes a `fixed` joint.
   - Loops are rejected, naming the mate. Unreached instances are fixed to
     the root with a warning.
   - Joint zero positions are the current assembly pose.
2. **Link frames.** A child link frame is its part frame translated to the
   joint origin on the axis, so the axis is the mate axis in part
   coordinates and meshes and inertials only shift by that offset.
3. **Physics from the kernel.** `GeometryKernel::inertia_about_com` is a new
   kernel-neutral method (default: unsupported) implemented by OCCT through
   `cadrum::Solid::inertia` and the parallel-axis theorem, and by the mock
   kernel as a cube. Density is the CLI default of 2700 kg/m³, as for every
   other reported mass.
4. **Files.** `export <assembly> <name>.urdf` writes the URDF and one binary
   STL per component next to it, referenced by bare file name so MuJoCo and
   most loaders resolve it relative to the URDF. Output is deterministic
   (12 significant digits; kernel noise snapped to 0).

## Verification

The robot arm export loads in MuJoCo 3.15 as three hinge joints; link world
positions at zero match the CAD placements within 1e-13 m, and the link
masses sum to the regenerated assembly mass. A 4 s simulation with initial
joint velocities stays finite. The OCCT inertia matches the analytic box
tensor within 1e-9 relative error at any placement.

## Consequences and follow-ups

- Joint limits and prismatic joints are now declared in the Design Graph
  ([ADR-030](ADR-030-assembly-joints.md)); joint-angle parameters are not.
- A lone concentric mate also permits sliding along the axis; the URDF models
  only the rotation.
- Nested sub-assemblies are rejected; parts only.
- Materials and per-part density are not read yet (`graph/materials.json` is
  unused); all parts use 2700 kg/m³.
- ROS tools that require `package://` mesh URIs need the meshes placed in a
  package; the export does not create one.
