//! Map pick selections to editable document parameters.

use std::collections::{BTreeMap, BTreeSet};

use opencad_feature::{
    ChamferFeature, CircularPatternFeature, ExtrudeFeature, FeatureDefinition, FeatureNode,
    FilletFeature, HoleFeature, LinearPatternFeature, MirrorPatternFeature, RevolveFeature,
};
use opencad_graph::parameter_names_in_expr;
use opencad_sketch::{Constraint, DistanceTarget, Sketch, SketchEntity};

use crate::pick::PickTarget;

pub fn related_parameter_ids(
    selection: &PickTarget,
    available_ids: &[String],
) -> Vec<String> {
    related_parameter_ids_for_features(
        selection,
        available_ids,
        &[],
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
}

pub fn related_parameter_ids_for_features(
    selection: &PickTarget,
    available_ids: &[String],
    feature_nodes: &[FeatureNode],
    sketches: &BTreeMap<String, Sketch>,
    parameter_name_to_id: &BTreeMap<String, String>,
) -> Vec<String> {
    let available: BTreeSet<&str> = available_ids.iter().map(String::as_str).collect();
    let graph_ids = graph_related_parameter_ids(
        selection,
        feature_nodes,
        sketches,
        parameter_name_to_id,
    );
    let merged = if graph_ids.is_empty() {
        related_parameter_candidates_heuristic(selection)
    } else {
        graph_ids
    };
    merged
        .into_iter()
        .filter(|id| available.contains(id.as_str()))
        .collect()
}

pub fn related_parameter_candidates(selection: &PickTarget) -> Vec<String> {
    related_parameter_candidates_heuristic(selection)
}

fn graph_related_parameter_ids(
    selection: &PickTarget,
    feature_nodes: &[FeatureNode],
    sketches: &BTreeMap<String, Sketch>,
    parameter_name_to_id: &BTreeMap<String, String>,
) -> Vec<String> {
    if feature_nodes.is_empty() {
        return Vec::new();
    }

    let exprs = match selection {
        PickTarget::None => return Vec::new(),
        PickTarget::SketchLine {
            sketch_id,
            entity_id,
            ..
        } => {
            let mut exprs = sketch_id
                .as_deref()
                .and_then(|id| {
                    sketches
                        .get(id)
                        .map(|sketch| exprs_for_sketch_line(sketch, entity_id.as_deref()))
                })
                .unwrap_or_default();
            if sketch_id.as_deref() == Some("sketch:profile") {
                exprs.extend(revolve_feature_exprs(feature_nodes));
            }
            exprs
        }
        PickTarget::SolidTriangle {
            inferred_feature_id,
            ..
        } => inferred_feature_id
            .as_deref()
            .map(|feature_id| {
                collect_feature_exprs(feature_id, feature_nodes, sketches, &mut BTreeSet::new())
            })
            .unwrap_or_default(),
    };

    map_expr_names_to_param_ids(&exprs, parameter_name_to_id)
}

fn collect_feature_exprs(
    feature_id: &str,
    feature_nodes: &[FeatureNode],
    sketches: &BTreeMap<String, Sketch>,
    visited: &mut BTreeSet<String>,
) -> Vec<String> {
    if !visited.insert(feature_id.to_string()) {
        return Vec::new();
    }

    let Some(node) = find_feature(feature_nodes, feature_id) else {
        return Vec::new();
    };

    let mut exprs = exprs_from_feature(node);
    if let Some(sketch_feature_id) = sketch_feature_id_for(node) {
        if let Some(sketch_node) = find_feature(feature_nodes, sketch_feature_id) {
            if let FeatureDefinition::Sketch(def) = &sketch_node.definition {
                if let Some(sketch) = sketches.get(def.sketch_id.as_str()) {
                    exprs.extend(exprs_from_sketch(sketch));
                }
            }
        }
    }

    for source_id in source_feature_ids(node) {
        exprs.extend(collect_feature_exprs(
            source_id.as_str(),
            feature_nodes,
            sketches,
            visited,
        ));
    }

    exprs
}

fn find_feature<'a>(feature_nodes: &'a [FeatureNode], feature_id: &str) -> Option<&'a FeatureNode> {
    feature_nodes
        .iter()
        .find(|node| node.id == feature_id)
}

