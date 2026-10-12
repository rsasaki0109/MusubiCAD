//! Serializable, backend-owned undo/redo history for design documents.
//!
//! History is intentionally separate from [`OcadDocument`].  It is a transport
//! value for command clients (including the desktop UI), not part of the
//! `.ocad` schema or the persisted Design Graph.  Viewport, camera, and
//! selection state therefore cannot accidentally enter document history.

use opencad_core::{OpenCadError, Result};
use serde::{Deserialize, Serialize};

use crate::expanded_dir::{serialize_document_files, CHECKSUMS_FILE};
use crate::OcadDocument;

/// One atomic document change represented by complete source snapshots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentHistoryEntry {
    /// Document state before the command was applied.
    pub before: OcadDocument,
    /// Document state after the command was applied.
    pub after: OcadDocument,
    /// Human-readable command description for clients and audit UI.
    pub description: String,
}

/// Backend undo/redo state carried independently of an `.ocad` document.
///
/// The vectors are serialized so a command client can pass this value back to
/// the backend without interpreting design semantics.  New document records
/// always clear the redo vector.  Undo and redo validate the current document
/// against the expected snapshot before changing either value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DocumentHistory {
    #[serde(default)]
    undo: Vec<DocumentHistoryEntry>,
    #[serde(default)]
    redo: Vec<DocumentHistoryEntry>,
}

/// Serializable result returned by a backend history command.
///
/// `history` is intentionally an opaque transport value to command clients:
/// clients pass it back unchanged and use only the capability flags to update
/// undo/redo controls.  Keeping the flags beside the value avoids requiring a
/// UI to inspect the snapshot stacks (or to recreate their semantics).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentHistoryState {
    pub history: DocumentHistory,
    pub can_undo: bool,
    pub can_redo: bool,
}

impl DocumentHistoryState {
    pub fn new(history: DocumentHistory) -> Self {
        Self {
            can_undo: history.can_undo(),
            can_redo: history.can_redo(),
            history,
        }
    }
}

impl DocumentHistory {
    /// Record a successful document change and clear any redo branch.
    pub fn record(
        &mut self,
        before: OcadDocument,
        after: OcadDocument,
        description: impl Into<String>,
    ) {
        self.undo.push(DocumentHistoryEntry {
            before,
            after,
            description: description.into(),
        });
        self.redo.clear();
    }

    /// Apply the latest undo entry after validating the current snapshot.
    pub fn undo(&mut self, current: &mut OcadDocument) -> Result<()> {
        let entry = self
            .undo
            .last()
            .cloned()
            .ok_or_else(|| OpenCadError::validation("document history has no undo entry"))?;
        if !same_source_snapshot(current, &entry.after)? {
            return Err(OpenCadError::validation(
                "document does not match the expected undo snapshot",
            ));
        }

        let Some(entry) = self.undo.pop() else {
            return Err(OpenCadError::validation(
                "document history changed while undoing",
            ));
        };
        *current = entry.before.clone();
        self.redo.push(entry);
        Ok(())
    }

    /// Apply the latest redo entry after validating the current snapshot.
    pub fn redo(&mut self, current: &mut OcadDocument) -> Result<()> {
        let entry = self
            .redo
            .last()
            .cloned()
            .ok_or_else(|| OpenCadError::validation("document history has no redo entry"))?;
        if !same_source_snapshot(current, &entry.before)? {
            return Err(OpenCadError::validation(
                "document does not match the expected redo snapshot",
            ));
        }

        let Some(entry) = self.redo.pop() else {
            return Err(OpenCadError::validation(
                "document history changed while redoing",
            ));
        };
        *current = entry.after.clone();
        self.undo.push(entry);
        Ok(())
    }

    /// Whether an undo operation is currently available.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether a redo operation is currently available.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Number of undo entries, useful for opaque transport clients to update
    /// button state without inspecting document semantics.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Number of redo entries, useful for opaque transport clients to update
    /// button state without inspecting document semantics.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
}

