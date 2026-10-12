# ADR-044: Explicit authored title blocks

- Status: Accepted
- Date: 2026-10-12
- Task: MCAD-P12-005

A sheet may contain optional TitleBlock metadata: title, drawing number and
revision, plus optional date, author, material and scale note. Values are explicit
single-line UTF-8 strings, with conservative field-specific character budgets.
Reject control characters and XML-forbidden BMP noncharacters; SVG escapes markup.
There are no inferred timestamps, authors, material or scale values. Scale note
is free authored metadata, not a model measurement.

Use existing AddSheet for creation or `set_title_block` through DesignPatch for
edit/clear. Schema/migration default old sheets to no block, preserving legacy
bytes. State, snapshots and authoring preserve metadata; layout failure prevents
persistence. Title blocks do not change model geometry or projection.

The initial template is a deterministic 180 × 48 mm frame at 5 mm bottom/right
margins. Four rows are 12 mm high: title; number/revision/date; author/material;
scale note. Labels use 2 mm font size, values 3 mm. Validate finite sheet size and
room for the frame. Rendering uses the shared sheet exporter on both kernel paths.
No automatic wrapping, overlap avoidance, inferred metadata or template library is
introduced. Font appearance/metrics depend on the SVG consumer; field budgets are
bounded initial layout constraints, not a manufacturing-standard certification.
