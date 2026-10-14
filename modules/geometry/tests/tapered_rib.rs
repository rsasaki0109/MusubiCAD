use opencad_geometry::*;
#[path = "support/tapered_rib_cases.rs"]
mod cases;
#[test]
fn physical_mock_rib_union_matches_independent_integrals_contact_bounds_and_mesh() {
    cases::physical(&MockGeometryKernel::with_analytic_primitives(), true);
}
#[test]
fn invalid_mock_rib_inputs_preserve_stock_and_default_mock_refuses() {
    cases::invalid(&MockGeometryKernel::with_analytic_primitives());
    let input = TaperedRibInput {
        entry: FacePick {
            point_m: [0.; 3],
            normal: [0., 0., 1.],
        },
        length_axis_hint: [1., 0., 0.],
        length_m: 0.006,
        base_thickness_m: 0.002,
        height_m: 0.003,
        angle_rad: 0.1,
    };
    assert!(MockGeometryKernel::new()
        .add_tapered_rib_stock(KernelBody::new(1), &input)
        .is_err());
}

#[test]
fn physical_rib_mesh_quality_and_unsupported_edits_refuse() {
    let k = MockGeometryKernel::with_analytic_primitives();
    let source = cases::stock(&k);
    let input = TaperedRibInput {
        entry: FacePick {
            point_m: [0.017, 0.014, 0.01],
            normal: [0., 0., 1.],
        },
        length_axis_hint: [1., 0., 0.],
        length_m: 0.006,
        base_thickness_m: 0.002,
        height_m: 0.003,
        angle_rad: 5_f64.to_radians(),
    };
    let body = k.add_tapered_rib_stock(source.clone(), &input).unwrap();
    let before = k.mass_properties(&body, 1.).unwrap();
    for q in [
        TessellationSettings {
            linear_deflection: 0.,
            angular_deflection_deg: 6.,
        },
        TessellationSettings {
            linear_deflection: 1e-12,
            angular_deflection_deg: 6.,
        },
        TessellationSettings {
            linear_deflection: 1e-6,
            angular_deflection_deg: 181.,
        },
    ] {
        assert!(k.tessellate(&body, &q).is_err());
    }
    assert!(k.boolean(body.clone(), source, BooleanOp::Union).is_err());
    assert!(k
        .cut_countersunk_stock(
            body.clone(),
            &CountersinkCut {
                entry: input.entry,
                bore_radius_m: 0.001,
                seat_radius_m: 0.002,
                bore_depth_m: 0.005,
                seat_depth_m: 0.002
            }
        )
        .is_err());
    let mut moved = body.clone();
    let mut refused = false;
    for _ in 0..20 {
        let near = RigidTransform {
            rotation: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1. + 2e-10]],
            translation_m: [0.; 3],
        };
        match k.transform_body(moved.clone(), near) {
            Ok(next) => moved = next,
            Err(_) => {
                refused = true;
                break;
            }
        }
    }
    assert!(refused);
    assert_eq!(k.mass_properties(&body, 1.).unwrap(), before);
}
