use opencad_core::{Length, TopoRefId};
use opencad_geometry::*;
use std::collections::BTreeSet;
#[path = "mesh_evidence.rs"]
mod evidence;
pub fn stock(k: &dyn GeometryKernel, scale: f64) -> KernelBody {
    cylinder(k, 0.01 * scale, 0.02 * scale)
}
fn cylinder(k: &dyn GeometryKernel, radius: f64, height: f64) -> KernelBody {
    let sketch = SolvedSketch {
        profile_ref: "stock".into(),
        points: vec![],
        circle: Some(SolvedCircle {
            center_m: [0.; 2],
            radius_m: radius,
        }),
        closed: true,
        ellipse: None,
        cubic_bezier: None,
        segments: vec![],
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
pub fn input(
    edge: u64,
    scale: f64,
    p: RigidTransform,
    variable: bool,
) -> FacetedCircularFilletInput {
    FacetedCircularFilletInput {
        end_edge_id: edge,
        origin_m: p.translation_m,
        phase_x: p.rotation.map(|r| r[0]),
        axis: p.rotation.map(|r| r[2]),
        cylinder_radius: Length::from_meters(0.01 * scale),
        cylinder_height: Length::from_meters(0.02 * scale),
        minimum_radius: Length::from_meters(0.0005 * scale),
        maximum_radius: Length::from_meters(if variable { 0.0015 } else { 0.0005 } * scale),
        mesh_precision: CircularFilletMeshPrecision {
            maximum_position_error: Length::from_meters(4e-4 * scale),
            maximum_normal_error: 0.8,
            max_vertices: 33026,
            max_triangles: 66048,
        },
        grid_precision: CircularFilletGridPrecision {
            maximum_position_error: Length::from_meters(4e-4 * scale),
            maximum_normal_error: 0.8,
        },
        volume_precision: CircularFilletGridVolumePrecision {
            maximum_reference_interval_width_m3: 1e-11 * scale.powi(3),
            maximum_absolute_difference_m3: 1e-7 * scale.powi(3),
            max_cells: 262144,
        },
        area_precision: CircularFilletGridAreaPrecision {
            maximum_reference_interval_width_m2: 1e-7 * scale.powi(2),
            maximum_absolute_difference_m2: 1e-5 * scale.powi(2),
            max_cells: 262144,
        },
        moment_precision: CircularFilletGridMomentPrecision {
            ideal: CircularFilletMomentPrecision {
                maximum_volume_interval_width_m3: 1e-11 * scale.powi(3),
                maximum_first_moment_interval_width_m4: 1e-12 * scale.powi(4),
                maximum_centroid_interval_width: Length::from_meters(1e-6 * scale),
                max_cells: 262144,
            },
            maximum_absolute_first_moment_difference_m4: 1e-9 * scale.powi(4),
            maximum_absolute_centroid_difference: Length::from_meters(2e-6 * scale),
        },
    }
}
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} != {b}, tol {t}");
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}
// Independent integer-nanometre tetrahedra; no production mass routine.
fn grid_mass(g: &CircularFilletGridSolid) -> (f64, f64, [f64; 3]) {
    let mut det = 0_i128;
    let mut first = [0_i128; 3];
    let mut area = 0.;
    for t in g.solid().triangles() {
        let p = t.map(|i| g.solid().vertices_m()[i as usize]);
        let [a, b, c] = p.map(|p| p.map(|x| (x / 1e-9).round() as i128));
        let d = a[0] * (b[1] * c[2] - b[2] * c[1])
            + a[1] * (b[2] * c[0] - b[0] * c[2])
            + a[2] * (b[0] * c[1] - b[1] * c[0]);
        det += d;
        for k in 0..3 {
            first[k] += d * (a[k] + b[k] + c[k]);
        }
        let n = evidence::cross(evidence::sub(p[1], p[0]), evidence::sub(p[2], p[0]));
        area += evidence::dot(n, n).sqrt() / 2.;
    }
    let v = det as f64 * 1e-27 / 6.;
    (v, area, first.map(|m| m as f64 * 1e-36 / 24. / v))
}
fn contacts(i: FacetedCircularFilletInput) -> [[f64; 3]; 4] {
    let r = i.cylinder_radius.meters();
    let h = i.cylinder_height.meters();
    let lo = i.minimum_radius.meters();
    let hi = i.maximum_radius.meters();
    [
        [r - hi, 0., h],
        [-r + lo, 0., h],
        [r, 0., h - hi],
        [-r, 0., h - lo],
    ]
    .map(|p| i.placement().unwrap().transform_point(p))
}
pub fn physical(k: &dyn GeometryKernel, mock: bool) {
    let moved = RigidTransform {
        rotation: [[0., -1., 0.], [0.6, 0., -0.8], [0.8, 0., 0.6]],
        translation_m: [0.05, -0.03, 0.08],
    };
    for (scale, variable, placement) in [
        (0.5, false, RigidTransform::identity()),
        (1., true, RigidTransform::identity()),
        (1., true, moved),
    ] {
        let base = stock(k, scale);
        let body = k.transform_body(base.clone(), placement).unwrap();
        let before = k.mass_properties(&body, 1000.).unwrap();
        let rims = k
            .cylindrical_stock_rims(&body, placement.rotation.map(|r| r[2]))
            .unwrap();
        let request = input(rims.end_edge_id, scale, placement, variable);
        let plan = request.prepare(&rims).unwrap();
        let replay = request.prepare(&rims).unwrap();
        assert_eq!(
            plan.grid().solid().vertices_m(),
            replay.grid().solid().vertices_m()
        );
        let out = k
            .grid_faceted_periodic_cosine(body.clone(), &request)
            .unwrap();
        let actual = k.mass_properties(&out, 1000.).unwrap();
        let (v, a, c) = grid_mass(plan.grid());
        close(actual.volume_m3, v, 1e-14 * scale.powi(3));
        close(actual.area_m2, a, 1e-11 * scale.powi(2));
        let centre = placement.transform_point(c);
        for (x, y) in actual.center_of_mass.into_iter().zip(centre) {
            close(x, y, 1e-9);
        }
        assert!(actual.volume_m3 < before.volume_m3);
        let ideal = plan.moment_comparison().ideal();
        assert!(actual.volume_m3 < ideal.volume_m3[0]); // Never substitute ideal mass.
        if !variable {
            let (r, h, f) = (0.01 * scale, 0.02 * scale, 0.0005 * scale);
            let pi = std::f64::consts::PI;
            let expected =
                pi * r * r * h - 2. * pi * ((r - f) * (1. - pi / 4.) * f * f + f.powi(3) / 6.);
            assert!(ideal.volume_m3[0] <= expected && expected <= ideal.volume_m3[1]);
            assert!(
                (actual.volume_m3 - expected).abs()
                    <= request.volume_precision.maximum_absolute_difference_m3 + 1e-14
            );
        }
        let mesh = k
            .tessellate(&out, &TessellationSettings::default())
            .unwrap();
        let ids = mesh
            .triangle_face_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), plan.grid().solid().triangles().len());
        assert_eq!(mesh.triangle_count(), plan.grid().solid().triangles().len());
        let (mv, mc) = evidence::integrated(&mesh, mock);
        close(mv, actual.volume_m3, 1e-10);
        for (a, b) in mc.into_iter().zip(centre) {
            close(a, b, 1e-8);
        }
        for point in contacts(request) {
            assert!(
                mesh.positions
                    .iter()
                    .any(|p| distance(p.map(f64::from), point) <= 1e-8),
                "missing physical contact {point:?}"
            );
        }
        let world = plan
            .grid()
            .solid()
            .vertices_m()
            .iter()
            .map(|p| placement.transform_point(*p))
            .collect::<Vec<_>>();
        let bbox = k.bounding_box(&out).unwrap();
        let gap = if mock { 0. } else { 1e-7 }; // pinned OCCT BRepBndLib/vertex tolerance
        for axis in 0..3 {
            close(
                bbox.min[axis],
                world.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min) - gap,
                2e-12,
            );
            close(
                bbox.max[axis],
                world
                    .iter()
                    .map(|p| p[axis])
                    .fold(f64::NEG_INFINITY, f64::max)
                    + gap,
                2e-12,
            );
        }
        assert!(k.cylindrical_stock_geometry(&out, request.axis).is_err());
        assert!(k.cylindrical_stock_rims(&out, request.axis).is_err());
        for id in [*ids.first().unwrap(), *ids.last().unwrap()] {
            k.assign_face_ref(&out, id, TopoRefId::new(format!("ref:owned:{id}")).unwrap())
                .unwrap();
        }
        assert!(k
            .assign_face_ref(&out, u64::MAX, TopoRefId::new("ref:outside").unwrap())
            .is_err());
        let after = k.mass_properties(&body, 1000.).unwrap();
        close(after.volume_m3, before.volume_m3, 1e-18);
        let again = k.cylindrical_stock_rims(&body, request.axis).unwrap();
        assert_eq!(rims, again);
    }
}
pub fn phase_and_seam(k: &dyn GeometryKernel) {
    let stock = stock(k, 1.);
    let rims = k.cylindrical_stock_rims(&stock, [0., 0., 1.]).unwrap();
    let identity = RigidTransform::identity();
    let request = input(rims.end_edge_id, 1., identity, true);
    let a = k
        .grid_faceted_periodic_cosine(stock.clone(), &request)
        .unwrap();
    let quarter = RigidTransform {
        rotation: [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
        ..identity
    };
    let rotated_stock = k.transform_body(stock.clone(), quarter).unwrap();
    let rotated_rims = k
        .cylindrical_stock_rims(&rotated_stock, [0., 0., 1.])
        .unwrap();
    let mut same = request;
    same.end_edge_id = rotated_rims.end_edge_id;
    let b = k
        .grid_faceted_periodic_cosine(rotated_stock, &same)
        .unwrap();
    let am = k.tessellate(&a, &TessellationSettings::default()).unwrap();
    let bm = k.tessellate(&b, &TessellationSettings::default()).unwrap();
    for p in contacts(request) {
        assert!(bm
            .positions
            .iter()
            .any(|q| distance(p, q.map(f64::from)) < 1e-8));
    }
    close(
        k.mass_properties(&a, 1.).unwrap().volume_m3,
        k.mass_properties(&b, 1.).unwrap().volume_m3,
        1e-18,
    );
    for phase in [[0., 1., 0.], [-1., 0., 0.]] {
        let mut changed = request;
        changed.phase_x = phase;
        let c = k
            .grid_faceted_periodic_cosine(stock.clone(), &changed)
            .unwrap();
        let cm = k.tessellate(&c, &TessellationSettings::default()).unwrap();
        close(
            k.mass_properties(&a, 1.).unwrap().volume_m3,
            k.mass_properties(&c, 1.).unwrap().volume_m3,
            1e-14,
        );
        assert!(cm
            .positions
            .iter()
            .all(|q| distance(contacts(request)[0], q.map(f64::from)) > 1e-5));
        assert_eq!(am.triangle_count(), cm.triangle_count());
    } // A small authored phase must survive the native matrix conversion; the
      // old generic identity/axis-angle thresholds would erase this rotation.
    let mut tiny = request;
    tiny.phase_x = [1., 1e-9, 0.];
    let out = k.grid_faceted_periodic_cosine(stock, &tiny).unwrap();
    let before = k.mass_properties(&a, 1.).unwrap().center_of_mass;
    let expected = tiny.placement().unwrap().transform_point(before);
    let after = k.mass_properties(&out, 1.).unwrap().center_of_mass;
    for (x, y) in after.into_iter().zip(expected) {
        close(x, y, 1e-15); // Separate floating mass-summation allowance; phase signal is ~1e-14 m.
    }
}
pub fn refusals(k: &dyn GeometryKernel) {
    let body = stock(k, 1.);
    let rims = k.cylindrical_stock_rims(&body, [0., 0., 1.]).unwrap();
    let base = input(rims.end_edge_id, 1., RigidTransform::identity(), false);
    let before = k.mass_properties(&body, 1.).unwrap();
    let mut invalid = Vec::new();
    for id in [0, rims.start_edge_id, u64::MAX] {
        let mut p = base;
        p.end_edge_id = id;
        invalid.push(p);
    }
    for x in [0., -1., f64::NAN, f64::INFINITY] {
        let mut p = base;
        p.minimum_radius = Length::from_meters(x);
        invalid.push(p);
        let mut p = base;
        p.cylinder_radius = Length::from_meters(x);
        invalid.push(p);
        let mut p = base;
        p.mesh_precision.maximum_position_error = Length::from_meters(x);
        invalid.push(p);
    }
    let mut p = base;
    p.phase_x = [2., 0., 0.];
    invalid.push(p);
    let mut p = base;
    p.phase_x = [0., 0., 1.];
    invalid.push(p);
    let mut p = base;
    p.phase_x = [f64::NAN, 0., 0.];
    invalid.push(p);
    let mut p = base;
    p.axis = [0., 0., -1.];
    invalid.push(p);
    let mut p = base;
    p.axis = [0., 0., 2.];
    invalid.push(p);
    let mut p = base;
    p.origin_m = [0., 0., 1e-5];
    invalid.push(p);
    let mut p = base;
    p.cylinder_height = Length::from_meters(0.021);
    invalid.push(p);
    let mut p = base;
    p.maximum_radius = Length::from_meters(0.02);
    invalid.push(p);
    let mut p = base;
    p.mesh_precision.max_triangles = 1;
    invalid.push(p);
    let mut p = base;
    p.grid_precision.maximum_position_error = Length::from_meters(1e-30);
    invalid.push(p);
    let mut p = base;
    p.volume_precision.maximum_absolute_difference_m3 = 1e-30;
    invalid.push(p);
    let mut p = base;
    p.area_precision.maximum_absolute_difference_m2 = 1e-30;
    invalid.push(p);
    let mut p = base;
    p.moment_precision
        .maximum_absolute_first_moment_difference_m4 = 1e-30;
    invalid.push(p);
    let mut p = base;
    p.moment_precision.maximum_absolute_centroid_difference = Length::from_meters(1e-30);
    invalid.push(p);
    let mut p = base;
    p.volume_precision.max_cells = 3;
    invalid.push(p);
    let mut p = base;
    p.area_precision.max_cells = 3;
    invalid.push(p);
    let mut p = base;
    p.moment_precision.ideal.max_cells = 3;
    invalid.push(p);
    for field in 0..3 {
        let mut p = base;
        p.maximum_radius = Length::from_meters(0.0015);
        match field {
            0 => p.volume_precision.max_cells = 4,
            1 => p.area_precision.max_cells = 4,
            _ => p.moment_precision.ideal.max_cells = 4,
        }
        invalid.push(p);
    }
    for request in invalid {
        assert!(
            k.grid_faceted_periodic_cosine(body.clone(), &request)
                .is_err(),
            "accepted {request:?}"
        );
    }
    assert!(k
        .grid_faceted_periodic_cosine(KernelBody::new(u64::MAX), &base)
        .is_err());
    close(
        k.mass_properties(&body, 1.).unwrap().volume_m3,
        before.volume_m3,
        1e-18,
    );
    assert_eq!(k.cylindrical_stock_rims(&body, [0., 0., 1.]).unwrap(), rims);
    let decoy = cylinder(k, 0.005, 0.08);
    close(
        k.mass_properties(&decoy, 1.).unwrap().volume_m3,
        before.volume_m3,
        1e-14,
    );
    let decoy_rim = k.cylindrical_stock_rims(&decoy, [0., 0., 1.]).unwrap();
    let mut wrong = base;
    wrong.end_edge_id = decoy_rim.end_edge_id;
    assert!(k.grid_faceted_periodic_cosine(decoy, &wrong).is_err());
    let out = k.grid_faceted_periodic_cosine(body, &base).unwrap();
    assert!(k.grid_faceted_periodic_cosine(out, &base).is_err());
}
