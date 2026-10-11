//! ADR-035: Mock contracts and OCCT analytic integration evidence.
use opencad_core::Length;
use opencad_feature::datum::{
    resolved_datum_sketches, DatumAxisFeature, DatumGeometry, DatumPlaneFeature,
};
use opencad_feature::{
    bracket_base_plate, derive_feature_graph, revolve_bushing, FeatureDefinition, FeatureNode,
    FeatureRegistry, PartModel, RegenerationCache,
};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_graph::{bracket_parameters, revolve_parameters, ParameterEntry};
use opencad_kernel_occt::OcctGeometryKernel;
use opencad_sketch::Workplane;

fn derive(model: &mut PartModel) {
    let mut order = model.graph.ordered_ids().to_vec();
    order.sort_by_key(|id| {
        !matches!(
            model.nodes[id].definition,
            FeatureDefinition::DatumPlane(_) | FeatureDefinition::DatumAxis(_)
        )
    });
    model.graph = derive_feature_graph(
        &model.nodes.values().cloned().collect::<Vec<_>>(),
        &order,
        &model.sketches.values().cloned().collect::<Vec<_>>(),
        &[],
    )
    .expect("graph");
}
fn plate() -> PartModel {
    let mut model = bracket_base_plate().expect("plate");
    model
        .add_node(FeatureNode::new(
            "feature:datum",
            "Offset plane",
            FeatureDefinition::DatumPlane(DatumPlaneFeature {
                base: Workplane::xy(),
                offset: Length::from_meters(0.005),
                offset_expr: Some("offset".into()),
            }),
        ))
        .expect("datum");
    model
        .sketches
        .values_mut()
        .next()
        .expect("sketch")
        .workplane = Workplane::DatumPlane {
        feature_id: "feature:datum".into(),
    };
    derive(&mut model);
    model
}
fn plane_contract<K: GeometryKernel>(kernel: &K, analytic: bool) {
    let mut model = plate();
    let mut params = bracket_parameters();
    params
        .add_parameter(ParameterEntry::new("param:offset", "offset", "5 mm"))
        .expect("param");
    let registry = FeatureRegistry::with_defaults();
    let mut cache = RegenerationCache::new();
    model
        .regenerate_with_cache(kernel, &registry, Some(&params), None, &mut cache)
        .expect("regen");
    let body = model.active_body().expect("body");
    let Some(DatumGeometry::Plane {
        workplane: Workplane::Custom { origin, .. },
    }) = &model.outputs["feature:datum"].datum
    else {
        panic!("plane")
    };
    assert!((origin[2] - 0.005).abs() < 1e-12);
    if analytic {
        let bounds = kernel.bounding_box(body).expect("bounds");
        assert!((bounds.min[2] - 0.005).abs() < 1e-6, "{bounds:?}");
        assert!((bounds.max[2] - 0.011).abs() < 1e-6, "{bounds:?}");
        assert!(
            (kernel.mass_properties(body, 1.0).expect("volume").volume_m3
                - 80.0 * 60.0 * 6.0 * 1e-9)
                .abs()
                < 1e-9
        );
    }
    assert_eq!(
        model.outputs.values().filter(|o| o.body.is_some()).count(),
        1
    );
    assert!(matches!(
        resolved_datum_sketches(&model).expect("overlay")[0].workplane,
        Workplane::Custom { .. }
    ));
    let warm = model
        .regenerate_with_cache(kernel, &registry, Some(&params), None, &mut cache)
        .expect("warm");
    assert_eq!(warm.cached_nodes.len(), 3);
    let mut changed = params.clone();
    changed.set_expr("param:offset", "15 mm").expect("edit");
    model
        .regenerate_with_cache(kernel, &registry, Some(&changed), None, &mut cache)
        .expect("edit regen");
    let Some(DatumGeometry::Plane {
        workplane: Workplane::Custom { origin, .. },
    }) = &model.outputs["feature:datum"].datum
    else {
        panic!("plane")
    };
    assert!((origin[2] - 0.015).abs() < 1e-12);
    if analytic {
        assert!(
            (kernel
                .bounding_box(model.active_body().expect("body"))
                .expect("bounds")
                .min[2]
                - 0.015)
                .abs()
                < 1e-6
        );
    }
    model
        .nodes
        .get_mut("feature:datum")
        .expect("datum")
        .suppressed = true;
    let before = model.outputs.clone();
    assert!(model
        .regenerate(kernel, &registry, Some(&changed), None)
        .is_err());
    assert_eq!(model.outputs, before, "failed regen preserves outputs");
}
#[test]
fn mock_plane_contract() {
    plane_contract(&MockGeometryKernel::new(), false);
}
#[test]
fn occt_plane_analytic_integration() {
    plane_contract(&OcctGeometryKernel::new(), true);
}

