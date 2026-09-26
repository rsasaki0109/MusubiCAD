//! ADR-022: lofts skin solids through section profiles (integration test:
//! requires OCCT).

use std::path::PathBuf;

use opencad_ai::DesignPatch;
use opencad_core::ValidationLevel;
use opencad_feature::FeatureRegistry;
use opencad_file::{apply_patch_to_document, dry_run_patch_document, read_ocad, OcadDocument};
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;

/// Agreement with the analytic frustum volume, in cubic metres
/// (0.01 mm^3).  Two planar sections skin to flat ruled sides.
const VOLUME_TOLERANCE_M3: f64 = 1e-11;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn loft_patch() -> DesignPatch {
    serde_json::from_str(
        &std::fs::read_to_string(repo().join("examples/agent/author_loft_frustum_patch.json"))
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

/// Square frustum of height 30 mm between square sides `base` and `top`
/// (millimetres): h/3 (A1 + A2 + sqrt(A1 A2)).
fn frustum_m3(base: f64, top: f64) -> f64 {
    let (a1, a2) = (base * base, top * top);
    30.0 / 3.0 * (a1 + a2 + (a1 * a2).sqrt()) * 1e-9
}

#[test]
fn a_two_section_loft_is_an_exact_frustum() {
    let report = dry_run_patch_document(&empty(), &loft_patch());
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert!(
        report
            .validation
            .messages
            .iter()
            .all(|message| message.level != ValidationLevel::Warning),
        "both sections are fully constrained: {:?}",
        report.validation
    );

    let mut doc = empty();
    apply_patch_to_document(&mut doc, &loft_patch()).expect("author");
    assert!(doc
        .feature_graph
        .dependency_edges()
        .iter()
        .any(|edge| edge.source == "feature:sketch_top" && edge.target == "feature:frustum"));

    let volume = volume_m3(&doc);
    let expected = frustum_m3(40.0, 20.0);
    assert!(
        (volume - expected).abs() <= VOLUME_TOLERANCE_M3,
        "volume {volume} m^3, expected {expected} m^3"
    );

    apply_patch_to_document(
        &mut doc,
        &DesignPatch::set_parameter("param:top_size", "30 mm"),
    )
    .expect("edit");
    let volume = volume_m3(&doc);
    let expected = frustum_m3(40.0, 30.0);
    assert!(
        (volume - expected).abs() <= VOLUME_TOLERANCE_M3,
        "edited volume {volume} m^3, expected {expected} m^3"
    );
}

#[test]
fn a_loft_needs_two_sections() {
    let mut patch = serde_json::to_value(loft_patch()).expect("json");
    let operations = patch["operations"].as_array_mut().expect("operations");
    let loft = operations.last_mut().expect("loft");
    loft["node"]["definition"]["sections"]
        .as_array_mut()
        .expect("sections")
        .truncate(1);
    let patch: DesignPatch = serde_json::from_value(patch).expect("patch");
    let report = dry_run_patch_document(&empty(), &patch);
    assert!(
        format!("{:?}", report.validation).contains("at least two sections"),
        "{:?}",
        report.validation
    );
}
