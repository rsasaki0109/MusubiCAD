//! `musubicad export` command (Task-125+).

use std::fs;
use std::path::Path;

use opencad_assembly::{regenerate_assembly, tessellate_assembly_scene, ChildPart, ResolvedChild};
use opencad_core::DocumentKind;
use opencad_core::{OpenCadError, Result};
use opencad_drawing::{render_sheet_svg, validate_svg, ModelReference, ViewMesh};
use opencad_feature::FeatureRegistry;
use opencad_file::{read_ocad, OcadDocument};
use opencad_geometry::{write_binary_stl, TessellationSettings};
use serde::{Deserialize, Serialize};

#[cfg(feature = "occt")]
use opencad_kernel_occt::OcctGeometryKernel;

pub use opencad_desktop::tessellate_active_body_detailed;

/// Summary printed by `musubicad export`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportSummary {
    pub format: String,
    /// Triangles written (STL, URDF meshes), drawing segments (SVG), or 0 (STEP).
    pub triangles: usize,
    pub output: String,
    /// Bytes written, for formats without a triangle count (STEP).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<usize>,
}

pub fn export_document(input: &str, output: &str) -> Result<ExportSummary> {
    let extension = Path::new(output)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("stl") => export_stl(input, output),
        Some("svg") => export_svg(input, output),
        Some("step" | "stp") => export_step(input, output),
        Some("urdf") => crate::urdf::export_urdf(input, output),
        Some("3mf") => export_mesh_package(input, output, "3mf"),
        Some("glb") => export_mesh_package(input, output, "glb"),
        Some("html") => crate::web_viewer::export_web_viewer(input, output),
        _ => Err(OpenCadError::validation(
            "export output must use .stl, .3mf, .glb, .step/.stp, .svg, .urdf, or .html extension",
        )),
    }
}

pub fn export_stl(input: &str, output: &str) -> Result<ExportSummary> {
    let doc = read_ocad(input)?;
    let name = doc.metadata.name.clone();
    let output_path = Path::new(output);
    if !output_path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("stl"))
    {
        return Err(OpenCadError::validation(
            "export output must use .stl extension",
        ));
    }

    let mesh = document_mesh(input, doc)?;

    write_binary_stl(output_path, &mesh, &name)?;
    Ok(ExportSummary {
        format: "stl".into(),
        triangles: mesh.triangle_count(),
        output: output.to_string(),
        bytes: None,
    })
}

/// Regenerated mesh of a part's active body, or of every placed assembly instance.
fn document_mesh(input: &str, doc: OcadDocument) -> Result<opencad_geometry::MeshSet> {
    if let Some(assembly) = &doc.assembly {
        return export_assembly_mesh(input, doc.metadata.id.as_str(), assembly);
    }
    let parameters = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    opencad_desktop::tessellate_active_body(&mut model, Some(&parameters), Some(&semantic_refs))
}

/// Named meshes: the part itself, or one placed mesh per assembly instance.
pub(crate) fn document_meshes(
    input: &str,
    doc: OcadDocument,
) -> Result<Vec<(String, opencad_geometry::MeshSet)>> {
    if doc.assembly.is_none() {
        let name = doc.metadata.name.clone();
        return Ok(vec![(name, document_mesh(input, doc)?)]);
    }
    Ok(assembly_instance_meshes(input, &doc)?
        .into_iter()
        .map(|(_, name, mesh)| (name, mesh))
        .collect())
}

/// Instance ID, name, and placed mesh of every assembly instance with a body.
pub(crate) fn assembly_instance_meshes(
    input: &str,
    doc: &OcadDocument,
) -> Result<Vec<(opencad_core::InstanceId, String, opencad_geometry::MeshSet)>> {
    let assembly = doc
        .assembly
        .as_ref()
        .ok_or_else(|| OpenCadError::validation("document is not an assembly"))?;
    #[cfg(feature = "occt")]
    {
        let kernel = OcctGeometryKernel::new();
        let report = regenerate_assembly(
            assembly,
            &doc.metadata.id,
            &assembly_root(input),
            &kernel,
            &FeatureRegistry::with_defaults(),
            &mut load_child_document,
        )?;
        let meshes = opencad_assembly::tessellate_assembly_instances(
            &kernel,
            &report.scene,
            &TessellationSettings::default(),
        )?;
        Ok(meshes
            .into_iter()
            .map(|instance| {
                let name = assembly
                    .instances
                    .iter()
                    .find(|candidate| candidate.id == instance.instance_id)
                    .map(|candidate| candidate.name.clone())
                    .unwrap_or_else(|| instance.instance_id.as_str().to_string());
                (instance.instance_id, name, instance.mesh_set)
            })
            .collect())
    }
    #[cfg(not(feature = "occt"))]
    {
        let _ = (input, assembly);
        Err(OpenCadError::Other(
            "assembly export requires OCCT; rebuild with --features occt".into(),
        ))
    }
}

