//! Triangle-to-face grouping for solid picking.

use opencad_core::Result;

use crate::mesh::RenderMesh;
use crate::scene::BoundingBox;

const NORMAL_AXIS_THRESHOLD: f32 = 0.85;
const PLANE_QUANTUM_M: f32 = 0.0005;
const CYLINDER_XY_QUANTUM_M: f32 = 0.001;

/// Semantic bucket for a coplanar or cylindrical face group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FaceRole {
    Top,
    Bottom,
    PosX,
    NegX,
    PosY,
    NegY,
    Cylindrical,
    Other,
}

impl FaceRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::PosX => "+x",
            Self::NegX => "-x",
            Self::PosY => "+y",
            Self::NegY => "-y",
            Self::Cylindrical => "cylindrical",
            Self::Other => "other",
        }
    }
}

/// A group of mesh triangles that share orientation and approximate position.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceGroup {
    pub index: usize,
    /// Zero-based scene mesh containing this face group.
    pub mesh_index: usize,
    pub kernel_face_id: Option<u64>,
    pub role: FaceRole,
    pub normal: [f32; 3],
    pub centroid: [f32; 3],
    pub triangle_count: usize,
}

/// Maps triangle indices to semantic face groups in a tessellated body.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceCatalog {
    pub groups: Vec<FaceGroup>,
    triangle_group: Vec<usize>,
}

impl FaceCatalog {
    pub fn from_meshes(meshes: &[RenderMesh], bounds: &BoundingBox) -> Result<Self> {
        let mut groups = Vec::new();
        let mut triangle_group = Vec::new();
        for (mesh_index, mesh) in meshes.iter().enumerate() {
            let use_kernel_faces = mesh.has_triangle_face_ids();
            for (mesh_triangle_index, triangle) in mesh.indices.chunks_exact(3).enumerate() {
                let positions = [
                    mesh.positions[triangle[0] as usize],
                    mesh.positions[triangle[1] as usize],
                    mesh.positions[triangle[2] as usize],
                ];
                let normal = triangle_normal(positions[0], positions[1], positions[2]);
                let centroid = triangle_centroid(positions);
                let role = classify_normal(normal, bounds);
                let kernel_face_id = if use_kernel_faces {
                    Some(mesh.triangle_face_ids[mesh_triangle_index])
                } else {
                    None
                };
                let group_index = if let Some(kernel_face_id) = kernel_face_id {
                    group_index_for_kernel_face(
                        &mut groups,
                        mesh_index,
                        kernel_face_id,
                        role,
                        normal,
                        centroid,
                    )
                } else {
                    group_index_for(&mut groups, mesh_index, role, normal, centroid)
                };
                triangle_group.push(group_index);
            }
        }

        Ok(Self {
            groups,
            triangle_group,
        })
    }

    pub fn triangle_count(&self) -> usize {
        self.triangle_group.len()
    }

    pub fn group_at(&self, triangle_index: usize) -> Option<&FaceGroup> {
        let group_index = *self.triangle_group.get(triangle_index)?;
        self.groups.get(group_index)
    }

