//! ADR-016: STEP files imported as fixed solids (integration test: requires
//! OCCT).

use std::path::{Path, PathBuf};
use std::process::Command;

use opencad_ai::{authoring_patch, build_patch_candidate, DesignPatch};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::FeatureRegistry;
use opencad_file::{
    apply_patch_to_document, document_design_state, read_ocad, validate_ocad, write_ocad,
    OcadDocument,
};
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::json;

/// Volume agreement between an imported solid and its source, in m^3.
const VOLUME_TOLERANCE_M3: f64 = 1e-12;

/// Bounding-box agreement for placed imports, in metres (OCCT boxes are
/// slightly enlarged).
const BOUNDS_TOLERANCE_M: f64 = 1e-6;

fn opencad(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_opencad"))
        .args(args)
        .output()
        .expect("run opencad")
}

fn ok(output: std::process::Output) -> String {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

fn text(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

/// Export the bracket as STEP into `dir`.
fn bracket_step(dir: &Path) -> PathBuf {
    let step = dir.join("Bracket Part.STEP");
    ok(opencad(&[
        "export",
        &text(&example("bracket.ocad.d")),
        &text(&step),
    ]));
    step
}

fn empty_part(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let metadata = DocumentMetadata::new(DocumentId::new("doc:holder").expect("id"), "Holder");
    write_ocad(&path, &OcadDocument::new(metadata)).expect("write empty part");
    path
}

/// Regenerate with OCCT; returns (volume m^3, bounds min, bounds max).
fn regenerate(doc: &OcadDocument) -> opencad_core::Result<(f64, [f64; 3], [f64; 3])> {
    let parameters = doc.parameters.clone();
    let mut model = doc.clone().into_part_model();
    let kernel = OcctGeometryKernel::new();
    model.regenerate(
        &kernel,
        &FeatureRegistry::with_defaults(),
        Some(&parameters),
        None,
    )?;
    let body = model.active_body().expect("body").clone();
    let mass = kernel.mass_properties(&body, 1.0)?;
    let bounds = kernel.bounding_box(&body)?;
    Ok((mass.volume_m3, bounds.min, bounds.max))
}

#[test]
fn an_imported_step_solid_is_placed_persisted_and_regenerated() {
    let dir = tempfile::tempdir().expect("tempdir");
    let step = bracket_step(dir.path());
    let part = empty_part(dir.path(), "holder.ocad.d");
    let summary: serde_json::Value = serde_json::from_str(&ok(opencad(&[
        "import-step",
        &text(&part),
        &text(&step),
        "--id",
        "feature:bracket",
        "--translate-mm",
        "100,0,0",
    ])))
    .expect("summary JSON");
    assert_eq!(summary["attachment"], "imports/bracket_part.step");

    // Checksums cover the attachment file.
    let doc = validate_ocad(&part).expect("checksums verify");
    assert!(part.join("imports/bracket_part.step").exists());
    assert_eq!(doc.attachments.len(), 1);

    let (volume, min, max) = regenerate(&doc).expect("regenerate imported solid");
    let (expected, _, _) = regenerate(&read_ocad(example("bracket.ocad.d")).expect("bracket"))
        .expect("regenerate bracket");
    assert!(
        (volume - expected).abs() <= VOLUME_TOLERANCE_M3,
        "{volume} vs {expected}"
    );
    assert!(
        (min[0] - 0.1).abs() <= BOUNDS_TOLERANCE_M,
        "min x {}",
        min[0]
    );
    assert!(
        (max[0] - 0.18).abs() <= BOUNDS_TOLERANCE_M,
        "max x {}",
        max[0]
    );

    // The document rebuilds from one structural patch, attachment included.
    let rebuilt_patch = authoring_patch(&document_design_state(&doc)).expect("authoring");
    let mut rebuilt = OcadDocument::new(doc.metadata.clone());
    apply_patch_to_document(&mut rebuilt, &rebuilt_patch).expect("rebuild");
    assert_eq!(rebuilt.attachments, doc.attachments);
    assert_eq!(rebuilt.feature_nodes, doc.feature_nodes);
}

#[test]
fn an_imported_solid_can_cut_a_body_and_tampering_fails_closed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let step = bracket_step(dir.path());
    let part = empty_part(dir.path(), "plate.ocad.d");
    let authoring: DesignPatch = serde_json::from_str(
        &std::fs::read_to_string(example("agent/author_plate_from_empty_patch.json"))
            .expect("plate patch"),
    )
    .expect("patch");
    let mut doc = read_ocad(&part).expect("read");
    apply_patch_to_document(&mut doc, &authoring).expect("author plate");
    write_ocad(&part, &doc).expect("write plate");
    let (plate_volume, _, _) = regenerate(&doc).expect("plate");

    // Cut the bracket, shifted 30 mm in x, out of the plate.
    ok(opencad(&[
        "import-step",
        &text(&part),
        &text(&step),
        "--id",
        "feature:pocket",
        "--operation",
        "cut",
        "--target",
        "feature:through_hole",
        "--translate-mm",
        "30,0,0",
    ]));
    let mut cut = read_ocad(&part).expect("read cut");
    let (cut_volume, _, max) = regenerate(&cut).expect("cut");
    assert!(cut_volume > 0.0 && cut_volume < plate_volume - 1e-7);
    // Only the plate's first 30 mm in x survive.
    assert!(
        (max[0] - 0.03).abs() <= BOUNDS_TOLERANCE_M,
        "max x {}",
        max[0]
    );

    // A tampered attachment fails regeneration instead of being used.
    let key = cut.attachments.keys().next().expect("attachment").clone();
    cut.attachments.get_mut(&key).unwrap().push(b' ');
    let error = regenerate(&cut).expect_err("digest mismatch");
    assert!(error.to_string().contains("expects"), "{error}");

    // An attachment in use cannot be removed.
    let state = document_design_state(&read_ocad(&part).expect("read"));
    let remove: DesignPatch = serde_json::from_value(json!({ "operations": [
        { "type": "remove_attachment", "path": key }
    ] }))
    .expect("patch");
    let error = build_patch_candidate(&state, &remove).expect_err("in use");
    assert!(
        error
            .to_string()
            .contains("still used by feature feature:pocket"),
        "{error}"
    );

    // Bytes that do not match their declared digest never enter a document.
    let forged: DesignPatch = serde_json::from_value(json!({ "operations": [
        { "type": "add_attachment", "path": "imports/forged.step",
          "sha256": "0".repeat(64), "content_base64": "SVNPLTEwMzAzLTIxOw==" }
    ] }))
    .expect("patch");
    let error = build_patch_candidate(&state, &forged).expect_err("digest");
    assert!(
        error.to_string().contains("but the patch declares"),
        "{error}"
    );
}
