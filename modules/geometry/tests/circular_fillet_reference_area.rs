use opencad_core::{Angle, Length};
use opencad_geometry::{CircularFilletReference, CircularFilletReferenceAreaBounds};
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
// Independent original (theta,alpha) parameterization and analytic tangents.
// No characteristic-circle coefficients or area integrator enter this oracle.
fn blend_area(r: CircularFilletReference, count: u32) -> f64 {
    simpson(
        |theta| {
            simpson(
                |alpha| {
                    let sample = r
                        .sample(Angle::from_radians(theta), Angle::from_radians(alpha))
                        .unwrap();
                    let a = sample.theta_tangent_m;
                    let b = sample.alpha_tangent_m;
                    [
                        a[1] * b[2] - a[2] * b[1],
                        a[2] * b[0] - a[0] * b[2],
                        a[0] * b[1] - a[1] * b[0],
                    ]
                    .into_iter()
                    .map(|v| v * v)
                    .sum::<f64>()
                    .sqrt()
                },
                FRAC_PI_2,
                count / 2,
            )
        },
        TAU,
        count,
    )
}
fn contains(bounds: [f64; 2], value: f64) {
    assert!(
        bounds[0] <= value && value <= bounds[1],
        "{bounds:?} excludes {value}"
    );
}
fn fingerprint(b: CircularFilletReferenceAreaBounds) -> Vec<u64> {
    [
        b.body_lower_m2,
        b.body_upper_m2,
        b.blend_lower_m2,
        b.blend_upper_m2,
    ]
    .into_iter()
    .chain(b.retained_side_m2)
    .chain(b.retained_end_m2)
    .chain(b.bottom_m2)
    .map(f64::to_bits)
    .chain([b.cells as u64])
    .collect()
}

#[test]
fn constant_area_and_components_match_analytic_solution_across_scales() {
    for scale in [1e-4_f64, 1., 1000.] {
        let r = reference(scale, 0.001, 0.001);
        let b = r.surface_area_bounds_m2(1e-15 * scale.powi(2), 4).unwrap();
        let (radius, height, fillet) = (0.01 * scale, 0.02 * scale, 0.001 * scale);
        let blend = TAU * fillet * ((radius - fillet) * FRAC_PI_2 + fillet);
        let side = TAU * radius * (height - fillet);
        let end = PI * (radius - fillet).powi(2);
        let bottom = PI * radius.powi(2);
        contains([b.blend_lower_m2, b.blend_upper_m2], blend);
        contains(b.retained_side_m2, side);
        contains(b.retained_end_m2, end);
        contains(b.bottom_m2, bottom);
        contains(
            [b.body_lower_m2, b.body_upper_m2],
            blend + side + end + bottom,
        );
        assert_eq!(b.cells, 0);
        assert!(b.body_upper_m2 - b.body_lower_m2 <= 1e-15 * scale.powi(2));
        let integral = blend_area(r, 1024);
        assert!((integral - blend).abs() < 1e-12 * blend);
    }
}

#[test]
fn variable_and_boundary_area_enclose_independent_surface_and_support_integrals() {
    for (scale, minimum, maximum) in [
        (1., 0.0005, 0.0015),
        (0.1, 0.0001, 0.0025),
        (10., 0.0001, 0.0025),
    ] {
        let r = reference(scale, minimum, maximum);
        let b = r
            .surface_area_bounds_m2(1e-7 * scale * scale, 262_144)
            .unwrap();
        let coarse = blend_area(r, 128);
        let blend = blend_area(r, 256);
        assert!((coarse - blend).abs() < 1e-12 * scale * scale);
        contains([b.blend_lower_m2, b.blend_upper_m2], blend);
        let (radius, height) = (0.01 * scale, 0.02 * scale);
        // Integrate the unchanged supports directly in their own parameters.
        let radial =
            |theta: f64| radius - scale * (minimum + (maximum - minimum) * (1. + theta.cos()) / 2.);
        let end = simpson(|theta| radial(theta).powi(2) / 2., TAU, 256);
        let side = simpson(|theta| radius * (height - radius + radial(theta)), TAU, 256);
        let bottom = PI * radius * radius;
        // Quadrature roundoff is a test allowance, not part of the certificate.
        for (bounds, area) in [(b.retained_end_m2, end), (b.retained_side_m2, side)] {
            assert!(bounds[0] - 1e-17 * scale * scale <= area);
            assert!(area <= bounds[1] + 1e-17 * scale * scale);
        }
        contains(
            [b.body_lower_m2, b.body_upper_m2],
            blend + side + end + bottom,
        );
        assert!(b.body_upper_m2 - b.body_lower_m2 <= 1e-7 * scale * scale);
        assert!(b.cells > 4 && b.cells <= 262_144);
    }
}

#[test]
fn tighter_total_area_width_refines_deterministically_without_mutation() {
    let r = reference(1., 0.0005, 0.0015);
    let coarse = r.surface_area_bounds_m2(1e-6, 262_144).unwrap();
    let fine = r.surface_area_bounds_m2(1e-7, 262_144).unwrap();
    assert!(fine.cells > coarse.cells);
    assert!(fine.body_lower_m2 >= coarse.body_lower_m2 - 1e-17);
    assert!(fine.body_upper_m2 <= coarse.body_upper_m2 + 1e-17);
    assert_eq!(
        fingerprint(fine),
        fingerprint(r.surface_area_bounds_m2(1e-7, 262_144).unwrap())
    );
    assert!(r.surface_area_bounds_m2(1e-20, 4).is_err());
    assert_eq!(
        fingerprint(coarse),
        fingerprint(r.surface_area_bounds_m2(1e-6, 262_144).unwrap())
    );
}

#[test]
fn invalid_requests_exhaustion_and_roundoff_floor_refuse() {
    let r = reference(1., 0.0005, 0.0015);
    for width in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(r.surface_area_bounds_m2(width, 4).is_err());
    }
    for cells in [0, 3, 262_145, usize::MAX] {
        assert!(r.surface_area_bounds_m2(1e-6, cells).is_err());
    }
    assert!(r.surface_area_bounds_m2(1e-7, 4).is_err());
    for r in [r, reference(1., 0.001, 0.001)] {
        assert!(r.surface_area_bounds_m2(f64::from_bits(1), 4).is_err());
    }
    let minimum = 0.001_f64;
    let maximum = f64::from_bits(minimum.to_bits() + 1);
    let tiny = reference(1., minimum, maximum)
        .surface_area_bounds_m2(1., 4)
        .unwrap();
    assert_eq!(
        tiny.cells, 4,
        "unequal authored radii must not snap to constant"
    );
}
