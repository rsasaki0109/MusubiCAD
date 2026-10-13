//! ADR-062 native integration: independent physical expectations, both kernels.
use std::f64::consts::PI;

use opencad_core::Length;
use opencad_geometry::{
    BooleanOp, ExtrudeExtent, ExtrudeOperation, GeometryKernel, KernelBody, MockGeometryKernel,
    RigidTransform, SketchPlacement, SolvedCircle, SolvedSketch,
};
use opencad_kernel_occt::OcctGeometryKernel;

fn rectangle(placement: SketchPlacement) -> SolvedSketch {
    SolvedSketch {
        profile_ref: "stock".into(),
        points: vec![[0., 0.], [0.04, 0.], [0.04, 0.03], [0., 0.03]],
        closed: true,
        circle: None,
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: Some(placement),
    }
}
fn circle(placement: SketchPlacement, radius_m: f64) -> SolvedSketch {
    SolvedSketch {
        circle: Some(SolvedCircle {
            center_m: [0.012, 0.009],
            radius_m,
        }),
        points: Vec::new(),
        profile_ref: "tool".into(),
        ..rectangle(placement)
    }
}
fn extrusion(
    kernel: &dyn GeometryKernel,
    profile: &SolvedSketch,
    length_m: f64,
    direction: [f64; 3],
) -> KernelBody {
    kernel
        .extrude(
            kernel.make_wire_from_sketch(profile).expect("wire"),
            ExtrudeExtent::Distance {
                length: Length::from_meters(length_m),
            },
            ExtrudeOperation::NewBody,
            None,
            direction,
        )
        .expect("extrusion")
}
fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual={actual}, expected={expected}, tolerance={tolerance}"
    );
}

#[test]
fn both_kernels_match_asymmetric_stepped_removal_area_first_moments_and_rigid_bounds() {
    let mock = MockGeometryKernel::with_analytic_primitives();
    let native = OcctGeometryKernel::new();
    // Independent SI integration: full bore plus annular top-seat increment.
    let bore_volume = PI * 0.0033_f64.powi(2) * 0.010;
    let seat_increment = PI * (0.0055_f64.powi(2) - 0.0033_f64.powi(2)) * 0.004;
    let removed = bore_volume + seat_increment;
    let stock_volume = 0.040 * 0.030 * 0.010;
    let remaining = stock_volume - removed;
    let center = [
        (stock_volume * 0.020 - removed * 0.012) / remaining,
        (stock_volume * 0.015 - removed * 0.009) / remaining,
        (stock_volume * 0.005 - bore_volume * 0.005 - seat_increment * 0.008) / remaining,
    ];
    let area = 2. * (0.040 * 0.030 + 0.040 * 0.010 + 0.030 * 0.010) - 2. * PI * 0.0033_f64.powi(2)
        + 2. * PI * 0.0033 * 0.006
        + 2. * PI * 0.0055 * 0.004;
    let placement = SketchPlacement {
        origin_m: [1., 2., 3.],
        x_axis_m: [0., 1., 0.],
        y_axis_m: [0., 0., 1.],
    };
    let rigid = RigidTransform {
        translation_m: [1., 2., 3.],
        rotation: [[0., 0., 1.], [1., 0., 0.], [0., 1., 0.]],
    };
    for kernel in [&mock as &dyn GeometryKernel, &native as &dyn GeometryKernel] {
        for placed in [false, true] {
            let plane = if placed {
                placement
            } else {
                SketchPlacement::global_xy()
            };
            let positive = if placed { [1., 0., 0.] } else { [0., 0., 1.] };
            let negative = positive.map(|v| -v);
            let mut top = plane;
            top.origin_m = if placed {
                [1.010, 2., 3.]
            } else {
                [0., 0., 0.010]
            };
            let stock = extrusion(kernel, &rectangle(plane), 0.010, positive);
            let bore = extrusion(kernel, &circle(top, 0.0033), 0.010, negative);
            let seat = extrusion(kernel, &circle(top, 0.0055), 0.004, negative);
            for (first, second) in [(&bore, &seat), (&seat, &bore)] {
                let intermediate = kernel
                    .boolean(stock.clone(), first.clone(), BooleanOp::Subtract)
                    .expect("first cut");
                let result = kernel
                    .body_boolean(intermediate, second.clone(), BooleanOp::Subtract)
                    .expect("second cut");
                let mass = kernel
                    .mass_properties(&result, 2700.)
                    .expect("physical mass");
                close(mass.volume_m3, remaining, 1e-12);
                close(mass.mass_kg, 2700. * remaining, 1e-9);
                close(mass.area_m2, area, 1e-10);
                let expected_center = if placed {
                    [1. + center[2], 2. + center[0], 3. + center[1]]
                } else {
                    center
                };
                for (actual, expected) in mass.center_of_mass.into_iter().zip(expected_center) {
                    close(actual, expected, 1e-9);
                }
                let bounds = kernel.bounding_box(&result).expect("physical bounds");
                let (minimum, maximum) = if placed {
                    ([1., 2., 3.], [1.010, 2.040, 3.030])
                } else {
                    ([0.; 3], [0.040, 0.030, 0.010])
                };
                for (actual, expected) in bounds.min.into_iter().zip(minimum) {
                    close(actual, expected, 2e-7);
                }
                for (actual, expected) in bounds.max.into_iter().zip(maximum) {
                    close(actual, expected, 2e-7);
                }
                if !placed {
                    let moved = kernel
                        .transform_body(result.clone(), rigid)
                        .expect("rigid placement");
                    let moved_mass = kernel
                        .mass_properties(&moved, 2700.)
                        .expect("moved physical mass");
                    close(moved_mass.volume_m3, remaining, 1e-12);
                    close(moved_mass.area_m2, area, 1e-10);
                    for (actual, expected) in moved_mass.center_of_mass.into_iter().zip([
                        1. + center[2],
                        2. + center[0],
                        3. + center[1],
                    ]) {
                        close(actual, expected, 1e-9);
                    }
                    let moved_bounds = kernel.bounding_box(&moved).expect("moved bounds");
                    for (actual, expected) in moved_bounds.min.into_iter().zip([1., 2., 3.]) {
                        close(actual, expected, 2e-7);
                    }
                    for (actual, expected) in
                        moved_bounds.max.into_iter().zip([1.010, 2.040, 3.030])
                    {
                        close(actual, expected, 2e-7);
                    }
                }
                // Inputs remain separate immutable kernel records.
                close(
                    kernel
                        .mass_properties(&stock, 1.)
                        .expect("original stock")
                        .volume_m3,
                    stock_volume,
                    1e-12,
                );
                close(
                    kernel
                        .mass_properties(&bore, 1.)
                        .expect("original tool")
                        .volume_m3,
                    bore_volume,
                    1e-12,
                );
            }
        }
    }
}

