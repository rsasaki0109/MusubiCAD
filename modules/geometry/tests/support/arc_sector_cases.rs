use opencad_core::Length;
use opencad_geometry::*;
use std::f64::consts::PI;
#[path = "mesh_evidence.rs"]
mod evidence;

pub fn profile(sweep: f64, reverse: bool) -> SolvedSketch {
    let c = [0.002, -0.003];
    let at = |a: f64| [c[0] + 0.01 * a.cos(), c[1] + 0.01 * a.sin()];
    let s = at(0.);
    let m = at(sweep / 2.);
    let e = at(sweep);
    let mut segments = vec![
        ProfileSegment::Arc {
            start_m: s,
            mid_m: m,
            end_m: e,
        },
        ProfileSegment::Line {
            start_m: e,
            end_m: c,
        },
        ProfileSegment::Line {
            start_m: c,
            end_m: s,
        },
    ];
    if reverse {
        segments = segments
            .into_iter()
            .rev()
            .map(|s| match s {
                ProfileSegment::Line { start_m, end_m } => ProfileSegment::Line {
                    start_m: end_m,
                    end_m: start_m,
                },
                ProfileSegment::Arc {
                    start_m,
                    mid_m,
                    end_m,
                } => ProfileSegment::Arc {
                    start_m: end_m,
                    mid_m,
                    end_m: start_m,
                },
            })
            .collect();
    }
    SolvedSketch {
        profile_ref: "sector".into(),
        points: Vec::new(),
        closed: true,
        circle: None,
        ellipse: None,
        cubic_bezier: None,
        segments,
        placement: None,
    }
}
pub fn body(
    k: &dyn GeometryKernel,
    p: &SolvedSketch,
    extent: ExtrudeExtent,
    direction: [f64; 3],
) -> KernelBody {
    k.extrude(
        k.make_wire_from_sketch(p).unwrap(),
        extent,
        ExtrudeOperation::NewBody,
        None,
        direction,
    )
    .unwrap()
}
fn depth() -> ExtrudeExtent {
    ExtrudeExtent::Distance {
        length: Length::from_meters(0.006),
    }
}
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} != {b}, tolerance {t}");
}
fn near(a: [f64; 3], b: [f64; 3]) {
    for i in 0..3 {
        close(a[i], b[i], 3e-7);
    }
}

