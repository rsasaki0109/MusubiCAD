//! ADR-013: every checked-in example is a structurally valid design state,
//! so whole-state validation can guard semantic merge results.

use std::path::PathBuf;

use opencad_ai::validate_design_state;
use opencad_file::{document_design_state, read_ocad};

#[test]
fn every_example_is_a_valid_design_state() {
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&examples)
        .expect("examples")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "d"))
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 18);
    for path in paths {
        let doc = read_ocad(&path).expect("read");
        validate_design_state(&document_design_state(&doc))
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
}
