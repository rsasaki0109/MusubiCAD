//! `opencad export` command (Task-125+).

use std::fs;
use std::path::Path;

use opencad_assembly::{regenerate_assembly, tessellate_assembly_scene, ChildPart, ResolvedChild};
use opencad_core::DocumentKind;
use opencad_core::{OpenCadError, Result};
use opencad_drawing::{render_sheet_svg, validate_svg, ModelReference, ViewMesh};
use opencad_feature::FeatureRegistry;
use opencad_file::read_ocad;
use opencad_geometry::{write_binary_stl, TessellationSettings};
use serde::{Deserialize, Serialize};

#[cfg(feature = "occt")]
use opencad_kernel_occt::OcctGeometryKernel;

pub use opencad_desktop::tessellate_active_body_detailed;

/// Summary printed by `opencad export`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportSummary {
    pub format: String,
    pub triangles: usize,
    pub output: String,
}

pub fn export_document(input: &str, output: &str) -> Result<ExportSummary> {
    match Path::new(output).extension().and_then(|ext| ext.to_str()) {
        Some("stl") => export_stl(input, output),
        Some("svg") => export_svg(input, output),
        _ => Err(OpenCadError::validation(
            "export output must use .stl or .svg extension",
        )),
    }
}

pub fn export_stl(input: &str, output: &str) -> Result<ExportSummary> {
    let doc = read_ocad(input)?;
    let name = doc.metadata.name.clone();
    let output_path = Path::new(output);
    if output_path.extension().and_then(|s| s.to_str()) != Some("stl") {
        return Err(OpenCadError::validation(
            "export output must use .stl extension",
        ));
    }

    let mesh = if let Some(assembly) = &doc.assembly {
        export_assembly_mesh(input, doc.metadata.id.as_str(), assembly)?
    } else {
        let parameters = doc.parameters.clone();
        let semantic_refs = doc.semantic_refs.clone();
        let mut model = doc.into_part_model();
        opencad_desktop::tessellate_active_body(
            &mut model,
            Some(&parameters),
            Some(&semantic_refs),
        )?
    };

    write_binary_stl(output_path, &mesh, &name)?;
    Ok(ExportSummary {
        format: "stl".into(),
        triangles: mesh.triangle_count(),
        output: output.to_string(),
    })
}

pub fn export_svg(input: &str, output: &str) -> Result<ExportSummary> {
    let doc = read_ocad(input)?;
    let output_path = Path::new(output);
    if output_path.extension().and_then(|s| s.to_str()) != Some("svg") {
        return Err(OpenCadError::validation(
            "export output must use .svg extension",
        ));
    }

    let drawing = doc
        .drawing
        .as_ref()
        .ok_or_else(|| OpenCadError::validation("document has no drawing model to export"))?;
    let sheet = drawing
        .sheets
        .first()
        .ok_or_else(|| OpenCadError::validation("drawing document has no sheets"))?;

    let drawing_root = document_root(input);
    let mut view_meshes = Vec::new();
    for view in &sheet.views {
        let mesh = tessellate_model_reference(&drawing_root, &view.model)?;
        view_meshes.push(ViewMesh {
            view_id: view.id.clone(),
            mesh_set: mesh,
        });
    }

    let svg = render_sheet_svg(sheet, &view_meshes)?;
    validate_svg(&svg)?;
    fs::write(output_path, svg).map_err(|err| OpenCadError::Other(err.to_string()))?;

    let segments = opencad_drawing::build_sheet_segments(sheet, &view_meshes)?.len();
    Ok(ExportSummary {
        format: "svg".into(),
        triangles: segments,
        output: output.to_string(),
    })
}

fn tessellate_model_reference(
    drawing_root: &Path,
    reference: &ModelReference,
) -> Result<opencad_geometry::MeshSet> {
    let path = drawing_root.join(&reference.source_path);
    let doc = read_ocad(&path)?;
    if doc.metadata.id != reference.source_doc {
        return Err(OpenCadError::validation(format!(
            "model reference '{}' expected document '{}' but found '{}'",
            reference.source_path, reference.source_doc, doc.metadata.id
        )));
    }

    match doc.metadata.kind {
        DocumentKind::Assembly => {
            let assembly = doc.assembly.ok_or_else(|| {
                OpenCadError::validation(format!(
                    "assembly document '{}' is missing assembly model",
                    path.display()
                ))
            })?;
            export_assembly_mesh(
                path.to_str().unwrap_or("."),
                doc.metadata.id.as_str(),
                &assembly,
            )
        }
        DocumentKind::Part | DocumentKind::Drawing => {
            let parameters = doc.parameters.clone();
            let semantic_refs = doc.semantic_refs.clone();
            let mut model = doc.into_part_model();
            opencad_desktop::tessellate_active_body(
                &mut model,
                Some(&parameters),
                Some(&semantic_refs),
            )
        }
    }
}

