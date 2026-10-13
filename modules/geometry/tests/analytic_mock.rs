use std::f64::consts::PI;

use opencad_core::{Length, TopoRefId};
use opencad_geometry::{
    BooleanOp, ExtrudeExtent, ExtrudeOperation, FilletEdgeSelector, GeometryKernel, KernelBody,
    KernelWire, MockGeometryKernel, ProfileSegment, RigidTransform, SectionPlane, SketchPlacement,
    SolvedCircle, SolvedSketch, TessellationSettings,
};

fn rectangle() -> SolvedSketch {
    SolvedSketch {
        profile_ref: "stock".into(),
        points: vec![[0., 0.], [0.04, 0.], [0.04, 0.03], [0., 0.03]],
        closed: true,
        circle: None,
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: None,
    }
}
fn circle(center: [f64; 2], radius: f64, z: f64) -> SolvedSketch {
    SolvedSketch {
        profile_ref: "tool".into(),
        points: Vec::new(),
        circle: Some(SolvedCircle {
            center_m: center,
            radius_m: radius,
        }),
        placement: Some(SketchPlacement {
            origin_m: [0., 0., z],
            ..SketchPlacement::global_xy()
        }),
        ..rectangle()
    }
}
fn distance(length: f64) -> ExtrudeExtent {
    ExtrudeExtent::Distance {
        length: Length::from_meters(length),
    }
}
fn extruded(
    kernel: &MockGeometryKernel,
    sketch: &SolvedSketch,
    length: f64,
    direction: [f64; 3],
) -> KernelBody {
    kernel
        .extrude(
            kernel.make_wire_from_sketch(sketch).expect("profile"),
            distance(length),
            ExtrudeOperation::NewBody,
            None,
            direction,
        )
        .expect("primitive")
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-12,
        "actual={actual}, expected={expected}"
    );
}

#[test]
fn physical_mode_is_explicit_and_never_promotes_unknown_or_legacy_records() {
    let legacy = MockGeometryKernel::new();
    let physical = MockGeometryKernel::with_analytic_primitives();
    let old = extruded(&legacy, &rectangle(), 0.010, [0., 0., 1.]);
    close(
        legacy.mass_properties(&old, 1.).expect("legacy").volume_m3,
        0.014_f64.powi(3),
    );
    assert!(!legacy.has_exact_rigid_placement(&old));
    let new = extruded(&physical, &rectangle(), 0.010, [0., 0., 1.]);
    assert!(physical.has_exact_rigid_placement(&new));
    let mass = physical.mass_properties(&new, 2700.).expect("physical");
    close(mass.volume_m3, 0.000012);
    close(mass.mass_kg, 0.0324);
    close(mass.area_m2, 0.0038);
    for (a, e) in mass.center_of_mass.into_iter().zip([0.020, 0.015, 0.005]) {
        close(a, e);
    }
    for unknown in [
        old,
        KernelBody::new(0),
        KernelBody::new(u64::MAX),
        KernelBody::new(u64::MAX / 4),
    ] {
        assert!(!physical.has_exact_rigid_placement(&unknown));
        assert!(physical.mass_properties(&unknown, 1.).is_err());
        assert!(physical.bounding_box(&unknown).is_err());
        assert!(physical
            .transform_body(unknown, RigidTransform::identity())
            .is_err());
    }
    assert!(physical
        .extrude(
            KernelWire::new(4),
            distance(0.010),
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.]
        )
        .is_err());
}

