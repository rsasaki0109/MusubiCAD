//! In-memory `.ocad` document model.

use std::collections::BTreeMap;

use opencad_assembly::AssemblyModel;
use opencad_core::{Assertion, DocumentMetadata};
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assertions: Vec<Assertion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<AssemblyModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawing: Option<DrawingModel>,
    /// Opaque attachment files by path under `imports/`, such as STEP files
    /// used by imported solids (ADR-016).  Stored as files, checksummed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attachments: BTreeMap<String, Vec<u8>>,
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
            assertions: Vec::new(),
            assembly: None,
            drawing: None,
            attachments: BTreeMap::new(),
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
            assertions: Vec::new(),
            assembly: None,
            drawing: None,
            attachments: part.attachments.clone(),
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
            assertions: Vec::new(),
            assembly: None,
            drawing: Some(drawing),
            attachments: BTreeMap::new(),
        }
    }

    /// Query parameters over this document's Design Graph (no scene).
    pub fn into_query_params(self, query: opencad_ai::DesignQuery) -> opencad_ai::QueryParams {
        opencad_ai::QueryParams {
            parameters: self.parameters,
            feature_nodes: self.feature_nodes,
            feature_graph: Some(self.feature_graph),
            sketches: self.sketches,
            scene: None,
            semantic_refs: self.semantic_refs,
            assembly: self.assembly,
            drawing: self.drawing,
            assertions: self.assertions,
            query,
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
        model.attachments = self.attachments;
        model
    }
}
