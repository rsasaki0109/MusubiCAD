//! ADR-097: independent integer-grid triangle areas and analytic ideal area.
use opencad_core::Length;
use opencad_geometry::{
    CircularFilletGridAreaPrecision as Precision, CircularFilletGridPrecision,
    CircularFilletGridSolid, CircularFilletMeshPrecision, CircularFilletReference,
};
fn grid(scale: f64, variable: bool, fine: bool) -> CircularFilletGridSolid {
    let p = if fine { 1e-4 } else { 4e-4 } * scale;
    let n = if fine { 0.5 } else { 0.8 };
    CircularFilletReference::new(
        Length::from_meters(0.01 * scale),
        Length::from_meters(0.02 * scale),
        Length::from_meters(0.0005 * scale),
        Length::from_meters(if variable { 0.0015 } else { 0.0005 } * scale),
    )
    .unwrap()
    .closed_reference_mesh(CircularFilletMeshPrecision {
        maximum_position_error: Length::from_meters(p),
        maximum_normal_error: n,
        max_vertices: 33026,
        max_triangles: 66048,
    })
    .unwrap()
    .grid_solid(CircularFilletGridPrecision {
        maximum_position_error: Length::from_meters(p),
        maximum_normal_error: n,
    })
    .unwrap()
}
fn precision(scale: f64) -> Precision {
    Precision {
        maximum_reference_interval_width_m2: 1e-8 * scale.powi(2),
        maximum_absolute_difference_m2: 1e-5 * scale.powi(2),
        max_cells: 262144,
    }
}
// Integer differences/cross products precede floating square roots. Numerical
// oracle allowance is separate from the outward production certificate.
fn integer_area(g: &CircularFilletGridSolid) -> f64 {
    g.solid()
        .triangles()
        .iter()
        .map(|t| {
            let [a, b, c] =
                t.map(|i| g.solid().vertices_m()[i as usize].map(|x| (x / 1e-9).round() as i128));
            let b: [i128; 3] = std::array::from_fn(|k| b[k] - a[k]);
            let c: [i128; 3] = std::array::from_fn(|k| c[k] - a[k]);
            let cross = [
                b[1] * c[2] - b[2] * c[1],
                b[2] * c[0] - b[0] * c[2],
                b[0] * c[1] - b[1] * c[0],
            ];
            (cross.into_iter().map(|x| x * x).sum::<i128>() as f64).sqrt() * 1e-18 / 2.
        })
        .sum()
}
fn encloses(interval: [f64; 2], value: f64, tol: f64) {
    assert!(interval.into_iter().all(f64::is_finite));
    assert!(interval[0] <= interval[1]);
    assert!(
        interval[0] - tol <= value && value <= interval[1] + tol,
        "{interval:?} excludes {value}"
    );
}
#[test]
fn constant_grid_area_and_ideal_difference_enclose_independent_formulae_at_scales() {
    for scale in [0.1_f64, 1., 10.] {
        let g = grid(scale, false, false);
        let c = g.area_comparison(precision(scale)).unwrap();
        let (r, h, f) = (0.01 * scale, 0.02 * scale, 0.0005 * scale);
        let pi = std::f64::consts::PI;
        let ideal = 2. * pi * f * ((r - f) * pi / 2. + f)
            + 2. * pi * r * (h - f)
            + pi * (r - f).powi(2)
            + pi * r * r;
        let actual = integer_area(&g);
        let tol = 1e-16 * scale.powi(2);
        encloses(c.grid_unsigned_area_m2(), actual, tol);
        encloses(c.ideal_area_m2(), ideal, tol);
        encloses(c.signed_difference_m2(), actual - ideal, tol);
        assert!(c.absolute_difference_upper_m2() + tol >= (actual - ideal).abs());
        assert!(c.relative_difference_upper() + 1e-12 >= ((actual - ideal) / ideal).abs());
        assert!(
            c.absolute_difference_upper_m2() <= precision(scale).maximum_absolute_difference_m2
        );
        assert!(
            c.grid_unsigned_area_m2()[1] - c.grid_unsigned_area_m2()[0] < 1e-13 * scale.powi(2)
        );
        assert_eq!(c.reference_cells(), 0);
        assert!(c.signed_difference_m2()[1] < 0.);
    }
}
#[test]
fn variable_comparison_keeps_its_reference_binding_and_certifies_refinement() {
    let coarse = grid(1., true, false);
    let fine = grid(1., true, true);
    let constant = grid(1., false, false);
    let a = coarse.area_comparison(precision(1.)).unwrap();
    let b = fine.area_comparison(precision(1.)).unwrap();
    let c = constant.area_comparison(precision(1.)).unwrap();
    assert!(a.reference_cells() >= 4 && a.reference_cells() <= 262144);
    assert!(a.ideal_area_m2()[1] - a.ideal_area_m2()[0] <= 1e-8);
    // Same variable reference, different actual grids. No caller reference argument.
    assert_eq!(a.ideal_area_m2(), b.ideal_area_m2());
    assert!(c.ideal_area_m2()[0] > a.ideal_area_m2()[1]);
    assert!(b.absolute_difference_upper_m2() < 0.4 * a.absolute_difference_upper_m2());
    encloses(a.grid_unsigned_area_m2(), integer_area(&coarse), 1e-16);
    encloses(b.grid_unsigned_area_m2(), integer_area(&fine), 1e-16);
    let [lo, hi] = a.signed_difference_m2();
    let actual = integer_area(&coarse);
    encloses([lo, hi], actual - a.ideal_area_m2()[0], 1e-16);
    encloses([lo, hi], actual - a.ideal_area_m2()[1], 1e-16);
    let mut wider = precision(1.);
    wider.maximum_reference_interval_width_m2 = 1e-6;
    let loose = coarse.area_comparison(wider).unwrap();
    assert!(a.reference_cells() >= loose.reference_cells());
    assert!(
        a.ideal_area_m2()[1] - a.ideal_area_m2()[0]
            < loose.ideal_area_m2()[1] - loose.ideal_area_m2()[0]
    );
}
#[test]
fn invalid_precision_work_and_real_excess_discrepancy_refuse_without_changing_grid() {
    let g = grid(1., true, false);
    let before = g.solid().vertices_m().to_vec();
    for value in [0., -1., f64::NAN, f64::INFINITY] {
        let mut p = precision(1.);
        p.maximum_reference_interval_width_m2 = value;
        assert!(g.area_comparison(p).is_err());
        let mut p = precision(1.);
        p.maximum_absolute_difference_m2 = value;
        assert!(g.area_comparison(p).is_err());
    }
    for budget in [0, 3, 262145, usize::MAX] {
        let mut p = precision(1.);
        p.max_cells = budget;
        assert!(g.area_comparison(p).is_err());
    }
    let mut p = precision(1.);
    p.max_cells = 4;
    p.maximum_reference_interval_width_m2 = 1e-30;
    assert!(g.area_comparison(p).is_err());
    let mut p = precision(1.);
    p.maximum_absolute_difference_m2 = 1e-12;
    assert!(g.area_comparison(p).is_err());
    assert_eq!(g.solid().vertices_m(), before);
    let c = g.area_comparison(precision(1.)).unwrap();
    let mut p = precision(1.);
    p.maximum_absolute_difference_m2 = c.absolute_difference_upper_m2();
    assert!(g.area_comparison(p).is_ok());
}
