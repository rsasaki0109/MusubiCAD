//! OpenCASCADE backend for OpenCAD.
//!
//! MVP uses [cadrum](https://crates.io/crates/cadrum) to statically link OCCT 8.0.0
//! without a system install. Enable with the `occt` feature (default).

pub mod backend;
pub mod convert;
pub mod ffi;
pub mod step;
pub mod store;

pub use backend::OcctGeometryKernel;
pub use store::KernelStore;

/// Report whether the OCCT backend is compiled in.
pub fn is_available() -> bool {
    cfg!(feature = "occt")
}

/// Report OCCT version string when available.
pub fn version() -> Option<&'static str> {
    if is_available() {
        Some(OcctGeometryKernel::occt_version())
    } else {
        None
    }
}
