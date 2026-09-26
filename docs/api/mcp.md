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
