//! Native integration of ADR-068; grades real output against independent oracles.
#![cfg(feature = "occt")]
#[path = "../../geometry/tests/support/nominal_thread_case.rs"]
mod support;
use opencad_geometry::TessellationSettings;
use opencad_geometry::{
    GeometryKernel, KernelBody, MetricThreadSize, NominalMetricThread, NominalThreadInput,
    RigidTransform, ThreadHandedness,
};
use opencad_kernel_occt::OcctGeometryKernel;

#[test]
fn native_thread_uses_real_nominal_geometry_and_square_ends() {
    let kernel = OcctGeometryKernel::new();
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
            let counts: &[u32] = if matches!(
                size,
                MetricThreadSize::M3 | MetricThreadSize::M6 | MetricThreadSize::M12
            ) {
                &[1, 8, 32]
            } else {
                &[1]
            };
            for &turns in counts {
                support::case(&kernel, size, hand, turns, false, false, true);
            }
        }
    }
}
#[test]
fn native_thread_phase_frame_reversal_and_rigid_placement_are_explicit() {
    let kernel = OcctGeometryKernel::new();
    for hand in [ThreadHandedness::Right, ThreadHandedness::Left] {
        for reverse in [false, true] {
            let (body, _) =
                support::case(&kernel, MetricThreadSize::M6, hand, 8, true, reverse, true);
            let before = kernel.mass_properties(&body, 1.).unwrap();
            let moved = kernel
                .transform_body(
                    body,
                    RigidTransform {
                        translation_m: [0.02, 0.03, 0.04],
                        rotation: [[0., 0., 1.], [0., 1., 0.], [-1., 0., 0.]],
                    },
                )
                .unwrap();
            let after = kernel.mass_properties(&moved, 1.).unwrap();
            support::near(after.volume_m3, before.volume_m3, before.volume_m3 * 1e-6);
            for (i, shift) in [0.02, 0.03, 0.04].into_iter().enumerate() {
                support::near(
                    after.center_of_mass[i],
                    [
                        before.center_of_mass[2],
                        before.center_of_mass[1],
                        -before.center_of_mass[0],
                    ][i] + shift,
                    2e-8,
                );
            }
            kernel.bounding_box(&moved).unwrap();
        }
    }
}
#[test]
fn native_thread_rejects_invalid_inputs_without_mutating_stock() {
    let kernel = OcctGeometryKernel::new();
    let size = MetricThreadSize::M6;
    let body = support::stock(&kernel, size, 8, support::placement(false)).unwrap();
    let before = kernel.mass_properties(&body, 1.).unwrap();
    let input = NominalThreadInput {
        profile: NominalMetricThread::new(size, ThreadHandedness::Right, 8).unwrap(),
        axis_hint: [0., 0., 1.],
        radial_axis: [1., 0., 0.],
    };
    for axis in [
        [0., 0., 0.],
        [0., 0., 1.01],
        [1., 0., 0.],
        [f64::NAN, 0., 1.],
    ] {
        assert!(kernel
            .cut_nominal_metric_thread(
                body.clone(),
                &NominalThreadInput {
                    axis_hint: axis,
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
            body.clone(),
            &NominalThreadInput {
                profile: NominalMetricThread::new(MetricThreadSize::M8, ThreadHandedness::Right, 8)
                    .unwrap(),
                ..input
            }
        )
        .is_err());
    let thread = kernel
        .cut_nominal_metric_thread(body.clone(), &input)
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
        assert!(kernel.tessellate(&thread, &quality).is_err());
    }
    assert!(kernel.mass_properties(&thread, f64::NAN).is_err());
    assert!(kernel
        .transform_body(
            thread.clone(),
            RigidTransform {
                translation_m: [0.; 3],
                rotation: [[1., 0., 0.], [0., -1., 0.], [0., 0., 1.]]
            }
        )
        .is_err());
    assert!(kernel
        .rotate_body(thread.clone(), [0.; 3], [0., 0., 1.], 0.4)
        .is_err());
    assert!(kernel.mirror_body(thread, [0.; 3], [0., 1., 0.]).is_err());
    assert_eq!(kernel.mass_properties(&body, 1.).unwrap(), before);
}
