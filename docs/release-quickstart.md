# MusubiCAD CLI quick start

This archive contains the historical `opencad` CLI executable and a real, reviewable bracket
example. MusubiCAD is early-stage software; archive signing and notarization are not yet available.

## Verify the executable

Linux and macOS:

```bash
./opencad version
```

Windows PowerShell:

```powershell
./opencad.exe version
```

The output must report the same version as the archive name. Only use archives downloaded from the
official [MusubiCAD releases](https://github.com/rsasaki0109/MusubiCAD/releases) page and verify the
archive against `SHA256SUMS` attached to that release.

## Regenerate the included model

```bash
./opencad regen examples/bracket.ocad.d
```

On Windows, replace `./opencad` with `./opencad.exe`. Successful regeneration reports the OCCT
kernel, regenerated features, volume in cubic metres, and mass in kilograms.

## Generate a complete design review

```bash
./opencad review \
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
