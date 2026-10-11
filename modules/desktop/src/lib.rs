//! Shared helpers for MusubiCAD desktop and CLI shells.

pub mod export;
pub mod fixture;
pub mod inspect;
pub mod intent;
pub mod parameters;
pub mod patch;
pub mod pick;
pub mod preview;
pub mod regen;
pub mod regenerate;
pub mod related_parameters;
pub mod scene_query;
pub mod smoke;
pub mod template;
pub mod viewport;

pub use export::{export_stl_document, ExportSummary};
pub use inspect::{inspect_document, DocumentInspect};
pub use intent::{
    inspect_document_regeneration, inspect_ocad_regeneration, inspect_parameter_intent,
    inspect_reference_intent, RegenInspectionResult, RegenInspectionStatus,
    DEFAULT_DENSITY_KG_PER_M3,
};
pub use parameters::{
    list_document_parameters, redo_document_with_history, set_document_parameter,
    set_document_parameter_with_history, undo_document_with_history, ParameterRow,
};
pub use patch::{
    apply_patch_and_regenerate, apply_patch_and_regenerate_with_trace, PatchRegenerationResult,
};
pub use pick::{
    build_pick_summary, highlight_segments_for_camera, pick_document, preview_highlight_segments,
    PickOptions, PickSummary, PickTarget, ScreenSegment,
};
pub use preview::{
    load_assembly_evidence_from_document, load_assembly_evidence_with_assertions,
    load_assembly_scene_from_document, load_view_data, preview_document, render_preview_png,
    AssemblyEvidence, CameraState, DocumentPreview, ViewData, PREVIEW_HEIGHT, PREVIEW_WIDTH,
};
pub use regen::{tessellate_active_body, tessellate_active_body_detailed, TessellatedBody};
pub use regenerate::{regenerate_document, DocumentRegeneration};
pub use related_parameters::{
    related_parameter_candidates, related_parameter_ids, related_parameter_ids_for_features,
};
pub use smoke::{run_desktop_smoke, DesktopSmokeSummary};
pub use template::{create_document, DocumentTemplate};
pub use viewport::{run_document_viewport, run_document_viewport_with_sync, PreviewSynced};

pub use opencad_ai::{ParameterIntent, ReferenceIntent};
pub use opencad_file::{DocumentHistory, DocumentHistoryState};
pub use scene_query::{infer_face_refs, topo_ref_for_group};

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::fixture::write_bracket_fixture_at;
    use crate::{inspect_document, preview_document};

    #[test]
    fn preview_bracket_fixture_renders_png() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        let preview = preview_document(path.to_str().expect("path")).expect("preview");
        assert!(preview.triangles > 0);
        assert!(preview.vertices > 0);
        assert!(!preview.png_base64.is_empty());
        assert!(preview.bounds_max_m[0] > preview.bounds_min_m[0]);
    }

    #[test]
    fn inspect_bracket_fixture() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        let info = inspect_document(path.to_str().expect("path")).expect("inspect");
        assert_eq!(info.name, "Bracket");
        assert!(info.features > 0);
        assert!(info.sketches > 0);
    }
}
