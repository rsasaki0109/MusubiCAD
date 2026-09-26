# CLI releases

MusubiCAD CLI releases are built entirely by `.github/workflows/release.yml`. Do not upload local
binaries manually.

The Tauri desktop shell has a separate [`Desktop` workflow](../../.github/workflows/desktop.yml).
It builds and uploads unsigned, versioned installer/archive artifacts for Windows x86-64, Linux
x86-64, macOS Apple Silicon, and macOS Intel on `main`, version tags, and manual dispatch. Pull
requests build and smoke-test Linux only, to keep review feedback fast. The exact artifact names, bundle formats, and checksum verification are documented in
the [desktop distribution quick start](desktop-releases.md). Desktop artifacts are not published
by the CLI `publish` job. Credential-gated Windows Authenticode, macOS signing/notarization, and
desktop release publication are isolated in the separate
[`Desktop signed release` workflow](../../.github/workflows/desktop-signed-release.yml), with the
current CI verification status tracked by `MCAD-P1-004`.

## Release contract

- The tag must be exactly `v` followed by the `opencad-cli` Cargo package version.
- The tagged commit must be reachable from `main`.
- Linux x86-64, Windows x86-64, macOS Apple Silicon, and macOS Intel must all build.
- Each native binary must report the expected version, regenerate the bracket through OCCT, and
  produce the expected `80 mm → 100 mm` semantic and geometry diff before it is packaged.
- Linux additionally generates the complete visual review against Mesa Vulkan. GitHub's headless
  macOS runners do not expose a Metal adapter, so macOS rendering requires a machine with a GPU.
- Every archive includes `LICENSE`, `README.md`, `QUICKSTART.md`, the bracket `.ocad.d` document,
  and its review DesignPatch.
- The release contains a generated `SHA256SUMS` file covering all four archives.

Pull requests that change the release inputs build and smoke-test the Linux archive and cannot
publish a release. Version tags run the complete build matrix. Non-Linux compile breakage is caught
on `main`, where the Desktop workflow builds every platform. In all three workflows, a newer push
to the same pull request cancels its running checks. The `publish` job receives `contents: write` only for a matching tag run after every build
passes.

## Publish a version

1. Update the workspace version and changelog material in a normal task PR.
2. Verify that the workspace crates, CLI, desktop package, Tauri
   configuration, both lockfiles, and intended tag agree:

```bash
python tools/test_release_version.py --tag v0.1.1
```

3. Wait for CI and the Release matrix to pass on `main`.
4. Create and push an annotated version tag:

```bash
git switch main
git pull --ff-only
git tag -a v0.1.0 -m "MusubiCAD CLI v0.1.0"
git push origin v0.1.0
```

5. Verify the GitHub Release has four platform archives and `SHA256SUMS`.
6. Download at least one archive, verify its checksum, and run the commands in `QUICKSTART.md`.

The CLI binaries remain unsigned. CLI signing is outside the desktop trust policy; the workflow
must not imply that a CLI checksum is a publisher signature. Desktop trust scope and the required
protected environment are documented in [desktop-releases.md](desktop-releases.md).
