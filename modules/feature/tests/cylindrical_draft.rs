//! Source-owned ADR-065 cases with independent slice and closed-mesh integrals.
use opencad_feature::*;
use opencad_geometry::{GeometryKernel, MockGeometryKernel, ReferenceStatus};
use opencad_kernel_occt::OcctGeometryKernel;
use opencad_sketch::Workplane;
#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod evidence;
#[path = "support/cylindrical_draft_fixture.rs"]
mod fixture;
fn physical<K: GeometryKernel>(kernel: &K, normals: bool) {
    for (tilt, sign) in [(0_f64, 1_f64), (25_f64, -1_f64)] {
        let (mut model, mut params, refs) = fixture::fixture();
        let radians = tilt.to_radians();
        let axis = [0., -radians.sin(), radians.cos()];
        let y_axis = [0., radians.cos(), radians.sin()];
        let translation = if tilt == 0. {
            [0.; 3]
        } else {
            [0.1, -0.02, 0.03]
        };
        model.sketches.get_mut("sketch:stock").unwrap().workplane = Workplane::Custom {
            origin: translation,
            normal: axis,
            x_axis: [1., 0., 0.],
        };
        let center: [f64; 3] =
            std::array::from_fn(|i| translation[i] + [0.012, 0., 0.][i] + y_axis[i] * 0.009);
        let registry = FeatureRegistry::with_defaults();
        let mut cache = RegenerationCache::new();
        for z in [0., 0.013, 0.03] {
            for degrees in [-7_f64, 5.] {
                let FeatureDefinition::CylindricalDraft(def) =
                    &mut model.nodes.get_mut("feature:draft").unwrap().definition
                else {
                    panic!("draft")
                };
                def.neutral_plane = Workplane::Custom {
                    origin: std::array::from_fn(|i| translation[i] + axis[i] * z),
                    normal: axis.map(|v| v * sign),
                    x_axis: [1., 0., 0.],
                };
                params
                    .set_expr("param:draft_angle", format!("{degrees} deg"))
                    .unwrap();
                let report = model
                    .regenerate_with_cache(
                        kernel,
                        &registry,
                        Some(&params),
                        Some(&refs),
                        &mut cache,
                    )
                    .unwrap();
                assert_eq!(
                    report.reference_provenance[0].status,
                    ReferenceStatus::Consumed
                );
                assert!(report.face_history.is_empty());
                let k = sign * degrees.to_radians().tan();
                let r = 0.02;
                let h = 0.03_f64;
                let pi = std::f64::consts::PI;
                let v = pi
                    * (r * r * h
                        + 2. * r * k * (z * h - h * h / 2.)
                        + k * k * (z * z * h - z * h * h + h.powi(3) / 3.));
                let m = pi
                    * (r * r * h * h / 2.
                        + 2. * r * k * (z * h * h / 2. - h.powi(3) / 3.)
                        + k * k * (z * z * h * h / 2. - 2. * z * h.powi(3) / 3. + h.powi(4) / 4.));
                let body = model.active_body().unwrap();
                let mass = kernel.mass_properties(body, 1.).unwrap();
                assert!((mass.volume_m3 - v).abs() < 1e-12);
                for i in 0..3 {
                    assert!((mass.center_of_mass[i] - (center[i] + axis[i] * m / v)).abs() < 1e-9);
                }
                let mesh = kernel
                    .tessellate(
                        body,
                        &opencad_geometry::TessellationSettings {
                            linear_deflection: 1e-6,
                            angular_deflection_deg: 1.,
                        },
                    )
                    .unwrap();
                let (mv, mc) = evidence::integrated(&mesh, normals);
                assert!((mv - v).abs() < v * 2e-4);
                for i in 0..3 {
                    assert!((mc[i] - (center[i] + axis[i] * m / v)).abs() < 3e-6);
                }
                for p in &mesh.positions {
                    let q = evidence::sub(p.map(f64::from), center);
                    let pz = evidence::dot(q, axis);
                    let radial = evidence::dot(q, [1., 0., 0.]).hypot(evidence::dot(q, y_axis));
                    if radial > 1e-7 {
                        assert!((radial - (r - k * (pz - z))).abs() < 5e-8, "{p:?}");
                    }
                }
                let stock = model.outputs["feature:stock"].body.as_ref().unwrap();
                assert!(
                    (kernel.mass_properties(stock, 1.).unwrap().volume_m3 - pi * r * r * h).abs()
                        < 1e-12
                );
                let warm = model
                    .regenerate_with_cache(
                        kernel,
                        &registry,
                        Some(&params),
                        Some(&refs),
                        &mut cache,
                    )
                    .unwrap();
                assert!(warm.cached_nodes.contains(&"feature:draft".into()));
                assert_eq!(
                    warm.reference_provenance[0].status,
                    ReferenceStatus::Consumed
                );
            }
        }
        let old = model.outputs.clone();
        params.set_expr("param:draft_angle", "46 deg").unwrap();
        assert!(model
            .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
            .is_err());
        assert_eq!(model.outputs, old);
    }
}
#[test]
fn mock_source_draft_matches_integrals_contact_consumption_and_cache() {
    physical(&MockGeometryKernel::with_analytic_primitives(), true);
}
#[test]
fn native_source_draft_matches_integrals_contact_consumption_and_cache() {
    physical(&OcctGeometryKernel::new(), false);
}
fn failures<K: GeometryKernel>(kernel: &K) {
    let registry = FeatureRegistry::with_defaults();
    for bad in [
        "missing",
        "duplicate",
        "foreign",
        "role",
        "normal",
        "fingerprint",
        "centroid",
        "cap",
        "oblique",
        "outside",
        "target",
        "consumed",
        "unit",
        "kind",
        "nonunit",
    ] {
        let (mut model, mut params, mut refs) = fixture::fixture();
        let mut cache = RegenerationCache::new();
        model
            .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
            .unwrap();
        let old = model.outputs.clone();
        match bad {
            "missing" => refs.clear(),
            "duplicate" => refs.push(refs[0].clone()),
            "foreign" => refs[0].semantic.created_by = "feature:sketch_stock".into(),
            "role" => refs[0].semantic.role = Some("top".into()),
            "normal" => refs[0].semantic.normal_hint = Some([0., 0., 1.]),
            "fingerprint" => {
                refs[0].geometric_fingerprint.as_mut().unwrap().surface_type = "plane".into()
            }
            "centroid" => {
                refs[0].geometric_fingerprint.as_mut().unwrap().bbox_hint =
                    Some([[10., 10., 10.], [10., 10., 10.]])
            }
            "cap" => {
                let stock = model.outputs["feature:stock"].body.as_ref().unwrap();
                let w = kernel
                    .cylindrical_stock_geometry(stock, [0., 0., 1.])
                    .unwrap();
                refs[0]
                    .geometric_fingerprint
                    .as_mut()
                    .unwrap()
                    .kernel_face_id = Some(w.end_face_id);
            }
            "unit" => params.set_expr("param:draft_angle", "5 mm").unwrap(),
            "kind" => {
                params
                    .add_parameter(opencad_graph::ParameterEntry::new(
                        "param:distance",
                        "distance",
                        "5 mm",
                    ))
                    .unwrap();
                let FeatureDefinition::CylindricalDraft(d) =
                    &mut model.nodes.get_mut("feature:draft").unwrap().definition
                else {
                    panic!("draft")
                };
                d.angle_expr = Some("distance".into());
            }
            "consumed" => {
                let mut other = model.nodes["feature:draft"].clone();
                other.id = "feature:other".into();
                model.add_node(other).unwrap();
            }
            _ => {
                let FeatureDefinition::CylindricalDraft(d) =
                    &mut model.nodes.get_mut("feature:draft").unwrap().definition
                else {
                    panic!("draft")
                };
                match bad {
                    "target" => d.target_feature = "feature:sketch_stock".into(),
                    "nonunit" => {
                        d.neutral_plane = Workplane::Custom {
                            origin: [0., 0., 0.015],
                            normal: [0., 0., 2.],
                            x_axis: [1., 0., 0.],
                        };
                    }
                    "oblique" => {
                        d.neutral_plane = Workplane::Custom {
                            origin: [0., 0., 0.015],
                            normal: [0.1, 0., 0.99498743710662],
                            x_axis: [0.99498743710662, 0., -0.1],
                        }
                    }
                    _ => {
                        d.neutral_plane = Workplane::Custom {
                            origin: [0., 0., 0.031],
                            normal: [0., 0., 1.],
                            x_axis: [1., 0., 0.],
                        }
                    }
                }
            }
        }
        assert!(
            model
                .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
                .is_err(),
            "{bad}"
        );
        assert_eq!(model.outputs, old, "{bad}");
    }
}
#[test]
fn mock_refusals_recheck_warm_cache_and_preserve_outputs() {
    failures(&MockGeometryKernel::with_analytic_primitives());
}
#[test]
fn native_refusals_recheck_warm_cache_and_preserve_outputs() {
    failures(&OcctGeometryKernel::new());
}
#[test]
fn strict_wire_and_genuine_planar_draft_reader_refusal() {
    let (model, _, _) = fixture::fixture();
    let def = &model.nodes["feature:draft"].definition;
    let value = serde_json::to_value(def).unwrap();
    assert_eq!(
        &serde_json::from_value::<FeatureDefinition>(value.clone()).unwrap(),
        def
    );
    for bad in ["extra", "angle_unit", "no_side", "no_plane"] {
        let mut v = value.clone();
        match bad {
            "extra" => v["curved"] = true.into(),
            "angle_unit" => v["angle"]["unit"] = "deg".into(),
            "no_side" => {
                v.as_object_mut().unwrap().remove("face_ref");
            }
            _ => {
                v.as_object_mut().unwrap().remove("neutral_plane");
            }
        }
        assert!(serde_json::from_value::<FeatureDefinition>(v).is_err());
    }
    #[derive(serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum OldFeature {
        Draft(DraftFeature),
        Extrude(ExtrudeFeature),
    }
    let legacy = serde_json::json!({"type":"draft","target_feature":"feature:stock","angle":{"value_si":0.1},"neutral_plane":{"type":"global","plane":"XY"},"face_refs":["ref:face:side"]});
    let OldFeature::Draft(old) = serde_json::from_value(legacy).unwrap() else {
        panic!("legacy")
    };
    assert_eq!(old.target_feature, "feature:stock");
    let OldFeature::Extrude(old) = serde_json::from_value(
        serde_json::to_value(&model.nodes["feature:stock"].definition).unwrap(),
    )
    .unwrap() else {
        panic!("stock")
    };
    assert_eq!(old.sketch_feature, "feature:sketch_stock");
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
        let FeatureDefinition::Sketch(def) = &self.model.nodes[id].definition else {
            return Err(opencad_core::OpenCadError::validation(
                "not an authored sketch",
            ));
        };
        Ok(&self.model.sketches[&def.sketch_id])
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
fn ownership_and_cache<K: GeometryKernel>(kernel: &K) {
    for z in [0., 0.015, 0.03] {
        let (mut model, params, mut refs) = fixture::fixture();
        let FeatureDefinition::CylindricalDraft(def) =
            &mut model.nodes.get_mut("feature:draft").unwrap().definition
        else {
            panic!("draft")
        };
        def.neutral_plane = Workplane::Custom {
            origin: [0., 0., z],
            normal: [0., 0., 1.],
            x_axis: [1., 0., 0.],
        };
        if z != 0.015 {
            refs.push(opencad_geometry::TopoRef::face(
                opencad_core::TopoRefId::new("ref:face:neutral_cap").unwrap(),
                "feature:stock",
                if z == 0. { "bottom" } else { "top" },
            ));
        }
        let mut cache = RegenerationCache::new();
        let registry = FeatureRegistry::with_defaults();
        let report = model
            .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
            .unwrap();
        let ctx = Context {
            kernel,
            model: &model,
            refs: &refs,
        };
        let faces = CylindricalDraftFeatureExecutor
            .discover_output_faces(
                &model.nodes["feature:draft"],
                &ctx,
                &model.outputs["feature:draft"],
            )
            .unwrap()
            .unwrap();
        assert_eq!(faces.len(), 3);
        assert_eq!(
            faces
                .iter()
                .filter(|f| f.feature_id.as_deref() == Some("feature:stock"))
                .count(),
            usize::from(z != 0.015)
        );
        assert!(faces
            .iter()
            .filter(|f| f.role == "conical")
            .all(|f| f.feature_id.as_deref() == Some("feature:draft")));
        let key = &report.trace.output_hashes_sha256["feature:draft"];
        let mut poisoned = cache.get(key).unwrap().clone();
        poisoned.face_discoveries.clear();
        cache.insert(key.clone(), poisoned);
        let warm = model
            .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
            .unwrap();
        assert!(warm.cached_nodes.contains(&"feature:draft".into()));
        if z != 0.015 {
            let cap = warm
                .reference_provenance
                .iter()
                .find(|r| r.ref_id == "ref:face:neutral_cap")
                .unwrap();
            assert!(!cap.status.is_unresolved(), "{cap:?}");
        }
        let old = model.outputs.clone();
        let mut bad = cache.get(key).unwrap().clone();
        bad.output.body = model.outputs["feature:stock"].body.clone();
        cache.insert(key.clone(), bad);
        assert!(model
            .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
            .is_err());
        assert_eq!(old, model.outputs);
    }
}
#[test]
fn mock_actual_cap_ownership_and_cached_evidence_are_revalidated() {
    ownership_and_cache(&MockGeometryKernel::with_analytic_primitives());
}
#[test]
fn native_actual_cap_ownership_and_cached_evidence_are_revalidated() {
    ownership_and_cache(&OcctGeometryKernel::new());
}
