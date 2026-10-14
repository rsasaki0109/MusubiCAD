use opencad_core::{Angle, Length};
use opencad_geometry::CircularFilletReference;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn reference(r: f64, h: f64, lo: f64, hi: f64) -> CircularFilletReference {
    CircularFilletReference::new(
        Length::from_meters(r),
        Length::from_meters(h),
        Length::from_meters(lo),
        Length::from_meters(hi),
    )
    .unwrap()
}
fn sample(p: CircularFilletReference, t: f64, a: f64) -> opencad_geometry::CircularFilletSample {
    p.sample(Angle::from_radians(t), Angle::from_radians(a))
        .unwrap()
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} != {b}, tolerance {tol}");
}
fn sphere(r: f64, h: f64, m: f64, a: f64, theta: f64) -> ([f64; 3], f64) {
    let radius = m + a * theta.cos();
    (
        [
            (r - radius) * theta.cos(),
            (r - radius) * theta.sin(),
            h - radius,
        ],
        radius,
    )
}

#[test]
fn contacts_envelope_normals_regular_tangents_and_seam_match_independent_equations() {
    for (r, h, lo, hi) in [
        (0.01, 0.02, 0.0005, 0.0015),
        (0.04, 0.04, 0.000_001, 0.01),
        (4e-8, 4e-8, 2e-9, 1e-8),
        (1000., 1000., 1.1e-9, 250.),
    ] {
        let p = reference(r, h, lo, hi);
        close(p.cylinder_radius().meters(), r, r * 1e-14);
        close(p.cylinder_height().meters(), h, h * 1e-14);
        close(p.min_radius().meters(), lo, lo * 1e-14);
        close(p.max_radius().meters(), hi, hi * 1e-14);
        for theta in [0., 0.37, FRAC_PI_2, PI, 5.8] {
            let (centre, radius) = sphere(r, h, (lo + hi) / 2., (hi - lo) / 2., theta);
            let dr = -(hi - lo) / 2. * theta.sin();
            let centre_prime = [
                -dr * theta.cos() - (r - radius) * theta.sin(),
                -dr * theta.sin() + (r - radius) * theta.cos(),
                -dr,
            ];
            close(
                p.radius_at(Angle::from_radians(theta)).unwrap().meters(),
                radius,
                r * 2e-12,
            );
            for alpha in [0., 0.2, PI / 4., 1.3, FRAC_PI_2] {
                let s = sample(p, theta, alpha);
                close(norm(sub(s.point_m, centre)), radius, r * 2e-12);
                close(norm(s.normal), 1., 2e-12);
                close(dot(s.normal, centre_prime), -dr, r * 2e-12);
                close(
                    dot(s.normal, s.theta_tangent_m),
                    0.,
                    norm(s.theta_tangent_m) * 2e-12,
                );
                close(
                    dot(s.normal, s.alpha_tangent_m),
                    0.,
                    norm(s.alpha_tangent_m) * 2e-12,
                );
                assert!(dot(cross(s.theta_tangent_m, s.alpha_tangent_m), s.normal) > 0.);
                assert!(s.point_m[2] <= h + r * 1e-12);
                assert!(s.point_m[0].hypot(s.point_m[1]) <= r + r * 1e-12);
            }
            for (alpha, expected, normal) in [
                (
                    0.,
                    [r * theta.cos(), r * theta.sin(), h - radius],
                    [theta.cos(), theta.sin(), 0.],
                ),
                (
                    FRAC_PI_2,
                    [(r - radius) * theta.cos(), (r - radius) * theta.sin(), h],
                    [0., 0., 1.],
                ),
            ] {
                let s = sample(p, theta, alpha);
                for (a, b) in s.point_m.into_iter().zip(expected) {
                    close(a, b, r * 2e-12);
                }
                for (a, b) in s.normal.into_iter().zip(normal) {
                    close(a, b, 2e-12);
                }
            }
        }
        for alpha in [0., 0.4, 1.1, FRAC_PI_2] {
            let start = sample(p, 0., alpha);
            let end = sample(p, TAU, alpha);
            for (a, b) in start
                .point_m
                .into_iter()
                .chain(start.theta_tangent_m)
                .chain(start.alpha_tangent_m)
                .zip(
                    end.point_m
                        .into_iter()
                        .chain(end.theta_tangent_m)
                        .chain(end.alpha_tangent_m),
                )
            {
                close(a, b, r * 1e-14);
            }
            for (a, b) in start.normal.into_iter().zip(end.normal) {
                close(a, b, 1e-14);
            }
        }
    }
}

