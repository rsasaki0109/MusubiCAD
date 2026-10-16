use opencad_geometry::*;
#[path = "support/faceted_circular_fillet_cases.rs"]
mod cases;
#[test]
fn analytic_physical_fillet_matches_independent_mass_contacts_and_placement() {
    cases::physical(&MockGeometryKernel::with_analytic_primitives(), true);
}
#[test]
fn analytic_phase_is_explicit_and_independent_of_stock_seam() {
    cases::phase_and_seam(&MockGeometryKernel::with_analytic_primitives());
}
#[test]
fn guarded_refusals_preserve_stock_and_default_mock_isolated() {
    let physical = MockGeometryKernel::with_analytic_primitives();
    cases::refusals(&physical);
    let p = cases::input(1, 1., RigidTransform::identity(), false);
    assert!(MockGeometryKernel::new()
        .grid_faceted_periodic_cosine(KernelBody::new(1), &p)
        .is_err());
    let body = cases::stock(&physical, 1.);
    let rims = physical
        .cylindrical_stock_rims(&body, [0., 0., 1.])
        .unwrap();
    let p = cases::input(rims.end_edge_id, 1., RigidTransform::identity(), false);
    let count = CountingGeometryKernel::new(&physical);
    assert!(count.grid_faceted_periodic_cosine(body, &p).is_ok());
    assert_eq!(count.call_count(), 1);
}
