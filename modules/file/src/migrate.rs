//! Schema version migrations (Task-116+).

use opencad_core::Result;

use crate::document::OcadDocument;

/// Normalize current models after deserialization.
///
/// ADR-035 adds explicit datum feature/workplane tags. Legacy revolve axes
/// deserialize with `axis_feature = None`; no datum association is inferred.
/// The additive wire forms need no version branch: numeric axes and workplanes
/// retain their meaning, and absent datum expressions default to None.
///
/// ADR-034: legacy drawing point pairs deserialize as `LinearMeasurement::Points`
/// through the untagged wire migration. No semantic association is invented and
/// current semantic dimensions retain only their `edge_ref` on serialization.
/// Legacy drawing sheets deserialize without `dimensions` as an empty collection;
/// sorting here makes newly added dimension records deterministic. Equal-target
/// wire compatibility is handled by `opencad_sketch::EqualTarget` itself:
/// legacy bare strings are interpreted as line lengths and canonical
/// serialization emits an explicit target object, so no document schema
/// version branch is needed for that change.
pub fn migrate_to_current(mut doc: OcadDocument) -> Result<OcadDocument> {
    if let Some(assembly) = doc.assembly.take() {
        doc.assembly = Some(assembly.sorted_deterministic());
    }
    if let Some(drawing) = doc.drawing.take() {
        doc.drawing = Some(drawing.sorted_deterministic());
    }
    Ok(doc)
}
