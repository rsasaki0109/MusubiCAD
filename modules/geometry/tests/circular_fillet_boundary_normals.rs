//! Independent rational derivative and analytic support witnesses (not proof samples).
use opencad_core::Length;
use opencad_geometry::{
    CircularFilletBoundaryNormalPrecision as Precision,
    CircularFilletCylindricalContactSnapshot as Cylinder, CircularFilletRationalPatch as Patch,
    CircularFilletRationalSnapshot as Surface, CircularFilletReference as Reference,
};
fn axes() -> [[f64; 3]; 3] {
    [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
}
fn reference(scale: f64, variable: bool) -> Reference {
    Reference::new(
        Length::from_meters(0.01 * scale),
        Length::from_meters(0.02 * scale),
        Length::from_meters(0.0005 * scale),
        Length::from_meters(if variable {
            0.0015 * scale
        } else {
            0.0005 * scale
        }),
    )
    .unwrap()
}
fn precision(scale: f64) -> Precision {
    Precision {
        maximum_position_error: Length::from_meters(1e-12 * scale),
        maximum_surface_normal_error: 2.5e-10,
        maximum_normal_gap: 1e-9,
        max_leaves: 4096,
    }
}
fn data(r: Reference) -> ([Surface; 4], [Cylinder; 4]) {
    (
        std::array::from_fn(|c| {
            let p = r.rational_patch(c as u8).unwrap();
            Surface::from_millimeter_controls(
                &p.control_points_m()
                    .map(|row| row.map(|p| p.map(|x| x * 1000.))),
                &p.weights().map(|row| row.map(|x| x * 8.)),
            )
            .unwrap()
        }),
        std::array::from_fn(|c| {
            let pc = r.cylindrical_contact_pcurve(c as u8).unwrap();
            let p: Vec<_> = pc
                .control_points_rad_m()
                .iter()
                .map(|p| [p[0], p[1] * 1000.])
                .collect();
            Cylinder::from_millimeter_controls(
                [0.; 3],
                axes(),
                r.cylinder_radius().meters() * 1000.,
                &p,
            )
            .unwrap()
        }),
    )
}
fn basis(n: usize, i: usize, t: f64) -> f64 {
    let mut choose = 1.;
    for k in 0..i {
        choose *= (n - k) as f64 / (k + 1) as f64;
    }
    choose * t.powi(i as i32) * (1. - t).powi((n - i) as i32)
}
fn derivative(n: usize, i: usize, t: f64) -> f64 {
    n as f64
        * ((if i > 0 { basis(n - 1, i - 1, t) } else { 0. })
            - (if i < n { basis(n - 1, i, t) } else { 0. }))
}
fn normal(p: &Patch, u: f64, v: f64) -> [f64; 3] {
    let mut h = [[0.; 4]; 3];
    let poles = p.control_points_m();
    let weights = p.weights();
    for (i, (row, ws)) in poles.iter().zip(weights).enumerate() {
        for (j, (pole, weight)) in row.iter().zip(ws).enumerate() {
            let factors = [
                basis(8, i, u) * basis(2, j, v),
                derivative(8, i, u) * basis(2, j, v),
                basis(8, i, u) * derivative(2, j, v),
            ];
            for (k, factor) in factors.into_iter().enumerate() {
                for (axis, value) in [pole[0], pole[1], pole[2], 1.].into_iter().enumerate() {
                    h[k][axis] += factor * weight * value;
                }
            }
        }
    }
    let d: [[f64; 3]; 2] = std::array::from_fn(|k| {
        std::array::from_fn(|a| (h[k + 1][a] - h[0][a] / h[0][3] * h[k + 1][3]) / h[0][3])
    });
    let n = [
        d[0][1] * d[1][2] - d[0][2] * d[1][1],
        d[0][2] * d[1][0] - d[0][0] * d[1][2],
        d[0][0] * d[1][1] - d[0][1] * d[1][0],
    ];
    let size = n.iter().map(|x| x * x).sum::<f64>().sqrt();
    n.map(|x| x / size)
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}
#[test]
fn independent_rational_boundary_normals_across_charts_laws_scales_and_gauges() {
    for scale in [0.1, 1., 10.] {
        for variable in [false, true] {
            let r = reference(scale, variable);
            let (s, c) = data(r);
            let b = r
                .certify_boundary_normals(&s, &c, [axes()[0], axes()[1]], precision(scale))
                .unwrap();
            for chart in 0..4 {
                let p = r.rational_patch(chart as u8).unwrap();
                let next = r.rational_patch(((chart + 1) % 4) as u8).unwrap();
                for t in [0_f64, 0.013, 0.37, 0.81, 1.] {
                    let theta = chart as f64 * std::f64::consts::FRAC_PI_2 + 2. * t.atan();
                    // Small independent evaluator rounding allowance, not a proof term.
                    assert!(
                        distance(normal(&p, t, 0.), [theta.cos(), theta.sin(), 0.])
                            <= b.side_normal_gap_upper[chart] + 5e-14
                    );
                    assert!(
                        distance(normal(&p, t, 1.), [0., 0., 1.])
                            <= b.end_normal_gap_upper[chart] + 5e-14
                    );
                    assert!(
                        distance(normal(&p, 1., t), normal(&next, 0., t))
                            <= b.seam_normal_gap_upper[chart] + 5e-14
                    );
                }
            }
        }
    }
}
#[test]
fn frame_angle_surface_changes_and_paired_budget_are_rechecked() {
    let r = reference(1., true);
    let (s, c) = data(r);
    let p = precision(1.);
    let b = r
        .certify_boundary_normals(&s, &c, [axes()[0], axes()[1]], p)
        .unwrap();
    let mut tight = p;
    let max_single = b
        .side_normal_gap_upper
        .iter()
        .chain(&b.end_normal_gap_upper)
        .copied()
        .fold(0_f64, f64::max);
    let max_seam = b
        .seam_normal_gap_upper
        .iter()
        .copied()
        .fold(0_f64, f64::max);
    assert!(max_single < max_seam);
    tight.maximum_normal_gap = (max_single + max_seam) * 0.5;
    assert!(b
        .side_normal_gap_upper
        .iter()
        .chain(&b.end_normal_gap_upper)
        .all(|x| *x < tight.maximum_normal_gap));
    assert!(b
        .seam_normal_gap_upper
        .iter()
        .any(|x| *x > tight.maximum_normal_gap));
    assert!(r
        .certify_boundary_normals(&s, &c, [axes()[0], axes()[1]], tight)
        .is_err());
    let mut plane = [axes()[0], axes()[1]];
    plane[0][2] = 1e-5;
    assert!(r.certify_boundary_normals(&s, &c, plane, p).is_err());
    let pc = r.cylindrical_contact_pcurve(0).unwrap();
    let mut controls: Vec<_> = pc
        .control_points_rad_m()
        .iter()
        .map(|p| [p[0], p[1] * 1000.])
        .collect();
    for q in &mut controls {
        q[0] += 1e-11;
    }
    let mut changed = c.clone();
    changed[0] = Cylinder::from_millimeter_controls([0.; 3], axes(), 10., &controls).unwrap();
    let shifted = r
        .certify_boundary_normals(&s, &changed, [axes()[0], axes()[1]], p)
        .unwrap();
    assert!(shifted.cylinder_normal_error_upper[0] > b.cylinder_normal_error_upper[0]);
    let mut frame = axes();
    frame[2][0] = 1e-5;
    changed[0] = Cylinder::from_millimeter_controls([0.; 3], frame, 10., &controls).unwrap();
    let mut loose = p;
    loose.maximum_position_error = Length::from_meters(1e-3);
    assert!(r
        .certify_boundary_normals(&s, &changed, [axes()[0], axes()[1]], loose)
        .is_err());
    let mut swapped = s.clone();
    swapped.swap(0, 1);
    assert!(r
        .certify_boundary_normals(&swapped, &c, [axes()[0], axes()[1]], p)
        .is_err());
}
#[test]
fn invalid_precision_work_units_and_orientation_refuse() {
    let r = reference(1., false);
    let (s, c) = data(r);
    let p = precision(1.);
    for max in [0., -1., 1., f64::INFINITY, f64::NAN, 1e-30] {
        let mut bad = p;
        bad.maximum_normal_gap = max;
        assert!(r
            .certify_boundary_normals(&s, &c, [axes()[0], axes()[1]], bad)
            .is_err());
    }
    for max in [0., 1., f64::NAN] {
        let mut bad = p;
        bad.maximum_surface_normal_error = max;
        assert!(r
            .certify_boundary_normals(&s, &c, [axes()[0], axes()[1]], bad)
            .is_err());
    }
    for max in [0., f64::NAN, 1e-30] {
        let mut bad = p;
        bad.maximum_position_error = Length::from_meters(max);
        assert!(r
            .certify_boundary_normals(&s, &c, [axes()[0], axes()[1]], bad)
            .is_err());
    }
    for leaves in [0, 1, 16385] {
        let mut bad = p;
        bad.max_leaves = leaves;
        assert!(r
            .certify_boundary_normals(&s, &c, [axes()[0], axes()[1]], bad)
            .is_err());
    }
    for plane in [
        [[0.; 3]; 2],
        [[1., 0., 0.], [0., -1., 0.]],
        [[f64::NAN; 3]; 2],
        [[1000., 0., 0.], [0., 1000., 0.]],
    ] {
        assert!(r.certify_boundary_normals(&s, &c, plane, p).is_err());
    }
}
