use serde::{Deserialize, Serialize};

use opencad_core::TopoRefId;

/// Semantic topological reference with optional geometric fingerprint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopoRef {
    pub ref_id: TopoRefId,
    pub kind: TopoRefKind,
    pub semantic: TopoRefSemantic,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometric_fingerprint: Option<GeometricFingerprint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_query: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopoRefKind {
    Face,
    Edge,
    Vertex,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopoRefSemantic {
    pub created_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normal_hint: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometricFingerprint {
    pub surface_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_face_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_edge_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area_range: Option<[f64; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox_hint: Option<[[f64; 3]; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjacent_feature_ids: Vec<String>,
}

impl TopoRef {
    pub fn face(ref_id: TopoRefId, created_by: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            ref_id,
            kind: TopoRefKind::Face,
            semantic: TopoRefSemantic {
                created_by: created_by.into(),
                role: Some(role.into()),
                normal_hint: None,
                intent: None,
            },
            geometric_fingerprint: None,
            fallback_query: None,
        }
    }

    pub fn kernel_face(
        ref_id: TopoRefId,
        created_by: impl Into<String>,
        role: impl Into<String>,
        kernel_face_id: u64,
        normal_hint: [f32; 3],
    ) -> Self {
        Self {
            ref_id,
            kind: TopoRefKind::Face,
            semantic: TopoRefSemantic {
                created_by: created_by.into(),
                role: Some(role.into()),
                normal_hint: Some([
                    normal_hint[0] as f64,
                    normal_hint[1] as f64,
                    normal_hint[2] as f64,
                ]),
                intent: None,
            },
            geometric_fingerprint: Some(GeometricFingerprint {
                surface_type: "brep_face".into(),
                kernel_face_id: Some(kernel_face_id),
                kernel_edge_id: None,
                area_range: None,
                bbox_hint: None,
                adjacent_feature_ids: Vec::new(),
            }),
            fallback_query: None,
        }
    }

    pub fn edge(ref_id: TopoRefId, created_by: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            ref_id,
            kind: TopoRefKind::Edge,
            semantic: TopoRefSemantic {
                created_by: created_by.into(),
                role: Some(role.into()),
                normal_hint: None,
                intent: None,
            },
            geometric_fingerprint: None,
            fallback_query: None,
        }
    }

    pub fn kernel_edge(
        ref_id: TopoRefId,
        created_by: impl Into<String>,
        role: impl Into<String>,
        kernel_edge_id: u64,
        midpoint_hint: [f32; 3],
        tangent_hint: [f32; 3],
    ) -> Self {
        Self {
            ref_id,
            kind: TopoRefKind::Edge,
            semantic: TopoRefSemantic {
                created_by: created_by.into(),
                role: Some(role.into()),
                normal_hint: None,
                intent: None,
            },
            geometric_fingerprint: Some(GeometricFingerprint {
                surface_type: "brep_edge".into(),
                kernel_face_id: None,
                kernel_edge_id: Some(kernel_edge_id),
                area_range: None,
                bbox_hint: Some([
                    [
                        midpoint_hint[0] as f64,
                        midpoint_hint[1] as f64,
                        midpoint_hint[2] as f64,
                    ],
                    [
                        tangent_hint[0] as f64,
                        tangent_hint[1] as f64,
                        tangent_hint[2] as f64,
                    ],
                ]),
                adjacent_feature_ids: Vec::new(),
            }),
            fallback_query: None,
        }
    }

    pub fn kernel_face_id(&self) -> Option<u64> {
        self.geometric_fingerprint
            .as_ref()
            .and_then(|fingerprint| fingerprint.kernel_face_id)
    }

    pub fn kernel_edge_id(&self) -> Option<u64> {
        self.geometric_fingerprint
            .as_ref()
            .and_then(|fingerprint| fingerprint.kernel_edge_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topo_ref_round_trip() {
        let topo = TopoRef::face(
            TopoRefId::new("ref:face:base_top").expect("id"),
            "feature:extrude_base",
            "top_face",
        );
        let json = serde_json::to_string(&topo).expect("serialize");
        let restored: TopoRef = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(topo, restored);
    }
}
