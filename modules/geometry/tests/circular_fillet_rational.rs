//! Independent tensor evaluation and geometric oracles, not native evidence.
use opencad_core::{Angle, Length};
use opencad_geometry::{
    CircularFilletRationalPatch, CircularFilletRationalSnapshot, CircularFilletReference,
};
use std::f64::consts::{FRAC_PI_2, PI};

fn reference(scale: f64, variable: bool) -> CircularFilletReference {
    CircularFilletReference::new(
        Length::from_meters(0.01 * scale),
        Length::from_meters(0.02 * scale),
        Length::from_meters(0.0005 * scale),
        Length::from_meters(if variable { 0.0015 } else { 0.0005 } * scale),
    )
    .unwrap()
}
fn basis(n: usize, i: usize, t: f64) -> f64 {
    let mut c = 1_usize;
    for k in 0..i.min(n - i) {
        c = c * (n - k) / (k + 1);
    }
    c as f64 * t.powi(i as i32) * (1. - t).powi((n - i) as i32)
}
fn derivative_basis(n: usize, i: usize, t: f64) -> f64 {
    n as f64
        * (if i > 0 { basis(n - 1, i - 1, t) } else { 0. }
            - if i < n { basis(n - 1, i, t) } else { 0. })
}
fn evaluate(p: &CircularFilletRationalPatch, u: f64, v: f64) -> [[f64; 3]; 3] {
    evaluate_controls(p.control_points_m(), p.weights(), u, v)
}
fn evaluate_controls(
    poles: &[[[f64; 3]; 3]; 9],
    weights: &[[f64; 3]; 9],
    u: f64,
    v: f64,
) -> [[f64; 3]; 3] {
    let mut h = [[0.; 4]; 3];
    for i in 0..9 {
        for j in 0..3 {
            let factors = [
                basis(8, i, u) * basis(2, j, v),
                derivative_basis(8, i, u) * basis(2, j, v),
                basis(8, i, u) * derivative_basis(2, j, v),
            ];
            for k in 0..3 {
                for (axis, component) in h[k].iter_mut().enumerate() {
                    *component +=
                        factors[k] * weights[i][j] * if axis == 3 { 1. } else { poles[i][j][axis] };
                }
            }
        }
    }
    let point: [f64; 3] = std::array::from_fn(|k| h[0][k] / h[0][3]);
    [
        point,
        std::array::from_fn(|k| (h[1][k] - point[k] * h[1][3]) / h[0][3]),
        std::array::from_fn(|k| (h[2][k] - point[k] * h[2][3]) / h[0][3]),
    ]
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}
fn normal(du: [f64; 3], dv: [f64; 3]) -> [f64; 3] {
    let cross: [f64; 3] = std::array::from_fn(|k| {
        du[(k + 1) % 3] * dv[(k + 2) % 3] - du[(k + 2) % 3] * dv[(k + 1) % 3]
    });
    let length = distance(cross, [0.; 3]);
    assert!(length > 0.);
    cross.map(|x| x / length)
}

#[test]
fn tensor_data_matches_independent_sphere_envelope_and_square_root_reference() {
    for scale in [0.001, 1., 1000.] {
        for variable in [false, true] {
            let reference = reference(scale, variable);
            for chart in 0..4 {
                let patch = reference.rational_patch(chart).unwrap();
                assert_eq!(patch.chart(), chart);
                assert_eq!((patch.u_degree(), patch.v_degree()), (8, 2));
                assert!(patch
                    .weights()
                    .iter()
                    .flatten()
                    .all(|x| x.is_finite() && *x > 0.));
                assert!(
                    patch.position_error_upper_m() > 0.
                        && patch.position_error_upper_m() < 1e-10 * scale
                );
                for i in 0..=16 {
                    for j in 0..=16 {
                        let (u, v) = (i as f64 / 16., j as f64 / 16.);
                        let theta = chart as f64 * FRAC_PI_2 + 2. * u.atan();
                        let r = (0.0005
                            + if variable {
                                0.0005 * (1. + theta.cos())
                            } else {
                                0.
                            })
                            * scale;
                        let k = if variable {
                            -0.0005 * scale * theta.sin() / (0.01 * scale - r)
                        } else {
                            0.
                        };
                        // alpha is recovered from the unnormalized meridian; the
                        // existing oracle uses a distinct square-root formula.
                        let alpha = (2. * v + 2. * k * k * v * v).atan2(1. - v * v);
                        let sample = reference
                            .sample(
                                Angle::from_radians(theta.min(2. * PI)),
                                Angle::from_radians(alpha),
                            )
                            .unwrap();
                        let [point, du, dv] = evaluate(&patch, u, v);
                        assert!(
                            distance(point, sample.point_m)
                                <= patch.position_error_upper_m() + 1e-14 * scale
                        );
                        let measured_normal = normal(du, dv);
                        assert!(distance(measured_normal, sample.normal) < 1e-10);
                        let center = [
                            (0.01 * scale - r) * theta.cos(),
                            (0.01 * scale - r) * theta.sin(),
                            0.02 * scale - r,
                        ];
                        assert!((distance(point, center) - r).abs() < 1e-14 * scale);
                        let from_center: [f64; 3] =
                            std::array::from_fn(|axis| (point[axis] - center[axis]) / r);
                        let nr = from_center[0] * theta.cos() + from_center[1] * theta.sin();
                        let nt = -from_center[0] * theta.sin() + from_center[1] * theta.cos();
                        assert!((nt - k * (nr + from_center[2] - 1.)).abs() < 1e-10);
                    }
                }
            }
        }
    }
}

