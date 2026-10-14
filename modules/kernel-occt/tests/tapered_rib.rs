#[path = "../../geometry/tests/support/tapered_rib_cases.rs"]
mod cases;
use opencad_kernel_occt::OcctGeometryKernel;
#[test]
fn physical_native_rib_union_matches_independent_integrals_contact_bounds_and_mesh() {
    cases::physical(&OcctGeometryKernel::new(), false);
}
#[test]
fn invalid_native_rib_inputs_preserve_stock() {
    cases::invalid(&OcctGeometryKernel::new());
}
