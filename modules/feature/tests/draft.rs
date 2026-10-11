//! Mock contract and OCCT integration: analytic wedge volumes (ADR-036).
use opencad_core::{Angle, TopoRefId};
use opencad_feature::{
    bracket_base_plate, derive_feature_graph, DraftFeature, FeatureDefinition, FeatureNode,
    FeatureRegistry, PartModel, RegenerationCache,
};
use opencad_geometry::{GeometryKernel, MockGeometryKernel, TopoRef};
use opencad_graph::{bracket_parameters, ParameterEntry};
use opencad_kernel_occt::OcctGeometryKernel;
use opencad_sketch::Workplane;

fn model() -> (PartModel, Vec<TopoRef>) {
    let mut model = bracket_base_plate().expect("plate");
    model
        .add_node(FeatureNode::new(
            "feature:draft",
            "Draft side",
            FeatureDefinition::Draft(DraftFeature {
                target_feature: "feature:extrude_base".into(),
                angle: Angle::from_degrees(5.0),
                angle_expr: Some("draft_angle".into()),
                neutral_plane: Workplane::xy(),
                face_refs: vec!["ref:face:right".into()],
            }),
        ))
        .expect("draft");
    let refs = vec![TopoRef::face(
        TopoRefId::new("ref:face:right").expect("id"),
        "feature:extrude_base",
        "+x",
    )];
    let order = model.graph.ordered_ids().to_vec();
    model.graph = derive_feature_graph(
        &model.nodes.values().cloned().collect::<Vec<_>>(),
        &order,
        &model.sketches.values().cloned().collect::<Vec<_>>(),
        &refs,
    )
    .expect("graph");
    (model, refs)
}
fn contract<K: GeometryKernel>(kernel: &K, analytic: bool) {
    let (mut model, mut refs) = model();
    if !analytic {
        refs[0] = TopoRef::face(
            TopoRefId::new("ref:face:right").expect("id"),
            "feature:extrude_base",
            "top",
        );
        let FeatureDefinition::Draft(def) = &mut model
            .nodes
            .get_mut("feature:draft")
            .expect("node")
            .definition
        else {
            panic!("draft")
        };
        def.neutral_plane = Workplane::xz();
    }
    let mut params = bracket_parameters();
    params
        .add_parameter(ParameterEntry::new("param:draft", "draft_angle", "5 deg"))
        .expect("param");
    let registry = FeatureRegistry::with_defaults();
    let mut cache = RegenerationCache::new();
    for angle in [5.0_f64, -5.0, 10.0] {
        params
            .set_expr("param:draft", format!("{angle} deg"))
            .expect("edit");
        model
            .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
            .expect("regen");
        if analytic {
            let body = model.active_body().expect("body");
            let expected =
                0.08 * 0.06 * 0.006 - 0.5 * 0.06 * 0.006_f64.powi(2) * angle.to_radians().tan();
            let actual = kernel.mass_properties(body, 1.0).expect("mass").volume_m3;
            assert!(
                (actual - expected).abs() < 1e-12,
                "angle={angle} actual={actual} expected={expected}"
            );
            let bb = kernel.bounding_box(body).expect("bounds");
            assert!((bb.min[2]).abs() < 1e-6 && (bb.max[2] - 0.006).abs() < 1e-6);
            let mesh = kernel.tessellate(body, &Default::default()).expect("mesh");
            let top_x = mesh
                .positions
                .iter()
                .filter(|p| (f64::from(p[2]) - 0.006).abs() < 1e-6)
                .map(|p| f64::from(p[0]))
                .fold(f64::NEG_INFINITY, f64::max);
            assert!((top_x - (0.08 - 0.006 * angle.to_radians().tan())).abs() < 1e-6);
            let expected_x = 0.08 + (-angle.to_radians().tan() * 0.006).max(0.0);
            assert!((bb.max[0] - expected_x).abs() < 1e-6, "{bb:?}");
        }
        let warm = model
            .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
            .expect("warm");
        assert_eq!(warm.cached_nodes.len(), 3);
    }
    let outputs = model.outputs.clone();
    let FeatureDefinition::Draft(def) = &mut model
        .nodes
        .get_mut("feature:draft")
        .expect("node")
        .definition
    else {
        panic!("draft")
    };
    def.face_refs = vec!["ref:face:missing".into()];
    assert!(model
        .regenerate(kernel, &registry, Some(&params), Some(&refs))
        .is_err());
    assert_eq!(model.outputs, outputs);
}
#[test]
fn draft_mock_contract_and_rollback() {
    contract(&MockGeometryKernel, false);
}
#[test]
fn draft_occt_analytic_volume_bounds_and_cache() {
    contract(&OcctGeometryKernel::new(), true);
}

