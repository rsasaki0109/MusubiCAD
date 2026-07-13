//! Git-friendly `.ocad.d` directory format (Task-110+).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use opencad_assembly::AssemblyModel;
use opencad_core::{DocumentMetadata, OpenCadError, Result};
use opencad_drawing::DrawingModel;
use opencad_geometry::TopoRef;
use opencad_sketch::{Constraint, Sketch};
use serde::{Deserialize, Serialize};

use crate::checksums::ChecksumManifest;
use crate::document::OcadDocument;
use crate::manifest::{manifest_for_document, MANIFEST_FILE};
use crate::serialize::to_canonical_json;

pub const DOCUMENT_FILE: &str = "document.ocad.json";
pub const CHECKSUMS_FILE: &str = "checksums.json";

const GRAPH_DIR: &str = "graph";
const PARAMETERS_FILE: &str = "graph/parameters.json";
const SKETCHES_FILE: &str = "graph/sketches.json";
const CONSTRAINTS_FILE: &str = "graph/constraints.json";
const FEATURES_FILE: &str = "graph/features.json";
const ASSEMBLIES_FILE: &str = "graph/assemblies.json";
const MATERIALS_FILE: &str = "graph/materials.json";
const SEMANTIC_REFS_FILE: &str = "graph/semantic_refs.json";
const DRAWINGS_FILE: &str = "graph/drawings.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DocumentEnvelope {
    schema: String,
    document: DocumentMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SketchesFile {
    sketches: Vec<Sketch>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ConstraintEntry {
    sketch_id: String,
    constraint: Constraint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ConstraintsFile {
    constraints: Vec<ConstraintEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FeaturesFile {
    feature_graph: opencad_graph::FeatureGraph,
    feature_nodes: Vec<opencad_feature::FeatureNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct AssembliesFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    assembly: Option<AssemblyModel>,
    /// Legacy placeholder from pre-M3.1 manifests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    assemblies: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MaterialsFile {
    materials: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DrawingsFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    drawing: Option<DrawingModel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SemanticRefsFile {
    semantic_refs: Vec<TopoRef>,
}

/// Write a document to an expanded `.ocad.d` directory.
pub fn write_expanded_dir(path: impl AsRef<Path>, doc: &OcadDocument) -> Result<()> {
    let path = path.as_ref();
    fs::create_dir_all(path.join(GRAPH_DIR)).map_err(io_error)?;
    let files = serialize_document_files(doc)?;
    for (relative, bytes) in &files {
        let file_path = path.join(relative);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(io_error)?;
        }
        fs::write(&file_path, bytes).map_err(io_error)?;
    }
    Ok(())
}

/// Read a document from an expanded `.ocad.d` directory.
pub fn read_expanded_dir(path: impl AsRef<Path>) -> Result<OcadDocument> {
    let path = path.as_ref();
    let files = read_directory_files(path)?;
    parse_document_files(&files)
}

/// Validate checksums and parse a document from an expanded directory.
pub fn validate_expanded_dir(path: impl AsRef<Path>) -> Result<OcadDocument> {
    let path = path.as_ref();
    let files = read_directory_files(path)?;
    if let Some(bytes) = files.get(CHECKSUMS_FILE) {
        let manifest: ChecksumManifest = serde_json::from_slice(bytes)?;
        manifest.verify(&files)?;
    }
    parse_document_files(&files)
}

pub fn serialize_document_files(doc: &OcadDocument) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();

    let manifest = manifest_for_document(doc);
    files.insert(
        MANIFEST_FILE.into(),
        to_canonical_json(&manifest)?.into_bytes(),
    );

    let envelope = DocumentEnvelope {
        schema: "opencad.document.v0.1".into(),
        document: doc.metadata.clone(),
    };
    files.insert(
        DOCUMENT_FILE.into(),
        to_canonical_json(&envelope)?.into_bytes(),
    );

    files.insert(
        PARAMETERS_FILE.into(),
        to_canonical_json(&doc.parameters)?.into_bytes(),
    );

    let sketches = SketchesFile {
        sketches: doc.sketches.clone(),
    };
    files.insert(
        SKETCHES_FILE.into(),
        to_canonical_json(&sketches)?.into_bytes(),
    );

    let constraints = ConstraintsFile {
        constraints: extract_constraints(&doc.sketches),
    };
    files.insert(
        CONSTRAINTS_FILE.into(),
        to_canonical_json(&constraints)?.into_bytes(),
    );

    let features = FeaturesFile {
        feature_graph: doc.feature_graph.clone(),
        feature_nodes: doc.feature_nodes.clone(),
    };
    files.insert(
        FEATURES_FILE.into(),
        to_canonical_json(&features)?.into_bytes(),
    );

    files.insert(
        ASSEMBLIES_FILE.into(),
        to_canonical_json(&AssembliesFile {
            assembly: doc.assembly.clone(),
            assemblies: Vec::new(),
        })?
        .into_bytes(),
    );
    files.insert(
        MATERIALS_FILE.into(),
        to_canonical_json(&MaterialsFile {
            materials: Vec::new(),
        })?
        .into_bytes(),
    );
    files.insert(
        DRAWINGS_FILE.into(),
        to_canonical_json(&DrawingsFile {
            drawing: doc.drawing.clone(),
        })?
        .into_bytes(),
    );
    files.insert(
        SEMANTIC_REFS_FILE.into(),
        to_canonical_json(&SemanticRefsFile {
            semantic_refs: doc.semantic_refs.clone(),
        })?
        .into_bytes(),
    );

    let checksums = ChecksumManifest::compute(&files);
    files.insert(
        CHECKSUMS_FILE.into(),
        to_canonical_json(&checksums)?.into_bytes(),
    );

    Ok(files)
}

pub(crate) fn parse_document_files(files: &BTreeMap<String, Vec<u8>>) -> Result<OcadDocument> {
    let envelope: DocumentEnvelope = read_json(files, DOCUMENT_FILE)?;
    let parameters = read_json(files, PARAMETERS_FILE).unwrap_or_default();
    let sketches_file: SketchesFile = read_json(files, SKETCHES_FILE)?;
    let features: FeaturesFile = read_json(files, FEATURES_FILE)?;
    let semantic_refs: SemanticRefsFile =
        read_json(files, SEMANTIC_REFS_FILE).unwrap_or(SemanticRefsFile {
            semantic_refs: Vec::new(),
        });
    let assemblies: AssembliesFile = read_json(files, ASSEMBLIES_FILE).unwrap_or(AssembliesFile {
        assembly: None,
        assemblies: Vec::new(),
    });
    let drawings: DrawingsFile =
        read_json(files, DRAWINGS_FILE).unwrap_or(DrawingsFile { drawing: None });

    Ok(OcadDocument {
        metadata: envelope.document,
        parameters,
        sketches: sketches_file.sketches,
        feature_graph: features.feature_graph,
        feature_nodes: features.feature_nodes,
        semantic_refs: semantic_refs.semantic_refs,
        assembly: assemblies.assembly,
        drawing: drawings.drawing,
    })
}

fn extract_constraints(sketches: &[Sketch]) -> Vec<ConstraintEntry> {
    let mut constraints = Vec::new();
    for sketch in sketches {
        for constraint in &sketch.constraints {
            constraints.push(ConstraintEntry {
                sketch_id: sketch.id.as_str().to_string(),
                constraint: constraint.clone(),
            });
        }
    }
    constraints.sort_by(|a, b| a.constraint.id().as_str().cmp(b.constraint.id().as_str()));
    constraints
}

fn read_json<T: for<'de> Deserialize<'de>>(
    files: &BTreeMap<String, Vec<u8>>,
    path: &str,
) -> Result<T> {
    let bytes = files
        .get(path)
        .ok_or_else(|| OpenCadError::not_found(format!("missing file '{path}'")))?;
    serde_json::from_slice(bytes).map_err(Into::into)
}

fn read_directory_files(root: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    collect_files(root, root, &mut files)?;
    Ok(files)
}

fn collect_files(root: &Path, current: &Path, out: &mut BTreeMap<String, Vec<u8>>) -> Result<()> {
    for entry in fs::read_dir(current).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| OpenCadError::validation("invalid path prefix"))?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(&path).map_err(io_error)?;
        out.insert(relative, bytes);
    }
    Ok(())
}

