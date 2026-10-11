use serde::{Deserialize, Serialize};

/// Global coordinate planes or a custom workplane (future).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Workplane {
    DatumPlane {
        feature_id: String,
    },
    Global {
        plane: GlobalPlane,
    },
    Custom {
        origin: [f64; 3],
        normal: [f64; 3],
        x_axis: [f64; 3],
    },
    /// Resolved at regeneration from a persisted `TopoRef` face id.
    FaceRef {
        face_ref: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlobalPlane {
    XY,
    YZ,
    XZ,
}

impl Default for Workplane {
    fn default() -> Self {
        Self::Global {
            plane: GlobalPlane::XY,
        }
    }
}

impl Workplane {
    pub fn xy() -> Self {
        Self::Global {
            plane: GlobalPlane::XY,
        }
    }

    pub fn xz() -> Self {
        Self::Global {
            plane: GlobalPlane::XZ,
        }
    }

    pub fn face_ref(face_ref: impl Into<String>) -> Self {
        Self::FaceRef {
            face_ref: face_ref.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workplane_round_trip() {
        let wp = Workplane::xy();
        let json = serde_json::to_string(&wp).expect("serialize");
        let restored: Workplane = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(wp, restored);
    }
}
