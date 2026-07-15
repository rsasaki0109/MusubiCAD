# OCCT Installation

MusubiCAD uses OpenCASCADE (OCCT) for B-Rep solid modeling.

## Default: static OCCT (no system install)

The `opencad-kernel-occt` crate depends on **cadrum**, which downloads a prebuilt
OCCT 8.0.0 binary on first `cargo build`:

```bash
cargo build -p opencad-kernel-occt
cargo test -p opencad-kernel-occt
```

Supported prebuilt targets:

| Target | Prebuilt |
|---|---|
| `x86_64-unknown-linux-gnu` | ✅ |
| `aarch64-unknown-linux-gnu` | ✅ |
| `x86_64-pc-windows-msvc` | ✅ |
| `aarch64-apple-darwin` | ✅ |

No `sudo`, no `apt`, no `LD_LIBRARY_PATH`.

### Windows toolset requirement

The cadrum OCCT 8.0.0 prebuilt requires MSVC 14.44 or newer. Install or update
Visual Studio 2022 to version 17.14 with the **Desktop development with C++**
workload. Older MSVC 14.38 installations fail to link internal STL symbols such
as `__std_max_element_d` and `__std_min_4i`.

## Optional: system OCCT (Debian/Ubuntu)

For developers who also want distro headers/libs (future direct C++ bridge):

```bash
MusubiCAD/tools/install_occt.sh
```

Or manually:

```bash
sudo apt-get install -y \
  libocct-foundation-dev \
  libocct-modeling-data-dev \
  libocct-modeling-algorithms-dev \
  libocct-data-exchange-dev
```

## Disable OCCT

```bash
cargo build -p opencad-kernel-occt --no-default-features
```

Use `opencad-geometry::MockGeometryKernel` when OCCT is disabled.

## Verify

```bash
cargo test -p opencad-kernel-occt -- --nocapture
```

Integration test `occt_extrude_plate_volume` checks volume ≈ 80×60×6 mm³.

## Units

MusubiCAD internal units are **SI meters**. OCCT operations receive meter-scale coordinates.
