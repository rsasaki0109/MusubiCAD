#![cfg(feature = "occt")]
use opencad_kernel_occt::OcctGeometryKernel;
#[path = "../../geometry/tests/support/cylindrical_draft_cases.rs"]
mod cases;
#[test]
fn native_cylinder_witness_and_neutral_draft_match_independent_integrals() {
    cases::physical(&OcctGeometryKernel::new(), false);
}
#[test]
fn invalid_native_cylindrical_draft_preserves_original_stock() {
    cases::refusals(&OcctGeometryKernel::new());
}