#[test]
fn global_exposure_factor_and_lower_bound_match_direct_sphere_distances() {
    let r = 0.01;
    let h = 0.02;
    for hi in [0.0005, 0.0015, r / 4.] {
        for lo in [hi / 100., hi / 2., hi] {
            let p = reference(r, h, lo, hi);
            let m = (lo + hi) / 2.;
            let a = (hi - lo) / 2.;
            assert!(p.exposure_margin_m2() >= 9. * r * r / 32. - r * r * 1e-14);
            for theta in [0., 0.6, FRAC_PI_2, 3.1, 5.9] {
                for alpha in [0., 0.4, PI / 4., 1.4, FRAC_PI_2] {
                    let x = sample(p, theta, alpha).point_m;
                    // Independent Fourier expansion of squared sphere distance.
                    let c1 = -2. * a * (r + h - x[2] - m) - 2. * (r - m) * x[0];
                    let s1 = -2. * (r - m) * x[1];
                    let c2 = a * a / 2. + a * x[0];
                    let s2 = a * x[1];
                    let b = -2. * (c2 * (2. * theta).cos() + s2 * (2. * theta).sin());
                    let d = -2. * (-c2 * (2. * theta).sin() + s2 * (2. * theta).cos());
                    let aa = b - (c1 * theta.cos() + s1 * theta.sin());
                    assert!(aa - b.hypot(d) >= p.exposure_margin_m2() - 2e-12 * r * r);
                    for delta in [-PI, -1.2, -0.1, 0.4, 2.7, PI] {
                        let (centre, radius) = sphere(r, h, m, a, theta + delta);
                        let direct = dot(sub(x, centre), sub(x, centre)) - radius * radius;
                        let factored =
                            (1. - delta.cos()) * (aa + b * delta.cos() + d * delta.sin());
                        close(direct, factored, 2e-12 * r * r);
                        assert!(
                            direct >= p.exposure_margin_m2() * (1. - delta.cos()) - 2e-12 * r * r
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn analytic_tangents_match_numerical_differences_and_have_tangential_shift() {
    let p = reference(0.01, 0.02, 0.0005, 0.0015);
    let step = 1e-5;
    for theta in [0.1, 0.8, 2.1, 4.7] {
        for alpha in [0.1, 0.4, 1.1, 1.4] {
            let s = sample(p, theta, alpha);
            for (derivative, hi, lo) in [
                (
                    s.theta_tangent_m,
                    sample(p, theta + step, alpha).point_m,
                    sample(p, theta - step, alpha).point_m,
                ),
                (
                    s.alpha_tangent_m,
                    sample(p, theta, alpha + step).point_m,
                    sample(p, theta, alpha - step).point_m,
                ),
            ] {
                for i in 0..3 {
                    close(derivative[i], (hi[i] - lo[i]) / (2. * step), 5e-12);
                }
            }
        }
    }
    let s = sample(p, FRAC_PI_2, PI / 4.);
    // A meridional quarter-circle revolution has no displacement along e_theta.
    assert!(dot(s.point_m, [-1., 0., 0.]).abs() > 1e-8);
}

fn simpson(f: impl Fn(f64) -> f64) -> f64 {
    let count = 1024;
    let step = FRAC_PI_2 / f64::from(count);
    let mut value = f(0.) + f(FRAC_PI_2);
    for i in 1..count {
        value += f(f64::from(i) * step) * if i % 2 == 0 { 2. } else { 4. };
    }
    value * step / 3.
}

#[test]
fn constant_volume_matches_independent_corner_and_surface_integrals() {
    for (r, h, radius) in [
        (0.01, 0.02, 0.0005),
        (0.04, 0.04, 0.01),
        (0.03, 0.12, 0.002),
    ] {
        let p = reference(r, h, radius, radius);
        let volume = p.constant_limit_volume_m3().unwrap();
        // x=radius*sin(beta) regularizes the removed-corner square-root endpoint.
        let removed = simpson(|b| {
            2. * PI * (r - radius + radius * b.sin()) * radius * radius * (1. - b.cos()) * b.cos()
        });
        close(volume, PI * r * r * h - removed, 1e-12 * PI * r * r * h);
        let blend = TAU
            * simpson(|alpha| {
                let s = sample(p, 0., alpha);
                dot(s.point_m, cross(s.theta_tangent_m, s.alpha_tangent_m))
            });
        let surface_volume =
            (blend + TAU * r * r * (h - radius) + PI * h * (r - radius).powi(2)) / 3.;
        close(volume, surface_volume, 1e-12 * PI * r * r * h);
    }
}

#[test]
fn unadmitted_inputs_and_nonconstant_volume_are_refused_without_snapping() {
    for dims in [
        [f64::NAN, 0.02, 0.001, 0.002],
        [0.01, f64::INFINITY, 0.001, 0.002],
        [0.01, 0.02, 0., 0.002],
        [0.01, 0.02, 1e-9, 0.002],
        [0.01, 0.02, 0.002, 0.001],
        [0.01, 0.02, 0.001, 0.0026],
        [0.01, 0.007, 0.001, 0.002],
        [1001., 10., 0.001, 0.002],
    ] {
        assert!(CircularFilletReference::new(
            Length::from_meters(dims[0]),
            Length::from_meters(dims[1]),
            Length::from_meters(dims[2]),
            Length::from_meters(dims[3])
        )
        .is_err());
    }
    let p = reference(0.01, 0.02, 0.0005, 0.0015);
    for theta in [-0.001, TAU + 0.001, f64::NAN, f64::INFINITY] {
        assert!(p.radius_at(Angle::from_radians(theta)).is_err());
        assert!(p
            .sample(Angle::from_radians(theta), Angle::from_radians(0.5))
            .is_err());
    }
    for alpha in [-0.001, FRAC_PI_2 + 0.001, f64::NAN, f64::INFINITY] {
        assert!(p
            .sample(Angle::from_radians(0.5), Angle::from_radians(alpha))
            .is_err());
    }
    assert!(p.constant_limit_volume_m3().is_err());
    let lo = 0.001_f64;
    let hi = f64::from_bits(lo.to_bits() + 1);
    assert!(reference(0.01, 0.02, lo, hi)
        .constant_limit_volume_m3()
        .is_err());
}