#[test]
fn enclosed_concentric_cavities_have_true_caps_and_refusals_preserve_stock() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let stock = extruded(&kernel, &rectangle(), 0.010, [0., 0., 1.]);
    let cylinder = extruded(
        &kernel,
        &circle([0.012, 0.009], 0.003, 0.002),
        0.004,
        [0., 0., 1.],
    );
    let result = kernel
        .boolean(stock.clone(), cylinder, BooleanOp::Subtract)
        .expect("bounded cavity");
    let original = kernel.mass_properties(&result, 1.).expect("mass");
    let removed = PI * 0.003_f64.powi(2) * 0.004;
    close(original.volume_m3, 0.000012 - removed);
    close(
        original.area_m2,
        0.0038 + 2. * PI * 0.003 * 0.004 + 2. * PI * 0.003_f64.powi(2),
    );
    close(
        original.center_of_mass[2],
        (0.000012 * 0.005 - removed * 0.004) / (0.000012 - removed),
    );
    for (sketch, length, direction) in [
        (circle([0.013, 0.009], 0.004, 0.010), 0.004, [0., 0., -1.]),
        (circle([0.003, 0.009], 0.003, 0.010), 0.004, [0., 0., -1.]),
        (circle([0.002, 0.009], 0.003, 0.010), 0.004, [0., 0., -1.]),
        (circle([0.012, 0.009], 0.004, 0.011), 0.004, [0., 0., -1.]),
        (circle([0.012, 0.009], 0.004, 0.010), 0.011, [0., 0., -1.]),
        (circle([0.012, 0.009], 0.004, 0.010), 0.004, [0.1, 0., -1.]),
        (
            circle([0.012, 0.009], 0.003 + 5e-10, 0.010),
            0.004,
            [0., 0., -1.],
        ),
        (
            circle([0.012, 0.009], 0.004, 0.006 + 5e-10),
            0.004,
            [0., 0., -1.],
        ),
    ] {
        let candidate = kernel
            .make_wire_from_sketch(&sketch)
            .expect("valid profile");
        assert!(kernel
            .extrude(
                candidate,
                distance(length),
                ExtrudeOperation::Cut,
                Some(result.clone()),
                direction
            )
            .is_err());
        assert_eq!(
            kernel.mass_properties(&result, 1.).expect("unchanged"),
            original
        );
        close(
            kernel
                .mass_properties(&stock, 1.)
                .expect("source")
                .volume_m3,
            0.000012,
        );
    }
}

#[test]
fn unsupported_physical_queries_and_edits_never_return_synthetic_geometry() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let stock = extruded(&kernel, &rectangle(), 0.010, [0., 0., 1.]);
    let original = kernel.mass_properties(&stock, 1.).expect("original");
    assert!(kernel
        .tessellate(&stock, &TessellationSettings::default())
        .is_err());
    assert!(kernel.discover_body_edges(&stock).is_err());
    assert!(kernel.discover_body_edges(&KernelBody::new(0)).is_err());
    assert!(kernel.edge_endpoints(&stock, 1).is_err());
    assert!(kernel.straight_edge_endpoints(&stock, 1).is_err());
    assert!(kernel.circular_edge_geometry(&stock, 5).is_err());
    assert!(kernel
        .assign_face_ref(&stock, 1, TopoRefId::new("ref:face").expect("reference ID"))
        .is_err());
    assert!(kernel.inertia_about_com(&stock, 1.).is_err());
    assert!(kernel.export_step(&stock).is_err());
    assert!(kernel.import_step(b"synthetic").is_err());
    assert!(kernel.intersection_volume(&stock, &stock).is_err());
    assert!(kernel
        .boolean(stock.clone(), stock.clone(), BooleanOp::Union)
        .is_err());
    assert!(kernel
        .boolean(stock.clone(), stock.clone(), BooleanOp::Subtract)
        .is_err());
    assert!(kernel.make_compound(std::slice::from_ref(&stock)).is_err());
    assert!(kernel
        .fillet_edges(stock.clone(), 0.001, FilletEdgeSelector::All)
        .is_err());
    assert!(kernel
        .chamfer_edges(stock.clone(), 0.001, FilletEdgeSelector::All)
        .is_err());
    assert!(kernel
        .variable_fillet_edge(
            stock.clone(),
            0.001,
            0.002,
            FilletEdgeSelector::KernelEdges {
                kernel_edge_ids: vec![1]
            }
        )
        .is_err());
    assert!(kernel
        .rotate_body(stock.clone(), [0.; 3], [0., 0., 1.], 0.5)
        .is_err());
    assert!(kernel
        .mirror_body(stock.clone(), [0.; 3], [1., 0., 0.])
        .is_err());
    let plane = SectionPlane {
        origin_m: [0., 0., 0.005],
        normal: [0., 0., 1.],
    };
    assert!(kernel.section_body(&stock, &plane).is_err());
    assert!(kernel.sweep(&rectangle(), &rectangle()).is_err());
    assert!(kernel.loft(&[rectangle(), rectangle()]).is_err());
    assert_eq!(
        kernel.mass_properties(&stock, 1.).expect("unchanged"),
        original
    );
}