#[test]
fn draft_rejects_invalid_and_unsupported_inputs() {
    use opencad_core::Length;
    use opencad_geometry::{DraftInput, ExtrudeExtent, ExtrudeOperation, FacePick, SolvedSketch};
    let kernel = OcctGeometryKernel::new();
    let circle = SolvedSketch {
        circle: Some(opencad_geometry::SolvedCircle {
            center_m: [0.0, 0.0],
            radius_m: 0.01,
        }),
        profile_ref: "circle".into(),
        points: vec![],
        closed: true,
        segments: vec![],
        placement: None,
    };
    let wire = kernel.make_wire_from_sketch(&circle).expect("wire");
    let body = kernel
        .extrude(
            wire,
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.02),
            },
            ExtrudeOperation::NewBody,
            None,
            [0.0, 0.0, 1.0],
        )
        .expect("cylinder");
    let mut input = DraftInput {
        angle_rad: 5_f64.to_radians(),
        neutral_origin_m: [0.0; 3],
        neutral_normal: [0.0, 0.0, 1.0],
        faces: vec![FacePick {
            point_m: [0.01, 0.0, 0.01],
            normal: [1.0, 0.0, 0.0],
        }],
    };
    assert!(kernel.draft_body(body.clone(), &input).is_err());
    input.angle_rad = f64::NAN;
    assert!(MockGeometryKernel.draft_body(body, &input).is_err());
    for angle in [0.0, 46_f64.to_radians(), f64::INFINITY] {
        assert!(opencad_feature::draft::validate_angle(angle).is_err());
    }
}

#[test]
fn draft_occt_refuses_nonconvex_planar_solid() {
    use opencad_core::Length;
    use opencad_geometry::{DraftInput, ExtrudeExtent, ExtrudeOperation, FacePick, SolvedSketch};
    let kernel = OcctGeometryKernel::new();
    let profile = SolvedSketch {
        profile_ref: "L-profile".into(),
        points: vec![
            [0.0, 0.0],
            [0.02, 0.0],
            [0.02, 0.01],
            [0.01, 0.01],
            [0.01, 0.02],
            [0.0, 0.02],
        ],
        closed: true,
        circle: None,
        segments: vec![],
        placement: None,
    };
    let wire = kernel.make_wire_from_sketch(&profile).expect("wire");
    let body = kernel
        .extrude(
            wire,
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.01),
            },
            ExtrudeOperation::NewBody,
            None,
            [0.0, 0.0, 1.0],
        )
        .expect("L prism");
    let before = kernel.mass_properties(&body, 1.0).expect("mass").volume_m3;
    let input = DraftInput {
        angle_rad: 5_f64.to_radians(),
        neutral_origin_m: [0.0; 3],
        neutral_normal: [0.0, 0.0, 1.0],
        faces: vec![FacePick {
            point_m: [0.02, 0.005, 0.005],
            normal: [1.0, 0.0, 0.0],
        }],
    };
    let error = kernel
        .draft_body(body.clone(), &input)
        .expect_err("nonconvex refusal");
    assert!(error.to_string().contains("convex"), "{error}");
    assert!(
        (kernel
            .mass_properties(&body, 1.0)
            .expect("source unchanged")
            .volume_m3
            - before)
            .abs()
            < 1e-15
    );
}
