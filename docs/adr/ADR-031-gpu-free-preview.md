# ADR-031: GPU-free preview images for agents

Status: Accepted  
Date: 2026-10-06  
Roadmap: MCAD-P8-010

## Context

An agent that edits geometry from text alone can only trust numbers. It
cannot see a misplaced hole or an inverted cut. MusubiCAD already rendered
PNGs and GIFs through wgpu, but that needs a GPU adapter, and the hosts that
run coding agents (cloud containers, CI, headless servers) usually have none
(see ADR-026 and the P8-002 review fallback). No MCP tool returned an image.

## Decision

1. **CPU renderer.** `opencad_render::render_preview` is a small software
   rasterizer that needs no GPU and no new dependency:
   - orthographic projection with +Z up as in the model, and named views
     `iso`, `front`, `top`, `right`;
   - two-sided Lambert shading from two lights, and a depth buffer;
   - dark outlines where depth jumps or the normal turns more than about 35°;
   - 2× supersampling with box-filter downsampling.

   It uses pure f32 arithmetic, so the output is deterministic on a given
   platform.
2. **One function, three surfaces.**
   - `modules/cli/src/preview.rs` regenerates a part, or every placed
     assembly instance with its own palette colour, renders it, and encodes
     a PNG with the existing `image` crate.
   - The CLI exposes it as `musubicad preview <doc> <out.png> [--view …]`.
   - The Agent API exposes it as `opencad.preview_document`, which returns
     base64 plus a summary.
   - MCP exposes it as `preview_document`, which returns MCP `image` content
     (`image/png`) followed by a text summary: view, size, triangle count,
     bounds in millimetres, and object names.
3. **The skill tells agents to look** after authoring or after a shape
   change, and to describe what they see before claiming the result matches
   the request.

## Verification

- Unit tests for the renderer:
  - A box fills the centre and leaves the corners empty in every view.
  - A top view frames a plate's long side horizontally.
  - Output is deterministic, and the size and view are validated.
- Preview tests:
  - The bearing carrier and the robot arm render to PNG, with 1 and 4
    objects.
  - The carrier's bounds are 96 × 72 × 14 mm.
  - CLI arguments parse.
- An MCP test checks that `preview_document` returns `image/png` content,
  that the content decodes to a PNG, and that an unknown view is an error.
- The full robot arm (4,296 triangles, 640×480) renders in about 0.25 s in
  release.

## Consequences

- Images show shape, not materials or exact appearance. The wgpu path is
  still used for review GIFs when a GPU exists.
- Drawings are not previewed. Agents export them as SVG.