#[test]
fn both_kernels_match_blind_cut_and_symmetric_cylinder_measures() {
    let mock = MockGeometryKernel::with_analytic_primitives();
    let native = OcctGeometryKernel::new();
    for kernel in [&mock as &dyn GeometryKernel, &native as &dyn GeometryKernel] {
        let stock = extrusion(
            kernel,
            &rectangle(SketchPlacement::global_xy()),
            0.010,
            [0., 0., 1.],
        );
        let top = SketchPlacement {
            origin_m: [0., 0., 0.010],
            ..SketchPlacement::global_xy()
        };
        let wire = kernel
            .make_wire_from_sketch(&circle(top, 0.0033))
            .expect("circle");
        let cut = kernel
            .extrude(
                wire,
                ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                ExtrudeOperation::Cut,
                Some(stock),
                [0., 0., -2.],
            )
            .expect("blind cut");
        let removed = PI * 0.0033_f64.powi(2) * 0.006;
        let volume = 0.000012 - removed;
        let mass = kernel.mass_properties(&cut, 1.).expect("blind mass");
        close(mass.volume_m3, volume, 1e-12);
        close(mass.area_m2, 0.0038 + 2. * PI * 0.0033 * 0.006, 1e-10);
        close(
            mass.center_of_mass[2],
            (0.000012 * 0.005 - removed * 0.007) / volume,
            1e-9,
        );
        let middle = SketchPlacement {
            origin_m: [0., 0., 0.005],
            ..SketchPlacement::global_xy()
        };
        let symmetric_target = extrusion(
            kernel,
            &rectangle(SketchPlacement::global_xy()),
            0.010,
            [0., 0., 1.],
        );
        let symmetric_cut = kernel
            .extrude(
                kernel
                    .make_wire_from_sketch(&circle(middle, 0.0033))
                    .expect("mid-plane profile"),
                ExtrudeExtent::Symmetric {
                    length: Length::from_meters(0.005),
                },
                ExtrudeOperation::Cut,
                Some(symmetric_target),
                [0., 0., -1.],
            )
            .expect("symmetric cut centered before subtraction");
        let symmetric_mass = kernel
            .mass_properties(&symmetric_cut, 1.)
            .expect("symmetric cut mass");
        close(
            symmetric_mass.volume_m3,
            0.000012 - PI * 0.0033_f64.powi(2) * 0.010,
            1e-12,
        );
        close(
            symmetric_mass.area_m2,
            0.0038 - 2. * PI * 0.0033_f64.powi(2) + 2. * PI * 0.0033 * 0.010,
            1e-10,
        );
        close(symmetric_mass.center_of_mass[2], 0.005, 1e-9);
        let profile = circle(SketchPlacement::global_xy(), 0.0033);
        let cylinder = kernel
            .extrude(
                kernel.make_wire_from_sketch(&profile).expect("wire"),
                ExtrudeExtent::Symmetric {
                    length: Length::from_meters(0.005),
                },
                ExtrudeOperation::NewBody,
                None,
                [0., 0., -1.],
            )
            .expect("symmetric cylinder");
        let mass = kernel
            .mass_properties(&cylinder, 1000.)
            .expect("cylinder mass");
        close(mass.volume_m3, PI * 0.0033_f64.powi(2) * 0.010, 1e-12);
        close(mass.area_m2, 2. * PI * 0.0033 * (0.010 + 0.0033), 1e-10);
        for (actual, expected) in mass.center_of_mass.into_iter().zip([0.012, 0.009, 0.]) {
            close(actual, expected, 1e-9);
        }
        let bounds = kernel.bounding_box(&cylinder).expect("cylinder bounds");
        for (actual, expected) in bounds.min.into_iter().zip([0.0087, 0.0057, -0.005]) {
            close(actual, expected, 2e-7);
        }
        for (actual, expected) in bounds.max.into_iter().zip([0.0153, 0.0123, 0.005]) {
            close(actual, expected, 2e-7);
        }
    }
}

