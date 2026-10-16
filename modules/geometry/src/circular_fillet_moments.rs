//! ADR-098: ideal first-moment/centroid enclosures, never kernel mass.
use crate::{
    interval::{AdjacentFloat, I},
    CircularFilletReference,
};
use opencad_core::{Length, OpenCadError, Result};
use std::{cmp::Ordering, collections::BinaryHeap};
fn invalid() -> OpenCadError {
    OpenCadError::validation("ideal reference moment domain, precision or cell budget exceeded")
}
/// All widths are absolute FULL enclosure widths; max_cells applies per pass.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletMomentPrecision {
    pub maximum_volume_interval_width_m3: f64,
    pub maximum_first_moment_interval_width_m4: f64,
    pub maximum_centroid_interval_width: Length,
    pub max_cells: usize,
}
/// Canonical ideal body (+Z axis, radius maximum +X), no native/placed proof.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletMomentBounds {
    pub volume_m3: [f64; 2],
    pub first_moments_m4: [[f64; 2]; 3],
    pub centroid_m: [[f64; 2]; 3],
    pub volume_cells: usize,
    pub moment_cells: usize,
}
fn dot(a: [I; 3], b: [I; 3]) -> I {
    a.into_iter()
        .zip(b)
        .fold(I::exact(0.), |s, (x, y)| s + x * y)
}

// A polynomial in cos(phi),sin(phi), total degree at most four.
type Poly = [[I; 5]; 5];
fn polynomial(coefficients: [I; 3]) -> Poly {
    let mut p = [[I::exact(0.); 5]; 5];
    p[0][0] = coefficients[0];
    p[1][0] = coefficients[1];
    p[0][1] = coefficients[2];
    p
}
fn product(a: Poly, b: Poly) -> Poly {
    let mut out = [[I::exact(0.); 5]; 5];
    for i in 0..5 {
        for j in 0..5 - i {
            for k in 0..5 - i - j {
                for l in 0..5 - i - j - k {
                    out[i + k][j + l] = out[i + k][j + l] + a[i][j] * b[k][l];
                }
            }
        }
    }
    out
}

/// Integrate the characteristic circle analytically; units are normalized R^4.
fn density(minimum: I, maximum: I, cosine: I, sine: I) -> [I; 2] {
    let one = I::exact(1.);
    let two = I::exact(2.);
    let three = I::exact(3.);
    let amplitude = (maximum - minimum) / two;
    let r = minimum + amplitude * (one + cosine);
    let d = -amplitude * sine;
    let dd = -amplitude * cosine;
    let b = one - r;
    let d2 = d.square();
    let v2 = b.square() + two * d2;
    let v = v2.sqrt();
    let f = d2 / v2;
    let o = [f, -d * b / v2, f];
    let c = -d2 / (b.square() + d2);
    let sn = (one - c.square()).sqrt();
    let mut argument = d2 / (b * v);
    // Its exact value is nonnegative; retain that proven domain at zero.
    argument.lo = argument.lo.max(0.);
    let omega = I::pi() / two + argument.atan();
    let a = [one - o[0], -o[1], -o[2]];
    let ez = [I::exact(0.), I::exact(0.), one];
    let bb = std::array::from_fn(|i| (ez[i] - o[i] - c * a[i]) / sn);
    let centre_second = [-b - dd, -two * d, -dd];
    let z = polynomial([o[2], a[2], bb[2]]);
    let complement = polynomial([one - o[2], -a[2], -bb[2]]);
    let determinant = polynomial([
        b.square() + d2 - r * dd - r * dot(o, centre_second),
        -r * dot(a, centre_second),
        -r * dot(bb, centre_second),
    ]);
    let p = product(product(z, complement), determinant);
    let x = polynomial([
        b * cosine + r * (o[0] * cosine - o[1] * sine),
        r * (a[0] * cosine - a[1] * sine),
        r * (bb[0] * cosine - bb[1] * sine),
    ]);
    let px = product(x, p);
    let pt = product(complement, p);
    let sn3 = sn * sn.square();
    let c3 = c * c.square();
    let integrals = [
        ((0, 0), omega),
        ((1, 0), sn),
        ((0, 1), one - c),
        ((2, 0), (omega + sn * c) / two),
        ((0, 2), (omega - sn * c) / two),
        ((1, 1), sn.square() / two),
        ((3, 0), sn - sn3 / three),
        ((2, 1), (one - c3) / three),
        ((1, 2), sn3 / three),
        ((0, 3), two / three - c + c3 / three),
        (
            (4, 0),
            sn * c3 / I::exact(4.) + three * (omega + sn * c) / I::exact(8.),
        ),
        (
            (0, 4),
            -c * sn3 / I::exact(4.) + three * (omega - sn * c) / I::exact(8.),
        ),
        ((3, 1), (one - c.square().square()) / I::exact(4.)),
        ((1, 3), sn.square().square() / I::exact(4.)),
        (
            (2, 2),
            (omega - sn * c * (c.square() - sn.square())) / I::exact(8.),
        ),
    ];
    let integrate = |p: Poly| {
        integrals
            .into_iter()
            .fold(I::exact(0.), |sum, ((i, j), w)| sum + p[i][j] * w)
    };
    [
        r.square() / v * integrate(px),
        r.square() * r / (two * v) * integrate(pt),
    ]
}