#[test]
fn contacts_constant_limit_phase_and_quarter_seams_match_independent_dimensions() {
    for variable in [false, true] {
        let reference = reference(1., variable);
        for chart in 0..4 {
            let patch = reference.rational_patch(chart).unwrap();
            let next = reference.rational_patch((chart + 1) % 4).unwrap();
            for j in 0..=16 {
                let t = j as f64 / 16.;
                let theta = chart as f64 * FRAC_PI_2 + 2. * t.atan();
                let r = 0.0005
                    + if variable {
                        0.0005 * (1. + theta.cos())
                    } else {
                        0.
                    };
                let [side, su, sv] = evaluate(&patch, t, 0.);
                let [end, eu, ev] = evaluate(&patch, t, 1.);
                assert!(distance(side, [0.01 * theta.cos(), 0.01 * theta.sin(), 0.02 - r]) < 1e-14);
                assert!(
                    distance(
                        end,
                        [(0.01 - r) * theta.cos(), (0.01 - r) * theta.sin(), 0.02]
                    ) < 1e-14
                );
                assert!(distance(normal(su, sv), [theta.cos(), theta.sin(), 0.]) < 1e-10);
                assert!(distance(normal(eu, ev), [0., 0., 1.]) < 1e-10);
                let [left, lu, lv] = evaluate(&patch, 1., t);
                let [right, ru, rv] = evaluate(&next, 0., t);
                assert!(distance(left, right) < 1e-14);
                assert!(distance(normal(lu, lv), normal(ru, rv)) < 1e-10);
                assert!(distance(ru, lu.map(|x| 2. * x)) < 1e-12);
                if !variable {
                    let [point, _, _] = evaluate(&patch, t, t);
                    let cosine = (1. - t * t) / (1. + t * t);
                    let sine = 2. * t / (1. + t * t);
                    assert!(
                        distance(
                            point,
                            [
                                (0.0095 + 0.0005 * cosine) * theta.cos(),
                                (0.0095 + 0.0005 * cosine) * theta.sin(),
                                0.0195 + 0.0005 * sine
                            ]
                        ) < 1e-14
                    );
                }
            }
        }
    }
    let patch = reference(1., true).rational_patch(1).unwrap();
    let [point, _, _] = evaluate(&patch, 0., 0.5);
    // At theta=pi/2 the actual envelope has a tangential displacement. A
    // revolved variable quarter-circle, despite correct contacts, fails this.
    assert!(point[0] > 1e-5);
}

#[test]
fn admission_boundary_and_tiny_amplitude_remain_finite_and_deterministic() {
    for (radius, height, minimum, maximum) in [
        (0.004, 0.004, 0.0002, 0.001),
        (0.01, 0.02, 0.0005, 0.0005 + 1e-18),
    ] {
        let reference = CircularFilletReference::new(
            Length::from_meters(radius),
            Length::from_meters(height),
            Length::from_meters(minimum),
            Length::from_meters(maximum),
        )
        .unwrap();
        for chart in 0..4 {
            let a = reference.rational_patch(chart).unwrap();
            let b = reference.rational_patch(chart).unwrap();
            assert_eq!(a.control_points_m(), b.control_points_m());
            assert_eq!(a.weights(), b.weights());
            assert_eq!(
                a.position_error_upper_m().to_bits(),
                b.position_error_upper_m().to_bits()
            );
            for v in [0., 0.5, 1.] {
                let [p, du, dv] = evaluate(&a, 0.5, v);
                assert!(p.into_iter().chain(du).chain(dv).all(f64::is_finite));
                assert!(normal(du, dv).into_iter().all(f64::is_finite));
            }
        }
    }
}

