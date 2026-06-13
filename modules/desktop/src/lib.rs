//! Shared helpers for ForgeCAD desktop and CLI shells.

pub mod fixture;
pub mod inspect;
pub mod parameters;
pub mod preview;
pub mod regen;
pub mod template;

pub use inspect::{inspect_document, DocumentInspect};
pub use parameters::{
    list_document_parameters, set_document_parameter, ParameterRow,
};
pub use preview::{
    load_view_data, preview_document, DocumentPreview, ViewData, PREVIEW_HEIGHT, PREVIEW_WIDTH,
};
pub use regen::{
    tessellate_active_body, tessellate_active_body_detailed, TessellatedBody,
};
pub use template::{create_document, DocumentTemplate};

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
