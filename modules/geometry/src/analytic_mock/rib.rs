//! Exact square-ended tapered addition in stock-local coordinates.
use super::*;
#[derive(Debug, Clone)]
pub(super) struct Rib {
    pub center: [f64; 2],
    pub along_u: bool,
    pub length: f64,
    pub base_thickness: f64,
    pub top_thickness: f64,
    pub height: f64,
}
impl Rib {
    pub(super) fn limits(&self, top: bool) -> [[f64; 2]; 2] {
        let width = if top {
            self.top_thickness
        } else {
            self.base_thickness
        };
        let half = if self.along_u {
            [self.length / 2., width / 2.]
        } else {
            [width / 2., self.length / 2.]
        };
        std::array::from_fn(|i| [self.center[i] - half[i], self.center[i] + half[i]])
    }
    pub(super) fn vertices(&self, stock_height: f64) -> [[f64; 3]; 8] {
        std::array::from_fn(|mask| {
            let top = mask & 4 != 0;
            let limits = self.limits(top);
            [
                limits[0][usize::from(mask & 1 != 0)],
                limits[1][usize::from(mask & 2 != 0)],
                stock_height + if top { self.height } else { 0. },
            ]
        })
    }
    pub(super) fn mass(&self, frame: Frame, dimensions: [f64; 3]) -> Result<(f64, f64, [f64; 3])> {
        mesh::validate_frame(frame)?;
        let [x, y, z] = dimensions;
        let stock = x * y * z;
        let added = self.length * self.height * (self.base_thickness + self.top_thickness) / 2.;
        let axial = self.height * (self.base_thickness + 2. * self.top_thickness)
            / (3. * (self.base_thickness + self.top_thickness));
        let center = std::array::from_fn(|i| {
            (stock * [x / 2., y / 2., z / 2.][i]
                + added * [self.center[0], self.center[1], z + axial][i])
                / (stock + added)
        });
        let area = 2. * (x * y + x * z + y * z) - self.length * self.base_thickness
            + self.length * self.top_thickness
            + self.height * (self.base_thickness + self.top_thickness)
            + 2. * self.length
                * self
                    .height
                    .hypot((self.top_thickness - self.base_thickness) / 2.);
        Ok((stock + added, area, frame.map(center)))
    }
}
