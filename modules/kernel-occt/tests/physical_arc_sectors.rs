#![cfg(feature = "occt")]
use opencad_kernel_occt::OcctGeometryKernel;
#[path = "../../geometry/tests/support/arc_sector_cases.rs"]
mod cases;
#[test]
fn native_exact_sector_minor_half_major_and_reversed_arcs_match_analytic_measurements() {
    cases::physical(&OcctGeometryKernel::new());
}
#[test]
fn native_exact_sector_placement_symmetric_extent_and_mesh_match_independent_oracles() {
    cases::placement_mesh(&OcctGeometryKernel::new(), false);
}
