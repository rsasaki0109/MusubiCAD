use opencad_feature::*;
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod evidence;
#[path = "support/tapered_rib_fixture.rs"]
mod fixture;
fn physical<K: GeometryKernel>(k: &K, normals: bool) {
    for tilt in [0_f64, 25.] {
        for low in [false, true] {
            let (mut model, mut params, mut refs) = fixture::fixture();
            let angle = tilt.to_radians();
            let axis = [0., -angle.sin(), angle.cos()];
            let y = [0., angle.cos(), angle.sin()];
            let translation = if tilt == 0. {
                [0.; 3]
            } else {
                [0.05, -0.03, 0.08]
            };
            let sign = if low { -1. } else { 1. };
            model.sketches.get_mut("sketch:base").unwrap().workplane =
                opencad_sketch::Workplane::Custom {
                    origin: translation,
                    normal: axis,
                    x_axis: [1., 0., 0.],
                };
            let base_center: [f64; 3] = std::array::from_fn(|i| {
                translation[i]
                    + [0.02, 0., 0.][i]
                    + y[i] * 0.015
                    + axis[i] * if low { 0. } else { 0.01 }
            });
            let stock_center: [f64; 3] = std::array::from_fn(|i| {
                translation[i] + [0.02, 0., 0.][i] + y[i] * 0.015 + axis[i] * 0.005
            });
            let rib_sketch = model.sketches.get_mut("sketch:rib").unwrap();
            rib_sketch.workplane = opencad_sketch::Workplane::Custom {
                origin: std::array::from_fn(|i| {
                    translation[i] + axis[i] * if low { 0. } else { 0.01 }
                }),
                normal: axis.map(|v| v * sign),
                x_axis: [1., 0., 0.],
            };
            if low {
                for e in &mut rib_sketch.entities {
                    if let opencad_sketch::SketchEntity::Point(p) = e {
                        p.y = opencad_sketch::Coord::literal(-0.015);
                    }
                }
            }
            refs[0].semantic.role = Some(
                if low {
                    "extrusion_start"
                } else {
                    "extrusion_end"
                }
                .into(),
            );
            refs[0].semantic.normal_hint = Some(axis.map(|v| v * sign));
            refs.push(opencad_geometry::TopoRef::face(
                opencad_core::TopoRefId::new("ref:face:opposite_end").unwrap(),
                "feature:extrude_base",
                if low {
                    "extrusion_end"
                } else {
                    "extrusion_start"
                },
            ));
            let registry = FeatureRegistry::with_defaults();
            let mut cache = RegenerationCache::new();
            for height in [0.02_f64, 0.012] {
                params
                    .set_expr("param:rib_height", format!("{} mm", height * 1000.))
                    .unwrap();
                let report = model
                    .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
                    .unwrap();
                assert!(
                    report
                        .reference_provenance
                        .iter()
                        .all(|r| !r.status.is_unresolved()),
                    "{:?}",
                    report.reference_provenance
                );
                assert!(report.face_history.is_empty());
                let v = 12e-6 + 0.03 * (0.004 * height - 0.05 * height * height);
                let m = 0.03 * (0.004 * height * height / 2. - 2. * 0.05 * height.powi(3) / 3.);
                let added = v - 12e-6;
                let center: [f64; 3] = std::array::from_fn(|i| {
                    (12e-6 * stock_center[i] + added * base_center[i] + axis[i] * sign * m) / v
                });
                let body = model.active_body().unwrap();
                let actual = k.mass_properties(body, 1.).unwrap();
                assert!((actual.volume_m3 - v).abs() < 1e-12);
                for (a, b) in actual.center_of_mass.into_iter().zip(center) {
                    assert!((a - b).abs() < 1e-9);
                }
                let mesh = k.tessellate(body, &Default::default()).unwrap();
                let (mv, mc) = evidence::integrated(&mesh, normals);
                assert!((mv - v).abs() < 1e-10);
                for (a, b) in mc.into_iter().zip(center) {
                    assert!((a - b).abs() < 2e-8);
                }
                let warm = model
                    .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
                    .unwrap();
                assert!(warm.cached_nodes.contains(&"feature:rib".into()));
                assert!(warm
                    .reference_provenance
                    .iter()
                    .all(|r| !r.status.is_unresolved()));
            }
            let old = model.outputs.clone();
            params.set_expr("param:rib_height", "50 mm").unwrap();
            assert!(model
                .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
                .is_err());
            assert_eq!(old, model.outputs);
        }
    }
}
#[test]
fn mock_tapered_source_edits_and_cache_match_independent_volume_com_mesh() {
    physical(&MockGeometryKernel::with_analytic_primitives(), true);
}
#[test]
fn native_tapered_source_edits_and_cache_match_independent_volume_com_mesh() {
    physical(&OcctGeometryKernel::new(), false);
}
fn failures<K: GeometryKernel>(k: &K) {
    let registry = FeatureRegistry::with_defaults();
    for bad in [
        "missing",
        "duplicate",
        "foreign",
        "end_role",
        "normal",
        "binding",
        "centroid",
        "plane",
        "line_missing",
        "line_oblique",
        "target",
        "height",
        "height_unit",
        "angle_unit",
        "length_kind",
        "angle_kind",
        "consumed",
        "construction_unconsumed",
    ] {
        let (mut model, mut params, mut refs) = fixture::fixture();
        let mut cache = RegenerationCache::new();
        model
            .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
            .unwrap();
        let old = model.outputs.clone();
        match bad {
            "missing" => refs.clear(),
            "duplicate" => refs.push(refs[0].clone()),
            "foreign" => refs[0].semantic.created_by = "feature:sketch_base".into(),
            "end_role" => refs[0].semantic.role = Some("extrusion_start".into()),
            "normal" => refs[0].semantic.normal_hint = Some([0., 0., -1.]),
            "centroid" => {
                refs[0].geometric_fingerprint = Some(opencad_geometry::GeometricFingerprint {
                    surface_type: "plane".into(),
                    kernel_face_id: None,
                    kernel_edge_id: None,
                    area_range: None,
                    bbox_hint: Some([[0.2, 0.2, 0.01], [0.2, 0.2, 0.01]]),
                    adjacent_feature_ids: Vec::new(),
                });
            }
            "binding" => {
                let stock = model.outputs["feature:extrude_base"].body.as_ref().unwrap();
                let opposite = k
                    .rectangular_stock_geometry(
                        stock,
                        opencad_geometry::FacePick {
                            point_m: [0.02, 0.015, 0.],
                            normal: [0., 0., -1.],
                        },
                    )
                    .unwrap();
                refs[0].geometric_fingerprint = Some(opencad_geometry::GeometricFingerprint {
                    surface_type: "plane".into(),
                    kernel_face_id: Some(opposite.entry_face_id),
                    kernel_edge_id: None,
                    area_range: None,
                    bbox_hint: None,
                    adjacent_feature_ids: Vec::new(),
                });
            }
            "plane" => {
                model.sketches.get_mut("sketch:rib").unwrap().workplane =
                    opencad_sketch::Workplane::Custom {
                        origin: [0., 0., 0.011],
                        normal: [0., 0., 1.],
                        x_axis: [1., 0., 0.],
                    }
            }
            "line_oblique" => {
                let s = model.sketches.get_mut("sketch:rib").unwrap();
                if let Some(opencad_sketch::SketchEntity::Point(p)) = s
                    .entities
                    .iter_mut()
                    .find(|e| e.id().as_str() == "ent:rib_end")
                {
                    p.y = opencad_sketch::Coord::literal(0.016);
                }
            }
            "height" => params.set_expr("param:rib_height", "100 mm").unwrap(),
            "height_unit" => params.set_expr("param:rib_height", "20 deg").unwrap(),
            "angle_unit" => params.set_expr("param:rib_angle", "3 mm").unwrap(),
            "consumed" => {
                model
                    .add_node(FeatureNode::new(
                        "feature:opening",
                        "Consuming shell",
                        FeatureDefinition::Shell(ShellFeature {
                            target_feature: "feature:extrude_base".into(),
                            thickness: opencad_core::Length::from_meters(0.001),
                            thickness_expr: None,
                            open_face_refs: vec!["ref:face:rib_base".into()],
                        }),
                    ))
                    .unwrap();
            }
            _ => {
                let FeatureDefinition::TaperedRib(d) =
                    &mut model.nodes.get_mut("feature:rib").unwrap().definition
                else {
                    panic!("rib")
                };
                match bad {
                    "target" => d.target_feature = "feature:sketch_base".into(),
                    "line_missing" => d.centerline_ref = "ent:missing".into(),
                    "length_kind" => d.height_expr = Some("rib_angle_rad".into()),
                    "angle_kind" => d.angle_expr = Some("rib_height".into()),
                    _ => {
                        model.nodes.shift_remove("feature:rib");
                    }
                }
            }
        }
        assert!(
            model
                .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
                .is_err(),
            "{bad}"
        );
        assert_eq!(old, model.outputs, "{bad}");
    }
}
#[test]
fn mock_invalid_tapered_source_and_refs_preserve_outputs() {
    failures(&MockGeometryKernel::with_analytic_primitives());
}
#[test]
fn native_invalid_tapered_source_and_refs_preserve_outputs() {
    failures(&OcctGeometryKernel::new());
}
#[test]
fn closed_wire_and_genuine_legacy_rib_reader_refusal() {
    let (model, _, _) = fixture::fixture();
    let value = serde_json::to_value(&model.nodes["feature:rib"].definition).unwrap();
    let same: FeatureDefinition = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(same, model.nodes["feature:rib"].definition);
    for bad in [
        "unknown",
        "length_unit",
        "angle_unit",
        "no_end",
        "end_kind",
        "no_face",
    ] {
        let mut v = value.clone();
        match bad {
            "unknown" => v["curved"] = true.into(),
            "length_unit" => v["height"]["unit"] = "mm".into(),
            "angle_unit" => v["angle"]["unit"] = "deg".into(),
            "end_kind" => v["ends"] = "round".into(),
            "no_end" => {
                v.as_object_mut().unwrap().remove("ends");
            }
            _ => {
                v.as_object_mut().unwrap().remove("face_ref");
            }
        }
        assert!(serde_json::from_value::<FeatureDefinition>(v).is_err());
    }
    #[derive(serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum OldFeature {
        Rib(RibFeature),
    }
    let old = serde_json::json!({"type":"rib","sketch_feature":"feature:sketch_rib","centerline_ref":"ent:rib_line","target_feature":"feature:extrude_base","thickness":{"value_si":0.004},"extent":{"type":"distance","length":{"value_si":0.02}}});
    let OldFeature::Rib(old) = serde_json::from_value(old).unwrap();
    assert_eq!(old.centerline_ref, "ent:rib_line");
    assert!(serde_json::from_value::<OldFeature>(value).is_err());
}
struct Context<'a> {
    kernel: &'a dyn GeometryKernel,
    model: &'a PartModel,
    refs: &'a [opencad_geometry::TopoRef],
}
impl RegenContext for Context<'_> {
    fn kernel(&self) -> &dyn GeometryKernel {
        self.kernel
    }
    fn source_feature_nodes(&self) -> opencad_core::Result<Vec<&FeatureNode>> {
        Ok(self.model.nodes.values().collect())
    }
    fn sketch_for_feature(&self, id: &str) -> opencad_core::Result<&opencad_sketch::Sketch> {
        let FeatureDefinition::Sketch(s) = &self.model.nodes[id].definition else {
            return Err(opencad_core::OpenCadError::validation(
                "not authored sketch",
            ));
        };
        Ok(&self.model.sketches[&s.sketch_id])
    }
    fn body_for_feature(&self, id: &str) -> opencad_core::Result<opencad_geometry::KernelBody> {
        self.model.outputs[id]
            .body
            .clone()
            .ok_or_else(|| opencad_core::OpenCadError::validation("no body"))
    }
    fn semantic_refs(&self) -> &[opencad_geometry::TopoRef] {
        self.refs
    }
}
fn cached_ownership<K: GeometryKernel>(k: &K) {
    let (mut model, params, refs) = fixture::fixture();
    let registry = FeatureRegistry::with_defaults();
    let mut cache = RegenerationCache::new();
    let report = model
        .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
        .unwrap();
    let ctx = Context {
        kernel: k,
        model: &model,
        refs: &refs,
    };
    let faces = TaperedRibFeatureExecutor
        .discover_output_faces(
            &model.nodes["feature:rib"],
            &ctx,
            &model.outputs["feature:rib"],
        )
        .unwrap()
        .unwrap();
    assert_eq!(faces.len(), 11);
    assert_eq!(
        faces
            .iter()
            .filter(|f| f.feature_id.as_deref() == Some("feature:extrude_base"))
            .count(),
        6
    );
    assert_eq!(
        faces
            .iter()
            .filter(|f| f.feature_id.as_deref() == Some("feature:rib"))
            .count(),
        5
    );
    assert!(faces
        .iter()
        .any(|f| f.role == "extrusion_end"
            && f.feature_id.as_deref() == Some("feature:extrude_base")));
    let key = &report.trace.output_hashes_sha256["feature:rib"];
    let mut wrong = cache.get(key).unwrap().clone();
    wrong.face_discoveries.clear();
    cache.insert(key.clone(), wrong);
    let warm = model
        .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
        .unwrap();
    assert!(warm.cached_nodes.contains(&"feature:rib".into()));
    assert!(warm
        .reference_provenance
        .iter()
        .all(|p| !p.status.is_unresolved()));
    let old = model.outputs.clone();
    let mut fake = cache.get(key).unwrap().clone();
    fake.output.body = model.outputs["feature:extrude_base"].body.clone();
    cache.insert(key.clone(), fake);
    assert!(model
        .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
        .is_err());
    assert_eq!(old, model.outputs);
}
#[test]
fn mock_cached_tapered_output_rechecks_physical_owners_and_body() {
    cached_ownership(&MockGeometryKernel::with_analytic_primitives());
}
#[test]
fn native_cached_tapered_output_rechecks_physical_owners_and_body() {
    cached_ownership(&OcctGeometryKernel::new());
}
fn signed_source<K: GeometryKernel>(k: &K) {
    let (mut model, mut params, refs) = fixture::fixture();
    params.set_expr("param:rib_angle", "-2 deg").unwrap();
    let report = model
        .regenerate(
            k,
            &FeatureRegistry::with_defaults(),
            Some(&params),
            Some(&refs),
        )
        .unwrap();
    assert!(report
        .reference_provenance
        .iter()
        .all(|p| !p.status.is_unresolved()));
    let expected = 12e-6 + 0.03 * (0.004 * 0.02 - (-2_f64).to_radians().tan() * 0.02 * 0.02);
    let mass = k.mass_properties(model.active_body().unwrap(), 1.).unwrap();
    assert!((mass.volume_m3 - expected).abs() < 1e-12);
}
#[test]
fn signed_tapered_source_widens_in_both_real_kernels() {
    signed_source(&MockGeometryKernel::with_analytic_primitives());
    signed_source(&OcctGeometryKernel::new());
}

#[test]
fn suppressing_tapered_rib_preserves_its_declared_construction_source() {
    fn check<K: GeometryKernel>(k: &K) {
        let (mut model, params, refs) = fixture::fixture();
        model.nodes.get_mut("feature:rib").unwrap().suppressed = true;
        let report = model
            .regenerate(
                k,
                &FeatureRegistry::with_defaults(),
                Some(&params),
                Some(&refs),
            )
            .unwrap();
        assert!(report.skipped_suppressed.contains(&"feature:rib".into()));
        assert!(
            (k.mass_properties(model.active_body().unwrap(), 1.)
                .unwrap()
                .volume_m3
                - 12e-6)
                .abs()
                < 1e-12
        );
    }
    check(&MockGeometryKernel::with_analytic_primitives());
    check(&OcctGeometryKernel::new());
}
