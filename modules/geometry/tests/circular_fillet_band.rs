use opencad_core::Length;
use opencad_geometry::{CircularFilletBezierBand, CircularFilletReference};
fn reference(scale: f64) -> CircularFilletReference {
    CircularFilletReference::new(
        Length::from_meters(0.01 * scale),
        Length::from_meters(0.02 * scale),
        Length::from_meters(0.0005 * scale),
        Length::from_meters(0.0015 * scale),
    )
    .unwrap()
}
fn band() -> CircularFilletBezierBand {
    reference(1.)
        .bezier_band(Length::from_meters(1e-4), 0.8, 4096)
        .unwrap()
}
fn same(a: [f64; 3], b: [f64; 3]) {
    assert_eq!(a.map(f64::to_bits), b.map(f64::to_bits));
}
#[test]
fn full_cover_has_conforming_shared_edges_and_independent_contact_dimensions() {
    let b = band();
    let n = b.divisions_per_chart_axis;
    assert!(n.is_power_of_two());
    assert_eq!(b.cells.len(), 4 * n * n);
    assert!(b.position_error_upper_m <= 1e-4 && b.normal_error_upper <= 0.8);
    let cell = |chart, i, j| &b.cells[(chart * n + i) * n + j];
    for chart in 0..4 {
        for i in 0..n {
            for j in 0..n {
                let c = cell(chart, i, j);
                assert_eq!(usize::from(c.chart), chart);
                // Exact authored grid representation, not geometric equality.
                assert_eq!(
                    c.chart_domain.map(f64::to_bits),
                    [
                        i as f64 / n as f64,
                        (i + 1) as f64 / n as f64,
                        j as f64 / n as f64,
                        (j + 1) as f64 / n as f64
                    ]
                    .map(f64::to_bits)
                );
                assert!(
                    c.normals.jacobian_area_lower_m2 > 0.
                        && c.normals.ideal_normal_alignment_lower > 0.
                );
                let next = if i + 1 < n {
                    cell(chart, i + 1, j)
                } else {
                    cell((chart + 1) % 4, 0, j)
                };
                for k in 0..4 {
                    same(
                        c.patch.control_points_m[3][k],
                        next.patch.control_points_m[0][k],
                    );
                }
                if j + 1 < n {
                    for k in 0..4 {
                        same(
                            c.patch.control_points_m[k][3],
                            cell(chart, i, j + 1).patch.control_points_m[k][0],
                        );
                    }
                }
                if j == 0 || j + 1 == n {
                    let k = if j == 0 { 0 } else { 3 };
                    let p = c.patch.control_points_m[0][k];
                    let theta = p[1].atan2(p[0]);
                    let radius = 0.001 + 0.0005 * theta.cos();
                    let rho = p[0].hypot(p[1]);
                    if j == 0 {
                        assert!((rho - 0.01).abs() < 1e-12);
                        assert!((p[2] - (0.02 - radius)).abs() < 1e-12);
                    } else {
                        assert!((p[2] - 0.02).abs() < 1e-12);
                        assert!((rho - (0.01 - radius)).abs() < 1e-12);
                    }
                }
            }
        }
    }
}
#[test]
fn refinement_budget_refusal_and_determinism_are_explicit() {
    let r = reference(1.);
    let fine = band();
    let coarse = r.bezier_band(Length::from_meters(1e-3), 2., 4096).unwrap();
    assert!(coarse.divisions_per_chart_axis < fine.divisions_per_chart_axis);
    assert!(r
        .bezier_band(Length::from_meters(1e-4), 0.8, fine.cells.len() - 1)
        .is_err());
    let repeated = band();
    for (a, b) in fine.cells.into_iter().zip(repeated.cells) {
        for i in 0..4 {
            for j in 0..4 {
                same(
                    a.patch.control_points_m[i][j],
                    b.patch.control_points_m[i][j],
                );
            }
        }
    }
}
#[test]
fn requested_si_error_scales_with_the_whole_band() {
    let original = band();
    for scale in [1e-4, 1000.] {
        let b = reference(scale)
            .bezier_band(Length::from_meters(1e-4 * scale), 0.8, 4096)
            .unwrap();
        assert_eq!(
            b.divisions_per_chart_axis,
            original.divisions_per_chart_axis
        );
        assert!(
            (b.position_error_upper_m / scale - original.position_error_upper_m).abs()
                < 1e-8 * original.position_error_upper_m
        );
        assert!((b.normal_error_upper - original.normal_error_upper).abs() < 1e-8);
    }
}
#[test]
fn malformed_or_unreachable_quality_never_returns_a_partial_cover() {
    let r = reference(1.);
    for (position, normal, budget) in [
        (0., 0.8, 4096),
        (-1., 0.8, 4096),
        (f64::NAN, 0.8, 4096),
        (1e-4, 0., 4096),
        (1e-4, 2.1, 4096),
        (1e-4, f64::INFINITY, 4096),
        (1e-4, 0.8, 3),
        (1e-4, 0.8, 16385),
        (1e-4, 0.8, usize::MAX),
        (1e-12, 1e-12, 16),
    ] {
        assert!(r
            .bezier_band(Length::from_meters(position), normal, budget)
            .is_err());
    }
}
