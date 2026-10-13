# MusubiCAD CLI v0.3.1 release evidence

Task: MCAD-P15-009. Observed on 2026-10-13 UTC.

The [official v0.3.1 release](https://github.com/rsasaki0109/MusubiCAD/releases/tag/v0.3.1) was published at
**2026-10-13T13:10:07Z**, neither draft nor prerelease. The existing tag workflow
built and published every archive; no local binaries were uploaded.

## Exact source and native gates

- Tagged main commit: `a9ec76f8616ecfce146f178c5b6d741b2d083825` (PR #100 merge).
- Annotated tag object: `afb13d59fde0bc623d96102082772dbdea9145dc`.
- `python tools/test_release_version.py --tag v0.3.1` passed.
- [Main CI 38394984501](https://github.com/rsasaki0109/MusubiCAD/actions/runs/38394984501) passed its complete expected job inventory.
- [Main Desktop 38394984443](https://github.com/rsasaki0109/MusubiCAD/actions/runs/38394984443) passed its complete expected job inventory.
- [Tag Desktop 38400692305](https://github.com/rsasaki0109/MusubiCAD/actions/runs/38400692305) passed its complete expected job inventory.
- [Tag Release 38400692271](https://github.com/rsasaki0109/MusubiCAD/actions/runs/38400692271) passed its complete expected job inventory.

Main Desktop and tag Desktop each include all four platforms and headless/packaged
Linux smoke. Tag Release includes plan, Linux/Windows/macOS Apple Silicon/macOS
Intel and publication. Every native CLI archive was extracted and both the
editing and flexible-assembly tutorials executed with its native executable
before upload. Guards checked exact SHA, tag/event/main reachability and complete
job inventories. No skipped native check was counted as a success.

## Official assets

Only Linux was independently downloaded and hashed locally. Other hashes below
are official manifest values; their native tutorial execution is CI evidence.

| Asset | Bytes | Official SHA-256 |
|---|---:|---|
| `SHA256SUMS` | 428 | `checksum manifest` |
| `musubicad-cli-v0.3.1-linux-x86_64.tar.gz` | 24857369 | `6e3a9f76e2945ae08f4dd688edc19d0e3b73b9629b183cfe982670799840080e` |
| `musubicad-cli-v0.3.1-macos-aarch64.tar.gz` | 19183027 | `fbcc3732203a3c7b4d0880b1ea73a3570503ce92b8a8e4a2fa0d6c514e3b53da` |
| `musubicad-cli-v0.3.1-macos-x86_64.tar.gz` | 20612557 | `5c356258b487d5749e6c5eab99922df612f31697ee0e1c243502075b248ae629` |
| `musubicad-cli-v0.3.1-windows-x86_64.zip` | 19996855 | `e53446dd441c6a0181e0b1112b0629c72cedd781652aedd438864fd9353ccfb7` |

## Independent downloaded Linux verification

Official archive and checksum manifest were downloaded over verified TLS.
Computed SHA-256 `6e3a9f76e2945ae08f4dd688edc19d0e3b73b9629b183cfe982670799840080e` matched before safe extraction or execution;
downloaded sizes matched official metadata. The manifest covers exactly the four
archives. A separate synthetic checksum mismatch refused extraction/execution;
that negative check is not official asset evidence. The binary reported `musubicad 0.3.1` and `OCCT 8.0.0 (cadrum static)`.

The included editing tutorial passed using the included ellipse/scenario through
real MCP/Agent: read-only review, typed edits, native analytic dimensions/volume,
stale/unit refusals, source preservation and canonical Undo/Redo.

The included assembly tutorial passed for both slider and hinge: exact source/
revision/driver/connector inspection, parent-only reviewed constraints, independent
SI physical frames/bounds/volumes, five closed exported STL members and independent
volume/centre/bounds, zero context DOF, child/other-copy preservation, stale/unit/
range/owner/reference refusal, protected unsupported outputs and Agent Undo/Redo.
The archive includes all nested child documents/assets and runner inputs.

Tutorial evidence is actual MCP stdio/Agent execution with **zero model turns**;
it does not prove Codex/Cursor/Gemini registration or autonomous model editing.
Existing host evidence remains in [host integration evidence](host-integration-evidence.md).

## Scope and maturity

v0.3.1 includes P15-008 preservation of original STL asset bytes under Windows
Git checkout, with checksum validation unchanged. The earlier v0.3.0 tag failed
its imported-asset hash gate and was not published; that immutable tag is retained.
v0.3.1 includes P13-011/012 parent-owned articulated copies and admitted analytic
parent-context constraints, plus P15-006 packaged tutorial inputs. Published
v0.2.0 predates them. Older readers refuse the new articulated/flexible component
source guards rather than silently using rigid semantics. Fixed outer instances
and immediate child drivers remain the supported scope; free/nonlinear/deeper
systems and hierarchical motion/reach/URDF/HTML remain explicitly refused.

CLI archives are unsigned; SHA-256 verifies integrity, not publisher identity.
Tag Desktop artifacts are unsigned CI artifacts under the unchanged trust policy.
Actual Cursor and authenticated Gemini editing remain unverified. This publication
adds no rubric credit: solid 55, sketch 60, assembly 70, drawing 50, I/O 60, GUI 45,
ecosystem 14; weighted general CAD 50.10 (about 50%), agent operation 90.
