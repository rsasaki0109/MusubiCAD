//! ADR-082: immutable support contact curves, not editable NURBS or a body.
use crate::{
    bernstein::P, circular_fillet_patch::norm_upper, interval::I, CircularFilletReference,
};
use opencad_core::{OpenCadError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircularFilletContactKind {
    Side,
    End,
}
/// Private data keep the positional certificate tied to the actual stored poles.
/// No mutation, serialization/source tag or geometric body admission is exposed.
#[derive(Debug, Clone)]
pub struct CircularFilletContactCurve {
    kind: CircularFilletContactKind,
    chart: u8,
    poles_m: Vec<[f64; 3]>,
    weights: Vec<f64>,
    error_m: f64,
}
impl CircularFilletContactCurve {
    pub fn kind(&self) -> CircularFilletContactKind {
        self.kind
    }
    pub fn chart(&self) -> u8 {
        self.chart
    }
    pub fn degree(&self) -> usize {
        self.weights.len() - 1
    }
    pub fn control_points_m(&self) -> &[[f64; 3]] {
        &self.poles_m
    }
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }
    /// Whole-curve corresponding-chart position error, metres. Native conversion
    /// and evaluation, derivatives, G1 and support trimming are not certified.
    pub fn position_error_upper_m(&self) -> f64 {
        self.error_m
    }
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("rational contact chart or numerical bound not certified")
}
fn midpoint(x: I) -> Result<f64> {
    if !x.valid() {
        return Err(invalid());
    }
    Ok((x.lo + (x.hi - x.lo) / 2.).max(x.lo).min(x.hi))
}
fn scale(p: &P, k: I) -> P {
    P {
        u: p.u,
        v: 0,
        c: p.c.iter().map(|x| *x * k).collect(),
    }
}
fn rotate(x: f64, y: f64, chart: u8) -> [f64; 2] {
    match chart {
        0 => [x, y],
        1 => [-y, x],
        2 => [-x, -y],
        _ => [y, -x],
    }
    .map(|v| if v == 0. { 0. } else { v })
}
impl CircularFilletReference {
    /// ADR-082: four fixed quarter charts of side/end support contacts. Positive
    /// weights and immutable poles carry a whole-curve position bound. Encoded
    /// side x/y satisfy their circle identity; encoded end z is constant. No
    /// native trimming, fillet surface, G1, editable NURBS or body is admitted.
    pub fn rational_contact_curve(
        self,
        kind: CircularFilletContactKind,
        chart: u8,
    ) -> Result<CircularFilletContactCurve> {
        if chart >= 4 {
            return Err(invalid());
        }
        let [r, h, min, max] = [
            self.cylinder_radius(),
            self.cylinder_height(),
            self.min_radius(),
            self.max_radius(),
        ]
        .map(|x| x.meters());
        let [ri, hi, mi, ma] = [r, h, min, max].map(I::exact);
        let mean = (mi + ma) / I::exact(2.);
        let amplitude = (ma - mi) / I::exact(2.);
        let base = |values: [f64; 3]| P {
            u: 2,
            v: 0,
            c: values.map(I::exact).to_vec(),
        };
        let d = base([1., 1., 2.]);
        let c = base([1., 1., 0.]);
        let s = base([0., 1., 2.]);
        let (c, s) = match chart {
            0 => (c, s),
            1 => (scale(&s, I::exact(-1.)), c),
            2 => (scale(&c, I::exact(-1.)), scale(&s, I::exact(-1.))),
            _ => (s, scale(&c, I::exact(-1.))),
        };
        let (denominator, numerators, weights) = match kind {
            CircularFilletContactKind::Side => (
                d.clone(),
                [
                    scale(&c, ri),
                    scale(&s, ri),
                    scale(&d, hi - mean).difference(&scale(&c, amplitude)),
                ],
                vec![1., 1., 2.],
            ),
            CircularFilletContactKind::End => {
                let w = d.product(&d);
                let x =
                    scale(&c.product(&d), ri - mean).difference(&scale(&c.product(&c), amplitude));
                let y =
                    scale(&s.product(&d), ri - mean).difference(&scale(&c.product(&s), amplitude));
                let z = scale(&w, hi);
                (w, [x, y, z], vec![1., 1., 4. / 3., 2., 4.])
            }
        };
        if denominator
            .c
            .iter()
            .chain(numerators.iter().flat_map(|p| &p.c))
            .any(|x| !x.valid())
        {
            return Err(invalid());
        }
        let degree = weights.len() - 1;
        let mut poles = Vec::with_capacity(degree + 1);
        for (i, w) in weights.iter().enumerate() {
            let mut p = [0.; 3];
            for k in 0..3 {
                p[k] = midpoint(numerators[k].c[i] / I::exact(*w))?;
            }
            poles.push(p);
        }
        if kind == CircularFilletContactKind::Side {
            for (p, [x, y]) in poles.iter_mut().zip([[r, 0.], [r, r], [0., r]]) {
                let q = rotate(x, y, chart);
                p[0] = q[0];
                p[1] = q[1];
            }
        } else {
            for p in &mut poles {
                p[2] = h;
            }
        }
        let middle = min + (max - min) / 2.;
        let radii = [max, middle, min, middle];
        for (index, quadrant) in [(0, chart), (degree, (chart + 1) % 4)] {
            let radius = radii[usize::from(quadrant)];
            let xy = rotate(
                if kind == CircularFilletContactKind::Side {
                    r
                } else {
                    r - radius
                },
                0.,
                quadrant,
            );
            poles[index] = [
                xy[0],
                xy[1],
                if kind == CircularFilletContactKind::Side {
                    h - radius
                } else {
                    h
                },
            ];
        }
        let wa = weights.iter().copied().fold(f64::INFINITY, f64::min);
        let wi = denominator
            .c
            .iter()
            .map(|w| w.lo)
            .fold(f64::INFINITY, f64::min);
        if wa <= 0. || wi <= 0. {
            return Err(invalid());
        }
        let magnitude = |x: I| x.lo.abs().max(x.hi.abs());
        let dw = weights
            .iter()
            .zip(&denominator.c)
            .map(|(w, b)| magnitude(I::exact(*w) - *b))
            .fold(0., f64::max);
        let errors = std::array::from_fn(|axis| {
            let mut dn = 0_f64;
            let mut nn = 0_f64;
            for (i, w) in weights.iter().enumerate() {
                dn = dn.max(magnitude(
                    I::exact(poles[i][axis]) * I::exact(*w) - numerators[axis].c[i],
                ));
                nn = nn.max(magnitude(numerators[axis].c[i]));
            }
            I::exact(dn) / I::exact(wa)
                + I::exact(nn) * I::exact(dw) / (I::exact(wa) * I::exact(wi))
        });
        if errors.iter().any(|x| !x.valid()) {
            return Err(invalid());
        }
        let error = norm_upper(errors);
        if !error.is_finite() || poles.iter().flatten().any(|x| !x.is_finite()) {
            return Err(invalid());
        }
        Ok(CircularFilletContactCurve {
            kind,
            chart,
            poles_m: poles,
            weights,
            error_m: error,
        })
    }
}
