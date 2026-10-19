use opencad_geometry::*;
use std::f64::consts::{PI, TAU};
#[path = "mesh_evidence.rs"]
mod mesh_evidence;
fn near(a: [f64; 3], b: [f64; 3], t: f64) {
    for (a, b) in a.into_iter().zip(b) {
        assert!((a - b).abs() < t, "{a} != {b}");
    }
}
pub fn input(
    plane: ProfilePlane,
    angle: f64,
    negative_axis: bool,
    negative_side: bool,
) -> RevolveInput {
    let (a, b) = (0.015, 0.025);
    let mut points = vec![[a, -0.007], [b, -0.007], [b, 0.013], [a, 0.013]];
    if negative_side {
        for p in &mut points {
            p[0] = 0.010 - p[0];
        }
    }
    let segments = (0..4)
        .map(|i| ProfileSegment::Line {
            start_m: points[i],
            end_m: points[(i + 1) % 4],
        })
        .collect();
    RevolveInput {
        sketch: SolvedSketch {
            profile_ref: "profile:test".into(),
            points,
            closed: true,
            circle: None,
            ellipse: None,
            cubic_bezier: None,
            segments,
            placement: None,
        },
        profile_plane: plane,
        axis_origin_m: plane.map_point(0.005, 0.004),
        axis_direction_m: plane.map_point(0., if negative_axis { -2. } else { 2. }),
        angle_rad: angle,
        operation: RevolveOperation::NewBody,
        target: None,
    }
}
pub fn physical<K: GeometryKernel>(k: &K) {
    for plane in [ProfilePlane::Xy, ProfilePlane::Yz, ProfilePlane::Xz] {
        for negative_axis in [false, true] {
            for negative_side in [false, true] {
                for deg in [60., 90., 180., 270., 360.] {
                    let t = deg * PI / 180.;
                    let mut input = input(plane, t, negative_axis, negative_side);
                    // Reverse winding, with matching ordered exact line endpoints.
                    if negative_axis {
                        input.sketch.points.reverse();
                        input.sketch.segments = (0..4)
                            .map(|i| ProfileSegment::Line {
                                start_m: input.sketch.points[i],
                                end_m: input.sketch.points[(i + 1) % 4],
                            })
                            .collect();
                    }
                    RectangularRevolveGeometry::from_input(&input).unwrap();
                    let body = k.revolve(&input).unwrap_or_else(|e| {
                        panic!("{plane:?}/{deg}/{negative_axis}/{negative_side}: {e}")
                    });
                    let m = k.mass_properties(&body, 2700.).unwrap();
                    let a = 0.01_f64;
                    let b = 0.02_f64;
                    let h = 0.02;
                    let v = (b * b - a * a) * t * h / 2.;
                    let area = (b * b - a * a) * t
                        + (a + b) * t * h
                        + if deg > 359. { 0. } else { 2. * (b - a) * h };
                    assert!(
                        (m.volume_m3 - v).abs() < 1e-12,
                        "{plane:?}/{deg}/{negative_axis}/{negative_side}: {m:?} vs {v}"
                    );
                    assert!((m.area_m2 - area).abs() < 1e-10, "area {m:?} vs {area}");
                    assert!((m.mass_kg - v * 2700.).abs() < 1e-9);
                    let sign = if negative_side { -1. } else { 1. };
                    let axis_sign = if negative_axis { -1. } else { 1. };
                    let x = plane.map_point(sign, 0.);
                    let z = plane.map_point(0., axis_sign);
                    let y = [
                        z[1] * x[2] - z[2] * x[1],
                        z[2] * x[0] - z[0] * x[2],
                        z[0] * x[1] - z[1] * x[0],
                    ];
                    let origin = plane.map_point(0.005, if negative_axis { 0.013 } else { -0.007 });
                    let point = |r: f64, angle: f64, height: f64| -> [f64; 3] {
                        std::array::from_fn(|i| {
                            origin[i]
                                + z[i] * height
                                + r * (x[i] * angle.cos() + y[i] * angle.sin())
                        })
                    };
                    let factor = 2. * (b.powi(3) - a.powi(3)) / (3. * (b * b - a * a) * t);
                    let center = std::array::from_fn(|i| {
                        origin[i]
                            + z[i] * h / 2.
                            + if deg > 359. {
                                0.
                            } else {
                                factor * (x[i] * t.sin() + y[i] * (1. - t.cos()))
                            }
                    });
                    assert!((0..3).all(|i| (m.center_of_mass[i]-center[i]).abs()<1e-9), "centroid {plane:?}/{deg}/{negative_axis}/{negative_side}: {:?} vs {center:?}",m.center_of_mass);
                    let mut lo = [f64::INFINITY; 3];
                    let mut hi = [f64::NEG_INFINITY; 3];
                    for r in [a, b] {
                        for height in [0., h] {
                            for angle in [0., t, PI / 2., PI, 3. * PI / 2., TAU] {
                                if angle > t + 1e-14 {
                                    continue;
                                }
                                for (i, c) in point(r, angle, height).into_iter().enumerate() {
                                    lo[i] = lo[i].min(c);
                                    hi[i] = hi[i].max(c);
                                }
                            }
                        }
                    }
                    let bb = k.bounding_box(&body).unwrap();
                    near(bb.min, lo, 3e-7);
                    near(bb.max, hi, 3e-7);
                    // Real closed mesh, outward normals and absolute contacts, independent of mass.
                    if deg == 90. || deg == 360. {
                        let mesh = k
                            .tessellate(
                                &body,
                                &TessellationSettings {
                                    linear_deflection: 1e-5,
                                    angular_deflection_deg: 4.,
                                },
                            )
                            .unwrap();
                        let (mv, mc) = mesh_evidence::integrated(&mesh, true);
                        assert!((mv - v).abs() < v * 0.001);
                        near(mc, center, 3e-6);
                        for radius in [a, b] {
                            for height in [0., h] {
                                for angle in [0., t] {
                                    let expected = point(radius, angle, height);
                                    assert!(
                                        mesh.positions.iter().any(|p| (0..3).all(|i| (f64::from(
                                            p[i]
                                        )
                                            - expected[i])
                                            .abs()
                                            < 3e-7)),
                                        "missing contact {expected:?}"
                                    );
                                }
                            }
                        }
                        let faces = mesh
                            .triangle_face_ids
                            .iter()
                            .copied()
                            .collect::<std::collections::BTreeSet<_>>();
                        assert_eq!(faces.len(), 4 + if deg > 359. { 0 } else { 2 });
                    }
                }
            }
        }
    }
}
pub fn placed<K: GeometryKernel>(k: &K, check_history: bool) {
    let p = input(ProfilePlane::Xy, PI / 2., false, false);
    let body = k.revolve(&p).unwrap();
    let translated = k.translate_body(body.clone(), [0.1, -0.2, 0.3]).unwrap();
    let translated_mesh = k
        .tessellate(&translated, &TessellationSettings::default())
        .unwrap();
    let (v, c) = mesh_evidence::integrated(&translated_mesh, true);
    assert!((v - 1500. * PI * 1e-9).abs() < 1e-8);
    let native_center = k.mass_properties(&translated, 1.).unwrap().center_of_mass;
    near(c, native_center, 1e-4);
    let transform = RigidTransform {
        rotation: [[0., 0., 1.], [1., 0., 0.], [0., 1., 0.]],
        translation_m: [0.04, -0.02, 0.03],
    };
    let moved = k.transform_body(body.clone(), transform).unwrap();
    let m = k.mass_properties(&moved, 1.).unwrap();
    let d = 2. * (0.02_f64.powi(3) - 0.01_f64.powi(3))
        / (3. * (0.02_f64.powi(2) - 0.01_f64.powi(2)) * (PI / 2.));
    near(m.center_of_mass, [0.04 - d, -0.015 + d, 0.033], 1e-9);
    let bb = k.bounding_box(&moved).unwrap();
    near(bb.min, [0.02, -0.015, 0.023], 3e-7);
    near(bb.max, [0.04, 0.005, 0.043], 3e-7);
    let mesh = k
        .tessellate(&moved, &TessellationSettings::default())
        .unwrap();
    mesh_evidence::integrated(&mesh, true);
    if check_history {
        let old = k
            .tessellate(&body, &TessellationSettings::default())
            .unwrap()
            .triangle_face_ids
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let new = mesh
            .triangle_face_ids
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let history = k.face_derivation_history(&moved);
        assert_eq!(history.len(), 6);
        assert!(history
            .iter()
            .all(|(a, b)| new.contains(a) && old.contains(b) && !old.contains(a)));
    }
}
// An oblique-in-XY edge-parallel axis is distinct from an unsupported skew
// axis. Independent 3-4-5 rotation keeps the entire source on global XY.
pub fn in_plane<K: GeometryKernel>(k: &K) {
    let mut p = input(ProfilePlane::Xy, PI / 2., false, false);
    for v in &mut p.sketch.points {
        *v = [0.6 * v[0] - 0.8 * v[1], 0.8 * v[0] + 0.6 * v[1]];
    }
    p.sketch.segments = (0..4)
        .map(|i| ProfileSegment::Line {
            start_m: p.sketch.points[i],
            end_m: p.sketch.points[(i + 1) % 4],
        })
        .collect();
    p.axis_direction_m = [-0.8, 0.6, 0.];
    p.axis_origin_m = [0.003 - 0.8 * 0.124, 0.004 + 0.6 * 0.124, 0.];
    RectangularRevolveGeometry::from_input(&p).unwrap();
    let b = k.revolve(&p).unwrap();
    let m = k.mass_properties(&b, 1.).unwrap();
    let d = 28. / (900. * PI);
    assert!((m.volume_m3 - 1500. * PI * 1e-9).abs() < 1e-12);
    near(
        m.center_of_mass,
        [0.0006 + 0.6 * d, 0.0058 + 0.8 * d, -d],
        1e-9,
    );
    let bb = k.bounding_box(&b).unwrap();
    near(bb.min, [-0.0074, -0.0002, -0.02], 3e-7);
    near(bb.max, [0.0206, 0.0278, 0.], 3e-7);
    let mesh = k
        .tessellate(
            &b,
            &TessellationSettings {
                linear_deflection: 1e-5,
                angular_deflection_deg: 4.,
            },
        )
        .unwrap();
    let (v, c) = mesh_evidence::integrated(&mesh, true);
    assert!((v - 1500. * PI * 1e-9).abs() < 1e-9);
    near(c, m.center_of_mass, 3e-6);
}
