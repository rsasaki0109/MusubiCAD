#[path = "support/nominal_thread_case.rs"]
mod support;
use opencad_geometry::{
    BooleanOp, GeometryKernel, KernelBody, MetricThreadSize, MockGeometryKernel,
    NominalMetricThread, NominalThreadInput, RigidTransform, TessellationSettings,
    ThreadHandedness,
};

#[test]
fn analytic_thread_has_independent_physical_mass_phase_bounds_and_closed_mesh() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    for size in [
        MetricThreadSize::M3,
        MetricThreadSize::M4,
        MetricThreadSize::M5,
        MetricThreadSize::M6,
        MetricThreadSize::M8,
        MetricThreadSize::M10,
        MetricThreadSize::M12,
    ] {
        for hand in [ThreadHandedness::Right, ThreadHandedness::Left] {
            for turns in [1, 8, 32] {
                support::case(&kernel, size, hand, turns, false, false, false);
            }
        }
    }
}
#[test]
fn analytic_thread_placement_axis_reversal_and_output_transform_remain_physical() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    for hand in [ThreadHandedness::Right, ThreadHandedness::Left] {
        for reverse in [false, true] {
            let (body, _) =
                support::case(&kernel, MetricThreadSize::M6, hand, 8, true, reverse, false);
            let before = kernel.mass_properties(&body, 1.).unwrap();
            let moved = kernel
                .transform_body(
                    body.clone(),
                    RigidTransform {
                        translation_m: [0.04, 0.03, -0.02],
                        rotation: [[0., 0., 1.], [0., 1., 0.], [-1., 0., 0.]],
                    },
                )
                .unwrap();
            let after = kernel.mass_properties(&moved, 1.).unwrap();
            support::near(after.volume_m3, before.volume_m3, 1e-18);
            for (i, shift) in [0.04, 0.03, -0.02].into_iter().enumerate() {
                support::near(
                    after.center_of_mass[i],
                    [
                        before.center_of_mass[2],
                        before.center_of_mass[1],
                        -before.center_of_mass[0],
                    ][i] + shift,
                    1e-12,
                );
            }
            let mesh = kernel
                .tessellate(&body, &TessellationSettings::default())
                .unwrap();
            assert_eq!(
                mesh,
                kernel
                    .tessellate(&body, &TessellationSettings::default())
                    .unwrap()
            );
        }
    }
}
#[test]
fn analytic_thread_invalid_stock_frame_quality_and_downstream_edits_refuse() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let size = MetricThreadSize::M6;
    let source = support::stock(&kernel, size, 8, support::placement(false)).unwrap();
    let input = NominalThreadInput {
        profile: NominalMetricThread::new(size, ThreadHandedness::Right, 8).unwrap(),
        axis_hint: [0., 0., 1.],
        radial_axis: [1., 0., 0.],
    };
    let before = kernel.mass_properties(&source, 1.).unwrap();
    for radial in [
        [0., 0., 0.],
        [0., 0., 1.],
        [1.01, 0., 0.],
        [f64::NAN, 0., 0.],
    ] {
        assert!(kernel
            .cut_nominal_metric_thread(
                source.clone(),
                &NominalThreadInput {
                    radial_axis: radial,
                    ..input
                }
            )
            .is_err());
    }
    assert!(kernel
        .cut_nominal_metric_thread(KernelBody(u64::MAX / 7), &input)
        .is_err());
    assert!(kernel
        .cut_nominal_metric_thread(
            source.clone(),
            &NominalThreadInput {
                profile: NominalMetricThread::new(size, ThreadHandedness::Right, 7).unwrap(),
                ..input
            }
        )
        .is_err());
    assert!(MockGeometryKernel::default()
        .cut_nominal_metric_thread(KernelBody(1), &input)
        .is_err());
    let body = kernel
        .cut_nominal_metric_thread(source.clone(), &input)
        .unwrap();
    for quality in [
        TessellationSettings {
            linear_deflection: f64::NAN,
            angular_deflection_deg: 12.,
        },
        TessellationSettings {
            linear_deflection: 1e-12,
            angular_deflection_deg: 12.,
        },
        TessellationSettings {
            linear_deflection: 0.001,
            angular_deflection_deg: 0.01,
        },
    ] {
        assert!(kernel.tessellate(&body, &quality).is_err());
    }
    assert!(kernel
        .boolean(body.clone(), source.clone(), BooleanOp::Union)
        .is_err());
    assert!(kernel
        .cylindrical_stock_geometry(&body, [0., 0., 1.])
        .is_err());
    assert!(kernel
        .transform_body(
            body,
            RigidTransform {
                translation_m: [0.; 3],
                rotation: [[1., 0., 0.], [0., -1., 0.], [0., 0., 1.]]
            }
        )
        .is_err());
    assert_eq!(kernel.mass_properties(&source, 1.).unwrap(), before);
}

#[test]
fn finer_thread_mesh_converges_to_the_independently_graded_ideal_volume() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let (body, _) = support::case(
        &kernel,
        MetricThreadSize::M6,
        ThreadHandedness::Right,
        1,
        false,
        false,
        false,
    );
    let ideal = kernel.mass_properties(&body, 1.).unwrap().volume_m3;
    let volume = |mesh: &opencad_geometry::MeshSet| {
        mesh.indices
            .chunks_exact(3)
            .map(|tri| {
                let [a, b, c] =
                    std::array::from_fn(|i| mesh.positions[tri[i] as usize].map(f64::from));
                (a[0] * (b[1] * c[2] - b[2] * c[1])
                    + a[1] * (b[2] * c[0] - b[0] * c[2])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]))
                    / 6.
            })
            .sum::<f64>()
    };
    let coarse = kernel
        .tessellate(&body, &TessellationSettings::default())
        .unwrap();
    let fine = kernel
        .tessellate(
            &body,
            &TessellationSettings {
                linear_deflection: 1e-5,
                angular_deflection_deg: 12.,
            },
        )
        .unwrap();
    assert!(fine.triangle_count() > coarse.triangle_count());
    assert!((volume(&fine) - ideal).abs() < (volume(&coarse) - ideal).abs());
}
