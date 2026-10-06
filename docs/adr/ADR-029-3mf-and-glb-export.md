# ADR-029: 3MF and GLB mesh export

Status: Accepted  
Date: 2026-10-06  
Roadmap: MCAD-P8-008

## Context

Agents that design parts for 3D printing need a file a slicer accepts
without repair, and web or chat viewers need a compact mesh with normals.
MusubiCAD exported STL (an unindexed triangle soup with no units) and STEP
(B-rep for CAD/CAM). Current slicers (Bambu Studio, PrusaSlicer,
OrcaSlicer) prefer 3MF: a zip package with an indexed, unit-bearing mesh.
glTF 2.0 binary (GLB) is the common format for viewers.

## Decision

1. Both encoders live in `modules/cli/src/mesh_formats.rs` as pure functions
   of the tessellated `MeshSet` (kernel metres). OCCT is not involved beyond
   the existing tessellation, and no OCCT toolkit is added.
2. **3MF** (`export *.3mf`):
   - unit `millimeter`, positions written with four decimals (0.1 µm,
     finer than the f32 mesh resolves at part scale);
   - vertices welded at 1 nm, because tessellation duplicates them along
     B-rep face boundaries. Triangles that collapse are dropped;
   - one object and build item per part, or per placed instance of an
     assembly. Parts that touch must not fuse into one non-manifold
     surface;
   - packaged with the existing `zip` dependency (deflate, fixed
     timestamps), so the output is deterministic.
3. **GLB** (`export *.glb`): glTF 2.0 binary with POSITION, NORMAL, and u32
   indices, one node and one mesh, metres. Vertex data keeps the model's
   +Z-up axes. The node carries a -90° rotation about X so viewers that use
   glTF's +Y-up convention show the part upright.

## Verification

- A unit cube welds from 24 to 8 vertices. Every undirected edge then has
  exactly two triangles.
- `bearing_carrier`, `robot_joint_actuator`, and `robot_arm_assembly`
  export as closed, consistently oriented meshes. Every directed edge
  appears once and its reverse once. The enclosed volume is within 1% of
  the regenerated solid (measured: 0.008%, 0.03%, 0.11%).
- 3MF: lib3mf in strict mode reads all three with 0 warnings and reports
  every object as manifold and oriented. trimesh reports them watertight.
- GLB: the Khronos glTF Validator reports 0 errors, 0 warnings, 0 infos.

## Consequences

- 3MF carries geometry only: no colours, materials, or print settings yet.
- GLB merges an assembly into one mesh; per-instance nodes and colours can
  follow.
- Mesh accuracy is the tessellation's default deflection, as for STL.
