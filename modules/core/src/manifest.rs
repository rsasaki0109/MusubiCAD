use serde::{Deserialize, Serialize};

/// Top-level manifest for a `.ocad` container.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcadManifest {
    pub schema: String,
    pub format_version: String,
    pub document_id: String,
    pub created_at: String,
    pub files: ManifestFiles,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestFiles {
    pub document: String,
    pub graph: ManifestGraphFiles,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai: Option<ManifestAiFiles>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<ManifestCacheFiles>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestGraphFiles {
    pub parameters: String,
    pub sketches: String,
    pub constraints: String,
    pub features: String,
    pub assemblies: String,
    pub materials: String,
    pub semantic_refs: String,
    #[serde(default = "default_drawings_path")]
    pub drawings: String,
}

fn default_drawings_path() -> String {
    "graph/drawings.json".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestAiFiles {
    pub design_intent: String,
    pub edit_history: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestCacheFiles {
    pub brep_dir: String,
    pub mesh_dir: String,
}

impl OcadManifest {
    pub fn new_v0_1(document_id: impl Into<String>) -> Self {
        Self {
            schema: "opencad.manifest.v0.1".into(),
            format_version: "0.1.0".into(),
            document_id: document_id.into(),
            created_at: "1970-01-01T00:00:00Z".into(),
            files: ManifestFiles {
                document: "document.ocad.json".into(),
                graph: ManifestGraphFiles {
                    parameters: "graph/parameters.json".into(),
                    sketches: "graph/sketches.json".into(),
                    constraints: "graph/constraints.json".into(),
                    features: "graph/features.json".into(),
                    assemblies: "graph/assemblies.json".into(),
                    materials: "graph/materials.json".into(),
                    semantic_refs: "graph/semantic_refs.json".into(),
                    drawings: "graph/drawings.json".into(),
                },
                ai: Some(ManifestAiFiles {
                    design_intent: "ai/design_intent.md".into(),
                    edit_history: "ai/edit_history.jsonl".into(),
                }),
                cache: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trip() {
        let manifest = OcadManifest::new_v0_1("doc:bracket_001");
        let json = serde_json::to_string_pretty(&manifest).expect("serialize");
        let restored: OcadManifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(manifest, restored);
    }
}
