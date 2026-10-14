//! Independent profile/section checks; deliberately no physical-kernel claim.
use std::f64::consts::{PI, TAU};

use opencad_core::{Angle, Length};
use opencad_geometry::{MetricThreadSize, NominalMetricThread, ThreadHandedness};

const SIZES: [(MetricThreadSize, f64, f64); 7] = [
    (MetricThreadSize::M3, 3.0, 0.5),
    (MetricThreadSize::M4, 4.0, 0.7),
    (MetricThreadSize::M5, 5.0, 0.8),
    (MetricThreadSize::M6, 6.0, 1.0),
    (MetricThreadSize::M8, 8.0, 1.25),
    (MetricThreadSize::M10, 10.0, 1.5),
    (MetricThreadSize::M12, 12.0, 1.75),
];

fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual:e} != {expected:e}, tolerance {tolerance:e}"
    );
}

fn radius(thread: NominalMetricThread, z_m: f64, angle_rad: f64) -> f64 {
    thread
        .radius_at(Length::from_meters(z_m), Angle::from_radians(angle_rad))
        .unwrap()
        .meters()
}

#[test]
fn coarse_sizes_and_basic_diameters_are_explicitly_nominal() {
    for (size, major_mm, pitch_mm) in SIZES {
        let thread = NominalMetricThread::new(size, ThreadHandedness::Right, 8).unwrap();
        assert_eq!(thread.size(), size);
        assert_eq!(thread.turns(), 8);
        assert_eq!(thread.handedness(), ThreadHandedness::Right);
        near(thread.major_diameter().millimeters(), major_mm, 1e-12);
        near(thread.pitch().millimeters(), pitch_mm, 1e-12);
        near(thread.length().millimeters(), 8.0 * pitch_mm, 1e-12);
    }
    let m6 = NominalMetricThread::new(MetricThreadSize::M6, ThreadHandedness::Right, 1).unwrap();
    near(
        m6.basic_minor_diameter().millimeters(),
        4.917_468_245_269_451,
        1e-12,
    );
    near(
        m6.pitch_diameter().millimeters(),
        5.350_480_947_161_671,
        1e-12,
    );
    // Rounded external d3=4.773130677972 is intentionally a different model.
    assert!((m6.basic_minor_diameter().millimeters() - 4.773_130_677_972).abs() > 0.14);
}

#[test]
fn meridional_crest_root_flanks_and_pitch_diameter_match_independent_dimensions() {
    for (size, major_mm, pitch_mm) in SIZES {
        let thread = NominalMetricThread::new(size, ThreadHandedness::Right, 2).unwrap();
        let p = pitch_mm * 1e-3;
        let crest = major_mm * 0.5e-3;
        let root = crest - p * 0.541_265_877_365_274_1;
        for t in [0.0, 1.0 / 32.0, 1.0 / 16.0, 15.0 / 16.0, 1.0] {
            near(radius(thread, p * t, 0.0), crest, 1e-15);
        }
        for t in [3.0 / 8.0, 0.5, 5.0 / 8.0] {
            near(radius(thread, p * t, 0.0), root, 1e-15);
        }
        for (a, b) in [(0.125, 0.25), (0.75, 0.875)] {
            let slope = (radius(thread, p * b, 0.0) - radius(thread, p * a, 0.0)) / ((b - a) * p);
            near(slope.abs(), 3.0_f64.sqrt(), 1e-12);
            near(slope.abs().atan().to_degrees(), 60.0, 1e-10);
        }
        near(
            radius(thread, p * 0.25, 0.0) * 2.0,
            thread.pitch_diameter().meters(),
            1e-15,
        );
        // Probe outside the flat boundaries: they must already be real flanks.
        assert!(radius(thread, p * (1.0 / 16.0 + 1e-4), 0.0) < crest - 1e-9 * p);
        assert!(radius(thread, p * (3.0 / 8.0 - 1e-4), 0.0) > root + 1e-9 * p);
    }
}

