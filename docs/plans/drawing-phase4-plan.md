# Phase 4 Drawing plan

## Status

In progress (M4.1)

## Goal

Add drawing documents that reference 3D models and export orthographic wireframe
SVG sheets.

## Milestones

### M4.1 — Drawing document + wireframe SVG (Task-174–178)

- [x] ADR-004: drawing document model
- [x] Core IDs: `SheetId`, `ViewId`, `DocumentKind::Drawing`
- [x] `opencad-drawing`: `DrawingModel`, projection, wireframe, SVG export
- [x] File I/O: `OcadDocument.drawing`, `graph/drawings.json`
- [x] CLI: `opencad new … drawing`, `opencad export … .svg`, inspect summary
- [x] Template: `bracket_front_view` with child `parts/bracket.ocad.d`
- [x] Schemas: `ocad.drawing.schema.json`, document `kind: drawing`
- [x] Example committed under `examples/bracket_front_view.ocad.d`
- [x] Agent API drawing queries/patches (Task-180+)

### M4.2 — Hidden-line removal (Task-177)

- [x] Deterministic mesh-based HLR with dashed hidden edges in SVG

### M4.3 — Dimensions (Task-179)

- [x] Model-driven aligned linear dimensions on sheets and SVG export

## Definition of done (M4.1)

- Drawing round-trip through `.ocad.d`
- `opencad export drawing.ocad.d out.svg` produces valid SVG wireframe
- `cargo test -p opencad-drawing -p opencad-file` pass without OCCT link issues
- Docs: ADR-004, `docs/architecture/drawing.md`

## Notes

- HLR uses a tessellated-mesh midpoint depth test; partially occluded edges are not split.
- Windows MSVC OCCT link errors (`LNK2019`) remain a pre-existing environment issue.
