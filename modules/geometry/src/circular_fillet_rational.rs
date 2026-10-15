//! ADR-084: fixed rational characteristic-circle charts, not editable NURBS.
use crate::{
    bernstein::P, circular_fillet_patch::norm_upper, interval::I, CircularFilletReference,
};
use opencad_core::{OpenCadError, Result};

/// Immutable rounded representation and its corresponding-parameter bound.
/// The transverse parameter is stereographic, not normalized polar angle.
#[derive(Debug, Clone)]
pub struct CircularFilletRationalPatch {
    chart: u8,
    poles_m: [[[f64; 3]; 3]; 9],
    weights: [[f64; 3]; 9],
    error_m: f64,
}
impl CircularFilletRationalPatch {
    pub fn chart(&self) -> u8 {
        self.chart
    }
    pub fn u_degree(&self) -> usize {
        8
    }
    pub fn v_degree(&self) -> usize {
        2
    }
    pub fn control_points_m(&self) -> &[[[f64; 3]; 3]; 9] {
        &self.poles_m
    }
    pub fn weights(&self) -> &[[f64; 3]; 9] {
        &self.weights
    }
    /// Whole-patch position bound in metres. Derivatives/native sewing/G1 are
    /// not certified by this value; no source or physical body is admitted.
    pub fn position_error_upper_m(&self) -> f64 {
        self.error_m
    }
}

/// Fixed-span analytic Hermite approximation to the cylindrical side contact.
/// Controls are [angle radians, height metres]; the bound includes their rounding.
#[derive(Debug, Clone)]
pub struct CircularFilletCylindricalPcurve {
    chart: u8,
    poles_rad_m: Vec<[f64; 2]>,
    angle_error_rad: f64,
    height_error_m: f64,
    lifted_error_m: f64,
}
impl CircularFilletCylindricalPcurve {
    pub fn chart(&self) -> u8 {
        self.chart
    }
    pub fn divisions(&self) -> usize {
        256
    }
    pub fn control_points_rad_m(&self) -> &[[f64; 2]] {
        &self.poles_rad_m
    }
    pub fn angle_error_upper_rad(&self) -> f64 {
        self.angle_error_rad
    }
    pub fn height_error_upper_m(&self) -> f64 {
        self.height_error_m
    }
    /// Whole supplied-pcurve lift/ideal contact position error. Native curve
    /// conversion and SameParameter/sewing effects are not certified here.
    pub fn position_error_upper_m(&self) -> f64 {
        self.lifted_error_m
    }
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("rational rolling-sphere chart or numerical bound not certified")
}
fn scale(p: &P, k: I) -> P {
    P {
        u: p.u,
        v: p.v,
        c: p.c.iter().map(|x| *x * k).collect(),
    }
}
fn sum(a: &P, b: &P) -> P {
    a.difference(&scale(b, I::exact(-1.)))
}
fn midpoint(x: I) -> Result<f64> {
    if !x.valid() {
        return Err(invalid());
    }
    Ok(x.lo + (x.hi - x.lo) / 2.)
}
impl CircularFilletReference {
    /// ADR-084's fixed 256-span cubic pcurve, with a fourth-derivative Hermite
    /// remainder plus actual-control rounding. No sampled error estimate is used.
    pub fn cylindrical_contact_pcurve(self, chart: u8) -> Result<CircularFilletCylindricalPcurve> {
        if chart >= 4 {
            return Err(invalid());
        }
        let [r, height, min, max] = [
            self.cylinder_radius(),
            self.cylinder_height(),
            self.min_radius(),
            self.max_radius(),
        ]
        .map(|x| I::exact(x.meters()));
        let m = (min + max) / I::exact(2.);
        let a = (max - min) / I::exact(2.);
        let endpoint = |index: usize| -> ([I; 2], [I; 2]) {
            let t = I::exact(index as f64 / 256.);
            let d = I::exact(1.) + t.square();
            let c = I::exact(1.) - t.square();
            let s = I::exact(2.) * t;
            let (c, s) = match chart {
                0 => (c, s),
                1 => (-s, c),
                2 => (-c, -s),
                _ => (s, -c),
            };
            let angle = if index == 0 || index == 256 {
                I::pi() * I::exact(f64::from(chart) + if index == 256 { 1. } else { 0. })
                    / I::exact(2.)
            } else {
                let mut reduced = t;
                for _ in 0..3 {
                    reduced = reduced / (I::exact(1.) + (I::exact(1.) + reduced.square()).sqrt());
                }
                I::pi() * I::exact(f64::from(chart)) / I::exact(2.) + I::exact(16.) * reduced.atan()
            };
            (
                [angle, height - m - a * c / d],
                [I::exact(2.) / d, I::exact(2.) * a * s / d.square()],
            )
        };
        let h = I::exact(1. / 256.);
        let third = h / I::exact(3.);
        let mut poles_rad_m = vec![[0.; 2]; 769];
        let mut rounding = [0_f64; 2];
        for i in 0..256 {
            let (p, dp) = endpoint(i);
            let (q, dq) = endpoint(i + 1);
            let controls = [
                p,
                std::array::from_fn(|k| p[k] + third * dp[k]),
                std::array::from_fn(|k| q[k] - third * dq[k]),
                q,
            ];
            for (j, control) in controls.iter().enumerate() {
                for (k, value) in control.iter().enumerate() {
                    let actual = midpoint(*value)?;
                    let delta = I::exact(actual) - *value;
                    if !delta.valid() || !actual.is_finite() {
                        return Err(invalid());
                    }
                    rounding[k] = rounding[k].max(delta.lo.abs().max(delta.hi.abs()));
                    poles_rad_m[3 * i + j][k] = actual;
                }
            }
        }
        let factor = h.square().square() / I::exact(384.);
        let angle = I::exact(48.) * factor + I::exact(rounding[0]);
        let z = I::exact(768.) * I::exact(a.hi.max(0.)) * factor + I::exact(rounding[1]);
        if !angle.valid() || !z.valid() {
            return Err(invalid());
        }
        let lifted_error_m = norm_upper([r * angle, z, I::exact(0.)]);
        if !lifted_error_m.is_finite() {
            return Err(invalid());
        }
        Ok(CircularFilletCylindricalPcurve {
            chart,
            poles_rad_m,
            angle_error_rad: angle.hi,
            height_error_m: z.hi,
            lifted_error_m,
        })
    }

