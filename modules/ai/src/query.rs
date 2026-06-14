//! Design graph queries (Task-148+).

use indexmap::IndexMap;
use opencad_core::{OpenCadError, Result};
use opencad_feature::{FeatureDefinition, FeatureNode};
use opencad_geometry::TopoRef;
use opencad_graph::{evaluate_param_graph, DependencyEdge, FeatureGraph, ParamGraph};
use opencad_sketch::{Constraint, Sketch, SketchEntity};
use serde::{Deserialize, Serialize};

/// Query target sent by agents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DesignQuery {
    ListParameters,
    GetParameter { id: String },
    ListFeatures,
    GetFeature { id: String },
    FeatureOrder,
    ListSketches,
    GetSketch { id: String },
    ListSketchConstraints { sketch_id: String },
    ListSketchEntities { sketch_id: String },
    FeatureDependencies,
    GetFeatureDependencies { id: String },
    ParameterDependencies,
    GetParameterDependencies { id: String },
    ListOverlayLines,
    ListFaceGroups,
    ListSemanticRefs,
    GetSemanticRef { ref_id: String },
}

/// In-memory query parameters.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct QueryParams {
    pub parameters: ParamGraph,
    pub feature_nodes: Vec<FeatureNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_graph: Option<FeatureGraph>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sketches: Vec<Sketch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<SceneQueryContext>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_refs: Vec<TopoRef>,
    pub query: DesignQuery,
}

/// Tessellated viewport data required by scene listing queries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneQueryContext {
    pub overlay_lines: Vec<OverlayLineInfo>,
    pub face_groups: Vec<FaceGroupInfo>,
}

/// Pickable sketch overlay segment exposed to agents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverlayLineInfo {
    pub line_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sketch_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment_index: Option<usize>,
    pub construction: bool,
    pub start_m: [f32; 3],
    pub end_m: [f32; 3],
}

/// Semantic face group from tessellated solids with optional feature inference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceGroupInfo {
    pub face_group_index: usize,
    pub face_role: String,
    pub triangle_count: usize,
    pub face_normal_m: [f32; 3],
    pub face_centroid_m: [f32; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_face_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inferred_feature_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inferred_topo_ref_id: Option<String>,
}

