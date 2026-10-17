//! ADR-105 native integration: oriented display for existing ellipse prisms/copies.
#![cfg(feature = "occt")]
use opencad_core::Length;
use opencad_geometry::{
    ExtrudeExtent, ExtrudeOperation, GeometryKernel, KernelBody, RigidTransform, SketchPlacement,
    SolvedEllipse, SolvedSketch, TessellationSettings,
};
use opencad_kernel_occt::OcctGeometryKernel;
#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod evidence;
fn body(kernel: &OcctGeometryKernel) -> KernelBody {
    let profile = SolvedSketch {
        profile_ref: "ellipse".into(),
        points: Vec::new(),
        closed: true,
        circle: None,
        ellipse: Some(SolvedEllipse {
            center_m: [0.002, -0.003],
            major_radius_m: 0.03,
            minor_radius_m: 0.02,
            axis_angle_rad: 3_f64.atan2(4.),
        }),
        cubic_bezier: None,
        segments: Vec::new(),
        placement: Some(SketchPlacement::global_xy()),
    };
    kernel
        .extrude(
            kernel.make_wire_from_sketch(&profile).unwrap(),
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.006),
            },
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.],
        )
        .unwrap()
}
fn check(kernel: &OcctGeometryKernel, body: &KernelBody, volume: f64, center: [f64; 3]) {
    let mass = kernel.mass_properties(body, 1.).unwrap();
    assert!((mass.volume_m3 - volume).abs() < 1e-12);
    let mesh = kernel
        .tessellate(
            body,
            &TessellationSettings {
                linear_deflection: 1e-5,
                angular_deflection_deg: 4.,
            },
        )
        .unwrap();
    let (actual, c) = evidence::integrated(&mesh, true);
    assert!((actual / volume - 1.).abs() < 0.01);
    for i in 0..3 {
        assert!((c[i] - center[i]).abs() < 1e-5);
        assert!((mass.center_of_mass[i] - center[i]).abs() < 3e-7);
    }
}
#[test]
fn native_ellipse_normals_survive_translation_rigid_transform_and_compound_members() {
    let kernel = OcctGeometryKernel::new();
    let original = body(&kernel);
    let volume = 3600. * std::f64::consts::PI * 1e-9;
    let translated = kernel
        .translate_body(original.clone(), [0.1, 0.2, 0.3])
        .unwrap();
    let rotated = kernel
        .transform_body(
            original.clone(),
            RigidTransform {
                rotation: [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
                translation_m: [0.01, -0.02, 0.03],
            },
        )
        .unwrap();
    check(&kernel, &original, volume, [0.002, -0.003, 0.003]);
    check(&kernel, &translated, volume, [0.102, 0.197, 0.303]);
    check(&kernel, &rotated, volume, [0.013, -0.018, 0.033]);
    let compound = kernel
        .make_compound(&[original.clone(), translated])
        .unwrap();
    check(&kernel, &compound, 2. * volume, [0.052, 0.097, 0.153]);
    let shifted = kernel
        .transform_body(compound, RigidTransform::from_translation([0.01, 0., 0.]))
        .unwrap();
    check(&kernel, &shifted, 2. * volume, [0.062, 0.097, 0.153]);
    check(&kernel, &original, volume, [0.002, -0.003, 0.003]);
}
#[test]
fn unsupported_ellipse_display_placements_refuse_without_changing_source_body() {
    let kernel = OcctGeometryKernel::new();
    let original = body(&kernel);
    for rotation in [
        [[-1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        [[1., 0.1, 0.], [0., 1., 0.], [0., 0., 1.]],
    ] {
        assert!(kernel
            .transform_body(
                original.clone(),
                RigidTransform {
                    rotation,
                    translation_m: [0.; 3]
                }
            )
            .is_err());
    }
    assert!(kernel
        .translate_body(original.clone(), [f64::NAN, 0., 0.])
        .is_err());
    check(
        &kernel,
        &original,
        3600. * std::f64::consts::PI * 1e-9,
        [0.002, -0.003, 0.003],
    );
}
