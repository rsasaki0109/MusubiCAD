//! MCAD-P11-003: rib feature contracts and OCCT analytic evidence.

use opencad_core::{EntityId, Length, SketchId};
use opencad_feature::{
    bracket_base_plate, derive_feature_graph, FeatureDefinition, FeatureNode, FeatureRegistry,
    PartModel, RegenerationCache, RibFeature, SketchFeatureDef,
};
use opencad_geometry::{ExtrudeExtent, GeometryKernel, MockGeometryKernel};
use opencad_graph::{bracket_parameters, ParameterEntry};
use opencad_kernel_occt::OcctGeometryKernel;
use opencad_sketch::{
    entity::{Coord, EntityBase, LineEntity, PointEntity, RectangleEntity, SketchEntity},
    workplane::Workplane,
    Sketch,
};

fn rib_model() -> PartModel {
    let mut model = bracket_base_plate().expect("plate");
    let mut sketch = Sketch::new(
        SketchId::new("sketch:rib").expect("id"),
        "Rib centerline",
        Workplane::Custom {
            origin: [0.0, 0.0, 0.006],
            normal: [0.0, 0.0, 1.0],
            x_axis: [1.0, 0.0, 0.0],
        },
    );
    for (id, x, y) in [("ent:rib_start", -0.020, 0.0), ("ent:rib_end", 0.020, 0.0)] {
        sketch
            .add_entity(SketchEntity::Point(PointEntity {
                base: EntityBase {
                    id: EntityId::new(id).expect("id"),
                    construction: false,
                },
                x: Coord::literal(x),
                y: Coord::literal(y),
            }))
            .expect("point");
    }
    sketch
        .add_entity(SketchEntity::Line(LineEntity {
            base: EntityBase {
                id: EntityId::new("ent:rib_line").expect("id"),
                construction: false,
            },
            start: EntityId::new("ent:rib_start").expect("id"),
            end: EntityId::new("ent:rib_end").expect("id"),
        }))
        .expect("line");
    sketch
        .add_entity(SketchEntity::Rectangle(RectangleEntity {
            base: EntityBase {
                id: EntityId::new("ent:rib_validator_profile").expect("id"),
                construction: false,
            },
            origin: [Coord::literal(-0.001), Coord::literal(-0.001)],
            size: [Coord::literal(0.001), Coord::literal(0.001)],
            corner_ids: vec![
                EntityId::new("ent:rib_v0").expect("id"),
                EntityId::new("ent:rib_v1").expect("id"),
                EntityId::new("ent:rib_v2").expect("id"),
                EntityId::new("ent:rib_v3").expect("id"),
            ],
            edge_ids: vec![
                EntityId::new("ent:rib_ve0").expect("id"),
                EntityId::new("ent:rib_ve1").expect("id"),
                EntityId::new("ent:rib_ve2").expect("id"),
                EntityId::new("ent:rib_ve3").expect("id"),
            ],
        }))
        .expect("validator profile");
    model.add_sketch(sketch).expect("sketch");
    model
        .add_node(FeatureNode::new(
            "feature:sketch_rib",
            "Rib Sketch",
            FeatureDefinition::Sketch(SketchFeatureDef {
                sketch_id: "sketch:rib".into(),
            }),
        ))
        .expect("sketch feature");
    model
        .add_node(FeatureNode::new(
            "feature:rib",
            "Centerline Rib",
            FeatureDefinition::Rib(RibFeature {
                sketch_feature: "feature:sketch_rib".into(),
                centerline_ref: "ent:rib_line".into(),
                target_feature: "feature:extrude_base".into(),
                thickness: Length::from_meters(0.004),
                thickness_expr: Some("rib_thickness".into()),
                extent: ExtrudeExtent::Distance {
                    length: Length::from_meters(0.012),
                },
                length_expr: Some("rib_height".into()),
            }),
        ))
        .expect("rib feature");
    let order = vec![
        "feature:sketch_base".to_string(),
        "feature:extrude_base".to_string(),
        "feature:sketch_rib".to_string(),
        "feature:rib".to_string(),
    ];
    model.graph = derive_feature_graph(
        &model.nodes.values().cloned().collect::<Vec<_>>(),
        &order,
        &model.sketches.values().cloned().collect::<Vec<_>>(),
        &[],
    )
    .expect("graph");
    model
}

