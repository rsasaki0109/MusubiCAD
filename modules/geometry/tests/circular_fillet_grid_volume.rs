//! ADR-095: independent grid determinants and analytic constant-limit volume.
use opencad_core::Length;
use opencad_geometry::{
    CircularFilletGridPrecision, CircularFilletGridSolid,
    CircularFilletGridVolumePrecision as Precision, CircularFilletMeshPrecision,
    CircularFilletReference,
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
        maximum_reference_interval_width_m3: 1e-12 * scale.powi(3),
        maximum_absolute_difference_m3: 1e-7 * scale.powi(3),
        max_cells: 262144,
    }
}
fn integer_volume(g: &CircularFilletGridSolid) -> f64 {
    let mut sum = 0_i128;
    for t in g.solid().triangles() {
        let [a, b, c] =
            t.map(|i| g.solid().vertices_m()[i as usize].map(|x| (x / 1e-9).round() as i128));
        sum += a[0] * (b[1] * c[2] - b[2] * c[1])
            + a[1] * (b[2] * c[0] - b[0] * c[2])
            + a[2] * (b[0] * c[1] - b[1] * c[0]);
    }
    sum as f64 * 1e-27 / 6.
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
fn constant_grid_flux_and_ideal_difference_enclose_independent_formulae_at_scales() {
    for scale in [0.1_f64, 1., 10.] {
        let g = grid(scale, false, false);
        let c = g.volume_comparison(precision(scale)).unwrap();
        let (r, h, f) = (0.01 * scale, 0.02 * scale, 0.0005 * scale);
        let ideal = std::f64::consts::PI * r * r * h
            - 2. * std::f64::consts::PI
                * ((r - f) * (1. - std::f64::consts::PI / 4.) * f * f + f * f * f / 6.);
        let actual = integer_volume(&g);
        let tol = 1e-19 * scale.powi(3);
        encloses(c.grid_signed_volume_flux_m3(), actual, tol);
        encloses(c.ideal_volume_m3(), ideal, tol);
        encloses(c.signed_difference_m3(), actual - ideal, tol);
        assert!(c.absolute_difference_upper_m3() + tol >= (actual - ideal).abs());
        assert!(c.relative_difference_upper() + 1e-12 >= ((actual - ideal) / ideal).abs());
        assert!(
            c.absolute_difference_upper_m3() <= precision(scale).maximum_absolute_difference_m3
        );
        assert!(
            c.grid_signed_volume_flux_m3()[1] - c.grid_signed_volume_flux_m3()[0]
                < 1e-17 * scale.powi(3)
        );
        assert_eq!(c.reference_cells(), 0);
        assert!(c.signed_difference_m3()[1] < 0.);
    }
}
#[test]
fn variable_comparison_keeps_its_reference_binding_and_certifies_refinement() {
    let coarse = grid(1., true, false);
    let fine = grid(1., true, true);
    let constant = grid(1., false, false);
    let a = coarse.volume_comparison(precision(1.)).unwrap();
    let b = fine.volume_comparison(precision(1.)).unwrap();
    let c = constant.volume_comparison(precision(1.)).unwrap();
    assert!(a.reference_cells() >= 4 && a.reference_cells() <= 262144);
    assert!(a.ideal_volume_m3()[1] - a.ideal_volume_m3()[0] <= 1e-12);
    // Same variable reference, different actual grids. No caller reference argument.
    assert_eq!(a.ideal_volume_m3(), b.ideal_volume_m3());
    assert!(c.ideal_volume_m3()[0] > a.ideal_volume_m3()[1]);
    assert!(b.absolute_difference_upper_m3() < 0.3 * a.absolute_difference_upper_m3());
    encloses(
        a.grid_signed_volume_flux_m3(),
        integer_volume(&coarse),
        1e-19,
    );
    encloses(b.grid_signed_volume_flux_m3(), integer_volume(&fine), 1e-19);
    let [lo, hi] = a.signed_difference_m3();
    let actual = integer_volume(&coarse);
    encloses([lo, hi], actual - a.ideal_volume_m3()[0], 1e-19);
    encloses([lo, hi], actual - a.ideal_volume_m3()[1], 1e-19);
    let mut wider = precision(1.);
    wider.maximum_reference_interval_width_m3 = 1e-10;
    let loose = coarse.volume_comparison(wider).unwrap();
    assert!(a.reference_cells() >= loose.reference_cells());
    assert!(
        a.ideal_volume_m3()[1] - a.ideal_volume_m3()[0]
            < loose.ideal_volume_m3()[1] - loose.ideal_volume_m3()[0]
    );
}
#[test]
fn invalid_precision_work_and_real_excess_discrepancy_refuse_without_changing_grid() {
    let g = grid(1., true, false);
    let before = g.solid().vertices_m().to_vec();
    for value in [0., -1., f64::NAN, f64::INFINITY] {
        let mut p = precision(1.);
        p.maximum_reference_interval_width_m3 = value;
        assert!(g.volume_comparison(p).is_err());
        let mut p = precision(1.);
        p.maximum_absolute_difference_m3 = value;
        assert!(g.volume_comparison(p).is_err());
    }
    for budget in [0, 3, 262145, usize::MAX] {
        let mut p = precision(1.);
        p.max_cells = budget;
        assert!(g.volume_comparison(p).is_err());
    }
    let mut p = precision(1.);
    p.max_cells = 4;
    p.maximum_reference_interval_width_m3 = 1e-30;
    assert!(g.volume_comparison(p).is_err());
    let mut p = precision(1.);
    p.maximum_absolute_difference_m3 = 1e-12;
    assert!(g.volume_comparison(p).is_err());
    assert_eq!(g.solid().vertices_m(), before);
    let c = g.volume_comparison(precision(1.)).unwrap();
    let mut p = precision(1.);
    p.maximum_absolute_difference_m3 = c.absolute_difference_upper_m3();
    assert!(g.volume_comparison(p).is_ok());
}
