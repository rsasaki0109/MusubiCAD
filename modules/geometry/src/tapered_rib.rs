//! ADR-066 transient SI input for an actual stock/trapezoidal-prism union.
use crate::{FacePick, RectangularStockGeometry, POSITION_TOLERANCE_M, ROTATION_TOLERANCE};
use opencad_core::{OpenCadError, Result};
fn invalid() -> OpenCadError {
    OpenCadError::validation("invalid bounded tapered rib")
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TaperedRibInput {
    /// Footprint centre on the actual stock plane; normal is outward.
    pub entry: FacePick,
    /// Unit orientation hint parallel to a witnessed stock in-plane axis.
    pub length_axis_hint: [f64; 3],
    pub length_m: f64,
    pub base_thickness_m: f64,
    pub height_m: f64,
    /// Positive narrows outward; the base is neutral and ends remain square.
    pub angle_rad: f64,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TaperedRibGeometry {
    pub base_center_m: [f64; 3],
    pub length_axis: [f64; 3],
    pub width_axis: [f64; 3],
    pub outward_normal: [f64; 3],
    pub length_m: f64,
    pub base_thickness_m: f64,
    pub top_thickness_m: f64,
    pub height_m: f64,
}
impl TaperedRibGeometry {
    pub fn vertices_m(&self) -> [[f64; 3]; 8] {
        std::array::from_fn(|mask| {
            let top = mask & 4 != 0;
            let width = if top {
                self.top_thickness_m
            } else {
                self.base_thickness_m
            };
            let s = if mask & 1 != 0 { 0.5 } else { -0.5 };
            let t = if mask & 2 != 0 { 0.5 } else { -0.5 };
            std::array::from_fn(|i| {
                self.base_center_m[i]
                    + self.length_axis[i] * self.length_m * s
                    + self.width_axis[i] * width * t
                    + if top {
                        self.outward_normal[i] * self.height_m
                    } else {
                        0.
                    }
            })
        })
    }
}
impl TaperedRibInput {
    /// Hints choose actual axes; they never establish stock or source ownership.
    pub fn admit(&self, stock: &RectangularStockGeometry) -> Result<TaperedRibGeometry> {
        if !stock.matches_pick(self.entry)?
            || self.length_axis_hint.iter().any(|v| !v.is_finite())
            || (norm(self.length_axis_hint) - 1.).abs() > ROTATION_TOLERANCE
            || !self.angle_rad.is_finite()
            || self.angle_rad.abs() < 1e-8
            || self.angle_rad.abs() > std::f64::consts::FRAC_PI_4
        {
            return Err(invalid());
        }
        let top = self.base_thickness_m - 2. * self.height_m * self.angle_rad.tan();
        if [
            self.length_m,
            self.base_thickness_m,
            self.height_m,
            top,
            stock.face_spans_m[0],
            stock.face_spans_m[1],
            stock.inward_thickness_m,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v <= POSITION_TOLERANCE_M || *v > 1000.)
            || (top - self.base_thickness_m).abs() <= POSITION_TOLERANCE_M
        {
            return Err(invalid());
        }
        let added = self.length_m * self.height_m * (self.base_thickness_m + top) / 2.;
        let stock_volume = stock.face_spans_m[0] * stock.face_spans_m[1] * stock.inward_thickness_m;
        if !added.is_finite() || added <= 64. * f64::EPSILON * stock_volume {
            return Err(invalid());
        }
        let p = sub(self.entry.point_m, stock.origin_m);
        let scale = self
            .entry
            .point_m
            .into_iter()
            .chain(stock.origin_m)
            .map(f64::abs)
            .fold(1., f64::max);
        if dot(p, stock.outward_normal).abs() > 64. * f64::EPSILON * scale {
            return Err(invalid());
        }
        let axes = [stock.u_axis, stock.v_axis];
        let coords = axes.map(|v| dot(p, v));
        let choices: Vec<_> = axes
            .into_iter()
            .enumerate()
            .filter_map(|(i, axis)| {
                let sign = dot(axis, self.length_axis_hint).signum();
                let selected = axis.map(|v| v * sign);
                (norm(sub(selected, self.length_axis_hint)) <= ROTATION_TOLERANCE)
                    .then_some((i, selected))
            })
            .collect();
        if choices.len() != 1 {
            return Err(invalid());
        }
        let (along, length_axis) = choices[0];
        for (i, c) in coords.into_iter().enumerate() {
            let half = if i == along {
                self.length_m / 2.
            } else {
                self.base_thickness_m / 2.
            };
            if c - half <= POSITION_TOLERANCE_M
                || stock.face_spans_m[i] - c - half <= POSITION_TOLERANCE_M
            {
                return Err(invalid());
            }
        }
        let geometry = TaperedRibGeometry {
            base_center_m: std::array::from_fn(|i| {
                stock.origin_m[i] + stock.u_axis[i] * coords[0] + stock.v_axis[i] * coords[1]
            }),
            length_axis,
            width_axis: cross(stock.outward_normal, length_axis),
            outward_normal: stock.outward_normal,
            length_m: self.length_m,
            base_thickness_m: self.base_thickness_m,
            top_thickness_m: top,
            height_m: self.height_m,
        };
        let stock_vertices: [[f64; 3]; 8] = std::array::from_fn(|mask| {
            std::array::from_fn(|i| {
                stock.origin_m[i]
                    + if mask & 1 != 0 {
                        stock.u_axis[i] * stock.face_spans_m[0]
                    } else {
                        0.
                    }
                    + if mask & 2 != 0 {
                        stock.v_axis[i] * stock.face_spans_m[1]
                    } else {
                        0.
                    }
                    - if mask & 4 != 0 {
                        stock.outward_normal[i] * stock.inward_thickness_m
                    } else {
                        0.
                    }
            })
        });
        if geometry
            .vertices_m()
            .iter()
            .chain(stock_vertices.iter())
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 1e6)
        {
            return Err(invalid());
        }
        Ok(geometry)
    }
}
