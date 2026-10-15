use opencad_core::Length;
use opencad_geometry::{
    CircularFilletBandCell, CircularFilletBezierBand, CircularFilletBezierPatch,
    CircularFilletBoundaryIntegrals, CircularFilletNormalBounds, CircularFilletReference,
};
fn prism(scale: f64, dx: f64, dy: f64) -> CircularFilletBezierBand {
    let corners = [
        [dx, dy],
        [dx + 2. * scale, dy],
        [dx + 2. * scale, dy + 3. * scale],
        [dx, dy + 3. * scale],
    ];
    let mut cells = Vec::new();
    for chart in 0..4 {
        let a = corners[chart];
        let b = corners[(chart + 1) % 4];
        let mut poles = [[[0.; 3]; 4]; 4];
        for (i, row) in poles.iter_mut().enumerate() {
            for (j, p) in row.iter_mut().enumerate() {
                let t = i as f64 / 3.;
                *p = [
                    if i == 3 {
                        b[0]
                    } else {
                        a[0] + (b[0] - a[0]) * t
                    },
                    if i == 3 {
                        b[1]
                    } else {
                        a[1] + (b[1] - a[1]) * t
                    },
                    j as f64 * 4. * scale / 3.,
                ];
            }
        }
        cells.push(CircularFilletBandCell {
            chart: chart as u8,
            chart_domain: [0., 1., 0., 1.],
            patch: CircularFilletBezierPatch {
                control_points_m: poles,
                reference_to_bilinear_error_upper_m: f64::NAN,
                control_net_deviation_upper_m: f64::NAN,
                position_error_upper_m: f64::NAN,
            },
            normals: CircularFilletNormalBounds {
                normal_lower: [0.; 3],
                normal_upper: [0.; 3],
                jacobian_area_lower_m2: 0.,
                jacobian_area_upper_m2: 0.,
                ideal_normal_alignment_lower: 0.,
                normal_error_upper: 0.,
            },
        });
    }
    CircularFilletBezierBand {
        divisions_per_chart_axis: 1,
        cells,
        position_error_upper_m: f64::NAN,
        normal_error_upper: f64::NAN,
    }
}
fn check(interval: [f64; 2], expected: f64, numerical: f64) {
    assert!(
        interval[0] <= expected + numerical && expected - numerical <= interval[1],
        "{interval:?} misses {expected}"
    );
    assert!(interval[1] - interval[0] < expected.abs().max(1e-30) * 1e-9 + numerical * 2.);
}
#[test]
fn independent_prism_volume_moments_translation_and_si_scaling() {
    for scale in [0.001, 1., 10.] {
        for (dx, dy) in [(0., 0.), (-5. * scale, 7. * scale)] {
            let result = prism(scale, dx, dy).closed_diagnostic_integrals().unwrap();
            let volume = 24. * scale.powi(3);
            check(result.volume_m3, volume, volume * 1e-14);
            for (interval, expected) in result.first_moments_m4.into_iter().zip([
                volume * (dx + scale),
                volume * (dy + 1.5 * scale),
                volume * 2. * scale,
            ]) {
                check(interval, expected, volume * scale * 1e-13);
            }
        }
    }
}
#[test]
fn malformed_nonfinite_nonplanar_and_unbounded_covers_refuse() {
    for mutation in 0..9 {
        let mut band = prism(1., 0., 0.);
        match mutation {
            0 => {
                band.cells.pop();
            }
            1 => band.divisions_per_chart_axis = 64,
            2 => band.cells[0].chart = 1,
            3 => band.cells[0].chart_domain[1] = 0.9,
            4 => band.cells[0].patch.control_points_m[1][1][0] = f64::NAN,
            5 => band.cells[0].patch.control_points_m[3][1][0] += 0.001,
            6 => band.cells[0].patch.control_points_m[1][3][2] += 0.001,
            7 => band.divisions_per_chart_axis = 0,
            _ => {
                for c in &mut band.cells {
                    for row in &mut c.patch.control_points_m {
                        row[3][2] = -1.;
                    }
                }
            }
        }
        assert!(
            band.closed_diagnostic_integrals().is_err(),
            "mutation {mutation}"
        );
    }
}
#[test]
fn current_interior_poles_change_integrals_without_trusting_error_metadata() {
    let mut band = prism(1., 0., 0.);
    let before = band.closed_diagnostic_integrals().unwrap();
    band.cells[0].patch.control_points_m[1][1][1] = -0.5;
    let after = band.closed_diagnostic_integrals().unwrap();
    assert!(after.volume_m3[0] > before.volume_m3[1]);
}
// Independent six-point Gauss-Legendre oracle: degree <=11 per coordinate.
const QUAD: [(f64, f64); 6] = [
    (-0.932469514203152, 0.1713244923791704),
    (-0.6612093864662645, 0.3607615730481386),
    (-0.2386191860831969, 0.467913934572691),
    (0.2386191860831969, 0.467913934572691),
    (0.6612093864662645, 0.3607615730481386),
    (0.932469514203152, 0.1713244923791704),
];
fn evaluate(p: &[[[f64; 3]; 4]; 4], u: f64, v: f64) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let basis = |x: f64| {
        [
            (1. - x).powi(3),
            3. * x * (1. - x).powi(2),
            3. * x * x * (1. - x),
            x.powi(3),
        ]
    };
    let derivative = |x: f64| {
        [
            -3. * (1. - x).powi(2),
            3. * (1. - x) * (1. - 3. * x),
            3. * x * (2. - 3. * x),
            3. * x * x,
        ]
    };
    let (a, b, da, db) = (basis(u), basis(v), derivative(u), derivative(v));
    let (mut point, mut du, mut dv) = ([0.; 3], [0.; 3], [0.; 3]);
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..3 {
                point[k] += a[i] * b[j] * p[i][j][k];
                du[k] += da[i] * b[j] * p[i][j][k];
                dv[k] += a[i] * db[j] * p[i][j][k];
            }
        }
    }
    (point, du, dv)
}
fn quadrature(band: &CircularFilletBezierBand) -> [f64; 4] {
    let mut result = [0.; 4];
    let n = band.divisions_per_chart_axis;
    let h = band.cells[n - 1].patch.control_points_m[0][3][2];
    for (index, cell) in band.cells.iter().enumerate() {
        for (u, wu) in QUAD {
            for (v, wv) in QUAD {
                let (p, du, dv) =
                    evaluate(&cell.patch.control_points_m, (u + 1.) / 2., (v + 1.) / 2.);
                let weight = wu * wv / 4.;
                let flux = p[2] * (du[0] * dv[1] - du[1] * dv[0]) * weight;
                for (sum, f) in
                    result
                        .iter_mut()
                        .zip([flux, p[0] * flux, p[1] * flux, p[2] * flux / 2.])
                {
                    *sum += f;
                }
            }
        }
        if index % n == n - 1 {
            for (u, w) in QUAD {
                let (p, du, _) = evaluate(&cell.patch.control_points_m, (u + 1.) / 2., 1.);
                let q = w / 2.;
                result[0] += h * p[0] * du[1] * q;
                result[1] += h * p[0] * p[0] * du[1] * q / 2.;
                result[2] -= h * p[1] * p[1] * du[0] * q / 2.;
                result[3] += h * h * p[0] * du[1] * q / 2.;
            }
        }
    }
    result
}
#[test]
fn real_constant_and_variable_bands_match_independent_polynomial_quadrature() {
    for maximum in [0.0005, 0.0015] {
        let reference = CircularFilletReference::new(
            Length::from_meters(0.01),
            Length::from_meters(0.02),
            Length::from_meters(0.0005),
            Length::from_meters(maximum),
        )
        .unwrap();
        let band = reference
            .bezier_band(Length::from_meters(1e-4), 0.8, 4096)
            .unwrap();
        let bounds: CircularFilletBoundaryIntegrals = band.closed_diagnostic_integrals().unwrap();
        let actual = quadrature(&band);
        check(bounds.volume_m3, actual[0], 1e-18);
        for (interval, expected) in bounds
            .first_moments_m4
            .into_iter()
            .zip(actual[1..].iter().copied())
        {
            check(interval, expected, 1e-20);
        }
    }
}

#[test]
fn signed_flux_never_repairs_a_reversed_boundary() {
    let mut band = prism(1., 0., 0.);
    for cell in &mut band.cells {
        for row in &mut cell.patch.control_points_m {
            for p in row {
                p[0] = -p[0];
            }
        }
    }
    let bounds = band.closed_diagnostic_integrals().unwrap();
    check(bounds.volume_m3, -24., 1e-12);
    check(bounds.first_moments_m4[0], 24., 1e-12);
    check(bounds.first_moments_m4[1], -36., 1e-12);
    check(bounds.first_moments_m4[2], -48., 1e-12);
}
