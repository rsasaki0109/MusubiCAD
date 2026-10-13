//! Explicit application-level dependency snapshots for verified assembly edits.
use crate::{read_ocad, OcadDocument};
use opencad_core::{sha256_hex, DocumentId, OpenCadError, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblySourceSnapshot {
    pub path: PathBuf,
    pub source_doc: DocumentId,
    pub digest: String,
}

/// Snapshot every referenced source, including metadata and attachment bytes.
/// Canonical containment/identity/cycles/budgets are enforced before any write.
pub fn snapshot_assembly_sources(
    path: &Path,
    doc: &OcadDocument,
) -> Result<Vec<AssemblySourceSnapshot>> {
    fn walk(
        path: &Path,
        doc: &OcadDocument,
        active: &mut BTreeSet<String>,
        snapshots: &mut BTreeMap<PathBuf, AssemblySourceSnapshot>,
        depth: usize,
    ) -> Result<()> {
        if depth > 64 || snapshots.len() > 10000 {
            return Err(OpenCadError::validation(
                "assembly dependency budget exceeded",
            ));
        }
        let key = doc.metadata.id.as_str().to_owned();
        if !active.insert(key.clone()) {
            return Err(OpenCadError::validation("assembly dependency cycle"));
        }
        let result = (|| {
            let Some(model) = &doc.assembly else {
                return Ok(());
            };
            model.validate(&doc.metadata.id)?;
            let root = if path.is_dir() {
                path
            } else {
                path.parent()
                    .ok_or_else(|| OpenCadError::validation("assembly dependency root missing"))?
            };
            for component in &model.components {
                let child_path =
                    opencad_assembly::validate_component_path(root, &component.source_path)?;
                let child = read_ocad(&child_path)?;
                if child.metadata.id != component.source_doc
                    || (child.assembly.is_some())
                        != (component.source_kind != opencad_assembly::ComponentSourceKind::Part)
                    || child.metadata.kind == opencad_core::DocumentKind::Drawing
                {
                    return Err(OpenCadError::validation(
                        "assembly dependency owner/kind mismatch",
                    ));
                }
                let snapshot = AssemblySourceSnapshot {
                    path: child_path.clone(),
                    source_doc: child.metadata.id.clone(),
                    digest: sha256_hex(&serde_json::to_vec(&child)?),
                };
                if let Some(previous) = snapshots.get(&child_path) {
                    if previous != &snapshot {
                        return Err(OpenCadError::validation(
                            "assembly dependency changed during snapshot",
                        ));
                    }
                } else {
                    snapshots.insert(child_path.clone(), snapshot);
                }
                walk(&child_path, &child, active, snapshots, depth + 1)?;
            }
            Ok(())
        })();
        active.remove(&key);
        result
    }
    let mut snapshots = BTreeMap::new();
    walk(path, doc, &mut BTreeSet::new(), &mut snapshots, 0)?;
    Ok(snapshots.into_values().collect())
}
