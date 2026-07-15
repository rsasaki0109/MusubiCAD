# CLI releases

MusubiCAD CLI releases are built entirely by `.github/workflows/release.yml`. Do not upload local
binaries manually.

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

Pull requests that change the release inputs run the complete build matrix but cannot publish a
release. The `publish` job receives `contents: write` only for a matching tag run after every build
passes.

## Publish a version

1. Update the workspace version and changelog material in a normal task PR.
2. Wait for CI and the Release matrix to pass on `main`.
3. Create and push an annotated version tag:

```bash
git switch main
git pull --ff-only
git tag -a v0.1.0 -m "MusubiCAD CLI v0.1.0"
git push origin v0.1.0
```

4. Verify the GitHub Release has four platform archives and `SHA256SUMS`.
5. Download at least one archive, verify its checksum, and run the commands in `QUICKSTART.md`.

The binaries are currently unsigned. Code signing, notarization, and desktop installers require a
separate release task and credentials; the workflow must not pretend those guarantees exist.
