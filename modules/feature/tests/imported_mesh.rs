//! MCAD-P12-010: graph source identity, cache invalidation and Mock mass evidence.
use opencad_core::{sha256_hex, LengthUnit};
use opencad_feature::{
    FeatureDefinition, FeatureNode, FeatureRegistry, ImportedMeshFeature, PartModel,
    RegenerationCache,
};
use opencad_geometry::{GeometryKernel, MockGeometryKernel, RigidTransform};

fn definition() -> ImportedMeshFeature {
    ImportedMeshFeature {
        source: "imports/plate.stl".into(),
        sha256: sha256_hex(include_bytes!(
            "../../../examples/import/closed_plate_mm.stl"
        )),
        source_unit: LengthUnit::Millimeter,
        transform: RigidTransform::identity(),
    }
}

#[test]
fn imported_mesh_requires_explicit_units_and_preserves_wire_identity() {
    let feature = FeatureDefinition::ImportedMesh(definition());
    let mut value = serde_json::to_value(&feature).expect("wire");
    assert_eq!(
        serde_json::from_value::<FeatureDefinition>(value.clone()).expect("round trip"),
        feature
    );
    value.as_object_mut().expect("object").remove("source_unit");
    assert!(serde_json::from_value::<FeatureDefinition>(value).is_err());
    let mut invalid = definition();
    invalid.source = "imports/../plate.stl".into();
    assert!(invalid.validate().is_err());
}

#[test]
fn warm_cache_must_not_hide_changed_or_missing_source_bytes() {
    let kernel = MockGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let mut cache = RegenerationCache::new();
    let mut model = PartModel::default();
    let def = definition();
    model.attachments.insert(
        def.source.clone(),
        include_bytes!("../../../examples/import/closed_plate_mm.stl").to_vec(),
    );
    model
        .add_node(FeatureNode::new(
            "feature:mesh",
            "STL",
            FeatureDefinition::ImportedMesh(def),
        ))
        .expect("node");
    model
        .regenerate_with_cache(&kernel, &registry, None, None, &mut cache)
        .expect("cold");
    let body = model.active_body().expect("body").clone();
    assert!((kernel.mass_properties(&body, 1.).expect("mass").volume_m3 - 2.88e-5).abs() < 1e-12);
    let warm = model
        .regenerate_with_cache(&kernel, &registry, None, None, &mut cache)
        .expect("warm");
    assert_eq!(warm.cached_nodes, vec!["feature:mesh"]);
    let outputs = model.outputs.clone();
    model
        .attachments
        .get_mut("imports/plate.stl")
        .expect("bytes")[0] ^= 1;
    assert!(model
        .regenerate_with_cache(&kernel, &registry, None, None, &mut cache)
        .is_err());
    assert_eq!(model.outputs, outputs);
    model.attachments.clear();
    assert!(model
        .regenerate_with_cache(&kernel, &registry, None, None, &mut cache)
        .is_err());
    assert_eq!(model.outputs, outputs);
}
