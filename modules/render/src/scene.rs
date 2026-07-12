//! Render scene graph built from tessellated bodies.

use opencad_core::Result;
use opencad_geometry::MeshSet;

use crate::camera::OrbitCamera;
use crate::face_catalog::FaceCatalog;
use crate::mesh::RenderMesh;

/// Axis-aligned bounding box in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl BoundingBox {
    pub fn empty() -> Self {
        Self {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
        }
    }

    pub fn from_positions(positions: &[[f32; 3]]) -> Result<Self> {
        if positions.is_empty() {
            return Err(opencad_core::OpenCadError::validation(
                "cannot compute bounds from empty mesh",
            ));
        }
        let mut bounds = Self::empty();
        for position in positions {
            bounds.include(*position);
        }
        Ok(bounds)
    }

    pub fn include(&mut self, point: [f32; 3]) {
        for (axis, value) in point.into_iter().enumerate() {
            self.min[axis] = self.min[axis].min(value);
            self.max[axis] = self.max[axis].max(value);
        }
    }

    pub fn merge(&mut self, other: &Self) {
        self.include(other.min);
        self.include(other.max);
    }

    pub fn center(&self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    pub fn extent(&self) -> [f32; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }

    pub fn diagonal(&self) -> f32 {
        let extent = self.extent();
        (extent[0] * extent[0] + extent[1] * extent[1] + extent[2] * extent[2]).sqrt()
    }
}

/// Scene ready for viewport upload.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderScene {
    pub meshes: Vec<RenderMesh>,
    pub bounds: BoundingBox,
    pub face_catalog: FaceCatalog,
}

impl RenderScene {
    pub fn from_mesh_sets(mesh_sets: &[MeshSet]) -> Result<Self> {
        Self::from_mesh_sets_with_colors(mesh_sets, None)
    }

    pub fn from_mesh_sets_with_colors(
        mesh_sets: &[MeshSet],
        colors: Option<&[[f32; 4]]>,
    ) -> Result<Self> {
        if mesh_sets.is_empty() {
            return Err(opencad_core::OpenCadError::validation(
                "scene requires at least one mesh",
            ));
        }

        let mut meshes = Vec::with_capacity(mesh_sets.len());
        let mut bounds = BoundingBox::empty();
        for (index, mesh_set) in mesh_sets.iter().enumerate() {
            let color = colors
                .and_then(|palette| palette.get(index).copied())
                .unwrap_or([0.72, 0.76, 0.82, 1.0]);
            let mesh = RenderMesh::from_mesh_set_with_color(mesh_set, color);
            let mesh_bounds = BoundingBox::from_positions(&mesh.positions)?;
            bounds.merge(&mesh_bounds);
            meshes.push(mesh);
        }

        let face_catalog = FaceCatalog::from_meshes(&meshes, &bounds)?;

        Ok(Self {
            meshes,
            bounds,
            face_catalog,
        })
    }

    pub fn from_mesh_set(mesh_set: &MeshSet) -> Result<Self> {
        Self::from_mesh_sets(std::slice::from_ref(mesh_set))
    }

    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(RenderMesh::triangle_count).sum()
    }

    pub fn face_group_at(&self, triangle_index: usize) -> Option<&crate::face_catalog::FaceGroup> {
        self.face_catalog.group_at(triangle_index)
    }

    pub fn default_camera(&self, aspect: f32) -> OrbitCamera {
        OrbitCamera::fit_bounds(&self.bounds, aspect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_geometry::MeshSet;

    #[test]
    fn computes_bounds_from_positions() {
        let bounds =
            BoundingBox::from_positions(&[[0.0, 0.0, 0.0], [0.08, 0.06, 0.006]]).expect("bounds");
        assert!((bounds.min[0] - 0.0).abs() < 1e-6);
        assert!((bounds.max[0] - 0.08).abs() < 1e-6);
        assert!((bounds.center()[1] - 0.03).abs() < 1e-6);
    }

    #[test]
    fn builds_scene_from_mesh_set() {
        let scene = RenderScene::from_mesh_set(&MeshSet::box_prism(0.01, 0.001)).expect("scene");
        assert_eq!(scene.meshes.len(), 1);
        assert!(scene.triangle_count() > 0);
        assert!(scene.bounds.diagonal() > 0.0);
    }
}