/// Export a part or assembly mesh as 3MF (millimetres, welded, for slicers)
/// or GLB (binary glTF 2.0 in metres, for viewers and the web).
pub fn export_mesh_package(input: &str, output: &str, format: &str) -> Result<ExportSummary> {
    let doc = read_ocad(input)?;
    let name = doc.metadata.name.clone();
    let objects = document_meshes(input, doc)?;
    let mesh = opencad_geometry::MeshSet::merge(
        &objects
            .iter()
            .map(|(_, mesh)| mesh.clone())
            .collect::<Vec<_>>(),
    );
    let bytes = match format {
        "3mf" => crate::mesh_formats::encode_3mf(&name, &objects)?,
        "glb" => crate::mesh_formats::encode_glb(&mesh, &name)?,
        other => {
            return Err(OpenCadError::validation(format!(
                "unknown mesh package format '{other}'"
            )))
        }
    };
    fs::write(output, &bytes)
        .map_err(|error| OpenCadError::Other(format!("cannot write '{output}': {error}")))?;
    Ok(ExportSummary {
        format: format.into(),
        triangles: mesh.triangle_count(),
        output: output.to_string(),
        bytes: Some(bytes.len()),
    })
}

pub fn export_svg(input: &str, output: &str) -> Result<ExportSummary> {
    let doc = read_ocad(input)?;
    let output_path = Path::new(output);
    if !output_path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
    {
        return Err(OpenCadError::validation(
            "export output must use .svg extension",
        ));
    }

    let (svg, segments) = render_drawing_svg(input, &doc)?;
    fs::write(output_path, svg).map_err(|err| OpenCadError::Other(err.to_string()))?;

    Ok(ExportSummary {
        format: "svg".into(),
        triangles: segments,
        output: output.to_string(),
        bytes: None,
    })
}

/// Export a part or assembly as millimetre STEP (AP214) B-rep geometry.
///
/// Parts export their regenerated active body; assemblies export every
/// placed instance as one compound.  Output is deterministic.
pub fn export_step(input: &str, output: &str) -> Result<ExportSummary> {
    let doc = read_ocad(input)?;
    if doc.drawing.is_some() {
        return Err(OpenCadError::validation(
            "drawings export as SVG; STEP export needs a part or assembly",
        ));
    }

    #[cfg(feature = "occt")]
    {
        use opencad_geometry::GeometryKernel;

        let kernel = OcctGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let body =
            if let Some(assembly) = &doc.assembly {
                let assembly_id = opencad_core::DocumentId::new(doc.metadata.id.as_str())?;
                let report = regenerate_assembly(
                    assembly,
                    &assembly_id,
                    &assembly_root(input),
                    &kernel,
                    &registry,
                    &mut load_child_document,
                )?;
                report.scene.compound_body.ok_or_else(|| {
                    OpenCadError::validation("assembly has no placed solid to export")
                })?
            } else {
                let parameters = doc.parameters.clone();
                let semantic_refs = doc.semantic_refs.clone();
                let mut model = doc.into_part_model();
                model.regenerate(&kernel, &registry, Some(&parameters), Some(&semantic_refs))?;
                model.active_body().cloned().ok_or_else(|| {
                    OpenCadError::validation("document has no solid body to export")
                })?
            };
        let step = kernel.export_step(&body)?;
        fs::write(output, &step).map_err(|err| OpenCadError::Other(err.to_string()))?;
        Ok(ExportSummary {
            format: "step".into(),
            triangles: 0,
            output: output.to_string(),
            bytes: Some(step.len()),
        })
    }

    #[cfg(not(feature = "occt"))]
    {
        let _ = (doc, output);
        Err(OpenCadError::Other(
            "STEP export requires OCCT; rebuild with --features occt".into(),
        ))
    }
}

pub(crate) fn render_drawing_svg(input: &str, doc: &OcadDocument) -> Result<(String, usize)> {
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
    let segments = opencad_drawing::build_sheet_segments(sheet, &view_meshes)?.len();
    Ok((svg, segments))
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
    match doc.metadata.kind {
        DocumentKind::Assembly => {
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
        }
        DocumentKind::Part => {
            let doc_id = doc.metadata.id.clone();
            let parameters = doc.parameters.clone();
            let semantic_refs = doc.semantic_refs.clone();
            let part = doc.into_part_model();
            Ok(ResolvedChild::Part(Box::new(ChildPart {
                doc_id,
                parameters,
                part,
                semantic_refs,
            })))
        }
        DocumentKind::Drawing => Err(opencad_core::OpenCadError::validation(format!(
            "child document '{}' has drawing kind; expected part or assembly",
            path.display()
        ))),
    }
}

