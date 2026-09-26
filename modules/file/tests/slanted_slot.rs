//! Tangent lines and arcs at a parameter-driven angle: a slanted slot whose
//! arc angles follow `slot_angle` (ADR-024) and whose straight sides stay
//! tangent to its end arcs (integration test: requires OCCT).

use std::path::PathBuf;

use opencad_ai::DesignPatch;
use opencad_core::ValidationLevel;
use opencad_feature::FeatureRegistry;
use opencad_file::{apply_patch_to_document, dry_run_patch_document, read_ocad, OcadDocument};
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;

/// Agreement with the analytic slot volume, in cubic metres (0.001 mm^3).
const VOLUME_TOLERANCE_M3: f64 = 1e-12;

/// Agreement of tessellated extents with the design, in metres; covers the
/// default chordal deflection on the end arcs.
const EXTENT_TOLERANCE_M: f64 = 1e-4;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn slot_patch() -> DesignPatch {
    serde_json::from_str(
        &std::fs::read_to_string(repo().join("examples/agent/author_slanted_slot_patch.json"))
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

/// Volume and the tessellated (x, y) extents of the slot body.
fn volume_and_extents(doc: &OcadDocument) -> (f64, [f64; 2]) {
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
    let body = model.active_body().expect("body");
    let mesh = kernel
        .tessellate(body, &opencad_geometry::TessellationSettings::default())
        .expect("mesh");
    let mut extents = [0.0; 2];
    for (axis, extent) in extents.iter_mut().enumerate() {
        let values = mesh.positions.iter().map(|p| f64::from(p[axis]));
        let max = values.clone().fold(f64::NEG_INFINITY, f64::max);
        let min = values.fold(f64::INFINITY, f64::min);
        *extent = max - min;
    }
    (
        kernel.mass_properties(body, 1.0).expect("mass").volume_m3,
        extents,
    )
}

/// 30 mm between centres, 8 mm wide, 5 mm thick.
fn slot_m3() -> f64 {
    let r = 0.004;
    (0.03 * 2.0 * r + std::f64::consts::PI * r * r) * 0.005
}

/// (x, y) extents of the slot at `angle_deg`.
fn slot_extents(angle_deg: f64) -> [f64; 2] {
    let (sin, cos) = angle_deg.to_radians().sin_cos();
    [0.03 * cos + 0.008, 0.03 * sin + 0.008]
}

fn assert_slot(doc: &OcadDocument, angle_deg: f64) {
    let (volume, extents) = volume_and_extents(doc);
    assert!(
        (volume - slot_m3()).abs() <= VOLUME_TOLERANCE_M3,
        "volume {volume} m^3 at {angle_deg} deg"
    );
    let expected = slot_extents(angle_deg);
    for axis in 0..2 {
        assert!(
            (extents[axis] - expected[axis]).abs() <= EXTENT_TOLERANCE_M,
            "extents {extents:?}, expected {expected:?} at {angle_deg} deg"
        );
    }
}

#[test]
fn a_slanted_slot_stays_tangent_when_its_angle_changes() {
    let report = dry_run_patch_document(&empty(), &slot_patch());
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert!(
        report
            .validation
            .messages
            .iter()
            .all(|message| message.level != ValidationLevel::Warning),
        "the slot is fully constrained: {:?}",
        report.validation
    );

    let mut doc = empty();
    apply_patch_to_document(&mut doc, &slot_patch()).expect("author");
    assert_slot(&doc, 30.0);

    apply_patch_to_document(
        &mut doc,
        &DesignPatch::set_parameter("param:slot_angle", "60 deg"),
    )
    .expect("steepen");
    assert_slot(&doc, 60.0);
}
