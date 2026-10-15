use opencad_core::Length;
use opencad_geometry::{
    CircularFilletContactKind as Kind, CircularFilletContactSnapshot as Edge,
    CircularFilletEndBoundaryPrecision as Precision, CircularFilletPlanarContactSnapshot as Plane,
    CircularFilletRationalSnapshot as Blend, CircularFilletReference,
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
fn precision(scale: f64) -> Precision {
    Precision {
        maximum_gap: Length::from_meters(1e-12 * scale),
        maximum_blend_normal_error: 1e-9,
        max_leaves: 4096,
    }
}
fn raw_edge(r: CircularFilletReference, chart: u8) -> ([[f64; 3]; 5], [f64; 5]) {
    let c = r.rational_contact_curve(Kind::End, chart).unwrap();
    (
        std::array::from_fn(|i| c.control_points_m()[i].map(|x| x * 1000.)),
        std::array::from_fn(|i| c.weights()[i]),
    )
}
fn raw_blend(r: CircularFilletReference, chart: u8) -> ([[[f64; 3]; 3]; 9], [[f64; 3]; 9]) {
    let p = r.rational_patch(chart).unwrap();
    (
        p.control_points_m()
            .map(|row| row.map(|p| p.map(|x| x * 1000.))),
        *p.weights(),
    )
}
// Independent rational de Casteljau, no interval or certificate implementation.
fn point<const N: usize>(poles: &[[f64; 3]; N], weights: &[f64; N], t: f64) -> [f64; 3] {
    let mut h: [[f64; 4]; N] = std::array::from_fn(|i| {
        [
            poles[i][0] * weights[i],
            poles[i][1] * weights[i],
            poles[i][2] * weights[i],
            weights[i],
        ]
    });
    for n in (1..N).rev() {
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
fn full_end_bounds_enclose_independent_affine_and_analytic_contacts_at_scales() {
    for scale in [0.001, 1., 1000.] {
        for variable in [false, true] {
            let r = reference(scale, variable);
            for chart in 0..4 {
                let (p, w) = raw_edge(r, chart);
                let (bp, bw) = raw_blend(r, chart);
                let edge = Edge::from_millimeter_controls(Kind::End, &p, &w).unwrap();
                let blend = Blend::from_millimeter_controls(&bp, &bw).unwrap();
                for turned in [false, true] {
                    let origin = if turned {
                        [0.125 * scale, -0.25 * scale, 20. * scale]
                    } else {
                        [0., 0., 20. * scale]
                    };
                    let x = if turned { [0., 1., 0.] } else { [1., 0., 0.] };
                    let y = if turned { [-1., 0., 0.] } else { [0., 1., 0.] };
                    let uv = std::array::from_fn(|i| {
                        if turned {
                            [p[i][1] - origin[1], origin[0] - p[i][0]]
                        } else {
                            [p[i][0], p[i][1]]
                        }
                    });
                    // A common positive pcurve weight gauge must not change geometry.
                    let pw = w.map(|a| a * 8.);
                    let plane = Plane::from_millimeter_controls(origin, x, y, &uv, &pw).unwrap();
                    let b = r
                        .certify_end_boundary(chart, &edge, &plane, &blend, precision(scale))
                        .unwrap();
                    assert!(
                        b.edge_to_plane_gap_upper_m <= 1e-12 * scale
                            && b.edge_to_blend_gap_upper_m <= 1e-12 * scale
                            && b.plane_to_blend_gap_upper_m <= 1e-12 * scale
                    );
                    let lifted =
                        uv.map(|q| std::array::from_fn(|a| origin[a] + q[0] * x[a] + q[1] * y[a]));
                    let boundary = std::array::from_fn::<_, 9, _>(|i| bp[i][2]);
                    let boundary_weights = std::array::from_fn(|i| bw[i][2]);
                    for i in 0..=73 {
                        let t = i as f64 / 73.;
                        let theta = f64::from(chart) * std::f64::consts::FRAC_PI_2 + 2. * t.atan();
                        let radius = if variable {
                            0.001 + 0.0005 * theta.cos()
                        } else {
                            0.0005
                        } * scale;
                        let expected = [
                            (0.01 * scale - radius) * theta.cos(),
                            (0.01 * scale - radius) * theta.sin(),
                            0.02 * scale,
                        ];
                        let e = point(&p, &w, t);
                        let p = point(&lifted, &pw, t);
                        let n = point(&boundary, &boundary_weights, t);
                        let numerical = 2e-16 * scale;
                        assert!(gap(e, expected) <= b.edge_position_error_upper_m + numerical);
                        assert!(
                            gap(p, expected) <= b.plane_lift_position_error_upper_m + numerical
                        );
                        assert!(gap(n, expected) <= b.blend_position_error_upper_m + numerical);
                        assert!(gap(e, p) <= b.edge_to_plane_gap_upper_m + numerical);
                        assert!(gap(e, n) <= b.edge_to_blend_gap_upper_m + numerical);
                        assert!(gap(p, n) <= b.plane_to_blend_gap_upper_m + numerical);
                    }
                    let repeated = r
                        .certify_end_boundary(chart, &edge, &plane, &blend, precision(scale))
                        .unwrap();
                    assert_eq!(
                        b.plane_to_blend_gap_upper_m.to_bits(),
                        repeated.plane_to_blend_gap_upper_m.to_bits()
                    );
                }
            }
        }
    }
}
#[test]
fn actual_frame_pcurve_edge_and_blend_changes_refuse_tight_gap() {
    let r = reference(1., true);
    let (p, w) = raw_edge(r, 0);
    let (bp, bw) = raw_blend(r, 0);
    let uv = p.map(|q| [q[0], q[1]]);
    let edge = Edge::from_millimeter_controls(Kind::End, &p, &w).unwrap();
    let blend = Blend::from_millimeter_controls(&bp, &bw).unwrap();
    let plane = Plane::from_millimeter_controls([0., 0., 20.], [1., 0., 0.], [0., 1., 0.], &uv, &w)
        .unwrap();
    for changed in 0..5 {
        let mut origin = [0., 0., 20.];
        let mut x = [1., 0., 0.];
        let mut pc = uv;
        let mut weights = w;
        match changed {
            0 => origin[2] += 1. / 1024.,
            1 => x[1] = 0.001,
            2 => pc[2][0] += 0.001,
            3 => weights[2] *= 1.001,
            _ => pc = uv.map(|q| q.map(|a| a / 1000.)),
        }
        let bad = Plane::from_millimeter_controls(origin, x, [0., 1., 0.], &pc, &weights).unwrap();
        assert!(r
            .certify_end_boundary(0, &edge, &bad, &blend, precision(1.))
            .is_err());
    }
    let mut changed = p;
    changed[2][2] += 0.001;
    let bad = Edge::from_millimeter_controls(Kind::End, &changed, &w).unwrap();
    assert!(r
        .certify_end_boundary(0, &bad, &plane, &blend, precision(1.))
        .is_err());
    let mut changed = bp;
    changed[4][2][2] += 0.001;
    let bad = Blend::from_millimeter_controls(&changed, &bw).unwrap();
    assert!(r
        .certify_end_boundary(0, &edge, &plane, &bad, precision(1.))
        .is_err());
    // The same original immutable snapshots still pass after every refusal.
    assert!(r
        .certify_end_boundary(0, &edge, &plane, &blend, precision(1.))
        .is_ok());
}
#[test]
fn individually_small_errors_still_refuse_when_pairwise_sum_exceeds_budget() {
    let r = reference(1., true);
    let (mut p, w) = raw_edge(r, 0);
    let uv = p.map(|q| [q[0], q[1]]);
    for q in &mut p {
        q[2] += 1. / 1024.;
    }
    let edge = Edge::from_millimeter_controls(Kind::End, &p, &w).unwrap();
    let plane = Plane::from_millimeter_controls(
        [0., 0., 20. - 1. / 1024.],
        [1., 0., 0.],
        [0., 1., 0.],
        &uv,
        &w,
    )
    .unwrap();
    let (bp, bw) = raw_blend(r, 0);
    let blend = Blend::from_millimeter_controls(&bp, &bw).unwrap();
    let mut pr = precision(1.);
    pr.maximum_gap = Length::from_meters(1e-4);
    let b = r
        .certify_end_boundary(0, &edge, &plane, &blend, pr)
        .unwrap();
    let budget = 1.5
        * b.edge_position_error_upper_m
            .max(b.plane_lift_position_error_upper_m)
            .max(b.blend_position_error_upper_m);
    assert!(
        b.edge_position_error_upper_m < budget
            && b.plane_lift_position_error_upper_m < budget
            && b.blend_position_error_upper_m < budget
    );
    assert!(b.edge_to_plane_gap_upper_m > budget);
    pr.maximum_gap = Length::from_meters(budget);
    assert!(r
        .certify_end_boundary(0, &edge, &plane, &blend, pr)
        .is_err());
}
#[test]
fn invalid_inputs_side_kind_chart_precision_and_work_refuse() {
    let r = reference(1., true);
    let (p, w) = raw_edge(r, 0);
    let (bp, bw) = raw_blend(r, 0);
    let uv = p.map(|q| [q[0], q[1]]);
    let edge = Edge::from_millimeter_controls(Kind::End, &p, &w).unwrap();
    let blend = Blend::from_millimeter_controls(&bp, &bw).unwrap();
    let plane = Plane::from_millimeter_controls([0., 0., 20.], [1., 0., 0.], [0., 1., 0.], &uv, &w)
        .unwrap();
    for chart in [1, 4, 255] {
        assert!(r
            .certify_end_boundary(chart, &edge, &plane, &blend, precision(1.))
            .is_err());
    }
    let c = r.rational_contact_curve(Kind::Side, 0).unwrap();
    let side: Vec<_> = c
        .control_points_m()
        .iter()
        .map(|p| p.map(|x| x * 1000.))
        .collect();
    let side = Edge::from_millimeter_controls(Kind::Side, &side, c.weights()).unwrap();
    assert!(r
        .certify_end_boundary(0, &side, &plane, &blend, precision(1.))
        .is_err());
    for value in [0., -1., f64::NAN, f64::INFINITY, 1e-30] {
        let mut pr = precision(1.);
        pr.maximum_gap = Length::from_meters(value);
        assert!(r
            .certify_end_boundary(0, &edge, &plane, &blend, pr)
            .is_err());
    }
    for value in [0., 1., f64::NAN, 1e-30] {
        let mut pr = precision(1.);
        pr.maximum_blend_normal_error = value;
        assert!(r
            .certify_end_boundary(0, &edge, &plane, &blend, pr)
            .is_err());
    }
    for value in [0, 1, 16385] {
        let mut pr = precision(1.);
        pr.max_leaves = value;
        assert!(r
            .certify_end_boundary(0, &edge, &plane, &blend, pr)
            .is_err());
    }
    for value in [f64::NAN, f64::INFINITY] {
        assert!(Plane::from_millimeter_controls(
            [value, 0., 20.],
            [1., 0., 0.],
            [0., 1., 0.],
            &uv,
            &w
        )
        .is_err());
        assert!(Plane::from_millimeter_controls(
            [0., 0., 20.],
            [value, 0., 0.],
            [0., 1., 0.],
            &uv,
            &w
        )
        .is_err());
        let mut bad = uv;
        bad[1][0] = value;
        assert!(Plane::from_millimeter_controls(
            [0., 0., 20.],
            [1., 0., 0.],
            [0., 1., 0.],
            &bad,
            &w
        )
        .is_err());
    }
    for value in [0., -1., f64::NAN, f64::INFINITY] {
        let mut bad = w;
        bad[1] = value;
        assert!(Plane::from_millimeter_controls(
            [0., 0., 20.],
            [1., 0., 0.],
            [0., 1., 0.],
            &uv,
            &bad
        )
        .is_err());
    }
}
