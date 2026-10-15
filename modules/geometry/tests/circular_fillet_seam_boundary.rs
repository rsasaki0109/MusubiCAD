//! Independent rational/analytic witnesses; no OCCT or native flag fixtures.
use opencad_core::Length;
use opencad_geometry::{
    CircularFilletRationalSnapshot as Surface, CircularFilletSeamSnapshot as Edge,
};
fn surface(p: [[f64; 3]; 3], w: [f64; 3]) -> Surface {
    Surface::from_millimeter_controls(&[p; 9], &[w; 9]).unwrap()
}
fn eval(p: [[f64; 3]; 3], w: [f64; 3], t: f64) -> [f64; 3] {
    let b = [(1. - t).powi(2), 2. * t * (1. - t), t * t];
    let den: f64 = (0..3).map(|i| b[i] * w[i]).sum();
    std::array::from_fn(|axis| {
        (0..3)
            .map(|i| b[i] * w[i] * p[i][axis] / 1000.)
            .sum::<f64>()
            / den
    })
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()
}
#[test]
fn analytic_circle_translations_scales_and_common_weight_gauges_are_enclosed() {
    for scale in [0.01, 1., 100.] {
        let p = [
            [2. * scale, 0., 5. * scale],
            [2. * scale, 2. * scale, 5. * scale],
            [0., 2. * scale, 5. * scale],
        ];
        let w = [1., std::f64::consts::FRAC_1_SQRT_2, 1.];
        let delta = 1e-7 * scale;
        let l = p.map(|q| [q[0] + delta, q[1], q[2]]);
        let r = p.map(|q| [q[0] - delta, q[1], q[2]]);
        let edge = Edge::from_millimeter_controls(&p, &w.map(|x| x * 8.)).unwrap();
        let b = edge
            .certify_boundary(
                &surface(l, w.map(|x| x * 0.125)),
                &surface(r, w.map(|x| x * 32.)),
                Length::from_meters(1e-6 * scale),
            )
            .unwrap();
        for t in [0., 0.017, 0.19, 0.5, 0.831, 1.] {
            let q = eval(p, w, t);
            assert!((q[0].hypot(q[1]) - 2. * scale / 1000.).abs() < 1e-15 * scale);
            assert!(distance(q, eval(l, w, t)) <= b.edge_to_left_gap_upper_m);
            assert!(distance(q, eval(r, w, t)) <= b.edge_to_right_gap_upper_m);
            assert!(distance(eval(l, w, t), eval(r, w, t)) <= b.left_to_right_gap_upper_m);
        }
        assert!(b.left_to_right_gap_upper_m >= 2. * delta / 1000.);
        // Each edge-to-face gap fits this budget, but the face-to-face gap does not.
        assert!(b.edge_to_left_gap_upper_m < 1.5 * delta / 1000.);
        assert!(b.edge_to_right_gap_upper_m < 1.5 * delta / 1000.);
        assert!(edge
            .certify_boundary(
                &surface(l, w),
                &surface(r, w),
                Length::from_meters(1.5 * delta / 1000.)
            )
            .is_err());
    }
}
#[test]
fn interior_poles_weights_and_wrong_units_refuse_despite_identical_endpoints() {
    let p = [[1., 0., 2.], [1., 1., 2.], [0., 1., 2.]];
    let w = [1., 0.75, 1.];
    let edge = Edge::from_millimeter_controls(&p, &w).unwrap();
    let s = surface(p, w);
    assert!(edge
        .certify_boundary(&s, &s, Length::from_meters(1e-12))
        .is_ok());
    let mut moved = p;
    moved[1][2] += 0.001;
    assert!(distance(eval(p, w, 0.), eval(moved, w, 0.)) < 1e-18);
    assert!(distance(eval(p, w, 1.), eval(moved, w, 1.)) < 1e-18);
    assert!(distance(eval(p, w, 0.5), eval(moved, w, 0.5)) > 1e-7);
    for bad in [
        surface(moved, w),
        surface(p, [1., 0.751, 1.]),
        surface(p.map(|row| row.map(|x| x / 1000.)), w),
    ] {
        assert!(edge
            .certify_boundary(&bad, &s, Length::from_meters(1e-12))
            .is_err());
        assert!(edge
            .certify_boundary(&s, &bad, Length::from_meters(1e-12))
            .is_err());
    }
    assert!(Edge::from_millimeter_controls(&moved, &w)
        .unwrap()
        .certify_boundary(&s, &s, Length::from_meters(1e-12))
        .is_err());
}
#[test]
fn actual_surface_boundary_rows_and_transverse_parameter_are_used() {
    let p = [[1., 0., 2.], [1., 1., 2.], [0., 1., 2.]];
    let w = [1., 0.75, 1.];
    let edge = Edge::from_millimeter_controls(&p, &w).unwrap();
    let mut lp = [[[100.; 3]; 3]; 9];
    lp[8] = p;
    let mut rp = [[[200.; 3]; 3]; 9];
    rp[0] = p;
    let left = Surface::from_millimeter_controls(&lp, &[w; 9]).unwrap();
    let right = Surface::from_millimeter_controls(&rp, &[w; 9]).unwrap();
    assert!(edge
        .certify_boundary(&left, &right, Length::from_meters(1e-12))
        .is_ok());
    assert!(edge
        .certify_boundary(&right, &left, Length::from_meters(1e-12))
        .is_err());
    rp[0].reverse();
    assert!(edge
        .certify_boundary(
            &left,
            &Surface::from_millimeter_controls(&rp, &[w; 9]).unwrap(),
            Length::from_meters(1e-12)
        )
        .is_err());
}
#[test]
fn invalid_data_overflow_and_unattainable_precision_refuse() {
    let p = [[1.; 3]; 3];
    let s = surface(p, [1.; 3]);
    let edge = Edge::from_millimeter_controls(&p, &[1.; 3]).unwrap();
    for max in [0., -1., f64::NAN, f64::INFINITY, 1e-30] {
        assert!(edge
            .certify_boundary(&s, &s, Length::from_meters(max))
            .is_err());
    }
    for w in [
        [0., 1., 1.],
        [-1., 1., 1.],
        [f64::NAN, 1., 1.],
        [f64::INFINITY; 3],
    ] {
        assert!(Edge::from_millimeter_controls(&p, &w).is_err());
    }
    for raw in [f64::NAN, f64::INFINITY] {
        assert!(Edge::from_millimeter_controls(&[[raw; 3]; 3], &[1.; 3]).is_err());
    }
    assert!(Edge::from_millimeter_controls(&[[f64::MAX; 3]; 3], &[f64::MAX; 3]).is_err());
    let tiny = Edge::from_millimeter_controls(&p, &[f64::from_bits(1); 3]).unwrap();
    assert!(tiny
        .certify_boundary(&s, &s, Length::from_meters(1e-12))
        .is_err());
}
