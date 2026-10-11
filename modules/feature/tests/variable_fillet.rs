//! MCAD-P11-005: semantic linear radius laws and native OCCT integration.
use opencad_core::{Length, TopoRefId};
use opencad_feature::{
    bracket_base_plate, FeatureDefinition, FeatureNode, FeatureRegistry, FilletFeature,
    FilletRadiusLaw, PartModel, RegenerationCache,
};
use opencad_geometry::{GeometryKernel, MockGeometryKernel, TopoRef};
use opencad_graph::{bracket_parameters, ParameterEntry};
use opencad_kernel_occt::OcctGeometryKernel;

fn model() -> (PartModel, Vec<TopoRef>) {
    let mut model = bracket_base_plate().expect("plate");
    let mut fillet = FilletFeature::on_edge_ref(
        "feature:extrude_base",
        "ref:edge:front",
        Length::from_meters(0.0005),
        Some("start_radius".into()),
    );
    fillet.radius_law = Some(FilletRadiusLaw::Linear {
        end_radius: Length::from_meters(0.0015),
        end_radius_expr: Some("end_radius".into()),
    });
    model
        .add_node(FeatureNode::new(
            "feature:variable_fillet",
            "Linear fillet",
            FeatureDefinition::Fillet(fillet),
        ))
        .expect("fillet");
    model
        .add_dependency("feature:extrude_base", "feature:variable_fillet")
        .expect("edge");
    let refs = vec![TopoRef::edge(
        TopoRefId::new("ref:edge:front").expect("id"),
        "feature:extrude_base",
        "top@-y",
    )];
    (model, refs)
}
fn contract<K: GeometryKernel>(kernel: &K, analytic: bool) {
    let (mut model, refs) = model();
    let mut params = bracket_parameters();
    params
        .add_parameter(ParameterEntry::new(
            "param:start_radius",
            "start_radius",
            "0.5 mm",
        ))
        .expect("param");
    params
        .add_parameter(ParameterEntry::new(
            "param:end_radius",
            "end_radius",
            "1.5 mm",
        ))
        .expect("param");
    let registry = FeatureRegistry::with_defaults();
    let mut cache = RegenerationCache::new();
    for (start, end) in [(1.0_f64, 1.0_f64), (0.5, 1.5), (1.5, 0.5)] {
        params
            .set_expr("param:start_radius", format!("{start} mm"))
            .expect("edit");
        params
            .set_expr("param:end_radius", format!("{end} mm"))
            .expect("edit");
        model
            .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
            .expect("regen");
        if analytic {
            let body = model.active_body().expect("body");
            let removed = (1.0 - std::f64::consts::FRAC_PI_4)
                * 80.0
                * (start * start + start * end + end * end)
                / 3.0
                * 1e-9;
            let actual = kernel.mass_properties(body, 1.0).expect("mass").volume_m3;
            let expected = 0.080 * 0.060 * 0.006 - removed;
            assert!((actual-expected).abs() < removed*1e-3,"r={start},{end}: actual={actual}, expected={expected}, removed={removed}, error={}",actual-expected);
            let points: Vec<_> = kernel
                .discover_body_edges(body)
                .expect("edges")
                .iter()
                .flat_map(|edge| {
                    kernel
                        .edge_endpoints(body, edge.kernel_edge_id)
                        .expect("endpoints")
                })
                .collect();
            for expected_point in [[0.0, start * 1e-3, 0.006], [0.080, end * 1e-3, 0.006]] {
                assert!(
                    points.iter().any(|p| p
                        .iter()
                        .zip(expected_point)
                        .all(|(actual, expected)| (actual - expected).abs() < 2e-6)),
                    "r={start},{end}: missing contact {expected_point:?}; points={points:?}"
                );
            }
            let bounds = kernel.bounding_box(body).expect("bounds");
            assert!((bounds.max[2] - 0.006).abs() < 1e-6);
        }
        let warm = model
            .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
            .expect("cache");
        assert!(warm
            .cached_nodes
            .contains(&"feature:variable_fillet".into()));
    }
    let before = model.outputs.clone();
    let FeatureDefinition::Fillet(def) = &mut model
        .nodes
        .get_mut("feature:variable_fillet")
        .expect("node")
        .definition
    else {
        panic!("fillet")
    };
    def.edge_ref = Some("ref:edge:missing".into());
    assert!(model
        .regenerate(kernel, &registry, Some(&params), Some(&refs))
        .is_err());
    assert_eq!(model.outputs, before);
}
#[test]
fn mock_linear_fillet_contract_and_rollback() {
    contract(&MockGeometryKernel, false);
}
#[test]
fn occt_linear_fillet_matches_straight_corner_volume() {
    contract(&OcctGeometryKernel::new(), true);
}
#[test]
fn invalid_variable_law_selection_and_radii_are_refused() {
    let (mut model, refs) = model();
    let FeatureDefinition::Fillet(def) = &mut model
        .nodes
        .get_mut("feature:variable_fillet")
        .expect("node")
        .definition
    else {
        panic!("fillet")
    };
    def.edge_ref = None;
    assert!(model
        .regenerate(
            &MockGeometryKernel,
            &FeatureRegistry::with_defaults(),
            None,
            Some(&refs)
        )
        .is_err());
    assert!(opencad_geometry::kernel::validate_variable_fillet_radii(0.001, f64::NAN).is_err());
    assert!(opencad_geometry::kernel::validate_variable_fillet_radii(0.001, 0.0).is_err());
}

