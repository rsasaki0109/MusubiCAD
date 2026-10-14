use opencad_core::{Length, TopoRefId};
use opencad_geometry::*;
use std::f64::consts::PI;
#[path = "mesh_evidence.rs"]
mod evidence;
pub fn stock(k: &dyn GeometryKernel) -> KernelBody {
    let profile = SolvedSketch {
        profile_ref: "circle".into(),
        points: Vec::new(),
        closed: true,
        circle: Some(SolvedCircle {
            center_m: [0.012, 0.009],
            radius_m: 0.02,
        }),
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: None,
    };
    k.extrude(
        k.make_wire_from_sketch(&profile).unwrap(),
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.03),
        },
        ExtrudeOperation::NewBody,
        None,
        [0., 0., 1.],
    )
    .unwrap()
}
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} != {b}, tolerance {t}");
}
fn input(
    s: &CylindricalStockGeometry,
    z: f64,
    angle: f64,
    reverse_normal: bool,
) -> CylindricalDraftInput {
    CylindricalDraftInput {
        axis_hint: s.axis,
        neutral_origin_m: std::array::from_fn(|i| s.origin_m[i] + s.axis[i] * z),
        neutral_normal: s.axis.map(|v| if reverse_normal { -v } else { v }),
        angle_rad: angle.to_radians(),
    }
}
pub fn physical(k: &dyn GeometryKernel, mock: bool) {
    let base = stock(k);
    let before = k.mass_properties(&base, 1.).unwrap();
    let transform = RigidTransform {
        rotation: [
            [1., 0., 0.],
            [0., 25_f64.to_radians().cos(), -25_f64.to_radians().sin()],
            [0., 25_f64.to_radians().sin(), 25_f64.to_radians().cos()],
        ],
        translation_m: [0.05, -0.03, 0.08],
    };
    let moved = k.transform_body(base.clone(), transform).unwrap();
    for (body, axis) in [
        (base.clone(), [0., 0., 1.]),
        (base.clone(), [0., 0., -1.]),
        (
            moved,
            [0., -25_f64.to_radians().sin(), 25_f64.to_radians().cos()],
        ),
    ] {
        let s = k.cylindrical_stock_geometry(&body, axis).unwrap();
        close(s.radius_m, 0.02, 1e-10);
        close(s.height_m, 0.03, 1e-10);
        for id in [s.start_face_id, s.end_face_id, s.lateral_face_id] {
            k.assign_face_ref(&body, id, TopoRefId::new("ref:owned").unwrap())
                .unwrap();
            assert!(k
                .tessellate(&body, &TessellationSettings::default())
                .unwrap()
                .triangle_face_ids
                .contains(&id));
        }
        for z in [0., 0.013, 0.03] {
            for angle in [-7_f64, 5.] {
                for reverse in [false, true] {
                    let i = input(&s, z, angle, reverse);
                    let out = k.draft_cylindrical_stock(body.clone(), &i).unwrap();
                    let h = s.height_m;
                    let r = s.radius_m;
                    let slope = angle.to_radians().tan() * if reverse { -1. } else { 1. };
                    // Expanded integral of (R+k*z0-k*z)^2, independent of endpoint formula.
                    let volume = PI
                        * (r * r * h
                            + 2. * r * slope * (z * h - h * h / 2.)
                            + slope * slope * (z * z * h - z * h * h + h * h * h / 3.));
                    let moment = PI
                        * (r * r * h * h / 2.
                            + 2. * r * slope * (z * h * h / 2. - h * h * h / 3.)
                            + slope
                                * slope
                                * (z * z * h * h / 2. - 2. * z * h * h * h / 3.
                                    + h * h * h * h / 4.));
                    let center: [f64; 3] =
                        std::array::from_fn(|j| s.origin_m[j] + s.axis[j] * moment / volume);
                    let a = r + slope * z;
                    let b = r - slope * (h - z);
                    let area = PI * (a * a + b * b) + PI * (a + b) * h / angle.to_radians().cos();
                    let mass = k.mass_properties(&out, 1.).unwrap();
                    close(mass.volume_m3, volume, 3e-12);
                    close(mass.area_m2, area, 3e-10);
                    for (a, b) in mass.center_of_mass.into_iter().zip(center) {
                        close(a, b, 2e-9);
                    }
                    let q = TessellationSettings {
                        linear_deflection: 1e-6,
                        angular_deflection_deg: 6.,
                    };
                    let mesh = k.tessellate(&out, &q).unwrap();
                    let (mv, mc) = evidence::integrated(&mesh, mock);
                    close(mv, volume, volume * 0.0005);
                    for (a, b) in mc.into_iter().zip(center) {
                        close(a, b, 2e-6);
                    }
                    if mock {
                        assert_eq!(mesh, k.tessellate(&out, &q).unwrap());
                    }
                    let mut side_vertices = 0;
                    for p in &mesh.positions {
                        let relative: [f64; 3] =
                            std::array::from_fn(|j| f64::from(p[j]) - s.origin_m[j]);
                        let axial: f64 = relative.into_iter().zip(s.axis).map(|(x, y)| x * y).sum();
                        let radial = relative
                            .into_iter()
                            .zip(s.axis)
                            .map(|(x, y)| (x - y * axial).powi(2))
                            .sum::<f64>()
                            .sqrt();
                        // Disk centres are permitted; every other generated contact is on the cone.
                        if radial > r * 0.2 {
                            close(radial, r - slope * (axial - z), 5e-8);
                            side_vertices += 1;
                        }
                    }
                    assert!(side_vertices > 20);
                    let bounds = k.bounding_box(&out).unwrap();
                    for j in 0..3 {
                        let circle = (1. - s.axis[j] * s.axis[j]).max(0.).sqrt();
                        let end = s.origin_m[j] + s.axis[j] * h;
                        close(
                            bounds.min[j],
                            (s.origin_m[j] - a * circle).min(end - b * circle),
                            2e-7,
                        );
                        close(
                            bounds.max[j],
                            (s.origin_m[j] + a * circle).max(end + b * circle),
                            2e-7,
                        );
                    }
                    if z.abs() < 1e-12 && (angle - 5.).abs() < 1e-12 && !reverse {
                        let placement = RigidTransform {
                            rotation: transform.rotation,
                            translation_m: [-0.03, 0.06, 0.02],
                        };
                        let moved_out = k.transform_body(out.clone(), placement).unwrap();
                        let moved_mass = k.mass_properties(&moved_out, 1.).unwrap();
                        close(moved_mass.volume_m3, volume, 3e-12);
                        for (a, b) in moved_mass
                            .center_of_mass
                            .into_iter()
                            .zip(placement.transform_point(center))
                        {
                            close(a, b, 2e-9);
                        }
                        let (v, c) =
                            evidence::integrated(&k.tessellate(&moved_out, &q).unwrap(), mock);
                        close(v, volume, volume * 0.0005);
                        for (a, b) in c.into_iter().zip(placement.transform_point(center)) {
                            close(a, b, 2e-6);
                        }
                    }
                    assert!(k.cylindrical_stock_geometry(&out, axis).is_err());
                    for id in mesh
                        .triangle_face_ids
                        .iter()
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>()
                    {
                        k.assign_face_ref(&out, id, TopoRefId::new("ref:out").unwrap())
                            .unwrap();
                    }
                    assert!(k.face_derivation_history(&out).is_empty());
                    assert!(k.draft_cylindrical_stock(out, &i).is_err());
                }
            }
        }
    }
    assert_eq!(before, k.mass_properties(&base, 1.).unwrap());
}
pub fn refusals(k: &dyn GeometryKernel) {
    let base = stock(k);
    let s = k.cylindrical_stock_geometry(&base, [0., 0., 1.]).unwrap();
    let valid = input(&s, 0., 5., false);
    let before = k.mass_properties(&base, 1.).unwrap();
    for angle in [0., 1e-9, 1e-8, f64::NAN, f64::INFINITY, 1., -1.] {
        assert!(k
            .draft_cylindrical_stock(
                base.clone(),
                &CylindricalDraftInput {
                    angle_rad: angle,
                    ..valid
                }
            )
            .is_err());
    }
    for normal in [
        [0., 0., 0.],
        [0., 0., 2.],
        [1., 0., 0.],
        [1e-5, 0., 1.],
        [f64::NAN, 0., 1.],
    ] {
        assert!(k.cylindrical_stock_geometry(&base, normal).is_err());
        assert!(k
            .draft_cylindrical_stock(
                base.clone(),
                &CylindricalDraftInput {
                    neutral_normal: normal,
                    ..valid
                }
            )
            .is_err());
    }
    for z in [-5e-10, 0.03 + 5e-10, f64::NAN, f64::INFINITY] {
        assert!(k
            .draft_cylindrical_stock(
                base.clone(),
                &CylindricalDraftInput {
                    neutral_origin_m: [0.012, 0.009, z],
                    ..valid
                }
            )
            .is_err());
    }
    assert!(k
        .draft_cylindrical_stock(
            base.clone(),
            &CylindricalDraftInput {
                angle_rad: 45_f64.to_radians(),
                ..valid
            }
        )
        .is_err());
    assert!(k
        .cylindrical_stock_geometry(&KernelBody::new(0), [0., 0., 1.])
        .is_err());
    let rectangle = SolvedSketch {
        points: vec![[0., 0.], [0.04, 0.], [0.04, 0.03], [0., 0.03]],
        circle: None,
        profile_ref: "rect".into(),
        closed: true,
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: None,
    };
    let block = k
        .extrude(
            k.make_wire_from_sketch(&rectangle).unwrap(),
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.03),
            },
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.],
        )
        .unwrap();
    assert!(k.cylindrical_stock_geometry(&block, [0., 0., 1.]).is_err());
    assert!(k.draft_cylindrical_stock(block, &valid).is_err());
    assert_eq!(before, k.mass_properties(&base, 1.).unwrap());
}
