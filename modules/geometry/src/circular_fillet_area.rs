//! ADR-081: unsigned parametric band/skirt area, excluding planar caps.
use crate::{bernstein::P, interval::I, CircularFilletBezierBand};
use opencad_core::{OpenCadError, Result};
use std::{cmp::Ordering, collections::BinaryHeap};

/// Area of the current mathematical polynomial band and ruled skirts only.
/// Counts area with multiplicity; no embedding or physical body admission.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletAreaBounds {
    pub curved_lower_m2: f64,
    pub curved_upper_m2: f64,
    pub leaves: usize,
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("curved diagnostic area width or work budget not certified")
}
fn norm(v: [I; 3]) -> I {
    {
        let mut squared = v.into_iter().fold(I::exact(0.), |s, x| s + x.square());
        squared.lo = squared.lo.max(0.);
        squared.sqrt()
    }
}
#[derive(Clone)]
struct Leaf {
    cross: [P; 3],
    weight: f64,
    bounds: I,
    serial: usize,
}
impl PartialEq for Leaf {
    fn eq(&self, b: &Self) -> bool {
        self.cmp(b) == Ordering::Equal
    }
}
impl Eq for Leaf {}
impl PartialOrd for Leaf {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}
impl Ord for Leaf {
    fn cmp(&self, b: &Self) -> Ordering {
        self.bounds
            .width()
            .total_cmp(&b.bounds.width())
            .then_with(|| b.serial.cmp(&self.serial))
    }
}
impl Leaf {
    fn new(cross: [P; 3], weight: f64, serial: usize) -> Result<Self> {
        let lower = norm(std::array::from_fn(|i| cross[i].integral()))
            .lo
            .max(0.);
        let mut upper = I::exact(0.);
        for j in 0..cross[0].c.len() {
            upper = upper + norm(std::array::from_fn(|i| cross[i].c[j]));
        }
        let bounds = I {
            lo: lower,
            hi: (upper / I::exact(cross[0].c.len() as f64)).hi,
        } * I::exact(weight);
        if !bounds.valid() || weight <= 0. {
            return Err(invalid());
        }
        Ok(Self {
            cross,
            weight,
            bounds,
            serial,
        })
    }
    fn from_poles(poles: [[[f64; 3]; 4]; 4], serial: usize) -> Result<Self> {
        let component = |axis: usize| P {
            u: 3,
            v: 3,
            c: poles
                .iter()
                .flat_map(|row| row.iter().map(move |p| I::exact(p[axis])))
                .collect(),
        };
        let du: [P; 3] = std::array::from_fn(|i| component(i).derivative(0));
        let dv: [P; 3] = std::array::from_fn(|i| component(i).derivative(1));
        let cross = std::array::from_fn(|i| {
            du[(i + 1) % 3]
                .product(&dv[(i + 2) % 3])
                .difference(&du[(i + 2) % 3].product(&dv[(i + 1) % 3]))
        });
        Self::new(cross, 1., serial)
    }
    fn children(self, serial: &mut usize) -> Result<Vec<Self>> {
        let halves: [_; 3] = std::array::from_fn(|i| self.cross[i].split_half(0));
        let mut children = Vec::with_capacity(4);
        for side in 0..2 {
            let quarters: [_; 3] = std::array::from_fn(|i| {
                if side == 0 {
                    halves[i].0.split_half(1)
                } else {
                    halves[i].1.split_half(1)
                }
            });
            for upper in 0..2 {
                let cross = std::array::from_fn(|i| {
                    if upper == 0 {
                        quarters[i].0.clone()
                    } else {
                        quarters[i].1.clone()
                    }
                });
                children.push(Self::new(cross, self.weight * 0.25, *serial)?);
                *serial += 1;
            }
        }
        Ok(children)
    }
}
impl CircularFilletBezierBand {
    /// ADR-081: bound unsigned parametric band + ruled skirt area (m²), excluding
    /// planar caps. Refine cross-product polynomial restrictions until the
    /// explicit absolute width is certified; otherwise refuse at the leaf budget.
    /// No sampling, embedding, mesh, native error bound or body is implied.
    pub fn curved_diagnostic_area_bounds(
        &self,
        absolute_width_m2: f64,
        max_leaves: usize,
    ) -> Result<CircularFilletAreaBounds> {
        if !absolute_width_m2.is_finite()
            || absolute_width_m2 <= 0.
            || !(8..=262144).contains(&max_leaves)
        {
            return Err(invalid());
        }
        self.diagnostic_top_height()?;
        let n = self.divisions_per_chart_axis;
        if self.cells.len() + 4 * n > max_leaves {
            return Err(invalid());
        }
        let mut heap = BinaryHeap::new();
        let mut serial = 0;
        let mut sum = I::exact(0.);
        for (index, cell) in self.cells.iter().enumerate() {
            let poles = cell.patch.control_points_m;
            let band = Leaf::from_poles(poles, serial)?;
            serial += 1;
            sum = sum + band.bounds;
            heap.push(band);
            if index % n == 0 {
                let skirt = std::array::from_fn(|i| {
                    std::array::from_fn(|j| {
                        [
                            poles[i][0][0],
                            poles[i][0][1],
                            if j == 3 {
                                poles[i][0][2]
                            } else {
                                poles[i][0][2] * j as f64 / 3.
                            },
                        ]
                    })
                });
                let side = Leaf::from_poles(skirt, serial)?;
                serial += 1;
                sum = sum + side.bounds;
                heap.push(side);
            }
        }
        loop {
            if !sum.valid() {
                return Err(invalid());
            }
            if sum.width() <= absolute_width_m2 {
                return Ok(CircularFilletAreaBounds {
                    curved_lower_m2: sum.lo.max(0.),
                    curved_upper_m2: sum.hi,
                    leaves: heap.len(),
                });
            }
            if heap.len() + 3 > max_leaves {
                return Err(invalid());
            }
            let leaf = heap.pop().ok_or_else(invalid)?;
            let parent = leaf.bounds;
            let mut children_sum = I::exact(0.);
            for child in leaf.children(&mut serial)? {
                children_sum = children_sum + child.bounds;
                heap.push(child);
            }
            // Replace endpoint sums, not whole intervals (which would retain
            // the removed parent's uncertainty). Each endpoint stays outward.
            sum = I {
                lo: (I::exact(sum.lo) - I::exact(parent.lo) + I::exact(children_sum.lo)).lo,
                hi: (I::exact(sum.hi) - I::exact(parent.hi) + I::exact(children_sum.hi)).hi,
            };
        }
    }
}