fn io_error(err: std::io::Error) -> OpenCadError {
    OpenCadError::Other(err.to_string())
}

/// Returns true when the path looks like an expanded `.ocad.d` directory.
pub fn is_expanded_dir(path: &Path) -> bool {
    path.is_dir() && path.join(MANIFEST_FILE).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::DocumentId;
    use opencad_feature::bracket_base_plate;

    fn bracket_document() -> OcadDocument {
        let part = bracket_base_plate().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket Base Plate",
        );
        OcadDocument::from_part_model(metadata, &part)
    }

    #[test]
    fn expanded_dir_round_trip() {
        let doc = bracket_document();
        let dir = tempfile::tempdir().expect("tempdir");
        write_expanded_dir(dir.path(), &doc).expect("write");
        let restored = read_expanded_dir(dir.path()).expect("read");
        assert_eq!(doc.metadata, restored.metadata);
        assert_eq!(doc.sketches.len(), restored.sketches.len());
        assert_eq!(doc.feature_nodes.len(), restored.feature_nodes.len());
        assert_eq!(doc.sketches, restored.sketches);
        assert_eq!(doc.feature_nodes, restored.feature_nodes);
    }

    #[test]
    fn semantic_refs_round_trip() {
        use opencad_core::TopoRefId;
        use opencad_geometry::TopoRef;

        let mut doc = bracket_document();
        doc.semantic_refs = vec![TopoRef::kernel_face(
            TopoRefId::new("ref:face:bracket_top").expect("id"),
            "feature:extrude_base",
            "top",
            42,
            [0.0, 0.0, 1.0],
        )];
        let dir = tempfile::tempdir().expect("tempdir");
        write_expanded_dir(dir.path(), &doc).expect("write");
        let restored = read_expanded_dir(dir.path()).expect("read");
        assert_eq!(restored.semantic_refs.len(), 1);
        assert_eq!(
            restored.semantic_refs[0].ref_id.as_str(),
            "ref:face:bracket_top"
        );
        assert_eq!(restored.semantic_refs[0].kernel_face_id(), Some(42));
    }

    #[test]
    fn validate_checks_checksums() {
        let doc = bracket_document();
        let dir = tempfile::tempdir().expect("tempdir");
        write_expanded_dir(dir.path(), &doc).expect("write");
        validate_expanded_dir(dir.path()).expect("validate");

        let tampered = dir.path().join(SKETCHES_FILE);
        fs::write(&tampered, br#"{"sketches":[]}"#).expect("tamper");
        assert!(validate_expanded_dir(dir.path()).is_err());
    }

    #[test]
    fn assembly_round_trip() {
        use opencad_assembly::{AssemblyModel, Component, Instance, Placement};
        use opencad_core::{ComponentId, DocumentId, DocumentMetadata, InstanceId};

        let mut doc = bracket_document();
        doc.metadata = DocumentMetadata::new_assembly(
            DocumentId::new("doc:assembly_two_brackets").expect("id"),
            "Two Brackets Assembly",
        );
        doc.assembly = Some(
            AssemblyModel {
                components: vec![Component::new(
                    ComponentId::new("component:bracket").expect("id"),
                    "parts/bracket.ocad.d",
                    DocumentId::new("doc:bracket_001").expect("id"),
                )],
                instances: vec![Instance::new(
                    InstanceId::new("instance:left").expect("id"),
                    ComponentId::new("component:bracket").expect("id"),
                    Placement::identity(),
                    "Left",
                )],
                mates: Vec::new(),
                ..Default::default()
            }
            .sorted_deterministic(),
        );

        let dir = tempfile::tempdir().expect("tempdir");
        write_expanded_dir(dir.path(), &doc).expect("write");
        let restored = read_expanded_dir(dir.path()).expect("read");
        assert_eq!(doc.assembly, restored.assembly);
    }

    #[test]
    fn legacy_empty_assemblies_file_is_compatible() {
        let doc = bracket_document();
        let dir = tempfile::tempdir().expect("tempdir");
        write_expanded_dir(dir.path(), &doc).expect("write");

        let legacy = dir.path().join(ASSEMBLIES_FILE);
        fs::write(&legacy, br#"{"assemblies":[]}"#).expect("legacy");
        let restored = read_expanded_dir(dir.path()).expect("read");
        assert!(restored.assembly.is_none());
    }

    #[test]
    fn drawing_round_trip() {
        use opencad_core::{DimensionId, DocumentMetadata, SheetId, ViewId};
        use opencad_drawing::{
            DrawingModel, DrawingView, LinearDimension, ModelReference, ProjectionKind, Sheet,
        };

        let mut doc = bracket_document();
        doc.metadata = DocumentMetadata::new_drawing(
            DocumentId::new("doc:bracket_front_view").expect("id"),
            "Bracket Front View",
        );
        doc.drawing = Some(
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
                    dimensions: vec![LinearDimension {
                        id: DimensionId::new("dim:width").expect("id"),
                        view_id: ViewId::new("view:front").expect("id"),
                        start_model_m: [0.0, 0.0, 0.0],
                        end_model_m: [0.08, 0.0, 0.0],
                        offset_m: -0.01,
                    }],
                }],
            }
            .sorted_deterministic(),
        );

        let dir = tempfile::tempdir().expect("tempdir");
        write_expanded_dir(dir.path(), &doc).expect("write");
        let restored = read_expanded_dir(dir.path()).expect("read");
        assert_eq!(doc.drawing, restored.drawing);
    }

    #[test]
    fn legacy_manifest_without_drawings_file_is_compatible() {
        let doc = bracket_document();
        let dir = tempfile::tempdir().expect("tempdir");
        write_expanded_dir(dir.path(), &doc).expect("write");

        let drawings = dir.path().join(DRAWINGS_FILE);
        fs::remove_file(&drawings).expect("remove");
        let restored = read_expanded_dir(dir.path()).expect("read");
        assert!(restored.drawing.is_none());
    }
}
