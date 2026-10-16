//! ADR-094: actual GeometryKernel body storage, not standalone native diagnostics.
use opencad_core::{Length, TopoRefId};
use opencad_geometry::{
    BooleanOp, CircularFilletGridPrecision, CircularFilletGridSolid, CircularFilletMeshPrecision,
    CircularFilletReference, CountingGeometryKernel, FacePick, FilletEdgeSelector, GeometryKernel,
    KernelBody, MeshSet, MockGeometryKernel, RigidTransform, TessellationSettings,
};
use opencad_kernel_occt::OcctGeometryKernel;
use std::collections::{BTreeMap, BTreeSet};
fn grid(variable: bool, fine: bool) -> CircularFilletGridSolid {
    let position = if fine { 1e-4 } else { 4e-4 };
    let normal = if fine { 0.5 } else { 0.8 };
    CircularFilletReference::new(
        Length::from_meters(0.01),
        Length::from_meters(0.02),
        Length::from_meters(0.0005),
        Length::from_meters(if variable { 0.0015 } else { 0.0005 }),
    )
    .unwrap()
    .closed_reference_mesh(CircularFilletMeshPrecision {
        maximum_position_error: Length::from_meters(position),
        maximum_normal_error: normal,
        max_vertices: 33026,
        max_triangles: 66048,
    })
    .unwrap()
    .grid_solid(CircularFilletGridPrecision {
        maximum_position_error: Length::from_meters(position),
        maximum_normal_error: normal,
    })
    .unwrap()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f64; 3]) -> f64 {
    a.into_iter().map(|v| v * v).sum::<f64>().sqrt()
}
fn independent(g: &CircularFilletGridSolid) -> (f64, f64, [f64; 3]) {
    let mut determinant = 0_i128;
    let mut area = 0.;
    let mut moment = [0.; 3];
    for t in g.solid().triangles() {
        let p = t.map(|i| g.solid().vertices_m()[i as usize]);
        let [a, b, c] = p.map(|p| p.map(|x| (x / 1e-9).round() as i128));
        let d = a[0] * (b[1] * c[2] - b[2] * c[1])
            + a[1] * (b[2] * c[0] - b[0] * c[2])
            + a[2] * (b[0] * c[1] - b[1] * c[0]);
        determinant += d;
        area += norm(cross(
            std::array::from_fn(|k| p[1][k] - p[0][k]),
            std::array::from_fn(|k| p[2][k] - p[0][k]),
        )) / 2.;
        for k in 0..3 {
            moment[k] += d as f64 * 1e-27 / 6. * (p[0][k] + p[1][k] + p[2][k]) / 4.;
        }
    }
    let volume = determinant as f64 * 1e-27 / 6.;
    (volume, area, moment.map(|x| x / volume))
}
fn closed(mesh: &MeshSet, expected: usize) {
    assert_eq!(mesh.triangle_count(), expected);
    assert!(mesh.has_triangle_face_ids());
    assert_eq!(
        mesh.triangle_face_ids.iter().collect::<BTreeSet<_>>().len(),
        expected
    );
    let key = |i: u32| mesh.positions[i as usize].map(|v| if v == 0. { 0 } else { v.to_bits() });
    let mut edges: BTreeMap<_, Vec<bool>> = BTreeMap::new();
    for t in mesh.indices.chunks_exact(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (key(a), key(b));
            edges.entry((a.min(b), a.max(b))).or_default().push(a < b);
        }
    }
    assert!(edges.values().all(|v| v.len() == 2 && v[0] != v[1]));
}
fn placement() -> RigidTransform {
    RigidTransform {
        translation_m: [0.1, -0.2, 0.3],
        rotation: [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
    }
}
#[test]
fn both_kernels_store_real_grid_mass_facets_bounds_and_rigid_placement() {
    for (variable, fine) in [(false, false), (true, true)] {
        let g = grid(variable, fine);
        let (v, a, center) = independent(&g);
        let mock = MockGeometryKernel::with_analytic_primitives();
        let native = OcctGeometryKernel::new();
        for kernel in [&mock as &dyn GeometryKernel, &native] {
            let body = kernel.import_circular_fillet_grid(&g).unwrap();
            let m = kernel.mass_properties(&body, 2500.).unwrap();
            assert!((m.volume_m3 - v).abs() < 1e-14);
            assert!((m.area_m2 - a).abs() < 1e-10);
            assert!((m.mass_kg - 2500. * v).abs() < 2.5e-11);
            assert!(m
                .center_of_mass
                .into_iter()
                .zip(center)
                .all(|(x, y)| (x - y).abs() < 1e-9));
            let bounds = kernel.bounding_box(&body).unwrap();
            let exact = g.solid().bounding_box();
            assert!(bounds
                .min
                .into_iter()
                .zip(exact.min)
                .chain(bounds.max.into_iter().zip(exact.max))
                .all(|(x, y)| (x - y).abs() < 2e-7));
            let mesh = kernel
                .tessellate(&body, &TessellationSettings::default())
                .unwrap();
            closed(&mesh, g.solid().triangles().len());
            for p in &mesh.positions {
                assert!(g
                    .solid()
                    .vertices_m()
                    .iter()
                    .any(|q| (0..3).all(|k| (f64::from(p[k]) - q[k]).abs() < 5e-9)));
            }
            let id = mesh.triangle_face_ids[0];
            kernel
                .assign_face_ref(&body, id, TopoRefId::new("ref:facet").unwrap())
                .unwrap();
            let moved = kernel.transform_body(body.clone(), placement()).unwrap();
            let mm = kernel.mass_properties(&moved, 2500.).unwrap();
            assert!((mm.volume_m3 - v).abs() < 1e-14);
            assert!((mm.area_m2 - a).abs() < 1e-10);
            assert!(mm
                .center_of_mass
                .into_iter()
                .zip(placement().transform_point(center))
                .all(|(x, y)| (x - y).abs() < 1e-9));
            let moved_mesh = kernel
                .tessellate(&moved, &TessellationSettings::default())
                .unwrap();
            closed(&moved_mesh, g.solid().triangles().len());
            for p in &moved_mesh.positions {
                assert!(g
                    .solid()
                    .vertices_m()
                    .iter()
                    .map(|q| placement().transform_point(*q))
                    .any(|q| (0..3).all(|k| (f64::from(p[k]) - q[k]).abs() < 3e-8)));
            }
            let unchanged = kernel.mass_properties(&body, 2500.).unwrap();
            assert!((unchanged.volume_m3 - m.volume_m3).abs() < 1e-18);
        }
    }
}
#[test]
fn owned_faces_unsupported_supports_and_invalid_repeated_compound_placement_preserve_bodies() {
    let g = grid(true, true);
    let smaller = grid(false, false);
    let mock = MockGeometryKernel::with_analytic_primitives();
    let native = OcctGeometryKernel::new();
    for (index, kernel) in [&mock as &dyn GeometryKernel, &native]
        .into_iter()
        .enumerate()
    {
        let body = kernel.import_circular_fillet_grid(&g).unwrap();
        let other = kernel.import_circular_fillet_grid(&smaller).unwrap();
        let source = kernel
            .tessellate(&body, &TessellationSettings::default())
            .unwrap();
        let mass = kernel.mass_properties(&body, 1.).unwrap();
        assert!(kernel
            .assign_face_ref(
                &other,
                *source.triangle_face_ids.iter().max().unwrap(),
                TopoRefId::new("ref:foreign").unwrap()
            )
            .is_err());
        assert!(kernel
            .cylindrical_stock_geometry(&body, [0., 0., 1.])
            .is_err());
        assert!(kernel.cylindrical_stock_rims(&body, [0., 0., 1.]).is_err());
        assert!(kernel
            .rectangular_stock_geometry(
                &body,
                FacePick {
                    point_m: [0., 0., 0.02],
                    normal: [0., 0., 1.]
                }
            )
            .is_err());
        assert!(kernel
            .mass_properties(&KernelBody::new(u64::MAX), 1.)
            .is_err());
        let moved = kernel.transform_body(body.clone(), placement()).unwrap();
        let compound = kernel.make_compound(&[body.clone(), moved.clone()]);
        if index == 1 {
            assert!(
                compound.is_ok(),
                "native compound must exercise member validation"
            );
        }
        for target in std::iter::once(body.clone())
            .chain(std::iter::once(moved))
            .chain(compound.ok())
        {
            for rotation in [
                [[-1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                [[2., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            ] {
                assert!(kernel
                    .transform_body(
                        target.clone(),
                        RigidTransform {
                            translation_m: [0.; 3],
                            rotation
                        }
                    )
                    .is_err());
            }
            let mut invalid = placement();
            invalid.translation_m[0] = f64::NAN;
            assert!(kernel.transform_body(target, invalid).is_err());
        }
        let after = kernel.mass_properties(&body, 1.).unwrap();
        assert!((after.volume_m3 - mass.volume_m3).abs() < 1e-18);
        assert_eq!(
            source,
            kernel
                .tessellate(&body, &TessellationSettings::default())
                .unwrap()
        );
    }
}
#[test]
fn analytic_mock_remains_opt_in_and_refuses_unsupported_edits_without_plumbing_fallback() {
    let g = grid(false, false);
    let legacy = MockGeometryKernel::new();
    assert!(legacy.import_circular_fillet_grid(&g).is_err());
    let mock = MockGeometryKernel::with_analytic_primitives();
    assert!(mock.import_triangle_solid(g.solid()).is_err());
    let counted = CountingGeometryKernel::new(&mock);
    let body = counted.import_circular_fillet_grid(&g).unwrap();
    assert_eq!(counted.call_count(), 1);
    let before = mock.mass_properties(&body, 1.).unwrap();
    assert!(mock
        .fillet_edges(body.clone(), 0.0001, FilletEdgeSelector::All)
        .is_err());
    assert!(mock
        .boolean(body.clone(), body.clone(), BooleanOp::Subtract)
        .is_err());
    assert!(mock.circular_edge_geometry(&body, 1).is_err());
    for value in [0., f64::NAN, f64::INFINITY] {
        assert!(mock
            .tessellate(
                &body,
                &TessellationSettings {
                    linear_deflection: value,
                    ..Default::default()
                }
            )
            .is_err());
    }
    let after = mock.mass_properties(&body, 1.).unwrap();
    assert!((after.volume_m3 - before.volume_m3).abs() < 1e-18);
    let again = MockGeometryKernel::with_analytic_primitives();
    let same = again.import_circular_fillet_grid(&g).unwrap();
    assert_eq!(
        mock.tessellate(&body, &Default::default()).unwrap(),
        again.tessellate(&same, &Default::default()).unwrap()
    );
}

#[test]
fn volume_comparison_encloses_actual_backend_mass_without_substituting_ideal_values() {
    let g = grid(true, false);
    let p = opencad_geometry::CircularFilletGridVolumePrecision {
        maximum_reference_interval_width_m3: 1e-12,
        maximum_absolute_difference_m3: 1e-7,
        max_cells: 262144,
    };
    let c = g.volume_comparison(p).unwrap();
    let mock = MockGeometryKernel::with_analytic_primitives();
    let native = OcctGeometryKernel::new();
    let independent_grid_volume = independent(&g).0;
    for kernel in [&mock as &dyn GeometryKernel, &native] {
        let body = kernel.import_circular_fillet_grid(&g).unwrap();
        let actual = kernel.mass_properties(&body, 1.).unwrap().volume_m3;
        assert!((actual - independent_grid_volume).abs() < 1e-14);
        assert!(
            c.grid_signed_volume_flux_m3()[0] - 1e-14 <= actual
                && actual <= c.grid_signed_volume_flux_m3()[1] + 1e-14
        );
        for ideal in c.ideal_volume_m3() {
            let delta = actual - ideal;
            assert!(
                c.signed_difference_m3()[0] - 1e-14 <= delta
                    && delta <= c.signed_difference_m3()[1] + 1e-14
            );
            assert!(delta.abs() > 1e-9, "backend must retain actual grid mass");
        }
        let mut tight = p;
        tight.maximum_absolute_difference_m3 = 1e-12;
        assert!(g.volume_comparison(tight).is_err());
        assert!((kernel.mass_properties(&body, 1.).unwrap().volume_m3 - actual).abs() < 1e-18);
    }
}

#[test]
fn ideal_area_never_replaces_actual_faceted_area_in_either_kernel() {
    for variable in [false, true] {
        let g = grid(variable, false);
        let r = CircularFilletReference::new(
            Length::from_meters(0.01),
            Length::from_meters(0.02),
            Length::from_meters(0.0005),
            Length::from_meters(if variable { 0.0015 } else { 0.0005 }),
        )
        .unwrap();
        let ideal = r.surface_area_bounds_m2(1e-7, 262144).unwrap();
        let actual_grid_area = independent(&g).1;
        let mock = MockGeometryKernel::with_analytic_primitives();
        let native = OcctGeometryKernel::new();
        for kernel in [&mock as &dyn GeometryKernel, &native] {
            let body = kernel.import_circular_fillet_grid(&g).unwrap();
            let before = kernel.mass_properties(&body, 1.).unwrap();
            // Numerical native conversion allowance; not an area certificate.
            assert!((before.area_m2 - actual_grid_area).abs() < 1e-10);
            for bound in [ideal.body_lower_m2, ideal.body_upper_m2] {
                assert!((before.area_m2 - bound).abs() > 1e-7);
            }
            assert!(r.surface_area_bounds_m2(f64::from_bits(1), 4).is_err());
            let after = kernel.mass_properties(&body, 1.).unwrap();
            assert!((before.area_m2 - after.area_m2).abs() < 1e-18);
            assert!((before.volume_m3 - after.volume_m3).abs() < 1e-18);
        }
    }
}