#[test]
fn semantic_face_fillets_resolve_and_refuse_unknown_faces() {
    for kernel in [false, true] {
        let mut model = bracket_base_plate().expect("plate");
        let mut face_fillet = FilletFeature::on_face_ref(
            "feature:extrude_base",
            "ref:face:top",
            Length::from_meters(0.001),
            None,
        );
        // A supplied fallback must never hide a missing semantic face.
        face_fillet.edge_selector = opencad_geometry::FilletEdgeSelector::EdgeRole {
            role: "top@-y".into(),
        };
        model
            .add_node(FeatureNode::new(
                "feature:face_fillet",
                "Face perimeter",
                FeatureDefinition::Fillet(face_fillet),
            ))
            .expect("fillet");
        model
            .add_dependency("feature:extrude_base", "feature:face_fillet")
            .expect("dependency");
        let mut refs = vec![TopoRef::face(
            TopoRefId::new("ref:face:top").expect("id"),
            "feature:extrude_base",
            "top",
        )];
        let registry = FeatureRegistry::with_defaults();
        if kernel {
            let occt = OcctGeometryKernel::new();
            model
                .regenerate(&occt, &registry, None, Some(&refs))
                .expect("face fillet");
            assert!(
                occt.mass_properties(model.active_body().expect("body"), 1.0)
                    .expect("mass")
                    .volume_m3
                    < 0.080 * 0.060 * 0.006
            );
            let before = model.outputs.clone();
            refs[0].semantic.role = Some("absent".into());
            assert!(model
                .regenerate(&occt, &registry, None, Some(&refs))
                .is_err());
            assert_eq!(before, model.outputs);
        } else {
            model
                .regenerate(&MockGeometryKernel, &registry, None, Some(&refs))
                .expect("face fillet");
            let before = model.outputs.clone();
            refs[0].semantic.role = Some("absent".into());
            assert!(model
                .regenerate(&MockGeometryKernel, &registry, None, Some(&refs))
                .is_err());
            assert_eq!(before, model.outputs);
        }
    }
}

#[test]
fn occt_refuses_ambiguous_roles_and_curved_edges() {
    use opencad_geometry::FilletEdgeSelector;
    let mut model = opencad_feature::bracket_with_hole().expect("plate with hole");
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();
    let mut refs = opencad_feature::bracket_semantic_refs();
    model
        .regenerate(&kernel, &registry, Some(&params), Some(&refs))
        .expect("plate");
    let body = model.active_body().expect("body").clone();
    let edges = kernel.discover_body_edges(&body).expect("edges");
    let curved = edges
        .iter()
        .find(|edge| edge.length_m < 1e-6)
        .expect("circular seam");
    let err = kernel
        .variable_fillet_edge(
            body,
            0.0005,
            0.0015,
            FilletEdgeSelector::KernelEdges {
                kernel_edge_ids: vec![curved.kernel_edge_id],
            },
        )
        .expect_err("curved edge");
    assert!(err.to_string().contains("straight edge"), "{err}");
    let duplicate = edges
        .iter()
        .find(|edge| {
            edges
                .iter()
                .filter(|candidate| candidate.role == edge.role)
                .count()
                > 1
        })
        .expect("ambiguous role");
    refs.push(TopoRef::edge(
        TopoRefId::new("ref:edge:ambiguous").expect("id"),
        "feature:hole_mount",
        &duplicate.role,
    ));
    let mut fillet = FilletFeature::on_edge_ref(
        "feature:hole_mount",
        "ref:edge:ambiguous",
        Length::from_meters(0.0005),
        None,
    );
    fillet.radius_law = Some(FilletRadiusLaw::Linear {
        end_radius: Length::from_meters(0.0015),
        end_radius_expr: None,
    });
    model
        .add_node(FeatureNode::new(
            "feature:ambiguous_fillet",
            "Ambiguous fillet",
            FeatureDefinition::Fillet(fillet),
        ))
        .expect("node");
    model
        .add_dependency("feature:hole_mount", "feature:ambiguous_fillet")
        .expect("dependency");
    let before = model.outputs.clone();
    let err = model
        .regenerate(&kernel, &registry, Some(&params), Some(&refs))
        .expect_err("ambiguous role");
    assert!(err.to_string().contains("Ambiguous"), "{err}");
    assert_eq!(before, model.outputs);

    // A stale kernel ID without a geometric fingerprint cannot break a tie.
    refs.last_mut()
        .expect("ambiguous reference")
        .geometric_fingerprint = Some(opencad_geometry::GeometricFingerprint {
        surface_type: "brep_edge".into(),
        kernel_face_id: None,
        kernel_edge_id: Some(u64::MAX),
        area_range: None,
        bbox_hint: None,
        adjacent_feature_ids: Vec::new(),
    });
    let err = model
        .regenerate(&kernel, &registry, Some(&params), Some(&refs))
        .expect_err("stale ambiguous role");
    assert!(err.to_string().contains("Ambiguous"), "{err}");
    assert_eq!(before, model.outputs);
}