fn params(thickness: &str, height: &str) -> opencad_graph::ParamGraph {
    let mut params = bracket_parameters();
    params
        .add_parameter(ParameterEntry::new(
            "param:rib_thickness",
            "rib_thickness",
            thickness,
        ))
        .expect("thickness");
    params
        .add_parameter(ParameterEntry::new(
            "param:rib_height",
            "rib_height",
            height,
        ))
        .expect("height");
    params
}

fn contract<K: GeometryKernel>(kernel: &K, analytic: bool) {
    let mut model = rib_model();
    let registry = FeatureRegistry::with_defaults();
    let mut cache = RegenerationCache::new();
    let mut params = params("4 mm", "12 mm");
    model
        .regenerate_with_cache(kernel, &registry, Some(&params), None, &mut cache)
        .expect("regen");
    let body = model.active_body().expect("body");
    if analytic {
        let expected = 0.080 * 0.060 * 0.006 + 0.040 * 0.004 * 0.012;
        let actual = kernel.mass_properties(body, 1.0).expect("mass").volume_m3;
        assert!((actual - expected).abs() < 1e-12, "{actual} vs {expected}");
        let bounds = kernel.bounding_box(body).expect("bounds");
        assert!((bounds.max[2] - 0.018).abs() < 1e-6, "{bounds:?}");
        assert!((bounds.min[0] + 0.020).abs() < 1e-6, "{bounds:?}");
    }
    let warm = model
        .regenerate_with_cache(kernel, &registry, Some(&params), None, &mut cache)
        .expect("warm");
    assert_eq!(warm.cached_nodes.len(), 2);

    params
        .set_expr("param:rib_height", "20 mm")
        .expect("edit height");
    model
        .regenerate_with_cache(kernel, &registry, Some(&params), None, &mut cache)
        .expect("height edit");
    if analytic {
        let expected = 0.080 * 0.060 * 0.006 + 0.040 * 0.004 * 0.020;
        let actual = kernel
            .mass_properties(model.active_body().expect("body"), 1.0)
            .expect("mass")
            .volume_m3;
        assert!((actual - expected).abs() < 1e-12, "{actual} vs {expected}");
    }

    let before = model.outputs.clone();
    let FeatureDefinition::Rib(def) =
        &mut model.nodes.get_mut("feature:rib").expect("rib").definition
    else {
        panic!("rib")
    };
    def.centerline_ref = "ent:missing".into();
    assert!(model
        .regenerate(kernel, &registry, Some(&params), None)
        .is_err());
    assert_eq!(model.outputs, before);
}

#[test]
fn rib_mock_contract_and_rollback() {
    contract(&MockGeometryKernel, false);
}

#[test]
fn rib_occt_analytic_volume_bounds_and_cache() {
    contract(&OcctGeometryKernel::new(), true);
}

#[test]
fn rib_rejects_degenerate_values() {
    let mut model = rib_model();
    let registry = FeatureRegistry::with_defaults();
    let params = params("0 mm", "12 mm");
    let error = model
        .regenerate(&MockGeometryKernel, &registry, Some(&params), None)
        .expect_err("zero thickness");
    assert!(error.to_string().contains("rib thickness"), "{error}");

    let mut model = rib_model();
    let FeatureDefinition::Rib(def) =
        &mut model.nodes.get_mut("feature:rib").expect("rib").definition
    else {
        panic!("rib")
    };
    def.extent = ExtrudeExtent::Distance {
        length: Length::from_meters(0.0),
    };
    def.length_expr = None;
    let error = model
        .regenerate(&MockGeometryKernel, &registry, None, None)
        .expect_err("zero extent");
    assert!(error.to_string().contains("extrude length"), "{error}");
}
