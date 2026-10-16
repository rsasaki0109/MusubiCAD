//! ADR-099: independent integer-grid moments and analytic constant oracle.
use opencad_core::Length;
use opencad_geometry::{
    CircularFilletGridMomentPrecision as Precision, CircularFilletGridPrecision,
    CircularFilletGridSolid, CircularFilletMeshPrecision, CircularFilletMomentPrecision,
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
        ideal: CircularFilletMomentPrecision {
            maximum_volume_interval_width_m3: 1e-11 * scale.powi(3),
            maximum_first_moment_interval_width_m4: 1e-12 * scale.powi(4),
            maximum_centroid_interval_width: Length::from_meters(1e-6 * scale),
            max_cells: 262144,
        },
        maximum_absolute_first_moment_difference_m4: 1e-9 * scale.powi(4),
        maximum_absolute_centroid_difference: Length::from_meters(2e-6 * scale),
    }
}
// All determinant/moment products and accumulation occur as exact i128 integer
// nanometres BEFORE the independent numerical oracle's final SI conversion.
fn integer_moments(g: &CircularFilletGridSolid) -> (f64, [f64; 3], [f64; 3]) {
    let mut det = 0_i128;
    let mut moment = [0_i128; 3];
    for t in g.solid().triangles() {
        let [a, b, c] =
            t.map(|i| g.solid().vertices_m()[i as usize].map(|x| (x / 1e-9).round() as i128));
        let d = a[0] * (b[1] * c[2] - b[2] * c[1])
            + a[1] * (b[2] * c[0] - b[0] * c[2])
            + a[2] * (b[0] * c[1] - b[1] * c[0]);
        det += d;
        for k in 0..3 {
            moment[k] += d * (a[k] + b[k] + c[k]);
        }
    }
    let v = det as f64 * 1e-27 / 6.;
    let m = moment.map(|x| x as f64 * 1e-36 / 24.);
    (v, m, m.map(|x| x / v))
}
fn encloses(i: [f64; 2], x: f64, tol: f64) {
    assert!(i[0].is_finite() && i[1].is_finite() && i[0] <= i[1]);
    assert!(i[0] - tol <= x && x <= i[1] + tol, "{i:?} excludes {x}");
}
#[test]
fn actual_grid_fluxes_and_constant_analytic_moments_agree_at_scales() {
    for scale in [0.1_f64, 1., 10.] {
        let g = grid(scale, false, false);
        let c = g.moment_comparison(precision(scale)).unwrap();
        let (v, m, centre) = integer_moments(&g);
        let (r, h, f) = (0.01 * scale, 0.02 * scale, 0.0005 * scale);
        let pi = std::f64::consts::PI;
        let removed = 2. * pi * f * f * ((r - f) * (1. - pi / 4.) + f / 6.);
        let ideal_v = pi * r * r * h - removed;
        let correction = pi * f.powi(3) * ((r - f) * (5. / 3. - pi / 2.) + f / 12.);
        let mz = h * (ideal_v - pi * r * r * h / 2.) + correction;
        let ideal_m = [0., 0., mz];
        let ideal_c = ideal_m.map(|x| x / ideal_v);
        encloses(c.grid_signed_volume_flux_m3(), v, 1e-18 * scale.powi(3));
        for k in 0..3 {
            let mtol = 1e-20 * scale.powi(4);
            let ctol = 1e-12 * scale;
            encloses(c.grid_signed_first_moments_m4()[k], m[k], mtol);
            encloses(c.grid_centroid_quotient_m()[k], centre[k], ctol);
            encloses(c.ideal().first_moments_m4[k], ideal_m[k], mtol);
            encloses(
                c.signed_first_moment_difference_m4()[k],
                m[k] - ideal_m[k],
                mtol,
            );
            encloses(
                c.signed_centroid_difference_m()[k],
                centre[k] - ideal_c[k],
                ctol,
            );
            assert!(
                c.absolute_first_moment_difference_upper_m4()[k] + mtol
                    >= (m[k] - ideal_m[k]).abs()
            );
            assert!(
                c.absolute_centroid_difference_upper_m()[k] + ctol
                    >= (centre[k] - ideal_c[k]).abs()
            );
        }
        assert_eq!(c.ideal().volume_cells, 0);
        assert_eq!(c.ideal().moment_cells, 0);
    }
}
#[test]
fn variable_binding_grid_refinement_and_replay_keep_actual_signed_axes() {
    let g = grid(1., true, false);
    let fine = grid(1., true, true);
    let a = g.moment_comparison(precision(1.)).unwrap();
    let b = fine.moment_comparison(precision(1.)).unwrap();
    let constant = grid(1., false, false)
        .moment_comparison(precision(1.))
        .unwrap();
    assert_eq!(a.ideal().first_moments_m4, b.ideal().first_moments_m4);
    assert!(a.ideal().first_moments_m4[0][1] < 0.);
    assert_eq!(a.ideal().first_moments_m4[1], [0.; 2]);
    assert_eq!(constant.ideal().first_moments_m4[0], [0.; 2]);
    assert!(a.grid_centroid_quotient_m()[1][0] > 0.); // Stored grid need not share ideal symmetry.
    assert!(
        b.absolute_centroid_difference_upper_m()[2] < a.absolute_centroid_difference_upper_m()[2]
    );
    let (_, m, c) = integer_moments(&g);
    for k in 0..3 {
        encloses(a.grid_signed_first_moments_m4()[k], m[k], 1e-20);
        encloses(a.grid_centroid_quotient_m()[k], c[k], 1e-12);
        for end in a.ideal().first_moments_m4[k] {
            encloses(a.signed_first_moment_difference_m4()[k], m[k] - end, 1e-20);
        }
        for end in a.ideal().centroid_m[k] {
            encloses(a.signed_centroid_difference_m()[k], c[k] - end, 1e-12);
        }
    }
    assert!(a.ideal().volume_cells >= 4 && a.ideal().moment_cells >= 4);
    let replay = g.moment_comparison(precision(1.)).unwrap();
    assert_eq!(format!("{a:?}"), format!("{replay:?}"));
}
#[test]
fn invalid_widths_work_and_independent_error_budgets_refuse_without_mutation() {
    // Constant reference makes invalid/budget checks inexpensive; separate real
    // variable work exhaustion must still refuse rather than partially return.
    let g = grid(1., false, false);
    let before = g.solid().vertices_m().to_vec();
    for x in [0., -1., f64::NAN, f64::INFINITY] {
        for field in 0..5 {
            let mut p = precision(1.);
            match field {
                0 => p.maximum_absolute_first_moment_difference_m4 = x,
                1 => p.maximum_absolute_centroid_difference = Length::from_meters(x),
                2 => p.ideal.maximum_volume_interval_width_m3 = x,
                3 => p.ideal.maximum_first_moment_interval_width_m4 = x,
                _ => p.ideal.maximum_centroid_interval_width = Length::from_meters(x),
            }
            assert!(g.moment_comparison(p).is_err());
        }
    }
    for cap in [0, 3, 262145, usize::MAX] {
        let mut p = precision(1.);
        p.ideal.max_cells = cap;
        assert!(g.moment_comparison(p).is_err());
    }
    let mut p = precision(1.);
    p.maximum_absolute_first_moment_difference_m4 = 1e-20;
    assert!(g.moment_comparison(p).is_err());
    let mut p = precision(1.);
    p.maximum_absolute_centroid_difference = Length::from_meters(1e-12);
    assert!(g.moment_comparison(p).is_err());
    let mut p = precision(1.);
    p.ideal.max_cells = 4;
    assert!(grid(1., true, false).moment_comparison(p).is_err());
    assert_eq!(g.solid().vertices_m(), before);
    let a = g.moment_comparison(precision(1.)).unwrap();
    let mut p = precision(1.);
    p.maximum_absolute_first_moment_difference_m4 = a
        .absolute_first_moment_difference_upper_m4()
        .into_iter()
        .fold(0., f64::max);
    p.maximum_absolute_centroid_difference = Length::from_meters(
        a.absolute_centroid_difference_upper_m()
            .into_iter()
            .fold(0., f64::max),
    );
    assert!(g.moment_comparison(p).is_ok()); // Inclusive limits.
}
