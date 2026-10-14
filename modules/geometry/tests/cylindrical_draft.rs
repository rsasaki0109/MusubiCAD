use opencad_geometry::*;
#[path = "support/cylindrical_draft_cases.rs"]
mod cases;
#[test]
fn analytic_cylinder_witness_and_neutral_draft_are_physical() {
    cases::physical(&MockGeometryKernel::with_analytic_primitives(), true);
}
#[test]
fn invalid_draft_preserves_source_and_default_mock_refuses() {
    cases::refusals(&MockGeometryKernel::with_analytic_primitives());
    let legacy = MockGeometryKernel::new();
    assert!(legacy
        .cylindrical_stock_geometry(&KernelBody::new(1), [0., 0., 1.])
        .is_err());
}
#[test]
fn frustum_mesh_quality_and_generic_edit_refusal_remain_bounded() {
    let k = MockGeometryKernel::with_analytic_primitives();
    let stock = cases::stock(&k);
    let input = CylindricalDraftInput {
        axis_hint: [0., 0., 1.],
        neutral_origin_m: [0., 0., 0.],
        neutral_normal: [0., 0., 1.],
        angle_rad: 5_f64.to_radians(),
    };
    let out = k.draft_cylindrical_stock(stock.clone(), &input).unwrap();
    for q in [
        TessellationSettings {
            linear_deflection: 1e-20,
            angular_deflection_deg: 6.,
        },
        TessellationSettings {
            linear_deflection: 1e-6,
            angular_deflection_deg: 181.,
        },
        TessellationSettings {
            linear_deflection: 1e-6,
            angular_deflection_deg: 0.001,
        },
    ] {
        assert!(k.tessellate(&out, &q).is_err());
    }
    assert!(k.boolean(out.clone(), stock, BooleanOp::Subtract).is_err());
    let original = k.mass_properties(&out, 1.).unwrap();
    let drift = RigidTransform {
        rotation: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1. + 2e-10]],
        translation_m: [0.; 3],
    };
    let mut last = out.clone();
    let mut rejected = false;
    for _ in 0..10 {
        match k.transform_body(last.clone(), drift) {
            Ok(next) => last = next,
            Err(_) => {
                rejected = true;
                break;
            }
        }
    }
    assert!(
        rejected,
        "accumulated non-rigid drift must refuse before publication"
    );
    assert_eq!(original, k.mass_properties(&out, 1.).unwrap());
}
