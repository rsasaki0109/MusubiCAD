//! `opencad export` command (Task-125+).

use std::path::Path;

use opencad_core::{OpenCadError, Result};
use opencad_desktop::tessellate_active_body;
use opencad_file::read_ocad;
use opencad_geometry::write_binary_stl;
use serde::{Deserialize, Serialize};

pub use opencad_desktop::tessellate_active_body_detailed;

/// Summary printed by `opencad export`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportSummary {
    pub format: String,
    pub triangles: usize,
    pub output: String,
}

pub fn export_stl(input: &str, output: &str) -> Result<ExportSummary> {
    let doc = read_ocad(input)?;
    let name = doc.metadata.name.clone();
    let parameters = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let mesh = tessellate_active_body(&mut model, Some(&parameters), Some(&semantic_refs))?;
    let output_path = Path::new(output);
    if output_path.extension().and_then(|s| s.to_str()) != Some("stl") {
        return Err(OpenCadError::validation(
            "export output must use .stl extension",
        ));
    }
    write_binary_stl(output_path, &mesh, &name)?;
    Ok(ExportSummary {
        format: "stl".into(),
        triangles: mesh.triangle_count(),
        output: output.to_string(),
    })
}

pub fn print_summary(summary: &ExportSummary) {
    println!("exported: {}", summary.output);
    println!("format: {}", summary.format);
    println!("triangles: {}", summary.triangles);
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{DocumentId, DocumentMetadata};
    use opencad_feature::bracket_base_plate;
    use opencad_file::{write_expanded_dir, OcadDocument};
    use opencad_graph::bracket_parameters;
    use tempfile::tempdir;

    #[test]
    fn exports_bracket_to_stl() {
        let part = bracket_base_plate().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket Base Plate",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();
        let dir = tempdir().expect("tempdir");
        write_expanded_dir(dir.path(), &doc).expect("write");
        let output = dir.path().join("bracket.stl");
        let summary = export_stl(
            dir.path().to_str().expect("path"),
            output.to_str().expect("stl"),
        )
        .expect("export");
        assert!(summary.triangles > 0);
        assert!(output.is_file());
    }
}
