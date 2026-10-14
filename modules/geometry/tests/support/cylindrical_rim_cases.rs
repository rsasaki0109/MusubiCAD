use opencad_core::{Length, TopoRefId};
use opencad_geometry::*;
use std::f64::consts::PI;
#[path = "mesh_evidence.rs"]
mod evidence;

pub fn cylinder(k: &dyn GeometryKernel, radius: f64, height: f64) -> KernelBody {
    let sketch = SolvedSketch {
        profile_ref: "rim_stock".into(),
        points: Vec::new(),
        circle: Some(SolvedCircle {
            center_m: [0.012, 0.009],
            radius_m: radius,
        }),
        closed: true,
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: None,
    };
    k.extrude(
        k.make_wire_from_sketch(&sketch).unwrap(),
        ExtrudeExtent::Distance {
            length: Length::from_meters(height),
        },
        ExtrudeOperation::NewBody,
        None,
        [0., 0., 1.],
    )
    .unwrap()
}
fn close(a: f64, b: f64, tolerance: f64) {
    assert!(
        (a - b).abs() <= tolerance,
        "{a} != {b}, tolerance={tolerance}"
    );
}
pub fn physical<K: GeometryKernel>(k: &K, mock: bool) {
    let base = cylinder(k, 0.02, 0.03);
    let before = k.mass_properties(&base, 1.).unwrap();
    let transform = RigidTransform {
        rotation: [[0., -1., 0.], [0.6, 0., -0.8], [0.8, 0., 0.6]],
        translation_m: [0.05, -0.03, 0.08],
    };
    let moved = k.transform_body(base.clone(), transform).unwrap();
    for (body, placement) in [
        (base.clone(), RigidTransform::identity()),
        (moved.clone(), transform),
    ] {
        let positive = placement.rotation.map(|row| row[2]);
        let forward = k.cylindrical_stock_rims(&body, positive).unwrap();
        let quality = TessellationSettings {
            linear_deflection: 1e-6,
            angular_deflection_deg: 6.,
        };
        let before_mesh = k.tessellate(&body, &quality).unwrap();
        for reverse in [false, true] {
            let axis = positive.map(|v| if reverse { -v } else { v });
            let rims = k.cylindrical_stock_rims(&body, axis).unwrap();
            assert_eq!(rims, k.cylindrical_stock_rims(&body, axis).unwrap());
            if reverse {
                assert_eq!(rims.start_edge_id, forward.end_edge_id);
                assert_eq!(rims.end_edge_id, forward.start_edge_id);
                assert_eq!(rims.stock.start_face_id, forward.stock.end_face_id);
                assert_eq!(rims.stock.end_face_id, forward.stock.start_face_id);
            }
            for (id, local) in [
                (
                    rims.start_edge_id,
                    [0.012, 0.009, if reverse { 0.03 } else { 0. }],
                ),
                (
                    rims.end_edge_id,
                    [0.012, 0.009, if reverse { 0. } else { 0.03 }],
                ),
            ] {
                let circle = k.circular_edge_geometry(&body, id).unwrap();
                for (actual, expected) in circle
                    .center_m
                    .into_iter()
                    .zip(placement.transform_point(local))
                {
                    close(actual, expected, 2e-10);
                }
                close(circle.radius_m, 0.02, 1e-10);
                close(2. * PI * circle.radius_m, 0.04 * PI, 1e-9);
                let alignment: f64 = circle
                    .normal
                    .into_iter()
                    .zip(axis)
                    .map(|(a, b)| a * b)
                    .sum();
                close(alignment.abs(), 1., 1e-9);
                if !mock {
                    let ends = k.edge_endpoints(&body, id).unwrap();
                    for p in ends {
                        let relative: [f64; 3] = std::array::from_fn(|i| p[i] - circle.center_m[i]);
                        close(
                            relative.into_iter().zip(axis).map(|(a, b)| a * b).sum(),
                            0.,
                            1e-9,
                        );
                        close(
                            relative.into_iter().map(|a| a * a).sum::<f64>().sqrt(),
                            0.02,
                            1e-9,
                        );
                    }
                    for (a, b) in ends[0].into_iter().zip(ends[1]) {
                        close(a, b, 1e-9);
                    }
                }
            }
            for id in [
                rims.stock.start_face_id,
                rims.stock.end_face_id,
                rims.stock.lateral_face_id,
            ] {
                assert!(before_mesh.triangle_face_ids.contains(&id));
                k.assign_face_ref(&body, id, TopoRefId::new("ref:physical_rim_face").unwrap())
                    .unwrap();
            }
        }
        let mass = k.mass_properties(&body, 1.).unwrap();
        close(mass.volume_m3, PI * 0.02_f64.powi(2) * 0.03, 2e-12);
        close(mass.area_m2, 2. * PI * 0.02 * (0.02 + 0.03), 2e-10);
        for (a, b) in mass
            .center_of_mass
            .into_iter()
            .zip(placement.transform_point([0.012, 0.009, 0.015]))
        {
            close(a, b, 1e-9);
        }
        let (volume, com) = evidence::integrated(&before_mesh, mock);
        close(volume, mass.volume_m3, mass.volume_m3 * 0.0005);
        for (a, b) in com.into_iter().zip(mass.center_of_mass) {
            close(a, b, 2e-6);
        }
        assert_eq!(before_mesh, k.tessellate(&body, &quality).unwrap());
    }
    // Equal scalar volume does not identify a circle: this radius/height differ.
    let other = cylinder(k, 0.01, 0.12);
    close(
        k.mass_properties(&other, 1.).unwrap().volume_m3,
        before.volume_m3,
        2e-12,
    );
    let other_rims = k.cylindrical_stock_rims(&other, [0., 0., 1.]).unwrap();
    close(
        k.circular_edge_geometry(&other, other_rims.end_edge_id)
            .unwrap()
            .radius_m,
        0.01,
        1e-10,
    );
    if mock {
        for id in [forward_id(k, &base), other_rims.start_edge_id] {
            assert!(k.circular_edge_geometry(&moved, id).is_err());
        }
        assert!(k
            .circular_edge_geometry(&base, other_rims.end_edge_id)
            .is_err());
        assert!(k.circular_edge_geometry(&base, 5).is_err());
    }
    let counted = CountingGeometryKernel::new(k);
    assert_eq!(
        counted.cylindrical_stock_rims(&base, [0., 0., 1.]).unwrap(),
        k.cylindrical_stock_rims(&base, [0., 0., 1.]).unwrap()
    );
    assert_eq!(counted.call_count(), 1);
    assert_eq!(before, k.mass_properties(&base, 1.).unwrap());
}
fn forward_id(k: &dyn GeometryKernel, body: &KernelBody) -> u64 {
    k.cylindrical_stock_rims(body, [0., 0., 1.])
        .unwrap()
        .end_edge_id
}
pub fn refusals(k: &dyn GeometryKernel) {
    let base = cylinder(k, 0.02, 0.03);
    let before = k.mass_properties(&base, 1.).unwrap();
    let bounds = k.bounding_box(&base).unwrap();
    let mesh = k
        .tessellate(&base, &TessellationSettings::default())
        .unwrap();
    for axis in [
        [0., 0., 0.],
        [0., 0., 2.],
        [1., 0., 0.],
        [f64::NAN, 0., 1.],
        [0., f64::INFINITY, 1.],
    ] {
        assert!(k.cylindrical_stock_rims(&base, axis).is_err());
    }
    assert!(k
        .cylindrical_stock_rims(&KernelBody::new(0), [0., 0., 1.])
        .is_err());
    let draft = k
        .draft_cylindrical_stock(
            base.clone(),
            &CylindricalDraftInput {
                axis_hint: [0., 0., 1.],
                neutral_origin_m: [0., 0., 0.],
                neutral_normal: [0., 0., 1.],
                angle_rad: 0.1,
            },
        )
        .unwrap();
    assert!(k.cylindrical_stock_rims(&draft, [0., 0., 1.]).is_err());
    let compound = k.make_compound(&[base.clone(), base.clone()]);
    if let Ok(compound) = compound {
        assert!(k.cylindrical_stock_rims(&compound, [0., 0., 1.]).is_err());
    }
    let mut sketch = SolvedSketch {
        profile_ref: "block".into(),
        points: vec![[0., 0.], [0.02, 0.], [0.02, 0.01], [0., 0.01]],
        closed: true,
        circle: None,
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: None,
    };
    let wire = k.make_wire_from_sketch(&sketch).unwrap();
    let block = k
        .extrude(
            wire,
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.01),
            },
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.],
        )
        .unwrap();
    assert!(k.cylindrical_stock_rims(&block, [0., 0., 1.]).is_err());
    sketch.points.clear();
    sketch.circle = Some(SolvedCircle {
        center_m: [0., 0.],
        radius_m: 0.003,
    });
    let stock = k
        .extrude(
            k.make_wire_from_sketch(&sketch).unwrap(),
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.001),
            },
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.],
        )
        .unwrap();
    let profile =
        NominalMetricThread::new(MetricThreadSize::M6, ThreadHandedness::Right, 1).unwrap();
    let thread = k
        .cut_nominal_metric_thread(
            stock,
            &NominalThreadInput {
                profile,
                axis_hint: [0., 0., 1.],
                radial_axis: [1., 0., 0.],
            },
        )
        .unwrap();
    assert!(k.cylindrical_stock_rims(&thread, [0., 0., 1.]).is_err());
    assert_eq!(before, k.mass_properties(&base, 1.).unwrap());
    assert_eq!(bounds, k.bounding_box(&base).unwrap());
    assert_eq!(
        mesh,
        k.tessellate(&base, &TessellationSettings::default())
            .unwrap()
    );
}
