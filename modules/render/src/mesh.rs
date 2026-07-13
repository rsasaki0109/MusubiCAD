//! GPU-ready mesh buffers derived from tessellated geometry.

use opencad_geometry::MeshSet;

/// Triangle mesh in render-friendly layout.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub triangle_face_ids: Vec<u64>,
    pub base_color: [f32; 4],
}

impl RenderMesh {
    pub fn from_mesh_set(mesh: &MeshSet) -> Self {
        Self::from_mesh_set_with_color(mesh, [0.22, 0.55, 0.86, 1.0])
    }

    pub fn from_mesh_set_with_color(mesh: &MeshSet, base_color: [f32; 4]) -> Self {
        Self {
            positions: mesh.positions.clone(),
            normals: mesh.normals.clone(),
            indices: mesh.indices.clone(),
            triangle_face_ids: mesh.triangle_face_ids.clone(),
            base_color,
        }
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn has_triangle_face_ids(&self) -> bool {
        self.triangle_face_ids.len() == self.triangle_count()
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_geometry::MeshSet;

    #[test]
    fn converts_mesh_set_to_render_mesh() {
        let source = MeshSet::box_prism(0.01, 0.001);
        let mesh = RenderMesh::from_mesh_set(&source);
        assert_eq!(mesh.triangle_count(), source.triangle_count());
        assert_eq!(mesh.vertex_count(), source.positions.len());
    }
}