#[test]
fn rejects_invalid_profiles_extents_density_and_nonrigid_transforms_atomically() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let mut candidates = vec![rectangle(); 7];
    candidates[0].closed = false;
    candidates[1].points[2] = [0.030, 0.030];
    candidates[2].points[0][0] = f64::NAN;
    candidates[3].points[1][0] = 5e-10;
    candidates[4].placement = Some(SketchPlacement {
        x_axis_m: [2., 0., 0.],
        ..SketchPlacement::global_xy()
    });
    candidates[5].placement = Some(SketchPlacement {
        y_axis_m: [1., 0., 0.],
        ..SketchPlacement::global_xy()
    });
    candidates[6].segments = vec![
        ProfileSegment::Line {
            start_m: [0., 0.],
            end_m: [0.01, 0.]
        };
        4
    ];
    for sketch in candidates {
        assert!(kernel.make_wire_from_sketch(&sketch).is_err());
    }
    for radius in [0., -1., f64::NAN, f64::INFINITY, 5e-10, 1001.] {
        assert!(kernel
            .make_wire_from_sketch(&circle([0.012, 0.009], radius, 0.))
            .is_err());
    }
    let profile = kernel.make_wire_from_sketch(&rectangle()).expect("wire");
    for length in [0., -1., f64::NAN, f64::INFINITY, 5e-10, 1001.] {
        assert!(kernel
            .extrude(
                profile.clone(),
                distance(length),
                ExtrudeOperation::NewBody,
                None,
                [0., 0., 1.]
            )
            .is_err());
    }
    assert!(kernel
        .extrude(
            profile.clone(),
            ExtrudeExtent::ThroughAll,
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.]
        )
        .is_err());
    for direction in [[0.; 3], [f64::NAN, 0., 1.], [0.1, 0., 1.], [1e-6, 0., 1.]] {
        assert!(kernel
            .extrude(
                profile.clone(),
                distance(0.010),
                ExtrudeOperation::NewBody,
                None,
                direction
            )
            .is_err());
    }
    let stock = kernel
        .extrude(
            profile,
            distance(0.010),
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.],
        )
        .expect("stock after refusals");
    let original = kernel.mass_properties(&stock, 1.).expect("original");
    for density in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(kernel.mass_properties(&stock, density).is_err());
    }
    for transform in [
        RigidTransform {
            translation_m: [f64::NAN, 0., 0.],
            ..RigidTransform::identity()
        },
        RigidTransform {
            rotation: [[-1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            ..RigidTransform::identity()
        },
        RigidTransform {
            rotation: [[2., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            ..RigidTransform::identity()
        },
    ] {
        assert!(kernel.transform_body(stock.clone(), transform).is_err());
        assert_eq!(
            kernel.mass_properties(&stock, 1.).expect("unchanged"),
            original
        );
    }
    let moved = kernel
        .translate_body(stock.clone(), [-0.020, 0.010, -0.030])
        .expect("signed translation");
    for (actual, expected) in kernel
        .mass_properties(&moved, 1.)
        .expect("moved")
        .center_of_mass
        .into_iter()
        .zip([0., 0.025, -0.025])
    {
        close(actual, expected);
    }
}

#[test]
fn repeated_cut_is_idempotent_and_cut_budget_failure_preserves_last_body() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let stock = extruded(&kernel, &rectangle(), 0.010, [0., 0., 1.]);
    let tool = extruded(
        &kernel,
        &circle([0.012, 0.009], 0.003, 0.010),
        0.010,
        [0., 0., -1.],
    );
    let mut current = stock;
    for _ in 0..64 {
        current = kernel
            .boolean(current, tool.clone(), BooleanOp::Subtract)
            .expect("within budget");
    }
    let mass = kernel.mass_properties(&current, 1.).expect("last admitted");
    close(mass.volume_m3, 0.000012 - PI * 0.003_f64.powi(2) * 0.010);
    assert!(kernel
        .boolean(current.clone(), tool, BooleanOp::Subtract)
        .is_err());
    assert_eq!(
        kernel.mass_properties(&current, 1.).expect("unchanged"),
        mass
    );
}
