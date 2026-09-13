//! Round-trip and Mock regeneration checks for the committed robot-arm assembly.

use std::path::{Path, PathBuf};

use opencad_assembly::{regenerate_assembly, ChildPart, InstanceRegenStatus, ResolvedChild};
use opencad_core::DocumentId;
use opencad_feature::FeatureRegistry;
use opencad_file::read_expanded_dir;
use opencad_geometry::MockGeometryKernel;

fn example_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/robot_arm_assembly.ocad.d")
}

#[test]
fn example_robot_arm_assembly_round_trip() {
    let path = example_root();
    let doc = read_expanded_dir(&path).expect("read example");
    let assembly = doc.assembly.as_ref().expect("assembly model");
    assert_eq!(assembly.components.len(), 4);
    assert_eq!(assembly.instances.len(), 4);
    assert_eq!(assembly.mates.len(), 4);
    assert_eq!(assembly.connectors.len(), 6);
    assert_eq!(doc.metadata.kind, opencad_core::DocumentKind::Assembly);
    assert_eq!(doc.metadata.id.as_str(), "doc:robot_arm_assembly_001");
}

#[test]
fn example_robot_arm_assembly_regenerates_with_mock_kernel() {
    let path = example_root();
    let doc = read_expanded_dir(&path).expect("read example");
    let assembly = doc.assembly.as_ref().expect("assembly");

    let kernel = MockGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let assembly_id = DocumentId::new("doc:robot_arm_assembly_001").expect("id");

    let mut loader = |child_path: &Path| {
        let child = read_expanded_dir(child_path).expect("read child");
        let doc_id = child.metadata.id.clone();
        let parameters = child.parameters.clone();
        let part = child.into_part_model();
        Ok(ResolvedChild::Part(Box::new(ChildPart {
            doc_id,
            parameters,
            part,
            semantic_refs: Vec::new(),
        })))
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

    assert_eq!(report.instance_count, 4);
    assert_eq!(report.successful_instances, 4);
    assert!(report
        .instances
        .iter()
        .all(|instance| matches!(instance.status, InstanceRegenStatus::Ok)));

    let mass = report.scene.mass.expect("mass");
    let bbox = report.scene.bounding_box.expect("bbox");
    assert!(mass.volume_m3 > 0.0);
    assert!(bbox.max[1] - bbox.min[1] > 0.1);
    let solve = report.mate_solve.expect("mate solve report");
    assert!(
        solve.max_error < 1e-6,
        "mate solve max error {}",
        solve.max_error
    );
}
