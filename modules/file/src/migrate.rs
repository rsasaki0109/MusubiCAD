//! Schema version migrations (Task-116+).

use opencad_core::Result;

use crate::document::OcadDocument;

/// Normalize current models after deserialization.
///
/// Legacy drawing sheets deserialize without `dimensions` as an empty collection;
/// sorting here makes newly added dimension records deterministic.
pub fn migrate_to_current(mut doc: OcadDocument) -> Result<OcadDocument> {
    if let Some(assembly) = doc.assembly.take() {
        doc.assembly = Some(assembly.sorted_deterministic());
    }
    if let Some(drawing) = doc.drawing.take() {
        doc.drawing = Some(drawing.sorted_deterministic());
    }
    Ok(doc)
}
