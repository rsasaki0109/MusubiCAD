#[path = "../../../examples/nominal_thread_sdk.rs"]
mod demo;
fn main() -> opencad_core::Result<()> {
    demo::run(&opencad_geometry::MockGeometryKernel::with_analytic_primitives())
}
