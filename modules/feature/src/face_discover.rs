//! Build semantic face discoveries from tessellated bodies during regeneration.

use std::collections::BTreeMap;

use opencad_geometry::{FaceRefDiscovery, GeometryKernel, KernelBody, MeshSet, TessellationSettings};

use crate::feature::FeatureNode;

pub fn discover_face_refs_from_body<K: GeometryKernel>(
    kernel: &K,
    body: &KernelBody,
    feature_nodes: &[FeatureNode],
) -> opencad_core::Result<Vec<FaceRefDiscovery>> {
    let mesh = kernel.tessellate(body, &TessellationSettings::default())?;
    Ok(discover_face_refs_from_mesh(&mesh, feature_nodes))
}

pub fn discover_face_refs_from_mesh(
    mesh: &MeshSet,
    feature_nodes: &[FeatureNode],
) -> Vec<FaceRefDiscovery> {
    let mut groups: BTreeMap<GroupKey, GroupAccum> = BTreeMap::new();

    for triangle_index in 0..mesh.triangle_count() {
        let base = triangle_index * 3;
        let positions = [
            mesh.positions[mesh.indices[base] as usize],
            mesh.positions[mesh.indices[base + 1] as usize],
            mesh.positions[mesh.indices[base + 2] as usize],
        ];
        let normal = triangle_normal(positions[0], positions[1], positions[2]);
        let centroid = triangle_centroid(positions);
        let role = role_from_normal(normal);
        let kernel_face_id = mesh
            .triangle_face_ids
            .get(triangle_index)
            .copied()
            .unwrap_or(0);
        let key = GroupKey {
            kernel_face_id,
            role: role.to_string(),
        };
        groups
            .entry(key)
            .or_insert_with(|| GroupAccum {
                normal_sum: [0.0; 3],
                centroid_sum: [0.0; 3],
                triangle_count: 0,
            })
            .add(normal, centroid);
    }

    groups
        .into_iter()
        .filter_map(|(key, accum)| {
            if accum.triangle_count == 0 {
                return None;
            }
            let count = accum.triangle_count as f32;
            let normal_m = [
                accum.normal_sum[0] / count,
                accum.normal_sum[1] / count,
                accum.normal_sum[2] / count,
            ];
            let centroid_m = [
                accum.centroid_sum[0] / count,
                accum.centroid_sum[1] / count,
                accum.centroid_sum[2] / count,
            ];
            let feature_id = infer_feature_id(feature_nodes, &key.role);
            Some(FaceRefDiscovery {
                kernel_face_id: key.kernel_face_id,
                role: key.role,
                normal_m,
                centroid_m,
                feature_id,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct GroupKey {
    kernel_face_id: u64,
    role: String,
}

#[derive(Debug, Clone, Default)]
struct GroupAccum {
    normal_sum: [f32; 3],
    centroid_sum: [f32; 3],
    triangle_count: u32,
}

impl GroupAccum {
    fn add(&mut self, normal: [f32; 3], centroid: [f32; 3]) {
        self.normal_sum[0] += normal[0];
        self.normal_sum[1] += normal[1];
        self.normal_sum[2] += normal[2];
        self.centroid_sum[0] += centroid[0];
        self.centroid_sum[1] += centroid[1];
        self.centroid_sum[2] += centroid[2];
        self.triangle_count += 1;
    }
}

fn triangle_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let normal = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if len < 1e-9 {
        return [0.0, 0.0, 1.0];
    }
    [normal[0] / len, normal[1] / len, normal[2] / len]
}

fn triangle_centroid(positions: [[f32; 3]; 3]) -> [f32; 3] {
    [
        (positions[0][0] + positions[1][0] + positions[2][0]) / 3.0,
        (positions[0][1] + positions[1][1] + positions[2][1]) / 3.0,
        (positions[0][2] + positions[1][2] + positions[2][2]) / 3.0,
    ]
}

fn role_from_normal(normal: [f32; 3]) -> &'static str {
    let abs = [normal[0].abs(), normal[1].abs(), normal[2].abs()];
    if abs[2] >= abs[0] && abs[2] >= abs[1] {
        if normal[2] >= 0.0 {
            "top"
        } else {
            "bottom"
        }
    } else if abs[0] >= abs[1] {
        if normal[0] >= 0.0 {
            "+x"
        } else {
            "-x"
        }
    } else if normal[1] >= 0.0 {
        "+y"
    } else {
        "-y"
    }
}

fn infer_feature_id(feature_nodes: &[FeatureNode], role: &str) -> Option<String> {
    let feature_type = match role {
        "cylindrical" => "hole",
        "top" => return find_feature_type(feature_nodes, &["fillet", "chamfer", "extrude"]),
        "bottom" | "+x" | "-x" | "+y" | "-y" => "extrude",
        _ => return None,
    };
    find_feature_type(feature_nodes, &[feature_type])
}

fn find_feature_type(feature_nodes: &[FeatureNode], feature_types: &[&str]) -> Option<String> {
    for feature_type in feature_types {
        if let Some(node) = feature_nodes
            .iter()
            .find(|node| node.definition.feature_type() == *feature_type)
        {
            return Some(node.id.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_geometry::MeshSet;

    #[test]
    fn discovers_top_face_from_box_mesh() {
        let mesh = MeshSet::box_prism(0.08, 0.001);
        let discoveries = discover_face_refs_from_mesh(&mesh, &[]);
        assert!(discoveries.iter().any(|discovery| discovery.role == "top"));
    }
}