fn sketch_feature_id_for(node: &FeatureNode) -> Option<&str> {
    match &node.definition {
        FeatureDefinition::Extrude(ExtrudeFeature { sketch_feature, .. })
        | FeatureDefinition::Hole(HoleFeature { sketch_feature, .. })
        | FeatureDefinition::Revolve(RevolveFeature { sketch_feature, .. }) => {
            Some(sketch_feature.as_str())
        }
        _ => None,
    }
}

fn source_feature_ids(node: &FeatureNode) -> Vec<String> {
    match &node.definition {
        FeatureDefinition::LinearPattern(LinearPatternFeature { source_feature, .. })
        | FeatureDefinition::CircularPattern(CircularPatternFeature { source_feature, .. })
        | FeatureDefinition::MirrorPattern(MirrorPatternFeature { source_feature, .. }) => {
            vec![source_feature.clone()]
        }
        FeatureDefinition::Fillet(FilletFeature { target_feature, .. })
        | FeatureDefinition::Chamfer(ChamferFeature { target_feature, .. }) => {
            vec![target_feature.clone()]
        }
        _ => Vec::new(),
    }
}

fn exprs_from_feature(node: &FeatureNode) -> Vec<String> {
    let mut exprs = Vec::new();
    match &node.definition {
        FeatureDefinition::Extrude(ExtrudeFeature { length_expr, .. }) => {
            push_expr_option(&mut exprs, length_expr);
        }
        FeatureDefinition::Hole(HoleFeature { depth_expr, .. }) => {
            push_expr_option(&mut exprs, depth_expr);
        }
        FeatureDefinition::Fillet(FilletFeature { radius_expr, .. }) => {
            push_expr_option(&mut exprs, radius_expr);
        }
        FeatureDefinition::Chamfer(ChamferFeature { distance_expr, .. }) => {
            push_expr_option(&mut exprs, distance_expr);
        }
        FeatureDefinition::LinearPattern(LinearPatternFeature { spacing_expr, .. }) => {
            push_expr_option(&mut exprs, spacing_expr);
        }
        FeatureDefinition::Revolve(RevolveFeature { angle_expr, .. }) => {
            push_expr_option(&mut exprs, angle_expr);
        }
        _ => {}
    }
    exprs
}

fn exprs_from_sketch(sketch: &Sketch) -> Vec<String> {
    sketch
        .constraints
        .iter()
        .filter_map(|constraint| match constraint {
            Constraint::Distance { expr, .. }
            | Constraint::Radius { expr, .. }
            | Constraint::Diameter { expr, .. } => Some(expr.as_str().to_string()),
            _ => None,
        })
        .collect()
}

fn exprs_for_sketch_line(sketch: &Sketch, entity_id: Option<&str>) -> Vec<String> {
    let Some(entity_id) = entity_id else {
        return exprs_from_sketch(sketch);
    };

    let mut direct = Vec::new();
    let mut adjacent = Vec::new();

    for constraint in &sketch.constraints {
        match constraint {
            Constraint::Distance {
                target: DistanceTarget::LineLength { line },
                expr,
                ..
            } if line.as_str() == entity_id => direct.push(expr.as_str().to_string()),
            Constraint::Radius { target, expr, .. } | Constraint::Diameter { target, expr, .. }
                if target.as_str() == entity_id =>
            {
                direct.push(expr.as_str().to_string());
            }
            Constraint::Distance {
                target:
                    DistanceTarget::PointToPoint {
                        a,
                        b,
                    },
                expr,
                ..
            } => {
                if line_touches_point(sketch, entity_id, a.as_str()) {
                    push_unique(&mut adjacent, expr.as_str());
                }
                if line_touches_point(sketch, entity_id, b.as_str()) {
                    push_unique(&mut adjacent, expr.as_str());
                }
            }
            _ => {}
        }
    }

    let mut exprs = if !direct.is_empty() {
        direct
    } else if !adjacent.is_empty() {
        adjacent
    } else {
        exprs_from_sketch(sketch)
    };

    if let Some(height) = shared_vertical_height_expr(sketch, entity_id) {
        push_unique(&mut exprs, &height);
    }

    exprs
}

