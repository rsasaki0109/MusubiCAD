#![cfg(feature = "occt")]
use opencad_kernel_occt::OcctGeometryKernel;
#[path = "../../geometry/tests/support/revolve_cases.rs"]
mod cases;
#[test]
fn native_exact_revolves_have_analytic_measurements_world_bounds_and_contacts() {
    cases::physical(&OcctGeometryKernel::new());
}
#[test]
fn native_exact_revolve_rigid_placement_and_normals_match_independent_oracle() {
    cases::placed(&OcctGeometryKernel::new(), false);
}
#[test]
fn native_in_plane_rotated_rectangle_and_axial_origin_shift_match_world_oracle() {
    cases::in_plane(&OcctGeometryKernel::new());
}
