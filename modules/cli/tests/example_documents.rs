//! Committed example documents regenerate with OCCT.

use opencad_feature::FeatureRegistry;
use opencad_file::{read_expanded_dir, validate_expanded_dir};
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn example_bracket_hole_row_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_hole_row.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 < plate_volume,
        "hole row example should reduce plate volume: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_pin_mirror_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_pin_mirror.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let pin_tool = model
        .outputs
        .get("feature:pin_tool")
        .and_then(|output| output.body.as_ref())
        .expect("pin tool");
    let pin_mass = kernel.mass_properties(pin_tool, 2700.0).expect("mass");
    assert!(
        mass.volume_m3 > pin_mass.volume_m3 * 1.5,
        "pin mirror example should union source and reflection: {} vs {}",
        mass.volume_m3,
        pin_mass.volume_m3
    );
}

#[test]
fn example_bracket_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    assert!(model.active_body().is_some());
}
