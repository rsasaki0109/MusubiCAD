use opencad_core::{Angle, Length};
use opencad_geometry::{CircularFilletReference, CircularFilletVolumeBounds};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn reference(scale: f64, lo: f64, hi: f64) -> CircularFilletReference {
    CircularFilletReference::new(
        Length::from_meters(0.01 * scale),
        Length::from_meters(0.02 * scale),
        Length::from_meters(lo * scale),
        Length::from_meters(hi * scale),
    )
    .unwrap()
}
fn simpson(f: impl Fn(f64) -> f64, end: f64, count: u32) -> f64 {
    let step = end / f64::from(count);
    (0..=count)
        .map(|i| {
            f(f64::from(i) * step)
                * if i == 0 || i == count {
                    1.
                } else if i % 2 == 0 {
                    2.
                } else {
                    4.
                }
        })
        .sum::<f64>()
        * step
        / 3.
}
// Independent two-dimensional divergence integral, using the original point and
// analytic tangents rather than the new characteristic-circle reduction.
fn removed_surface(p: CircularFilletReference, theta_count: u32, alpha_count: u32) -> f64 {
    let h = p.cylinder_height().meters();
    simpson(
        |theta| {
            simpson(
                |alpha| {
                    let s = p
                        .sample(Angle::from_radians(theta), Angle::from_radians(alpha))
                        .unwrap();
                    let cross_z = s.theta_tangent_m[0] * s.alpha_tangent_m[1]
                        - s.theta_tangent_m[1] * s.alpha_tangent_m[0];
                    (h - s.point_m[2]) * cross_z
                },
                FRAC_PI_2,
                alpha_count,
            )
        },
        TAU,
        theta_count,
    )
}
fn contains(b: CircularFilletVolumeBounds, body: f64, removed: f64) {
    assert!(
        b.body_lower_m3 <= body && body <= b.body_upper_m3,
        "body {body} outside {b:?}"
    );
    assert!(
        b.removed_lower_m3 <= removed && removed <= b.removed_upper_m3,
        "removed {removed} outside {b:?}"
    );
}
#[test]
fn nonconstant_volume_encloses_independent_refined_surface_integral() {
    let p = reference(1., 0.0005, 0.0015);
    let removed_coarse = removed_surface(p, 128, 64);
    let removed = removed_surface(p, 256, 128);
    assert!((removed_coarse - removed).abs() < 1e-14);
    let body = PI * 0.01_f64.powi(2) * 0.02 - removed;
    let b = p.volume_bounds_m3(1e-12, 262_144).unwrap();
    contains(b, body, removed);
    assert!(b.body_upper_m3 - b.body_lower_m3 <= 1e-12);
    assert!(b.cells > 4 && b.cells <= 262_144);
    assert!(p.constant_limit_volume_m3().is_err());
}
#[test]
fn refinement_and_repetition_are_explicit_and_deterministic() {
    let p = reference(1., 0.0005, 0.0015);
    let coarse = p.volume_bounds_m3(1e-10, 10_000).unwrap();
    let fine = p.volume_bounds_m3(2e-11, 30_000).unwrap();
    assert!(fine.cells > coarse.cells);
    assert!(fine.body_lower_m3 >= coarse.body_lower_m3 - 1e-20);
    assert!(fine.body_upper_m3 <= coarse.body_upper_m3 + 1e-20);
    let repeat = p.volume_bounds_m3(2e-11, 30_000).unwrap();
    assert_eq!(fine.body_lower_m3.to_bits(), repeat.body_lower_m3.to_bits());
    assert_eq!(fine.body_upper_m3.to_bits(), repeat.body_upper_m3.to_bits());
    assert_eq!(
        fine.removed_lower_m3.to_bits(),
        repeat.removed_lower_m3.to_bits()
    );
    assert_eq!(
        fine.removed_upper_m3.to_bits(),
        repeat.removed_upper_m3.to_bits()
    );
    assert_eq!(fine.cells, repeat.cells);
}
#[test]
fn scale_ratio_boundary_and_constant_limits_match_independent_measurements() {
    for scale in [1e-4_f64, 1., 1000.] {
        let p = reference(scale, 0.001, 0.001);
        let b = p.volume_bounds_m3(1e-15 * scale.powi(3), 4).unwrap();
        let r = 0.01 * scale;
        let h = 0.02 * scale;
        let f = 0.001 * scale;
        // Independently integrate the removed meridional corner's first moment.
        let removed = simpson(
            |beta| 2. * PI * (r - f + f * beta.sin()) * f * f * (1. - beta.cos()) * beta.cos(),
            FRAC_PI_2,
            4096,
        );
        let oracle = PI * r * r * h - removed;
        let exact = p.constant_limit_volume_m3().unwrap();
        assert!((oracle - exact).abs() <= 1e-13 * oracle);
        assert!(b.body_lower_m3 <= exact && exact <= b.body_upper_m3);
        assert_eq!(b.cells, 0);
        let varying = reference(scale, 0.0001, 0.0025);
        let varying_bounds = varying
            .volume_bounds_m3(1e-9 * scale.powi(3), 30_000)
            .unwrap();
        let removed = removed_surface(varying, 128, 64);
        contains(varying_bounds, PI * r * r * h - removed, removed);
    }
}
#[test]
fn explicit_positive_amplitude_is_not_replaced_by_constant() {
    let lo = 0.001_f64;
    let hi = f64::from_bits(lo.to_bits() + 1);
    let p = reference(1., lo, hi);
    let bounds = p.volume_bounds_m3(1e-6, 4).unwrap();
    assert_eq!(bounds.cells, 4);
    assert!(p.constant_limit_volume_m3().is_err());
}
#[test]
fn invalid_requests_budget_exhaustion_and_unattainable_precision_refuse() {
    let p = reference(1., 0.0005, 0.0015);
    for width in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(p.volume_bounds_m3(width, 4).is_err());
    }
    for budget in [0, 3, 262_145, usize::MAX] {
        assert!(p.volume_bounds_m3(1e-8, budget).is_err());
    }
    assert!(p.volume_bounds_m3(1e-12, 4).is_err());
    assert!(p.volume_bounds_m3(f64::from_bits(1), 4).is_err());
    assert!(reference(1., 0.001, 0.001)
        .volume_bounds_m3(f64::from_bits(1), 4)
        .is_err());
    // Read-only failed computations leave the same subsequent enclosure.
    let first = p.volume_bounds_m3(1e-9, 10_000).unwrap();
    assert!(p.volume_bounds_m3(1e-20, 4).is_err());
    let after = p.volume_bounds_m3(1e-9, 10_000).unwrap();
    assert_eq!(first.body_lower_m3.to_bits(), after.body_lower_m3.to_bits());
    assert_eq!(first.body_upper_m3.to_bits(), after.body_upper_m3.to_bits());
}
