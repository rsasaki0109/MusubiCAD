# P15-003 actual host integration evidence

Codex evidence observed on 2026-10-13, based on main
`1e65a634c48f5d50b412161a8c9dc291c9db5141`; Gemini startup follow-up is based
on `03fc404c04ddfacfca5d1979200b1f56c3a28b86`. P15-003 remains **partial**.
No additional maturity points are assigned: general 49.65 (about 50%),
ecosystem 13, agent operation 89. Fixed rubric targets and weights are unchanged.

| Host | Version / platform | Observed scope | Remaining evidence |
|---|---|---|---|
| Codex | CLI 0.159.2, Linux x86_64 | Actual app-server MCP startup, 35-tool inventory, typed review/apply, refusal, source retention and native regeneration | Model-driven tool selection, persistent plugin installation and graphical client interaction untested |
| Windows | Native CLI archive tests on Windows release runner | Packaging/tutorial evidence from P15-002 only | Actual Windows desktop/client session unavailable in this Linux environment |
| Cursor | Executable unavailable | No actual session | Versioned client session required |
| Gemini | CLI 0.63.0, Linux x86_64 | Actual extension link/load, native MCP Connected, ACP initialize | ACP session/new refuses without API key; tools/list, host edits and authenticated model session untested |

An absent credential binding does not establish that every possible host login
is unavailable. The Gemini ACP empty-session attempt below actually refused;
no login, model turn or simulated host success is claimed. Installing a CLI
alone would not satisfy the missing editing-session evidence.

## Reproduce the Codex evidence

Use Python 3.10+, Codex with the experimental app-server API and a trusted
native MusubiCAD binary. Run from the checkout:

```bash
python tools/run_codex_host_smoke.py --binary /absolute/path/to/musubicad
python tools/test_codex_host_smoke.py
```

The smoke runner executes the supplied binary and the installed Codex executable
(`--codex` can select another executable). Version output is captured dynamically;
experimental host APIs may change. An incompatible API, timeout, missing tool,
unexpected refusal or mismatched geometry fails the run with a nonzero exit.
The regression tests use transport doubles solely to check failure handling;
they do not establish host integration. CI runs these tests, not a model session.

The runner copies `examples/ellipse_plate.ocad.d` to a temporary directory,
starts its own Codex app-server with process-local MCP configuration overrides,
and creates an **empty ephemeral thread**. It does not send `turn/start`, edit
persistent configuration, install plugins or call an external model API. All
editing operations are dispatched through Codex's actual `mcpServer/tool/call`
API to the actual MusubiCAD MCP process. Existing host configuration can still
be loaded by Codex; use an appropriately trusted host profile. Unrelated host
inventory, notifications and stderr are not included in the public report.
Temporary source and stderr files are removed and the owned app-server is
terminated after success or failure.

Both current development and downloaded official v0.2.0 Linux binaries passed:

- Live discovery of 35 tools, including the seven required editing tools.
- Sketch review and feature review leave all source/checksum bytes unchanged.
- Typed ellipse expressions become `40 mm`, `15 mm`, `0 deg`; extrusion becomes
  `10 mm`, through backend DesignPatch and transaction validation.
- Reusing the old feature revision and submitting a unitless point coordinate
  both refuse and preserve every source/checksum byte.
- Authored points, parameter expressions, document identity, feature order and
  profile reference remain intact; the original fixture remains byte-identical.
- Fresh OCCT regeneration yields `1.8849555921538775e-5 m³`, independently checked
  against `π × 0.04 m × 0.015 m × 0.01 m` with absolute tolerance `1e-12 m³`.
- Bounds are `0.0800002 m × 0.0300002 m × 0.0100002 m`, within `3e-7 m` of
  analytic `80 mm × 30 mm × 10 mm` dimensions (OCCT bounding tolerance).

