//! Build semantic edge discoveries from kernel topology during regeneration.

use opencad_geometry::{EdgeRefDiscovery, GeometryKernel, KernelBody};

use crate::feature::FeatureNode;

pub fn discover_edge_refs_from_body<K: GeometryKernel>(
    kernel: &K,
    body: &KernelBody,
    feature_nodes: &[FeatureNode],
) -> opencad_core::Result<Vec<EdgeRefDiscovery>> {
    let mut discoveries = kernel.discover_body_edges(body)?;
    for discovery in &mut discoveries {
        if discovery.feature_id.is_none() {
            discovery.feature_id = infer_feature_id(feature_nodes, &discovery.role);
        }
    }
    Ok(discoveries)
}

fn infer_feature_id(feature_nodes: &[FeatureNode], role: &str) -> Option<String> {
    if !role.starts_with("top") {
        return None;
    }
    find_feature_type(feature_nodes, &["hole", "extrude"])
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