fn push_unique(exprs: &mut Vec<String>, expr: &str) {
    if !exprs.iter().any(|existing| existing == expr) {
        exprs.push(expr.to_string());
    }
}

fn line_touches_point(sketch: &Sketch, line_id: &str, point_id: &str) -> bool {
    let Some(SketchEntity::Line(line)) = sketch.find_entity(line_id) else {
        return false;
    };
    line.start.as_str() == point_id || line.end.as_str() == point_id
}

fn shared_vertical_height_expr(sketch: &Sketch, line_id: &str) -> Option<String> {
    let is_vertical = sketch.constraints.iter().any(
        |constraint| matches!(constraint, Constraint::Vertical { line, .. } if line.as_str() == line_id),
    );
    if !is_vertical {
        return None;
    }
    sketch.constraints.iter().find_map(|constraint| match constraint {
        Constraint::Distance {
            target: DistanceTarget::LineLength { .. },
            expr,
            ..
        } if expr.as_str() == "height" => Some(expr.as_str().to_string()),
        _ => None,
    })
}

fn revolve_feature_exprs(feature_nodes: &[FeatureNode]) -> Vec<String> {
    feature_nodes
        .iter()
        .find_map(|node| match &node.definition {
            FeatureDefinition::Revolve(RevolveFeature { angle_expr, .. }) => {
                angle_expr.as_ref().map(|expr| vec![expr.clone()])
            }
            _ => None,
        })
        .unwrap_or_default()
}

fn push_expr_option(exprs: &mut Vec<String>, value: &Option<String>) {
    if let Some(expr) = value {
        exprs.push(expr.clone());
    }
}

fn map_expr_names_to_param_ids(
    exprs: &[String],
    parameter_name_to_id: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut ids = Vec::new();
    let mut seen = BTreeSet::new();
    for expr in exprs {
        for name in parameter_names_in_expr(expr) {
            let param_id = parameter_name_to_id
                .get(&name)
                .cloned()
                .or_else(|| {
                    let direct = format!("param:{name}");
                    if parameter_name_to_id.values().any(|id| id == &direct) {
                        Some(direct)
                    } else {
                        None
                    }
                });
            if let Some(param_id) = param_id {
                if seen.insert(param_id.clone()) {
                    ids.push(param_id);
                }
            }
        }
    }
    ids
}

