//! Round-trip and golden checks for the committed assembly example.

use std::path::{Path, PathBuf};

use opencad_assembly::{regenerate_assembly, InstanceRegenStatus};
use opencad_core::DocumentId;
use opencad_feature::{bracket_with_hole, FeatureRegistry};
use opencad_file::read_expanded_dir;
use opencad_geometry::MockGeometryKernel;
use opencad_graph::bracket_parameters;

#[test]
fn example_assembly_document_round_trip() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/assembly_two_brackets.ocad.d");
    let doc = read_expanded_dir(&path).expect("read example");
    let assembly = doc.assembly.as_ref().expect("assembly model");
    assert_eq!(assembly.components.len(), 1);
    assert_eq!(assembly.instances.len(), 2);
    assert_eq!(assembly.mates.len(), 2);
    assert_eq!(doc.metadata.kind, opencad_core::DocumentKind::Assembly);
}

#[test]
fn golden_two_bracket_instances_regen_mock() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/assembly_two_brackets.ocad.d");
    let doc = read_expanded_dir(&path).expect("read example");
    let assembly = doc.assembly.as_ref().expect("assembly");

    let kernel = MockGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let assembly_id = DocumentId::new("doc:assembly_two_brackets").expect("id");

    let mut loader = |_child_path: &Path| {
        let part = bracket_with_hole().expect("model");
        let params = bracket_parameters();
        Ok(opencad_assembly::ResolvedChild::Part(Box::new(
            opencad_assembly::ChildPart {
                parameters: params,
                part,
                semantic_refs: Vec::new(),
            },
        )))
    };
    let report = regenerate_assembly(
        assembly,
        &assembly_id,
        &path,
        &kernel,
        &registry,
        &mut loader,
    )
    .expect("regen");

    assert_eq!(report.instance_count, 2);
    assert_eq!(report.successful_instances, 2);
    assert!(report
        .instances
        .iter()
        .all(|instance| { matches!(instance.status, InstanceRegenStatus::Ok) }));

    let mass = report.scene.mass.expect("mass");
    let bbox = report.scene.bounding_box.expect("bbox");
    assert!(mass.volume_m3 > 0.0);
    assert!((bbox.max[0] - bbox.min[0]) > 0.1);
}
