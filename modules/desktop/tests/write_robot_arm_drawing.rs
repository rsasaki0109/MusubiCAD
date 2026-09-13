//! Generates `examples/robot_arm_assembly_drawing.ocad.d` (run once via
//! `cargo test -p opencad-desktop --test write_robot_arm_drawing -- --ignored`).
//!
//! The drawing references the sibling arm assembly at `../robot_arm_assembly.ocad.d`
//! instead of duplicating it, so the two examples cannot drift.

use std::path::PathBuf;

use opencad_core::{DimensionId, DocumentId, DocumentMetadata, SheetId, ViewId};
use opencad_drawing::{
    DrawingModel, DrawingView, LinearDimension, ModelReference, ProjectionKind, Sheet, A4_HEIGHT_M,
    A4_WIDTH_M,
};
use opencad_file::{write_expanded_dir, OcadDocument};

#[test]
#[ignore = "run manually to refresh examples/robot_arm_assembly_drawing.ocad.d"]
fn write_robot_arm_drawing_example() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let example_root = manifest_dir.join("../../examples/robot_arm_assembly_drawing.ocad.d");

    let child_relative = "../robot_arm_assembly.ocad.d";
    let drawing = DrawingModel {
        sheets: vec![Sheet {
            id: SheetId::new("sheet:a4").expect("sheet id"),
            name: "Sheet 1".into(),
            width_m: A4_WIDTH_M,
            height_m: A4_HEIGHT_M,
            views: vec![DrawingView::new(
                ViewId::new("view:front").expect("view id"),
                "Front",
                ModelReference::new(
                    child_relative,
                    DocumentId::new("doc:robot_arm_assembly_001").expect("doc id"),
                ),
                ProjectionKind::Front,
                0.5,
                [0.05, 0.05],
            )],
            dimensions: vec![LinearDimension {
                id: DimensionId::new("dim:upper_arm_reach").expect("dim id"),
                view_id: ViewId::new("view:front").expect("view id"),
                start_model_m: [0.0, 0.0, 0.0],
                end_model_m: [0.0, 0.16, 0.0],
                offset_m: -0.02,
            }],
        }],
    }
    .sorted_deterministic();

    let doc = OcadDocument::from_drawing_model(
        DocumentMetadata::new_drawing(
            DocumentId::new("doc:robot_arm_assembly_drawing").expect("doc id"),
            "Robot Arm Assembly Front View",
        ),
        drawing,
    );
    write_expanded_dir(&example_root, &doc).expect("write drawing");
}