/// Persisted semantic topology reference exposed to agents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticRefInfo {
    pub ref_id: String,
    pub kind: String,
    pub created_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_face_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterInfo {
    pub id: String,
    pub name: String,
    pub expr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_m: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureInfo {
    pub id: String,
    pub name: String,
    pub feature_type: String,
    pub suppressed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureDetail {
    #[serde(flatten)]
    pub info: FeatureInfo,
    pub definition: FeatureDefinition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchInfo {
    pub id: String,
    pub name: String,
    pub entity_count: usize,
    pub constraint_count: usize,
    pub profile_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchEntityInfo {
    pub id: String,
    pub kind: String,
    pub construction: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyNeighbors {
    pub id: String,
    pub upstream: Vec<String>,
    pub downstream: Vec<String>,
}

/// Query response payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueryResult {
    Parameters {
        items: Vec<ParameterInfo>,
    },
    Parameter {
        item: ParameterInfo,
    },
    Features {
        items: Vec<FeatureInfo>,
    },
    Feature {
        item: FeatureDetail,
    },
    FeatureOrder {
        order: Vec<String>,
    },
    Sketches {
        items: Vec<SketchInfo>,
    },
    Sketch {
        item: Sketch,
    },
    SketchConstraints {
        sketch_id: String,
        items: Vec<Constraint>,
    },
    SketchEntities {
        sketch_id: String,
        items: Vec<SketchEntityInfo>,
    },
    FeatureDependencies {
        edges: Vec<DependencyEdge>,
    },
    FeatureDependencyNeighbors {
        item: DependencyNeighbors,
    },
    ParameterDependencies {
        edges: Vec<DependencyEdge>,
    },
    ParameterDependencyNeighbors {
        item: DependencyNeighbors,
    },
    OverlayLines {
        items: Vec<OverlayLineInfo>,
    },
    FaceGroups {
        items: Vec<FaceGroupInfo>,
    },
    SemanticRefs {
        items: Vec<SemanticRefInfo>,
    },
    SemanticRef {
        item: SemanticRefInfo,
    },
}

pub fn query_needs_scene(query: &DesignQuery) -> bool {
    matches!(
        query,
        DesignQuery::ListOverlayLines | DesignQuery::ListFaceGroups
    )
}

pub fn run_query(params: &QueryParams) -> Result<QueryResult> {
    let values = evaluate_param_graph(&params.parameters).ok();

    match &params.query {
        DesignQuery::ListParameters => Ok(QueryResult::Parameters {
            items: list_parameters(&params.parameters, values.as_ref()),
        }),
        DesignQuery::GetParameter { id } => {
            let item = parameter_info(&params.parameters, id, values.as_ref())?;
            Ok(QueryResult::Parameter { item })
        }
        DesignQuery::ListFeatures => Ok(QueryResult::Features {
            items: list_features(&params.feature_nodes),
        }),
        DesignQuery::GetFeature { id } => {
            let item = feature_detail(&params.feature_nodes, id)?;
            Ok(QueryResult::Feature { item })
        }
        DesignQuery::FeatureOrder => Ok(QueryResult::FeatureOrder {
            order: feature_order(&params.feature_graph, &params.feature_nodes)?,
        }),
        DesignQuery::ListSketches => Ok(QueryResult::Sketches {
            items: list_sketches(&params.sketches),
        }),
        DesignQuery::GetSketch { id } => {
            let item = get_sketch(&params.sketches, id)?;
            Ok(QueryResult::Sketch { item })
        }
        DesignQuery::ListSketchConstraints { sketch_id } => {
            let items = list_sketch_constraints(&params.sketches, sketch_id)?;
            Ok(QueryResult::SketchConstraints {
                sketch_id: sketch_id.clone(),
                items,
            })
        }
        DesignQuery::ListSketchEntities { sketch_id } => {
            let items = list_sketch_entities(&params.sketches, sketch_id)?;
            Ok(QueryResult::SketchEntities {
                sketch_id: sketch_id.clone(),
                items,
            })
        }
        DesignQuery::FeatureDependencies => Ok(QueryResult::FeatureDependencies {
            edges: feature_dependency_edges(&params.feature_graph)?,
        }),
        DesignQuery::GetFeatureDependencies { id } => Ok(QueryResult::FeatureDependencyNeighbors {
            item: feature_dependency_neighbors(&params.feature_graph, id)?,
        }),
        DesignQuery::ParameterDependencies => Ok(QueryResult::ParameterDependencies {
            edges: params.parameters.dependency_edges().to_vec(),
        }),
        DesignQuery::GetParameterDependencies { id } => {
            Ok(QueryResult::ParameterDependencyNeighbors {
                item: parameter_dependency_neighbors(&params.parameters, id)?,
            })
        }
        DesignQuery::ListOverlayLines => {
            let scene = scene_context(params)?;
            Ok(QueryResult::OverlayLines {
                items: scene.overlay_lines.clone(),
            })
        }
        DesignQuery::ListFaceGroups => {
            let scene = scene_context(params)?;
            Ok(QueryResult::FaceGroups {
                items: scene.face_groups.clone(),
            })
        }
        DesignQuery::ListSemanticRefs => Ok(QueryResult::SemanticRefs {
            items: list_semantic_refs(&params.semantic_refs),
        }),
        DesignQuery::GetSemanticRef { ref_id } => {
            let item = get_semantic_ref(&params.semantic_refs, ref_id)?;
            Ok(QueryResult::SemanticRef { item })
        }
    }
}

pub fn get_semantic_ref(semantic_refs: &[TopoRef], ref_id: &str) -> Result<SemanticRefInfo> {
    semantic_refs
        .iter()
        .find(|topo_ref| topo_ref.ref_id.as_str() == ref_id)
        .map(|topo_ref| SemanticRefInfo {
            ref_id: topo_ref.ref_id.as_str().to_string(),
            kind: match topo_ref.kind {
                opencad_geometry::TopoRefKind::Face => "face".into(),
                opencad_geometry::TopoRefKind::Edge => "edge".into(),
                opencad_geometry::TopoRefKind::Vertex => "vertex".into(),
            },
            created_by: topo_ref.semantic.created_by.clone(),
            role: topo_ref.semantic.role.clone(),
            kernel_face_id: topo_ref.kernel_face_id(),
            intent: topo_ref.semantic.intent.clone(),
        })
        .ok_or_else(|| OpenCadError::not_found(format!("semantic ref '{ref_id}'")))
}

pub fn list_semantic_refs(semantic_refs: &[TopoRef]) -> Vec<SemanticRefInfo> {
    let mut items: Vec<SemanticRefInfo> = semantic_refs
        .iter()
        .map(|topo_ref| SemanticRefInfo {
            ref_id: topo_ref.ref_id.as_str().to_string(),
            kind: match topo_ref.kind {
                opencad_geometry::TopoRefKind::Face => "face".into(),
                opencad_geometry::TopoRefKind::Edge => "edge".into(),
                opencad_geometry::TopoRefKind::Vertex => "vertex".into(),
            },
            created_by: topo_ref.semantic.created_by.clone(),
            role: topo_ref.semantic.role.clone(),
            kernel_face_id: topo_ref.kernel_face_id(),
            intent: topo_ref.semantic.intent.clone(),
        })
        .collect();
    items.sort_by(|left, right| left.ref_id.cmp(&right.ref_id));
    items
}

fn scene_context(params: &QueryParams) -> Result<&SceneQueryContext> {
    params.scene.as_ref().ok_or_else(|| {
        OpenCadError::validation(
            "scene tessellation is required; use opencad.query_document or provide scene context",
        )
    })
}

pub(crate) fn list_parameters(
    graph: &ParamGraph,
    values: Option<&IndexMap<String, f64>>,
) -> Vec<ParameterInfo> {
    let mut ids = graph.parameter_ids();
    ids.sort();
    ids.into_iter()
        .filter_map(|id| parameter_info(graph, &id, values).ok())
        .collect()
}

fn parameter_info(
    graph: &ParamGraph,
    id: &str,
    values: Option<&IndexMap<String, f64>>,
) -> Result<ParameterInfo> {
    let entry = graph
        .get(id)
        .ok_or_else(|| OpenCadError::not_found(format!("parameter '{id}'")))?;
    let value_m = values.and_then(|map| map.get(&entry.name).copied());
    Ok(ParameterInfo {
        id: entry.id.clone(),
        name: entry.name.clone(),
        expr: entry.expr.clone(),
        value_m,
    })
}

pub(crate) fn list_features(nodes: &[FeatureNode]) -> Vec<FeatureInfo> {
    let mut items: Vec<FeatureInfo> = nodes.iter().map(feature_info).collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));
    items
}

fn feature_info(node: &FeatureNode) -> FeatureInfo {
    FeatureInfo {
        id: node.id.clone(),
        name: node.name.clone(),
        feature_type: node.definition.feature_type().to_string(),
        suppressed: node.suppressed,
    }
}

fn feature_detail(nodes: &[FeatureNode], id: &str) -> Result<FeatureDetail> {
    let node = nodes
        .iter()
        .find(|node| node.id == id)
        .ok_or_else(|| OpenCadError::not_found(format!("feature '{id}'")))?;
    Ok(FeatureDetail {
        info: feature_info(node),
        definition: node.definition.clone(),
    })
}

pub(crate) fn feature_order(
    graph: &Option<FeatureGraph>,
    nodes: &[FeatureNode],
) -> Result<Vec<String>> {
    if let Some(graph) = graph {
        return graph.recompute_order();
    }
    let mut order: Vec<String> = nodes.iter().map(|node| node.id.clone()).collect();
    order.sort();
    Ok(order)
}

fn list_sketches(sketches: &[Sketch]) -> Vec<SketchInfo> {
    let mut items: Vec<SketchInfo> = sketches.iter().map(sketch_info).collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));
    items
}

