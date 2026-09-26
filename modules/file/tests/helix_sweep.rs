//! ADR-025: helical sweeps move a closed profile along a helix through its
//! centre (integration test: requires OCCT).

use std::f64::consts::PI;
use std::path::PathBuf;

use opencad_ai::DesignPatch;
use opencad_core::ValidationLevel;
use opencad_feature::FeatureRegistry;
use opencad_file::{apply_patch_to_document, dry_run_patch_document, read_ocad, OcadDocument};
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;

/// Relative agreement with the analytic coil volume (dimensionless).  OCCT
/// approximates helical pipe surfaces with B-splines, and the error grows
/// with the turn count: the 4-turn coil lands 4.5e-5 low, the 6-turn coil
/// 1.3e-4 low.
const VOLUME_RELATIVE_TOLERANCE: f64 = 5e-4;

/// Agreement of tessellated extents with the design, in metres; covers the
/// default chordal deflection.
const EXTENT_TOLERANCE_M: f64 = 2e-4;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn coil_patch() -> DesignPatch {
    serde_json::from_str(
        &std::fs::read_to_string(repo().join("examples/agent/author_coil_spring_patch.json"))
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

/// A screw motion moves the wire section `2πR` per turn normal to its
/// plane: `πa² · 2πR · turns` (millimetres in, cubic metres out).
fn coil_m3(wire_radius: f64, coil_radius: f64, turns: f64) -> f64 {
    PI * wire_radius * wire_radius * 2.0 * PI * coil_radius * turns * 1e-9
}

fn assert_coil(doc: &OcadDocument, coil_radius: f64, height: f64, turns: f64) {
    let (volume, bounds) = volume_and_bounds(doc);
    let expected = coil_m3(1.0, coil_radius, turns);
    assert!(
        (volume - expected).abs() <= VOLUME_RELATIVE_TOLERANCE * expected,
        "volume {volume} m^3, expected {expected} m^3"
    );
    // The wire's outer edge reaches R + a around the axis; it spans the
    // helix height plus the wire diameter along it.
    let outer = (coil_radius + 1.0) * 1e-3;
    for (axis, expected) in [(0, outer), (1, outer)] {
        assert!(
            (bounds[1][axis] - expected).abs() <= EXTENT_TOLERANCE_M,
            "bounds {bounds:?}"
        );
    }
    assert!(
        (bounds[0][2] + 1e-3).abs() <= EXTENT_TOLERANCE_M
            && (bounds[1][2] - (height + 1.0) * 1e-3).abs() <= EXTENT_TOLERANCE_M,
        "bounds {bounds:?}"
    );
}

#[test]
fn a_coil_spring_matches_the_screw_motion_volume() {
    let report = dry_run_patch_document(&empty(), &coil_patch());
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert!(
        report
            .validation
            .messages
            .iter()
            .all(|message| message.level != ValidationLevel::Warning),
        "the wire section is fully constrained: {:?}",
        report.validation
    );

    let mut doc = empty();
    apply_patch_to_document(&mut doc, &coil_patch()).expect("author");
    assert_coil(&doc, 10.0, 20.0, 4.0);

    // A wider, taller coil follows its parameters.
    apply_patch_to_document(
        &mut doc,
        &DesignPatch::set_parameter("param:coil_radius", "12 mm"),
    )
    .expect("widen");
    apply_patch_to_document(
        &mut doc,
        &DesignPatch::set_parameter("param:coil_height", "30 mm"),
    )
    .expect("lengthen");
    assert_coil(&doc, 12.0, 30.0, 6.0);
}

#[test]
fn a_profile_on_the_axis_is_rejected() {
    let mut doc = empty();
    apply_patch_to_document(&mut doc, &coil_patch()).expect("author");
    let applied = apply_patch_to_document(
        &mut doc,
        &DesignPatch::set_parameter("param:coil_radius", "0 mm"),
    );
    // Either the patch is refused, or regeneration fails closed.
    if applied.is_ok() {
        let kernel = OcctGeometryKernel::new();
        let parameters = doc.parameters.clone();
        let mut model = doc.clone().into_part_model();
        assert!(model
            .regenerate(
                &kernel,
                &FeatureRegistry::with_defaults(),
                Some(&parameters),
                None,
            )
            .is_err());
    }
}
