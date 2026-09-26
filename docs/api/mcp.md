# MCP server

`opencad mcp` serves the Model Context Protocol over stdio
([ADR-014](../adr/ADR-014-mcp-server.md)). Every tool delegates to the
[Agent API](agent.md) or to a file-layer function the CLI already uses, so each
change still goes through `DesignPatch` validation, transactions, and history.
No network listener is started.

## Setup

Build or install the CLI, then register the server with an MCP host. For
example, with Claude Code:

```bash
claude mcp add musubicad -- opencad mcp
```

Any host that launches a stdio MCP server with the command `opencad mcp` works
the same way. Supported protocol versions: `2025-11-25`, `2025-06-18`,
`2025-03-26`, and `2024-11-05`.

## Tools

| Tool | Arguments | Writes | Delegates to |
|---|---|---|---|
| `new_document` | `path`, `kind` (`part`/`assembly`/`drawing`), `document_id`, `name` | yes (new path only) | `OcadDocument::new` + `write_ocad` |
| `inspect_document` | `path` | no | `opencad.inspect` |
| `validate_document` | `path` | no | `opencad.validate` |
| `explain_document` | `path` | no | `opencad.explain_document` |
| `query_document` | `path`, `query` | no | `opencad.query_document` |
| `authoring_patch` | `path` | no | `opencad_ai::authoring_patch` |
| `patch_dry_run` | `path`, `patch` | no | `opencad.patch_dry_run_document` |
| `review_patch` | `path`, `patch`, `output_dir` | review artifacts only | `opencad review` |
| `patch_apply` | `path`, `patch` | yes | `opencad.patch_apply_document` |
| `regen_document` | `path` | no | `opencad.regen_document` |
| `import_step` | `path`, `step_path`, `feature_id`, `name`, `operation`, `target_feature`, `translation_mm` | yes | `opencad import-step` |
| `export_document` | `path`, `output` (`.step`/`.stp`, `.stl`, `.svg`) | output file only | `opencad.export` |
| `diff_document` | `before`, `after` or `patch`, `geometry` | no | `opencad.diff_document` |

A successful call returns the delegated JSON result as `structuredContent` and
a pretty-printed text copy in `content`. A failed operation returns
`isError: true`, and its text is the validation message: dependent lists,
unknown references, and conflict reasons are named by stable ID so the agent
can repair its patch. Unknown tools and methods are JSON-RPC errors (`-32602`
and `-32601`).

`new_document` never overwrites an existing path. `review_patch` also writes
the submitted patch as `patch.json` in `output_dir`. A design without a body
before the patch (for example, a new document) is reviewed against an empty
"before" frame.

## Resources

| URI | Content |
|---|---|
| `musubicad://schema/design-patch` | `schemas/ocad.patch.schema.json` |
| `musubicad://guide/structural-patch` | [Structural patch authoring guide](mcp-authoring-guide.md) |

## Example session

The CLI integration test `modules/cli/tests/mcp.rs` drives a real
`opencad mcp` process:

1. `new_document` creates an empty part.
2. `patch_dry_run` validates
   [`examples/agent/author_plate_from_empty_patch.json`](../../examples/agent/author_plate_from_empty_patch.json).
3. `review_patch` renders the before/after review.
4. `patch_apply` writes the part.
5. `regen_document` reports the OCCT volume of a 60 × 40 × 5 mm plate with a
   10 mm through hole.

## Evaluating agent hosts

`tools/mcp_eval.py` measures how well a real MCP host authors parts with
these tools. It runs Claude Code headless, one task at a time, against
`opencad mcp` with only the `musubicad` tools allowed. It then checks the
result independently:

- `opencad regen` must report the analytic volume within 0.1 mm³;
- every required named parameter must exist.

The report records pass/fail, volume, turns, cost, and duration for each
task. The tasks are defined in `tools/mcp_eval_tasks.json`:

| Task | What the agent must do | Expected volume |
|---|---|---|
| `enclosure` | Author a 50 × 30 × 15 mm open-top enclosure with 1.5 mm walls from nothing | 5 368.5 mm³ |
| `two_hole_plate` | Author an 80 × 50 × 6 mm plate with two Ø8 mm through holes | 23 400.7 mm³ |
| `edit_bracket` | Widen and thicken the example bracket while keeping it rectangular. Its sketch starts constrained only by side lengths. | 47 375.7 mm³ |

Expected volumes treat sketch circles as the 32-sided inscribed polygons that
the kernel receives today (`CIRCLE_SEGMENTS`).

```bash
python tools/mcp_eval.py --opencad target/debug/opencad.exe --out eval.json
python tools/mcp_eval.py --only edit_bracket
python tools/mcp_eval.py --self-test   # checker only, no model calls
```

The harness calls a paid model, so it is not part of CI.

The first baseline run, on 2026-09-26, passed 3 of 3 tasks. It cost
$1.54 in total, and each task took 9–16 turns and 26–144 s. In
`edit_bracket`, the agent added horizontal and vertical constraints to the
bracket's under-constrained sketch before editing it.
