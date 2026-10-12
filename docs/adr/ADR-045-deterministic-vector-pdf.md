# ADR-045: Deterministic vector PDF output

- Status: Accepted
- Date: 2026-10-12
- Task: MCAD-P12-006

Reuse the existing validated SVG sheet layout as the vector rendering source.
Use pinned svg2pdf 0.13.0 (text only; images/filters disabled) and pdf-writer 0.12.1
for multi-page documents. This adds text-shaping/font-subsetting dependencies,
instead of a second independent drawing renderer or platform PDF subprocess.

Bundle unmodified DejaVu Sans with its license. Font resolution is memory-only,
with an explicit family; no system-font lookup or external image/resource
resolution is permitted. Strict conversion rejects missing glyphs before output
is written. Font coverage is bounded; unsupported CJK/other text fails closed.
The converter's strict PDF/A chunk mode is used for glyph validation; no PDF/A
certification or archival conformance is claimed.

Generate pages in canonical sheet order, with explicitly configured 72 DPI.
Replace only the PDF intermediate SVG root's physical size/viewBox with precise
sheet values: legacy SVG's rounded root dimensions must not round paper size.
PDF MediaBox uses sheet metres ×1000 ×72/25.4 points. Renumber per-sheet Form
XObject references deterministically and scale normalized objects to the page's
point dimensions. Preserve vector lines/arcs, patterns, even-odd holes, text and
embedded subset fonts. No timestamps or inferred metadata are emitted.

Regenerate every sheet/view/dimension before converting; assemble the complete
buffer before replacing the output file. Model files remain unchanged. Bound to
128 sheets and one million PDF objects; unsupported/invalid layouts, resources
or glyphs fail rather than dropping content. Existing SVG layout/wireframe
limitations carry through. No `.ocad` schema or model mutation is introduced.

A repository-external prototype verified five drawing examples, matching A4
point dimensions, fresh-process byte determinism, strict glyph refusal and a
multi-page section with clear hole hatching, independently read/rasterized by
Poppler. Tests in this task retain those checks as applicable.
