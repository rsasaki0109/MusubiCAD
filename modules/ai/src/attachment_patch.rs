//! Attachment operations for `DesignPatch` (ADR-016).
//!
//! Attachments are opaque files under `imports/`, such as STEP files used by
//! imported solids.  They enter a document only through `add_attachment`,
//! whose decoded bytes must match the declared SHA-256.

use std::collections::{BTreeMap, BTreeSet};

use base64::Engine as _;
use opencad_core::{sha256_hex, OpenCadError, Result};
use opencad_feature::FeatureDefinition;
use opencad_graph::SemanticChange;

use crate::patch::PatchOperation;
use crate::DesignState;

/// Maximum decoded attachment size in bytes (64 MiB).
pub const MAX_ATTACHMENT_BYTES: usize = 64 * 1024 * 1024;

/// Validate `imports/[a-z0-9_]+([.-][a-z0-9_]+)*\.(step|stp)`.
pub(crate) fn validate_attachment_path(path: &str) -> Result<()> {
    let valid = path
        .strip_prefix("imports/")
        .and_then(|name| {
            name.strip_suffix(".step")
                .or_else(|| name.strip_suffix(".stp"))
        })
        .is_some_and(|stem| {
            !stem.is_empty()
                && stem.split(['.', '-']).all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                })
        });
    if valid {
        Ok(())
    } else {
        Err(OpenCadError::validation(format!(
            "invalid attachment path '{path}': expected imports/<lowercase_name>.step or .stp"
        )))
    }
}

/// Apply attachment operations in order with local checks only.
pub(crate) fn apply_attachment_operations(
    operations: &[PatchOperation],
    attachments: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let mut removed = BTreeSet::new();
    for operation in operations {
        match operation {
            PatchOperation::AddAttachment {
                path,
                sha256,
                content_base64,
            } => {
                validate_attachment_path(path)?;
                if removed.contains(path.as_str()) {
                    return Err(OpenCadError::validation(format!(
                        "attachment path '{path}' was removed earlier in this patch and cannot be reused"
                    )));
                }
                if attachments.contains_key(path) {
                    return Err(OpenCadError::validation(format!(
                        "attachment '{path}' already exists"
                    )));
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(content_base64)
                    .map_err(|error| {
                        OpenCadError::validation(format!(
                            "attachment '{path}' content_base64 is not valid base64: {error}"
                        ))
                    })?;
                if bytes.is_empty() || bytes.len() > MAX_ATTACHMENT_BYTES {
                    return Err(OpenCadError::validation(format!(
                        "attachment '{path}' must hold 1..={MAX_ATTACHMENT_BYTES} bytes"
                    )));
                }
                let actual = sha256_hex(&bytes);
                if actual != *sha256 {
                    return Err(OpenCadError::validation(format!(
                        "attachment '{path}' has sha256 {actual}, but the patch declares {sha256}"
                    )));
                }
                attachments.insert(path.clone(), bytes);
            }
            PatchOperation::RemoveAttachment { path } => {
                attachments.remove(path).ok_or_else(|| {
                    OpenCadError::validation(format!("unknown attachment '{path}'"))
                })?;
                removed.insert(path.as_str());
            }
            _ => {}
        }
    }
    Ok(())
}

/// Removed attachments may not leave an imported solid without its source.
pub(crate) fn validate_attachment_candidate(
    operations: &[PatchOperation],
    after: &DesignState,
    failures: &mut BTreeSet<String>,
) {
    for operation in operations {
        let PatchOperation::RemoveAttachment { path } = operation else {
            continue;
        };
        if after.attachments.contains_key(path) {
            continue;
        }
        let users: BTreeSet<&str> = after
            .feature_nodes
            .iter()
            .filter(|node| {
                matches!(&node.definition, FeatureDefinition::ImportedSolid(def) if def.source == *path)
            })
            .map(|node| node.id.as_str())
            .collect();
        if !users.is_empty() {
            failures.insert(format!(
                "cannot remove attachment '{path}': still used by feature {}",
                users.into_iter().collect::<Vec<_>>().join(", feature ")
            ));
        }
    }
}

/// Path → SHA-256 of every attachment, used for revisions and comparison.
pub(crate) fn attachment_digests(
    attachments: &BTreeMap<String, Vec<u8>>,
) -> BTreeMap<String, String> {
    attachments
        .iter()
        .map(|(path, bytes)| (path.clone(), sha256_hex(bytes)))
        .collect()
}

/// Report added, removed, and changed attachments by path.
pub(crate) fn diff_attachments(
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
) -> Vec<SemanticChange> {
    let (before, after) = (attachment_digests(before), attachment_digests(after));
    let paths: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    paths
        .into_iter()
        .filter_map(|path| match (before.get(path), after.get(path)) {
            (Some(_), None) => Some(SemanticChange::AttachmentRemoved { path: path.clone() }),
            (None, Some(_)) => Some(SemanticChange::AttachmentAdded { path: path.clone() }),
            (Some(old), Some(new)) if old != new => Some(SemanticChange::AttachmentChanged {
                path: path.clone(),
                before: old.clone(),
                after: new.clone(),
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_paths_are_restricted_to_step_files_under_imports() {
        for ok in [
            "imports/motor.step",
            "imports/nema17-motor.stp",
            "imports/a.b_c.step",
        ] {
            assert!(validate_attachment_path(ok).is_ok(), "{ok}");
        }
        for bad in [
            "motor.step",
            "imports/Motor.step",
            "imports/../x.step",
            "imports/motor.stl",
            "imports/.step",
            "imports/sub/motor.step",
        ] {
            assert!(validate_attachment_path(bad).is_err(), "{bad}");
        }
    }
}