#[test]
fn invalid_charts_refuse_without_wrapping_or_mutating_reference() {
    let reference = reference(1., true);
    let before = reference.rational_patch(0).unwrap();
    for chart in [4, 5, 255] {
        assert!(reference.rational_patch(chart).is_err());
        assert!(reference.cylindrical_contact_pcurve(chart).is_err());
    }
    let after = reference.rational_patch(0).unwrap();
    assert_eq!(before.control_points_m(), after.control_points_m());
    assert_eq!(before.weights(), after.weights());
}

#[test]
fn actual_pcurve_controls_bound_off_knot_angles_heights_and_cylinder_lifts() {
    for scale in [0.001, 1., 1000.] {
        for variable in [false, true] {
            let reference = reference(scale, variable);
            for chart in 0..4 {
                let pc = reference.cylindrical_contact_pcurve(chart).unwrap();
                let again = reference.cylindrical_contact_pcurve(chart).unwrap();
                assert_eq!(pc.control_points_rad_m(), again.control_points_rad_m());
                assert_eq!(
                    pc.position_error_upper_m().to_bits(),
                    again.position_error_upper_m().to_bits()
                );
                assert_eq!(pc.chart(), chart);
                assert_eq!(pc.divisions(), 256);
                assert_eq!(pc.control_points_rad_m().len(), 769);
                assert!(pc.angle_error_upper_rad() > 0. && pc.angle_error_upper_rad() < 1e-9);
                assert!(
                    pc.height_error_upper_m() > 0. && pc.height_error_upper_m() < 1e-11 * scale
                );
                assert!(
                    pc.position_error_upper_m() > 0. && pc.position_error_upper_m() < 1e-11 * scale
                );
                for span in 0..256 {
                    for local in [0., 0.25, 0.5, 0.75, 1.] {
                        let t = (span as f64 + local) / 256.;
                        let measured: [f64; 2] = std::array::from_fn(|axis| {
                            (0..4)
                                .map(|j| {
                                    basis(3, j, local)
                                        * pc.control_points_rad_m()[3 * span + j][axis]
                                })
                                .sum()
                        });
                        let angle = chart as f64 * FRAC_PI_2 + 2. * t.atan();
                        let z = (0.02
                            - 0.0005
                            - if variable {
                                0.0005 * (1. + angle.cos())
                            } else {
                                0.
                            })
                            * scale;
                        assert!((measured[0] - angle).abs() <= pc.angle_error_upper_rad() + 1e-14);
                        assert!(
                            (measured[1] - z).abs() <= pc.height_error_upper_m() + 1e-16 * scale
                        );
                        let p = [
                            0.01 * scale * measured[0].cos(),
                            0.01 * scale * measured[0].sin(),
                            measured[1],
                        ];
                        let q = [0.01 * scale * angle.cos(), 0.01 * scale * angle.sin(), z];
                        assert!(distance(p, q) <= pc.position_error_upper_m() + 1e-16 * scale);
                    }
                }
            }
        }
    }
}