fn sketch_info(sketch: &Sketch) -> SketchInfo {
    SketchInfo {
        id: sketch.id.as_str().to_string(),
        name: sketch.name.clone(),
        entity_count: sketch.entities.len(),
        constraint_count: sketch.constraints.len(),
        profile_count: sketch.profiles.len(),
    }
}

fn get_sketch(sketches: &[Sketch], id: &str) -> Result<Sketch> {
    sketches
        .iter()
        .find(|sketch| sketch.id.as_str() == id)
        .cloned()
        .ok_or_else(|| OpenCadError::not_found(format!("sketch '{id}'")))
}

fn list_sketch_constraints(sketches: &[Sketch], sketch_id: &str) -> Result<Vec<Constraint>> {
    let sketch = sketches
        .iter()
        .find(|sketch| sketch.id.as_str() == sketch_id)
        .ok_or_else(|| OpenCadError::not_found(format!("sketch '{sketch_id}'")))?;
    Ok(sketch.constraints.clone())
}

fn list_sketch_entities(sketches: &[Sketch], sketch_id: &str) -> Result<Vec<SketchEntityInfo>> {
    let sketch = sketches
        .iter()
        .find(|sketch| sketch.id.as_str() == sketch_id)
        .ok_or_else(|| OpenCadError::not_found(format!("sketch '{sketch_id}'")))?;
    let mut items: Vec<SketchEntityInfo> = sketch.entities.iter().map(sketch_entity_info).collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(items)
}

