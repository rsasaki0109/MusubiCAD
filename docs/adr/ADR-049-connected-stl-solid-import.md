# ADR-049: Explicit-unit connected STL solid import

- Status: Accepted
- Date: 2026-10-12
- Task: MCAD-P12-010

Import bounded binary or ASCII STL as an immutable triangle asset, referenced by
an `imported_mesh` feature with SHA-256, mandatory source length unit and proper
rigid transform. STL declares no trustworthy length unit; never infer it from
headers or destination metadata. New feature tags make older readers refuse
unknown data rather than treat STL bytes as STEP. Extend attachment/schema
admission together, preserving old STEP data and migration behaviour.

Initial admission is one connected, closed, consistently oriented manifold shell
with positive finite volume. Reject open/disconnected/degenerate/nonmanifold or
inverted meshes; do not repair or merge distinct solids implicitly. Weld shared
vertices on an explicit 1e-9 m grid, with finite coordinates and triangle budgets.
Mass/area/bounds calculations use f64 and a local origin to preserve translated
mesh precision. Normals are derived from vertex winding; STL normals/colors do
not override geometric truth. Preserve source bytes as a validated attachment.

Require one connected incident face fan at each vertex. Detect coplanar and
noncoplanar triangle intersections with analytic segment/plane and projected
edge tests, allowing only shared topology contacts within 1 nm. A deterministic
x-axis sweep is bounded to 1,000,000 candidate pairs; exceeding the budget
refuses admission rather than omitting checks. Input is capped at 4 MiB and
10,000 triangles, coordinates at 1e6 m. No sampled intersection heuristic.

The geometry boundary exposes neutral triangle data and opaque body handles.
OCCT constructs planar triangle faces, sews one closed shell at explicit 1e-8 m
tolerance, makes a solid, checks BRep validity/self-interference and compares
analytic mesh volume before returning private BREP bytes to cadrum. A standalone
native prototype verified a 80×60×6 mm box: valid solid, no self-interference,
volume 2.88e-5 m³. No OCCT type crosses the kernel boundary, and no curve-sampling
approximation is introduced.

Mock owns imported-mesh state per kernel instance, retaining accurate neutral
mesh properties/tessellation/transforms while preserving existing synthetic
behaviour for other handles. It must not encode a fake cube volume as an imported
mesh. No global mutable state. Source and derived shapes remain distinct.
Mock shape edits, CSG, compounds containing imported meshes and mesh inertia
explicitly refuse in this initial implementation; accurate import, mass, bounds,
tessellation and rigid placement are supported.

CLI/Agent/MCP build AddAttachment/AddFeature DesignPatch, verify candidate
regeneration/assertions and use backend transaction/history before persistence.
Invalid units, topology, digests or geometry refuse without model changes. Schema
updates, additive migration policy and old/new round trips accompany the feature.
Include Mock/OCCT analytic dimensions/volume, source-preserving refusals, examples,
MCP authoring guide/skill and feasible eval evidence. Open scan meshes, repair,
multiple disconnected solids and surface editing remain outside this admission.

Necessary source-integrity fixes accompany the import: validate source digests
before cache lookup, compare opaque attachments as bytes in history, and remove
obsolete checksum-owned assets from expanded documents during undo/removal.
Changed assets and symlinks refuse removal before model writes. Unknown user
files are preserved; no broad directory cleanup is performed.

See [SolidCraft reference notes](../plans/solidcraft-reference.md). SolidCraft
(formerly SolveCraft) separates mesh bodies from B-rep solids. Our initial scope
instead admits only meshes verified as one closed solid shell; open-mesh loading
requires a separate body contract and must not bypass this kernel's solid-handle
invariant. This comparison does not expand the accepted input scope.
