//! ADR-024: arc angles follow parameters (integration test: requires OCCT).

use std::path::PathBuf;

use opencad_ai::DesignPatch;
use opencad_core::ValidationLevel;
use opencad_feature::FeatureRegistry;
use opencad_file::{apply_patch_to_document, dry_run_patch_document, read_ocad, OcadDocument};
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::json;

/// Agreement with the analytic sector volume, in cubic metres (0.001 mm^3).
const VOLUME_TOLERANCE_M3: f64 = 1e-12;

fn empty() -> OcadDocument {
    OcadDocument::new(
        read_ocad(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bracket.ocad.d"))
            .expect("bracket")
            .metadata,
    )
}

/// A 30 mm radius, 5 mm thick sector whose opening angle is the
/// `sweep_angle` parameter: two radii and an arc joined by endpoint points.
fn sector_patch() -> DesignPatch {
    let entity = |entity: serde_json::Value| json!({ "type": "add_sketch_entity", "sketch_id": "sketch:sector", "entity": entity });
    serde_json::from_value(json!({ "operations": [
        { "type": "add_parameter", "id": "param:radius", "name": "radius", "expr": "30 mm" },
        { "type": "add_parameter", "id": "param:sweep_angle", "name": "sweep_angle", "expr": "90 deg" },
        { "type": "add_sketch", "id": "sketch:sector", "name": "Sector",
          "workplane": { "type": "global", "plane": "XY" } },
        entity(json!({ "type": "point", "id": "ent:c", "x": 0.0, "y": 0.0 })),
        entity(json!({ "type": "point", "id": "ent:p0", "x": 0.03, "y": 0.0 })),
        entity(json!({ "type": "point", "id": "ent:p1", "x": 0.0, "y": 0.03 })),
        entity(json!({ "type": "line", "id": "ent:r0", "start": "ent:c", "end": "ent:p0" })),
        entity(json!({ "type": "arc", "id": "ent:rim", "center": "ent:c", "radius": 0.03,
                       "start_angle": "0 deg", "end_angle": "sweep_angle",
                       "start_point": "ent:p0", "end_point": "ent:p1" })),
        entity(json!({ "type": "line", "id": "ent:r1", "start": "ent:p1", "end": "ent:c" })),
        { "type": "add_sketch_constraint", "sketch_id": "sketch:sector",
          "constraint": { "type": "radius", "id": "con:rim_radius", "target": "ent:rim", "expr": "radius" } },
        { "type": "add_feature", "position": { "at": "end" },
          "node": { "id": "feature:sketch_sector", "name": "Sector Sketch",
                    "definition": { "type": "sketch", "sketch_id": "sketch:sector" } } },
        { "type": "add_feature", "position": { "at": "end" },
          "node": { "id": "feature:sector", "name": "Sector",
                    "definition": { "type": "extrude", "sketch_feature": "feature:sketch_sector",
                                    "profile_ref": "sketch:sector/profile:outer",
                                    "extent": { "type": "distance", "length": { "value_si": 0.005 } },
                                    "operation": "new_body" } } }
    ] }))
    .expect("patch")
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

/// theta / 2 * r^2 * h for r = 30 mm, h = 5 mm.
fn sector_m3(theta_deg: f64) -> f64 {
    theta_deg.to_radians() / 2.0 * 30.0 * 30.0 * 5.0 * 1e-9
}

#[test]
fn a_parameter_driven_arc_angle_reshapes_the_profile() {
    let report = dry_run_patch_document(&empty(), &sector_patch());
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert!(
        report
            .validation
            .messages
            .iter()
            .all(|message| message.level != ValidationLevel::Warning),
        "the sector is fully constrained: {:?}",
        report.validation
    );

    let mut doc = empty();
    apply_patch_to_document(&mut doc, &sector_patch()).expect("author");
    let volume = volume_m3(&doc);
    assert!(
        (volume - sector_m3(90.0)).abs() <= VOLUME_TOLERANCE_M3,
        "volume {volume} m^3"
    );

    // Widening the angle moves the arc's end point, and the closing radius
    // follows it.
    apply_patch_to_document(
        &mut doc,
        &DesignPatch::set_parameter("param:sweep_angle", "120 deg"),
    )
    .expect("widen");
    let volume = volume_m3(&doc);
    assert!(
        (volume - sector_m3(120.0)).abs() <= VOLUME_TOLERANCE_M3,
        "widened volume {volume} m^3"
    );

    // The document keeps the expression, not the resolved radians.
    let sketch = doc
        .sketches
        .iter()
        .find(|sketch| sketch.id.as_str() == "sketch:sector")
        .expect("sketch");
    let serialized = serde_json::to_string(sketch).expect("json");
    assert!(
        serialized.contains("\"end_angle\":\"sweep_angle\""),
        "{serialized}"
    );
    assert!(!serialized.contains("resolved_angles"), "{serialized}");
}