pub fn physical(k: &dyn GeometryKernel) {
    let d = 0.04 / (3. * PI);
    for (theta, volume, area, center, low, high) in [
        (
            PI / 2.,
            150. * PI * 1e-9,
            (80. * PI + 120.) * 1e-6,
            [0.002 + d, -0.003 + d, 0.003],
            [0.002, -0.003, 0.],
            [0.012, 0.007, 0.006],
        ),
        (
            PI,
            300. * PI * 1e-9,
            (160. * PI + 120.) * 1e-6,
            [0.002, -0.003 + d, 0.003],
            [-0.008, -0.003, 0.],
            [0.012, 0.007, 0.006],
        ),
        (
            3. * PI / 2.,
            450. * PI * 1e-9,
            (240. * PI + 120.) * 1e-6,
            [0.002 - d / 3., -0.003 + d / 3., 0.003],
            [-0.008, -0.013, 0.],
            [0.012, 0.007, 0.006],
        ),
    ] {
        for reverse in [false, true] {
            let p = profile(theta, reverse);
            let original = p.clone();
            let b = body(k, &p, depth(), [0., 0., 1.]);
            for _ in 0..2 {
                let m = k.mass_properties(&b, 2.5).unwrap();
                close(m.volume_m3, volume, 1e-12);
                close(m.area_m2, area, 1e-10);
                close(m.mass_kg, 2.5 * volume, 1e-12);
                near(m.center_of_mass, center);
                let bounds = k.bounding_box(&b).unwrap();
                near(bounds.min, low);
                near(bounds.max, high);
            }
            assert_eq!(p, original);
            let mesh = k
                .tessellate(
                    &b,
                    &TessellationSettings {
                        linear_deflection: 1e-5,
                        angular_deflection_deg: 4.,
                    },
                )
                .unwrap();
            let (mesh_volume, mesh_center) = evidence::integrated(&mesh, true);
            assert!((mesh_volume / volume - 1.).abs() < 0.01);
            for i in 0..3 {
                close(mesh_center[i], center[i], 1e-5);
            }
        }
    }
}
pub fn placement_mesh(k: &dyn GeometryKernel, mock: bool) {
    let d = 0.04 / (3. * PI);
    let planes = [
        (
            SketchPlacement::global_xy(),
            [0., 0., 1.],
            [0.002, -0.003, 0.],
            [0.012, 0.007, 0.006],
            [0.002 + d, -0.003 + d, 0.003],
        ),
        (
            SketchPlacement {
                origin_m: [0.; 3],
                x_axis_m: [0., 1., 0.],
                y_axis_m: [0., 0., 1.],
            },
            [1., 0., 0.],
            [0., 0.002, -0.003],
            [0.006, 0.012, 0.007],
            [0.003, 0.002 + d, -0.003 + d],
        ),
        (
            SketchPlacement {
                origin_m: [0.; 3],
                x_axis_m: [1., 0., 0.],
                y_axis_m: [0., 0., 1.],
            },
            [0., -1., 0.],
            [0.002, -0.006, -0.003],
            [0.012, 0., 0.007],
            [0.002 + d, -0.003, -0.003 + d],
        ),
        (
            SketchPlacement {
                origin_m: [0.01, -0.017, 0.034],
                x_axis_m: [1., 0., 0.],
                y_axis_m: [0., 0.8, -0.6],
            },
            [0., 0.6, 0.8],
            [0.012, -0.0194, 0.0298],
            [0.022, -0.0078, 0.0406],
            [0.012 + d, -0.0176 + 0.8 * d, 0.0382 - 0.6 * d],
        ),
    ];
    let quality = TessellationSettings {
        linear_deflection: 1e-5,
        angular_deflection_deg: 4.,
    };
    for (plane, normal, low, high, center) in planes {
        for reverse in [false, true] {
            let mut p = profile(PI / 2., reverse);
            p.placement = Some(plane);
            let b = body(k, &p, depth(), normal);
            let mass = k.mass_properties(&b, 1.).unwrap();
            near(mass.center_of_mass, center);
            let bounds = k.bounding_box(&b).unwrap();
            near(bounds.min, low);
            near(bounds.max, high);
            let mesh = k.tessellate(&b, &quality).unwrap();
            assert!(mesh.has_triangle_face_ids());
            assert_eq!(
                mesh.triangle_face_ids
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                5
            );
            assert_eq!(mesh, k.tessellate(&b, &quality).unwrap());
            let (v, c) = evidence::integrated(&mesh, true);
            assert!((v / (150. * PI * 1e-9) - 1.).abs() < 0.01);
            for i in 0..3 {
                close(c[i], center[i], 1e-5);
            }
            if mock {
                let old: std::collections::BTreeSet<_> =
                    mesh.triangle_face_ids.iter().copied().collect();
                let moved = k
                    .transform_body(
                        b.clone(),
                        RigidTransform::from_translation([0.1, -0.2, 0.3]),
                    )
                    .unwrap();
                let next = k.tessellate(&moved, &quality).unwrap();
                let new: std::collections::BTreeSet<_> =
                    next.triangle_face_ids.iter().copied().collect();
                assert!(old.is_disjoint(&new), "new body must own its face IDs");
                let ancestry = k.face_derivation_history(&moved);
                assert_eq!(ancestry.len(), 5);
                assert_eq!(
                    ancestry
                        .iter()
                        .map(|(child, _)| *child)
                        .collect::<std::collections::BTreeSet<_>>(),
                    new
                );
                assert_eq!(
                    ancestry
                        .iter()
                        .map(|(_, parent)| *parent)
                        .collect::<std::collections::BTreeSet<_>>(),
                    old
                );
                assert_eq!(k.tessellate(&b, &quality).unwrap(), mesh);
            }
        }
    }
    let p = profile(PI / 2., false);
    let symmetric = body(
        k,
        &p,
        ExtrudeExtent::Symmetric {
            length: Length::from_meters(0.003),
        },
        [0., 0., 1.],
    );
    near(
        k.bounding_box(&symmetric).unwrap().min,
        [0.002, -0.003, -0.003],
    );
    near(
        k.bounding_box(&symmetric).unwrap().max,
        [0.012, 0.007, 0.003],
    );
    near(
        k.mass_properties(&symmetric, 1.).unwrap().center_of_mass,
        [0.002 + d, -0.003 + d, 0.],
    );
    let negative = body(k, &p, depth(), [0., 0., -1.]);
    near(
        k.bounding_box(&negative).unwrap().min,
        [0.002, -0.003, -0.006],
    );
    let base = body(k, &p, depth(), [0., 0., 1.]);
    let moved = k
        .transform_body(
            base.clone(),
            RigidTransform {
                rotation: [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
                translation_m: [0.03, -0.02, 0.01],
            },
        )
        .unwrap();
    near(k.bounding_box(&moved).unwrap().min, [0.023, -0.018, 0.01]);
    near(k.bounding_box(&moved).unwrap().max, [0.033, -0.008, 0.016]);
    near(
        k.mass_properties(&moved, 1.).unwrap().center_of_mass,
        [0.033 - d, -0.018 + d, 0.013],
    );
    let moved_mesh = k.tessellate(&moved, &quality).unwrap();
    let (_, moved_center) = evidence::integrated(&moved_mesh, true);
    for (actual, wanted) in moved_center.into_iter().zip([0.033 - d, -0.018 + d, 0.013]) {
        close(actual, wanted, 1e-5);
    }
    // At 45 degrees, the world Y maximum lies inside the selected arc rather
    // than at a source endpoint. A corners-only or sampled bound cannot pass.
    let q = 0.5_f64.sqrt();
    let diagonal = k
        .transform_body(
            base.clone(),
            RigidTransform {
                rotation: [[q, -q, 0.], [q, q, 0.], [0., 0., 1.]],
                translation_m: [0.; 3],
            },
        )
        .unwrap();
    near(
        k.bounding_box(&diagonal).unwrap().min,
        [-0.005 * q, -0.001 * q, 0.],
    );
    near(
        k.bounding_box(&diagonal).unwrap().max,
        [0.015 * q, -0.001 * q + 0.01, 0.006],
    );
    near(
        k.mass_properties(&diagonal, 1.).unwrap().center_of_mass,
        [0.005 * q, (-0.001 + 2. * d) * q, 0.003],
    );
    let (_, diagonal_center) =
        evidence::integrated(&k.tessellate(&diagonal, &quality).unwrap(), true);
    for (actual, wanted) in
        diagonal_center
            .into_iter()
            .zip([0.005 * q, (-0.001 + 2. * d) * q, 0.003])
    {
        close(actual, wanted, 1e-5);
    }
    assert!(k
        .transform_body(
            base.clone(),
            RigidTransform {
                rotation: [[-1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                translation_m: [0.; 3],
            }
        )
        .is_err());
    near(
        k.mass_properties(&base, 1.).unwrap().center_of_mass,
        [0.002 + d, -0.003 + d, 0.003],
    );
}
