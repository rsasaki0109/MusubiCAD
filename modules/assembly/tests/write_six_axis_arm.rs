//! Generates `examples/six_axis_arm.ocad.d` (run once via
//! `cargo test -p opencad-assembly --test write_six_axis_arm -- --ignored`).

use std::path::PathBuf;

use opencad_assembly::{six_axis_arm, six_axis_arm_model};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::{
    robot_arm_base, robot_arm_forearm, robot_arm_gripper, robot_arm_upper_arm, six_axis_hand,
    six_axis_turntable, six_axis_wrist, PartModel,
};
use opencad_file::{write_expanded_dir, OcadDocument};
use opencad_graph::ParamGraph;

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
#[ignore = "run manually to refresh examples/six_axis_arm.ocad.d"]
fn write_six_axis_arm_example() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let example_root = manifest_dir.join("../../examples/six_axis_arm.ocad.d");

    for (relative, doc) in [
        (
            six_axis_arm::BASE_PATH,
            part_document(
                robot_arm_base().expect("base"),
                opencad_graph::robot_arm_base_parameters(),
                six_axis_arm::BASE_DOC,
                "Robot Arm Base",
            ),
        ),
        (
            six_axis_arm::TURNTABLE_PATH,
            part_document(
                six_axis_turntable().expect("turntable"),
                opencad_graph::six_axis_turntable_parameters(),
                six_axis_arm::TURNTABLE_DOC,
                "Six-Axis Arm Turntable",
            ),
        ),
        (
            six_axis_arm::UPPER_ARM_PATH,
            part_document(
                robot_arm_upper_arm().expect("upper arm"),
                opencad_graph::robot_arm_upper_arm_parameters(),
                six_axis_arm::UPPER_ARM_DOC,
                "Robot Arm Upper Link",
            ),
        ),
        (
            six_axis_arm::FOREARM_PATH,
            part_document(
                robot_arm_forearm().expect("forearm"),
                opencad_graph::robot_arm_forearm_parameters(),
                six_axis_arm::FOREARM_DOC,
                "Robot Arm Forearm Link",
            ),
        ),
        (
            six_axis_arm::WRIST_PATH,
            part_document(
                six_axis_wrist().expect("wrist"),
                opencad_graph::six_axis_wrist_parameters(),
                six_axis_arm::WRIST_DOC,
                "Six-Axis Arm Wrist Link",
            ),
        ),
        (
            six_axis_arm::HAND_PATH,
            part_document(
                six_axis_hand().expect("hand"),
                opencad_graph::six_axis_hand_parameters(),
                six_axis_arm::HAND_DOC,
                "Six-Axis Arm Hand",
            ),
        ),
        (
            six_axis_arm::GRIPPER_PATH,
            part_document(
                robot_arm_gripper().expect("gripper"),
                opencad_graph::robot_arm_gripper_parameters(),
                six_axis_arm::GRIPPER_DOC,
                "Robot Arm Wrist Gripper",
            ),
        ),
    ] {
        write_expanded_dir(example_root.join(relative), &doc).expect("write child part");
    }

    let doc = OcadDocument {
        metadata: DocumentMetadata::new_assembly(
            DocumentId::new("doc:six_axis_arm_001").expect("id"),
            "Six-Axis Robot Arm",
        ),
        parameters: ParamGraph::new(),
        sketches: Vec::new(),
        feature_graph: opencad_graph::FeatureGraph::new(),
        feature_nodes: Vec::new(),
        semantic_refs: Vec::new(),
        assertions: Vec::new(),
        assembly: Some(six_axis_arm_model().expect("assembly model")),
        drawing: None,
        attachments: Default::default(),
    };
    write_expanded_dir(&example_root, &doc).expect("write assembly");
}