#[test]
fn both_kernels_center_symmetric_rectangles_and_bound_tilted_cylinders() {
    let mock = MockGeometryKernel::with_analytic_primitives();
    let native = OcctGeometryKernel::new();
    let placement = SketchPlacement {
        origin_m: [0.1, 0.2, 0.3],
        x_axis_m: [1., 0., 0.],
        y_axis_m: [0., 0.8, -0.6],
    };
    for kernel in [&mock as &dyn GeometryKernel, &native as &dyn GeometryKernel] {
        for (profile, center, minimum, maximum) in [
            (
                rectangle(placement),
                [0.120, 0.212, 0.291],
                [0.100, 0.197, 0.278],
                [0.140, 0.227, 0.304],
            ),
            (
                circle(placement, 0.0033),
                [0.112, 0.2072, 0.2946],
                [0.1087, 0.20156, 0.28862],
                [0.1153, 0.21284, 0.30058],
            ),
        ] {
            let body = kernel
                .extrude(
                    kernel
                        .make_wire_from_sketch(&profile)
                        .expect("tilted profile"),
                    ExtrudeExtent::Symmetric {
                        length: Length::from_meters(0.005),
                    },
                    ExtrudeOperation::NewBody,
                    None,
                    [0., 3., 4.],
                )
                .expect("centered symmetric primitive");
            let mass = kernel.mass_properties(&body, 1.).expect("tilted mass");
            let expected_volume = if profile.circle.is_some() {
                PI * 0.0033_f64.powi(2) * 0.010
            } else {
                0.000012
            };
            close(mass.volume_m3, expected_volume, 1e-12);
            for (actual, expected) in mass.center_of_mass.into_iter().zip(center) {
                close(actual, expected, 1e-9);
            }
            let bounds = kernel.bounding_box(&body).expect("true tilted bounds");
            for (actual, expected) in bounds.min.into_iter().zip(minimum) {
                close(actual, expected, 2e-7);
            }
            for (actual, expected) in bounds.max.into_iter().zip(maximum) {
                close(actual, expected, 2e-7);
            }
        }
    }
}
