//! ADR-100: guarded numerical grid-faceted runtime; no source/native certificate.
use crate::{
    CircularFilletGridAreaComparison, CircularFilletGridAreaPrecision,
    CircularFilletGridMomentComparison, CircularFilletGridMomentPrecision,
    CircularFilletGridPrecision, CircularFilletGridSolid, CircularFilletGridVolumeComparison,
    CircularFilletGridVolumePrecision, CircularFilletMeshPrecision, CircularFilletReference,
    CylindricalRimsGeometry, RigidTransform,
};
use opencad_core::{Length, OpenCadError, Result};

/// Explicit authored frame, current body-local end rim and canonical quality.
/// All discrepancy/quality bounds refer to the unplaced input grid ONLY.
#[derive(Debug, Clone, Copy)]
pub struct FacetedCircularFilletInput {
    pub end_edge_id: u64,
    pub origin_m: [f64; 3],
    pub phase_x: [f64; 3],
    pub axis: [f64; 3],
    pub cylinder_radius: Length,
    pub cylinder_height: Length,
    pub minimum_radius: Length,
    pub maximum_radius: Length,
    pub mesh_precision: CircularFilletMeshPrecision,
    pub grid_precision: CircularFilletGridPrecision,
    pub volume_precision: CircularFilletGridVolumePrecision,
    pub area_precision: CircularFilletGridAreaPrecision,
    pub moment_precision: CircularFilletGridMomentPrecision,
}
/// Immutable preparation, not authority over a kernel body. Kernels requery the
/// actual current cylinder/rim rather than accepting caller-prepared records.
#[derive(Debug, Clone)]
pub struct FacetedCircularFilletPlan {
    grid: CircularFilletGridSolid,
    placement: RigidTransform,
    volume: CircularFilletGridVolumeComparison,
    area: CircularFilletGridAreaComparison,
    moments: CircularFilletGridMomentComparison,
}
impl FacetedCircularFilletPlan {
    pub fn grid(&self) -> &CircularFilletGridSolid {
        &self.grid
    }
    pub fn placement(&self) -> RigidTransform {
        self.placement
    }
    pub fn volume_comparison(&self) -> CircularFilletGridVolumeComparison {
        self.volume
    }
    pub fn area_comparison(&self) -> CircularFilletGridAreaComparison {
        self.area
    }
    pub fn moment_comparison(&self) -> CircularFilletGridMomentComparison {
        self.moments
    }
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("invalid guarded grid-faceted periodic cylinder fillet")
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let norm = dot(a, a).sqrt();
    a.map(|v| v / norm)
}
impl FacetedCircularFilletInput {
    /// Check the authored directions BEFORE normalization, never infer phase
    /// from a native circle seam or edge orientation.
    pub fn placement(&self) -> Result<RigidTransform> {
        if self
            .origin_m
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1e6)
            || self
                .axis
                .into_iter()
                .chain(self.phase_x)
                .any(|v| !v.is_finite())
            || (dot(self.axis, self.axis).sqrt() - 1.).abs() > 1e-12
            || (dot(self.phase_x, self.phase_x).sqrt() - 1.).abs() > 1e-12
            || dot(self.axis, self.phase_x).abs() > 1e-12
        {
            return Err(invalid());
        }
        let z = unit(self.axis);
        let y = unit(cross(z, unit(self.phase_x)));
        let x = cross(y, z);
        let frame = RigidTransform {
            translation_m: self.origin_m,
            rotation: std::array::from_fn(|i| [x[i], y[i], z[i]]),
        };
        crate::cubic_bezier::validate_rigid_transform(frame)?;
        Ok(frame)
    }
    /// Pure preparation for a supplied witness. A kernel MUST independently
    /// obtain that witness from the current (body,edge) pair before execution.
    pub fn prepare(&self, rims: &CylindricalRimsGeometry) -> Result<FacetedCircularFilletPlan> {
        let placement = self.placement()?;
        rims.validate()?;
        let stock = rims.stock;
        if self.end_edge_id == 0
            || self.end_edge_id != rims.end_edge_id
            || (stock.radius_m - self.cylinder_radius.meters()).abs() > 1e-9
            || (stock.height_m - self.cylinder_height.meters()).abs() > 1e-9
            || stock
                .origin_m
                .into_iter()
                .zip(self.origin_m)
                .any(|(a, b)| (a - b).abs() > 1e-9)
            || stock
                .axis
                .into_iter()
                .zip(placement.rotation.map(|r| r[2]))
                .any(|(a, b)| (a - b).abs() > 1e-12)
        {
            return Err(invalid());
        }
        let reference = CircularFilletReference::new(
            self.cylinder_radius,
            self.cylinder_height,
            self.minimum_radius,
            self.maximum_radius,
        )?;
        let grid = reference
            .closed_reference_mesh(self.mesh_precision)?
            .grid_solid(self.grid_precision)?;
        // Range/finite placement refusal also precedes any kernel allocation.
        grid.solid().transformed(placement)?;
        // No output body exists until EVERY explicit budget has passed.
        let volume = grid.volume_comparison(self.volume_precision)?;
        let area = grid.area_comparison(self.area_precision)?;
        let moments = grid.moment_comparison(self.moment_precision)?;
        Ok(FacetedCircularFilletPlan {
            grid,
            placement,
            volume,
            area,
            moments,
        })
    }
}
