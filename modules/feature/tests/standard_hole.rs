//! MCAD-P11-004: standard table, Mock contract and OCCT cylinder evidence.
use opencad_core::{EntityId, Length, SketchId};
use opencad_feature::{
    bracket_base_plate, derive_feature_graph, ClearanceFit, FeatureDefinition, FeatureNode,
    FeatureRegistry, HoleFeature, HoleStandard, MetricSize, PartModel, RegenerationCache,
    SketchFeatureDef, StandardHoleFeature, ThreadRepresentation,
};
use opencad_geometry::{ExtrudeExtent, GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
use opencad_sketch::{
    entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
    Sketch, Workplane,
};

fn model(standard: HoleStandard, diameter_m: f64) -> PartModel {
    let mut model = bracket_base_plate().expect("base");
    let mut sketch = Sketch::new(
        SketchId::new("sketch:standard").expect("id"),
        "Standard hole",
        Workplane::Custom {
            origin: [0.0, 0.0, 0.006],
            normal: [0.0, 0.0, 1.0],
            x_axis: [1.0, 0.0, 0.0],
        },
    );
    sketch
        .add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new("ent:center").expect("id"),
                construction: false,
            },
            x: Coord::literal(0.020),
            y: Coord::literal(0.020),
        }))
        .expect("point");
    sketch
        .add_entity(SketchEntity::Circle(CircleEntity {
            base: EntityBase {
                id: EntityId::new("ent:circle").expect("id"),
                construction: false,
            },
            center: EntityId::new("ent:center").expect("id"),
            radius: Coord::literal(diameter_m / 2.0),
        }))
        .expect("circle");
    model.add_sketch(sketch).expect("sketch");
    model
        .add_node(FeatureNode::new(
            "feature:sketch_standard",
            "Hole sketch",
            FeatureDefinition::Sketch(SketchFeatureDef {
                sketch_id: "sketch:standard".into(),
            }),
        ))
        .expect("feature");
    model
        .add_node(FeatureNode::new(
            "feature:standard",
            "Standard hole",
            FeatureDefinition::StandardHole(StandardHoleFeature {
                hole: HoleFeature::through(
                    "feature:sketch_standard",
                    "sketch:standard/profile:outer",
                    ExtrudeExtent::Distance {
                        length: Length::from_meters(0.003),
                    },
                    "feature:extrude_base",
                ),
                standard,
            }),
        ))
        .expect("hole");
    let order = [
        "feature:sketch_base",
        "feature:extrude_base",
        "feature:sketch_standard",
        "feature:standard",
    ]
    .map(String::from);
    model.graph = derive_feature_graph(
        &model.nodes.values().cloned().collect::<Vec<_>>(),
        &order,
        &model.sketches.values().cloned().collect::<Vec<_>>(),
        &[],
    )
    .expect("graph");
    model
}

#[test]
fn supported_table_matches_explicit_mm_values() {
    // Independent tabulated expectations, including pitch and recommended drills.
    for (size, nominal, pitch, drill, clearances) in [
        (MetricSize::M3, 3.0, 0.5, 2.5, [3.2, 3.4, 3.6]),
        (MetricSize::M4, 4.0, 0.7, 3.3, [4.3, 4.5, 4.8]),
        (MetricSize::M5, 5.0, 0.8, 4.2, [5.3, 5.5, 5.8]),
        (MetricSize::M6, 6.0, 1.0, 5.0, [6.4, 6.6, 7.0]),
        (MetricSize::M8, 8.0, 1.25, 6.75, [8.4, 9.0, 10.0]),
        (MetricSize::M10, 10.0, 1.5, 8.5, [10.5, 11.0, 12.0]),
        (MetricSize::M12, 12.0, 1.75, 10.25, [13.0, 13.5, 14.5]),
    ] {
        let tapped = HoleStandard::IsoMetricCoarse {
            size,
            representation: ThreadRepresentation::TapDrill,
        }
        .dimensions();
        assert!((tapped.nominal_diameter.meters() - nominal * 1e-3).abs() < 1e-12);
        assert!((tapped.pitch.meters() - pitch * 1e-3).abs() < 1e-12);
        assert!((tapped.cut_diameter.meters() - drill * 1e-3).abs() < 1e-12);
        for (fit, diameter) in [
            ClearanceFit::Close,
            ClearanceFit::Normal,
            ClearanceFit::Loose,
        ]
        .into_iter()
        .zip(clearances)
        {
            let dims = HoleStandard::Iso273 { size, fit }.dimensions();
            assert!((dims.cut_diameter.meters() - diameter * 1e-3).abs() < 1e-12);
        }
    }
    for json in [
        r#"{"type":"iso273","size":"M7","fit":"normal"}"#,
        r#"{"type":"iso_metric_coarse","size":"M6","representation":"modeled"}"#,
        r#"{"type":"iso_metric_coarse","size":"M6","representation":"tap_drill","pitch_m":0.002}"#,
        r#"{"type":"iso_metric_coarse","size":"M6"}"#,
    ] {
        assert!(serde_json::from_str::<HoleStandard>(json).is_err());
    }
}

