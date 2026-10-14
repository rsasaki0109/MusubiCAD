use opencad_core::{Length, TopoRefId};
use opencad_geometry::*;
#[path = "mesh_evidence.rs"]
mod evidence;
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    evidence::dot(a, b)
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    evidence::sub(a, b)
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    evidence::cross(a, b)
}
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} != {b}, tolerance {t}");
}
pub fn stock(k: &dyn GeometryKernel) -> KernelBody {
    let s = SolvedSketch {
        profile_ref: "stock".into(),
        points: vec![[0., 0.], [0.04, 0.], [0.04, 0.03], [0., 0.03]],
        closed: true,
        circle: None,
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: None,
    };
    k.extrude(
        k.make_wire_from_sketch(&s).unwrap(),
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.01),
        },
        ExtrudeOperation::NewBody,
        None,
        [0., 0., 1.],
    )
    .unwrap()
}
fn rotate(t: RigidTransform, p: [f64; 3]) -> [f64; 3] {
    t.rotation.map(|r| dot(r, p))
}
pub fn physical(k: &dyn GeometryKernel, mock: bool) {
    let original = stock(k);
    let before = k.mass_properties(&original, 1.).unwrap();
    let angle = 25_f64.to_radians();
    let moved = RigidTransform {
        rotation: [
            [1., 0., 0.],
            [0., angle.cos(), -angle.sin()],
            [0., angle.sin(), angle.cos()],
        ],
        translation_m: [0.05, -0.03, 0.08],
    };
    let moved_body = k.transform_body(original.clone(), moved).unwrap();
    let identity = RigidTransform {
        rotation: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        translation_m: [0.; 3],
    };
    let dims = [0.04, 0.03, 0.01];
    for (body, transform) in [(original.clone(), identity), (moved_body, moved)] {
        for axis in 0..3 {
            for high in [false, true] {
                for along in [(axis + 1) % 3, (axis + 2) % 3] {
                    for direction in [-1_f64, 1.] {
                        for degrees in [-5_f64, 5.] {
                            let mut center = [0.017, 0.014, 0.006];
                            center[axis] = if high { dims[axis] } else { 0. };
                            let mut normal = [0.; 3];
                            normal[axis] = if high { 1. } else { -1. };
                            let mut tangent = [0.; 3];
                            tangent[along] = direction;
                            let center = transform.transform_point(center);
                            let normal = rotate(transform, normal);
                            let tangent = rotate(transform, tangent);
                            let width = cross(normal, tangent);
                            let l = 0.006;
                            let t0 = 0.002;
                            let h = 0.003;
                            let slope = degrees.to_radians().tan();
                            let input = TaperedRibInput {
                                entry: FacePick {
                                    point_m: center,
                                    normal,
                                },
                                length_axis_hint: tangent,
                                length_m: l,
                                base_thickness_m: t0,
                                height_m: h,
                                angle_rad: degrees.to_radians(),
                            };
                            let output = k.add_tapered_rib_stock(body.clone(), &input).unwrap();
                            assert!(k.face_derivation_history(&output).is_empty());
                            let actual = k.mass_properties(&output, 1.).unwrap();
                            // Independently integrate cross-section L*(T0-2*tan(theta)*z).
                            let added = l * (t0 * h - slope * h * h);
                            let first = l * (t0 * h * h / 2. - 2. * slope * h * h * h / 3.);
                            let volume = 12e-6 + added;
                            close(actual.volume_m3, volume, volume * 1e-8);
                            let stock_center = transform.transform_point([0.02, 0.015, 0.005]);
                            let expected: [f64; 3] = std::array::from_fn(|i| {
                                (12e-6 * stock_center[i] + added * center[i] + normal[i] * first)
                                    / volume
                            });
                            for (a, b) in actual.center_of_mass.into_iter().zip(expected) {
                                close(a, b, 1e-9);
                            }
                            let area = 0.0038 - 2. * l * slope * h + 2. * t0 * h
                                - 2. * slope * h * h
                                + 2. * l * h / degrees.to_radians().cos();
                            close(actual.area_m2, area, area * 1e-8);
                            let mesh = k
                                .tessellate(
                                    &output,
                                    &TessellationSettings {
                                        linear_deflection: 1e-6,
                                        angular_deflection_deg: 6.,
                                    },
                                )
                                .unwrap();
                            let (v, c) = evidence::integrated(&mesh, mock);
                            close(v, volume, volume * 1e-5);
                            for (a, b) in c.into_iter().zip(expected) {
                                close(a, b, 2e-8);
                            }
                            let ids: std::collections::BTreeSet<_> =
                                mesh.triangle_face_ids.iter().copied().collect();
                            assert_eq!(ids.len(), 11);
                            for id in ids {
                                k.assign_face_ref(
                                    &output,
                                    id,
                                    TopoRefId::new(format!("ref:face:runtime_{id}")).unwrap(),
                                )
                                .unwrap();
                            }
                            assert!(k
                                .assign_face_ref(
                                    &output,
                                    999999,
                                    TopoRefId::new("ref:face:invalid").unwrap()
                                )
                                .is_err());
                            if mock {
                                assert_eq!(mesh.triangle_count(), 68);
                                assert_eq!(
                                    mesh,
                                    k.tessellate(
                                        &output,
                                        &TessellationSettings {
                                            linear_deflection: 1e-6,
                                            angular_deflection_deg: 6.
                                        }
                                    )
                                    .unwrap()
                                );
                            }
                            for p in &mesh.positions {
                                let q = sub(p.map(f64::from), center);
                                let z = dot(q, normal);
                                if z > 1e-7 {
                                    let s = dot(q, tangent);
                                    let t = dot(q, width);
                                    let radius = (t0 - 2. * slope * z) / 2.;
                                    assert!(
                                        z <= h + 3e-8
                                            && s.abs() <= l / 2. + 3e-8
                                            && t.abs() <= radius + 3e-8
                                    );
                                    // Every rib exterior vertex contacts a proved square end,
                                    // straight tapered wall or top plane; no invented interior.
                                    assert!(
                                        (s.abs() - l / 2.).abs() < 3e-8
                                            || (t.abs() - radius).abs() < 3e-8
                                            || (z - h).abs() < 3e-8
                                    );
                                }
                            }
                            for t in mesh.indices.chunks_exact(3) {
                                let p: [[f64; 3]; 3] = std::array::from_fn(|i| {
                                    mesh.positions[t[i] as usize].map(f64::from)
                                });
                                let q = p.map(|p| sub(p, center));
                                if q.iter().all(|q| dot(*q, normal).abs() < 3e-8) {
                                    let centroid =
                                        std::array::from_fn(|i| (q[0][i] + q[1][i] + q[2][i]) / 3.);
                                    assert!(
                                        !(dot(centroid, tangent).abs() < l / 2. - 1e-7
                                            && dot(centroid, width).abs() < t0 / 2. - 1e-7),
                                        "internal contact triangle survived"
                                    );
                                }
                            }
                            let mut points: Vec<_> = (0..8)
                                .map(|mask| {
                                    transform.transform_point(std::array::from_fn(|i| {
                                        if mask & (1 << i) != 0 {
                                            dims[i]
                                        } else {
                                            0.
                                        }
                                    }))
                                })
                                .collect();
                            for end in [-0.5, 0.5] {
                                for side in [-0.5, 0.5] {
                                    for z in [0., h] {
                                        points.push(std::array::from_fn(|i| {
                                            center[i]
                                                + tangent[i] * l * end
                                                + width[i] * (t0 - 2. * slope * z) * side
                                                + normal[i] * z
                                        }));
                                    }
                                }
                            }
                            let bounds = k.bounding_box(&output).unwrap();
                            for i in 0..3 {
                                close(
                                    bounds.min[i],
                                    points.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min),
                                    2e-7,
                                );
                                close(
                                    bounds.max[i],
                                    points
                                        .iter()
                                        .map(|p| p[i])
                                        .fold(f64::NEG_INFINITY, f64::max),
                                    2e-7,
                                );
                            }
                            close(
                                k.mass_properties(&body, 1.).unwrap().volume_m3,
                                12e-6,
                                1e-12,
                            );
                        }
                    }
                }
            }
        }
    }
    // Negative draft can overhang the original stock while its entire base
    // stays strictly contained. The real union must retain that added material.
    let input = TaperedRibInput {
        entry: FacePick {
            point_m: [0.017, 0.014, 0.01],
            normal: [0., 0., 1.],
        },
        length_axis_hint: [1., 0., 0.],
        length_m: 0.006,
        base_thickness_m: 0.002,
        height_m: 0.03,
        angle_rad: -std::f64::consts::FRAC_PI_4,
    };
    let output = k.add_tapered_rib_stock(original.clone(), &input).unwrap();
    let mass = k.mass_properties(&output, 1.).unwrap();
    let added = 0.006 * (0.002 * 0.03 + 0.03 * 0.03);
    let volume = 12e-6 + added;
    close(mass.volume_m3, volume, 1e-12);
    let axial =
        (0.002 * 0.03 * 0.03 / 2. + 2. * 0.03_f64.powi(3) / 3.) / (0.002 * 0.03 + 0.03 * 0.03);
    let expected: [f64; 3] = std::array::from_fn(|i| {
        (12e-6 * [0.02, 0.015, 0.005][i] + added * [0.017, 0.014, 0.01 + axial][i]) / volume
    });
    let out_mesh = k
        .tessellate(&output, &TessellationSettings::default())
        .unwrap();
    let (v, c) = evidence::integrated(&out_mesh, mock);
    close(v, volume, volume * 1e-5);
    for (a, b) in c.into_iter().zip(expected) {
        close(a, b, 2e-8);
    }
    let bbox = k.bounding_box(&output).unwrap();
    close(bbox.min[1], -0.017, 2e-7);
    close(bbox.max[1], 0.045, 2e-7);
    let placed = k.transform_body(output, moved).unwrap();
    let placed_mass = k.mass_properties(&placed, 1.).unwrap();
    close(placed_mass.volume_m3, volume, 1e-12);
    let placed_expected = moved.transform_point(expected);
    for (a, b) in placed_mass.center_of_mass.into_iter().zip(placed_expected) {
        close(a, b, 1e-9);
    }
    let placed_mesh = k
        .tessellate(&placed, &TessellationSettings::default())
        .unwrap();
    let (v, c) = evidence::integrated(&placed_mesh, mock);
    close(v, volume, volume * 1e-5);
    for (a, b) in c.into_iter().zip(placed_expected) {
        close(a, b, 2e-8);
    }
    assert_eq!(k.mass_properties(&original, 1.).unwrap(), before);
}
pub fn invalid(k: &dyn GeometryKernel) {
    let body = stock(k);
    let before = k.mass_properties(&body, 1.).unwrap();
    let base = TaperedRibInput {
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
    for bad in [
        "angle0",
        "angle_tiny",
        "angle_large",
        "angle_nan",
        "length0",
        "height_negative",
        "width_nan",
        "collapse",
        "normal",
        "axis",
        "axis_oblique",
        "plane",
        "outside",
        "touch",
        "unresolvable_volume",
    ] {
        let mut input = base;
        match bad {
            "angle0" => input.angle_rad = 0.,
            "angle_tiny" => input.angle_rad = 1e-8,
            "angle_large" => input.angle_rad = 1.,
            "angle_nan" => input.angle_rad = f64::NAN,
            "unresolvable_volume" => {
                input.length_m = 1e-8;
                input.base_thickness_m = 4e-8;
                input.height_m = 1e-8;
                input.angle_rad = -std::f64::consts::FRAC_PI_4;
            }
            "length0" => input.length_m = 0.,
            "height_negative" => input.height_m = -0.001,
            "width_nan" => input.base_thickness_m = f64::NAN,
            "collapse" => input.angle_rad = std::f64::consts::FRAC_PI_4,
            "normal" => input.entry.normal = [0., 0., 2.],
            "axis" => input.length_axis_hint = [0., 0., 0.],
            "axis_oblique" => input.length_axis_hint = [0.6, 0.8, 0.],
            "plane" => input.entry.point_m[2] += 5e-10,
            "outside" => input.entry.point_m[0] = 0.1,
            _ => input.entry.point_m[0] = input.length_m / 2.,
        }
        assert!(
            k.add_tapered_rib_stock(body.clone(), &input).is_err(),
            "{bad}"
        );
        assert_eq!(k.mass_properties(&body, 1.).unwrap(), before);
    }
    assert!(k
        .add_tapered_rib_stock(KernelBody::new(u64::MAX / 7), &base)
        .is_err());
    let output = k.add_tapered_rib_stock(body, &base).unwrap();
    assert!(k.add_tapered_rib_stock(output, &base).is_err());
}