fn document_root(path: &str) -> std::path::PathBuf {
    let path = Path::new(path);
    if path.extension().and_then(|ext| ext.to_str()) == Some("ocad") {
        path.parent()
            .map(|parent| parent.to_path_buf())
            .unwrap_or_else(|| Path::new(".").to_path_buf())
    } else {
        path.to_path_buf()
    }
}

fn export_assembly_mesh(
    input: &str,
    assembly_doc_id: &str,
    assembly: &opencad_assembly::AssemblyModel,
) -> Result<opencad_geometry::MeshSet> {
    let registry = FeatureRegistry::with_defaults();
    let assembly_root = assembly_root(input);
    let assembly_id = opencad_core::DocumentId::new(assembly_doc_id)?;

    #[cfg(feature = "occt")]
    {
        let kernel = OcctGeometryKernel::new();
        let report = regenerate_assembly(
            assembly,
            &assembly_id,
            &assembly_root,
            &kernel,
            &registry,
            &mut load_child_document,
        )?;
        tessellate_assembly_scene(&kernel, &report.scene, &TessellationSettings::default())
    }

    #[cfg(not(feature = "occt"))]
    {
        let _ = (registry, assembly_root, assembly_id);
        Err(OpenCadError::Other(
            "assembly export requires OCCT; rebuild with --features occt".into(),
        ))
    }
}

fn assembly_root(path: &str) -> std::path::PathBuf {
    let path = Path::new(path);
    if path.extension().and_then(|ext| ext.to_str()) == Some("ocad") {
        path.parent()
            .map(|parent| parent.to_path_buf())
            .unwrap_or_else(|| Path::new(".").to_path_buf())
    } else {
        path.to_path_buf()
    }
}

fn load_child_document(path: &Path) -> Result<ResolvedChild> {
    let doc = read_ocad(path)?;
    if doc.metadata.kind == DocumentKind::Assembly {
        let assembly = doc.assembly.ok_or_else(|| {
            opencad_core::OpenCadError::validation(format!(
                "assembly document '{}' is missing assembly model",
                path.display()
            ))
        })?;
        Ok(ResolvedChild::Assembly {
            model: Box::new(assembly),
            doc_id: doc.metadata.id,
        })
    } else {
        let parameters = doc.parameters.clone();
        let semantic_refs = doc.semantic_refs.clone();
        let part = doc.into_part_model();
        Ok(ResolvedChild::Part(Box::new(ChildPart {
            parameters,
            part,
            semantic_refs,
        })))
    }
}

pub fn print_summary(summary: &ExportSummary) {
    println!("exported: {}", summary.output);
    println!("format: {}", summary.format);
    if summary.format == "svg" {
        println!("wire segments: {}", summary.triangles);
    } else {
        println!("triangles: {}", summary.triangles);
    }
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

    #[test]
    fn exports_drawing_to_svg() {
        use opencad_core::{SheetId, ViewId};
        use opencad_drawing::{DrawingModel, DrawingView, ModelReference, ProjectionKind, Sheet};

        let part = bracket_base_plate().expect("model");
        let part_metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket Base Plate",
        );
        let mut part_doc = OcadDocument::from_part_model(part_metadata, &part);
        part_doc.parameters = bracket_parameters();

        let dir = tempdir().expect("tempdir");
        let drawing_path = dir.path().join("bracket_front_view.ocad.d");
        let child_path = drawing_path.join("parts/bracket.ocad.d");
        write_expanded_dir(&child_path, &part_doc).expect("write part");
        let drawing_doc = OcadDocument::from_drawing_model(
            DocumentMetadata::new_drawing(
                DocumentId::new("doc:bracket_front_view").expect("id"),
                "Bracket Front View",
            ),
            DrawingModel {
                sheets: vec![Sheet {
                    id: SheetId::new("sheet:a4").expect("id"),
                    name: "Sheet 1".into(),
                    width_m: 0.210,
                    height_m: 0.297,
                    views: vec![DrawingView::new(
                        ViewId::new("view:front").expect("id"),
                        "Front",
                        ModelReference::new(
                            "parts/bracket.ocad.d",
                            DocumentId::new("doc:bracket_001").expect("id"),
                        ),
                        ProjectionKind::Front,
                        1.0,
                        [0.05, 0.05],
                    )],
                }],
            }
            .sorted_deterministic(),
        );
        write_expanded_dir(&drawing_path, &drawing_doc).expect("write drawing");

        let output = dir.path().join("bracket_front.svg");
        let summary = export_svg(
            drawing_path.to_str().expect("path"),
            output.to_str().expect("svg"),
        )
        .expect("export");
        assert!(summary.triangles > 0);
        assert!(output.is_file());
    }
}
