# ADR-014: Model Context Protocol server over the Agent API

Status: Accepted  
Date: 2026-09-25  
Roadmap: MCAD-P7-002

## Context

ADR-013 lets agents author complete designs through structural
`DesignPatch` operations. The only programmatic entry point, however, is
`opencad agent`: a project-specific JSON-RPC 2.0 method set over stdio.
General-purpose agent hosts (Claude Code, Claude Desktop, IDE agents) speak
the Model Context Protocol (MCP). Using `opencad agent` from them requires
custom glue, so MusubiCAD's intended workflow cannot be used by the agents it
was built for:

> an agent proposes a typed patch → MusubiCAD verifies it → a human approves

## Decision

Add `opencad mcp`, an MCP server on stdio, as a thin adapter over the existing
Agent API dispatch (`handle_agent_request_with_plugins`) and the file-layer
functions the CLI already uses.

- **Transport and protocol.** Newline-delimited JSON-RPC 2.0 on stdio. The
  server implements `initialize`, `notifications/initialized`, `ping`,
  `tools/list`, `tools/call`, `resources/list`, and `resources/read`, and
  negotiates protocol versions `2025-11-25`, `2025-06-18`, `2025-03-26`, and
  `2024-11-05`. It echoes a supported requested version and otherwise answers
  with its newest. Notifications receive no response. No network listener is
  started, as for `opencad agent`.
- **No new dependency.** The protocol subset is small and uses the
  `serde_json` types the Agent API already uses. An MCP SDK crate would add an
  async runtime and a large dependency tree to the CLI for little benefit
  (AGENTS.md §1).
- **Tools delegate; they never re-implement.**

  | Tool | Delegates to |
  |---|---|
  | `inspect_document`, `validate_document`, `query_document`, `explain_document`, `regen_document`, `diff_document` | The existing `opencad.*` Agent API methods |
  | `patch_dry_run`, `patch_apply` | `opencad.patch_dry_run_document`, `opencad.patch_apply_document` |
  | `review_patch` | `opencad review` (`generate_review`) |
  | `authoring_patch` | `opencad_ai::authoring_patch` on the document's design state |
  | `new_document` | Creates an empty part, assembly, or drawing document and refuses to overwrite an existing path |

  Every mutation still goes through `DesignPatch` validation, the transaction
  and history boundary, and the file layer. MCP adds no mutation path.
- **Results are structured.** A successful call returns the delegated JSON
  result as `structuredContent`, plus a pretty-printed text copy for hosts that
  only read text. A failed operation returns `isError: true` with the
  validation message, so an agent can read dependent lists and conflict
  reasons and repair its patch. Unknown tools and malformed arguments are
  JSON-RPC errors.
- **Resources give agents the contract.**
  - `musubicad://schema/design-patch` serves `schemas/ocad.patch.schema.json`.
  - `musubicad://guide/structural-patch` serves a short authoring guide
    embedded in the binary.
- **Surface parity.** MCP is an additional client of the Agent API, not a new
  command surface. A parity test asserts that every tool maps to an existing
  Agent method or a documented file-layer function (ADR-009).

## Consequences

- Any MCP host can inspect, author, dry-run, review, and apply designs with
  no MusubiCAD-specific glue.
- Protocol evolution is owned locally. New MCP versions need an explicit
  version-list change and tests.
- Tools take document paths, so the host's file permissions and the MCP
  client's tool-approval prompts govern which documents an agent may change.
  `patch_apply` and `new_document` are the only tools that write.
- History for undo and redo stays an opaque value that clients pass back, as
  in the Agent API. The first tool set does not expose undo or redo.