fn sketch_entity_info(entity: &SketchEntity) -> SketchEntityInfo {
    SketchEntityInfo {
        id: entity.id().as_str().to_string(),
        kind: sketch_entity_kind(entity).to_string(),
        construction: entity.is_construction(),
    }
}

fn sketch_entity_kind(entity: &SketchEntity) -> &'static str {
    match entity {
        SketchEntity::Point(_) => "point",
        SketchEntity::Line(_) => "line",
        SketchEntity::Circle(_) => "circle",
        SketchEntity::Arc(_) => "arc",
        SketchEntity::Rectangle(_) => "rectangle",
    }
}

fn feature_dependency_edges(graph: &Option<FeatureGraph>) -> Result<Vec<DependencyEdge>> {
    let graph = graph
        .as_ref()
        .ok_or_else(|| OpenCadError::validation("feature_graph is required"))?;
    Ok(graph.dependency_edges().to_vec())
}

fn feature_dependency_neighbors(
    graph: &Option<FeatureGraph>,
    id: &str,
) -> Result<DependencyNeighbors> {
    let graph = graph
        .as_ref()
        .ok_or_else(|| OpenCadError::validation("feature_graph is required"))?;
    if graph.get(id).is_none() {
        return Err(OpenCadError::not_found(format!("feature '{id}'")));
    }
    Ok(dependency_neighbors(id, graph.dependency_edges()))
}

fn parameter_dependency_neighbors(graph: &ParamGraph, id: &str) -> Result<DependencyNeighbors> {
    if graph.get(id).is_none() {
        return Err(OpenCadError::not_found(format!("parameter '{id}'")));
    }
    Ok(dependency_neighbors(id, graph.dependency_edges()))
}

