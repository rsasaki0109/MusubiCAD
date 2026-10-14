use opencad_core::{Angle, Length};
use opencad_geometry::{CircularFilletPatchBounds, CircularFilletReference};
use std::f64::consts::{FRAC_PI_2, TAU};
fn reference(scale: f64, constant: bool) -> CircularFilletReference {
    CircularFilletReference::new(
        Length::from_meters(0.01 * scale),
        Length::from_meters(0.02 * scale),
        Length::from_meters(0.0005 * scale),
        Length::from_meters(if constant {
            0.0005 * scale
        } else {
            0.0015 * scale
        }),
    )
    .unwrap()
}
fn oracle(
    p: CircularFilletReference,
    chart: u8,
    u: f64,
    v: f64,
) -> ([f64; 3], [f64; 3], [f64; 3], [f64; 3]) {
    let theta = (f64::from(chart) * FRAC_PI_2 + 2. * u.atan()).min(TAU);
    let alpha = 2. * v.atan();
    let s = p
        .sample(Angle::from_radians(theta), Angle::from_radians(alpha))
        .unwrap();
    (
        s.point_m,
        s.normal,
        s.theta_tangent_m.map(|x| x * 2. / (1. + u * u)),
        s.alpha_tangent_m.map(|x| x * 2. / (1. + v * v)),
    )
}
fn norm(x: [f64; 3]) -> f64 {
    x.into_iter().map(|v| v * v).sum::<f64>().sqrt()
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn contained(lo: [f64; 3], hi: [f64; 3], value: [f64; 3], tolerance: f64) {
    for i in 0..3 {
        assert!(
            lo[i] - tolerance <= value[i] && value[i] <= hi[i] + tolerance,
            "{} outside [{},{}]",
            value[i],
            lo[i],
            hi[i]
        );
    }
}

#[test]
fn point_normal_and_chart_derivatives_enclose_independent_angular_reference() {
    for constant in [true, false] {
        for chart in 0..4 {
            let p = reference(1., constant);
            let b = p.patch_bounds(chart, 0.2, 0.3, 0.4, 0.5).unwrap();
            for u in [0.2, 0.25, 0.3] {
                for v in [0.4, 0.45, 0.5] {
                    let (point, normal, du, dv) = oracle(p, chart, u, v);
                    contained(b.position_lower_m, b.position_upper_m, point, 1e-14);
                    contained(b.normal_lower, b.normal_upper, normal, 1e-12);
                    contained(b.du_lower_m, b.du_upper_m, du, 1e-14);
                    contained(b.dv_lower_m, b.dv_upper_m, dv, 1e-14);
                }
            }
            // Numerical derivatives of an independent angular first-derivative API.
            let (u, v, step) = (0.25, 0.45, 1e-5);
            let hessians = [
                sub(
                    oracle(p, chart, u + step, v).2,
                    oracle(p, chart, u - step, v).2,
                ),
                sub(
                    oracle(p, chart, u, v + step).2,
                    oracle(p, chart, u, v - step).2,
                ),
                sub(
                    oracle(p, chart, u, v + step).3,
                    oracle(p, chart, u, v - step).3,
                ),
            ];
            for (h, bound) in hessians.into_iter().zip(b.second_derivative_norm_upper_m) {
                assert!(norm(h.map(|x| x / (2. * step))) <= bound + 1e-12);
            }
        }
    }
}
#[test]
fn rounded_triangle_interpolation_stays_inside_the_proven_error_budget() {
    for chart in 0..4 {
        for constant in [true, false] {
            let p = reference(1., constant);
            let (u0, u1, v0, v1) = (0.25, 0.255, 0.3, 0.305);
            let b = p.patch_bounds(chart, u0, u1, v0, v1).unwrap();
            assert!(b.corner_rounding_error_upper_m > 0.);
            assert!(b.linear_interpolation_error_upper_m < 1e-6);
            let params = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
            for triangle in [[0, 1, 2], [0, 2, 3]] {
                for weights in [
                    [1., 0., 0.],
                    [0., 1., 0.],
                    [0., 0., 1.],
                    [0.2, 0.3, 0.5],
                    [0.5, 0.5, 0.],
                    [1. / 3.; 3],
                ] {
                    let uv: [f64; 2] = std::array::from_fn(|axis| {
                        triangle
                            .into_iter()
                            .zip(weights)
                            .map(|(i, w)| params[i][axis] * w)
                            .sum()
                    });
                    let interpolated: [f64; 3] = std::array::from_fn(|axis| {
                        triangle
                            .into_iter()
                            .zip(weights)
                            .map(|(i, w)| b.corner_points_m[i][axis] * w)
                            .sum()
                    });
                    let exact = oracle(p, chart, uv[0], uv[1]).0;
                    let error = norm(sub(interpolated, exact));
                    // The angular/libm oracle and test interpolation are numerical;
                    // the API's proof is interval Taylor arithmetic, not this grid.
                    assert!(error <= b.linear_interpolation_error_upper_m + 1e-14);
                }
            }
            for (i, [u, v]) in params.into_iter().enumerate() {
                assert!(
                    norm(sub(b.corner_points_m[i], oracle(p, chart, u, v).0))
                        <= b.corner_rounding_error_upper_m + 1e-14
                );
            }
        }
    }
}
#[test]
fn subdivision_sharpens_bounds_and_scales_in_metres() {
    let p = reference(1., false);
    let coarse = p.patch_bounds(0, 0.2, 0.4, 0.3, 0.5).unwrap();
    let fine = p.patch_bounds(0, 0.25, 0.35, 0.35, 0.45).unwrap();
    assert!(
        fine.linear_interpolation_error_upper_m < coarse.linear_interpolation_error_upper_m / 3.
    );
    for scale in [1e-4_f64, 1000.] {
        let b = reference(scale, false)
            .patch_bounds(0, 0.25, 0.35, 0.35, 0.45)
            .unwrap();
        assert!(
            (b.linear_interpolation_error_upper_m / scale
                - fine.linear_interpolation_error_upper_m)
                .abs()
                < 1e-9 * fine.linear_interpolation_error_upper_m
        );
    }
}
fn same_vertex(a: [f64; 3], b: [f64; 3]) {
    assert_eq!(a.map(f64::to_bits), b.map(f64::to_bits));
}
#[test]
fn shared_corners_and_all_chart_seams_have_identical_vertex_bits() {
    let p = reference(1., false);
    for chart in 0..4 {
        let left = p.patch_bounds(chart, 0.25, 0.5, 0.2, 0.3).unwrap();
        let right = p.patch_bounds(chart, 0.5, 0.75, 0.2, 0.3).unwrap();
        same_vertex(left.corner_points_m[1], right.corner_points_m[0]);
        same_vertex(left.corner_points_m[2], right.corner_points_m[3]);
        let end = p.patch_bounds(chart, 0.9, 1., 0.2, 0.3).unwrap();
        let start = p.patch_bounds((chart + 1) % 4, 0., 0.1, 0.2, 0.3).unwrap();
        same_vertex(end.corner_points_m[1], start.corner_points_m[0]);
        same_vertex(end.corner_points_m[2], start.corner_points_m[3]);
    }
}
fn finite(b: CircularFilletPatchBounds) {
    assert!(b
        .position_lower_m
        .into_iter()
        .chain(b.position_upper_m)
        .chain(b.normal_lower)
        .chain(b.normal_upper)
        .chain(b.second_derivative_norm_upper_m)
        .chain([b.linear_interpolation_error_upper_m])
        .all(f64::is_finite));
}
#[test]
fn unadmitted_rectangles_refuse_and_full_chart_enclosures_remain_finite() {
    let p = reference(1., false);
    for (chart, u0, u1, v0, v1) in [
        (4, 0., 1., 0., 1.),
        (0, -0.1, 1., 0., 1.),
        (0, 0., 1.1, 0., 1.),
        (0, 0.5, 0.5, 0., 1.),
        (0, 0.7, 0.2, 0., 1.),
        (0, 0., 1., 0.5, 0.5),
        (0, 0., 1., 0., f64::NAN),
        (0, f64::INFINITY, 1., 0., 1.),
    ] {
        assert!(p.patch_bounds(chart, u0, u1, v0, v1).is_err());
    }
    for chart in 0..4 {
        finite(p.patch_bounds(chart, 0., 1., 0., 1.).unwrap());
    }
    // Tiny valid chart widths, including subnormals, do not produce NaN norms.
    finite(
        p.patch_bounds(0, 0., f64::from_bits(1), 0., f64::from_bits(1))
            .unwrap(),
    );
    let extreme = CircularFilletReference::new(
        Length::from_meters(1000.),
        Length::from_meters(1000.),
        Length::from_meters(1.1e-9),
        Length::from_meters(250.),
    )
    .unwrap();
    finite(extreme.patch_bounds(2, 0.2, 0.3, 0.4, 0.5).unwrap());
}
