#![cfg(feature = "occt")]
use opencad_kernel_occt::OcctGeometryKernel;
#[path = "../../geometry/tests/support/countersink_cases.rs"]
mod cases;
#[test]
fn native_countersink_matches_independent_physical_union() {
    cases::physical(&OcctGeometryKernel::new(), false);
}
#[test]
fn native_countersink_refuses_invalid_or_unsupported_stock() {
    cases::refusals(&OcctGeometryKernel::new());
}

#[test]
fn countersink_negative_extrusion_remains_closed_and_physical() {
    cases::negative_extrusion(&OcctGeometryKernel::new(), false);
}
