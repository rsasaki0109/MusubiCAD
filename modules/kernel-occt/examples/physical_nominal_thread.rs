#[path = "../../../examples/nominal_thread_sdk.rs"]
mod demo;
fn main() -> opencad_core::Result<()> {
    demo::run(&opencad_kernel_occt::OcctGeometryKernel::new())
}
