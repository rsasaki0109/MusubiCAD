//! ADR-092: bounded closed reference triangle shell, not a product/kernel body.
use crate::{
    circular_fillet_patch::{corner, norm_upper},
    interval::I,
    CircularFilletReference,
};
use opencad_core::{Length, OpenCadError, Result};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletMeshPrecision {
    pub maximum_position_error: Length,
    pub maximum_normal_error: f64,
    pub max_vertices: usize,
    pub max_triangles: usize,
}
/// Signed boundary integrals, not an assertion of global geometric embedding.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletMeshIntegrals {
    pub volume_m3: [f64; 2],
    pub area_m2: [f64; 2],
    pub first_moments_m4: [[f64; 2]; 3],
}
/// Immutable f64 SI mesh; the bounds exclude later f32/native conversion.
#[derive(Debug, Clone)]
pub struct CircularFilletReferenceMesh {
    reference: CircularFilletReference,
    n: usize,
    vertices: Vec<[f64; 3]>,
    triangles: Vec<[u32; 3]>,
    groups: Vec<u8>,
    position_error: f64,
    normal_error: f64,
    integrals: CircularFilletMeshIntegrals,
}
/// Final quality after the existing TriangleSolid grid admission (ADR-093).
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletGridPrecision {
    pub maximum_position_error: Length,
    pub maximum_normal_error: f64,
}
/// Immutable conversion certificate, not a document feature or native body.
#[derive(Debug, Clone)]
pub struct CircularFilletGridSolid {
    reference: CircularFilletReference,
    solid: crate::TriangleSolid,
    position_error: f64,
    normal_error: f64,
    displacement: f64,
    normal_change: f64,
}
/// Explicit SI integral precision and permitted absolute signed-volume difference.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletGridVolumePrecision {
    pub maximum_reference_interval_width_m3: f64,
    pub maximum_absolute_difference_m3: f64,
    pub max_cells: usize,
}
/// ADR-095: signed boundary-integral comparison, not an embedding/native proof.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletGridVolumeComparison {
    grid_flux_m3: [f64; 2],
    ideal_volume_m3: [f64; 2],
    difference_m3: [f64; 2],
    absolute_difference: f64,
    relative_difference: f64,
    reference_cells: usize,
}
impl CircularFilletGridVolumeComparison {
    pub fn grid_signed_volume_flux_m3(&self) -> [f64; 2] {
        self.grid_flux_m3
    }
    pub fn ideal_volume_m3(&self) -> [f64; 2] {
        self.ideal_volume_m3
    }
    pub fn signed_difference_m3(&self) -> [f64; 2] {
        self.difference_m3
    }
    pub fn absolute_difference_upper_m3(&self) -> f64 {
        self.absolute_difference
    }
    pub fn relative_difference_upper(&self) -> f64 {
        self.relative_difference
    }
    pub fn reference_cells(&self) -> usize {
        self.reference_cells
    }
}
/// Explicit SI ideal-area width and permitted absolute area difference (ADR-097).
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletGridAreaPrecision {
    pub maximum_reference_interval_width_m2: f64,
    pub maximum_absolute_difference_m2: f64,
    pub max_cells: usize,
}
/// Stored-grid unsigned triangle area comparison; no embedding/native proof.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletGridAreaComparison {
    grid_area_m2: [f64; 2],
    ideal_area_m2: [f64; 2],
    difference_m2: [f64; 2],
    absolute_difference: f64,
    relative_difference: f64,
    reference_cells: usize,
}
impl CircularFilletGridAreaComparison {
    pub fn grid_unsigned_area_m2(&self) -> [f64; 2] {
        self.grid_area_m2
    }
    pub fn ideal_area_m2(&self) -> [f64; 2] {
        self.ideal_area_m2
    }
    pub fn signed_difference_m2(&self) -> [f64; 2] {
        self.difference_m2
    }
    pub fn absolute_difference_upper_m2(&self) -> f64 {
        self.absolute_difference
    }
    pub fn relative_difference_upper(&self) -> f64 {
        self.relative_difference
    }
    pub fn reference_cells(&self) -> usize {
        self.reference_cells
    }
}
/// Explicit ideal widths and maximum error in EACH signed moment/centroid axis.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletGridMomentPrecision {
    pub ideal: crate::CircularFilletMomentPrecision,
    pub maximum_absolute_first_moment_difference_m4: f64,
    pub maximum_absolute_centroid_difference: Length,
}
/// ADR-099: boundary-integral/quotient comparison; no embedding/native proof.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletGridMomentComparison {
    grid_volume_m3: [f64; 2],
    grid_first_moments_m4: [[f64; 2]; 3],
    grid_centroid_m: [[f64; 2]; 3],
    ideal: crate::CircularFilletMomentBounds,
    first_moment_difference_m4: [[f64; 2]; 3],
    centroid_difference_m: [[f64; 2]; 3],
    absolute_first_moment_difference_upper_m4: [f64; 3],
    absolute_centroid_difference_upper_m: [f64; 3],
}
impl CircularFilletGridMomentComparison {
    pub fn grid_signed_volume_flux_m3(&self) -> [f64; 2] {
        self.grid_volume_m3
    }
    pub fn grid_signed_first_moments_m4(&self) -> [[f64; 2]; 3] {
        self.grid_first_moments_m4
    }
    pub fn grid_centroid_quotient_m(&self) -> [[f64; 2]; 3] {
        self.grid_centroid_m
    }
    pub fn ideal(&self) -> crate::CircularFilletMomentBounds {
        self.ideal
    }
    pub fn signed_first_moment_difference_m4(&self) -> [[f64; 2]; 3] {
        self.first_moment_difference_m4
    }
    pub fn signed_centroid_difference_m(&self) -> [[f64; 2]; 3] {
        self.centroid_difference_m
    }
    pub fn absolute_first_moment_difference_upper_m4(&self) -> [f64; 3] {
        self.absolute_first_moment_difference_upper_m4
    }
    pub fn absolute_centroid_difference_upper_m(&self) -> [f64; 3] {
        self.absolute_centroid_difference_upper_m
    }
}
impl CircularFilletGridSolid {
    /// Compare actual stored signed fluxes/positive-volume quotients with the
    /// privately bound ideal. No native/placed/f32 or embedding admission.
    pub fn moment_comparison(
        &self,
        p: CircularFilletGridMomentPrecision,
    ) -> Result<CircularFilletGridMomentComparison> {
        let invalid = || {
            OpenCadError::validation(
                "grid reference moment precision, work or discrepancy not certified",
            )
        };
        let maximum_centroid = p.maximum_absolute_centroid_difference.meters();
        if !p.maximum_absolute_first_moment_difference_m4.is_finite()
            || p.maximum_absolute_first_moment_difference_m4 <= 0.
            || !maximum_centroid.is_finite()
            || maximum_centroid <= 0.
        {
            return Err(invalid());
        }
        let ideal = self.reference.moment_bounds(p.ideal)?;
        let (volume, moments) = grid_moment_flux(&self.solid);
        if !volume.valid() || volume.lo <= 0. {
            return Err(invalid());
        }
        let mut result = CircularFilletGridMomentComparison {
            grid_volume_m3: [volume.lo, volume.hi],
            grid_first_moments_m4: [[0.; 2]; 3],
            grid_centroid_m: [[0.; 2]; 3],
            ideal,
            first_moment_difference_m4: [[0.; 2]; 3],
            centroid_difference_m: [[0.; 2]; 3],
            absolute_first_moment_difference_upper_m4: [0.; 3],
            absolute_centroid_difference_upper_m: [0.; 3],
        };
        for (axis, moment) in moments.into_iter().enumerate() {
            let centroid = moment / volume;
            let [lo, hi] = ideal.first_moments_m4[axis];
            let md = moment - I { lo, hi };
            let [lo, hi] = ideal.centroid_m[axis];
            let cd = centroid - I { lo, hi };
            let me = md.lo.abs().max(md.hi.abs());
            let ce = cd.lo.abs().max(cd.hi.abs());
            if !moment.valid()
                || !centroid.valid()
                || !md.valid()
                || !cd.valid()
                || !me.is_finite()
                || !ce.is_finite()
                || me > p.maximum_absolute_first_moment_difference_m4
                || ce > maximum_centroid
            {
                return Err(invalid());
            }
            result.grid_first_moments_m4[axis] = [moment.lo, moment.hi];
            result.grid_centroid_m[axis] = [centroid.lo, centroid.hi];
            result.first_moment_difference_m4[axis] = [md.lo, md.hi];
            result.centroid_difference_m[axis] = [cd.lo, cd.hi];
            result.absolute_first_moment_difference_upper_m4[axis] = me;
            result.absolute_centroid_difference_upper_m[axis] = ce;
        }
        Ok(result)
    }
    /// Compare unsigned area of actual stored triangles, counted with
    /// multiplicity, to the privately bound ideal. Excludes native/placed/f32.
    pub fn area_comparison(
        &self,
        p: CircularFilletGridAreaPrecision,
    ) -> Result<CircularFilletGridAreaComparison> {
        let invalid = || {
            OpenCadError::validation(
                "grid reference area precision, work or discrepancy not certified",
            )
        };
        if !p.maximum_reference_interval_width_m2.is_finite()
            || p.maximum_reference_interval_width_m2 <= 0.
            || !p.maximum_absolute_difference_m2.is_finite()
            || p.maximum_absolute_difference_m2 <= 0.
            || !(4..=262144).contains(&p.max_cells)
        {
            return Err(invalid());
        }
        let reference = self
            .reference
            .surface_area_bounds_m2(p.maximum_reference_interval_width_m2, p.max_cells)?;
        let ideal = I {
            lo: reference.body_lower_m2,
            hi: reference.body_upper_m2,
        };
        let area = grid_surface_area(&self.solid)?;
        if !area.valid() || area.lo <= 0. || !ideal.valid() || ideal.lo <= 0. {
            return Err(invalid());
        }
        let delta = area - ideal;
        let absolute = delta.lo.abs().max(delta.hi.abs());
        let relative = (I::exact(absolute) / I::exact(ideal.lo)).hi;
        if !delta.valid()
            || !absolute.is_finite()
            || !relative.is_finite()
            || absolute > p.maximum_absolute_difference_m2
        {
            return Err(invalid());
        }
        Ok(CircularFilletGridAreaComparison {
            grid_area_m2: [area.lo, area.hi],
            ideal_area_m2: [ideal.lo, ideal.hi],
            difference_m2: [delta.lo, delta.hi],
            absolute_difference: absolute,
            relative_difference: relative,
            reference_cells: reference.cells,
        })
    }
    /// Compare the exact stored-grid signed flux to its privately bound ideal reference.
    /// No native/placed/f32 or exact global embedding certificate is implied.
    pub fn volume_comparison(
        &self,
        p: CircularFilletGridVolumePrecision,
    ) -> Result<CircularFilletGridVolumeComparison> {
        let invalid = || {
            OpenCadError::validation(
                "grid reference volume precision, work or discrepancy not certified",
            )
        };
        if !p.maximum_reference_interval_width_m3.is_finite()
            || p.maximum_reference_interval_width_m3 <= 0.
            || !p.maximum_absolute_difference_m3.is_finite()
            || p.maximum_absolute_difference_m3 <= 0.
            || !(4..=262144).contains(&p.max_cells)
        {
            return Err(invalid());
        }
        let reference = self
            .reference
            .volume_bounds_m3(p.maximum_reference_interval_width_m3, p.max_cells)?;
        let ideal = I {
            lo: reference.body_lower_m3,
            hi: reference.body_upper_m3,
        };
        let flux = grid_volume_flux(&self.solid);
        if !flux.valid() || flux.lo <= 0. || !ideal.valid() || ideal.lo <= 0. {
            return Err(invalid());
        }
        let delta = flux - ideal;
        let absolute = delta.lo.abs().max(delta.hi.abs());
        let relative = (I::exact(absolute) / I::exact(ideal.lo)).hi;
        if !delta.valid()
            || !absolute.is_finite()
            || !relative.is_finite()
            || absolute > p.maximum_absolute_difference_m3
        {
            return Err(invalid());
        }
        Ok(CircularFilletGridVolumeComparison {
            grid_flux_m3: [flux.lo, flux.hi],
            ideal_volume_m3: [ideal.lo, ideal.hi],
            difference_m3: [delta.lo, delta.hi],
            absolute_difference: absolute,
            relative_difference: relative,
            reference_cells: reference.cells,
        })
    }
    pub fn solid(&self) -> &crate::TriangleSolid {
        &self.solid
    }
    pub fn position_error_upper_m(&self) -> f64 {
        self.position_error
    }
    pub fn normal_error_upper(&self) -> f64 {
        self.normal_error
    }
    pub fn grid_displacement_upper_m(&self) -> f64 {
        self.displacement
    }
    pub fn grid_normal_change_upper(&self) -> f64 {
        self.normal_change
    }
}
fn grid_volume_flux(solid: &crate::TriangleSolid) -> I {
    solid.triangles().iter().fold(I::exact(0.), |sum, t| {
        let [a, b, c] = t.map(|i| solid.vertices_m()[i as usize].map(I::exact));
        sum + dot(a, cross(b, c)) / I::exact(6.)
    })
}
fn grid_moment_flux(solid: &crate::TriangleSolid) -> (I, [I; 3]) {
    solid
        .triangles()
        .iter()
        .fold((I::exact(0.), [I::exact(0.); 3]), |(v, mut m), t| {
            let [a, b, c] = t.map(|i| solid.vertices_m()[i as usize].map(I::exact));
            let vt = dot(a, cross(b, c)) / I::exact(6.);
            for k in 0..3 {
                m[k] = m[k] + vt * (a[k] + b[k] + c[k]) / I::exact(4.);
            }
            (v + vt, m)
        })
}
fn grid_surface_area(solid: &crate::TriangleSolid) -> Result<I> {
    solid.triangles().iter().try_fold(I::exact(0.), |sum, t| {
        let points = t.map(|i| solid.vertices_m()[i as usize]);
        Ok(sum + facet(points)?.1)
    })
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("closed periodic reference mesh or requested quality not certified")
}
fn dot(a: [I; 3], b: [I; 3]) -> I {
    a.into_iter()
        .zip(b)
        .fold(I::exact(0.), |s, (a, b)| s + a * b)
}
fn cross(a: [I; 3], b: [I; 3]) -> [I; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn facet(points: [[f64; 3]; 3]) -> Result<([I; 3], I)> {
    let [a, b, c] = points.map(|p| p.map(I::exact));
    let n = cross(
        std::array::from_fn(|k| b[k] - a[k]),
        std::array::from_fn(|k| c[k] - a[k]),
    );
    let q = n.into_iter().fold(I::exact(0.), |s, x| s + x.square());
    if !q.valid() || q.lo <= 0. {
        return Err(invalid());
    }
    let size = q.sqrt();
    let unit = n.map(|x| x / size);
    if unit.iter().any(|x| !x.valid()) {
        return Err(invalid());
    }
    Ok((unit, size / I::exact(2.)))
}
fn circle_normal(chart: u8, u0: f64, u1: f64) -> [I; 3] {
    let u = I { lo: u0, hi: u1 };
    let den = I::exact(1.) + u.square();
    let c = (I::exact(1.) - u.square()) / den;
    let s = I::exact(2.) * u / den;
    let (x, y) = match chart {
        0 => (c, s),
        1 => (-s, c),
        2 => (-c, -s),
        _ => (s, -c),
    };
    [x, y, I::exact(0.)]
}
impl CircularFilletReferenceMesh {
    pub fn divisions_per_chart_axis(&self) -> usize {
        self.n
    }
    pub fn vertices_m(&self) -> &[[f64; 3]] {
        &self.vertices
    }
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
    /// Logical diagnostic groups, NOT semantic/native topology IDs.
    pub fn triangle_face_groups(&self) -> &[u8] {
        &self.groups
    }
    pub fn position_error_upper_m(&self) -> f64 {
        self.position_error
    }
    pub fn normal_error_upper(&self) -> f64 {
        self.normal_error
    }
    pub fn boundary_integrals(&self) -> CircularFilletMeshIntegrals {
        self.integrals
    }
    /// Admit the actual grid solid and certify its final whole-facet quality.
    /// Existing admission limits and numeric intersection checks are unchanged.
    /// No refinement, repair, transform or partial result is performed.
    pub fn grid_solid(&self, p: CircularFilletGridPrecision) -> Result<CircularFilletGridSolid> {
        let maximum = p.maximum_position_error.meters();
        if !maximum.is_finite()
            || maximum <= 0.
            || !p.maximum_normal_error.is_finite()
            || p.maximum_normal_error <= 0.
            || p.maximum_normal_error >= 1.
            || self.position_error > maximum
            || self.normal_error > p.maximum_normal_error
            || self.triangles.len() > crate::triangle_solid::MAX_STL_TRIANGLES
        {
            return Err(invalid());
        }
        let points: Vec<_> = self
            .triangles
            .iter()
            .map(|t| t.map(|i| self.vertices[i as usize]))
            .collect();
        let solid = crate::TriangleSolid::from_triangles(&points)?;
        let (displacement, normal_change) = grid_changes(self, &solid)?;
        let position_error = (I::exact(self.position_error) + I::exact(displacement)).hi;
        let normal_error = (I::exact(self.normal_error) + I::exact(normal_change)).hi;
        if !position_error.is_finite()
            || !normal_error.is_finite()
            || position_error > maximum
            || normal_error > p.maximum_normal_error
        {
            return Err(invalid());
        }
        Ok(CircularFilletGridSolid {
            reference: self.reference,
            solid,
            position_error,
            normal_error,
            displacement,
            normal_change,
        })
    }
    fn add(&mut self, t: [u32; 3], group: u8, ideal: [I; 3], maximum: f64) -> Result<()> {
        let (normal, _) = facet(t.map(|i| self.vertices[i as usize]))?;
        let error = norm_upper(std::array::from_fn(|k| normal[k] - ideal[k]));
        let alignment = dot(normal, ideal);
        if !alignment.valid() || alignment.lo <= 0. || !error.is_finite() || error > maximum {
            return Err(invalid());
        }
        self.normal_error = self.normal_error.max(error);
        self.triangles.push(t);
        self.groups.push(group);
        Ok(())
    }
    fn validate_and_measure(&mut self) -> Result<()> {
        if self.triangles.len() != self.groups.len() {
            return Err(invalid());
        }
        let mut edges: BTreeMap<(u32, u32), Vec<(usize, bool)>> = BTreeMap::new();
        let mut used = BTreeSet::new();
        let mut neighbours = vec![Vec::new(); self.triangles.len()];
        let mut sums = [I::exact(0.); 5];
        for (face, t) in self.triangles.iter().enumerate() {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                used.insert(a);
                edges
                    .entry((a.min(b), a.max(b)))
                    .or_default()
                    .push((face, a < b));
            }
            let p = t.map(|i| self.vertices[i as usize]);
            let [a, b, c] = p.map(|p| p.map(I::exact));
            let volume = dot(a, cross(b, c)) / I::exact(6.);
            sums[0] = sums[0] + volume;
            sums[1] = sums[1] + facet(p)?.1;
            for k in 0..3 {
                sums[k + 2] = sums[k + 2] + volume * (a[k] + b[k] + c[k]) / I::exact(4.);
            }
        }
        for uses in edges.values() {
            if uses.len() != 2 || uses[0].1 == uses[1].1 {
                return Err(invalid());
            }
            neighbours[uses[0].0].push(uses[1].0);
            neighbours[uses[1].0].push(uses[0].0);
        }
        let mut seen = BTreeSet::new();
        let mut pending = vec![0];
        while let Some(i) = pending.pop() {
            if seen.insert(i) {
                pending.extend(&neighbours[i]);
            }
        }
        if seen.len() != self.triangles.len()
            || used.len() != self.vertices.len()
            || self.vertices.len() + self.triangles.len() != edges.len() + 2
            || self.groups.iter().copied().collect::<BTreeSet<_>>() != (1..=10).collect()
            || sums.iter().any(|x| !x.valid())
            || sums[0].lo <= 0.
        {
            return Err(invalid());
        }
        self.integrals = CircularFilletMeshIntegrals {
            volume_m3: [sums[0].lo, sums[0].hi],
            area_m2: [sums[1].lo, sums[1].hi],
            first_moments_m4: std::array::from_fn(|k| [sums[k + 2].lo, sums[k + 2].hi]),
        };
        Ok(())
    }
}
// Ordered face/corner identity, not a tolerance-based nearest-point match.
fn grid_changes(
    mesh: &CircularFilletReferenceMesh,
    solid: &crate::TriangleSolid,
) -> Result<(f64, f64)> {
    if mesh.vertices.len() != solid.vertices_m().len()
        || mesh.triangles.len() != solid.triangles().len()
    {
        return Err(invalid());
    }
    let mut forward = vec![None; mesh.vertices.len()];
    let mut reverse = vec![None; solid.vertices_m().len()];
    let mut displacement = 0_f64;
    let mut normal_change = 0_f64;
    for (original, admitted) in mesh.triangles.iter().zip(solid.triangles()) {
        for (&a, &b) in original.iter().zip(admitted) {
            let (a, b) = (a as usize, b as usize);
            if forward[a].is_some_and(|id| id != b) || reverse[b].is_some_and(|id| id != a) {
                return Err(invalid());
            }
            forward[a] = Some(b);
            reverse[b] = Some(a);
            let d = norm_upper(std::array::from_fn(|k| {
                I::exact(solid.vertices_m()[b][k]) - I::exact(mesh.vertices[a][k])
            }));
            if !d.is_finite() {
                return Err(invalid());
            }
            displacement = displacement.max(d);
        }
        let (a, _) = facet(original.map(|i| mesh.vertices[i as usize]))?;
        let (b, _) = facet(admitted.map(|i| solid.vertices_m()[i as usize]))?;
        let d = norm_upper(std::array::from_fn(|k| b[k] - a[k]));
        if !d.is_finite() {
            return Err(invalid());
        }
        normal_change = normal_change.max(d);
    }
    if forward.iter().any(Option::is_none) || reverse.iter().any(Option::is_none) {
        return Err(invalid());
    }
    Ok((displacement, normal_change))
}
fn level(
    r: CircularFilletReference,
    n: usize,
    p: CircularFilletMeshPrecision,
) -> Result<CircularFilletReferenceMesh> {
    let k = 4 * n;
    let stride = 2 * n + 1;
    let mut mesh = CircularFilletReferenceMesh {
        reference: r,
        n,
        vertices: vec![[0.; 3]; k * stride + 2],
        triangles: Vec::with_capacity(16 * n * n + 8 * n),
        groups: Vec::with_capacity(16 * n * n + 8 * n),
        position_error: 0.,
        normal_error: 0.,
        integrals: CircularFilletMeshIntegrals {
            volume_m3: [0.; 2],
            area_m2: [0.; 2],
            first_moments_m4: [[0.; 2]; 3],
        },
    };
    let mut side_round = 0_f64;
    let mut end_round = 0_f64;
    for i in 0..k {
        let (chart, u) = ((i / n) as u8, (i % n) as f64 / n as f64);
        for j in 0..=n {
            let (v, e) = corner(r, chart, u, j as f64 / n as f64)?;
            mesh.vertices[i * stride + n + j] = v;
            if j == 0 {
                side_round = side_round.max(e);
            }
            if j == n {
                end_round = end_round.max(e);
            }
        }
    }
    let contact_round = side_round;
    for i in 0..k {
        let s = mesh.vertices[i * stride + n];
        for j in 0..n {
            let w = j as f64 / n as f64;
            let v = [s[0], s[1], s[2] * w];
            mesh.vertices[i * stride + j] = v;
            // Original ideal side contact is inside each component radius.
            let exact: [I; 3] = std::array::from_fn(|a| {
                let x = I {
                    lo: (I::exact(s[a]) - I::exact(contact_round)).lo,
                    hi: (I::exact(s[a]) + I::exact(contact_round)).hi,
                };
                if a == 2 {
                    x * I::exact(w)
                } else {
                    x
                }
            });
            side_round = side_round.max(norm_upper(std::array::from_fn(|a| {
                I::exact(v[a]) - exact[a]
            })));
        }
    }
    let top = k * stride;
    let bottom = top + 1;
    mesh.vertices[top] = [0., 0., r.cylinder_height().meters()];
    let radius = I::exact(r.cylinder_radius().meters());
    let amplitude =
        (I::exact(r.max_radius().meters()) - I::exact(r.min_radius().meters())) / I::exact(2.);
    let h = I::exact(1.) / I::exact(n as f64);
    let side_bound = ((radius.square() + amplitude.square()).sqrt() + amplitude / I::exact(2.))
        * h.square()
        + I::exact(side_round);
    let top_bound = (radius + I::exact(2.) * amplitude) * h.square() + I::exact(end_round);
    let bottom_bound = radius * h.square() + I::exact(side_round);
    for b in [side_bound, top_bound, bottom_bound] {
        if !b.valid() || b.hi > p.maximum_position_error.meters() {
            return Err(invalid());
        }
        mesh.position_error = mesh.position_error.max(b.hi);
    }
    let id = |i: usize, j: usize| ((i % k) * stride + j) as u32;
    for i in 0..k {
        let chart = (i / n) as u8;
        let u0 = (i % n) as f64 / n as f64;
        let u1 = (i % n + 1) as f64 / n as f64;
        for j in 0..2 * n {
            let points = [id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)];
            let (group, ideal) = if j < n {
                (5 + chart, circle_normal(chart, u0, u1))
            } else {
                let b = r.patch_bounds(
                    chart,
                    u0,
                    u1,
                    (j - n) as f64 / n as f64,
                    (j - n + 1) as f64 / n as f64,
                )?;
                if b.linear_interpolation_error_upper_m > p.maximum_position_error.meters() {
                    return Err(invalid());
                }
                // Representation identity of shared canonical corners, not proximity.
                for (id, corner) in points.into_iter().zip(b.corner_points_m) {
                    if mesh.vertices[id as usize]
                        .into_iter()
                        .zip(corner)
                        .any(|(x, y)| x.to_bits() != y.to_bits() && !(x == 0. && y == 0.))
                    {
                        return Err(invalid());
                    }
                }
                mesh.position_error = mesh
                    .position_error
                    .max(b.linear_interpolation_error_upper_m);
                (
                    1 + chart,
                    std::array::from_fn(|a| I {
                        lo: b.normal_lower[a],
                        hi: b.normal_upper[a],
                    }),
                )
            };
            for t in [
                [points[0], points[1], points[2]],
                [points[0], points[2], points[3]],
            ] {
                mesh.add(t, group, ideal, p.maximum_normal_error)?;
            }
        }
        mesh.add(
            [id(i, 2 * n), id(i + 1, 2 * n), top as u32],
            9,
            [I::exact(0.), I::exact(0.), I::exact(1.)],
            p.maximum_normal_error,
        )?;
        mesh.add(
            [id(i + 1, 0), id(i, 0), bottom as u32],
            10,
            [I::exact(0.), I::exact(0.), I::exact(-1.)],
            p.maximum_normal_error,
        )?;
    }
    mesh.validate_and_measure()?;
    Ok(mesh)
}
impl CircularFilletReference {
    /// Complete dyadic shell with bounded position and facet-normal errors.
    /// No global embedding, kernel body, STL welding or f32/native conversion
    /// admission. No partial result, external error metadata or hidden I/O.
    pub fn closed_reference_mesh(
        self,
        p: CircularFilletMeshPrecision,
    ) -> Result<CircularFilletReferenceMesh> {
        let position = p.maximum_position_error.meters();
        if !position.is_finite()
            || position <= 0.
            || !p.maximum_normal_error.is_finite()
            || p.maximum_normal_error <= 0.
            || p.maximum_normal_error >= 1.
            || !(14..=33026).contains(&p.max_vertices)
            || !(24..=66048).contains(&p.max_triangles)
        {
            return Err(invalid());
        }
        for n in [1, 2, 4, 8, 16, 32, 64] {
            if 4 * n * (2 * n + 1) + 2 > p.max_vertices || 16 * n * n + 8 * n > p.max_triangles {
                return Err(invalid());
            }
            if let Ok(mesh) = level(self, n, p) {
                return Ok(mesh);
            }
        }
        Err(invalid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_moments_and_quotients_enclose_exact_dyadic_solids() {
        // Compare exact binary64 endpoint fractions to signed rational answers,
        // independently of interval arithmetic and rounded division oracles.
        fn rational_contains(i: I, n: i128, d: i128) {
            assert!(i.valid());
            for (x, lower) in [(i.lo, true), (i.hi, false)] {
                assert!(x.is_normal());
                let b = x.to_bits();
                let mant = ((b & ((1_u64 << 52) - 1)) | (1_u64 << 52)) as i128;
                let exponent = ((b >> 52) & 0x7ff) as i32 - 1023 - 52;
                assert!((-100..0).contains(&exponent));
                let numerator = if x.is_sign_negative() { -mant } else { mant };
                let denominator = 1_i128 << (-exponent as u32);
                assert!(if lower {
                    numerator * d <= n * denominator
                } else {
                    numerator * d >= n * denominator
                });
            }
        }
        let (o, x, y, z) = ([0.; 3], [0.5, 0., 0.], [0., 0.25, 0.], [0., 0., 0.125]);
        let tetra =
            crate::TriangleSolid::from_triangles(&[[o, y, x], [o, x, z], [o, z, y], [x, y, z]])
                .unwrap();
        let (v, m) = grid_moment_flux(&tetra);
        rational_contains(v, 1, 384);
        for (k, den) in [8, 16, 32].into_iter().enumerate() {
            rational_contains(m[k], 1, 384 * den);
            rational_contains(m[k] / v, 1, den);
        }
        // Translated box: V=1/64, centroid=(-1/4,3/8,5/16).
        let p = [
            [-0.5, 0.25, 0.25],
            [0., 0.25, 0.25],
            [0., 0.5, 0.25],
            [-0.5, 0.5, 0.25],
            [-0.5, 0.25, 0.375],
            [0., 0.25, 0.375],
            [0., 0.5, 0.375],
            [-0.5, 0.5, 0.375],
        ];
        let faces = [
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let solid = crate::TriangleSolid::from_triangles(&faces.map(|t| t.map(|i| p[i]))).unwrap();
        let (v, m) = grid_moment_flux(&solid);
        rational_contains(v, 1, 64);
        for (k, (n, d)) in [(-1, 4), (3, 8), (5, 16)].into_iter().enumerate() {
            rational_contains(m[k], n, d * 64);
            rational_contains(m[k] / v, n, d);
        }
    }
    #[test]
    fn unsigned_area_encloses_exact_dyadic_box_by_integer_endpoint_comparison() {
        let p = [
            [0., 0., 0.],
            [0.5, 0., 0.],
            [0.5, 0.25, 0.],
            [0., 0.25, 0.],
            [0., 0., 0.125],
            [0.5, 0., 0.125],
            [0.5, 0.25, 0.125],
            [0., 0.25, 0.125],
        ];
        let faces = [
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let triangles = faces.map(|t| t.map(|i| p[i]));
        let solid = crate::TriangleSolid::from_triangles(&triangles).unwrap();
        let area = grid_surface_area(&solid).unwrap();
        assert!(area.valid() && area.lo > 0.);
        // Exact area 2*(1/8+1/16+1/32)=7/16. No rounded numerical oracle.
        for (v, lower) in [(area.lo, true), (area.hi, false)] {
            let bits = v.to_bits();
            let n = u128::from(bits & ((1_u64 << 52) - 1)) + (1_u128 << 52);
            let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023 - 52;
            assert!((-100..0).contains(&exponent));
            let d = 1_u128 << (-exponent as u32);
            assert!(if lower {
                n * 16 <= d * 7
            } else {
                n * 16 >= d * 7
            });
        }
    }
    #[test]
    fn signed_flux_encloses_exact_dyadic_simplex_volume_by_integer_bit_comparison() {
        let (o, x, y, z) = ([0.; 3], [0.5, 0., 0.], [0., 0.25, 0.], [0., 0., 0.125]);
        let tetra =
            crate::TriangleSolid::from_triangles(&[[o, y, x], [o, x, z], [o, z, y], [x, y, z]])
                .unwrap();
        let flux = grid_volume_flux(&tetra);
        assert!(flux.valid() && flux.lo > 0.);
        // Exact volume is 1/384. Compare exact binary64 endpoint fractions,
        // without using interval arithmetic or a rounded 1/384 as the oracle.
        let rational = |v: f64| {
            let bits = v.to_bits();
            let numerator = (u128::from(bits & ((1_u64 << 52) - 1))) + (1_u128 << 52);
            let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023 - 52;
            assert!((-100..0).contains(&exponent));
            (numerator, 1_u128 << (-exponent as u32))
        };
        let (n, d) = rational(flux.lo);
        assert!(n * 384 <= d);
        let (n, d) = rational(flux.hi);
        assert!(n * 384 >= d);
    }
    #[test]
    fn grid_conversion_rejects_unbound_corner_correspondence_and_collapsed_geometry() {
        let r = CircularFilletReference::new(
            Length::from_meters(0.01),
            Length::from_meters(0.02),
            Length::from_meters(0.0005),
            Length::from_meters(0.0015),
        )
        .unwrap();
        let mesh = r
            .closed_reference_mesh(CircularFilletMeshPrecision {
                maximum_position_error: Length::from_meters(4e-4),
                maximum_normal_error: 0.8,
                max_vertices: 33026,
                max_triangles: 66048,
            })
            .unwrap();
        let mut points: Vec<_> = mesh
            .triangles
            .iter()
            .map(|t| t.map(|i| mesh.vertices[i as usize]))
            .collect();
        points[0].rotate_left(1); // Same valid facet, incorrect ordered correspondence.
        let solid = crate::TriangleSolid::from_triangles(&points).unwrap();
        assert!(grid_changes(&mesh, &solid).is_err());
        let mut collapsed = mesh;
        collapsed.vertices.fill([0.; 3]);
        assert!(collapsed
            .grid_solid(CircularFilletGridPrecision {
                maximum_position_error: Length::from_meters(1e-3),
                maximum_normal_error: 0.9
            })
            .is_err());
    }
    #[test]
    fn internal_missing_reversed_or_group_malformed_shells_refuse() {
        assert!(facet([[0.; 3]; 3]).is_err());
        let r = CircularFilletReference::new(
            Length::from_meters(0.01),
            Length::from_meters(0.02),
            Length::from_meters(0.0005),
            Length::from_meters(0.0015),
        )
        .unwrap();
        let p = CircularFilletMeshPrecision {
            maximum_position_error: Length::from_meters(4e-4),
            maximum_normal_error: 0.8,
            max_vertices: 33026,
            max_triangles: 66048,
        };
        let m = r.closed_reference_mesh(p).unwrap();
        let mut bad = m.clone();
        bad.triangles.pop();
        assert!(bad.validate_and_measure().is_err());
        let mut bad = m.clone();
        bad.triangles[0].swap(0, 1);
        assert!(bad.validate_and_measure().is_err());
        let mut bad = m;
        bad.groups.pop();
        assert!(bad.validate_and_measure().is_err());
    }
}
