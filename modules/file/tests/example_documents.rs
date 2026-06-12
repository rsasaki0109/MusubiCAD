//! Committed example documents round-trip and regenerate.

use opencad_feature::FeatureRegistry;
use opencad_file::{read_expanded_dir, validate_expanded_dir};
use opencad_geometry::MockGeometryKernel;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn example_bracket_document_is_valid() {
    let path = workspace_root().join("examples/bracket.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = MockGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    assert!(model.active_body().is_some());
}
