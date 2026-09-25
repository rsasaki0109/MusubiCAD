//! Generates `examples/robot_arm_assembly.ocad.d` (run once via
//! `cargo test -p opencad-assembly --test write_robot_arm -- --ignored`).

use std::path::PathBuf;

use opencad_assembly::{robot_arm, robot_arm_assembly_model};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::{
    robot_arm_base, robot_arm_forearm, robot_arm_gripper, robot_arm_upper_arm, PartModel,
};
use opencad_file::{write_expanded_dir, OcadDocument};
use opencad_graph::{robot_arm_base_parameters, ParamGraph};

fn part_document(
    part: PartModel,
    parameters: ParamGraph,
    doc_id: &str,
    name: &str,
) -> OcadDocument {
    let metadata = DocumentMetadata::new(DocumentId::new(doc_id).expect("doc id"), name);
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = parameters;
    doc
}

#[test]
#[ignore = "run manually to refresh examples/robot_arm_assembly.ocad.d"]
fn write_robot_arm_example() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let example_root = manifest_dir.join("../../examples/robot_arm_assembly.ocad.d");

    for (relative, doc) in [
        (
            robot_arm::BASE_PATH,
            part_document(
                robot_arm_base().expect("base"),
                robot_arm_base_parameters(),
                robot_arm::BASE_DOC,
                "Robot Arm Base",
            ),
        ),
        (
            robot_arm::UPPER_ARM_PATH,
            part_document(
                robot_arm_upper_arm().expect("upper arm"),
                opencad_graph::robot_arm_upper_arm_parameters(),
                robot_arm::UPPER_ARM_DOC,
                "Robot Arm Upper Link",
            ),
        ),
        (
            robot_arm::FOREARM_PATH,
            part_document(
                robot_arm_forearm().expect("forearm"),
                opencad_graph::robot_arm_forearm_parameters(),
                robot_arm::FOREARM_DOC,
                "Robot Arm Forearm Link",
            ),
        ),
        (
            robot_arm::GRIPPER_PATH,
            part_document(
                robot_arm_gripper().expect("gripper"),
                opencad_graph::robot_arm_gripper_parameters(),
                robot_arm::GRIPPER_DOC,
                "Robot Arm Wrist Gripper",
            ),
        ),
    ] {
        write_expanded_dir(example_root.join(relative), &doc).expect("write child part");
    }

    let assembly = robot_arm_assembly_model().expect("assembly model");
    let doc = OcadDocument {
        metadata: DocumentMetadata::new_assembly(
            DocumentId::new("doc:robot_arm_assembly_001").expect("id"),
            "Articulated Robot Arm",
        ),
        parameters: ParamGraph::new(),
        sketches: Vec::new(),
        feature_graph: opencad_graph::FeatureGraph::new(),
        feature_nodes: Vec::new(),
        semantic_refs: Vec::new(),
        assertions: Vec::new(),
        assembly: Some(assembly),
        drawing: None,
        attachments: Default::default(),
    };
    write_expanded_dir(&example_root, &doc).expect("write assembly");
}