fn axis_contract<K: GeometryKernel>(kernel: &K, analytic: bool) {
    let mut model = revolve_bushing().expect("bushing");
    let mut params = revolve_parameters("360 deg");
    params
        .add_parameter(ParameterEntry::new("param:axis_x", "axis_x", "5 mm"))
        .expect("param");
    model
        .add_node(FeatureNode::new(
            "feature:axis",
            "Axis",
            FeatureDefinition::DatumAxis(DatumAxisFeature {
                origin_m: [0.0; 3],
                direction: [0.0, 2.0, 0.0],
                origin_expr: Some([Some("axis_x".into()), None, None]),
            }),
        ))
        .expect("axis");
    let FeatureDefinition::Revolve(def) = &mut model
        .nodes
        .get_mut("feature:revolve_bushing")
        .expect("revolve")
        .definition
    else {
        panic!("revolve")
    };
    def.axis_feature = Some("feature:axis".into());
    derive(&mut model);
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(kernel, &registry, Some(&params), None)
        .expect("regen");
    let Some(DatumGeometry::Axis { direction, .. }) = &model.outputs["feature:axis"].datum else {
        panic!("axis output")
    };
    for (actual, expected) in direction.iter().zip([0.0, 1.0, 0.0]) {
        assert!((actual - expected).abs() < 1e-12);
    }
    if analytic {
        // Radii 15/25 mm about x=5 mm become 10/20 mm, height 20 mm.
        let expected = std::f64::consts::PI * (0.020_f64.powi(2) - 0.010_f64.powi(2)) * 0.020;
        let actual = kernel
            .mass_properties(model.active_body().expect("body"), 1.0)
            .expect("mass")
            .volume_m3;
        assert!((actual - expected).abs() < 1e-9, "{actual} vs {expected}");
    }
    params.set_expr("param:axis_x", "10 mm").expect("move axis");
    model
        .regenerate(kernel, &registry, Some(&params), None)
        .expect("moved axis");
    if analytic {
        let expected = std::f64::consts::PI * (0.015_f64.powi(2) - 0.005_f64.powi(2)) * 0.020;
        let actual = kernel
            .mass_properties(model.active_body().expect("body"), 1.0)
            .expect("mass")
            .volume_m3;
        assert!((actual - expected).abs() < 1e-9);
    }
    let FeatureDefinition::DatumAxis(def) = &mut model
        .nodes
        .get_mut("feature:axis")
        .expect("axis")
        .definition
    else {
        panic!("axis")
    };
    def.direction = [0.0; 3];
    assert!(model
        .regenerate(kernel, &registry, Some(&params), None)
        .is_err());
}
#[test]
fn mock_axis_contract() {
    axis_contract(&MockGeometryKernel::new(), false);
}
#[test]
fn occt_axis_analytic_integration() {
    axis_contract(&OcctGeometryKernel::new(), true);
}

#[test]
fn datum_dependencies_and_invalid_frames_fail_closed() {
    let mut model = plate();
    let FeatureDefinition::DatumPlane(def) = &mut model
        .nodes
        .get_mut("feature:datum")
        .expect("plane")
        .definition
    else {
        panic!("plane")
    };
    def.base = Workplane::DatumPlane {
        feature_id: "feature:datum".into(),
    };
    let nodes = model.nodes.values().cloned().collect::<Vec<_>>();
    let sketches = model.sketches.values().cloned().collect::<Vec<_>>();
    assert!(derive_feature_graph(&nodes, model.graph.ordered_ids(), &sketches, &[]).is_err());
    let mut model = plate();
    let FeatureDefinition::DatumPlane(def) = &mut model
        .nodes
        .get_mut("feature:datum")
        .expect("plane")
        .definition
    else {
        panic!("plane")
    };
    def.offset_expr = None;
    def.base = Workplane::Custom {
        origin: [0.0; 3],
        normal: [0.0, 0.0, 1.0],
        x_axis: [1.0, 0.0, 1.0],
    };
    assert!(model
        .regenerate(
            &MockGeometryKernel::new(),
            &FeatureRegistry::with_defaults(),
            None,
            None
        )
        .is_err());
    let mut model = plate();
    assert!(model
        .regenerate(
            &MockGeometryKernel::new(),
            &FeatureRegistry::with_defaults(),
            Some(&opencad_graph::ParamGraph::new()),
            None
        )
        .is_err());
}

#[test]
fn wrong_kind_datum_refuses_consumers() {
    let mut model = plate();
    model
        .nodes
        .get_mut("feature:datum")
        .expect("datum")
        .definition = FeatureDefinition::DatumAxis(DatumAxisFeature {
        origin_m: [0.0; 3],
        direction: [0.0, 1.0, 0.0],
        origin_expr: None,
    });
    let error = model
        .regenerate(
            &MockGeometryKernel::new(),
            &FeatureRegistry::with_defaults(),
            None,
            None,
        )
        .expect_err("axis is not a plane");
    assert!(error.to_string().contains("not a datum plane"));
}

#[test]
fn sketch_only_datum_consumer_refuses_suppressed_plane() {
    let mut model = plate();
    model.nodes.shift_remove("feature:extrude_base");
    let order = vec![
        "feature:datum".to_string(),
        "feature:sketch_base".to_string(),
    ];
    model.graph = derive_feature_graph(
        &model.nodes.values().cloned().collect::<Vec<_>>(),
        &order,
        &model.sketches.values().cloned().collect::<Vec<_>>(),
        &[],
    )
    .expect("graph");
    let FeatureDefinition::DatumPlane(def) = &mut model
        .nodes
        .get_mut("feature:datum")
        .expect("plane")
        .definition
    else {
        panic!("plane")
    };
    def.offset_expr = None;
    model
        .nodes
        .get_mut("feature:datum")
        .expect("plane")
        .suppressed = true;
    assert!(model
        .regenerate(
            &MockGeometryKernel::new(),
            &FeatureRegistry::with_defaults(),
            None,
            None
        )
        .is_err());
}
