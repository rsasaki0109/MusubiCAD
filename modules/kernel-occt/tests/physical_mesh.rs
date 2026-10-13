//! Native BREP admission and independent actual-triangle evidence.
use opencad_core::{Length, TopoRefId};
use opencad_geometry::*;
use opencad_kernel_occt::OcctGeometryKernel;
use std::f64::consts::PI;
#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod evidence;
fn rectangle() -> SolvedSketch {
    SolvedSketch {
        profile_ref: "stock".into(),
        points: vec![[0., 0.], [0.04, 0.], [0.04, 0.03], [0., 0.03]],
        closed: true,
        circle: None,
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: Some(SketchPlacement::global_xy()),
    }
}
fn circle(z: f64, r: f64) -> SolvedSketch {
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
        ..rectangle()
    }
}
fn extrusion(k: &dyn GeometryKernel, s: &SolvedSketch, l: f64, d: [f64; 3]) -> KernelBody {
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
    assert!((a - b).abs() <= t, "{a} != {b} ({t})");
}
#[test]
fn both_kernels_require_real_cuboid_and_unique_outward_in_face_pick() {
    let mock = MockGeometryKernel::with_analytic_primitives();
    let native = OcctGeometryKernel::new();
    for k in [&mock as &dyn GeometryKernel, &native] {
        let stock = extrusion(k, &rectangle(), 0.01, [0., 0., 1.]);
        for (point, normal, thickness, spans) in [
            ([0.012, 0.009, 0.01], [0., 0., 1.], 0.01, [0.03, 0.04]),
            ([0.012, 0.009, 0.], [0., 0., -1.], 0.01, [0.03, 0.04]),
            ([0., 0.015, 0.005], [-1., 0., 0.], 0.04, [0.01, 0.03]),
        ] {
            let pick = FacePick {
                point_m: point,
                normal,
            };
            let w = k.rectangular_stock_geometry(&stock, pick).unwrap();
            close(w.inward_thickness_m, thickness, 1e-12);
            let mut actual = w.face_spans_m;
            actual.sort_by(f64::total_cmp);
            assert!(actual
                .into_iter()
                .zip(spans)
                .all(|(a, b)| (a - b).abs() < 1e-12));
            k.assign_face_ref(
                &stock,
                w.entry_face_id,
                TopoRefId::new("ref:entry").unwrap(),
            )
            .unwrap();
            let mesh = k
                .tessellate(&stock, &TessellationSettings::default())
                .unwrap();
            let mut count = 0;
            for (t, id) in mesh.indices.chunks_exact(3).zip(&mesh.triangle_face_ids) {
                if *id == w.entry_face_id {
                    count += 1;
                    for &i in t {
                        let p = mesh.positions[i as usize].map(f64::from);
                        close(
                            evidence::dot(evidence::sub(p, w.origin_m), normal),
                            0.,
                            3e-9,
                        );
                    }
                }
            }
            assert!(count > 0);
        }
        let pick = FacePick {
            point_m: [0.012, 0.009, 0.01],
            normal: [0., 0., 1.],
        };
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
                point_m: [0.06, 0.009, 0.01],
                ..pick
            },
            FacePick {
                point_m: [0.012, 0.009, 0.01000001],
                ..pick
            },
            FacePick {
                normal: [0., 0., f64::NAN],
                ..pick
            },
        ] {
            assert!(k.rectangular_stock_geometry(&stock, bad).is_err());
        }
        assert!(k
            .rectangular_stock_geometry(&KernelBody::new(u64::MAX), pick)
            .is_err());
        let cylinder = extrusion(k, &circle(0.002, 0.003), 0.004, [0., 0., 1.]);
        assert!(k.rectangular_stock_geometry(&cylinder, pick).is_err());
        let cut = k
            .boolean(stock.clone(), cylinder, BooleanOp::Subtract)
            .unwrap();
        assert!(k.rectangular_stock_geometry(&cut, pick).is_err());
        let moved = k
            .transform_body(
                stock,
                RigidTransform {
                    rotation: [[0., 0., 1.], [1., 0., 0.], [0., 1., 0.]],
                    translation_m: [1., 2., 3.],
                },
            )
            .unwrap();
        let w = k
            .rectangular_stock_geometry(
                &moved,
                FacePick {
                    point_m: [1.01, 2.012, 3.009],
                    normal: [1., 0., 0.],
                },
            )
            .unwrap();
        close(w.inward_thickness_m, 0.01, 1e-12);
    }
    let stock = extrusion(&native, &rectangle(), 0.01, [0., 0., 1.]);
    let copy = native.translate_body(stock.clone(), [0.1, 0., 0.]).unwrap();
    let compound = native.make_compound(&[stock, copy]).unwrap();
    assert!(native
        .rectangular_stock_geometry(
            &compound,
            FacePick {
                point_m: [0.012, 0.009, 0.01],
                normal: [0., 0., 1.]
            }
        )
        .is_err());
    let mut slanted = rectangle();
    slanted.points = vec![[0., 0.], [0.04, 0.], [0.05, 0.03], [0.01, 0.03]];
    let body = extrusion(&native, &slanted, 0.01, [0., 0., 1.]);
    assert!(native
        .rectangular_stock_geometry(
            &body,
            FacePick {
                point_m: [0.02, 0.01, 0.01],
                normal: [0., 0., 1.]
            }
        )
        .is_err());
}
#[test]
fn native_closed_mesh_has_true_ring_contact_and_bounded_volume_centroid_error() {
    let k = OcctGeometryKernel::new();
    let q = TessellationSettings {
        linear_deflection: 1e-5,
        angular_deflection_deg: 8.,
    };
    for reverse in [false, true] {
        let mut body = extrusion(&k, &rectangle(), 0.01, [0., 0., 1.]);
        let mut cuts = [(0.0033, 0.01), (0.0055, 0.004)];
        if reverse {
            cuts.reverse();
        }
        for (r, d) in cuts {
            let tool = extrusion(&k, &circle(0.01, r), d, [0., 0., -1.]);
            body = k.boolean(body, tool, BooleanOp::Subtract).unwrap();
        }
        let mesh = k.tessellate(&body, &q).unwrap();
        let (v, c) = evidence::integrated(&mesh, false);
        let exact = k.mass_properties(&body, 1.).unwrap();
        let removed =
            PI * (0.0033_f64.powi(2) * 0.01 + (0.0055_f64.powi(2) - 0.0033_f64.powi(2)) * 0.004);
        // Native angular deflection is a normal-turn bound; actual ring gaps
        // are measured independently and determine the integration allowance.
        let mut rings = [Vec::new(), Vec::new()];
        for p in &mesh.positions {
            let p = p.map(f64::from);
            let r = ((p[0] - 0.012).powi(2) + (p[1] - 0.009).powi(2)).sqrt();
            if r < 0.006 {
                assert!((r - 0.0033).abs() < 3e-9 || (r - 0.0055).abs() < 3e-9);
                let ring = if (r - 0.0033).abs() < 3e-9 { 0 } else { 1 };
                rings[ring].push((p[1] - 0.009).atan2(p[0] - 0.012).rem_euclid(2. * PI));
            }
        }
        let mut a: f64 = 0.;
        for (angles, radius) in rings.iter_mut().zip([0.0033, 0.0055]) {
            angles.sort_by(f64::total_cmp);
            angles.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            let gap = angles
                .windows(2)
                .map(|p| p[1] - p[0])
                .chain([2. * PI + angles[0] - angles[angles.len() - 1]])
                .fold(0., f64::max);
            assert!(gap <= q.angular_deflection_deg.to_radians() + 1e-6);
            assert!(radius * (1. - (gap / 2.).cos()) <= q.linear_deflection + 3e-9);
            a = a.max(gap);
        }
        assert!(v >= exact.volume_m3 - 2e-12);
        close(v, exact.volume_m3, removed * a * a / 6. + 2e-12);
        for (a, b) in c.into_iter().zip(exact.center_of_mass) {
            close(a, b, 5e-6);
        }
    }
    for (z, l) in [(0., 0.01), (0.002, 0.004)] {
        let stock = extrusion(&k, &rectangle(), 0.01, [0., 0., 1.]);
        let tool = extrusion(&k, &circle(z, 0.003), l, [0., 0., 1.]);
        evidence::integrated(&k.tessellate(&tool, &q).unwrap(), false);
        let cut = k.boolean(stock, tool, BooleanOp::Subtract).unwrap();
        let (v, c) = evidence::integrated(&k.tessellate(&cut, &q).unwrap(), false);
        let exact = k.mass_properties(&cut, 1.).unwrap();
        close(v, exact.volume_m3, PI * 0.003_f64.powi(2) * l * 0.004);
        for (a, b) in c.into_iter().zip(exact.center_of_mass) {
            close(a, b, 5e-6);
        }
    }
}
