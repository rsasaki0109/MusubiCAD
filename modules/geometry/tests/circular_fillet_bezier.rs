use opencad_core::{Angle, Length};
use opencad_geometry::{CircularFilletBezierPatch, CircularFilletReference};
use std::f64::consts::FRAC_PI_2;

fn reference(scale: f64, constant: bool) -> CircularFilletReference {
    CircularFilletReference::new(
        Length::from_meters(0.01 * scale),
        Length::from_meters(0.02 * scale),
        Length::from_meters(0.0005 * scale),
        Length::from_meters(if constant { 0.0005 } else { 0.0015 } * scale),
    )
    .unwrap()
}
fn basis(x: f64) -> [f64; 4] {
    [
        (1. - x).powi(3),
        3. * x * (1. - x).powi(2),
        3. * x * x * (1. - x),
        x.powi(3),
    ]
}
fn point(p: CircularFilletBezierPatch, u: f64, v: f64) -> [f64; 3] {
    std::array::from_fn(|axis| {
        basis(u)
            .into_iter()
            .enumerate()
            .map(|(i, a)| {
                basis(v)
                    .into_iter()
                    .enumerate()
                    .map(|(j, b)| a * b * p.control_points_m[i][j][axis])
                    .sum::<f64>()
            })
            .sum()
    })
}
fn norm(x: [f64; 3]) -> f64 {
    x.into_iter().map(|x| x * x).sum::<f64>().sqrt()
}
fn difference(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn same(a: [f64; 3], b: [f64; 3]) {
    // Representation identity of shared poles, not a geometric comparison.
    assert_eq!(a.map(f64::to_bits), b.map(f64::to_bits));
}
#[test]
fn all_chart_bernstein_surfaces_stay_within_reference_error_bounds() {
    for constant in [true, false] {
        let r = reference(1., constant);
        for chart in 0..4 {
            for (v0, v1) in [(0., 0.005), (0.3, 0.305), (0.995, 1.)] {
                let p = r.bezier_patch(chart, 0.25, 0.255, v0, v1).unwrap();
                assert!(p.position_error_upper_m < 1e-6);
                assert!(
                    p.position_error_upper_m
                        >= p.reference_to_bilinear_error_upper_m + p.control_net_deviation_upper_m
                );
                for u in [0_f64, 0.25, 0.5, 0.75, 1.] {
                    for v in [0., 0.25, 0.5, 0.75, 1.] {
                        let expected = r
                            .sample(
                                Angle::from_radians(
                                    f64::from(chart) * FRAC_PI_2 + 2. * (0.25 + u * 0.005).atan(),
                                ),
                                Angle::from_radians(2. * (v0 + v * (v1 - v0)).atan()),
                            )
                            .unwrap()
                            .point_m;
                        assert!(
                            norm(difference(point(p, u, v), expected))
                                <= p.position_error_upper_m + 1e-14
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn constant_contact_derivatives_match_independent_exact_limits() {
    let r = reference(1., true);
    let p = r.bezier_patch(0, 0., 0.005, 0., 0.005).unwrap();
    let du = difference(p.control_points_m[1][0], p.control_points_m[0][0]).map(|x| 3. * x / 0.005);
    let dv = difference(p.control_points_m[0][1], p.control_points_m[0][0]).map(|x| 3. * x / 0.005);
    assert!(norm(difference(du, [0., 0.02, 0.])) < 1e-14);
    assert!(norm(difference(dv, [0., 0., 0.001])) < 1e-14);
    assert!(norm(difference(point(p, 0., 0.), [0.01, 0., 0.0195])) < 1e-14);
}
#[test]
fn adjacent_patches_share_entire_edge_control_nets_including_wraparound() {
    let r = reference(1., false);
    for chart in 0..4 {
        let left = r.bezier_patch(chart, 0.25, 0.5, 0.2, 0.3).unwrap();
        let right = r.bezier_patch(chart, 0.5, 0.75, 0.2, 0.3).unwrap();
        let end = r.bezier_patch(chart, 0.9, 1., 0.2, 0.3).unwrap();
        let start = r.bezier_patch((chart + 1) % 4, 0., 0.1, 0.2, 0.3).unwrap();
        let upper = r.bezier_patch(chart, 0.25, 0.5, 0.3, 0.5).unwrap();
        for i in 0..4 {
            same(left.control_points_m[3][i], right.control_points_m[0][i]);
            same(end.control_points_m[3][i], start.control_points_m[0][i]);
            same(left.control_points_m[i][3], upper.control_points_m[i][0]);
        }
    }
}
#[test]
fn refinement_scales_and_invalid_domains_preserve_the_contract() {
    let r = reference(1., false);
    let coarse = r.bezier_patch(0, 0.2, 0.4, 0.3, 0.5).unwrap();
    let fine = r.bezier_patch(0, 0.25, 0.35, 0.35, 0.45).unwrap();
    assert!(fine.position_error_upper_m < coarse.position_error_upper_m / 3.);
    for scale in [1e-4, 1000.] {
        let p = reference(scale, false)
            .bezier_patch(0, 0.25, 0.35, 0.35, 0.45)
            .unwrap();
        assert!(
            (p.position_error_upper_m / scale - fine.position_error_upper_m).abs()
                < 1e-9 * fine.position_error_upper_m
        );
    }
    for (chart, u0, u1, v0, v1) in [
        (4, 0., 1., 0., 1.),
        (0, 0.2, 0.2, 0., 1.),
        (0, 0., 1., 0.2, 0.1),
        (0, f64::NAN, 1., 0., 1.),
    ] {
        assert!(r.bezier_patch(chart, u0, u1, v0, v1).is_err());
    }
    assert!(r
        .bezier_patch(0, 0., f64::from_bits(1), 0., f64::from_bits(1))
        .is_ok());
}