pub fn print_summary(summary: &ExportSummary) {
    println!("exported: {}", summary.output);
    println!("format: {}", summary.format);
    if summary.format == "svg" {
        println!("wire segments: {}", summary.triangles);
    } else if let Some(bytes) = summary.bytes {
        println!("bytes: {bytes}");
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

    /// Every exported 3MF must be a closed, manifold surface whose enclosed
    /// volume matches the regenerated solid; slicers reject anything else.
    #[test]
    fn three_mf_exports_of_examples_are_closed_and_match_the_solid_volume() {
        use std::collections::HashMap;
        use std::io::Read;

        let dir = tempdir().expect("tempdir");
        for example in [
            "bearing_carrier.ocad.d",
            "robot_joint_actuator.ocad.d",
            "robot_arm_assembly.ocad.d",
        ] {
            let input = format!("{}/../../examples/{example}", env!("CARGO_MANIFEST_DIR"));
            let output = dir.path().join(format!("{example}.3mf"));
            let summary = export_document(&input, output.to_str().expect("path")).expect("3mf");
            assert_eq!(summary.format, "3mf");

            let file = fs::File::open(&output).expect("open");
            let mut archive = zip::ZipArchive::new(file).expect("zip");
            let mut model = String::new();
            archive
                .by_name("3D/3dmodel.model")
                .expect("model")
                .read_to_string(&mut model)
                .expect("read");
            let attribute = |line: &str, key: &str| -> f64 {
                let start = line.find(&format!(" {key}=\"")).expect(key) + key.len() + 3;
                line[start..]
                    .split('"')
                    .next()
                    .expect("value")
                    .parse()
                    .expect("number")
            };
            // Vertex indices restart in every object; check each one separately.
            let mut volume_mm3 = 0.0;
            let objects: Vec<&str> = model.split("<object ").skip(1).collect();
            assert!(!objects.is_empty(), "{example}: no objects");
            for object in objects {
                let vertices: Vec<[f64; 3]> = object
                    .lines()
                    .filter(|line| line.trim_start().starts_with("<vertex "))
                    .map(|line| {
                        [
                            attribute(line, "x"),
                            attribute(line, "y"),
                            attribute(line, "z"),
                        ]
                    })
                    .collect();
                let triangles: Vec<[usize; 3]> = object
                    .lines()
                    .filter(|line| line.trim_start().starts_with("<triangle "))
                    .map(|line| {
                        [
                            attribute(line, "v1") as usize,
                            attribute(line, "v2") as usize,
                            attribute(line, "v3") as usize,
                        ]
                    })
                    .collect();
                let mut directed: HashMap<(usize, usize), usize> = HashMap::new();
                for [a, b, c] in &triangles {
                    for edge in [(*a, *b), (*b, *c), (*c, *a)] {
                        *directed.entry(edge).or_default() += 1;
                    }
                }
                // Closed and consistently oriented: each directed edge appears
                // once and its reverse appears once.
                for (&(from, to), &count) in &directed {
                    assert_eq!(count, 1, "{example}: edge {from}->{to} used {count} times");
                    assert_eq!(
                        directed.get(&(to, from)),
                        Some(&1),
                        "{example}: open edge {from}->{to}"
                    );
                }
                // Divergence theorem: signed volume of the closed mesh.
                volume_mm3 += triangles
                    .iter()
                    .map(|[a, b, c]| {
                        let (p, q, r) = (vertices[*a], vertices[*b], vertices[*c]);
                        (p[0] * (q[1] * r[2] - q[2] * r[1]) - p[1] * (q[0] * r[2] - q[2] * r[0])
                            + p[2] * (q[0] * r[1] - q[1] * r[0]))
                            / 6.0
                    })
                    .sum::<f64>();
            }
            let solid_m3 = crate::regen::regen_document(&input, false)
                .expect("regen")
                .volume_m3
                .expect("volume");
            let relative = (volume_mm3 * 1e-9 - solid_m3).abs() / solid_m3;
            // Chordal tessellation of curved faces under-fills slightly.
            assert!(
                relative < 0.01,
                "{example}: mesh {volume_mm3} mm³ vs solid {solid_m3} m³"
            );
        }
    }

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
    fn robot_arm_assembly_drawing_exports_deterministic_svg() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|parent| parent.parent())
            .expect("workspace root");
        let path = root.join("examples/robot_arm_assembly_drawing.ocad.d");
        let doc = read_ocad(&path).expect("read drawing");

        let (first, segments) =
            render_drawing_svg(path.to_str().expect("path"), &doc).expect("render");
        let (second, _) =
            render_drawing_svg(path.to_str().expect("path"), &doc).expect("render second");
        assert_eq!(first, second, "drawing SVG must be deterministic");
        assert!(segments > 0, "arm drawing must produce wire segments");
        assert!(
            first.contains("160.00 mm"),
            "arm drawing must carry the model-driven upper-arm dimension"
        );
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
                    dimensions: Vec::new(),
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
