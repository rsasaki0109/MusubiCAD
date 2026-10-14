use opencad_core::{Length, TopoRefId};
use opencad_geometry::*;
use std::{collections::BTreeSet, f64::consts::PI};
#[path = "mesh_evidence.rs"]
mod evidence;
pub fn rectangle() -> SolvedSketch {
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
pub fn stock(k: &dyn GeometryKernel) -> KernelBody {
    k.extrude(
        k.make_wire_from_sketch(&rectangle()).unwrap(),
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.01),
        },
        ExtrudeOperation::NewBody,
        None,
        [0., 0., 1.],
    )
    .unwrap()
}
pub fn input(bottom: bool, blind: bool) -> CountersinkCut {
    CountersinkCut {
        entry: FacePick {
            point_m: [0.012, 0.009, if bottom { 0. } else { 0.01 }],
            normal: [0., 0., if bottom { -1. } else { 1. }],
        },
        bore_radius_m: 0.0033,
        seat_radius_m: 0.006,
        bore_depth_m: if blind { 0.008 } else { 0.01 },
        seat_depth_m: 0.0027,
    }
}
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} != {b}, tol {t}");
}
// Independent union oracle: bore + annular excess, expressed about entry.
fn oracle(c: &CountersinkCut) -> (f64, [f64; 3], f64) {
    let (r, cap, d, s) = (
        c.bore_radius_m,
        c.seat_radius_m,
        c.bore_depth_m,
        c.seat_depth_m,
    );
    let cylinder = PI * r * r * d;
    let extra = PI * s * (cap * cap + cap * r - 2. * r * r) / 3.;
    let removed = cylinder + extra;
    let axial_first =
        cylinder * d / 2. + PI * s * s * (cap * cap + 2. * cap * r - 3. * r * r) / 12.;
    let volume = 0.04 * 0.03 * 0.01 - removed;
    let original = [12e-6 * 0.02, 12e-6 * 0.015, 12e-6 * 0.005];
    let center = std::array::from_fn(|i| {
        (original[i] - removed * c.entry.point_m[i] + axial_first * c.entry.normal[i]) / volume
    });
    let area = 2. * (0.04 * 0.03 + 0.04 * 0.01 + 0.03 * 0.01) - PI * cap * cap
        + PI * (cap + r) * (s * s + (cap - r) * (cap - r)).sqrt()
        + 2. * PI * r * (d - s)
        + if d < 0.01 { PI * r * r } else { -PI * r * r };
    (volume, center, area)
}
pub fn physical(k: &dyn GeometryKernel, mock: bool) {
    let base = stock(k);
    let original = k.mass_properties(&base, 1.).unwrap();
    let q = TessellationSettings {
        linear_deflection: 1e-6,
        angular_deflection_deg: 6.,
    };
    for bottom in [false, true] {
        for blind in [false, true] {
            for seat_depth_m in [0.0015, 0.0027, 0.0041] {
                let c = CountersinkCut {
                    seat_depth_m,
                    ..input(bottom, blind)
                };
                let body = k.cut_countersunk_stock(base.clone(), &c).unwrap();
                let (v, center, area) = oracle(&c);
                let actual = k.mass_properties(&body, 1.).unwrap();
                close(actual.volume_m3, v, 2e-13);
                close(actual.area_m2, area, 2e-10);
                for (a, b) in actual.center_of_mass.into_iter().zip(center) {
                    close(a, b, 2e-9);
                }
                let mesh = k.tessellate(&body, &q).unwrap();
                if mock {
                    assert_eq!(mesh, k.tessellate(&body, &q).unwrap());
                }
                let (mv, mc) = evidence::integrated(&mesh, mock);
                close(mv, v, 2e-9);
                for (a, b) in mc.into_iter().zip(center) {
                    close(a, b, 2e-6);
                }
                let ids: BTreeSet<_> = mesh.triangle_face_ids.iter().copied().collect();
                if mock {
                    let old: BTreeSet<_> = k
                        .tessellate(&base, &q)
                        .unwrap()
                        .triangle_face_ids
                        .into_iter()
                        .collect();
                    assert!(old.is_subset(&ids));
                    assert_eq!(ids.len(), if blind { 9 } else { 8 });
                }
                for id in &ids {
                    k.assign_face_ref(&body, *id, TopoRefId::new("ref:proof").unwrap())
                        .unwrap();
                }
                assert!(k
                    .assign_face_ref(&body, 0, TopoRefId::new("ref:bad").unwrap())
                    .is_err());
                assert_eq!(
                    k.bounding_box(&body).unwrap(),
                    k.bounding_box(&base).unwrap()
                );
                assert!(k.cut_countersunk_stock(body.clone(), &c).is_err());
                assert!(k.rectangular_stock_geometry(&body, c.entry).is_err());
                let transform = RigidTransform {
                    rotation: [[0., 0., 1.], [1., 0., 0.], [0., 1., 0.]],
                    translation_m: [0.1, -0.05, 0.2],
                };
                let moved = k.transform_body(body, transform).unwrap();
                let mm = k.mass_properties(&moved, 1.).unwrap();
                close(mm.volume_m3, v, 2e-13);
                for (a, b) in mm
                    .center_of_mass
                    .into_iter()
                    .zip(transform.transform_point(center))
                {
                    close(a, b, 2e-9);
                }
                let (mv, mc) = evidence::integrated(&k.tessellate(&moved, &q).unwrap(), mock);
                close(mv, v, 2e-9);
                for (a, b) in mc.into_iter().zip(transform.transform_point(center)) {
                    close(a, b, 2e-6);
                }
                let mut moved_cut = c;
                moved_cut.entry.point_m = transform.transform_point(c.entry.point_m);
                moved_cut.entry.normal = [c.entry.normal[2], c.entry.normal[0], c.entry.normal[1]];
                let moved_stock = k.transform_body(base.clone(), transform).unwrap();
                let direct = k.cut_countersunk_stock(moved_stock, &moved_cut).unwrap();
                close(k.mass_properties(&direct, 1.).unwrap().volume_m3, v, 2e-13);
            }
        }
    }
    assert_eq!(original, k.mass_properties(&base, 1.).unwrap());
}
pub fn refusals(k: &dyn GeometryKernel) {
    let base = stock(k);
    let before = k.mass_properties(&base, 1.).unwrap();
    let valid = input(false, false);
    let mut bad = Vec::new();
    for value in [0., -1., f64::NAN, f64::INFINITY, 1001.] {
        bad.push(CountersinkCut {
            bore_radius_m: value,
            ..valid
        });
        bad.push(CountersinkCut {
            seat_radius_m: value,
            ..valid
        });
        bad.push(CountersinkCut {
            bore_depth_m: value,
            ..valid
        });
        bad.push(CountersinkCut {
            seat_depth_m: value,
            ..valid
        });
    }
    bad.extend([
        CountersinkCut {
            seat_radius_m: valid.bore_radius_m,
            ..valid
        },
        CountersinkCut {
            seat_radius_m: valid.bore_radius_m + 5e-10,
            ..valid
        },
        CountersinkCut {
            seat_radius_m: 0.009,
            ..valid
        },
        CountersinkCut {
            bore_depth_m: 0.01 + 5e-10,
            ..valid
        },
        CountersinkCut {
            seat_depth_m: valid.bore_depth_m,
            ..valid
        },
        CountersinkCut {
            seat_depth_m: valid.bore_depth_m - 5e-10,
            ..valid
        },
        CountersinkCut {
            entry: FacePick {
                normal: [0., 0., -1.],
                ..valid.entry
            },
            ..valid
        },
        CountersinkCut {
            entry: FacePick {
                normal: [0., 0., 2.],
                ..valid.entry
            },
            ..valid
        },
        CountersinkCut {
            entry: FacePick {
                normal: [1e-6, 0., 1.],
                ..valid.entry
            },
            ..valid
        },
        CountersinkCut {
            entry: FacePick {
                point_m: [0.012, 0.009, 0.009],
                ..valid.entry
            },
            ..valid
        },
        CountersinkCut {
            entry: FacePick {
                point_m: [f64::NAN, 0.009, 0.01],
                ..valid.entry
            },
            ..valid
        },
    ]);
    bad.push(CountersinkCut {
        entry: FacePick {
            point_m: [0.012, 0.009, 0.01 + 5e-10],
            ..valid.entry
        },
        ..valid
    });
    for c in bad {
        assert!(
            k.cut_countersunk_stock(base.clone(), &c).is_err(),
            "accepted {c:?}"
        );
    }
    assert!(k.cut_countersunk_stock(KernelBody::new(0), &valid).is_err());
    let mut curved = rectangle();
    curved.points.clear();
    curved.circle = Some(SolvedCircle {
        center_m: [0., 0.],
        radius_m: 0.02,
    });
    let cylinder = k
        .extrude(
            k.make_wire_from_sketch(&curved).unwrap(),
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.01),
            },
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.],
        )
        .unwrap();
    assert!(k.cut_countersunk_stock(cylinder, &valid).is_err());
    assert_eq!(before, k.mass_properties(&base, 1.).unwrap());
}

pub fn negative_extrusion(k: &dyn GeometryKernel, mock: bool) {
    let mut s = rectangle();
    s.placement = Some(SketchPlacement {
        origin_m: [0., 0., 0.01],
        ..SketchPlacement::global_xy()
    });
    let stock = k
        .extrude(
            k.make_wire_from_sketch(&s).unwrap(),
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.01),
            },
            ExtrudeOperation::NewBody,
            None,
            [0., 0., -1.],
        )
        .unwrap();
    let c = input(false, false);
    let result = k.cut_countersunk_stock(stock, &c).unwrap();
    let (v, center, _) = oracle(&c);
    close(k.mass_properties(&result, 1.).unwrap().volume_m3, v, 2e-13);
    let q = TessellationSettings {
        linear_deflection: 1e-6,
        angular_deflection_deg: 6.,
    };
    let (mv, mc) = evidence::integrated(&k.tessellate(&result, &q).unwrap(), mock);
    close(mv, v, 2e-9);
    for (a, b) in mc.into_iter().zip(center) {
        close(a, b, 2e-6);
    }
}
