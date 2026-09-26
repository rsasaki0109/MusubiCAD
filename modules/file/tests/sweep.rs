//! ADR-023: sweeps move a closed profile along a path of lines and arcs
//! (integration test: requires OCCT).

use std::path::PathBuf;

use opencad_ai::DesignPatch;
use opencad_core::ValidationLevel;
use opencad_feature::FeatureRegistry;
use opencad_file::{apply_patch_to_document, dry_run_patch_document, read_ocad, OcadDocument};
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;

/// Agreement with Pappus' volume, in cubic metres (0.01 mm^3).
const VOLUME_TOLERANCE_M3: f64 = 1e-11;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn elbow_patch() -> DesignPatch {
    serde_json::from_str(
        &std::fs::read_to_string(repo().join("examples/agent/author_sweep_elbow_patch.json"))
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

fn volume_and_bounds(doc: &OcadDocument) -> (f64, [[f64; 3]; 2]) {
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
    // OCCT bounding boxes of swept B-spline faces are padded by their
    // control points, so the extents come from the tessellation instead.
    let mesh = kernel
        .tessellate(body, &opencad_geometry::TessellationSettings::default())
        .expect("mesh");
    let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
    for position in &mesh.positions {
        for axis in 0..3 {
            bounds[0][axis] = bounds[0][axis].min(f64::from(position[axis]));
            bounds[1][axis] = bounds[1][axis].max(f64::from(position[axis]));
        }
    }
    (
        kernel.mass_properties(body, 1.0).expect("mass").volume_m3,
        bounds,
    )
}

/// Agreement of tessellated extents with the design, in metres; covers the
/// default chordal deflection.
const EXTENT_TOLERANCE_M: f64 = 1e-4;

/// Pappus: section area times centroid path length, for a radius `r`
/// section along a `straight` line and a quarter bend of radius `bend`
/// (millimetres).
fn elbow_m3(r: f64, straight: f64, bend: f64) -> f64 {
    let pi = std::f64::consts::PI;
    pi * r * r * (straight + pi / 2.0 * bend) * 1e-9
}

#[test]
fn a_swept_elbow_matches_pappus() {
    let report = dry_run_patch_document(&empty(), &elbow_patch());
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert!(
        report
            .validation
            .messages
            .iter()
            .all(|message| message.level != ValidationLevel::Warning),
        "both sketches are fully constrained: {:?}",
        report.validation
    );

    let mut doc = empty();
    apply_patch_to_document(&mut doc, &elbow_patch()).expect("author");
    assert!(doc
        .feature_graph
        .dependency_edges()
        .iter()
        .any(|edge| edge.source == "feature:sketch_path" && edge.target == "feature:pipe"));

    let (volume, [min, max]) = volume_and_bounds(&doc);
    let expected = elbow_m3(5.0, 30.0, 20.0);
    assert!(
        (volume - expected).abs() <= VOLUME_TOLERANCE_M3,
        "volume {volume} m^3, expected {expected} m^3"
    );
    // Up 30 mm, then bent over towards +x: the bend's top reaches
    // z = 30 + 20 + 5 mm and its end x = 20 mm.
    assert!(
        (max[2] - 0.055).abs() < EXTENT_TOLERANCE_M,
        "top {} m",
        max[2]
    );
    assert!(
        (max[0] - 0.02).abs() < EXTENT_TOLERANCE_M,
        "end x {} m",
        max[0]
    );
    assert!(min[2].abs() < EXTENT_TOLERANCE_M, "bottom {} m", min[2]);

    apply_patch_to_document(
        &mut doc,
        &DesignPatch::set_parameter("param:bend_radius", "30 mm"),
    )
    .expect("edit");
    let (volume, _) = volume_and_bounds(&doc);
    let expected = elbow_m3(5.0, 30.0, 30.0);
    assert!(
        (volume - expected).abs() <= VOLUME_TOLERANCE_M3,
        "edited volume {volume} m^3, expected {expected} m^3"
    );
}
