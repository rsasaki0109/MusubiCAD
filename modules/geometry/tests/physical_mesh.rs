//! Independent exported-triangle integration and ownership evidence (ADR-063).
use opencad_core::{Length, TopoRefId};
use opencad_geometry::*;
use std::collections::BTreeSet;
#[path = "support/mesh_evidence.rs"]
mod evidence;
fn integrated(mesh: &MeshSet) -> (f64, [f64; 3]) {
    evidence::integrated(mesh, true)
}
use std::f64::consts::PI;
fn stock() -> SolvedSketch {
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
fn tool(z: f64, r: f64) -> SolvedSketch {
    SolvedSketch {
        points: Vec::new(),
        circle: Some(SolvedCircle {
            center_m: [0.012, 0.009],
            radius_m: r,
        }),
        placement: Some(SketchPlacement {
            origin_m: [0., 0., z],
            ..SketchPlacement::global_xy()
        }),
        ..stock()
    }
}
fn extrude(k: &dyn GeometryKernel, s: &SolvedSketch, l: f64, d: [f64; 3]) -> KernelBody {
    k.extrude(
        k.make_wire_from_sketch(s).unwrap(),
        ExtrudeExtent::Distance {
            length: Length::from_meters(l),
        },
        ExtrudeOperation::NewBody,
        None,
        d,
    )
    .unwrap()
}
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} != {b} (tol {t})");
}
#[test]
fn closed_meshes_match_independent_stock_cylinder_and_stepped_cut_integrals() {
    let k = MockGeometryKernel::with_analytic_primitives();
    let base = extrude(&k, &stock(), 0.01, [0., 0., 1.]);
    let q = TessellationSettings {
        linear_deflection: 1e-5,
        angular_deflection_deg: 8.,
    };
    let mesh = k.tessellate(&base, &q).unwrap();
    let (v, c) = integrated(&mesh);
    close(v, 12e-6, 2e-12);
    for (a, b) in c.into_iter().zip([0.02, 0.015, 0.005]) {
        close(a, b, 2e-9);
    }
    let original = k.mass_properties(&base, 1.).unwrap();
    for reversed in [false, true] {
        let mut body = base.clone();
        let mut cuts = [(0.0033, 0.010), (0.0055, 0.004)];
        if reversed {
            cuts.reverse();
        }
        for (r, d) in cuts {
            let cutter = extrude(&k, &tool(0.01, r), d, [0., 0., -1.]);
            body = k.boolean(body, cutter, BooleanOp::Subtract).unwrap();
        }
        let mesh = k.tessellate(&body, &q).unwrap();
        assert_eq!(mesh, k.tessellate(&body, &q).unwrap());
        let (v, c) = integrated(&mesh);
        let removed =
            PI * (0.0033_f64.powi(2) * 0.01 + (0.0055_f64.powi(2) - 0.0033_f64.powi(2)) * 0.004);
        let expected = 12e-6 - removed;
        assert!(v >= expected - 2e-12);
        assert!(v - expected <= removed * (4_f64.to_radians()).powi(2) / 6. + 2e-12);
        let exact = k.mass_properties(&body, 1.).unwrap();
        for (a, b) in c.into_iter().zip(exact.center_of_mass) {
            close(a, b, 2e-6);
        }
        let mut angles = Vec::new();
        for p in &mesh.positions {
            let p = p.map(f64::from);
            let radius = ((p[0] - 0.012).powi(2) + (p[1] - 0.009).powi(2)).sqrt();
            if radius < 0.006 {
                assert!(
                    (radius - 0.0033).abs() < 3e-9 || (radius - 0.0055).abs() < 3e-9,
                    "wrong ring radius {radius}"
                );
                angles.push((p[1] - 0.009).atan2(p[0] - 0.012).rem_euclid(2. * PI));
            }
            assert!(
                p[0] >= -1e-9
                    && p[0] <= 0.04 + 3e-9
                    && p[1] >= -1e-9
                    && p[1] <= 0.03 + 3e-9
                    && p[2] >= -1e-9
                    && p[2] <= 0.01 + 1e-9
            );
        }
        angles.sort_by(f64::total_cmp);
        angles.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        let maxgap = angles
            .windows(2)
            .map(|p| p[1] - p[0])
            .chain([2. * PI + angles[0] - angles[angles.len() - 1]])
            .fold(0., f64::max);
        assert!(maxgap <= q.angular_deflection_deg.to_radians() + 1e-6);
        // Every actual rectangle corner must remain on both caps; an inscribed
        // outer polygon can pass a loose volume bound but fails this check.
        for x in [0., 0.04] {
            for y in [0., 0.03] {
                for z in [0., 0.01] {
                    assert!(mesh
                        .positions
                        .iter()
                        .any(|p| (f64::from(p[0]) - x).abs() < 3e-9
                            && (f64::from(p[1]) - y).abs() < 3e-9
                            && (f64::from(p[2]) - z).abs() < 3e-9));
                }
            }
        }
        let old: BTreeSet<_> = k
            .tessellate(&base, &q)
            .unwrap()
            .triangle_face_ids
            .into_iter()
            .collect();
        let now: BTreeSet<_> = mesh.triangle_face_ids.iter().copied().collect();
        assert!(old.is_subset(&now));
        assert_eq!(now.len(), 9);
        assert!(k
            .rectangular_stock_geometry(
                &body,
                FacePick {
                    point_m: [0.02, 0.015, 0.01],
                    normal: [0., 0., 1.]
                }
            )
            .is_err());
    }
    assert_eq!(original, k.mass_properties(&base, 1.).unwrap());
    for (z, l) in [(0., 0.01), (0.002, 0.004)] {
        let cylinder = extrude(&k, &tool(z, 0.003), l, [0., 0., 1.]);
        let mesh = k.tessellate(&cylinder, &q).unwrap();
        let (v, c) = integrated(&mesh);
        close(
            v,
            PI * 0.003_f64.powi(2) * l,
            PI * 0.003_f64.powi(2) * l * 0.001,
        );
        for (a, b) in c.into_iter().zip([0.012, 0.009, z + l / 2.]) {
            close(a, b, 2e-9);
        }
        let cut = k
            .boolean(base.clone(), cylinder, BooleanOp::Subtract)
            .unwrap();
        let mesh = k.tessellate(&cut, &q).unwrap();
        let (v, _) = integrated(&mesh);
        close(
            v,
            12e-6 - PI * 0.003_f64.powi(2) * l,
            PI * 0.003_f64.powi(2) * l * 0.001,
        );
    }
}
#[test]
fn stock_witnesses_are_owned_outward_finite_and_survive_rigid_history() {
    let k = MockGeometryKernel::with_analytic_primitives();
    let a = extrude(&k, &stock(), 0.01, [0., 0., 1.]);
    let b = extrude(&k, &stock(), 0.01, [0., 0., 1.]);
    let pick = FacePick {
        point_m: [0.012, 0.009, 0.01],
        normal: [0., 0., 1.],
    };
    let face = k.rectangular_stock_geometry(&a, pick).unwrap();
    assert_eq!(face.face_spans_m, [0.04, 0.03]);
    close(face.inward_thickness_m, 0.01, 1e-12);
    let id = TopoRefId::new("ref:entry").unwrap();
    k.assign_face_ref(&a, face.entry_face_id, id.clone())
        .unwrap();
    assert!(k
        .assign_face_ref(&b, face.entry_face_id, id.clone())
        .is_err());
    for bad in [
        FacePick {
            normal: [0., 0., -1.],
            ..pick
        },
        FacePick {
            normal: [1e-8, 0., 1.],
            ..pick
        },
        FacePick {
            point_m: [0.05, 0.009, 0.01],
            ..pick
        },
        FacePick {
            point_m: [0.012, 0.009, 0.01000001],
            ..pick
        },
        FacePick {
            point_m: [f64::NAN, 0.009, 0.01],
            ..pick
        },
        FacePick {
            normal: [0., 0., 2.],
            ..pick
        },
    ] {
        assert!(k.rectangular_stock_geometry(&a, bad).is_err());
    }
    let moved = k.translate_body(a.clone(), [1., 2., 3.]).unwrap();
    let new = k
        .rectangular_stock_geometry(
            &moved,
            FacePick {
                point_m: [1.012, 2.009, 3.01],
                ..pick
            },
        )
        .unwrap();
    assert_ne!(new.entry_face_id, face.entry_face_id);
    assert!(k
        .face_derivation_history(&moved)
        .contains(&(new.entry_face_id, face.entry_face_id)));
    assert!(k.assign_face_ref(&moved, face.entry_face_id, id).is_err());
    integrated(
        &k.tessellate(&moved, &TessellationSettings::default())
            .unwrap(),
    );
    assert!(MockGeometryKernel::new()
        .rectangular_stock_geometry(&KernelBody::new(14), pick)
        .is_err());
    assert!(k
        .rectangular_stock_geometry(&KernelBody::new(0), pick)
        .is_err());
}
#[test]
fn mesh_admission_refuses_invalid_quality_budget_and_coordinate_collapse() {
    let k = MockGeometryKernel::with_analytic_primitives();
    let cylinder = extrude(&k, &tool(0., 0.0033), 0.01, [0., 0., 1.]);
    for (linear, angle) in [
        (0., 12.),
        (f64::NAN, 12.),
        (1e-3, 0.),
        (1e-3, 181.),
        (1e-3, f64::INFINITY),
        (1e-15, 12.),
        (1e-3, 0.001),
    ] {
        assert!(k
            .tessellate(
                &cylinder,
                &TessellationSettings {
                    linear_deflection: linear,
                    angular_deflection_deg: angle
                }
            )
            .is_err());
    }
    let moved = k.translate_body(cylinder.clone(), [999_000.; 3]).unwrap();
    assert!(k
        .tessellate(&moved, &TessellationSettings::default())
        .is_err());
    assert!(k
        .tessellate(&KernelBody::new(0), &TessellationSettings::default())
        .is_err());
    assert!(k
        .tessellate(&cylinder, &TessellationSettings::default())
        .is_ok());
}
#[test]
fn negative_and_tilted_frames_keep_outward_closed_winding() {
    let k = MockGeometryKernel::with_analytic_primitives();
    for direction in [[0., 0., -1.], [0., 0., 1.]] {
        let mut s = stock();
        s.placement = Some(SketchPlacement {
            origin_m: [0.1, 0.2, 0.3],
            x_axis_m: [1., 0., 0.],
            y_axis_m: [0., 0.8, -0.6],
        });
        let d = [direction[2] * 0., direction[2] * 0.6, direction[2] * 0.8];
        let b = extrude(&k, &s, 0.01, d);
        let mut circle = tool(0., 0.0033);
        circle.placement = s.placement;
        let cutter = extrude(&k, &circle, 0.004, d);
        let (cylinder_volume, _) = integrated(
            &k.tessellate(&cutter, &TessellationSettings::default())
                .unwrap(),
        );
        close(cylinder_volume, PI * 0.0033_f64.powi(2) * 0.004, 2e-9);
        let cut = k.boolean(b.clone(), cutter, BooleanOp::Subtract).unwrap();
        let (cut_volume, cut_center) = integrated(
            &k.tessellate(&cut, &TessellationSettings::default())
                .unwrap(),
        );
        let exact = k.mass_properties(&cut, 1.).unwrap();
        close(cut_volume, exact.volume_m3, 2e-9);
        for (a, b) in cut_center.into_iter().zip(exact.center_of_mass) {
            close(a, b, 2e-6);
        }

        let (v, c) = integrated(&k.tessellate(&b, &TessellationSettings::default()).unwrap());
        close(v, 12e-6, 2e-11);
        for (a, b) in c
            .into_iter()
            .zip(k.mass_properties(&b, 1.).unwrap().center_of_mass)
        {
            close(a, b, 2e-8);
        }
    }
}