fn dependency_neighbors(id: &str, edges: &[DependencyEdge]) -> DependencyNeighbors {
    let mut upstream = Vec::new();
    let mut downstream = Vec::new();
    for edge in edges {
        if edge.target == id {
            upstream.push(edge.source.clone());
        }
        if edge.source == id {
            downstream.push(edge.target.clone());
        }
    }
    upstream.sort();
    downstream.sort();
    DependencyNeighbors {
        id: id.to_string(),
        upstream,
        downstream,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_feature::bracket_with_hole;
    use opencad_graph::bracket_parameters;

    fn bracket_query(query: DesignQuery) -> QueryParams {
        let part = bracket_with_hole().expect("model");
        QueryParams {
            parameters: bracket_parameters(),
            feature_nodes: part.nodes.into_values().collect(),
            feature_graph: Some(part.graph),
            sketches: part.sketches.into_values().collect(),
            scene: None,
            semantic_refs: Vec::new(),
            query,
        }
    }

    #[test]
    fn lists_parameters_with_evaluated_values() {
        let result = run_query(&bracket_query(DesignQuery::ListParameters)).expect("query");
        let QueryResult::Parameters { items } = result else {
            panic!("expected parameters list");
        };
        assert_eq!(items.len(), 8);
        let width = items
            .iter()
            .find(|item| item.name == "width")
            .expect("width");
        assert!((width.value_m.expect("value") - 0.08).abs() < 1e-9);
    }

    #[test]
    fn gets_feature_detail() {
        let result = run_query(&bracket_query(DesignQuery::GetFeature {
            id: "feature:extrude_base".into(),
        }))
        .expect("query");
        let QueryResult::Feature { item } = result else {
            panic!("expected feature detail");
        };
        assert_eq!(item.info.feature_type, "extrude");
    }

    #[test]
    fn returns_feature_order_from_graph() {
        let result = run_query(&bracket_query(DesignQuery::FeatureOrder)).expect("query");
        let QueryResult::FeatureOrder { order } = result else {
            panic!("expected feature order");
        };
        assert_eq!(
            order.first().map(String::as_str),
            Some("feature:sketch_base")
        );
        assert_eq!(order.last().map(String::as_str), Some("feature:hole_mount"));
    }

    #[test]
    fn missing_parameter_returns_error() {
        let err = run_query(&bracket_query(DesignQuery::GetParameter {
            id: "param:missing".into(),
        }))
        .expect_err("missing");
        assert!(err.to_string().contains("parameter"));
    }

    #[test]
    fn lists_sketches_with_counts() {
        let result = run_query(&bracket_query(DesignQuery::ListSketches)).expect("query");
        let QueryResult::Sketches { items } = result else {
            panic!("expected sketches list");
        };
        assert_eq!(items.len(), 2);
        let base = items
            .iter()
            .find(|item| item.id == "sketch:base")
            .expect("base sketch");
        assert_eq!(base.constraint_count, 2);
    }

    #[test]
    fn lists_sketch_constraints() {
        let result = run_query(&bracket_query(DesignQuery::ListSketchConstraints {
            sketch_id: "sketch:hole".into(),
        }))
        .expect("query");
        let QueryResult::SketchConstraints { sketch_id, items } = result else {
            panic!("expected sketch constraints");
        };
        assert_eq!(sketch_id, "sketch:hole");
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn lists_sketch_entities() {
        let result = run_query(&bracket_query(DesignQuery::ListSketchEntities {
            sketch_id: "sketch:hole".into(),
        }))
        .expect("query");
        let QueryResult::SketchEntities { sketch_id, items } = result else {
            panic!("expected sketch entities");
        };
        assert_eq!(sketch_id, "sketch:hole");
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|item| item.kind == "point"));
        assert!(items.iter().any(|item| item.kind == "circle"));
    }

    #[test]
    fn returns_feature_dependency_neighbors() {
        let result = run_query(&bracket_query(DesignQuery::GetFeatureDependencies {
            id: "feature:hole_mount".into(),
        }))
        .expect("query");
        let QueryResult::FeatureDependencyNeighbors { item } = result else {
            panic!("expected feature dependency neighbors");
        };
        assert!(item.upstream.contains(&"feature:extrude_base".to_string()));
        assert!(item.upstream.contains(&"feature:sketch_hole".to_string()));
        assert!(item.downstream.is_empty());
    }

    #[test]
    fn returns_parameter_dependency_neighbors() {
        let mut params = bracket_query(DesignQuery::GetParameterDependencies {
            id: "param:width".into(),
        });
        params
            .parameters
            .add_parameter(opencad_graph::ParameterEntry::new(
                "param:half",
                "half",
                "width / 2",
            ))
            .expect("half");
        params
            .parameters
            .add_dependency("param:width", "param:half")
            .expect("dep");

        let result = run_query(&params).expect("query");
        let QueryResult::ParameterDependencyNeighbors { item } = result else {
            panic!("expected parameter dependency neighbors");
        };
        assert!(item.downstream.contains(&"param:half".to_string()));
        assert!(item.upstream.is_empty());
    }

    #[test]
    fn list_overlay_lines_requires_scene_context() {
        let err = run_query(&bracket_query(DesignQuery::ListOverlayLines)).expect_err("missing");
        assert!(err.to_string().contains("scene tessellation"));
    }

    #[test]
    fn lists_overlay_lines_from_scene_context() {
        let mut params = bracket_query(DesignQuery::ListOverlayLines);
        params.scene = Some(SceneQueryContext {
            overlay_lines: vec![OverlayLineInfo {
                line_index: 0,
                sketch_id: Some("sketch:base".into()),
                entity_id: Some("ent:e0".into()),
                entity_kind: Some("line".into()),
                segment_index: None,
                construction: false,
                start_m: [0.0, 0.0, 0.0],
                end_m: [0.08, 0.0, 0.0],
            }],
            face_groups: Vec::new(),
        });
        let result = run_query(&params).expect("query");
        let QueryResult::OverlayLines { items } = result else {
            panic!("expected overlay lines");
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].entity_id.as_deref(), Some("ent:e0"));
    }

    #[test]
    fn lists_face_groups_from_scene_context() {
        let mut params = bracket_query(DesignQuery::ListFaceGroups);
        params.scene = Some(SceneQueryContext {
            overlay_lines: Vec::new(),
            face_groups: vec![FaceGroupInfo {
                face_group_index: 2,
                face_role: "top".into(),
                triangle_count: 12,
                face_normal_m: [0.0, 0.0, 1.0],
                face_centroid_m: [0.0, 0.0, 0.006],
                kernel_face_id: Some(42),
                inferred_feature_id: Some("feature:extrude_base".into()),
                inferred_topo_ref_id: Some("ref:face:kernel_42".into()),
            }],
        });
        let result = run_query(&params).expect("query");
        let QueryResult::FaceGroups { items } = result else {
            panic!("expected face groups");
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].face_role, "top");
    }

    #[test]
    fn lists_semantic_refs_from_document_context() {
        use opencad_core::TopoRefId;
        use opencad_geometry::TopoRef;

        let mut params = bracket_query(DesignQuery::ListSemanticRefs);
        params.semantic_refs = vec![TopoRef::kernel_face(
            TopoRefId::new("ref:face:bracket_top").expect("id"),
            "feature:extrude_base",
            "top",
            42,
            [0.0, 0.0, 1.0],
        )];
        let result = run_query(&params).expect("query");
        let QueryResult::SemanticRefs { items } = result else {
            panic!("expected semantic refs");
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].ref_id, "ref:face:bracket_top");
        assert_eq!(items[0].kernel_face_id, Some(42));
        assert_eq!(items[0].role.as_deref(), Some("top"));
    }

    #[test]
    fn gets_semantic_ref_by_id() {
        use opencad_core::TopoRefId;
        use opencad_geometry::TopoRef;

        let mut params = bracket_query(DesignQuery::GetSemanticRef {
            ref_id: "ref:face:bracket_top".into(),
        });
        params.semantic_refs = vec![TopoRef::kernel_face(
            TopoRefId::new("ref:face:bracket_top").expect("id"),
            "feature:extrude_base",
            "top",
            42,
            [0.0, 0.0, 1.0],
        )];
        let result = run_query(&params).expect("query");
        let QueryResult::SemanticRef { item } = result else {
            panic!("expected semantic ref");
        };
        assert_eq!(item.ref_id, "ref:face:bracket_top");
        assert_eq!(item.created_by, "feature:extrude_base");
    }

    #[test]
    fn missing_semantic_ref_returns_error() {
        let err = run_query(&bracket_query(DesignQuery::GetSemanticRef {
            ref_id: "ref:face:missing".into(),
        }))
        .expect_err("missing");
        assert!(err.to_string().contains("semantic ref"));
    }
}
