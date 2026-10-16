//! ADR-096: ideal unsigned area enclosures, never a backend mass property.
use crate::{
    interval::{AdjacentFloat, I},
    CircularFilletReference,
};
use opencad_core::{OpenCadError, Result};
use std::{cmp::Ordering, collections::BinaryHeap};

fn invalid() -> OpenCadError {
    OpenCadError::validation("ideal reference area domain, precision or cell budget exceeded")
}

/// Absolute square-metre enclosures of the ideal boundary, with no interfaces.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletReferenceAreaBounds {
    pub body_lower_m2: f64,
    pub body_upper_m2: f64,
    pub blend_lower_m2: f64,
    pub blend_upper_m2: f64,
    pub retained_side_m2: [f64; 2],
    pub retained_end_m2: [f64; 2],
    pub bottom_m2: [f64; 2],
    /// Leaves partitioning the four angular charts; zero for authored constant.
    pub cells: usize,
}

fn dot(a: [I; 3], b: [I; 3]) -> I {
    a.into_iter()
        .zip(b)
        .fold(I::exact(0.), |s, (x, y)| s + x * y)
}

// ADR-073's positive r*D/v Jacobian integrated exactly over the characteristic
// circle. Only degree-one trig moments enter area; units are normalized R².
fn density(minimum: I, maximum: I, cosine: I, sine: I) -> I {
    let one = I::exact(1.);
    let two = I::exact(2.);
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
    argument.lo = argument.lo.max(0.); // Proven nonnegative exact argument.
    let omega = I::pi() / two + argument.atan();
    let a = [one - o[0], -o[1], -o[2]];
    let ez = [I::exact(0.), I::exact(0.), one];
    let bb = std::array::from_fn(|i| (ez[i] - o[i] - c * a[i]) / sn);
    let centre_second = [-b - dd, -two * d, -dd];
    let constant = b.square() + d2 - r * dd - r * dot(o, centre_second);
    let cosine_term = -r * dot(a, centre_second);
    let sine_term = -r * dot(bb, centre_second);
    r / v * (constant * omega + cosine_term * sn + sine_term * (one - c))
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    chart: u8,
    lo: f64,
    hi: f64,
    integral: I,
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
        let mut integral =
            density(minimum, maximum, cosine, sine) * two / den * (I::exact(hi) - I::exact(lo));
        if !integral.valid() {
            return Err(invalid());
        }
        // ADR-072/073 proves positivity over the entire admitted ideal surface.
        integral.lo = integral.lo.max(0.);
        if !integral.valid() {
            return Err(invalid());
        }
        Ok(Self {
            chart,
            lo,
            hi,
            integral,
        })
    }
}
impl Ord for Cell {
    fn cmp(&self, other: &Self) -> Ordering {
        self.integral
            .width()
            .total_cmp(&other.integral.width())
            .then_with(|| self.chart.cmp(&other.chart))
            .then_with(|| self.lo.total_cmp(&other.lo))
            .then_with(|| self.hi.total_cmp(&other.hi))
    }
}
impl PartialOrd for Cell {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Cell {}

impl CircularFilletReference {
    /// Bound ideal total/blend/support unsigned areas in m². The finite positive
    /// width applies to TOTAL area; work is 4..=262144 leaves. Exhaustion refuses.
    pub fn surface_area_bounds_m2(
        self,
        max_width_m2: f64,
        max_cells: usize,
    ) -> Result<CircularFilletReferenceAreaBounds> {
        if !max_width_m2.is_finite() || max_width_m2 <= 0. || !(4..=262_144).contains(&max_cells) {
            return Err(invalid());
        }
        let r = I::exact(self.cylinder_radius().meters());
        let h = I::exact(self.cylinder_height().meters());
        let minimum = I::exact(self.min_radius().meters());
        let maximum = I::exact(self.max_radius().meters());
        let two = I::exact(2.);
        let mean = (minimum + maximum) / two;
        let amplitude = (maximum - minimum) / two;
        let side = two * I::pi() * r * (h - mean);
        let end = I::pi() * ((r - mean).square() + amplitude.square() / two);
        let bottom = I::pi() * r.square();
        let finish = |mut blend: I, cells| -> Result<CircularFilletReferenceAreaBounds> {
            blend.lo = blend.lo.max(0.);
            let body = blend + side + end + bottom;
            if [blend, side, end, bottom, body]
                .into_iter()
                .any(|i| !i.valid() || i.lo <= 0.)
                || body.width() > max_width_m2
            {
                return Err(invalid());
            }
            Ok(CircularFilletReferenceAreaBounds {
                body_lower_m2: body.lo,
                body_upper_m2: body.hi,
                blend_lower_m2: blend.lo,
                blend_upper_m2: blend.hi,
                retained_side_m2: [side.lo, side.hi],
                retained_end_m2: [end.lo, end.hi],
                bottom_m2: [bottom.lo, bottom.hi],
                cells,
            })
        };
        // Authored parameter identity, not a geometric epsilon comparison.
        if minimum.lo.to_bits() == maximum.lo.to_bits() {
            let blend = two * I::pi() * minimum * ((r - minimum) * I::pi() / two + minimum);
            return finish(blend, 0);
        }
        let scale = r.square();
        let mut heap = BinaryHeap::new();
        let mut total = I::exact(0.);
        for chart in 0..4 {
            let cell = Cell::new(chart, 0., 1., minimum / r, maximum / r)?;
            total = total + cell.integral;
            heap.push(cell);
        }
        loop {
            if let Ok(bounds) = finish(total * scale, heap.len()) {
                return Ok(bounds);
            }
            if heap.len() >= max_cells {
                return Err(invalid());
            }
            let parent = heap.pop().ok_or_else(invalid)?;
            let mid = parent.lo + (parent.hi - parent.lo) / 2.;
            if mid <= parent.lo || mid >= parent.hi {
                return Err(invalid());
            }
            let left = Cell::new(parent.chart, parent.lo, mid, minimum / r, maximum / r)?;
            let right = Cell::new(parent.chart, mid, parent.hi, minimum / r, maximum / r)?;
            total.lo = ((total.lo - parent.integral.lo).adjacent_down() + left.integral.lo)
                .adjacent_down();
            total.lo = (total.lo + right.integral.lo).adjacent_down();
            total.hi =
                ((total.hi - parent.integral.hi).adjacent_up() + left.integral.hi).adjacent_up();
            total.hi = (total.hi + right.integral.hi).adjacent_up();
            if !total.valid() {
                return Err(invalid());
            }
            heap.push(left);
            heap.push(right);
        }
    }
}
