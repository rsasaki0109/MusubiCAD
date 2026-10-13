# MusubiCAD CLI v0.2.0 release evidence

Task: MCAD-P15-002. Observed on 2026-10-13 UTC.

The [official v0.2.0 release](https://github.com/rsasaki0109/MusubiCAD/releases/tag/v0.2.0)
was published at **2026-10-13 01:50:10 UTC**, neither draft nor prerelease.
No local binaries were uploaded; the existing tag workflow built, verified and
published its native artifacts. This is the first current release after v0.1.1.

## Source, version and exact-head gates

- Tagged main commit: `c2b7ffbfed87294a5524893f5a2c2075aa98bff5` (PR #90 merge).
- Annotated tag object: `8cf7345297a596ea156ea573b81350ea6708d99f`.
- `python tools/test_release_version.py --tag v0.2.0` passed: workspace, CLI,
  desktop package/config, both lockfiles and plugin manifests agree on 0.2.0.
- [Main CI 38331983824](https://github.com/rsasaki0109/MusubiCAD/actions/runs/38331983824)
  passed both Rust and desktop workflow contract jobs.
- [Main Desktop 38331984008](https://github.com/rsasaki0109/MusubiCAD/actions/runs/38331984008)
  passed plan, all four native platforms and headless/packaged Linux smoke.
- [Tag Release 38333073638](https://github.com/rsasaki0109/MusubiCAD/actions/runs/38333073638)
  passed plan, Linux/Windows/macOS Apple Silicon/macOS Intel and publication.
  Each native archive was extracted and its included editing tutorial executed
  with its packaged binary before upload. Windows used the corrected portable
  path keys and explicit UTF-8 transport.
- [Tag Desktop 38333073668](https://github.com/rsasaki0109/MusubiCAD/actions/runs/38333073668)
  passed all six expected jobs; these remain unsigned workflow artifacts under
  the existing desktop trust policy.

The guards matched exact SHA, event, tag/main and complete expected job inventory;
no skipped native check was treated as success.

## Official assets

These are the observed release sizes and official published SHA256SUMS entries.
Only Linux was independently downloaded/hashed locally; the other hashes are
published manifest values and their native executions are tag-workflow evidence.

| Asset | Bytes | Official SHA-256 |
|---|---:|---|
| `musubicad-cli-v0.2.0-linux-x86_64.tar.gz` | 24629843 | `613ab73ae3b82b56499930361f6d8e58eb7a704fe65d60d6e461439d8352c752` |
| `musubicad-cli-v0.2.0-macos-aarch64.tar.gz` | 18947346 | `bdf4f07eb0bf6996d74cb3b2749a8e6a201703ffead8b8ee41f9d17368c98cbf` |
| `musubicad-cli-v0.2.0-macos-x86_64.tar.gz` | 20350096 | `e6904b94a24826938cc25fc53b123cd5cd52839a5e3e6556d76f7d3fd030ddc0` |
| `musubicad-cli-v0.2.0-windows-x86_64.zip` | 19713918 | `c9d7368427076a6fca1c226909e18d881aa59fab161baf79cbaa0500e836f8b2` |
| `SHA256SUMS` | 428 | `checksum manifest` |

## Independent downloaded Linux verification

The official Linux archive and `SHA256SUMS` were downloaded over verified TLS.
Its computed SHA-256 matched
`613ab73ae3b82b56499930361f6d8e58eb7a704fe65d60d6e461439d8352c752`
before safe archive extraction or execution. A checksum-mismatch refusal was
also exercised; no output directory or executable was touched.

The extracted binary reported `musubicad 0.2.0` and `OCCT 8.0.0 (cadrum static)`.
Its packaged `tools/run_editing_tutorial.py` passed against its packaged ellipse
fixture/scenario: real MCP discovery, read-only sketch/feature reviews, typed
verified edits, stale/unit refusal preserving bytes/checksums, opaque Agent
Undo/Redo canonical identity, unchanged parameters/points/profile/document ID,
`80×30×10 mm` dimensions (3e-7 m tolerance), and fresh native volume
`1.8849555921538775e-5 m³` = `18849.555921538775 mm³` (1e-12 m³ tolerance).
The original fixture remained unchanged.

## Actual Codex host API smoke

The same checksum-verified downloaded binary was launched by actual
**Codex CLI 0.159.2 app-server** with a temporary `musubi_qa` stdio server
configuration. An ephemeral thread, negotiated experimental host API and
`mcpServerStatus/list` discovered all **35** MusubiCAD tools. Real
`mcpServer/tool/call` requests performed sketch inspection, read-only sketch and
feature reviews, typed apply commands, stale/unit refusals and fresh native
regeneration on a disposable fixture copy. Measured volume matched the analytic
result above; refusal preserved files and the original fixture was unchanged.
Owned server processes were stopped after the test.

**Model turns: 0.** This proves the actual host's startup/discovery/dispatch
path, not autonomous model tool selection, a persistent plugin installation,
Codex GUI interaction or a comparative benchmark. Separate Agent/Tauri history
and actual Linux Tauri interaction were already verified in P14-004/P15-001.
Cursor/Gemini sessions and Windows desktop GUI interaction remain unverified;
Windows native CLI/MCP/archive execution is the successful tag runner evidence.

## Scope and maturity credit

CLI artifacts remain unsigned: an official checksum is not a publisher
signature. Desktop signing/notarization credentials and publication remain
separate (ADR-005); no trust policy was weakened. No new CAD operation, source
schema, dependency or golden file is changed by this evidence task.

After this evidence PR's main merge, the official release, native archive
execution and independently downloaded distribution verification justify
ecosystem **11 → 13** (+0.30 weighted): general **49.35 → 49.65 (about 50%)**.
Agent operation remains 89. Before merge, the baseline remains about 49% / 89%.
The fixed ecosystem target 15 is not yet met; remaining client/GUI host evidence
stays MCAD-P15-003. Rubric weights/targets are unchanged.
