//! Private outward Bernstein arithmetic shared by ADR-080 and ADR-081.
use crate::interval::I;

#[derive(Clone)]
pub(crate) struct P {
    pub(crate) u: usize,
    pub(crate) v: usize,
    pub(crate) c: Vec<I>,
}
fn binomial(n: usize, k: usize) -> f64 {
    // Degrees are <=11, hence these integer coefficients are exactly binary64.
    let mut out = 1_usize;
    for j in 0..k.min(n - k) {
        out = out * (n - j) / (j + 1);
    }
    out as f64
}
impl P {
    pub(crate) fn split_half(&self, axis: usize) -> (P, P) {
        let p = self;
        let mut a = P::zero(p.u, p.v);
        let mut b = a.clone();
        let (degree, rows) = if axis == 0 { (p.u, p.v) } else { (p.v, p.u) };
        for row in 0..=rows {
            let mut values: Vec<_> = (0..=degree)
                .map(|i| {
                    if axis == 0 {
                        p.at(i, row)
                    } else {
                        p.at(row, i)
                    }
                })
                .collect();
            for level in 0..=degree {
                let ia = if axis == 0 {
                    level * (p.v + 1) + row
                } else {
                    row * (p.v + 1) + level
                };
                let ib = if axis == 0 {
                    (degree - level) * (p.v + 1) + row
                } else {
                    row * (p.v + 1) + degree - level
                };
                a.c[ia] = values[0];
                b.c[ib] = values[degree - level];
                for i in 0..degree - level {
                    values[i] = (values[i] + values[i + 1]) * I::exact(0.5);
                }
            }
        }
        (a, b)
    }

    pub(crate) fn zero(u: usize, v: usize) -> Self {
        Self {
            u,
            v,
            c: vec![I::exact(0.); (u + 1) * (v + 1)],
        }
    }
    pub(crate) fn at(&self, i: usize, j: usize) -> I {
        self.c[i * (self.v + 1) + j]
    }
    pub(crate) fn derivative(&self, axis: usize) -> Self {
        let mut out = Self::zero(
            self.u - usize::from(axis == 0),
            self.v - usize::from(axis == 1),
        );
        let degree = I::exact(if axis == 0 { self.u } else { self.v } as f64);
        for i in 0..=out.u {
            for j in 0..=out.v {
                out.c[i * (out.v + 1) + j] = (self
                    .at(i + usize::from(axis == 0), j + usize::from(axis == 1))
                    - self.at(i, j))
                    * degree;
            }
        }
        out
    }
    pub(crate) fn product(&self, other: &Self) -> Self {
        let mut out = Self::zero(self.u + other.u, self.v + other.v);
        let weights = |a: usize, b: usize| -> Vec<Vec<I>> {
            (0..=a)
                .map(|i| {
                    (0..=b)
                        .map(|j| {
                            I::exact(binomial(a, i)) * I::exact(binomial(b, j))
                                / I::exact(binomial(a + b, i + j))
                        })
                        .collect()
                })
                .collect()
        };
        let wu = weights(self.u, other.u);
        let wv = weights(self.v, other.v);
        for (i, row_u) in wu.iter().enumerate() {
            for (j, row_v) in wv.iter().enumerate() {
                for (k, weight_u) in row_u.iter().enumerate() {
                    for (l, weight_v) in row_v.iter().enumerate() {
                        let index = (i + k) * (out.v + 1) + j + l;
                        out.c[index] =
                            out.c[index] + self.at(i, j) * other.at(k, l) * *weight_u * *weight_v;
                    }
                }
            }
        }
        out
    }
    pub(crate) fn difference(&self, other: &Self) -> Self {
        let mut out = self.clone();
        for (a, b) in out.c.iter_mut().zip(&other.c) {
            *a = *a - *b;
        }
        out
    }
    pub(crate) fn integral(&self) -> I {
        self.c.iter().fold(I::exact(0.), |s, x| s + *x)
            / I::exact(((self.u + 1) * (self.v + 1)) as f64)
    }
}
