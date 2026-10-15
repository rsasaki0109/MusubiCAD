//! Independent integer-grid volume and conversion displacement checks (ADR-093).
use opencad_core::Length;
use opencad_geometry::{
    CircularFilletGridPrecision as GridPrecision, CircularFilletMeshPrecision as MeshPrecision,
    CircularFilletReference, CircularFilletReferenceMesh,
};
fn mesh(scale: f64, variable: bool, fine: bool) -> CircularFilletReferenceMesh {
    CircularFilletReference::new(
        Length::from_meters(0.01 * scale),
        Length::from_meters(0.02 * scale),
        Length::from_meters(0.0005 * scale),
        Length::from_meters(if variable { 0.0015 } else { 0.0005 } * scale),
    )
    .unwrap()
    .closed_reference_mesh(MeshPrecision {
        maximum_position_error: Length::from_meters(if fine { 1e-4 } else { 4e-4 } * scale),
        maximum_normal_error: if fine { 0.5 } else { 0.8 },
        max_vertices: 33026,
        max_triangles: 66048,
    })
    .unwrap()
}
fn precision(scale: f64) -> GridPrecision {
    GridPrecision {
        maximum_position_error: Length::from_meters(4e-4 * scale),
        maximum_normal_error: 0.8,
    }
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(p: [f64; 3]) -> f64 {
    p.into_iter().map(|v| v * v).sum::<f64>().sqrt()
}
fn normal(p: [[f64; 3]; 3]) -> [f64; 3] {
    let n = cross(
        std::array::from_fn(|k| p[1][k] - p[0][k]),
        std::array::from_fn(|k| p[2][k] - p[0][k]),
    );
    n.map(|v| v / norm(n))
}
#[test]
fn grid_mass_matches_independent_integer_determinants_and_actual_triangle_area() {
    for (scale, variable, fine) in [
        (0.1, false, false),
        (1., false, true),
        (1., true, true),
        (10., true, false),
    ] {
        let m = mesh(scale, variable, fine);
        let g = m.grid_solid(precision(scale)).unwrap();
        let solid = g.solid();
        let mut determinant = 0_i128;
        let mut area = 0.;
        for t in solid.triangles() {
            let points = t.map(|i| solid.vertices_m()[i as usize]);
            let [a, b, c] = points.map(|p| p.map(|v| (v / 1e-9).round() as i128));
            determinant += a[0] * (b[1] * c[2] - b[2] * c[1])
                + a[1] * (b[2] * c[0] - b[0] * c[2])
                + a[2] * (b[0] * c[1] - b[1] * c[0]);
            area += norm(cross(
                std::array::from_fn(|k| points[1][k] - points[0][k]),
                std::array::from_fn(|k| points[2][k] - points[0][k]),
            )) / 2.;
        }
        let mass = solid.mass_properties(2.).unwrap();
        let expected = determinant as f64 * 1e-27 / 6.;
        assert!((mass.volume_m3 - expected).abs() < 1e-18 * scale.powi(3));
        assert!((mass.area_m2 - area).abs() < 1e-14 * scale.powi(2));
        assert!((mass.mass_kg - 2. * expected).abs() < 2e-18 * scale.powi(3));
        assert!((solid.bounding_box().max[2] - 0.02 * scale).abs() < 1e-9);
        assert!(g.position_error_upper_m() <= precision(scale).maximum_position_error.meters());
        assert!(g.normal_error_upper() <= 0.8);
    }
}
#[test]
fn complete_ordered_correspondence_covers_vertex_and_facet_changes_and_is_deterministic() {
    let m = mesh(1., true, true);
    let g = m.grid_solid(precision(1.)).unwrap();
    let again = m.grid_solid(precision(1.)).unwrap();
    let s = g.solid();
    assert_eq!(s.vertices_m(), again.solid().vertices_m());
    assert_eq!(s.triangles(), again.solid().triangles());
    let mut pairs = std::collections::BTreeMap::new();
    let mut maximum = 0_f64;
    let mut normals = 0_f64;
    for (a, b) in m.triangles().iter().zip(s.triangles()) {
        for (&i, &j) in a.iter().zip(b) {
            assert!(pairs.insert(i, j).map_or(true, |old| old == j));
            let d = norm(std::array::from_fn(|k| {
                m.vertices_m()[i as usize][k] - s.vertices_m()[j as usize][k]
            }));
            maximum = maximum.max(d);
            assert!(d <= g.grid_displacement_upper_m());
        }
        let na = normal(a.map(|i| m.vertices_m()[i as usize]));
        let nb = normal(b.map(|i| s.vertices_m()[i as usize]));
        let d = norm(std::array::from_fn(|k| na[k] - nb[k]));
        normals = normals.max(d);
        assert!(d <= g.grid_normal_change_upper() + 1e-14);
    }
    assert_eq!(pairs.len(), m.vertices_m().len());
    assert_eq!(
        pairs
            .values()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        s.vertices_m().len()
    );
    assert!(maximum > 1e-10 && g.grid_displacement_upper_m() < 1e-9);
    assert!(normals > 1e-9 && g.grid_normal_change_upper() < 0.01);
    assert!(
        g.position_error_upper_m() >= m.position_error_upper_m() + g.grid_displacement_upper_m()
    );
    assert!(g.normal_error_upper() >= m.normal_error_upper() + g.grid_normal_change_upper());
}
#[test]
fn invalid_final_quality_and_unchanged_stl_cap_refuse_without_mutation_or_coarsening() {
    let m = mesh(1., true, false);
    let before = m.vertices_m().to_vec();
    for value in [0., -1., f64::NAN, f64::INFINITY, m.position_error_upper_m()] {
        let mut p = precision(1.);
        p.maximum_position_error = Length::from_meters(value);
        assert!(m.grid_solid(p).is_err());
    }
    for value in [0., 1., f64::NAN, f64::INFINITY, m.normal_error_upper()] {
        let mut p = precision(1.);
        p.maximum_normal_error = value;
        assert!(m.grid_solid(p).is_err());
    }
    assert_eq!(before, m.vertices_m());
    assert!(m.grid_solid(precision(1.)).is_ok());
    let r = CircularFilletReference::new(
        Length::from_meters(0.01),
        Length::from_meters(0.02),
        Length::from_meters(0.0005),
        Length::from_meters(0.0015),
    )
    .unwrap();
    let large = r
        .closed_reference_mesh(MeshPrecision {
            maximum_position_error: Length::from_meters(2e-5),
            maximum_normal_error: 0.5,
            max_vertices: 33026,
            max_triangles: 66048,
        })
        .unwrap();
    assert!(large.triangles().len() > 10000);
    assert!(large.grid_solid(precision(1.)).is_err());
}
