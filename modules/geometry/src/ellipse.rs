//! ADR-053: analytic ellipse frames and normal prisms; no polygon truth.
use crate::{BoundingBox, MassProperties, RigidTransform, SketchPlacement};
use opencad_core::{OpenCadError, Result};
use serde::{Deserialize, Serialize};
fn invalid() -> OpenCadError {
    OpenCadError::validation("ellipse requires finite major >= minor > 1 nm and a proper orthonormal plane; only normal extrusion is supported")
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
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
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    a.map(|v| v * s)
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SolvedEllipse {
    pub center_m: [f64; 2],
    pub major_radius_m: f64,
    pub minor_radius_m: f64,
    pub axis_angle_rad: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EllipseFrame {
    pub center_m: [f64; 3],
    pub major_vector_m: [f64; 3],
    pub minor_vector_m: [f64; 3],
}
impl SolvedEllipse {
    pub fn frame(self, plane: SketchPlacement) -> Result<EllipseFrame> {
        let a = self.major_radius_m;
        let b = self.minor_radius_m;
        if !self
            .center_m
            .iter()
            .chain([a, b, self.axis_angle_rad].iter())
            .all(|v| v.is_finite())
            || b <= 1e-9
            || a < b
            || a > 1e6
        {
            return Err(invalid());
        }
        let x = plane.x_axis_m;
        let y = plane.y_axis_m;
        if !plane
            .origin_m
            .iter()
            .chain(x.iter())
            .chain(y.iter())
            .all(|v| v.is_finite())
            || (norm(x) - 1.).abs() > 1e-9
            || (norm(y) - 1.).abs() > 1e-9
            || dot(x, y).abs() > 1e-9
        {
            return Err(invalid());
        }
        let (sin, cos) = self.axis_angle_rad.sin_cos();
        let mut major = [0.; 3];
        let mut minor = [0.; 3];
        for i in 0..3 {
            major[i] = a * (x[i] * cos + y[i] * sin);
            minor[i] = b * (-x[i] * sin + y[i] * cos);
        }
        Ok(EllipseFrame {
            center_m: plane.map_point(self.center_m[0], self.center_m[1]),
            major_vector_m: major,
            minor_vector_m: minor,
        })
    }
}
impl EllipseFrame {
    pub fn radii(self) -> [f64; 2] {
        [norm(self.major_vector_m), norm(self.minor_vector_m)]
    }
    pub fn normal(self) -> [f64; 3] {
        let n = cross(self.major_vector_m, self.minor_vector_m);
        scale(n, 1. / norm(n))
    }
    pub fn native_data(self) -> [f64; 9] {
        let mut data = [0.; 9];
        data[..3].copy_from_slice(&self.center_m);
        data[3..6].copy_from_slice(&self.major_vector_m);
        data[6..].copy_from_slice(&self.minor_vector_m);
        data
    }
    pub fn prism(self, direction: [f64; 3], length_m: f64) -> Result<EllipsePrism> {
        let magnitude = norm(direction);
        if !direction.iter().all(|v| v.is_finite())
            || !length_m.is_finite()
            || length_m <= 1e-9
            || magnitude <= 1e-12
            || (dot(scale(direction, 1. / magnitude), self.normal()).abs() - 1.).abs() > 1e-9
        {
            return Err(invalid());
        }
        Ok(EllipsePrism {
            frame: self,
            delta_m: scale(direction, length_m / magnitude),
        })
    }
}
/// Analytic Mock metadata, not a BREP or document entity.
#[derive(Debug, Clone, Copy)]
pub struct EllipsePrism {
    pub frame: EllipseFrame,
    pub delta_m: [f64; 3],
}
impl EllipsePrism {
    pub fn bounding_box(self) -> BoundingBox {
        let mut min = [0.; 3];
        let mut max = [0.; 3];
        for i in 0..3 {
            let extent = self.frame.major_vector_m[i].hypot(self.frame.minor_vector_m[i]);
            min[i] = self.frame.center_m[i] - extent + self.delta_m[i].min(0.);
            max[i] = self.frame.center_m[i] + extent + self.delta_m[i].max(0.);
        }
        BoundingBox { min, max }
    }
    pub fn mass_properties(self, density: f64) -> Result<MassProperties> {
        if !density.is_finite() || density <= 0. {
            return Err(OpenCadError::validation("density must be positive kg/m3"));
        }
        let [a, b] = self.frame.radii();
        let h = norm(self.delta_m);
        let area = std::f64::consts::PI * a * b;
        let volume = area * h;
        let mut centre = self.frame.center_m;
        for (p, delta) in centre.iter_mut().zip(self.delta_m) {
            *p += delta / 2.;
        }
        Ok(MassProperties {
            volume_m3: volume,
            area_m2: 2. * area + ellipse_perimeter(a, b)? * h,
            mass_kg: volume * density,
            center_of_mass: centre,
        })
    }
    pub fn transformed(self, t: RigidTransform) -> Result<Self> {
        let columns: [[f64; 3]; 3] =
            std::array::from_fn(|i| std::array::from_fn(|j| t.rotation[j][i]));
        if !t
            .translation_m
            .iter()
            .chain(t.rotation.iter().flatten())
            .all(|v| v.is_finite())
            || (0..3).any(|i| (norm(columns[i]) - 1.).abs() > 1e-9)
            || (0..3).any(|i| (i + 1..3).any(|j| dot(columns[i], columns[j]).abs() > 1e-9))
            || (dot(cross(columns[0], columns[1]), columns[2]) - 1.).abs() > 1e-9
        {
            return Err(invalid());
        }
        let rotate = |v: [f64; 3]| std::array::from_fn(|i| dot(t.rotation[i], v));
        Ok(Self {
            frame: EllipseFrame {
                center_m: t.transform_point(self.frame.center_m),
                major_vector_m: rotate(self.frame.major_vector_m),
                minor_vector_m: rotate(self.frame.minor_vector_m),
            },
            delta_m: rotate(self.delta_m),
        })
    }
}
/// Complete elliptic integral E via AGM (bounded arithmetic, no curve sampling).
pub fn ellipse_perimeter(major: f64, minor: f64) -> Result<f64> {
    if !major.is_finite() || !minor.is_finite() || minor <= 1e-9 || major < minor {
        return Err(invalid());
    }
    let ratio = minor / major;
    let mut a = 1.;
    let mut b = ratio;
    let mut sum = (1. - ratio * ratio) / 2.;
    let mut weight = 1.;
    for _ in 0..32 {
        let c = (a - b) / 2.;
        sum += weight * c * c;
        let next_a = (a + b) / 2.;
        let next_b = (a * b).sqrt();
        a = next_a;
        b = next_b;
        if c.abs() <= a * 1e-15 {
            let e = std::f64::consts::PI / (2. * a) * (1. - sum);
            return Ok(4. * major * e);
        }
        weight *= 2.;
    }
    Err(OpenCadError::validation(
        "ellipse perimeter AGM did not converge",
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn circle_and_ellipse_perimeters_match_independent_references() {
        assert!(
            (ellipse_perimeter(0.02, 0.02).expect("circle") - std::f64::consts::TAU * 0.02).abs()
                < 1e-12
        );
        // Independent complete elliptic integral reference, a=30,b=20 mm.
        assert!(
            (ellipse_perimeter(0.03, 0.02).expect("ellipse") - 0.1586543958929059).abs() < 1e-12
        );
    }
}