#[test]
fn both_hands_follow_opposite_screw_motion_and_repeat_at_each_pitch() {
    for (size, _, pitch_mm) in SIZES {
        for (hand, sign) in [
            (ThreadHandedness::Right, 1.0),
            (ThreadHandedness::Left, -1.0),
        ] {
            let thread = NominalMetricThread::new(size, hand, 32).unwrap();
            let p = pitch_mm * 1e-3;
            for fraction in [0.0, 0.2, 0.35, 0.65, 0.9] {
                let z = fraction * p;
                near(
                    radius(thread, z, sign * fraction * TAU),
                    thread.major_diameter().meters() / 2.0,
                    1e-15,
                );
                for shift in [1.0, 8.0, 31.0] {
                    near(
                        radius(thread, z, 0.4),
                        radius(thread, z + shift * p, 0.4),
                        1e-15,
                    );
                }
                near(
                    radius(thread, z, 0.4),
                    radius(thread, z, 0.4 + 32.0 * TAU),
                    1e-15,
                );
                near(
                    radius(thread, z, 0.4),
                    radius(thread, z, 0.4 - 32.0 * TAU),
                    1e-15,
                );
            }
            let opposite = NominalMetricThread::new(
                size,
                if hand == ThreadHandedness::Right {
                    ThreadHandedness::Left
                } else {
                    ThreadHandedness::Right
                },
                32,
            )
            .unwrap();
            // Away from symmetric phases, a hand reversal changes the surface.
            assert!(
                (radius(thread, 0.2 * p, sign * 0.2 * TAU)
                    - radius(opposite, 0.2 * p, sign * 0.2 * TAU))
                .abs()
                    > 0.5 * p
            );
            near(
                radius(thread, thread.length().meters(), 0.7),
                radius(thread, 0.0, 0.7),
                1e-15,
            );
        }
    }
}

#[test]
fn independent_piece_and_angular_integrals_grade_ideal_area_and_volume() {
    for (size, major_mm, pitch_mm) in SIZES {
        for hand in [ThreadHandedness::Right, ThreadHandedness::Left] {
            let thread = NominalMetricThread::new(size, hand, 7).unwrap();
            let p = pitch_mm * 1e-3;
            let major = major_mm * 0.5e-3;
            let root = major - p * 0.541_265_877_365_274_1;
            // Integrate each linear flank using dr/dz, not the implementation's
            // quadratic coefficient formula or its radius_at helper.
            let integral = p * (root * root / 4.0 + major * major / 8.0)
                + 2.0 * (major.powi(3) - root.powi(3)) / (3.0 * 3.0_f64.sqrt());
            let expected_area = PI * integral / p;
            near(thread.ideal_section_area_m2(), expected_area, 1e-18);
            near(thread.ideal_volume_m3(), expected_area * 7.0 * p, 1e-20);
            assert!(thread.ideal_volume_m3() < PI * major * major * 7.0 * p);
            assert!(thread.ideal_volume_m3() > PI * root * root * 7.0 * p);

            // Numerical polar integration at arbitrary axial slices independently
            // checks phase rotation, endpoint caps and both hands.
            const SAMPLES: u32 = 8192;
            for fraction in [0.0, 0.217, 0.5, 7.0] {
                let mut area = 0.0;
                for i in 0..SAMPLES {
                    let theta = TAU * (f64::from(i) + 0.5) / f64::from(SAMPLES);
                    area += radius(thread, fraction * p, theta).powi(2) * TAU
                        / (2.0 * f64::from(SAMPLES));
                }
                near(area, expected_area, expected_area * 1e-7);
            }
        }
    }
}

#[test]
fn invalid_count_nonfinite_phase_and_outside_square_ends_refuse() {
    for count in [0, 33, u32::MAX] {
        assert!(
            NominalMetricThread::new(MetricThreadSize::M6, ThreadHandedness::Right, count).is_err()
        );
    }
    let thread = NominalMetricThread::new(MetricThreadSize::M6, ThreadHandedness::Left, 1).unwrap();
    for z in [
        -1e-12,
        0.001 + 1e-12,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert!(thread
            .radius_at(Length::from_meters(z), Angle::from_radians(0.0))
            .is_err());
    }
    for theta in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        65.0 * TAU,
        -65.0 * TAU,
    ] {
        assert!(thread
            .radius_at(Length::from_meters(0.0), Angle::from_radians(theta))
            .is_err());
    }
    assert!(thread
        .radius_at(Length::from_meters(0.0), Angle::from_radians(64.0 * TAU))
        .is_ok());
    assert!(thread
        .radius_at(thread.length(), Angle::from_radians(-64.0 * TAU))
        .is_ok());
}
