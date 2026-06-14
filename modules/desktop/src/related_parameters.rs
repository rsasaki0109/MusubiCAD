//! Map pick selections to editable document parameters.

use opencad_feature::FeatureNode;

use crate::pick::PickTarget;

pub fn related_parameter_ids(
    selection: &PickTarget,
    available_ids: &[String],
) -> Vec<String> {
    let available: std::collections::BTreeSet<&str> =
        available_ids.iter().map(String::as_str).collect();
    related_parameter_candidates(selection)
        .into_iter()
        .filter(|id| available.contains(id.as_str()))
        .collect()
}

pub fn related_parameter_candidates(selection: &PickTarget) -> Vec<String> {
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
            if feature.contains("hole") || role == "cylindrical" {
                return vec![
                    "param:hole_diameter".into(),
                    "param:hole_pitch".into(),
                    "param:thickness".into(),
                ];
            }
            if feature.contains("boss") {
                return vec![
                    "param:boss_diameter".into(),
                    "param:boss_height".into(),
                    "param:thickness".into(),
                ];
            }
            if feature.contains("pattern")
                || feature.contains("pin_row")
                || feature.contains("hole_row")
            {
                return vec![
                    "param:hole_pitch".into(),
                    "param:hole_diameter".into(),
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

#[allow(dead_code)]
pub fn related_parameter_ids_for_features(
    selection: &PickTarget,
    available_ids: &[String],
    _feature_nodes: &[FeatureNode],
) -> Vec<String> {
    related_parameter_ids(selection, available_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let available = vec![
            "param:inner_radius".into(),
            "param:outer_radius".into(),
            "param:height".into(),
            "param:revolve_angle".into(),
        ];
        let ids = related_parameter_ids(&selection, &available);
        assert_eq!(
            ids,
            vec![
                "param:revolve_angle".to_string(),
                "param:outer_radius".to_string(),
                "param:inner_radius".to_string(),
                "param:height".to_string(),
            ]
        );
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
}
