# MCP server

`musubicad mcp` serves the Model Context Protocol over stdio
([ADR-014](../adr/ADR-014-mcp-server.md)). Every tool delegates to the
[Agent API](agent.md) or to a file-layer function the CLI already uses, so each
change still goes through `DesignPatch` validation, transactions, and history.
No network listener is started.

## Setup

Install the prebuilt CLI (one self-contained executable, checksum-verified;
[ADR-027](../adr/ADR-027-agent-plugin-distribution.md)):

```bash
curl -fsSL https://raw.githubusercontent.com/rsasaki0109/MusubiCAD/main/install.sh | sh
```

On Windows PowerShell:
`irm https://raw.githubusercontent.com/rsasaki0109/MusubiCAD/main/install.ps1 | iex`.

Then add the plugin, which registers this server and the `musubicad` skill,
in Claude Code:

```bash
claude plugin marketplace add rsasaki0109/MusubiCAD
claude plugin install musubicad@musubicad
```

or register only the server:

```bash
claude mcp add musubicad -- musubicad mcp
```

The repository also carries Codex (`.codex-plugin/`), Cursor
(`.cursor-plugin/`), and Gemini CLI (`gemini-extension.json`) manifests and a
`skills.sh.json` listing; they launch the same server but have not been
verified on those hosts yet.

Any host that launches a stdio MCP server with the command `musubicad mcp` works
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
| `patch_dry_run` | `path`, `patch`, `verify` | no | `opencad.patch_dry_run_document` |
| `review_patch` | `path`, `patch`, `output_dir` | review artifacts only | `musubicad review` |
| `patch_apply` | `path`, `patch`, `verify` | yes, only when verified | `opencad.patch_apply_document` |
| `regen_document` | `path` | no | `opencad.regen_document` |
| `import_step` | `path`, `step_path`, `feature_id`, `name`, `operation`, `target_feature`, `translation_mm` | yes | `musubicad import-step` |
| `export_document` | `path`, `output` (`.step`/`.stp`, `.stl`, `.svg`) | output file only | `opencad.export` |
| `diff_document` | `before`, `after` or `patch`, `geometry` | no | `opencad.diff_document` |

A successful call returns the delegated JSON result as `structuredContent` and
a pretty-printed text copy in `content`. A failed operation returns
`isError: true`, and its text is the validation message: dependent lists,
unknown references, and conflict reasons are named by stable ID so the agent
can repair its patch. Unknown tools and methods are JSON-RPC errors (`-32602`
and `-32601`).

`patch_dry_run` and `patch_apply` regenerate the model before and after the
patch and check the patch's declared `expected_effects`
([geometry verification](agent.md#geometry-verification)). `patch_apply`
refuses a patch whose model does not regenerate or whose expected effects do
not hold, and the error names the failing feature or effect; the document is
not written. Pass `verify: false` only for a staged authoring step that cannot
regenerate yet.

`new_document` never overwrites an existing path. `review_patch` also writes
the submitted patch as `patch.json` in `output_dir`. On a host without a GPU
adapter it writes the reports without images and sets `images_skipped`. A design without a body
before the patch (for example, a new document) is reviewed against an empty
"before" frame.

## Resources

| URI | Content |
|---|---|
| `musubicad://schema/design-patch` | `schemas/ocad.patch.schema.json` |
| `musubicad://guide/structural-patch` | [Structural patch authoring guide](mcp-authoring-guide.md) |

## Example session

The CLI integration test `modules/cli/tests/mcp.rs` drives a real
`musubicad mcp` process:

1. `new_document` creates an empty part.
2. `patch_dry_run` validates and verifies
   [`examples/agent/author_plate_from_empty_patch.json`](../../examples/agent/author_plate_from_empty_patch.json).
3. `review_patch` renders the before/after review.
4. `patch_apply` writes the part.
5. `regen_document` reports the OCCT volume of a 60 × 40 × 5 mm plate with a
   10 mm through hole.

## Evaluating agent hosts

`tools/mcp_eval.py` measures how well a real MCP host authors parts with
these tools. It runs Claude Code headless, one task at a time, against
`musubicad mcp` with only the `musubicad` tools allowed. It then checks the
result independently:

- `musubicad regen` must report the analytic volume within 0.1 mm³;
- every required named parameter must exist.
- for assembly tasks, the instance and active mate counts must match, and
  `mate_max_error` must be at most 1 µm;
- for drawing tasks, the view projections and dimension lengths (within
  1 µm) must match, and `musubicad export` must render the drawing to SVG.
  Drawing tasks have no expected volume.

The report records pass/fail, volume, turns, cost, and duration for each
task. The tasks are defined in `tools/mcp_eval_tasks.json`:

| Task | What the agent must do | Expected volume |
|---|---|---|
| `enclosure` | Author a 50 × 30 × 15 mm open-top enclosure with 1.5 mm walls from nothing | 5 368.5 mm³ |
| `two_hole_plate` | Author an 80 × 50 × 6 mm plate with two Ø8 mm through holes | 23 396.8 mm³ |
| `slot_plate` | Author a 60 × 40 × 5 mm plate with a 30 × 8 mm through slot (lines and arcs, ADR-021) | 10 548.7 mm³ |
| `edit_bracket` | Widen and thicken the example bracket while keeping it rectangular. Its sketch starts constrained only by side lengths. | 47 371.7 mm³ |
| `sweep_elbow` | Sweep a Ø10 mm section up 30 mm and around a 20 mm radius quarter bend (ADR-023) | 4 823.6 mm³ |
| `coil_spring` | Coil a Ø2 mm wire at a 10 mm radius, 5 mm pitch, 20 mm high (ADR-025); OCCT lands 0.036 mm³ low | 789.6 mm³ |
| `loft_cone` | Loft a Ø40 mm circle into a Ø20 mm circle 30 mm higher (ADR-022) | 21 991.1 mm³ |
| `import_step` | Import a STEP file (exported by a `setup` step) as a fixed solid | 28 328.8 mm³ |
| `fillet_bracket_top` | Round the bracket's top outer edges and hole rim with 1 mm fillets | 28 262.0 mm³ |
| `bracket_drawing` | Draw the bracket (created by a `new` setup step) in front and top views with an 80 mm width dimension | — |
| `assembly_third_bracket` | Add a third bracket instance to the two-bracket assembly and place it with a 120 mm distance mate | 84 986.3 mm³ |

`setup` lists `musubicad` argument lists that run before the agent starts.
Their arguments may use `{root}`, `{workdir}`, and `{document}`. Expected
volumes are analytic, except `fillet_bracket_top`, which uses the verified
fillet golden value. Sketch circles reach the kernel as exact
circles (ADR-020). The first baseline below still built 32-sided polygons.

```bash
python tools/mcp_eval.py --musubicad target/debug/musubicad.exe --out eval.json
python tools/mcp_eval.py --only edit_bracket
python tools/mcp_eval.py --self-test   # checker only, no model calls
```

The harness calls a paid model, so it is not part of CI.

On 2026-09-26, `slot_plate`, `import_step`, and `fillet_bracket_top`
passed 3 of 3 on their first run, for $1.75 in total. `loft_cone` passed on its first run in 8 turns for $0.38; the agent put
the top circle on a raised custom workplane. The agent drew the
slot with lines and endpoint arcs. The first baseline run, on 2026-09-26,
passed 3 of 3 tasks. It cost
$1.54 in total, and each task took 9–16 turns and 26–144 s. In
`edit_bracket`, the agent added horizontal and vertical constraints to the
bracket's under-constrained sketch before editing it.
