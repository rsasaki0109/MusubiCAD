# Drawing milestone plan (historical Phase 4)

## Status

Complete

This is the historical Drawing implementation record. The active roadmap uses
Phase 4 for the Plugin API; remaining Drawing quality work is tracked as
`MCAD-P5-003` and `MCAD-P5-005` in the [canonical roadmap](roadmap.md).

## Goal

Add drawing documents that reference 3D models and export orthographic wireframe
SVG sheets.

## Milestones

### M4.1 — Drawing document + wireframe SVG (historical Task-174–178)

- [x] ADR-004: drawing document model
- [x] Core IDs: `SheetId`, `ViewId`, `DocumentKind::Drawing`
- [x] `opencad-drawing`: `DrawingModel`, projection, wireframe, SVG export
- [x] File I/O: `OcadDocument.drawing`, `graph/drawings.json`
- [x] CLI: `musubicad new … drawing`, `musubicad export … .svg`, inspect summary
- [x] Template: `bracket_front_view` with child `parts/bracket.ocad.d`
- [x] Schemas: `ocad.drawing.schema.json`, document `kind: drawing`
- [x] Example committed under `examples/bracket_front_view.ocad.d`
- [x] Agent API drawing queries/patches (historical Task-180+)

### M4.2 — Hidden-line removal (historical Task-177)

- [x] Deterministic mesh-based HLR with dashed hidden edges in SVG

### M4.3 — Dimensions (historical Task-179)

- [x] Model-driven aligned linear dimensions on sheets and SVG export

## Definition of done (M4.1)

- Drawing round-trip through `.ocad.d`
- `musubicad export drawing.ocad.d out.svg` produces valid SVG wireframe
- `cargo test -p opencad-drawing -p opencad-file` pass without OCCT link issues
- Docs: ADR-004, `docs/architecture/drawing.md`

## Notes

- HLR uses a tessellated-mesh midpoint depth test; partially occluded edges are not split.
- Windows OCCT validation requires MSVC 14.44 or newer for the cadrum prebuilt ABI.
