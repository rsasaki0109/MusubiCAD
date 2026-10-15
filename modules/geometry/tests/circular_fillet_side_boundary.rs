use opencad_core::Length;
use opencad_geometry::{
    CircularFilletContactKind as Kind, CircularFilletContactSnapshot as Edge,
    CircularFilletCylindricalContactSnapshot as Cylinder, CircularFilletRationalSnapshot as Blend,
    CircularFilletReference, CircularFilletSideBoundaryPrecision as Precision,
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
fn axes() -> [[f64; 3]; 3] {
    [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
}
fn precision(scale: f64) -> Precision {
    Precision {
        maximum_gap: Length::from_meters(1e-12 * scale),
        maximum_blend_normal_error: 1e-9,
        max_leaves: 4096,
    }
}
fn controls(r: CircularFilletReference, chart: u8) -> Vec<[f64; 2]> {
    r.cylindrical_contact_pcurve(chart)
        .unwrap()
        .control_points_rad_m()
        .iter()
        .map(|p| [p[0], p[1] * 1000.])
        .collect()
}
fn edge(r: CircularFilletReference, chart: u8, z_mm: f64) -> Edge {
    let c = r.rational_contact_curve(Kind::Side, chart).unwrap();
    let p: Vec<_> = c
        .control_points_m()
        .iter()
        .map(|p| [p[0] * 1000., p[1] * 1000., p[2] * 1000. + z_mm])
        .collect();
    Edge::from_millimeter_controls(Kind::Side, &p, c.weights()).unwrap()
}
fn blend(r: CircularFilletReference, chart: u8, offset_mm: f64) -> Blend {
    let c = r.rational_patch(chart).unwrap();
    let mut p = c
        .control_points_m()
        .map(|row| row.map(|p| p.map(|x| x * 1000.)));
    p[4][0][2] += offset_mm;
    Blend::from_millimeter_controls(&p, c.weights()).unwrap()
}
// Independent cubic Bernstein evaluation and direct frame/cylinder lift.
fn point(
    p: &[[f64; 2]],
    origin: [f64; 3],
    directions: [[f64; 3]; 3],
    radius: f64,
    t: f64,
) -> [f64; 3] {
    let i = (t * 256.).floor().min(255.) as usize;
    let s = t * 256. - i as f64;
    let b = [
        (1. - s).powi(3),
        3. * s * (1. - s).powi(2),
        3. * s * s * (1. - s),
        s.powi(3),
    ];
    let q: [f64; 2] = std::array::from_fn(|a| (0..4).map(|j| b[j] * p[3 * i + j][a]).sum());
    std::array::from_fn(|a| {
        (origin[a]
            + radius * (q[0].cos() * directions[0][a] + q[0].sin() * directions[1][a])
            + q[1] * directions[2][a])
            / 1000.
    })
}
fn gap(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}
#[test]
fn complete_side_bounds_enclose_independent_analytic_cubic_and_frame_lifts() {
    for scale in [0.001, 1., 1000.] {
        for variable in [false, true] {
            let r = reference(scale, variable);
            for chart in 0..4 {
                let p = controls(r, chart);
                let e = edge(r, chart, 0.);
                let b = blend(r, chart, 0.);
                let c =
                    Cylinder::from_millimeter_controls([0.; 3], axes(), 10. * scale, &p).unwrap();
                let bound = r
                    .certify_side_boundary(chart, &e, &c, &b, precision(scale))
                    .unwrap();
                let lift = bound.cylindrical_lift;
                assert!(
                    bound.edge_to_cylinder_gap_upper_m <= 1e-12 * scale
                        && bound.edge_to_blend_gap_upper_m <= 1e-12 * scale
                        && bound.cylinder_to_blend_gap_upper_m <= 1e-12 * scale
                );
                for changed in [false, true] {
                    let origin = if changed {
                        [1e-9 * scale, -2e-9 * scale, 3e-9 * scale]
                    } else {
                        [0.; 3]
                    };
                    let mut directions = axes();
                    if changed {
                        directions[0][1] = 1e-10;
                        directions[2][2] += 1e-10;
                    }
                    let radius = 10. * scale + if changed { 1e-9 * scale } else { 0. };
                    let snapshot =
                        Cylinder::from_millimeter_controls(origin, directions, radius, &p).unwrap();
                    let c = r
                        .certify_cylindrical_contact_snapshot(
                            chart,
                            &snapshot,
                            Length::from_meters(1e-10 * scale),
                        )
                        .unwrap();
                    if changed {
                        assert!(c.support_frame_error_upper_m > lift.support_frame_error_upper_m);
                    }
                    for i in 0..=73 {
                        let t = i as f64 / 73.;
                        let theta = f64::from(chart) * std::f64::consts::FRAC_PI_2 + 2. * t.atan();
                        let radius = if variable {
                            0.001 + 0.0005 * theta.cos()
                        } else {
                            0.0005
                        } * scale;
                        let ideal = [
                            0.01 * scale * theta.cos(),
                            0.01 * scale * theta.sin(),
                            0.02 * scale - radius,
                        ];
                        assert!(
                            gap(
                                point(
                                    &p,
                                    origin,
                                    directions,
                                    10. * scale + if changed { 1e-9 * scale } else { 0. },
                                    t
                                ),
                                ideal
                            ) <= c.position_error_upper_m + 2e-16 * scale
                        );
                    }
                }
                let again = r
                    .certify_side_boundary(chart, &e, &c, &b, precision(scale))
                    .unwrap();
                assert_eq!(
                    bound.cylinder_to_blend_gap_upper_m.to_bits(),
                    again.cylinder_to_blend_gap_upper_m.to_bits()
                );
            }
        }
    }
}
#[test]
fn actual_control_frame_radius_edge_and_blend_changes_refuse_tight_gap() {
    let r = reference(1., true);
    let p = controls(r, 0);
    let e = edge(r, 0, 0.);
    let b = blend(r, 0, 0.);
    let cylinder = Cylinder::from_millimeter_controls([0.; 3], axes(), 10., &p).unwrap();
    for change in 0..7 {
        let mut q = p.clone();
        let mut origin = [0.; 3];
        let mut d = axes();
        let mut radius = 10.;
        match change {
            0 => origin[2] = 0.001,
            1 => d[0][1] = 0.001,
            2 => radius += 0.001,
            3 => q[384][0] += 1e-5,
            4 => q[384][1] += 0.001,
            5 => q = p.iter().map(|v| [v[0], v[1] / 1000.]).collect(),
            _ => {
                q = p
                    .iter()
                    .map(|v| [v[0] * 180. / std::f64::consts::PI, v[1]])
                    .collect()
            }
        }
        let bad = Cylinder::from_millimeter_controls(origin, d, radius, &q).unwrap();
        assert!(r
            .certify_side_boundary(0, &e, &bad, &b, precision(1.))
            .is_err());
    }
    assert!(r
        .certify_side_boundary(0, &edge(r, 0, 0.001), &cylinder, &b, precision(1.))
        .is_err());
    assert!(r
        .certify_side_boundary(0, &e, &cylinder, &blend(r, 0, 0.001), precision(1.))
        .is_err());
    assert!(r
        .certify_side_boundary(0, &e, &cylinder, &b, precision(1.))
        .is_ok());
}
#[test]
fn individually_small_errors_refuse_when_complete_pairwise_sum_exceeds_budget() {
    let r = reference(1., true);
    let p = controls(r, 0);
    let e = edge(r, 0, 1. / 1024.);
    let b = blend(r, 0, 0.);
    let c = Cylinder::from_millimeter_controls([0., 0., -1. / 1024.], axes(), 10., &p).unwrap();
    let mut pr = precision(1.);
    pr.maximum_gap = Length::from_meters(1e-4);
    let bounds = r.certify_side_boundary(0, &e, &c, &b, pr).unwrap();
    let individual = bounds
        .edge_position_error_upper_m
        .max(bounds.cylindrical_lift.position_error_upper_m)
        .max(bounds.blend_position_error_upper_m);
    let budget = (individual + bounds.edge_to_cylinder_gap_upper_m) / 2.;
    assert!(individual < budget && bounds.edge_to_cylinder_gap_upper_m > budget);
    pr.maximum_gap = Length::from_meters(budget);
    assert!(r.certify_side_boundary(0, &e, &c, &b, pr).is_err());
}
#[test]
fn invalid_kind_chart_controls_units_precision_work_and_overflow_refuse() {
    let r = reference(1., true);
    let p = controls(r, 0);
    let e = edge(r, 0, 0.);
    let b = blend(r, 0, 0.);
    let c = Cylinder::from_millimeter_controls([0.; 3], axes(), 10., &p).unwrap();
    let end = r.rational_contact_curve(Kind::End, 0).unwrap();
    let raw: Vec<_> = end
        .control_points_m()
        .iter()
        .map(|p| p.map(|x| x * 1000.))
        .collect();
    let end = Edge::from_millimeter_controls(Kind::End, &raw, end.weights()).unwrap();
    assert!(r
        .certify_side_boundary(0, &end, &c, &b, precision(1.))
        .is_err());
    for chart in [1, 4, 255] {
        assert!(r
            .certify_side_boundary(chart, &e, &c, &b, precision(1.))
            .is_err());
    }
    for value in [0., -1., f64::NAN, f64::INFINITY, 1e-30] {
        let mut pr = precision(1.);
        pr.maximum_gap = Length::from_meters(value);
        assert!(r.certify_side_boundary(0, &e, &c, &b, pr).is_err());
    }
    for value in [0., 1., f64::NAN, 1e-30] {
        let mut pr = precision(1.);
        pr.maximum_blend_normal_error = value;
        assert!(r.certify_side_boundary(0, &e, &c, &b, pr).is_err());
    }
    for value in [0, 1, 16385] {
        let mut pr = precision(1.);
        pr.max_leaves = value;
        assert!(r.certify_side_boundary(0, &e, &c, &b, pr).is_err());
    }
    for value in [0., -1., f64::NAN, f64::INFINITY, f64::from_bits(1)] {
        assert!(Cylinder::from_millimeter_controls([0.; 3], axes(), value, &p).is_err());
    }
    for n in [0, 768, 770] {
        assert!(
            Cylinder::from_millimeter_controls([0.; 3], axes(), 10., &vec![[0., 0.]; n]).is_err()
        );
    }
    for value in [f64::NAN, f64::INFINITY] {
        assert!(Cylinder::from_millimeter_controls([value, 0., 0.], axes(), 10., &p).is_err());
        let mut d = axes();
        d[0][0] = value;
        assert!(Cylinder::from_millimeter_controls([0.; 3], d, 10., &p).is_err());
        let mut q = p.clone();
        q[384][1] = value;
        assert!(Cylinder::from_millimeter_controls([0.; 3], axes(), 10., &q).is_err());
    }
    let huge = Cylinder::from_millimeter_controls([0.; 3], [[f64::MAX; 3]; 3], 10., &p).unwrap();
    assert!(r
        .certify_cylindrical_contact_snapshot(0, &huge, Length::from_meters(1.))
        .is_err());
}
