use opencad_core::{Angle, Length};
use opencad_geometry::{
    CircularFilletContactCurve, CircularFilletContactKind as Kind, CircularFilletReference,
};
use std::f64::consts::FRAC_PI_2;
fn reference(scale: f64, variable: bool) -> CircularFilletReference {
    CircularFilletReference::new(
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
fn choose(n: usize, k: usize) -> f64 {
    let mut value = 1.;
    for i in 0..k {
        value *= (n - i) as f64 / (i + 1) as f64;
    }
    value
}
fn basis(n: usize, i: usize, t: f64) -> f64 {
    choose(n, i) * t.powi(i as i32) * (1. - t).powi((n - i) as i32)
}
fn evaluate(c: &CircularFilletContactCurve, t: f64) -> ([f64; 3], [f64; 3]) {
    let n = c.degree();
    let p = c.control_points_m();
    let w = c.weights();
    let (mut num, mut derivative, mut den, mut dd) = ([0.; 3], [0.; 3], 0., 0.);
    for i in 0..=n {
        let b = basis(n, i, t);
        den += b * w[i];
        for k in 0..3 {
            num[k] += b * w[i] * p[i][k];
        }
    }
    for i in 0..n {
        let b = n as f64 * basis(n - 1, i, t);
        dd += b * (w[i + 1] - w[i]);
        for k in 0..3 {
            derivative[k] += b * (w[i + 1] * p[i + 1][k] - w[i] * p[i][k]);
        }
    }
    (
        num.map(|v| v / den),
        std::array::from_fn(|k| (derivative[k] * den - num[k] * dd) / (den * den)),
    )
}
fn gap(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}
#[test]
fn encoded_support_identities_positive_weights_and_shared_endpoints_are_preserved() {
    for scale in [0.001, 1., 1000.] {
        for variable in [false, true] {
            let r = reference(scale, variable);
            let radius = r.cylinder_radius().meters();
            let height = r.cylinder_height().meters();
            for kind in [Kind::Side, Kind::End] {
                for chart in 0..4 {
                    let curve = r.rational_contact_curve(kind, chart).unwrap();
                    assert_eq!(curve.kind(), kind);
                    assert_eq!(curve.chart(), chart);
                    assert!(curve.weights().iter().all(|w| w.is_finite() && *w > 0.));
                    if kind == Kind::Side {
                        assert_eq!(curve.degree(), 2);
                        assert_eq!(curve.weights(), [1., 1., 2.]);
                        let rotate = |x: f64, y: f64| {
                            match chart {
                                0 => [x, y],
                                1 => [-y, x],
                                2 => [-x, -y],
                                _ => [y, -x],
                            }
                            .map(|v| if v == 0. { 0. } else { v })
                        };
                        for (actual, [x, y]) in curve.control_points_m().iter().zip([
                            [radius, 0.],
                            [radius, radius],
                            [0., radius],
                        ]) {
                            // Stored representation identity proves the rational circle
                            // polynomial identity; this is not a proximity assertion.
                            assert_eq!(
                                [actual[0].to_bits(), actual[1].to_bits()],
                                rotate(x, y).map(f64::to_bits)
                            );
                        }
                    } else {
                        assert_eq!(curve.degree(), 4);
                        for p in curve.control_points_m() {
                            assert_eq!(p[2].to_bits(), height.to_bits());
                        }
                    }
                    let next = r.rational_contact_curve(kind, (chart + 1) % 4).unwrap();
                    assert_eq!(
                        curve.control_points_m()[curve.degree()].map(f64::to_bits),
                        next.control_points_m()[0].map(f64::to_bits)
                    );
                }
            }
        }
    }
}
#[test]
fn whole_curve_position_bounds_match_independent_contacts_and_reference_at_scales() {
    for scale in [0.001, 1., 1000.] {
        for variable in [false, true] {
            let r = reference(scale, variable);
            for kind in [Kind::Side, Kind::End] {
                for chart in 0..4 {
                    let curve = r.rational_contact_curve(kind, chart).unwrap();
                    let bound = curve.position_error_upper_m();
                    assert!(bound.is_finite() && bound > 0. && bound < 1e-11 * scale);
                    for index in 0..=100 {
                        let t = index as f64 / 100.;
                        let theta = f64::from(chart) * FRAC_PI_2 + 2. * t.atan();
                        let radius = (if variable {
                            0.001 + 0.0005 * theta.cos()
                        } else {
                            0.0005
                        }) * scale;
                        let (p, d) = evaluate(&curve, t);
                        let base = if kind == Kind::Side {
                            0.01 * scale
                        } else {
                            0.01 * scale - radius
                        };
                        let expected = [
                            base * theta.cos(),
                            base * theta.sin(),
                            if kind == Kind::Side {
                                0.02 * scale - radius
                            } else {
                                0.02 * scale
                            },
                        ];
                        assert!(gap(p, expected) <= bound + 2e-16 * scale);
                        if kind == Kind::Side {
                            assert!((p[0] * d[0] + p[1] * d[1]).abs() < 1e-14 * scale * scale);
                        } else {
                            assert!(d[2].abs() < 1e-13 * scale);
                        }
                    }
                    let t = 0.37_f64;
                    let theta = f64::from(chart) * FRAC_PI_2 + 2. * t.atan();
                    let ideal = r
                        .sample(
                            Angle::from_radians(theta),
                            Angle::from_radians(if kind == Kind::Side { 0. } else { FRAC_PI_2 }),
                        )
                        .unwrap();
                    assert!(gap(evaluate(&curve, t).0, ideal.point_m) <= bound + 2e-16 * scale);
                }
            }
        }
    }
}
#[test]
fn authored_phase_and_constant_limit_have_independent_contact_dimensions() {
    for variable in [false, true] {
        let r = reference(1., variable);
        for kind in [Kind::Side, Kind::End] {
            for (chart, radius) in [0.0015, 0.001, 0.0005, 0.001].into_iter().enumerate() {
                let curve = r.rational_contact_curve(kind, chart as u8).unwrap();
                let p = curve.control_points_m()[0];
                let expected = if variable { radius } else { 0.0005 };
                let actual = if kind == Kind::Side {
                    0.02 - p[2]
                } else {
                    0.01 - p[0].hypot(p[1])
                };
                assert!((actual - expected).abs() < 1e-15);
            }
        }
    }
}
#[test]
fn undeclared_charts_refuse_without_wrapping_or_mutation() {
    let r = reference(1., true);
    for kind in [Kind::Side, Kind::End] {
        for chart in [4, 5, 255] {
            assert!(r.rational_contact_curve(kind, chart).is_err());
        }
    }
    let before = r.rational_contact_curve(Kind::Side, 0).unwrap();
    let after = r.rational_contact_curve(Kind::Side, 0).unwrap();
    assert_eq!(before.control_points_m(), after.control_points_m());
    assert_eq!(before.weights(), after.weights());
}
