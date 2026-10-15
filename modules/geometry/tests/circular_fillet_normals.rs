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
fn norm(x: [f64; 3]) -> f64 {
    x.into_iter().map(|v| v * v).sum::<f64>().sqrt()
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[(i + 1) % 3] * b[(i + 2) % 3] - a[(i + 2) % 3] * b[(i + 1) % 3])
}
fn normal(p: CircularFilletBezierPatch, u: f64, v: f64) -> ([f64; 3], f64) {
    let b = |x: f64| {
        [
            (1. - x).powi(3),
            3. * x * (1. - x).powi(2),
            3. * x * x * (1. - x),
            x.powi(3),
        ]
    };
    let q = |x: f64| [(1. - x).powi(2), 2. * x * (1. - x), x * x];
    let du = std::array::from_fn(|k| {
        (0..3)
            .flat_map(|i| {
                (0..4).map(move |j| {
                    3. * q(u)[i]
                        * b(v)[j]
                        * (p.control_points_m[i + 1][j][k] - p.control_points_m[i][j][k])
                })
            })
            .sum()
    });
    let dv = std::array::from_fn(|k| {
        (0..4)
            .flat_map(|i| {
                (0..3).map(move |j| {
                    3. * b(u)[i]
                        * q(v)[j]
                        * (p.control_points_m[i][j + 1][k] - p.control_points_m[i][j][k])
                })
            })
            .sum()
    });
    let w = cross(du, dv);
    let area = norm(w);
    (w.map(|x| x / area), area)
}

#[test]
fn independent_planar_normal_and_area_are_enclosed() {
    let r = reference(1., true);
    let mut p = r.bezier_patch(0, 0., 0.005, 0., 0.005).unwrap();
    p.control_points_m = std::array::from_fn(|i| {
        std::array::from_fn(|j| [0.01, i as f64 * 0.001 / 3., j as f64 * 0.001 / 3.])
    });
    let bounds = p.normal_bounds(r, 0, 0., 0.005, 0., 0.005).unwrap();
    assert!(bounds.jacobian_area_lower_m2 <= 1e-6 && bounds.jacobian_area_upper_m2 >= 1e-6);
    for i in 0..3 {
        assert!(
            bounds.normal_lower[i] <= [1., 0., 0.][i] && bounds.normal_upper[i] >= [1., 0., 0.][i]
        );
    }
    assert!(bounds.ideal_normal_alignment_lower > 0.99);
    // Positional metadata cannot influence recomputation of current poles.
    p.position_error_upper_m = f64::NAN;
    p.reference_to_bilinear_error_upper_m = f64::INFINITY;
    p.control_net_deviation_upper_m = -1.;
    let again = p.normal_bounds(r, 0, 0., 0.005, 0., 0.005).unwrap();
    assert_eq!(
        again.normal_error_upper.to_bits(),
        bounds.normal_error_upper.to_bits()
    );
}
#[test]
fn normals_and_area_enclose_independent_bernstein_and_angular_oracles() {
    for constant in [true, false] {
        let r = reference(1., constant);
        for chart in 0..4 {
            for (u0, u1, v0, v1) in [
                (0.25_f64, 0.255, 0., 0.005),
                (0.25, 0.255, 0.3, 0.305),
                (0.25, 0.255, 0.995, 1.),
                (0., 0.005, 0.3, 0.305),
                (0.995, 1., 0.3, 0.305),
            ] {
                let p = r.bezier_patch(chart, u0, u1, v0, v1).unwrap();
                let c = p.normal_bounds(r, chart, u0, u1, v0, v1).unwrap();
                assert!(c.ideal_normal_alignment_lower > 0.9 && c.normal_error_upper < 0.1);
                for u in [0., 0.5, 1.] {
                    for v in [0., 0.5, 1.] {
                        let (n, area) = normal(p, u, v);
                        let ideal = r
                            .sample(
                                Angle::from_radians(
                                    f64::from(chart) * FRAC_PI_2 + 2. * (u0 + u * (u1 - u0)).atan(),
                                ),
                                Angle::from_radians(2. * (v0 + v * (v1 - v0)).atan()),
                            )
                            .unwrap()
                            .normal;
                        for (i, value) in n.into_iter().enumerate() {
                            assert!(
                                c.normal_lower[i] - 1e-12 <= value
                                    && value <= c.normal_upper[i] + 1e-12
                            );
                        }
                        assert!(
                            c.jacobian_area_lower_m2 <= area + 1e-24
                                && area <= c.jacobian_area_upper_m2 + 1e-24
                        );
                        assert!(norm(sub(n, ideal)) <= c.normal_error_upper + 1e-12);
                        let alignment: f64 = n.into_iter().zip(ideal).map(|(a, b)| a * b).sum();
                        assert!(alignment + 1e-12 >= c.ideal_normal_alignment_lower);
                    }
                }
            }
        }
    }
}
#[test]
fn area_scales_quadratically_and_normal_error_is_dimensionless() {
    let r = reference(1., false);
    let p = r.bezier_patch(0, 0.25, 0.255, 0.3, 0.305).unwrap();
    let c = p.normal_bounds(r, 0, 0.25, 0.255, 0.3, 0.305).unwrap();
    for scale in [1e-4, 1000.] {
        let r = reference(scale, false);
        let p = r.bezier_patch(0, 0.25, 0.255, 0.3, 0.305).unwrap();
        let b = p.normal_bounds(r, 0, 0.25, 0.255, 0.3, 0.305).unwrap();
        assert!(
            (b.jacobian_area_lower_m2 / (scale * scale) / c.jacobian_area_lower_m2 - 1.).abs()
                < 1e-8
        );
        assert!((b.normal_error_upper - c.normal_error_upper).abs() < 1e-8);
    }
}
#[test]
fn collapsed_reversed_nonfinite_and_interior_corruption_refuse() {
    let r = reference(1., false);
    let original = r.bezier_patch(0, 0.25, 0.255, 0.3, 0.305).unwrap();
    for corruption in 0..5 {
        let mut p = original;
        match corruption {
            0 => p.control_points_m = [[[0.; 3]; 4]; 4],
            1 => p.control_points_m.reverse(),
            2 => p.control_points_m[1][1][0] = f64::NAN,
            3 => p.control_points_m[1][1][0] = f64::INFINITY,
            _ => p.control_points_m[1][1] = [1000., -1000., 1000.],
        }
        assert!(p.normal_bounds(r, 0, 0.25, 0.255, 0.3, 0.305).is_err());
    }
    assert!(original
        .normal_bounds(r, 4, 0.25, 0.255, 0.3, 0.305)
        .is_err());
    assert!(original.normal_bounds(r, 0, 0.3, 0.2, 0.3, 0.305).is_err());
    let tiny = r
        .bezier_patch(0, 0., f64::from_bits(1), 0., f64::from_bits(1))
        .unwrap();
    assert!(tiny
        .normal_bounds(r, 0, 0., f64::from_bits(1), 0., f64::from_bits(1))
        .is_err());
}