#[derive(Debug, Clone, Copy)]
struct Cell {
    chart: u8,
    lo: f64,
    hi: f64,
    integrals: [I; 2],
}
impl Cell {
    fn new(chart: u8, lo: f64, hi: f64, minimum: I, maximum: I) -> Result<Self> {
        let u = I { lo, hi };
        let one = I::exact(1.);
        let two = I::exact(2.);
        let den = one + u.square();
        let c = (one - u.square()) / den;
        let s = two * u / den;
        let (cosine, sine) = match chart {
            0 => (c, s),
            1 => (-s, c),
            2 => (-c, -s),
            _ => (s, -c),
        };
        let mut integrals = density(minimum, maximum, cosine, sine)
            .map(|x| x * two / den * (I::exact(hi) - I::exact(lo)));
        if integrals.iter().any(|i| !i.valid()) {
            return Err(invalid());
        }
        integrals[1].lo = integrals[1].lo.max(0.); // Only T has a proved nonnegative integrand.
        if integrals.iter().any(|i| !i.valid()) {
            return Err(invalid());
        }
        Ok(Self {
            chart,
            lo,
            hi,
            integrals,
        })
    }
    fn width(&self) -> f64 {
        self.integrals[0].width().max(self.integrals[1].width())
    }
}
impl Ord for Cell {
    fn cmp(&self, b: &Self) -> Ordering {
        self.width()
            .total_cmp(&b.width())
            .then_with(|| self.chart.cmp(&b.chart))
            .then_with(|| self.lo.total_cmp(&b.lo))
            .then_with(|| self.hi.total_cmp(&b.hi))
    }
}
impl PartialOrd for Cell {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}
impl PartialEq for Cell {
    fn eq(&self, b: &Self) -> bool {
        self.cmp(b) == Ordering::Equal
    }
}
impl Eq for Cell {}
impl CircularFilletReference {
    /// Bound canonical ideal first moments (m⁴) and centroid (m). Both integration
    /// passes have their own explicit max_cells cap; every requested width must fit.
    pub fn moment_bounds(
        self,
        p: CircularFilletMomentPrecision,
    ) -> Result<CircularFilletMomentBounds> {
        let centre_width = p.maximum_centroid_interval_width.meters();
        if [
            p.maximum_volume_interval_width_m3,
            p.maximum_first_moment_interval_width_m4,
            centre_width,
        ]
        .into_iter()
        .any(|v| !v.is_finite() || v <= 0.)
            || !(4..=262144).contains(&p.max_cells)
        {
            return Err(invalid());
        }
        let vb = self.volume_bounds_m3(p.maximum_volume_interval_width_m3, p.max_cells)?;
        let volume = I {
            lo: vb.body_lower_m3,
            hi: vb.body_upper_m3,
        };
        if !volume.valid() || volume.lo <= 0. {
            return Err(invalid());
        }
        let r = I::exact(self.cylinder_radius().meters());
        let h = I::exact(self.cylinder_height().meters());
        let minimum = I::exact(self.min_radius().meters());
        let maximum = I::exact(self.max_radius().meters());
        let stock = I::pi() * r.square() * h;
        let constant = minimum.lo.to_bits() == maximum.lo.to_bits(); // Authored identity, no geometric epsilon.
        let finish = |mx: I, t: I, moment_cells| -> Result<CircularFilletMomentBounds> {
            let moments = [mx, I::exact(0.), h * (volume - stock / I::exact(2.)) + t];
            if moments
                .iter()
                .any(|i| !i.valid() || i.width() > p.maximum_first_moment_interval_width_m4)
            {
                return Err(invalid());
            }
            let centroid = [
                if constant { I::exact(0.) } else { mx / volume },
                I::exact(0.),
                moments[2] / volume,
            ];
            if centroid
                .iter()
                .any(|i| !i.valid() || i.width() > centre_width)
            {
                return Err(invalid());
            }
            Ok(CircularFilletMomentBounds {
                volume_m3: [volume.lo, volume.hi],
                first_moments_m4: moments.map(|i| [i.lo, i.hi]),
                centroid_m: centroid.map(|i| [i.lo, i.hi]),
                volume_cells: vb.cells,
                moment_cells,
            })
        };
        if constant {
            let t = I::pi()
                * minimum.square()
                * minimum
                * ((r - minimum) * (I::exact(5.) / I::exact(3.) - I::pi() / I::exact(2.))
                    + minimum / I::exact(12.));
            return finish(I::exact(0.), t, 0);
        }
        let scale = r.square().square();
        let mut total = [I::exact(0.); 2];
        let mut heap = BinaryHeap::new();
        for chart in 0..4 {
            let c = Cell::new(chart, 0., 1., minimum / r, maximum / r)?;
            for (sum, x) in total.iter_mut().zip(c.integrals) {
                *sum = *sum + x;
            }
            heap.push(c);
        }
        loop {
            if let Ok(b) = finish(-total[0] * scale, total[1] * scale, heap.len()) {
                return Ok(b);
            }
            if heap.len() >= p.max_cells {
                return Err(invalid());
            }
            let parent = heap.pop().ok_or_else(invalid)?;
            let mid = parent.lo + (parent.hi - parent.lo) / 2.;
            if mid <= parent.lo || mid >= parent.hi {
                return Err(invalid());
            }
            let left = Cell::new(parent.chart, parent.lo, mid, minimum / r, maximum / r)?;
            let right = Cell::new(parent.chart, mid, parent.hi, minimum / r, maximum / r)?;
            for (i, sum) in total.iter_mut().enumerate() {
                sum.lo = ((sum.lo - parent.integrals[i].lo).adjacent_down() + left.integrals[i].lo)
                    .adjacent_down();
                sum.lo = (sum.lo + right.integrals[i].lo).adjacent_down();
                sum.hi = ((sum.hi - parent.integrals[i].hi).adjacent_up() + left.integrals[i].hi)
                    .adjacent_up();
                sum.hi = (sum.hi + right.integrals[i].hi).adjacent_up();
                if !sum.valid() {
                    return Err(invalid());
                }
            }
            heap.push(left);
            heap.push(right);
        }
    }
}
