//! Example documents are stored exactly as the current writer produces them.
//!
//! Examples written by an older writer made every later edit also carry
//! unrelated normalisation churn (a new `kind` field, an empty
//! `drawings.json`, a reformatted `assemblies.json`).  This test pins the
//! canonical form; after an intentional writer change, rewrite the examples
//! with
//!
//! ```text
//! MUSUBICAD_BLESS_EXAMPLES=1 cargo test -p opencad-file --test example_canonical_form
//! ```

use std::path::{Path, PathBuf};

use opencad_file::expanded_dir::serialize_document_files;
use opencad_file::{read_ocad, write_ocad};

fn document_dirs(root: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(root)
        .expect("read dir")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.is_dir())
        .collect();
    entries.sort();
    for path in entries {
        if path.extension().is_some_and(|ext| ext == "d") {
            found.push(path.clone());
        }
        document_dirs(&path, found);
    }
}

/// Git may check text files out with CRLF line endings.
fn normalized(bytes: &[u8]) -> Vec<u8> {
    String::from_utf8_lossy(bytes)
        .replace("\r\n", "\n")
        .into_bytes()
}

#[test]
fn example_documents_are_in_canonical_writer_form() {
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut dirs = Vec::new();
    document_dirs(&examples, &mut dirs);
    assert!(dirs.len() >= 20, "found {} example documents", dirs.len());

    let bless = std::env::var_os("MUSUBICAD_BLESS_EXAMPLES").is_some();
    let mut drift = Vec::new();
    for dir in &dirs {
        let doc = read_ocad(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
        if bless {
            write_ocad(dir, &doc).expect("rewrite");
            continue;
        }
        for (relative, bytes) in serialize_document_files(&doc).expect("serialize") {
            let on_disk = std::fs::read(dir.join(&relative)).unwrap_or_default();
            if normalized(&on_disk) != bytes {
                drift.push(format!("{}/{relative}", dir.display()));
            }
        }
    }
    assert!(
        drift.is_empty(),
        "example files differ from the canonical writer output \
         (rerun with MUSUBICAD_BLESS_EXAMPLES=1 after an intentional change):\n{}",
        drift.join("\n")
    );
}
