# Rendering architecture

ForgeCAD renders disposable tessellated meshes derived from the Design Graph.
The renderer never owns or mutates document state.

## Presentation pipeline

`opencad-render` provides a deterministic presentation path for screenshots and
animation:

1. `RenderScene` contains one or more tessellated meshes and their bounds.
2. `OrbitCamera::fit_bounds` establishes a model-relative camera distance.
3. `AnimationOptions::camera` derives yaw and pitch solely from explicit options
   and the zero-based frame index.
4. `presentation_overlay` adds a model-relative floor grid and B-Rep feature
   edges. Tessellation diagonals within one kernel face are omitted.
5. The wgpu solid shader applies fixed key, fill, ambient, and rim lighting.
6. `render_orbit_gif` renders RGBA frames and encodes an infinite-loop GIF.

The animation path performs no network access and reads no clock or random
state. Equal scene data and options produce the same camera sequence and image
dimensions. GPU raster differences between adapters remain a known limitation.

## Multi-mesh face identity

`FaceCatalog` namespaces kernel face IDs by scene mesh index. Kernel IDs are
stable within a regenerated body but are not assumed globally unique between
assembly instances.

## Units

Scene and grid coordinates use meters. Camera angles and CLI animation options
use explicitly named degrees. Image dimensions use pixels and timing uses frames
per second.