#[test]
fn rounded_normal_certificates_enclose_independent_sphere_normals_and_jacobians() {
    // These samples are regression oracles. Complete coverage comes from the
    // certificate's interval cover, never from the density of this test grid.
    for scale in [0.001, 1., 1000.] {
        for variable in [false, true] {
            let reference = reference(scale, variable);
            for chart in 0..4 {
                let patch = reference.rational_patch(chart).unwrap();
                let bounds = patch.normal_bounds(1e-9, 4096).unwrap();
                assert!(bounds.jacobian_area_lower_m2 > 0.);
                assert!(bounds.ideal_normal_alignment_lower > 0.);
                assert!(bounds.normal_error_upper > 0. && bounds.normal_error_upper <= 1e-9);
                assert!(bounds.du_error_upper_m > 0. && bounds.dv_error_upper_m > 0.);
                assert!((1..=4096).contains(&bounds.leaves));
                for i in 0..=12 {
                    for j in 0..=12 {
                        let (u, v) = (i as f64 / 12., j as f64 / 12.);
                        let theta = chart as f64 * FRAC_PI_2 + 2. * u.atan();
                        let r = (0.0005
                            + if variable {
                                0.0005 * (1. + theta.cos())
                            } else {
                                0.
                            })
                            * scale;
                        let k = if variable {
                            -0.0005 * scale * theta.sin() / (0.01 * scale - r)
                        } else {
                            0.
                        };
                        let alpha = (2. * v + 2. * k * k * v * v).atan2(1. - v * v);
                        let ideal = reference
                            .sample(
                                Angle::from_radians(theta.min(2. * PI)),
                                Angle::from_radians(alpha),
                            )
                            .unwrap();
                        let [_, du, dv] = evaluate(&patch, u, v);
                        let actual = normal(du, dv);
                        if !variable {
                            let rho = 0.01 * scale - r + r * alpha.cos();
                            let tu = 2. / (1. + u * u);
                            let av = 2. / (1. + v * v);
                            let exact_du = [-rho * theta.sin() * tu, rho * theta.cos() * tu, 0.];
                            let exact_dv = [
                                -r * alpha.sin() * theta.cos() * av,
                                -r * alpha.sin() * theta.sin() * av,
                                r * alpha.cos() * av,
                            ];
                            assert!(
                                distance(du, exact_du) <= bounds.du_error_upper_m + 1e-16 * scale
                            );
                            assert!(
                                distance(dv, exact_dv) <= bounds.dv_error_upper_m + 1e-16 * scale
                            );
                        }
                        let area = distance(
                            std::array::from_fn(|k| {
                                du[(k + 1) % 3] * dv[(k + 2) % 3]
                                    - du[(k + 2) % 3] * dv[(k + 1) % 3]
                            }),
                            [0.; 3],
                        );
                        assert!(
                            area >= bounds.jacobian_area_lower_m2
                                && area <= bounds.jacobian_area_upper_m2
                        );
                        assert!(
                            distance(actual, ideal.normal) <= bounds.normal_error_upper + 1e-13
                        );
                        let alignment: f64 = actual
                            .into_iter()
                            .zip(ideal.normal)
                            .map(|(x, y)| x * y)
                            .sum();
                        assert!(alignment + 1e-14 >= bounds.ideal_normal_alignment_lower);
                    }
                }
                if !variable {
                    // Exact toroidal chart: theta_u,alpha_v in [1,2], and
                    // radial distance R-r+r*cos(alpha) in [R-r,R].
                    let min = (0.01 - 0.0005) * 0.0005 * scale * scale;
                    let max = 4. * 0.01 * 0.0005 * scale * scale;
                    assert!(bounds.jacobian_area_lower_m2 <= min);
                    assert!(bounds.jacobian_area_upper_m2 >= max);
                }
            }
        }
    }
}

#[test]
fn complete_certificates_imply_support_and_adjacent_chart_normal_bounds() {
    // Include the exact admission boundary R=4*r_max, H=4*r_max.
    let reference = CircularFilletReference::new(
        Length::from_meters(0.004),
        Length::from_meters(0.004),
        Length::from_meters(0.0002),
        Length::from_meters(0.001),
    )
    .unwrap();
    for chart in 0..4 {
        let patch = reference.rational_patch(chart).unwrap();
        let next = reference.rational_patch((chart + 1) % 4).unwrap();
        let a = patch.normal_bounds(1e-9, 4096).unwrap();
        let b = next.normal_bounds(1e-9, 4096).unwrap();
        let pc = reference.cylindrical_contact_pcurve(chart).unwrap();
        for i in 0..=32 {
            let t = i as f64 / 32.;
            let theta = chart as f64 * FRAC_PI_2 + 2. * t.atan();
            let [_, du, dv] = evaluate(&patch, t, 0.);
            assert!(
                distance(normal(du, dv), [theta.cos(), theta.sin(), 0.])
                    <= a.normal_error_upper + 1e-13
            );
            let span = (t * 256.).min(255.) as usize;
            let local = t * 256. - span as f64;
            let angle: f64 = (0..4)
                .map(|j| basis(3, j, local) * pc.control_points_rad_m()[3 * span + j][0])
                .sum();
            assert!(
                distance(normal(du, dv), [angle.cos(), angle.sin(), 0.])
                    <= a.normal_error_upper + pc.angle_error_upper_rad() + 1e-13
            );
            let [_, du, dv] = evaluate(&patch, t, 1.);
            assert!(distance(normal(du, dv), [0., 0., 1.]) <= a.normal_error_upper + 1e-13);
            let [_, du, dv] = evaluate(&patch, 1., t);
            let [_, nu, nv] = evaluate(&next, 0., t);
            assert!(
                distance(normal(du, dv), normal(nu, nv))
                    <= a.normal_error_upper + b.normal_error_upper + 1e-13
            );
        }
    }
}