#[test]
fn maximum_cut_chain_mesh_budget_and_split_wall_history_are_explicit() {
    let k = MockGeometryKernel::with_analytic_primitives();
    let mut body = extrude(&k, &stock(), 0.01, [0., 0., 1.]);
    let outer: BTreeSet<_> = k
        .tessellate(&body, &TessellationSettings::default())
        .unwrap()
        .triangle_face_ids
        .into_iter()
        .collect();
    let mut first_wall = None;
    for i in 0..64 {
        let cutter = extrude(
            &k,
            &tool(0.01, 0.001 + i as f64 * 0.00004),
            0.01 - i as f64 * 0.00014,
            [0., 0., -1.],
        );
        body = k.boolean(body, cutter, BooleanOp::Subtract).unwrap();
        if i == 0 {
            first_wall = k
                .tessellate(&body, &TessellationSettings::default())
                .unwrap()
                .triangle_face_ids
                .into_iter()
                .find(|id| !outer.contains(id));
        }
        if i == 1 {
            assert!(k
                .face_derivation_history(&body)
                .iter()
                .any(|&(new, old)| Some(old) == first_wall && new != old));
        }
    }
    let mass = k.mass_properties(&body, 1.).unwrap();
    let mesh = k
        .tessellate(&body, &TessellationSettings::default())
        .unwrap();
    integrated(&mesh);
    assert!(mesh.positions.len() <= 100_000);
    assert!(mesh.triangle_count() <= 200_000);
    assert!(k
        .tessellate(
            &body,
            &TessellationSettings {
                linear_deflection: 0.001,
                angular_deflection_deg: 2.
            }
        )
        .is_err());
    assert_eq!(mass, k.mass_properties(&body, 1.).unwrap());
}
#[test]
fn accumulated_near_rigid_drift_and_unrepresentable_stock_quality_refuse() {
    let k = MockGeometryKernel::with_analytic_primitives();
    let mut body = extrude(&k, &stock(), 0.01, [0., 0., 1.]);
    assert!(k
        .tessellate(
            &body,
            &TessellationSettings {
                linear_deflection: 1e-15,
                angular_deflection_deg: 12.
            }
        )
        .is_err());
    let s = 1. + 1e-10;
    let transform = RigidTransform {
        rotation: [[s, 0., 0.], [0., s, 0.], [0., 0., s]],
        translation_m: [0.; 3],
    };
    for _ in 0..8 {
        body = k.transform_body(body, transform).unwrap();
    }
    assert!(k
        .tessellate(&body, &TessellationSettings::default())
        .is_err());
    assert!(k
        .rectangular_stock_geometry(
            &body,
            FacePick {
                point_m: [0.01, 0.01, 0.01],
                normal: [0., 0., 1.]
            }
        )
        .is_err());
}
