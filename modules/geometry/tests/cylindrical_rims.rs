use opencad_geometry::*;
#[path = "support/cylindrical_rim_cases.rs"]
mod cases;
#[test]
fn analytic_rims_are_current_physical_circles_with_independent_measurements() {
    cases::physical(&MockGeometryKernel::with_analytic_primitives(), true);
}
#[test]
fn unsupported_analytic_rims_refuse_without_changing_stock() {
    cases::refusals(&MockGeometryKernel::with_analytic_primitives());
    let legacy = MockGeometryKernel::new();
    assert!(legacy
        .cylindrical_stock_rims(&KernelBody::new(1), [0., 0., 1.])
        .is_err());
    assert!(legacy
        .circular_edge_geometry(&KernelBody::new(1), 5)
        .is_ok());
}
#[test]
fn rim_data_validation_rejects_wrong_ids_and_circle_geometry() {
    let k = MockGeometryKernel::with_analytic_primitives();
    let body = cases::cylinder(&k, 0.02, 0.03);
    let rims = k.cylindrical_stock_rims(&body, [0., 0., 1.]).unwrap();
    let start = k.circular_edge_geometry(&body, rims.start_edge_id).unwrap();
    let end = k.circular_edge_geometry(&body, rims.end_edge_id).unwrap();
    for id in [0, rims.end_edge_id] {
        assert!(CylindricalRimsGeometry {
            start_edge_id: id,
            ..rims
        }
        .validate()
        .is_err());
    }
    assert!(rims.validate_circles(&end, &start).is_err());
    for bad in [
        CircularEdgeGeometry {
            radius_m: f64::NAN,
            ..end
        },
        CircularEdgeGeometry {
            radius_m: 0.021,
            ..end
        },
        CircularEdgeGeometry {
            center_m: [f64::NAN, 0., 0.],
            ..end
        },
        CircularEdgeGeometry {
            normal: [0., 0., 2.],
            ..end
        },
        CircularEdgeGeometry {
            normal: [1., 0., 0.],
            ..end
        },
    ] {
        assert!(rims.validate_circles(&start, &bad).is_err());
    }
}
