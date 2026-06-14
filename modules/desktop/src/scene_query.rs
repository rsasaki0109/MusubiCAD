//! Face-group inference helpers shared by pick and scene queries.

use opencad_feature::FeatureNode;
use opencad_geometry::{resolve_topo_ref_id_with_history, FaceDerivation, TopoRef};
use opencad_render::{FaceGroup, FaceRole};

pub fn topo_ref_for_group(
    group: &FaceGroup,
    inferred: &(Option<String>, Option<String>),
    semantic_refs: &[TopoRef],
    face_history: &[FaceDerivation],
) -> Option<String> {
    if let Some(kernel_face_id) = group.kernel_face_id {
        let direct = resolve_topo_ref_id_with_history(semantic_refs, kernel_face_id, face_history);
        if direct
            .as_deref()
            .is_some_and(|ref_id| !ref_id.starts_with("ref:face:kernel_"))
        {
            return direct;
        }

        if let Some(feature_id) = inferred.0.as_deref() {
            if let Some(custom) = semantic_refs.iter().find(|topo_ref| {
                topo_ref.semantic.role.as_deref() == Some(group.role.as_str())
                    && topo_ref.semantic.created_by == feature_id
                    && !topo_ref.ref_id.as_str().starts_with("ref:face:kernel_")
            }) {
                return Some(custom.ref_id.as_str().to_string());
            }
        }

        return direct;
    }
    inferred.1.clone()
}

pub fn infer_face_refs(
    features: &[FeatureNode],
    face: &FaceGroup,
) -> (Option<String>, Option<String>) {
    let feature_id = match face.role {
        FaceRole::Cylindrical => find_feature_by_type(features, "hole")
            .or_else(|| find_feature_by_type(features, "revolve")),
        FaceRole::Top => find_feature_by_type(features, "fillet")
            .or_else(|| find_feature_by_type(features, "chamfer"))
            .or_else(|| find_feature_by_type(features, "extrude"))
            .or_else(|| find_feature_by_type(features, "revolve")),
        FaceRole::Bottom | FaceRole::PosX | FaceRole::NegX | FaceRole::PosY | FaceRole::NegY => {
            find_feature_by_type(features, "extrude")
                .or_else(|| find_feature_by_type(features, "revolve"))
        }
        FaceRole::Other => find_feature_by_type(features, "revolve"),
    };
    let topo_ref_id = feature_id
        .as_deref()
        .and_then(|feature_id| infer_topo_ref_id(feature_id, face.role));
    (feature_id, topo_ref_id)
}

fn find_feature_by_type(features: &[FeatureNode], feature_type: &str) -> Option<String> {
    features
        .iter()
        .find(|node| node.definition.feature_type() == feature_type)
        .map(|node| node.id.clone())
}

fn infer_topo_ref_id(feature_id: &str, role: FaceRole) -> Option<String> {
    let suffix = match role {
        FaceRole::Top => "top",
        FaceRole::Bottom => "bottom",
        FaceRole::Cylindrical => "wall",
        FaceRole::PosX => "pos_x",
        FaceRole::NegX => "neg_x",
        FaceRole::PosY => "pos_y",
        FaceRole::NegY => "neg_y",
        FaceRole::Other => "other",
    };
    let stem = feature_id.strip_prefix("feature:").unwrap_or(feature_id);
    Some(format!("ref:face:{stem}_{suffix}"))
}
