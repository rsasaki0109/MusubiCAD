//! Lightweight document inspection for the desktop shell.

use opencad_core::Result;
use opencad_file::read_ocad;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentInspect {
    pub id: String,
    pub name: String,
    pub sketches: usize,
    pub features: usize,
    pub parameters: usize,
    pub semantic_refs: usize,
}

pub fn inspect_document(path: &str) -> Result<DocumentInspect> {
    let doc = read_ocad(path)?;
    Ok(DocumentInspect {
        id: doc.metadata.id.as_str().to_string(),
        name: doc.metadata.name.clone(),
        sketches: doc.sketches.len(),
        features: doc.feature_nodes.len(),
        parameters: doc.parameters.evaluation_order()?.len(),
        semantic_refs: doc.semantic_refs.len(),
    })
}
