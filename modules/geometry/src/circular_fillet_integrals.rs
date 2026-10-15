//! ADR-080: mathematical boundary flux, not a native/Mock mass certificate.
use crate::{bernstein::P, interval::I, CircularFilletBezierBand};
use opencad_core::{OpenCadError, Result};

/// Outward enclosures for the actual mathematical polynomial boundary.
/// These signed integrals do not establish an embedded physical solid.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletBoundaryIntegrals {
    pub volume_m3: [f64; 2],
    /// Integrals of x, y, z over signed volume, respectively, in m⁴.
    pub first_moments_m4: [[f64; 2]; 3],
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("closed diagnostic boundary or integral not certified")
}
fn same(a: [f64; 3], b: [f64; 3]) -> bool {
    // Exact representation identity is the shared-boundary contract, not a
    // floating-point proximity test. Signed zeros describe the same polynomial.
    a.into_iter()
        .zip(b)
        .all(|(x, y)| x.to_bits() == y.to_bits() || (x == 0. && y == 0.))
}
impl CircularFilletBezierBand {
    pub(crate) fn diagnostic_top_height(&self) -> Result<f64> {
        let n = self.divisions_per_chart_axis;
        if n == 0 || n > 32 || !n.is_power_of_two() || self.cells.len() != 4 * n * n {
            return Err(invalid());
        }
        let height = self.cells[n - 1].patch.control_points_m[0][3][2];
        if !height.is_finite() || height <= 0. {
            return Err(invalid());
        }
        for (index, cell) in self.cells.iter().enumerate() {
            let chart = index / (n * n);
            let i = index / n % n;
            let j = index % n;
            if usize::from(cell.chart) != chart
                || cell.chart_domain
                    != [
                        i as f64 / n as f64,
                        (i + 1) as f64 / n as f64,
                        j as f64 / n as f64,
                        (j + 1) as f64 / n as f64,
                    ]
            {
                return Err(invalid());
            }
            let poles = cell.patch.control_points_m;
            if poles.iter().flatten().flatten().any(|x| !x.is_finite()) {
                return Err(invalid());
            }
            // u wraps across quarter charts and the full period.
            let next_u = &self.cells[(index + n) % self.cells.len()]
                .patch
                .control_points_m;
            for k in 0..4 {
                if !same(poles[3][k], next_u[0][k]) {
                    return Err(invalid());
                }
                if j + 1 < n
                    && !same(
                        poles[k][3],
                        self.cells[index + 1].patch.control_points_m[k][0],
                    )
                {
                    return Err(invalid());
                }
                if j + 1 == n && poles[k][3][2].to_bits() != height.to_bits() {
                    return Err(invalid());
                }
            }
        }
        Ok(height)
    }

    /// ADR-080: enclose signed flux integrals of the current band, ruled side
    /// skirts down to z=0 and planar caps. At most 4096 cells; shared edges and
    /// a common positive top height are required. No embedding or body admission.
    pub fn closed_diagnostic_integrals(&self) -> Result<CircularFilletBoundaryIntegrals> {
        let height = self.diagnostic_top_height()?;
        let n = self.divisions_per_chart_axis;
        let mut sums = [I::exact(0.); 4];
        let h = I::exact(height);
        let two = I::exact(2.);
        for (index, cell) in self.cells.iter().enumerate() {
            let component = |axis: usize| -> P {
                P {
                    u: 3,
                    v: 3,
                    c: cell
                        .patch
                        .control_points_m
                        .iter()
                        .flat_map(|row| row.iter().map(move |p| I::exact(p[axis])))
                        .collect(),
                }
            };
            let x = component(0);
            let y = component(1);
            let z = component(2);
            let jacobian = x
                .derivative(0)
                .product(&y.derivative(1))
                .difference(&x.derivative(1).product(&y.derivative(0)));
            let zj = z.product(&jacobian);
            sums[0] = sums[0] + zj.integral();
            sums[1] = sums[1] + x.product(&zj).integral();
            sums[2] = sums[2] + y.product(&zj).integral();
            sums[3] = sums[3] + z.product(&zj).integral() / two;
            if index % n == n - 1 {
                let curve = |p: &P| P {
                    u: 3,
                    v: 0,
                    c: (0..4).map(|i| p.at(i, 3)).collect(),
                };
                let cx = curve(&x);
                let cy = curve(&y);
                let area = cx.product(&cy.derivative(0)).integral();
                sums[0] = sums[0] + h * area;
                sums[1] = sums[1] + h * cx.product(&cx).product(&cy.derivative(0)).integral() / two;
                sums[2] = sums[2] - h * cy.product(&cy).product(&cx.derivative(0)).integral() / two;
                sums[3] = sums[3] + h * h * area / two;
            }
        }
        if sums.iter().any(|s| !s.valid()) {
            return Err(invalid());
        }
        let bounds = |s: I| [s.lo, s.hi];
        Ok(CircularFilletBoundaryIntegrals {
            volume_m3: bounds(sums[0]),
            first_moments_m4: [bounds(sums[1]), bounds(sums[2]), bounds(sums[3])],
        })
    }
}