fn related_parameter_candidates_heuristic(selection: &PickTarget) -> Vec<String> {
    match selection {
        PickTarget::None => Vec::new(),
        PickTarget::SketchLine { .. } => {
            vec![
                "param:width".into(),
                "param:height".into(),
                "param:inner_radius".into(),
                "param:outer_radius".into(),
            ]
        }
        PickTarget::SolidTriangle {
            inferred_feature_id,
            face_role,
            ..
        } => {
            let feature = inferred_feature_id.as_deref().unwrap_or("");
            let role = face_role.as_deref().unwrap_or("");
            if feature.contains("revolve") {
                return vec![
                    "param:revolve_angle".into(),
                    "param:outer_radius".into(),
                    "param:inner_radius".into(),
                    "param:height".into(),
                ];
            }
            if feature.contains("boss_join")
                || feature.contains("face_pin")
                || feature.contains("pin_boss")
            {
                return vec![
                    "param:boss_height".into(),
                    "param:hole_diameter".into(),
                    "param:thickness".into(),
                ];
            }
            if feature.contains("pin_holes")
                || feature.contains("pin_hole_ring")
                || feature.contains("pin_ring")
                || feature.contains("pin_mirror")
                || feature.contains("linear_pattern")
                || feature.contains("circular_pattern")
            {
                return vec![
                    "param:hole_pitch".into(),
                    "param:hole_diameter".into(),
                    "param:thickness".into(),
                ];
            }
            if feature.contains("hole") || role == "cylindrical" {
                return vec![
                    "param:hole_diameter".into(),
                    "param:hole_pitch".into(),
                    "param:thickness".into(),
                ];
            }
            if feature.contains("mirror") {
                return vec!["param:hole_pitch".into(), "param:width".into()];
            }
            if feature.contains("fillet") {
                return vec!["param:fillet_radius".into()];
            }
            if feature.contains("chamfer") {
                return vec!["param:chamfer_distance".into()];
            }
            if feature.contains("extrude") {
                return match role {
                    "top" | "bottom" => vec!["param:thickness".into()],
                    "+x" | "-x" => vec!["param:width".into()],
                    "+y" | "-y" => vec!["param:height".into()],
                    _ => vec![
                        "param:width".into(),
                        "param:height".into(),
                        "param:thickness".into(),
                    ],
                };
            }
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_feature::{
        bracket_base_plate, bracket_boss_join, bracket_hole_row, bracket_with_hole,
        revolve_bushing,
    };
    use opencad_graph::{bracket_parameters, revolve_parameters};

    fn model_context(
        model: opencad_feature::PartModel,
        params: opencad_graph::ParamGraph,
    ) -> (Vec<FeatureNode>, BTreeMap<String, Sketch>, BTreeMap<String, String>) {
        let nodes: Vec<FeatureNode> = model.nodes.into_values().collect();
        let sketches = model
            .sketches
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let parameter_name_to_id = params
            .parameter_ids()
            .into_iter()
            .filter_map(|id| params.get(&id).map(|entry| (entry.name.clone(), id)))
            .collect();
        (nodes, sketches, parameter_name_to_id)
    }

    #[test]
    fn revolve_pick_suggests_revolve_parameters() {
        let selection = PickTarget::SolidTriangle {
            triangle_index: 0,
            vertices_m: [[0.0; 3]; 3],
            face_group_index: Some(0),
            face_role: Some("cylindrical".into()),
            face_normal_m: None,
            face_centroid_m: None,
            kernel_face_id: None,
            inferred_feature_id: Some("feature:revolve_bushing".into()),
            inferred_topo_ref_id: None,
        };
        let model = revolve_bushing().expect("model");
        let params = revolve_parameters("360 deg");
        let (nodes, sketches, name_map) = model_context(model, params);
        let available = vec![
            "param:inner_radius".into(),
            "param:outer_radius".into(),
            "param:height".into(),
            "param:revolve_angle".into(),
        ];
        let ids = related_parameter_ids_for_features(
            &selection,
            &available,
            &nodes,
            &sketches,
            &name_map,
        );
        assert_eq!(
            ids,
            vec![
                "param:revolve_angle".to_string(),
                "param:inner_radius".to_string(),
                "param:outer_radius".to_string(),
                "param:height".to_string(),
            ]
        );
    }

    #[test]
    fn pattern_hole_pick_suggests_pitch_and_diameter() {
        let selection = PickTarget::SolidTriangle {
            triangle_index: 0,
            vertices_m: [[0.0; 3]; 3],
            face_group_index: Some(0),
            face_role: Some("cylindrical".into()),
            face_normal_m: None,
            face_centroid_m: None,
            kernel_face_id: None,
            inferred_feature_id: Some("feature:pin_holes".into()),
            inferred_topo_ref_id: None,
        };
        let model = bracket_hole_row().expect("model");
        let params = bracket_parameters();
        let (nodes, sketches, name_map) = model_context(model, params);
        let available = vec![
            "param:hole_pitch".into(),
            "param:hole_diameter".into(),
            "param:thickness".into(),
        ];
        let ids = related_parameter_ids_for_features(
            &selection,
            &available,
            &nodes,
            &sketches,
            &name_map,
        );
        assert_eq!(
            ids,
            vec![
                "param:hole_pitch".to_string(),
                "param:thickness".to_string(),
                "param:hole_diameter".to_string(),
            ]
        );
    }

    #[test]
    fn boss_join_pick_uses_feature_graph_exprs() {
        let selection = PickTarget::SolidTriangle {
            triangle_index: 0,
            vertices_m: [[0.0; 3]; 3],
            face_group_index: Some(0),
            face_role: Some("top".into()),
            face_normal_m: None,
            face_centroid_m: None,
            kernel_face_id: None,
            inferred_feature_id: Some("feature:boss_join".into()),
            inferred_topo_ref_id: None,
        };
        let model = bracket_boss_join().expect("model");
        let params = bracket_parameters();
        let (nodes, sketches, name_map) = model_context(model, params);
        let available = vec![
            "param:boss_height".into(),
            "param:hole_diameter".into(),
            "param:thickness".into(),
        ];
        let ids = related_parameter_ids_for_features(
            &selection,
            &available,
            &nodes,
            &sketches,
            &name_map,
        );
        assert_eq!(
            ids,
            vec![
                "param:boss_height".to_string(),
                "param:hole_diameter".to_string(),
            ]
        );
    }

    #[test]
    fn base_sketch_width_line_targets_width_only() {
        let selection = PickTarget::SketchLine {
            line_index: 0,
            sketch_id: Some("sketch:base".into()),
            entity_id: Some("ent:e0".into()),
            entity_kind: Some("line".into()),
            segment_index: None,
            construction: false,
            start_m: [0.0; 3],
            end_m: [1.0, 0.0, 0.0],
        };
        let model = bracket_base_plate().expect("model");
        let params = bracket_parameters();
        let (nodes, sketches, name_map) = model_context(model, params);
        let available = vec![
            "param:width".into(),
            "param:height".into(),
            "param:inner_radius".into(),
            "param:outer_radius".into(),
        ];
        let ids = related_parameter_ids_for_features(
            &selection,
            &available,
            &nodes,
            &sketches,
            &name_map,
        );
        assert_eq!(ids, vec!["param:width".to_string()]);
    }

    #[test]
    fn base_sketch_height_line_targets_height_only() {
        let selection = PickTarget::SketchLine {
            line_index: 0,
            sketch_id: Some("sketch:base".into()),
            entity_id: Some("ent:e1".into()),
            entity_kind: Some("line".into()),
            segment_index: None,
            construction: false,
            start_m: [0.0; 3],
            end_m: [0.0, 1.0, 0.0],
        };
        let model = bracket_base_plate().expect("model");
        let params = bracket_parameters();
        let (nodes, sketches, name_map) = model_context(model, params);
        let available = vec!["param:width".into(), "param:height".into()];
        let ids = related_parameter_ids_for_features(
            &selection,
            &available,
            &nodes,
            &sketches,
            &name_map,
        );
        assert_eq!(ids, vec!["param:height".to_string()]);
    }

    #[test]
    fn revolve_profile_outer_vertical_targets_height_and_angle() {
        let selection = PickTarget::SketchLine {
            line_index: 0,
            sketch_id: Some("sketch:profile".into()),
            entity_id: Some("ent:e1".into()),
            entity_kind: Some("line".into()),
            segment_index: None,
            construction: false,
            start_m: [0.0; 3],
            end_m: [0.0, 1.0, 0.0],
        };
        let model = revolve_bushing().expect("model");
        let params = revolve_parameters("360 deg");
        let (nodes, sketches, name_map) = model_context(model, params);
        let available = vec![
            "param:height".into(),
            "param:inner_radius".into(),
            "param:outer_radius".into(),
            "param:revolve_angle".into(),
        ];
        let ids = related_parameter_ids_for_features(
            &selection,
            &available,
            &nodes,
            &sketches,
            &name_map,
        );
        assert_eq!(
            ids,
            vec![
                "param:height".to_string(),
                "param:revolve_angle".to_string(),
            ]
        );
    }

    #[test]
    fn revolve_profile_bottom_line_targets_radii_and_angle() {
        let selection = PickTarget::SketchLine {
            line_index: 0,
            sketch_id: Some("sketch:profile".into()),
            entity_id: Some("ent:e0".into()),
            entity_kind: Some("line".into()),
            segment_index: None,
            construction: false,
            start_m: [0.0; 3],
            end_m: [1.0, 0.0, 0.0],
        };
        let model = revolve_bushing().expect("model");
        let params = revolve_parameters("360 deg");
        let (nodes, sketches, name_map) = model_context(model, params);
        let available = vec![
            "param:height".into(),
            "param:inner_radius".into(),
            "param:outer_radius".into(),
            "param:revolve_angle".into(),
        ];
        let ids = related_parameter_ids_for_features(
            &selection,
            &available,
            &nodes,
            &sketches,
            &name_map,
        );
        assert_eq!(
            ids,
            vec![
                "param:inner_radius".to_string(),
                "param:outer_radius".to_string(),
                "param:revolve_angle".to_string(),
            ]
        );
    }

    #[test]
    fn revolve_profile_inner_vertical_targets_inner_radius_height_and_angle() {
        let selection = PickTarget::SketchLine {
            line_index: 0,
            sketch_id: Some("sketch:profile".into()),
            entity_id: Some("ent:e3".into()),
            entity_kind: Some("line".into()),
            segment_index: None,
            construction: false,
            start_m: [0.0; 3],
            end_m: [0.0, 1.0, 0.0],
        };
        let model = revolve_bushing().expect("model");
        let params = revolve_parameters("360 deg");
        let (nodes, sketches, name_map) = model_context(model, params);
        let available = vec![
            "param:height".into(),
            "param:inner_radius".into(),
            "param:outer_radius".into(),
            "param:revolve_angle".into(),
        ];
        let ids = related_parameter_ids_for_features(
            &selection,
            &available,
            &nodes,
            &sketches,
            &name_map,
        );
        assert_eq!(
            ids,
            vec![
                "param:inner_radius".to_string(),
                "param:height".to_string(),
                "param:revolve_angle".to_string(),
            ]
        );
    }

    #[test]
    fn hole_circle_pick_targets_hole_diameter_only() {
        let selection = PickTarget::SketchLine {
            line_index: 0,
            sketch_id: Some("sketch:hole".into()),
            entity_id: Some("ent:hole_circle".into()),
            entity_kind: Some("circle".into()),
            segment_index: Some(0),
            construction: false,
            start_m: [0.0; 3],
            end_m: [1.0, 0.0, 0.0],
        };
        let model = bracket_with_hole().expect("model");
        let params = bracket_parameters();
        let (nodes, sketches, name_map) = model_context(model, params);
        let available = vec![
            "param:hole_diameter".into(),
            "param:width".into(),
            "param:height".into(),
            "param:thickness".into(),
        ];
        let ids = related_parameter_ids_for_features(
            &selection,
            &available,
            &nodes,
            &sketches,
            &name_map,
        );
        assert_eq!(ids, vec!["param:hole_diameter".to_string()]);
    }

    #[test]
    fn filters_unknown_parameter_ids() {
        let selection = PickTarget::SketchLine {
            line_index: 0,
            sketch_id: None,
            entity_id: None,
            entity_kind: None,
            segment_index: None,
            construction: false,
            start_m: [0.0; 3],
            end_m: [1.0, 0.0, 0.0],
        };
        let ids = related_parameter_ids(&selection, &["param:width".into()]);
        assert_eq!(ids, vec!["param:width".to_string()]);
    }

    #[test]
    fn parameter_names_in_expr_extracts_identifiers() {
        let names = parameter_names_in_expr("hole_diameter / 2");
        assert_eq!(names, vec!["hole_diameter".to_string()]);
    }
}
