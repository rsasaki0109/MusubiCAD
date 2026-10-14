//! ADR-069 authored source, physical phase, provenance and warm-cache admission.
use opencad_feature::*;
use opencad_geometry::{
    GeometryKernel, MockGeometryKernel, ReferenceStatus, TessellationSettings, ThreadHandedness,
};
use opencad_kernel_occt::OcctGeometryKernel;
use opencad_sketch::Workplane;
#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod evidence;
#[path = "support/nominal_thread_fixture.rs"]
mod fixture;
fn expected_radius(z: f64, theta: f64, hand: ThreadHandedness) -> f64 {
    let sign = if hand == ThreadHandedness::Right {
        1.
    } else {
        -1.
    };
    let q = (z / 0.001 - sign * theta / std::f64::consts::TAU).rem_euclid(1.);
    let t = q.min(1. - q);
    0.003
        - if t <= 0.0625 {
            0.
        } else if t >= 0.375 {
            0.000_541_265_877_365_274
        } else {
            0.001_732_050_807_568_877_2 * (t - 0.0625)
        }
}
fn physical<K: GeometryKernel>(k: &K) {
    for (turns, hand, tilt) in [
        (1, ThreadHandedness::Right, false),
        (8, ThreadHandedness::Left, false),
        (32, ThreadHandedness::Right, true),
    ] {
        let (mut m, params, mut refs) = fixture::fixture();
        let FeatureDefinition::NominalMetricThread(d) =
            &mut m.nodes.get_mut("feature:thread").unwrap().definition
        else {
            panic!()
        };
        d.turns = turns;
        d.handedness = hand;
        let length = turns as f64 * 0.001;
        let FeatureDefinition::Extrude(e) =
            &mut m.nodes.get_mut("feature:stock").unwrap().definition
        else {
            panic!()
        };
        e.extent = opencad_geometry::ExtrudeExtent::Distance {
            length: opencad_core::Length::from_meters(length),
        };
        let (x, y, axis, translation) = if tilt {
            (
                [0., 1., 0.],
                [0.6, 0., -0.8],
                [-0.8, 0., -0.6],
                [0.04, -0.02, 0.03],
            )
        } else {
            ([1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0.; 3])
        };
        m.sketches.get_mut("sketch:stock").unwrap().workplane = Workplane::Custom {
            origin: translation,
            normal: axis,
            x_axis: x,
        };
        refs[0].semantic.normal_hint = Some(x);
        let origin: [f64; 3] =
            std::array::from_fn(|i| translation[i] + x[i] * 0.012 + y[i] * 0.009);
        let mut cache = RegenerationCache::new();
        let registry = FeatureRegistry::with_defaults();
        let report = m
            .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
            .unwrap();
        assert_eq!(
            report.reference_provenance[0].status,
            ReferenceStatus::Consumed
        );
        assert!(report.face_history.is_empty());
        let body = m.active_body().unwrap();
        let mass = k.mass_properties(body, 1.).unwrap();
        let r = 0.003_f64;
        let root = r - 0.000_541_265_877_365_274;
        let piece = 0.001 * (root * root / 4. + r * r / 8.)
            + 2. * (r.powi(3) - root.powi(3)) / (3. * 3_f64.sqrt());
        let section = std::f64::consts::PI * piece / 0.001;
        let volume = section * length;
        assert!((mass.volume_m3 - volume).abs() < volume * 2e-5);
        let a = 0.001 / std::f64::consts::TAU;
        let dr = (r - root) / 8192.;
        let flank = (0..8192)
            .map(|i| {
                let v = root + (i as f64 + 0.5) * dr;
                (4. * v * v + 3. * a * a).sqrt() * dr
            })
            .sum::<f64>();
        let area = 2. * section
            + std::f64::consts::TAU * length / 0.001
                * (0.001 * (root / 4. + r / 8.) + 2. / 3_f64.sqrt() * flank);
        assert!((mass.area_m2 - area).abs() < area * 2e-5);
        for i in 0..3 {
            assert!((mass.center_of_mass[i] - origin[i] - axis[i] * length / 2.).abs() < 2e-8);
        }
        let stock = m.outputs["feature:stock"].body.as_ref().unwrap();
        assert!(
            (k.mass_properties(stock, 1.).unwrap().volume_m3
                - std::f64::consts::PI * r * r * length)
                .abs()
                < 1e-14
        );
        let mesh = k
            .tessellate(body, &TessellationSettings::default())
            .unwrap();
        let (mv, mc) = evidence::integrated(&mesh, false);
        assert!((mv - volume).abs() < volume * 0.025);
        for (i, v) in mc.iter().enumerate() {
            assert!((*v - mass.center_of_mass[i]).abs() < 1e-5);
        }
        let mut checked = 0;
        for p in &mesh.positions {
            let q = evidence::sub(p.map(f64::from), origin);
            let z = evidence::dot(q, axis);
            let px = evidence::dot(q, x);
            let py = evidence::dot(q, y);
            let radius = px.hypot(py);
            if z > 1e-7 && z < length - 1e-7 {
                assert!((radius - expected_radius(z, py.atan2(px), hand)).abs() < 1e-7);
                checked += 1;
            }
        }
        assert!(checked > 32);
        let key = &report.trace.output_hashes_sha256["feature:thread"];
        let mut entry = cache.get(key).unwrap().clone();
        assert!(entry
            .face_discoveries
            .iter()
            .all(|f| f.feature_id.as_deref() == Some("feature:thread")));
        assert!(entry
            .face_discoveries
            .iter()
            .any(|f| f.role == "thread_start"));
        assert!(entry
            .face_discoveries
            .iter()
            .any(|f| f.role == "thread_end"));

        let mut surface = opencad_geometry::TopoRef::face(
            opencad_core::TopoRefId::new("ref:face:thread_surface").unwrap(),
            "feature:thread",
            "thread_surface",
        );
        surface.geometric_fingerprint = Some(opencad_geometry::GeometricFingerprint {
            surface_type: "thread".into(),
            kernel_face_id: Some(
                entry
                    .face_discoveries
                    .iter()
                    .find(|f| f.role == "thread_surface")
                    .unwrap()
                    .kernel_face_id,
            ),
            kernel_edge_id: None,
            area_range: None,
            bbox_hint: None,
            adjacent_feature_ids: Vec::new(),
        });
        assert!(
            opencad_geometry::resolve_kernel_face_id_for_topo_ref_with_discoveries(
                std::slice::from_ref(&surface),
                &[],
                surface.ref_id.as_str(),
                Some(&entry.face_discoveries)
            )
            .is_err()
        );
        assert!(opencad_geometry::resolve_face_ref_with_provenance(
            std::slice::from_ref(&surface),
            &[],
            surface.ref_id.as_str(),
            Some(&entry.face_discoveries),
            Default::default(),
            true
        )
        .is_err());
        surface.semantic.role = None;
        assert!(
            opencad_geometry::resolve_kernel_face_id_for_topo_ref_with_discoveries(
                std::slice::from_ref(&surface),
                &[],
                surface.ref_id.as_str(),
                Some(&entry.face_discoveries)
            )
            .is_err()
        );
        let end = opencad_geometry::TopoRef::face(
            opencad_core::TopoRefId::new("ref:face:thread_end").unwrap(),
            "feature:thread",
            "thread_end",
        );
        assert!(
            opencad_geometry::resolve_kernel_face_id_for_topo_ref_with_discoveries(
                std::slice::from_ref(&end),
                &[],
                end.ref_id.as_str(),
                Some(&entry.face_discoveries)
            )
            .is_ok()
        );
        entry.face_discoveries.clear();
        cache.insert(key.clone(), entry);
        let warm = m
            .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
            .unwrap();
        assert!(warm.cached_nodes.contains(&"feature:thread".into()));
        assert_eq!(
            warm.reference_provenance[0].status,
            ReferenceStatus::Consumed
        );
    }
}
#[test]
fn mock_source_integrals_phase_placement_and_cache() {
    physical(&MockGeometryKernel::with_analytic_primitives());
}
#[test]
fn native_source_integrals_phase_placement_and_cache() {
    physical(&OcctGeometryKernel::new());
}
fn failures<K: GeometryKernel>(k: &K) {
    let (mut base, params, refs) = fixture::fixture();
    let registry = FeatureRegistry::with_defaults();
    let mut cache = RegenerationCache::new();
    let report = base
        .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
        .unwrap();
    for bad in [
        "missing",
        "foreign",
        "cap",
        "normal",
        "area",
        "fingerprint",
        "length",
        "radius",
        "turns",
        "nonunit",
        "suppressed",
        "operation",
        "fallback",
        "length_unit",
        "angular_param",
        "alias",
        "precision",
        "duplicate",
        "target",
        "point_unit",
        "constraint_unit",
    ] {
        let mut m = base.clone();
        let mut refs = refs.clone();
        let mut params = params.clone();
        let mut cache = cache.clone();
        let old = m.outputs.clone();
        match bad {
            "missing" => refs.clear(),
            "duplicate" => refs.push(refs[0].clone()),
            "point_unit" => {
                let opencad_sketch::SketchEntity::Point(p) =
                    &mut m.sketches.get_mut("sketch:stock").unwrap().entities[0]
                else {
                    panic!()
                };
                p.x = opencad_sketch::Coord::expr("12 deg").unwrap();
            }
            "constraint_unit" => {
                m.sketches
                    .get_mut("sketch:stock")
                    .unwrap()
                    .constraints
                    .push(opencad_sketch::Constraint::Radius {
                        id: opencad_core::ConstraintId::new("con:wrong").unwrap(),
                        target: opencad_core::EntityId::new("ent:circle").unwrap(),
                        expr: opencad_core::Expression::new("0.003 deg").unwrap(),
                    });
            }
            "precision" => {
                m.sketches.get_mut("sketch:stock").unwrap().workplane = Workplane::Custom {
                    origin: [1000., 0., 0.],
                    normal: [0., 0., 1.],
                    x_axis: [1., 0., 0.],
                }
            }
            "target" => {
                let FeatureDefinition::NominalMetricThread(d) =
                    &mut m.nodes.get_mut("feature:thread").unwrap().definition
                else {
                    panic!()
                };
                d.target_feature = "feature:sketch_stock".into();
            }
            "foreign" => refs[0].semantic.created_by = "feature:sketch_stock".into(),
            "cap" => {
                let stock = k
                    .cylindrical_stock_geometry(
                        m.outputs["feature:stock"].body.as_ref().unwrap(),
                        [0., 0., 1.],
                    )
                    .unwrap();
                refs[0]
                    .geometric_fingerprint
                    .as_mut()
                    .unwrap()
                    .kernel_face_id = Some(stock.start_face_id);
            }
            "normal" => refs[0].semantic.normal_hint = Some([0., 0., 1.]),
            "area" => {
                refs[0].geometric_fingerprint.as_mut().unwrap().area_range = Some([0., 1e-12])
            }
            "fingerprint" => {
                refs[0].geometric_fingerprint.as_mut().unwrap().surface_type = "plane".into()
            }
            "nonunit" => {
                m.sketches.get_mut("sketch:stock").unwrap().workplane = Workplane::Custom {
                    origin: [0.; 3],
                    normal: [0., 0., 2.],
                    x_axis: [1., 0., 0.],
                }
            }
            "suppressed" => m.nodes.get_mut("feature:stock").unwrap().suppressed = true,
            "radius" => {
                let opencad_sketch::SketchEntity::Circle(c) =
                    &mut m.sketches.get_mut("sketch:stock").unwrap().entities[1]
                else {
                    panic!()
                };
                c.radius = opencad_sketch::Coord::literal(0.004);
            }
            "turns" => {
                let FeatureDefinition::NominalMetricThread(d) =
                    &mut m.nodes.get_mut("feature:thread").unwrap().definition
                else {
                    panic!()
                };
                d.turns = 0;
            }
            "alias" => {
                let mut n = m.nodes["feature:thread"].clone();
                n.id = "feature:alias".into();
                m.nodes.insert(n.id.clone(), n);
            }
            other => {
                let FeatureDefinition::Extrude(e) =
                    &mut m.nodes.get_mut("feature:stock").unwrap().definition
                else {
                    panic!()
                };
                match other {
                    "operation" => e.operation = opencad_geometry::ExtrudeOperation::Cut,
                    "fallback" => {
                        e.extent = opencad_geometry::ExtrudeExtent::Distance {
                            length: opencad_core::Length::from_meters(-1.),
                        };
                        e.length_expr = Some("8 mm".into());
                    }
                    "length_unit" => e.length_expr = Some("8 deg".into()),
                    "angular_param" => {
                        params
                            .add_parameter(opencad_graph::ParameterEntry::new(
                                "param:wrong",
                                "length_rad",
                                "0.008 rad",
                            ))
                            .unwrap();
                        e.length_expr = Some("length_rad".into());
                    }
                    _ => {
                        e.extent = opencad_geometry::ExtrudeExtent::Distance {
                            length: opencad_core::Length::from_meters(0.009),
                        }
                    }
                }
            }
        }
        assert!(
            m.regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
                .is_err(),
            "{bad}"
        );
        assert_eq!(m.outputs, old, "{bad}");
    }
    // Poison only the cached output with a physically valid opposite-hand solid.
    let mut other = base.clone();
    let FeatureDefinition::NominalMetricThread(d) =
        &mut other.nodes.get_mut("feature:thread").unwrap().definition
    else {
        panic!()
    };
    d.handedness = ThreadHandedness::Left;
    other
        .regenerate(k, &registry, Some(&params), Some(&refs))
        .unwrap();
    let key = &report.trace.output_hashes_sha256["feature:thread"];
    let mut poisoned = cache.get(key).unwrap().clone();
    poisoned.output = other.outputs["feature:thread"].clone();
    cache.insert(key.clone(), poisoned);
    let old = base.outputs.clone();
    assert!(base
        .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
        .is_err());
    assert_eq!(base.outputs, old);
}
#[test]
fn mock_refusals_and_wrong_hand_cache_preserve_previous_output() {
    failures(&MockGeometryKernel::with_analytic_primitives());
}
#[test]
fn native_refusals_and_wrong_hand_cache_preserve_previous_output() {
    failures(&OcctGeometryKernel::new());
}
#[test]
fn closed_wire_sizes_hands_and_genuine_older_reader() {
    let (m, _, _) = fixture::fixture();
    let v = serde_json::to_value(&m.nodes["feature:thread"].definition).unwrap();
    for size in ["m3", "m4", "m5", "m6", "m8", "m10", "m12"] {
        for hand in ["right", "left"] {
            let mut a = v.clone();
            a["size"] = size.into();
            a["handedness"] = hand.into();
            let d: FeatureDefinition = serde_json::from_value(a).unwrap();
            d.validate_values(&Default::default()).unwrap();
        }
    }
    for field in [
        "representation",
        "size",
        "handedness",
        "turns",
        "ends",
        "face_ref",
        "target_feature",
    ] {
        let mut a = v.clone();
        a.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<FeatureDefinition>(a).is_err());
    }
    for (field, value) in [
        ("turns", serde_json::json!(0)),
        ("turns", serde_json::json!(33)),
        ("turns", serde_json::json!(1.5)),
        ("turns", serde_json::json!("8")),
        ("ends", serde_json::json!("runout")),
        ("size", serde_json::json!("m7")),
        ("handedness", serde_json::json!("auto")),
        ("representation", serde_json::json!("cosmetic")),
        ("pitch", serde_json::json!({"value_si":0.001})),
    ] {
        let mut a = v.clone();
        a[field] = value;
        assert!(
            serde_json::from_value::<FeatureDefinition>(a).is_err(),
            "{field}"
        );
    }
    #[derive(serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum Old {
        Extrude(ExtrudeFeature),
        CylindricalDraft(CylindricalDraftFeature),
    }
    let Old::Extrude(e) =
        serde_json::from_value(serde_json::to_value(&m.nodes["feature:stock"].definition).unwrap())
            .unwrap()
    else {
        panic!()
    };
    assert_eq!(e.sketch_feature, "feature:sketch_stock");
    let old:Old=serde_json::from_value(serde_json::json!({"type":"cylindrical_draft","target_feature":"feature:stock","face_ref":"ref:face:side","angle":{"value_si":0.1},"neutral_plane":{"type":"global","plane":"XY"}})).unwrap();
    let Old::CylindricalDraft(d) = old else {
        panic!()
    };
    assert_eq!(d.target_feature, "feature:stock");
    assert!(serde_json::from_value::<Old>(v).is_err());
}

#[test]
fn synthetic_mock_and_unscoped_source_expression_refuse() {
    let (mut m, p, r) = fixture::fixture();
    assert!(m
        .regenerate(
            &MockGeometryKernel::default(),
            &FeatureRegistry::with_defaults(),
            Some(&p),
            Some(&r)
        )
        .is_err());
    let (mut m, _, r) = fixture::fixture();
    let FeatureDefinition::Extrude(e) = &mut m.nodes.get_mut("feature:stock").unwrap().definition
    else {
        panic!()
    };
    e.length_expr = Some("8 deg".into());
    assert!(m
        .regenerate(
            &MockGeometryKernel::with_analytic_primitives(),
            &FeatureRegistry::with_defaults(),
            None,
            Some(&r)
        )
        .is_err());
}
