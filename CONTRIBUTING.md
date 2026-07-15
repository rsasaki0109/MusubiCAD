# Contributing to MusubiCAD

MusubiCAD welcomes focused contributions to parametric modeling, geometry, sketches, rendering,
file formats, AI-safe workflows, documentation, and examples. The Design Graph—not the GUI, mesh,
or cached B-Rep—is always the source of truth.

## Start in 60 seconds

Before building, open the real generated review:

```bash
./quickstart.sh
```

Windows PowerShell users can run `./quickstart.ps1`. This read-only tour shows the semantic,
geometric, and expected-effect evidence a contribution must preserve.

## Choose one task

1. Pick an open issue labeled [`good first issue`](https://github.com/rsasaki0109/MusubiCAD/labels/good%20first%20issue)
   or [`help wanted`](https://github.com/rsasaki0109/MusubiCAD/labels/help%20wanted).
2. Comment on the issue before substantial work so scope is visible.
3. Keep one task per pull request. Do not combine refactoring with a feature or fix.
4. Read [`AGENTS.md`](AGENTS.md), even when you are not using an AI coding agent. It defines module
   boundaries, validation rules, and design invariants.

For a proposal that changes architecture, open an issue first. An accepted design must be recorded
as an ADR under `docs/adr/` before implementation.

## Build and test

Install stable Rust, then run:

```bash
cargo test --workspace
cargo run -p opencad-cli -- version
```

OCCT 8.0 is statically provided through `cadrum`; no system OCCT installation is required. Linux
render tests require a Vulkan driver such as Mesa. See the
[developer guide](docs/developer-guide/index.md) for workspace details.

Before opening a code pull request, all required checks must pass:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

Documentation-only changes do not require Rust tests, but links, commands, YAML, and rendered
Markdown must still be checked in proportion to the change.

## Implementation rules

- Model mutations go through transactions.
- AI changes go through `DesignPatch`; never bypass validation.
- Failed regeneration must leave the document unchanged.
- Public numeric values use explicit units.
- Geometry comparisons use documented tolerances, never exact floating-point equality.
- `.ocad` serialization and graph traversal remain deterministic.
- OCCT types never escape `modules/kernel-occt`.
- UI commands must also be available through the CLI or Agent API.

Every feature needs tests and at least one example under `examples/`. Public API changes update
`docs/api/`; architecture changes update `docs/architecture/`; schema changes update `schemas/`,
migrations, and round-trip tests.

## Pull request contract

Use the title format `Task-XXX: Short imperative title`. The pull request template asks for:

- linked task or issue
- summary and user impact
- implementation notes
- tests added and commands run
- docs and examples updated
- known limitations

Explain golden-file changes and do not disable tests. Maintainers may ask to split unrelated work
before review.

## Reporting problems and proposing features

Use the structured [issue forms](https://github.com/rsasaki0109/MusubiCAD/issues/new/choose). Include
the exact command, platform, MusubiCAD version, explicit units, and the smallest shareable `.ocad.d`
or `DesignPatch` example. Remove secrets and proprietary geometry before uploading files.
