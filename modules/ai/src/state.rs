//! In-memory design state for agent operations (no file I/O).

use std::collections::BTreeMap;

use opencad_assembly::AssemblyModel;
use opencad_drawing::DrawingModel;
use opencad_feature::FeatureNode;
use opencad_geometry::TopoRef;
use opencad_graph::{build_summary, diff_param_graphs, diff_semantic_refs, DesignDiff, ParamGraph};

/// Serializable design intent used by in-memory agent operations.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignState {
    pub parameters: ParamGraph,
    pub feature_nodes: Vec<FeatureNode>,
    pub semantic_refs: Vec<TopoRef>,
    pub assembly: Option<AssemblyModel>,
    pub drawing: Option<DrawingModel>,
}

impl DesignState {
    pub fn new(parameters: ParamGraph, feature_nodes: Vec<FeatureNode>) -> Self {
        Self {
            parameters,
            feature_nodes,
            semantic_refs: Vec::new(),
            assembly: None,
            drawing: None,
        }
    }

    pub fn with_semantic_refs(
        parameters: ParamGraph,
        feature_nodes: Vec<FeatureNode>,
        semantic_refs: Vec<TopoRef>,
    ) -> Self {
        Self {
            parameters,
            feature_nodes,
            semantic_refs,
            assembly: None,
            drawing: None,
        }
    }

    pub fn with_assembly(
        parameters: ParamGraph,
        feature_nodes: Vec<FeatureNode>,
        semantic_refs: Vec<TopoRef>,
        assembly: Option<AssemblyModel>,
    ) -> Self {
        Self {
            parameters,
            feature_nodes,
            semantic_refs,
            assembly,
            drawing: None,
        }
    }

    pub fn with_models(
        parameters: ParamGraph,
        feature_nodes: Vec<FeatureNode>,
        semantic_refs: Vec<TopoRef>,
        assembly: Option<AssemblyModel>,
        drawing: Option<DrawingModel>,
    ) -> Self {
        Self {
            parameters,
            feature_nodes,
            semantic_refs,
            assembly,
            drawing,
        }
    }
}

/// Compare two design states and return a semantic diff.
pub fn diff_design_state(before: &DesignState, after: &DesignState) -> DesignDiff {
    let mut changes = diff_param_graphs(&before.parameters, &after.parameters);
    changes.extend(diff_feature_nodes(
        &before.feature_nodes,
        &after.feature_nodes,
    ));
    changes.extend(diff_semantic_refs(
        &before.semantic_refs,
        &after.semantic_refs,
    ));
    if let (Some(before_assembly), Some(after_assembly)) = (&before.assembly, &after.assembly) {
        let assembly_diff = crate::assembly::diff_assembly_models(before_assembly, after_assembly);
        changes.extend(assembly_diff.changes);
    }
    if let (Some(before_drawing), Some(after_drawing)) = (&before.drawing, &after.drawing) {
        changes.extend(crate::drawing::diff_drawing_models(before_drawing, after_drawing).changes);
    }
    DesignDiff::semantic(build_summary(&changes), changes)
}

fn diff_feature_nodes(
    before: &[FeatureNode],
    after: &[FeatureNode],
) -> Vec<opencad_graph::SemanticChange> {
    use opencad_graph::SemanticChange;

    let before_map: BTreeMap<String, &FeatureNode> =
        before.iter().map(|node| (node.id.clone(), node)).collect();
    let after_map: BTreeMap<String, &FeatureNode> =
        after.iter().map(|node| (node.id.clone(), node)).collect();

    let mut ids = BTreeMap::new();
    for id in before_map.keys() {
        ids.insert(id.clone(), ());
    }
    for id in after_map.keys() {
        ids.insert(id.clone(), ());
    }

    let mut changes = Vec::new();
    for id in ids.keys() {
        match (before_map.get(id), after_map.get(id)) {
            (Some(_), None) => changes.push(SemanticChange::FeatureRemoved { id: id.clone() }),
            (None, Some(after_node)) => changes.push(SemanticChange::FeatureAdded {
                id: id.clone(),
                feature_type: after_node.definition.feature_type().to_string(),
            }),
            (Some(before_node), Some(after_node)) if before_node != after_node => {
                changes.extend(diff_feature_node(before_node, after_node));
            }
            _ => {}
        }
    }
    changes
}

fn diff_feature_node(
    before: &FeatureNode,
    after: &FeatureNode,
) -> Vec<opencad_graph::SemanticChange> {
    use opencad_graph::SemanticChange;

    let mut changes = Vec::new();
    if before.name != after.name {
        changes.push(SemanticChange::FeatureModified {
            id: before.id.clone(),
            field: "name".into(),
            before: before.name.clone(),
            after: after.name.clone(),
        });
    }
    if before.suppressed != after.suppressed {
        changes.push(SemanticChange::FeatureModified {
            id: before.id.clone(),
            field: "suppressed".into(),
            before: before.suppressed.to_string(),
            after: after.suppressed.to_string(),
        });
    }
    if before.definition != after.definition {
        changes.push(SemanticChange::FeatureModified {
            id: before.id.clone(),
            field: "definition".into(),
            before: serde_json::to_string(&before.definition).unwrap_or_default(),
            after: serde_json::to_string(&after.definition).unwrap_or_default(),
        });
    }
    changes
}