The downloaded Linux archive's SHA-256 was verified before extraction/execution:
`613ab73ae3b82b56499930361f6d8e58eb7a704fe65d60d6e461439d8352c752`.
See [v0.2.0 release evidence](cli-release-v0.2.0-evidence.md) for its exact source,
publication, platform CI and archive trust scope. No other host's success is
inferred from this result. Opaque Undo/Redo remains separately tested by the
real Agent/tutorial route; MCP does not expose history tools in this catalog.

## Completing the remaining sessions

For each real Windows/Cursor/Gemini host, record OS and client versions, exact
MusubiCAD source or verified archive hash, actual installation/launch method,
live tool inventory and actual host-dispatched reviewed edits. Verify review
and refusal byte preservation, source IDs/expressions and fresh analytic native
volume/dimensions. Keep secrets and unrelated host traffic out of evidence.
Label any unavailable or failed check explicitly. A model-driven session and a
direct host API session must be distinguished. Retain P15-003 as partial until
its required host evidence is available; release-runner Windows execution is
not sufficient to claim interactive Windows desktop validation.


## P15-004 Gemini extension startup follow-up

Official npm package `@google/gemini-cli@0.63.0` was installed outside the
checkout with exact version, retained package-lock and `--ignore-scripts`.
`npm ci --ignore-scripts` also succeeded. Node.js meets the package's `>=20`
requirement. npm verified the registry package integrity:

```text
sha512-mmhqMmAdsoplCzrLQQC4yJBqr4uSKXghnrLspgUU6N9drmsaq7pi7oF09skRYzEq2dYK6eMT4UpFQP6rG2o/ew==
```

The actual Gemini executable loaded the checkout's `gemini-extension.json`,
which declares the MusubiCAD skill as its context file, in a disposable profile. No manifest changes,
global profile edits, disabled trust checks or model prompts were necessary.
For a trusted checkout and native binary, reproduce the connection check:

```bash
musubi_gemini=/workspace/.musubicad-setup/gemini-host/node_modules/.bin/gemini
musubi_session=$(mktemp -d)
mkdir -p "$musubi_session/profile" "$musubi_session/work"
(
  cd "$musubi_session/work"
  GEMINI_CLI_HOME="$musubi_session/profile" "$musubi_gemini" extensions link /workspace/MusubiCAD --consent
  GEMINI_CLI_HOME="$musubi_session/profile" PATH=/workspace/MusubiCAD/target/debug:$PATH "$musubi_gemini" mcp list
)
# Delete only the disposable directory created above after examining the result.
rm -r -- "$musubi_session"
```

Observed output included `Extension "musubicad" linked successfully and enabled`
and `musubicad (from musubicad): musubicad mcp (stdio) - Connected`.
**Check the reported state, not just the process exit code:** Gemini's list
command can exit successfully for disconnected servers. This command tests
extension loading and MCP initialization/connection; it does not list tools,
review/apply CAD edits, or demonstrate a model choosing the correct tools.

A separate actual `gemini --acp` process accepted JSON-RPC `initialize` with
protocol version 1 and reported agent `gemini-cli` version `0.63.0`. Calling
`session/new` with the disposable working directory and an empty `mcpServers`
list returned error code `-32000`, reporting a missing Gemini API key. No
`session/prompt` or `authenticate` request was sent; model turns remain zero.
The owned process was terminated and its stderr was not published.

The runtime has no configured GCP outbound connection or ready Gemini secret
binding. Presence of `GOOGLE_APPLICATION_CREDENTIALS` was checked by name only;
its contents were not read and its presence does not establish valid Vertex AI
setup. No API key, login, auth workaround or extra model-service network access
was invented. Actual tool discovery, editing/refusal and native regeneration
through Gemini remain unverified. Windows desktop and Cursor remain unavailable.

The cloud draft preserves existing Rust/OCCT/Mesa/Tauri setup and adds the pinned
Gemini install/startup instructions. Saving the draft does not apply or publish
it, and a new-task restore remains unverified. P15-004 is complete only for its
bounded startup acceptance; P15-003 remains partial, with no new maturity credit.
