//! In-memory `.ocad` document model.

use opencad_assembly::AssemblyModel;
use opencad_core::DocumentMetadata;
use opencad_drawing::DrawingModel;
use opencad_feature::{FeatureNode, PartModel};
use opencad_geometry::TopoRef;
use opencad_graph::{FeatureGraph, ParamGraph};
use opencad_sketch::Sketch;
use serde::{Deserialize, Serialize};

/// Serializable design document (source of truth, no B-Rep cache).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcadDocument {
    pub metadata: DocumentMetadata,
    pub parameters: ParamGraph,
    pub sketches: Vec<Sketch>,
    pub feature_graph: FeatureGraph,
    pub feature_nodes: Vec<FeatureNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_refs: Vec<TopoRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<AssemblyModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawing: Option<DrawingModel>,
}

impl OcadDocument {
    pub fn new(metadata: DocumentMetadata) -> Self {
        Self {
            metadata,
            parameters: ParamGraph::new(),
            sketches: Vec::new(),
            feature_graph: FeatureGraph::new(),
            feature_nodes: Vec::new(),
            semantic_refs: Vec::new(),
            assembly: None,
            drawing: None,
        }
    }

    pub fn from_part_model(metadata: DocumentMetadata, part: &PartModel) -> Self {
        let mut sketches: Vec<Sketch> = part.sketches.values().cloned().collect();
        sketches.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

        let mut feature_nodes: Vec<FeatureNode> = part.nodes.values().cloned().collect();
        feature_nodes.sort_by(|a, b| a.id.cmp(&b.id));

        Self {
            metadata,
            parameters: ParamGraph::new(),
            sketches,
            feature_graph: part.graph.clone(),
            feature_nodes,
            semantic_refs: Vec::new(),
            assembly: None,
            drawing: None,
        }
    }

    pub fn from_drawing_model(metadata: DocumentMetadata, drawing: DrawingModel) -> Self {
        Self {
            metadata,
            parameters: ParamGraph::new(),
            sketches: Vec::new(),
            feature_graph: FeatureGraph::new(),
            feature_nodes: Vec::new(),
            semantic_refs: Vec::new(),
            assembly: None,
            drawing: Some(drawing),
        }
    }

    pub fn into_part_model(self) -> PartModel {
        let mut model = PartModel::new();
        model.graph = self.feature_graph;
        for sketch in self.sketches {
            model
                .sketches
                .insert(sketch.id.as_str().to_string(), sketch);
        }
        for node in self.feature_nodes {
            model.nodes.insert(node.id.clone(), node);
        }
        model
    }
}
