# MusubiCAD CLI quick start

This archive contains the `musubicad` CLI executable and a real, reviewable bracket example.
MusubiCAD is early-stage software; archive signing and notarization are not yet available.

The CLI was called `opencad` before v0.2.0. The archive keeps that name working: `opencad` is a
symbolic link on Linux and macOS, and `opencad.cmd` forwards to `musubicad.exe` on Windows.

## Verify the executable

Linux and macOS:

```bash
./musubicad version
```

Windows PowerShell:

```powershell
./musubicad.exe version
```

The output must report the same version as the archive name. Only use archives downloaded from the
official [MusubiCAD releases](https://github.com/rsasaki0109/MusubiCAD/releases) page and verify the
archive against `SHA256SUMS` attached to that release.

## Regenerate the included model

```bash
./musubicad regen examples/bracket.ocad.d
```

On Windows, replace `./musubicad` with `./musubicad.exe`. Successful regeneration reports the OCCT
kernel, regenerated features, volume in cubic metres, and mass in kilograms.

## Generate a complete design review

```bash
./musubicad review \
  examples/bracket.ocad.d \
  examples/agent/review_width_patch.json \
  --output review
```

Open `review/review.html` to inspect the `80 mm → 100 mm` change. The source document is not
mutated. Rendering requires a working Vulkan driver on Linux, DirectX 12/Vulkan on Windows, or
Metal on macOS.

## Platform security notices

The archives are not yet code-signed. Windows SmartScreen or macOS Gatekeeper may therefore ask for
confirmation. Inspect the checksum and repository source before choosing to run an unsigned build.

Reproducible editing tutorial: `docs/tutorials/editing.md` (distributed as
`EDITING-TUTORIAL.md` in CLI archives). Run `python tools/run_editing_tutorial.py`
in a built checkout, or pass the packaged `--binary` and `--fixture` paths.
The real MCP/Agent exercise checks reviews, stale/unit refusals, source identity,
Undo/Redo and native analytic dimensions/volume; it is not actual client-host
integration evidence. The inputs are in `examples/agent/editing_tutorial.json`.


Future current-main archives also contain `ASSEMBLY-TUTORIAL.md` and
`tools/run_assembly_tutorial.py`. Run the latter with `--binary ./musubicad`
(or `.\musubicad.exe` on Windows) to exercise both parent-context mechanisms
through actual MCP/Agent and independently check the exported geometry.
Published v0.2.0 predates these APIs and does not contain those tutorial inputs.
