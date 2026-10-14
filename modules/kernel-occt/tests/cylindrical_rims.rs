#![cfg(feature = "occt")]
use opencad_kernel_occt::OcctGeometryKernel;
#[path = "../../geometry/tests/support/cylindrical_rim_cases.rs"]
mod cases;
#[test]
fn native_rim_ids_name_owned_complete_circles_in_current_edge_iteration() {
    cases::physical(&OcctGeometryKernel::new(), false);
}
#[test]
fn native_rim_queries_refuse_modified_or_unsupported_stock_without_mutation() {
    cases::refusals(&OcctGeometryKernel::new());
}
