//! Independent polygon-frustum integrals and actual indexed-shell checks.
use opencad_core::Length;
use opencad_geometry::{
    CircularFilletMeshPrecision as Precision, CircularFilletReference as Reference,
};
use std::collections::BTreeMap;
fn reference(scale: f64, variable: bool) -> Reference {
    Reference::new(
        Length::from_meters(0.01 * scale),
        Length::from_meters(0.02 * scale),
        Length::from_meters(0.0005 * scale),
        Length::from_meters(if variable {
            0.0015 * scale
        } else {
            0.0005 * scale
        }),
    )
    .unwrap()
}
fn precision(scale: f64, fine: bool) -> Precision {
    Precision {
        maximum_position_error: Length::from_meters(if fine { 1e-4 * scale } else { 4e-4 * scale }),
        maximum_normal_error: if fine { 0.5 } else { 0.8 },
        max_vertices: 33026,
        max_triangles: 66048,
    }
}
fn theta(i: usize, n: usize) -> f64 {
    (i / n) as f64 * std::f64::consts::FRAC_PI_2 + 2. * ((i % n) as f64 / n as f64).atan()
}
fn constant_integrals(n: usize, scale: f64) -> [f64; 3] {
    let (r, h, f) = (0.01 * scale, 0.02 * scale, 0.0005 * scale);
    let mut s = 0.;
    let mut area = 0.;
    for i in 0..4 * n {
        let half = (theta(i + 1, n) - theta(i, n)) / 2.;
        s += (2. * half).sin() / 2.;
        area += 2. * half.sin() * r * (h - f);
        for j in 0..n {
            let a = 2. * (j as f64 / n as f64).atan();
            let b = 2. * ((j + 1) as f64 / n as f64).atan();
            let p = r - f + f * a.cos();
            let q = r - f + f * b.cos();
            let dz = f * (b.sin() - a.sin());
            area += half.sin() * (p + q) * ((q - p) * half.cos()).hypot(dz);
        }
    }
    area += s * (r * r + (r - f).powi(2));
    let mut v = r * r * (h - f);
    let mut mz = r * r * (h - f).powi(2) / 2.;
    for j in 0..n {
        let a = 2. * (j as f64 / n as f64).atan();
        let b = 2. * ((j + 1) as f64 / n as f64).atan();
        let p = r - f + f * a.cos();
        let q = r - f + f * b.cos();
        let z = h - f + f * a.sin();
        let dz = f * (b.sin() - a.sin());
        let q2 = p * p + p * q + q * q;
        v += dz * q2 / 3.;
        mz += z * dz * q2 / 3. + dz * dz * (p * p + 2. * p * q + 3. * q * q) / 12.;
    }
    [s * v, area, s * mz]
}
#[test]
fn constant_polygon_frustum_volume_area_and_first_moment_match_independent_formulae() {
    for scale in [0.1, 1., 10.] {
        let r = reference(scale, false);
        let mesh = r.closed_reference_mesh(precision(scale, true)).unwrap();
        let expected = constant_integrals(mesh.divisions_per_chart_axis(), scale);
        let m = mesh.boundary_integrals();
        for (expected, interval, tol) in [
            (expected[0], m.volume_m3, 1e-18 * scale.powi(3)),
            (expected[1], m.area_m2, 1e-13 * scale.powi(2)),
            (expected[2], m.first_moments_m4[2], 1e-18 * scale.powi(4)),
        ] {
            assert!(
                interval[0] - tol <= expected && expected <= interval[1] + tol,
                "{expected} {interval:?}"
            );
        }
        for axis in 0..2 {
            assert!(
                m.first_moments_m4[axis][0] <= 1e-18 * scale.powi(4)
                    && m.first_moments_m4[axis][1] >= -1e-18 * scale.powi(4)
            );
        }
    }
}
#[test]
fn variable_shell_has_shared_periodic_ids_all_groups_and_independent_volume_refinement() {
    let r = reference(1., true);
    let coarse = r.closed_reference_mesh(precision(1., false)).unwrap();
    let fine = r.closed_reference_mesh(precision(1., true)).unwrap();
    let again = r.closed_reference_mesh(precision(1., true)).unwrap();
    assert_eq!(fine.vertices_m(), again.vertices_m());
    assert_eq!(fine.triangles(), again.triangles());
    let mut edges: BTreeMap<(u32, u32), Vec<bool>> = BTreeMap::new();
    for t in fine.triangles() {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            edges.entry((a.min(b), a.max(b))).or_default().push(a < b);
        }
    }
    assert!(edges.values().all(|v| v.len() == 2 && v[0] != v[1]));
    assert_eq!(
        fine.vertices_m().len() + fine.triangles().len(),
        edges.len() + 2
    );
    assert_eq!(
        fine.triangle_face_groups()
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        (1..=10).collect()
    );
    let ideal = r.volume_bounds_m3(1e-12, 262144).unwrap();
    let midpoint = (ideal.body_lower_m3 + ideal.body_upper_m3) / 2.;
    let value =
        |m: opencad_geometry::CircularFilletMeshIntegrals| (m.volume_m3[0] + m.volume_m3[1]) / 2.;
    assert!(
        (value(fine.boundary_integrals()) - midpoint).abs()
            < 0.3 * (value(coarse.boundary_integrals()) - midpoint).abs()
    );
    assert!(fine.position_error_upper_m() <= precision(1., true).maximum_position_error.meters());
    assert!(fine.normal_error_upper() <= 0.5);
    // Untouched cylindrical supports, not polynomial side skirts.
    let n = fine.divisions_per_chart_axis();
    for i in 0..4 * n {
        for j in 0..=n {
            let p = fine.vertices_m()[i * (2 * n + 1) + j];
            assert!((p[0].hypot(p[1]) - 0.01).abs() < 1e-14);
        }
    }
}
#[test]
fn invalid_quality_and_exhausted_vertex_triangle_budgets_never_return_partial_shells() {
    let r = reference(1., true);
    let p = precision(1., true);
    for value in [0., -1., f64::NAN, f64::INFINITY, 1e-30] {
        let mut b = p;
        b.maximum_position_error = Length::from_meters(value);
        assert!(r.closed_reference_mesh(b).is_err());
    }
    for value in [0., 1., f64::NAN, 1e-30] {
        let mut b = p;
        b.maximum_normal_error = value;
        assert!(r.closed_reference_mesh(b).is_err());
    }
    for (v, t) in [
        (13, 66048),
        (33027, 66048),
        (33026, 23),
        (33026, 66049),
        (14, 24),
        (1100, 4000),
    ] {
        let mut b = p;
        b.max_vertices = v;
        b.max_triangles = t;
        assert!(r.closed_reference_mesh(b).is_err());
    }
}
