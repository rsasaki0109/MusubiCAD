use opencad_core::Length;
use opencad_geometry::{
    CircularFilletContactKind as Kind, CircularFilletContactSnapshot as Snapshot,
    CircularFilletReference,
};
fn reference(scale: f64, variable: bool) -> CircularFilletReference {
    CircularFilletReference::new(
        Length::from_meters(0.01 * scale),
        Length::from_meters(0.02 * scale),
        Length::from_meters(0.0005 * scale),
        Length::from_meters(if variable { 0.0015 } else { 0.0005 } * scale),
    )
    .unwrap()
}
fn raw(r: CircularFilletReference, kind: Kind, chart: u8) -> (Vec<[f64; 3]>, Vec<f64>) {
    let curve = r.rational_contact_curve(kind, chart).unwrap();
    (
        curve
            .control_points_m()
            .iter()
            .map(|p| p.map(|x| x * 1000.))
            .collect(),
        curve.weights().to_vec(),
    )
}
// Independent rational de Casteljau evaluator, not the proof's coefficient hull.
fn point(poles_mm: &[[f64; 3]], weights: &[f64], t: f64) -> [f64; 3] {
    let mut h: Vec<[f64; 4]> = poles_mm
        .iter()
        .zip(weights)
        .map(|(p, w)| [p[0] * w, p[1] * w, p[2] * w, *w])
        .collect();
    for n in (1..h.len()).rev() {
        for i in 0..n {
            h[i] = std::array::from_fn(|a| (1. - t) * h[i][a] + t * h[i + 1][a]);
        }
    }
    std::array::from_fn(|a| h[0][a] / h[0][3] / 1000.)
}
fn gap(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}
#[test]
fn entire_curve_bounds_enclose_independent_analytic_contacts_at_scales_and_gauges() {
    for scale in [0.001, 1., 1000.] {
        for variable in [false, true] {
            let r = reference(scale, variable);
            for kind in [Kind::Side, Kind::End] {
                for chart in 0..4 {
                    let (poles, weights) = raw(r, kind, chart);
                    for gauge in [0.125, 1., 1024.] {
                        let weights: Vec<_> = weights.iter().map(|w| w * gauge).collect();
                        let snapshot =
                            Snapshot::from_millimeter_controls(kind, &poles, &weights).unwrap();
                        let bound = r
                            .certify_contact_snapshot(
                                chart,
                                &snapshot,
                                Length::from_meters(1e-12 * scale),
                            )
                            .unwrap()
                            .position_error_upper_m;
                        assert!(bound > 0. && bound < 1e-12 * scale);
                        for i in 0..=97 {
                            let t = i as f64 / 97.;
                            let theta =
                                f64::from(chart) * std::f64::consts::FRAC_PI_2 + 2. * t.atan();
                            let radius = if variable {
                                0.001 + 0.0005 * theta.cos()
                            } else {
                                0.0005
                            } * scale;
                            let rho = if kind == Kind::Side {
                                0.01 * scale
                            } else {
                                0.01 * scale - radius
                            };
                            let expected = [
                                rho * theta.cos(),
                                rho * theta.sin(),
                                if kind == Kind::Side {
                                    0.02 * scale - radius
                                } else {
                                    0.02 * scale
                                },
                            ];
                            assert!(
                                gap(point(&poles, &weights, t), expected) <= bound + 1e-16 * scale
                            );
                        }
                        let repeated = r
                            .certify_contact_snapshot(
                                chart,
                                &snapshot,
                                Length::from_meters(1e-12 * scale),
                            )
                            .unwrap();
                        assert_eq!(bound.to_bits(), repeated.position_error_upper_m.to_bits());
                    }
                }
            }
        }
    }
}
#[test]
fn pole_and_nonuniform_weight_changes_receive_real_bounds_or_refuse() {
    let r = reference(1., true);
    for kind in [Kind::Side, Kind::End] {
        let (mut poles, weights) = raw(r, kind, 0);
        // All native poles shifted by exactly 1/1024 mm along z. An independent
        // endpoint dimension witnesses the real physical error, not a mock ID.
        for p in &mut poles {
            p[2] += 1. / 1024.;
        }
        let snapshot = Snapshot::from_millimeter_controls(kind, &poles, &weights).unwrap();
        assert!(r
            .certify_contact_snapshot(0, &snapshot, Length::from_meters(1e-9))
            .is_err());
        let bound = r
            .certify_contact_snapshot(0, &snapshot, Length::from_meters(1e-5))
            .unwrap();
        assert!(bound.position_error_upper_m >= 1. / 1_024_000.);
        let (poles, mut weights) = raw(r, kind, 0);
        weights[1] *= 1.0001;
        let snapshot = Snapshot::from_millimeter_controls(kind, &poles, &weights).unwrap();
        assert!(r
            .certify_contact_snapshot(0, &snapshot, Length::from_meters(1e-12))
            .is_err());
        let bound = r
            .certify_contact_snapshot(0, &snapshot, Length::from_meters(1e-4))
            .unwrap();
        let curve = r.rational_contact_curve(kind, 0).unwrap();
        let supplied: Vec<_> = curve
            .control_points_m()
            .iter()
            .map(|p| p.map(|x| x * 1000.))
            .collect();
        for i in 0..=97 {
            let t = i as f64 / 97.;
            assert!(
                gap(
                    point(&poles, &weights, t),
                    point(&supplied, curve.weights(), t)
                ) <= bound.position_error_upper_m
            );
        }
    }
}
#[test]
fn invalid_layout_units_phase_kind_chart_and_precision_refuse_without_mutation() {
    let r = reference(1., true);
    for kind in [Kind::Side, Kind::End] {
        let (poles, weights) = raw(r, kind, 0);
        let snapshot = Snapshot::from_millimeter_controls(kind, &poles, &weights).unwrap();
        for maximum in [0., -1., f64::NAN, f64::INFINITY, 1e-30] {
            assert!(r
                .certify_contact_snapshot(0, &snapshot, Length::from_meters(maximum))
                .is_err());
        }
        for chart in [1, 2, 3, 4, 255] {
            assert!(r
                .certify_contact_snapshot(chart, &snapshot, Length::from_meters(1e-12))
                .is_err());
        }
        assert!(Snapshot::from_millimeter_controls(kind, &poles[..2], &weights).is_err());
        assert!(Snapshot::from_millimeter_controls(kind, &poles, &weights[..2]).is_err());
        let other = if kind == Kind::Side {
            Kind::End
        } else {
            Kind::Side
        };
        assert!(Snapshot::from_millimeter_controls(other, &poles, &weights).is_err());
        for value in [0., -1., f64::NAN, f64::INFINITY] {
            let mut bad = weights.clone();
            bad[1] = value;
            assert!(Snapshot::from_millimeter_controls(kind, &poles, &bad).is_err());
        }
        for value in [f64::NAN, f64::INFINITY] {
            let mut bad = poles.clone();
            bad[1][0] = value;
            assert!(Snapshot::from_millimeter_controls(kind, &bad, &weights).is_err());
        }
        let wrong_units: Vec<_> = poles.iter().map(|p| p.map(|x| x / 1000.)).collect();
        let bad = Snapshot::from_millimeter_controls(kind, &wrong_units, &weights).unwrap();
        assert!(r
            .certify_contact_snapshot(0, &bad, Length::from_meters(1e-12))
            .is_err());
        assert!(Snapshot::from_millimeter_controls(
            kind,
            &vec![[f64::MAX; 3]; weights.len()],
            &vec![f64::MAX; weights.len()]
        )
        .is_err());
        assert!(r
            .certify_contact_snapshot(0, &snapshot, Length::from_meters(1e-12))
            .is_ok());
        assert_eq!(poles, raw(r, kind, 0).0);
    }
}