/// Compare canonical source files rather than transitively comparing geometry
/// floating-point fields. JSON object member order is normalized while
/// comparing because opaque serde transports may deserialize `IndexMap` fields
/// through a sorted JSON map; values and array order remain exact. History
/// identity is serialized source identity, not a geometric equivalence
/// relation. The checksum manifest is derived from those source files and is
/// omitted from identity comparison.
fn same_source_snapshot(left: &OcadDocument, right: &OcadDocument) -> Result<bool> {
    #[derive(PartialEq)]
    enum SourceFile {
        Json(serde_json::Value),
        Attachment(Vec<u8>),
    }
    fn canonical_files(
        doc: &OcadDocument,
    ) -> Result<std::collections::BTreeMap<String, SourceFile>> {
        serialize_document_files(doc)?
            .into_iter()
            .filter(|(name, _)| name != CHECKSUMS_FILE)
            .map(|(name, bytes)| {
                // Imported assets are immutable opaque bytes, not JSON models.
                if name.starts_with("imports/") {
                    return Ok((name, SourceFile::Attachment(bytes)));
                }
                serde_json::from_slice(&bytes)
                    .map(|value| (name, SourceFile::Json(value)))
                    .map_err(opencad_core::OpenCadError::from)
            })
            .collect()
    }

    Ok(canonical_files(left)? == canonical_files(right)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{DocumentId, DocumentMetadata};
    use opencad_feature::bracket_base_plate;
    use opencad_graph::bracket_parameters;

    fn bracket_document() -> OcadDocument {
        let part = bracket_base_plate().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:history").expect("id"),
            "History fixture",
        );
        let mut document = OcadDocument::from_part_model(metadata, &part);
        document.parameters = bracket_parameters();
        document
    }

    fn changed_document(mut document: OcadDocument, expr: &str) -> OcadDocument {
        document
            .parameters
            .set_expr("param:width", expr)
            .expect("width");
        document.parameters.mark_dirty("param:width");
        document
    }

    #[test]
    fn history_serializes_and_round_trips_full_snapshots() {
        let before = bracket_document();
        let after = changed_document(before.clone(), "100 mm");
        let mut history = DocumentHistory::default();
        history.record(before, after, "Set width to 100 mm");

        let json = serde_json::to_string(&history).expect("serialize history");
        let restored: DocumentHistory = serde_json::from_str(&json).expect("deserialize history");
        assert_eq!(history, restored);
        assert_eq!(restored.undo_len(), 1);
        assert_eq!(restored.redo_len(), 0);
    }

    #[test]
    fn full_document_undo_and_redo_restore_exact_snapshots() {
        let before = bracket_document();
        let after = changed_document(before.clone(), "100 mm");
        let mut current = after.clone();
        let mut history = DocumentHistory::default();
        history.record(before.clone(), after.clone(), "Set width");

        history.undo(&mut current).expect("undo");
        assert_eq!(current, before);
        assert!(!history.can_undo());
        assert!(history.can_redo());

        history.redo(&mut current).expect("redo");
        assert_eq!(current, after);
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn mismatched_undo_and_redo_leave_document_and_history_unchanged() {
        let before = bracket_document();
        let after = changed_document(before.clone(), "100 mm");
        let mut history = DocumentHistory::default();
        history.record(before, after.clone(), "Set width");
        let original_history = history.clone();
        let mut current = bracket_document();

        let error = history.undo(&mut current).expect_err("stale undo");
        assert!(error.to_string().contains("expected undo snapshot"));
        assert_eq!(history, original_history);
        assert_eq!(current, bracket_document());

        let mut current = after;
        history.undo(&mut current).expect("undo");
        let original_history = history.clone();
        let mut stale = changed_document(bracket_document(), "110 mm");
        let error = history.redo(&mut stale).expect_err("stale redo");
        assert!(error.to_string().contains("expected redo snapshot"));
        assert_eq!(history, original_history);
        assert_eq!(stale, changed_document(bracket_document(), "110 mm"));
    }

    #[test]
    fn new_record_clears_redo_branch() {
        let before = bracket_document();
        let middle = changed_document(before.clone(), "100 mm");
        let after = changed_document(middle.clone(), "120 mm");
        let mut history = DocumentHistory::default();
        history.record(before, middle.clone(), "Set width to 100 mm");
        let mut current = middle;
        history.undo(&mut current).expect("undo");
        assert!(history.can_redo());

        history.record(current.clone(), after, "Set width to 120 mm");
        assert!(!history.can_redo());
        assert!(history.can_undo());
    }
}
