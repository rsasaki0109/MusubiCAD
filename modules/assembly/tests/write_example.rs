//! Generates `examples/assembly_two_brackets.ocad.d` (run once via `cargo test -p opencad-assembly --test write_example`).

use std::path::PathBuf;

use opencad_assembly::{AssemblyModel, Component, Instance, Mate, MateEntity, MateKind, Placement};
use opencad_core::{ComponentId, DocumentId, DocumentMetadata, InstanceId, MateId, TopoRefId};
use opencad_feature::bracket_with_hole;
use opencad_file::{write_expanded_dir, OcadDocument};
use opencad_geometry::{RigidTransform, TopoRef};
use opencad_graph::{bracket_parameters, FeatureGraph, ParamGraph};

#[test]
#[ignore = "run manually to refresh examples/assembly_two_brackets.ocad.d"]
fn write_two_brackets_example() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let example_root = manifest_dir.join("../../examples/assembly_two_brackets.ocad.d");
    let child_relative = "parts/bracket.ocad.d";
    let child_path = example_root.join(child_relative);

    if let Some(parent) = child_path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir child parent");
    }

    let part = bracket_with_hole().expect("bracket model");
    let mut child_doc = OcadDocument::from_part_model(
        DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Mounting Hole",
        ),
        &part,
    );
    child_doc.parameters = bracket_parameters();
    write_expanded_dir(&child_path, &child_doc).expect("write child");

    let assembly = AssemblyModel {
        components: vec![Component::new(
            ComponentId::new("component:bracket").expect("id"),
            child_relative.replace('\\', "/"),
            DocumentId::new("doc:bracket_001").expect("id"),
        )],
        instances: vec![
            Instance::new(
                InstanceId::new("instance:left").expect("id"),
                ComponentId::new("component:bracket").expect("id"),
                Placement::identity(),
                "Left Bracket",
            ),
            Instance::new(
                InstanceId::new("instance:right").expect("id"),
                ComponentId::new("component:bracket").expect("id"),
                Placement::new(RigidTransform::from_translation([0.05, 0.0, 0.0])),
                "Right Bracket",
            ),
        ],
        mates: vec![
            Mate::new(
                MateId::new("mate:ground_left").expect("id"),
                MateKind::Ground {
                    instance: InstanceId::new("instance:left").expect("id"),
                },
            ),
            Mate::new(
                MateId::new("mate:spacing").expect("id"),
                MateKind::Distance {
                    a: MateEntity::at_origin(
                        InstanceId::new("instance:left").expect("id"),
                        TopoRef::face(
                            TopoRefId::new("ref:face:left_origin").expect("id"),
                            "feature:extrude_base",
                            "origin",
                        ),
                    ),
                    b: MateEntity::at_origin(
                        InstanceId::new("instance:right").expect("id"),
                        TopoRef::face(
                            TopoRefId::new("ref:face:right_origin").expect("id"),
                            "feature:extrude_base",
                            "origin",
                        ),
                    ),
                    distance_m: 0.12,
                },
            ),
        ],
        ..Default::default()
    }
    .sorted_deterministic();

    let assembly_doc = OcadDocument {
        metadata: DocumentMetadata::new_assembly(
            DocumentId::new("doc:assembly_two_brackets").expect("id"),
            "Two Brackets Assembly",
        ),
        parameters: ParamGraph::new(),
        sketches: Vec::new(),
        feature_graph: FeatureGraph::new(),
        feature_nodes: Vec::new(),
        semantic_refs: Vec::new(),
        assembly: Some(assembly),
        drawing: None,
    };

    write_expanded_dir(&example_root, &assembly_doc).expect("write assembly");
}
