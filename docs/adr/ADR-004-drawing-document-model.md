# ADR-004: Drawing document model

## Status

Accepted

## Context

MusubiCAD Phase 4 adds 2D drawing documents that reference regenerated 3D
part or assembly geometry and export orthographic views to SVG. The manifest
already reserves graph slots; assemblies established the pattern of extending
`OcadDocument` with a dedicated model and `DocumentMetadata.kind`.

## Decision

Adopt the same approach as ADR-003:

1. Add `DocumentMetadata.kind = drawing` (alongside `part` and `assembly`).
2. Add `OcadDocument.drawing: Option<DrawingModel>` (skip when `None`).
3. Serialize drawing data to `graph/drawings.json`.
4. Drawing documents do not own B-Rep; each view references a child `.ocad`
   path and stores sheet placement + orthographic projection kind.
5. SVG export classifies hidden mesh edges and renders model-driven linear dimensions.

```
DrawingModel
└─ sheets: Vec<Sheet>
   ├─ views: Vec<DrawingView>   // model ref + projection + sheet placement
   └─ dimensions: Vec<LinearDimension> // view ref + model-space measurement points
```

## Consequences

### Positive

- Reuses CLI, file I/O, and Agent API document pipeline
- Kernel-neutral projection and SVG live in `opencad-drawing`
- Dimension values remain derived from referenced-model geometry

### Negative

- Drawing documents still carry unused part fields
- Mesh midpoint HLR does not split partially occluded edges

## Alternatives considered

| Alternative | Rejected because |
|---|---|
| Separate `.odrw` format | Duplicates manifest and migration plumbing |
| Embed geometry in drawing | Breaks single source of truth for 3D models |
