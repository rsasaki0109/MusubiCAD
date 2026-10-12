# SolidCraft (formerly SolveCraft) reference notes

Reviewed: 2026-10-12. User-supplied introduction:
https://fabscene.com/new/make/solvecraft-open-source-parametric-3d-cad-rust-fusion-76-parts-mcp/amp/
The introduction returned HTTP 403 in this environment; these findings come
from upstream documentation and source, not from reading that article.

The old repository redirects to
[bherbruck/solidcraft](https://github.com/bherbruck/solidcraft). Reference snapshot:
`07e263d7e95cac9a6851e48bffcb79f302aabfb8`.
This is a design reference, not a replacement for the fixed maturity rubric or
evidence of functionality tested in MusubiCAD. No upstream application was run.

| MusubiCAD work | Reference and useful practice | Application to MusubiCAD |
|---|---|---|
| P12-010 STL input | Upstream distinguishes mesh imports from solid B-reps; mesh bodies render, measure, move and export, while solid features require B-reps. `crates/io/src/lib.rs` validates imported mesh objects before making the feature. | State the narrower initial closed connected shell admission explicitly. Preserve units/source bytes and validate before committing. Open mesh support would need a separate neutral mesh-body contract; never pretend an arbitrary mesh is a solid. |
| Phase 13 sketch | README describes construction geometry, projection, trimming and associative offsets, and calls out partial/ambiguous profile identity limits. | Build editable operations through DesignPatch, retain semantic source identities, and test edits that consume corners or invalidate references. Splines remain gated by ADR-012 acceptance. |
| Phase 14 GUI | The engine command layer serves desktop UI, scripts, CLI and MCP. `docs/mcp.md` specifies command discovery, timeline inspection/editing and precise batch failure information. | Use a shared backend command catalog and DesignPatch adapters for Tauri. Test that equivalent UI/CLI/MCP actions produce the same graph/history outcome; keep selection and camera state outside model ownership. |
| Phase 14 transactions | MCP documentation describes checkpoint rollback restoring design, active sketch and undo history after batch failure. | Add meaningful failure-after-success and long-history tests for existing transaction semantics before exposing batched interactive editing. Do not replace rollback with merely issuing a guessed number of undo commands. |
| Phase 15 evaluations | `docs/oracle.md` reports 76 reference recipes and compares body count, volume, area and topology, with explicit tolerances and per-case exceptions. | Expand independently reproducible analytic recipes and MCP edit/rebuild tasks. Include semantic-reference stability, undo/redo and refusal cases alongside shape measurements. Keep benchmark coverage separate from overall maturity. |

The upstream 76/76 report uses relative tolerance 1e-3 for volume/area and
seam-normalised topology when necessary. It also documents exceptions, including
helix tolerances and a tight-bend case omitting area comparison. Its Fusion source
recipes/measurements are described as local and uncommitted. The README explicitly
says these checks do not establish complete geometric or workflow equivalence;
do not describe the project as fully equivalent to Fusion on that basis.

P14-001 / ADR-058 now pins the backend-only desktop editing contract: shared
typed commands/queries, explicit units, source/scene revision and semantic
admission, concrete DesignPatch preview, verified candidate regeneration and
checkpoint/history publication. The current source-only parameter/history
publication gap is scheduled for the editor gateway in P14-002. This is an
accepted architecture plan, with no new editor capability or maturity credit.

Primary references at the snapshot:

- [README and architecture](https://github.com/bherbruck/solidcraft/blob/07e263d7e95cac9a6851e48bffcb79f302aabfb8/README.md)
- [MCP contract](https://github.com/bherbruck/solidcraft/blob/07e263d7e95cac9a6851e48bffcb79f302aabfb8/docs/mcp.md)
- [Benchmark scope and exceptions](https://github.com/bherbruck/solidcraft/blob/07e263d7e95cac9a6851e48bffcb79f302aabfb8/docs/oracle.md)
- [Mesh import validation and STL parsing](https://github.com/bherbruck/solidcraft/blob/07e263d7e95cac9a6851e48bffcb79f302aabfb8/crates/io/src/lib.rs)

The README states MIT OR Apache-2.0 and retains third-party licence requirements.
These notes adopt no code or assets. Any later reuse must check the specific
file's licence and attribution, as well as MusubiCAD's module boundaries.