    /// Triangle indices that belong to a face group.
    pub fn triangle_indices_in_group(&self, group_index: usize) -> Vec<usize> {
        self.triangle_group
            .iter()
            .enumerate()
            .filter_map(|(triangle_index, mapped_group)| {
                (*mapped_group == group_index).then_some(triangle_index)
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GroupKey {
    role: FaceRole,
    bucket_a: i32,
    bucket_b: i32,
}

fn group_index_for_kernel_face(
    groups: &mut Vec<FaceGroup>,
    mesh_index: usize,
    kernel_face_id: u64,
    role: FaceRole,
    normal: [f32; 3],
    centroid: [f32; 3],
) -> usize {
    if let Some(index) = groups.iter().position(|group| {
        group.mesh_index == mesh_index && group.kernel_face_id == Some(kernel_face_id)
    }) {
        groups[index].triangle_count += 1;
        return index;
    }

    let index = groups.len();
    groups.push(FaceGroup {
        index,
        mesh_index,
        kernel_face_id: Some(kernel_face_id),
        role,
        normal,
        centroid,
        triangle_count: 1,
    });
    index
}

fn group_index_for(
    groups: &mut Vec<FaceGroup>,
    mesh_index: usize,
    role: FaceRole,
    normal: [f32; 3],
    centroid: [f32; 3],
) -> usize {
    let key = group_key(role, normal, centroid);
    if let Some(index) = groups.iter().position(|group| {
        group.mesh_index == mesh_index && group_key(group.role, group.normal, group.centroid) == key
    }) {
        groups[index].triangle_count += 1;
        return index;
    }

    let index = groups.len();
    groups.push(FaceGroup {
        index,
        mesh_index,
        kernel_face_id: None,
        role,
        normal,
        centroid,
        triangle_count: 1,
    });
    index
}

fn group_key(role: FaceRole, _normal: [f32; 3], centroid: [f32; 3]) -> GroupKey {
    match role {
        FaceRole::Top | FaceRole::Bottom => GroupKey {
            role,
            bucket_a: quantize(centroid[2], PLANE_QUANTUM_M),
            bucket_b: 0,
        },
        FaceRole::PosX | FaceRole::NegX => GroupKey {
            role,
            bucket_a: quantize(centroid[0], PLANE_QUANTUM_M),
            bucket_b: 0,
        },
        FaceRole::PosY | FaceRole::NegY => GroupKey {
            role,
            bucket_a: quantize(centroid[1], PLANE_QUANTUM_M),
            bucket_b: 0,
        },
        FaceRole::Cylindrical => GroupKey {
            role,
            bucket_a: quantize(centroid[0], CYLINDER_XY_QUANTUM_M),
            bucket_b: quantize(centroid[1], CYLINDER_XY_QUANTUM_M),
        },
        FaceRole::Other => GroupKey {
            role,
            bucket_a: quantize(centroid[0], PLANE_QUANTUM_M),
            bucket_b: quantize(centroid[1], PLANE_QUANTUM_M),
        },
    }
}

fn classify_normal(normal: [f32; 3], bounds: &BoundingBox) -> FaceRole {
    let n = normalize(normal);
    if n[2].abs() >= NORMAL_AXIS_THRESHOLD {
        if n[2] > 0.0 {
            FaceRole::Top
        } else {
            FaceRole::Bottom
        }
    } else if n[0].abs() >= NORMAL_AXIS_THRESHOLD {
        if n[0] > 0.0 {
            FaceRole::PosX
        } else {
            FaceRole::NegX
        }
    } else if n[1].abs() >= NORMAL_AXIS_THRESHOLD {
        if n[1] > 0.0 {
            FaceRole::PosY
        } else {
            FaceRole::NegY
        }
    } else if bounds.extent()[2] > f32::EPSILON {
        FaceRole::Cylindrical
    } else {
        FaceRole::Other
    }
}

fn triangle_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let ux = b[0] - a[0];
    let uy = b[1] - a[1];
    let uz = b[2] - a[2];
    let vx = c[0] - a[0];
    let vy = c[1] - a[1];
    let vz = c[2] - a[2];
    normalize([uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx])
}

fn triangle_centroid(points: [[f32; 3]; 3]) -> [f32; 3] {
    [
        (points[0][0] + points[1][0] + points[2][0]) / 3.0,
        (points[0][1] + points[1][1] + points[2][1]) / 3.0,
        (points[0][2] + points[1][2] + points[2][2]) / 3.0,
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len <= f32::EPSILON {
        return [0.0, 0.0, 1.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

fn quantize(value: f32, step: f32) -> i32 {
    (value / step).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::RenderMesh;
    use opencad_geometry::MeshSet;

    #[test]
    fn catalog_groups_planar_triangles_by_role() {
        let mesh = RenderMesh::from_mesh_set(&MeshSet::box_prism(0.08, 0.001));
        let bounds = BoundingBox::from_positions(&mesh.positions).expect("bounds");
        let catalog =
            FaceCatalog::from_meshes(std::slice::from_ref(&mesh), &bounds).expect("catalog");
        assert_eq!(catalog.triangle_count(), mesh.triangle_count());
        assert!(catalog
            .groups
            .iter()
            .any(|group| group.role == FaceRole::Top));
        assert!(catalog
            .groups
            .iter()
            .any(|group| group.role == FaceRole::Bottom));
        assert!(catalog.group_at(0).is_some());
        let top_group = catalog
            .groups
            .iter()
            .find(|group| group.role == FaceRole::Top)
            .expect("top");
        let top_triangles = catalog.triangle_indices_in_group(top_group.index);
        assert!(top_triangles.len() >= 2);
    }

    #[test]
    fn catalog_groups_by_kernel_face_id_when_present() {
        let mesh = RenderMesh::from_mesh_set(&MeshSet::box_prism(0.08, 0.001));
        let bounds = BoundingBox::from_positions(&mesh.positions).expect("bounds");
        let catalog =
            FaceCatalog::from_meshes(std::slice::from_ref(&mesh), &bounds).expect("catalog");
        assert_eq!(catalog.groups.len(), 2);
        assert!(catalog
            .groups
            .iter()
            .all(|group| group.kernel_face_id.is_some()));
        assert_eq!(
            catalog.group_at(0).and_then(|group| group.kernel_face_id),
            Some(1)
        );
        assert_eq!(
            catalog.group_at(2).and_then(|group| group.kernel_face_id),
            Some(2)
        );
    }

    #[test]
    fn catalog_keeps_kernel_faces_separate_across_meshes() {
        let first = RenderMesh::from_mesh_set(&MeshSet::box_prism(0.08, 0.001));
        let mut second = first.clone();
        for position in &mut second.positions {
            position[0] += 0.1;
        }
        let mut bounds = BoundingBox::from_positions(&first.positions).expect("first bounds");
        bounds.merge(&BoundingBox::from_positions(&second.positions).expect("second bounds"));
        let catalog = FaceCatalog::from_meshes(&[first, second], &bounds).expect("catalog");
        assert_eq!(catalog.triangle_count(), 8);
        assert_eq!(catalog.groups.len(), 4);
        assert_eq!(
            catalog
                .groups
                .iter()
                .filter(|group| group.mesh_index == 1)
                .count(),
            2
        );
    }
}