#[test]
fn certificate_precision_and_work_refuse_deterministically_without_mutation() {
    let patch = reference(1., true).rational_patch(0).unwrap();
    let poles = *patch.control_points_m();
    let weights = *patch.weights();
    let before = patch.normal_bounds(1e-9, 4096).unwrap();
    assert!(before.leaves > 1); // Actually exercises restriction, including D1.
    for (error, leaves) in [
        (0., 4096),
        (-1., 4096),
        (1., 4096),
        (f64::NAN, 4096),
        (f64::INFINITY, 4096),
        (1e-9, 0),
        (1e-9, 16385),
        (1e-9, 1),
        (1e-30, 64),
    ] {
        assert!(patch.normal_bounds(error, leaves).is_err());
    }
    let after = patch.normal_bounds(1e-9, 4096).unwrap();
    assert_eq!(format!("{before:?}"), format!("{after:?}"));
    assert_eq!(patch.control_points_m(), &poles);
    assert_eq!(patch.weights(), &weights);
}

#[test]
fn native_snapshot_si_bounds_match_independent_envelope_and_constant_derivatives() {
    for scale in [0.001, 1., 1000.] {
        for variable in [false, true] {
            let reference = reference(scale, variable);
            for chart in 0..4 {
                let patch = reference.rational_patch(chart).unwrap();
                let mm = patch
                    .control_points_m()
                    .map(|row| row.map(|p| p.map(|x| x * 1000.)));
                let snapshot =
                    CircularFilletRationalSnapshot::from_millimeter_controls(&mm, patch.weights())
                        .unwrap();
                let cert = reference
                    .certify_rational_snapshot(
                        chart,
                        &snapshot,
                        Length::from_meters(1e-12 * scale),
                        1e-9,
                        4096,
                    )
                    .unwrap();
                assert!(
                    cert.position_error_upper_m > 0.
                        && cert.position_error_upper_m <= 1e-12 * scale
                );
                assert!(
                    cert.normal.jacobian_area_lower_m2 > 0.
                        && cert.normal.ideal_normal_alignment_lower > 0.
                );
                assert!(cert.normal.normal_error_upper <= 1e-9);
                let si = mm.map(|row| row.map(|p| p.map(|x| x / 1000.)));
                for i in 0..=8 {
                    for j in 0..=8 {
                        let (u, v) = (i as f64 / 8., j as f64 / 8.);
                        let theta = chart as f64 * FRAC_PI_2 + 2. * u.atan();
                        let r = (0.0005
                            + if variable {
                                0.0005 * (1. + theta.cos())
                            } else {
                                0.
                            })
                            * scale;
                        let k = if variable {
                            -0.0005 * scale * theta.sin() / (0.01 * scale - r)
                        } else {
                            0.
                        };
                        let alpha = (2. * v + 2. * k * k * v * v).atan2(1. - v * v);
                        let ideal = reference
                            .sample(
                                Angle::from_radians(theta.min(2. * PI)),
                                Angle::from_radians(alpha),
                            )
                            .unwrap();
                        let [p, du, dv] = evaluate_controls(&si, patch.weights(), u, v);
                        assert!(
                            distance(p, ideal.point_m)
                                <= cert.position_error_upper_m + 1e-16 * scale
                        );
                        assert!(
                            distance(normal(du, dv), ideal.normal)
                                <= cert.normal.normal_error_upper + 1e-13
                        );
                        let jacobian = distance(
                            std::array::from_fn(|k| {
                                du[(k + 1) % 3] * dv[(k + 2) % 3]
                                    - du[(k + 2) % 3] * dv[(k + 1) % 3]
                            }),
                            [0.; 3],
                        );
                        assert!(
                            cert.normal.jacobian_area_lower_m2 <= jacobian
                                && jacobian <= cert.normal.jacobian_area_upper_m2
                        );
                        if !variable {
                            let rho = 0.01 * scale - r + r * alpha.cos();
                            let tu = 2. / (1. + u * u);
                            let av = 2. / (1. + v * v);
                            assert!(
                                distance(du, [-rho * theta.sin() * tu, rho * theta.cos() * tu, 0.])
                                    <= cert.normal.du_error_upper_m + 1e-16 * scale
                            );
                            assert!(
                                distance(
                                    dv,
                                    [
                                        -r * alpha.sin() * theta.cos() * av,
                                        -r * alpha.sin() * theta.sin() * av,
                                        r * alpha.cos() * av
                                    ]
                                ) <= cert.normal.dv_error_upper_m + 1e-16 * scale
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn native_snapshot_weight_gauge_and_precision_work_are_deterministic() {
    let reference = reference(1., true);
    let patch = reference.rational_patch(0).unwrap();
    let mm = patch
        .control_points_m()
        .map(|row| row.map(|p| p.map(|x| x * 1000.)));
    for factor in [0.125, 1., 32.] {
        let weights = patch.weights().map(|row| row.map(|w| w * factor));
        let snapshot =
            CircularFilletRationalSnapshot::from_millimeter_controls(&mm, &weights).unwrap();
        let before = reference
            .certify_rational_snapshot(0, &snapshot, Length::from_meters(1e-12), 1e-9, 4096)
            .unwrap();
        for (pos, normal, leaves) in [
            (0., 1e-9, 4096),
            (-1., 1e-9, 4096),
            (f64::NAN, 1e-9, 4096),
            (f64::INFINITY, 1e-9, 4096),
            (1e-30, 1e-9, 64),
            (1e-12, 0., 4096),
            (1e-12, 1., 4096),
            (1e-12, f64::NAN, 4096),
            (1e-12, 1e-9, 0),
            (1e-12, 1e-9, 16385),
            (1e-12, 1e-9, 1),
        ] {
            assert!(reference
                .certify_rational_snapshot(0, &snapshot, Length::from_meters(pos), normal, leaves)
                .is_err());
        }
        let after = reference
            .certify_rational_snapshot(0, &snapshot, Length::from_meters(1e-12), 1e-9, 4096)
            .unwrap();
        assert_eq!(format!("{before:?}"), format!("{after:?}"));
        assert!(reference
            .certify_rational_snapshot(4, &snapshot, Length::from_meters(1e-12), 1e-9, 4096)
            .is_err());
    }
}

#[test]
fn native_snapshot_rejects_wrong_units_phase_deformation_weights_and_invalid_data() {
    let reference = reference(1., true);
    let patch = reference.rational_patch(0).unwrap();
    let mm = patch
        .control_points_m()
        .map(|row| row.map(|p| p.map(|x| x * 1000.)));
    let mut reversed = mm;
    for row in &mut reversed {
        row.reverse();
    }
    let mut changed = mm;
    changed[4][1][2] += 0.1;
    let rotated = mm.map(|row| row.map(|p| [-p[1], p[0], p[2]]));
    let mut nonuniform = *patch.weights();
    nonuniform[4][1] *= 1.001;
    for (poles, weights) in [
        (*patch.control_points_m(), *patch.weights()),
        ([[[0.; 3]; 3]; 9], *patch.weights()),
        (reversed, *patch.weights()),
        (changed, *patch.weights()),
        (rotated, *patch.weights()),
        (mm, nonuniform),
    ] {
        let snapshot =
            CircularFilletRationalSnapshot::from_millimeter_controls(&poles, &weights).unwrap();
        assert!(reference
            .certify_rational_snapshot(0, &snapshot, Length::from_meters(1e-12), 1e-9, 64)
            .is_err());
    }
    for value in [0., -1., f64::NAN, f64::INFINITY] {
        let mut weights = *patch.weights();
        weights[4][1] = value;
        assert!(CircularFilletRationalSnapshot::from_millimeter_controls(&mm, &weights).is_err());
    }
    for value in [f64::NAN, f64::INFINITY] {
        let mut poles = mm;
        poles[4][1][2] = value;
        assert!(
            CircularFilletRationalSnapshot::from_millimeter_controls(&poles, patch.weights())
                .is_err()
        );
    }
}