fn contract<K: GeometryKernel>(kernel: &K, analytic: bool) {
    for (standard, diameter_m) in [
        (
            HoleStandard::Iso273 {
                size: MetricSize::M6,
                fit: ClearanceFit::Normal,
            },
            0.0066,
        ),
        (
            HoleStandard::IsoMetricCoarse {
                size: MetricSize::M6,
                representation: ThreadRepresentation::TapDrill,
            },
            0.005,
        ),
    ] {
        let mut model = model(standard, diameter_m);
        let registry = FeatureRegistry::with_defaults();
        let mut cache = RegenerationCache::new();
        model
            .regenerate_with_cache(kernel, &registry, None, None, &mut cache)
            .expect("regen");
        let body = model.active_body().expect("body");
        if analytic {
            let expected =
                0.080 * 0.060 * 0.006 - std::f64::consts::PI * (diameter_m / 2.0).powi(2) * 0.003;
            let actual = kernel.mass_properties(body, 1.0).expect("mass").volume_m3;
            assert!((actual - expected).abs() < 1e-12, "{actual} vs {expected}");
            let bounds = kernel.bounding_box(body).expect("bounds");
            assert!((bounds.max[2] - 0.006).abs() < 1e-6);
        }
        let warm = model
            .regenerate_with_cache(kernel, &registry, None, None, &mut cache)
            .expect("cache");
        assert!(warm.cached_nodes.contains(&"feature:standard".into()));
        let before = model.outputs.clone();
        let FeatureDefinition::StandardHole(def) = &mut model
            .nodes
            .get_mut("feature:standard")
            .expect("hole")
            .definition
        else {
            panic!("hole")
        };
        def.standard = HoleStandard::Iso273 {
            size: MetricSize::M12,
            fit: ClearanceFit::Loose,
        };
        let err = model
            .regenerate_with_cache(kernel, &registry, None, None, &mut cache)
            .expect_err("mismatch");
        assert!(err.to_string().contains("diameter mismatch"), "{err}");
        assert_eq!(model.outputs, before);
    }
}
#[test]
fn mock_standard_hole_contract_and_rollback() {
    contract(&MockGeometryKernel, false);
}
#[test]
fn occt_standard_holes_match_analytic_cylinders() {
    contract(&OcctGeometryKernel::new(), true);
}

#[test]
fn noncircular_standard_hole_profile_is_refused() {
    let mut model = model(
        HoleStandard::Iso273 {
            size: MetricSize::M6,
            fit: ClearanceFit::Normal,
        },
        0.0066,
    );
    let FeatureDefinition::StandardHole(def) = &mut model
        .nodes
        .get_mut("feature:standard")
        .expect("hole")
        .definition
    else {
        panic!("hole")
    };
    def.hole.sketch_feature = "feature:sketch_base".into();
    def.hole.profile_ref = "sketch:base/profile:outer".into();
    let err = model
        .regenerate(
            &MockGeometryKernel,
            &FeatureRegistry::with_defaults(),
            None,
            None,
        )
        .expect_err("rectangle");
    assert!(err.to_string().contains("circular profile"), "{err}");
}
