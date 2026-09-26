//! ADR-021: loops of lines and arcs become exact profiles (integration
//! test: requires OCCT).

use std::path::PathBuf;

use opencad_ai::DesignPatch;
use opencad_core::ValidationLevel;
use opencad_feature::FeatureRegistry;
use opencad_file::{apply_patch_to_document, dry_run_patch_document, read_ocad, OcadDocument};
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;

/// Agreement with the analytic volume, in cubic metres (0.001 mm^3).
const VOLUME_TOLERANCE_M3: f64 = 1e-12;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn slot_patch() -> DesignPatch {
    serde_json::from_str(
        &std::fs::read_to_string(repo().join("examples/agent/author_slot_plate_patch.json"))
            .expect("read"),
    )
    .expect("patch json")
}

fn empty() -> OcadDocument {
    OcadDocument::new(
        read_ocad(repo().join("examples/bracket.ocad.d"))
            .expect("bracket")
            .metadata,
    )
}

fn volume_m3(doc: &OcadDocument) -> f64 {
    let kernel = OcctGeometryKernel::new();
    let parameters = doc.parameters.clone();
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&parameters),
            None,
        )
        .expect("regenerate");
    kernel
        .mass_properties(model.active_body().expect("body"), 1.0)
        .expect("mass")
        .volume_m3
}

/// A 60 x 40 x 5 mm plate minus a through slot: two straight sides of
/// `length` and two half circles of radius `width / 2`, in millimetres.
fn slotted_plate_m3(length: f64, width: f64) -> f64 {
    let radius = width / 2.0;
    (60.0 * 40.0 - (length * width + std::f64::consts::PI * radius * radius)) * 5.0 * 1e-9
}

#[test]
fn a_slot_of_lines_and_arcs_cuts_its_exact_area() {
    let report = dry_run_patch_document(&empty(), &slot_patch());
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert!(
        report
            .validation
            .messages
            .iter()
            .all(|message| message.level != ValidationLevel::Warning),
        "the slot sketch is fully constrained: {:?}",
        report.validation
    );

    let mut doc = empty();
    apply_patch_to_document(&mut doc, &slot_patch()).expect("author");
    let slot = doc
        .sketches
        .iter()
        .find(|sketch| sketch.id.as_str() == "sketch:slot")
        .expect("slot sketch");
    let outer = slot
        .profiles
        .iter()
        .find(|profile| profile.profile_ref.as_deref() == Some("sketch:slot/profile:outer"))
        .expect("closed slot profile");
    assert_eq!(outer.entity_ids.len(), 4, "two lines and two arcs");

    let volume = volume_m3(&doc);
    let expected = slotted_plate_m3(30.0, 8.0);
    assert!(
        (volume - expected).abs() <= VOLUME_TOLERANCE_M3,
        "volume {volume} m^3, expected {expected} m^3"
    );

    // Arc endpoints stay on their arcs, so widening the slot keeps it closed.
    apply_patch_to_document(
        &mut doc,
        &DesignPatch::set_parameter("param:slot_width", "10 mm"),
    )
    .expect("widen");
    let volume = volume_m3(&doc);
    let expected = slotted_plate_m3(30.0, 10.0);
    assert!(
        (volume - expected).abs() <= VOLUME_TOLERANCE_M3,
        "widened volume {volume} m^3, expected {expected} m^3"
    );
}
