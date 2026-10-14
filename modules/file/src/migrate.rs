// MCAD-P16-006 / ADR-066: tapered_rib is a distinct closed source tag.
// No taper/end/stock ownership is inferred for legacy rib; old readers refuse
// the unknown tag. Current schema-version migration remains additive.
// MCAD-P16-005 / ADR-065: cylindrical_draft is a distinct closed source tag.
// No angle, lateral ownership or neutral plane is inferred for legacy draft;
// genuine old enums refuse it. Existing schema-version migration stays additive.
// MCAD-P16-004 / ADR-064: countersink is a strict seat discriminant with explicit
// SI included angle; counterbore-only readers refuse it. Never infer an angle,
// seat diameter, face ownership or frustum depth in legacy hole/counterbore data.
// MCAD-P16-003 / ADR-063: seated_standard_hole is an additive strict feature
// tag with a closed counterbore seat. Old readers refuse the new tag. No seat,
// face, standard or depth is inferred for legacy hole/standard_hole records.
// Existing source version policy needs no numeric version branch; packed/
// expanded round trips and genuine old-form decoding cover this admission.
// MCAD-P13-011 / ADR-059: absent instance.articulation remains None. The
// mandatory articulated_assembly source-kind guard rejects old readers; missing
// or orphan guards refuse on decoding. No coordinates or child motion inferred.
// MCAD-P13-008 / ADR-057: cubic_bezier is an additive strict entity tag.
// Existing files are not inferred/mutated. Fixed non-rational convention,
// explicit closed endpoint/control IDs; unknown fields or tags refuse.
// Neutral resolved cubic metadata is optional/absent on legacy profile wire.
// Coupled followers use mandatory guarded joint wire records; current decoding
// validates the exact association, old motion enums reject the new tag.
// MCAD-P13-007 / ADR-056: absent couplings default empty and serialize absent.
// No relation, phase, motion or coordinate units inferred; derived poses transient.
// MCAD-P13-006 / ADR-055: absent exploded_views default to empty, omitted
// on serialization. No offsets inferred; old/new coordinates stay mechanical.
// MCAD-P12-005: absent sheet.title_block stays None; no inferred metadata.
// MCAD-P12-004: absent view.detail stays None; no implicit parent/crop bindings.
// MCAD-P12-003: absent view.section stays None; never infer persisted contours.
// MCAD-P12-002: absent radial_dimensions defaults empty; never infer circle witnesses.
//! Schema version migrations (Task-116+).
// MCAD-P13-005: projected_sketch is an additive graph-owned binding tag.
// Old authored sketches remain unchanged; never infer projection associations
// or serialize derived output coordinates as source. Generated FeatureOutput
// sketches are transient regeneration data; old outputs default to None.
// MCAD-P13-004: ellipse is an additive analytic entity tag with required axes
// and angle; never infer ellipses from old circles/polygons. Resolved ellipse
// parameters are transient and skipped on the wire. Unknown tags must be
// refused by older readers; current old/new round trips preserve source IDs.
// MCAD-P13-002: primitive entity IDs/type/fields retain their wire meanings.
// replace_sketch_entity and solved-point expected effects are additive patch
// tags; legacy documents/arc defaults remain unchanged. No association inferred.
// MCAD-P13-001: construction remains the existing default-false entity field;
// the additive patch operation does not infer flags or alter old source IDs.
// MCAD-P12-010: imported_mesh is additive and requires an explicit source_unit.
// Legacy STEP sources retain their meaning; never infer a mesh or unit. Older
// readers must refuse the unknown feature tag. Round trips cover both variants.

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
// ADR-061 additive migration: absent context_constraints remains empty. A
// flexible_assembly discriminator is mandatory for constrained components;
// readers predating P13-012 refuse instead of dropping the constraints.
pub fn migrate_to_current(mut doc: OcadDocument) -> Result<OcadDocument> {
    if let Some(assembly) = doc.assembly.take() {
        doc.assembly = Some(assembly.sorted_deterministic());
    }
    if let Some(drawing) = doc.drawing.take() {
        doc.drawing = Some(drawing.sorted_deterministic());
    }
    Ok(doc)
}
