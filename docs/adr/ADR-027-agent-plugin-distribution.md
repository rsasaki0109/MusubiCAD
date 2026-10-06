# ADR-027: Agent plugin distribution

Status: Accepted  
Date: 2026-10-06  
Roadmap: MCAD-P8-005

## Context

`musubicad mcp` (ADR-014) already gives agent hosts verified design tools
(ADR-026), but reaching it took a Rust toolchain and a first OpenCASCADE
build, or a manual archive download, followed by hand-written MCP host
configuration. Agent users expect one install line per host.

Package registries were considered and rejected for now:

- npm (per-platform binary packages run through `npx`) would add a registry
  account, a publish step, and a Node dependency to every launch.
- PyPI wheels with an embedded binary (`uvx`) would add the same publish
  step and a Python toolchain.

The CLI is already one self-contained executable per platform (OpenCASCADE
is linked statically through `cadrum`) published on GitHub Releases with
`SHA256SUMS`.

## Decision

1. **Binary.** `install.sh` (Linux, macOS) and `install.ps1` (Windows)
   download the release archive for the machine, verify it against the
   release's `SHA256SUMS`, and install `musubicad` into
   `~/.local/bin` or `%LOCALAPPDATA%\Programs\musubicad` (added to the user
   PATH). `MUSUBICAD_VERSION` pins a release. They refuse a checksum
   mismatch and an archive from before the `musubicad` name, and install
   nothing in either case.
2. **One server definition.** Every host launches the same server,
   `{"command": "musubicad", "args": ["mcp"]}`, found on PATH:
   `plugin/mcp.json` for the Claude Code, Codex, and Cursor plugin
   manifests (`.claude-plugin/`, `.codex-plugin/`, `.cursor-plugin/`), and
   inline in `gemini-extension.json`. A PATH command needs no
   platform-specific launcher script.
3. **One skill.** `skills/musubicad/SKILL.md` teaches the workflow
   (parameters first, dry-run until `verification.passed`, show the diff,
   apply, export) and the equivalent CLI commands for hosts without MCP.
   `skills.sh.json` lists it for `npx skills add`, which reads the
   repository and needs no published package.
4. **Versions move together.** Plugin manifests carry the workspace
   version; `tools/test_release_version.py` enforces it, and
   `tools/test_agent_plugin.py` checks the manifests, skill frontmatter, and
   runs `install.sh` against a local release (success, tampered checksum,
   old archive) in CI.

## Consequences

- Installing is two steps per host: the binary, then the plugin or MCP
  entry. The skill tells the agent how to do the first step, so "ask your
  agent to install MusubiCAD" works.
- The plugin does not pin the binary version. The MCP server reports its
  version in `initialize`, and the skill asks for 0.2.0 or newer.
- Windows host behaviour (resolving `musubicad` to `musubicad.exe` on PATH
  when an agent host spawns the server) still needs a check on a Windows
  machine before it is advertised.
- Archives are not yet code-signed (ADR-005).