    /// Construct one immutable degree-(8,2) rational reference quarter. The
    /// ideal chart is exactly ADR-072's characteristic arc reparameterized;
    /// outward arithmetic bounds positional rounding of all actual controls.
    /// This does not admit physical Mock/source geometry or editable NURBS.
    pub fn rational_patch(self, chart: u8) -> Result<CircularFilletRationalPatch> {
        if chart >= 4 {
            return Err(invalid());
        }
        let [r, h, min, max] = [
            self.cylinder_radius(),
            self.cylinder_height(),
            self.min_radius(),
            self.max_radius(),
        ]
        .map(|x| I::exact(x.meters()));
        let m = (min + max) / I::exact(2.);
        let a = (max - min) / I::exact(2.);
        let base = |values: [f64; 3], transverse| P {
            u: if transverse { 0 } else { 2 },
            v: if transverse { 2 } else { 0 },
            c: values.map(I::exact).to_vec(),
        };
        let d = base([1., 1., 2.], false);
        let c = base([1., 1., 0.], false);
        let s = base([0., 1., 2.], false);
        let (c, s) = match chart {
            0 => (c, s),
            1 => (scale(&s, I::exact(-1.)), c),
            2 => (scale(&c, I::exact(-1.)), scale(&s, I::exact(-1.))),
            _ => (s, scale(&c, I::exact(-1.))),
        };
        let n = sum(&scale(&d, m), &scale(&c, a));
        let l = scale(&d, r).difference(&n);
        let k = scale(&s, -a);
        let l2 = l.product(&l);
        let k2 = k.product(&k);
        let two_v2 = base([0., 0., 2.], true);
        let wc = sum(&l2.product(&base([1., 1., 2.], true)), &k2.product(&two_v2));
        let nr = l2.product(&base([1., 1., 0.], true));
        let nt = k.product(&l).product(&base([0., 1., 0.], true));
        let nz = sum(&l2.product(&base([0., 1., 2.], true)), &k2.product(&two_v2));
        let w = d.product(&d).product(&wc);
        let numerators = [
            sum(
                &l.product(&c).product(&wc),
                &n.product(&nr.product(&c).difference(&nt.product(&s))),
            ),
            sum(
                &l.product(&s).product(&wc),
                &n.product(&sum(&nr.product(&s), &nt.product(&c))),
            ),
            sum(
                &scale(&d, h).difference(&n).product(&d).product(&wc),
                &n.product(&d).product(&nz),
            ),
        ];
        if w.c
            .iter()
            .chain(numerators.iter().flat_map(|p| &p.c))
            .any(|x| !x.valid())
        {
            return Err(invalid());
        }
        let mut poles_m = [[[0.; 3]; 3]; 9];
        let mut weights = [[0.; 3]; 9];
        for (i, row) in weights.iter_mut().enumerate() {
            for (j, weight) in row.iter_mut().enumerate() {
                *weight = midpoint(w.at(i, j))?;
                if *weight <= 0. || w.at(i, j).lo <= 0. {
                    return Err(invalid());
                }
                for (axis, p) in numerators.iter().enumerate() {
                    poles_m[i][j][axis] = midpoint(p.at(i, j) / I::exact(*weight))?;
                }
            }
        }
        let wa = weights
            .iter()
            .flatten()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let wi = w.c.iter().map(|x| x.lo).fold(f64::INFINITY, f64::min);
        let magnitude = |x: I| x.lo.abs().max(x.hi.abs());
        let dw = weights
            .iter()
            .flatten()
            .zip(&w.c)
            .map(|(actual, ideal)| magnitude(I::exact(*actual) - *ideal))
            .fold(0., f64::max);
        let errors = std::array::from_fn(|axis| {
            let mut dn = 0_f64;
            let mut nn = 0_f64;
            for i in 0..9 {
                for j in 0..3 {
                    dn = dn.max(magnitude(
                        I::exact(poles_m[i][j][axis]) * I::exact(weights[i][j])
                            - numerators[axis].at(i, j),
                    ));
                    nn = nn.max(magnitude(numerators[axis].at(i, j)));
                }
            }
            I::exact(dn) / I::exact(wa)
                + I::exact(nn) * I::exact(dw) / (I::exact(wa) * I::exact(wi))
        });
        if errors.iter().any(|x| !x.valid()) {
            return Err(invalid());
        }
        let error_m = norm_upper(errors);
        if !error_m.is_finite()
            || poles_m.iter().flatten().flatten().any(|x| !x.is_finite())
            || weights.iter().flatten().any(|x| !x.is_finite())
        {
            return Err(invalid());
        }
        Ok(CircularFilletRationalPatch {
            chart,
            poles_m,
            weights,
            error_m,
        })
    }
}
