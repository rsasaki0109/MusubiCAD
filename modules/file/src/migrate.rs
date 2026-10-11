//! Schema version migrations (Task-116+).

use opencad_core::Result;

use crate::document::OcadDocument;

/// Normalize current models after deserialization.
///
/// ADR-038: absent fillet radius_law defaults to None and legacy fillets remain
/// constant-radius. The additive field needs no version branch; variable/legacy
/// round trips verify that no law is inferred and endpoint expressions persist.
///
/// ADR-037 adds the standard_hole tag with required standards and explicit
/// tap-drill representation. Legacy hole variants remain unchanged; no thread
/// or manufacturing standard is inferred. The additive form requires no schema
/// version branch; legacy/current round trips cover this migration policy.
///
/// ADR-036 adds an explicit draft tag (SI Angle and neutral Workplane). This is
/// additive: legacy features deserialize unchanged and no draft is inferred.
/// Round-trip/authoring tests cover both old datums and current draft files.
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
/// MCAD-P11-006: `body_boolean` is an additive tagged definition. Existing
/// bodies are never combined implicitly during migration; legacy documents
/// keep their original feature graph and round-trip unchanged.
/// MCAD-P12-001: legacy sheets default to an empty angular collection.
/// Angular annotations are never inferred from old geometry or labels.
pub fn migrate_to_current(mut doc: OcadDocument) -> Result<OcadDocument> {
    if let Some(assembly) = doc.assembly.take() {
        doc.assembly = Some(assembly.sorted_deterministic());
    }
    if let Some(drawing) = doc.drawing.take() {
        doc.drawing = Some(drawing.sorted_deterministic());
    }
    Ok(doc)
}
