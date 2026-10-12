# ADR-055: Explicit exploded assembly presentations

Status: Accepted

Task: MCAD-P13-006

## Decision

Persist named exploded views in the assembly graph, with stable `view:` IDs
and explicit world-space `translation_m` offsets for declared top-level instance
IDs. Create, replace and remove configurations through transactional DesignPatch
operations. No mechanical placement, mate or joint is changed.

Derive separate presentation tessellations by translating cloned SI vertices
after mechanical regeneration. Rendering and PNG preview export consume those
meshes. This kernel-neutral operation supports both Mock and OCCT tessellations. Mechanical regeneration, interference, BOM and URDF continue to
consume the original model and scene. Both Mock and OCCT use the same
translation contract; mesh volume is invariant and bounds translate analytically.
Preserve normals and topology. Mesh coordinates remain f32, with precision
limited by the existing renderer; analytic checks use explicit tolerances.

The current assembly renderer represents a child assembly as one compound body.
Move it as a rigid whole. Nested occurrence paths, generated pattern instances,
rotational explosion, automatic spacing and drawing-view integration are outside
this admission scope. Do not flatten, guess or silently ignore unresolved IDs.
Offsets are finite SI metres, bounded to one million metres per component;
each nonempty view has at most 256 unique target instances. Unlisted instances
retain their mechanical positions. A failed or incomplete regeneration cannot
produce a partial exploded presentation.

Absent collections migrate to empty and serialize as absent. Sort views by ID
and offsets by instance ID. Never infer a view from mechanical placements.
Add document/patch schemas and old/new expanded/packed/history round trips.

## Validation and risks

Named tests cover analytic translated bounds and unchanged volume in both
kernels; unchanged source scenes and mechanical evidence; compound-child
translation; duplicate/missing IDs, finite bounds and removal references;
transaction rollback and undo/redo; deterministic PNG export and old/new files.
Authoring example, guide, skill and an eval exercise the shared patch/preview API.
Presentation bounds must never be advertised as physical interference evidence.
