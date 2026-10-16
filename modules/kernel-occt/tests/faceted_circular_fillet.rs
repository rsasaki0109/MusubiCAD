#![cfg(feature = "occt")]
use opencad_kernel_occt::OcctGeometryKernel;
#[path = "../../geometry/tests/support/faceted_circular_fillet_cases.rs"]
mod cases;
#[test]
fn native_physical_fillet_matches_independent_mass_contacts_and_placement() {
    cases::physical(&OcctGeometryKernel::new(), false);
}
#[test]
fn native_phase_is_explicit_and_independent_of_brep_seam() {
    cases::phase_and_seam(&OcctGeometryKernel::new());
}
#[test]
fn native_guarded_refusals_preserve_stock() {
    cases::refusals(&OcctGeometryKernel::new());
}
