use opencad_core::{Angle, Length};
use opencad_geometry::{
    CircularFilletMomentBounds, CircularFilletMomentPrecision, CircularFilletReference,
};
use std::f64::consts::{FRAC_PI_2, PI, TAU};
fn reference(scale: f64, minimum: f64, maximum: f64) -> CircularFilletReference {
    CircularFilletReference::new(
        Length::from_meters(0.01 * scale),
        Length::from_meters(0.02 * scale),
        Length::from_meters(minimum * scale),
        Length::from_meters(maximum * scale),
    )
    .unwrap()
}
fn precision(scale: f64) -> CircularFilletMomentPrecision {
    CircularFilletMomentPrecision {
        maximum_volume_interval_width_m3: 1e-11 * scale.powi(3),
        maximum_first_moment_interval_width_m4: 1e-12 * scale.powi(4),
        maximum_centroid_interval_width: Length::from_meters(1e-6 * scale),
        max_cells: 262144,
    }
}
fn simpson<const N: usize>(f: impl Fn(f64) -> [f64; N], end: f64, count: u32) -> [f64; N] {
    let step = end / f64::from(count);
    let mut sum = [0.; N];
    for i in 0..=count {
        let w = if i == 0 || i == count {
            1.
        } else if i % 2 == 0 {
            2.
        } else {
            4.
        };
        for (s, x) in sum.iter_mut().zip(f(f64::from(i) * step)) {
            *s += w * x;
        }
    }
    sum.map(|s| s * step / 3.)
}
// Original theta/alpha point and analytic tangents, using direct divergence
// fields, not the new characteristic polynomial or its degree-four moments.
fn surface(r: CircularFilletReference, n: u32) -> [f64; 4] {
    let h = r.cylinder_height().meters();
    simpson(
        |theta| {
            simpson(
                |alpha| {
                    let s = r
                        .sample(Angle::from_radians(theta), Angle::from_radians(alpha))
                        .unwrap();
                    let zflux = s.theta_tangent_m[0] * s.alpha_tangent_m[1]
                        - s.theta_tangent_m[1] * s.alpha_tangent_m[0];
                    let dv = (h - s.point_m[2]) * zflux;
                    [
                        dv,
                        s.point_m[0] * dv,
                        s.point_m[1] * dv,
                        (h * h - s.point_m[2] * s.point_m[2]) / 2. * zflux,
                    ]
                },
                FRAC_PI_2,
                n / 2,
            )
        },
        TAU,
        n,
    )
}
fn contains(i: [f64; 2], x: f64, tol: f64) {
    assert!(i[0] - tol <= x && x <= i[1] + tol, "{i:?} excludes {x}");
}
fn fingerprint(b: CircularFilletMomentBounds) -> Vec<u64> {
    b.volume_m3
        .into_iter()
        .chain(b.first_moments_m4.into_iter().flatten())
        .chain(b.centroid_m.into_iter().flatten())
        .map(f64::to_bits)
        .chain([b.volume_cells as u64, b.moment_cells as u64])
        .collect()
}
fn check(
    r: CircularFilletReference,
    p: CircularFilletMomentPrecision,
    removed: [f64; 4],
) -> CircularFilletMomentBounds {
    let b = r.moment_bounds(p).unwrap();
    let stock = PI * r.cylinder_radius().meters().powi(2) * r.cylinder_height().meters();
    let v = stock - removed[0];
    let h = r.cylinder_height().meters();
    let m = [-removed[1], -removed[2], stock * h / 2. - removed[3]];
    contains(b.volume_m3, v, p.maximum_volume_interval_width_m3 * 1e-3);
    for (k, moment) in m.into_iter().enumerate() {
        contains(
            b.first_moments_m4[k],
            moment,
            p.maximum_first_moment_interval_width_m4 * 1e-4,
        );
        contains(
            b.centroid_m[k],
            moment / v,
            p.maximum_centroid_interval_width.meters() * 1e-6,
        );
        assert!(
            b.first_moments_m4[k][1] - b.first_moments_m4[k][0]
                <= p.maximum_first_moment_interval_width_m4
        );
        assert!(
            b.centroid_m[k][1] - b.centroid_m[k][0] <= p.maximum_centroid_interval_width.meters()
        );
    }
    assert!(b.volume_m3[1] - b.volume_m3[0] <= p.maximum_volume_interval_width_m3);
    assert_eq!(b.first_moments_m4[1], [0.; 2]);
    assert_eq!(b.centroid_m[1], [0.; 2]);
    b
}
#[test]
fn variable_and_radius_boundary_moments_enclose_independent_surface_fluxes() {
    for (scale, lo, hi) in [
        (1., 0.0005, 0.0015),
        (0.1, 0.0001, 0.0025),
        (10., 0.0001, 0.0025),
    ] {
        let r = reference(scale, lo, hi);
        let coarse = surface(r, 128);
        let fine = surface(r, 256);
        for k in 1..4 {
            assert!((coarse[k] - fine[k]).abs() < 1e-16 * scale.powi(4));
        }
        let b = check(r, precision(scale), fine);
        assert!(b.first_moments_m4[0][1] < 0.);
        assert!(b.centroid_m[0][1] < 0.);
        assert!(
            b.volume_cells >= 4
                && b.volume_cells <= 262144
                && b.moment_cells >= 4
                && b.moment_cells <= 262144
        );
    }
}
#[test]
fn constant_moments_match_independent_meridional_integrals_and_scale_powers() {
    for scale in [1e-4_f64, 1., 1000.] {
        let r = reference(scale, 0.0005, 0.0005);
        let radius = 0.01 * scale;
        let height = 0.02 * scale;
        let f = 0.0005 * scale;
        let removed = simpson(
            |beta| {
                let rho = radius - f + f * beta.sin();
                let z = height - f + f * beta.cos();
                let dr = f * beta.cos();
                [
                    TAU * rho * (height - z) * dr,
                    PI * rho * (height * height - z * z) * dr,
                ]
            },
            FRAC_PI_2,
            4096,
        );
        let b = check(r, precision(scale), [removed[0], 0., 0., removed[1]]);
        assert_eq!(b.volume_cells, 0);
        assert_eq!(b.moment_cells, 0);
        assert_eq!(b.first_moments_m4[0], [0.; 2]);
        assert_eq!(b.centroid_m[0], [0.; 2]);
    }
}
#[test]
fn refinement_and_repeated_queries_are_deterministic_and_failures_preserve_reference() {
    let r = reference(1., 0.0005, 0.0015);
    let fine = r.moment_bounds(precision(1.)).unwrap();
    let mut loose = precision(1.);
    loose.maximum_volume_interval_width_m3 = 1e-10;
    loose.maximum_first_moment_interval_width_m4 = 1e-10;
    loose.maximum_centroid_interval_width = Length::from_meters(1e-4);
    let coarse = r.moment_bounds(loose).unwrap();
    assert!(fine.volume_cells > coarse.volume_cells && fine.moment_cells > coarse.moment_cells);
    for k in [0, 2] {
        assert!(
            fine.first_moments_m4[k][1] - fine.first_moments_m4[k][0]
                < coarse.first_moments_m4[k][1] - coarse.first_moments_m4[k][0]
        );
    }
    assert_eq!(
        fingerprint(fine),
        fingerprint(r.moment_bounds(precision(1.)).unwrap())
    );
    let mut bad = precision(1.);
    bad.max_cells = 4;
    assert!(r.moment_bounds(bad).is_err());
    assert_eq!(
        fingerprint(coarse),
        fingerprint(r.moment_bounds(loose).unwrap())
    );
}
#[test]
fn malformed_widths_budgets_and_unattainable_precision_refuse_without_snapping_amplitude() {
    let r = reference(1., 0.0005, 0.0015);
    for width in [0., -1., f64::NAN, f64::INFINITY] {
        let mut p = precision(1.);
        p.maximum_volume_interval_width_m3 = width;
        assert!(r.moment_bounds(p).is_err());
        let mut p = precision(1.);
        p.maximum_first_moment_interval_width_m4 = width;
        assert!(r.moment_bounds(p).is_err());
        let mut p = precision(1.);
        p.maximum_centroid_interval_width = Length::from_meters(width);
        assert!(r.moment_bounds(p).is_err());
    }
    for cells in [0, 3, 262145, usize::MAX] {
        let mut p = precision(1.);
        p.max_cells = cells;
        assert!(r.moment_bounds(p).is_err());
    }
    let mut bad = precision(1.);
    bad.max_cells = 4;
    bad.maximum_volume_interval_width_m3 = 1e-5;
    bad.maximum_first_moment_interval_width_m4 = 1e-5;
    bad.maximum_centroid_interval_width = Length::from_meters(1e-20);
    assert!(r.moment_bounds(bad).is_err());
    let mut exhausted = precision(1.);
    exhausted.max_cells = 4;
    exhausted.maximum_volume_interval_width_m3 = 1e-5;
    exhausted.maximum_first_moment_interval_width_m4 = 1e-30;
    exhausted.maximum_centroid_interval_width = Length::from_meters(1.);
    assert!(r.moment_bounds(exhausted).is_err());
    let constant = reference(1., 0.0005, 0.0005);
    let mut bad = precision(1.);
    bad.maximum_first_moment_interval_width_m4 = f64::from_bits(1);
    assert!(constant.moment_bounds(bad).is_err());
    let lo = 0.001_f64;
    let tiny = reference(1., lo, f64::from_bits(lo.to_bits() + 1));
    let mut p = precision(1.);
    p.maximum_volume_interval_width_m3 = 1.;
    p.maximum_first_moment_interval_width_m4 = 1.;
    p.maximum_centroid_interval_width = Length::from_meters(1.);
    p.max_cells = 4;
    let b = tiny.moment_bounds(p).unwrap();
    assert_eq!(b.volume_cells, 4);
    assert_eq!(b.moment_cells, 4);
}
