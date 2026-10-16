//! Source/phase, physical numerical output, ownership and cached failure admission.
use opencad_core::Length;
use opencad_feature::*;
use opencad_geometry::{
    GeometryKernel, MockGeometryKernel, ReferenceStatus, RigidTransform, TessellationSettings,
};
use opencad_kernel_occt::OcctGeometryKernel;
#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod evidence;
#[path = "support/faceted_fillet_fixture.rs"]
mod fixture;
fn physical<K: GeometryKernel>(k: &K) {
    for (constant, moved) in [(true, false), (false, false), (false, true)] {
        let (mut m, params, mut refs) = fixture::fixture();
        if constant {
            if let FeatureDefinition::FacetedCircularFillet(f) =
                &mut m.nodes.get_mut("feature:fillet").unwrap().definition
            {
                f.maximum_radius = f.minimum_radius;
            }
        }
        let p = if moved {
            RigidTransform {
                rotation: [[0., 0.6, -0.8], [1., 0., 0.], [0., -0.8, -0.6]],
                translation_m: [0.05, -0.03, 0.08],
            }
        } else {
            RigidTransform::identity()
        };
        m.sketches.get_mut("sketch:stock").unwrap().workplane = opencad_sketch::Workplane::Custom {
            origin: p.translation_m,
            normal: p.rotation.map(|r| r[2]),
            x_axis: p.rotation.map(|r| r[0]),
        };
        refs[0].semantic.normal_hint = Some(p.rotation.map(|r| r[2]));
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
        let body = m.active_body().unwrap().clone();
        let mass = k.mass_properties(&body, 1.).unwrap();
        // Independent analytic constant-limit volume; approximation has explicit SI error.
        if constant {
            let (r, h, f) = (0.01_f64, 0.02_f64, 0.0005_f64);
            let exact = std::f64::consts::PI * r * r * h
                - 2. * std::f64::consts::PI
                    * f
                    * f
                    * ((r - f) * (1. - std::f64::consts::PI / 4.) + f / 6.);
            assert!((mass.volume_m3 - exact).abs() < 1e-7);
        }
        assert!(mass.volume_m3 > 6.1e-6 && mass.volume_m3 < 6.29e-6);
        let stock = m.outputs["feature:stock"].body.as_ref().unwrap();
        assert!(
            (k.mass_properties(stock, 1.).unwrap().volume_m3
                - std::f64::consts::PI * 0.01_f64.powi(2) * 0.02)
                .abs()
                < 1e-15
        );
        let mesh = k
            .tessellate(&body, &TessellationSettings::default())
            .unwrap();
        let (mesh_volume, mesh_centroid) = evidence::integrated(&mesh, true);
        assert!((mesh_volume - mass.volume_m3).abs() < mass.volume_m3 * 2e-6);
        for (measured, expected) in mesh_centroid.into_iter().zip(mass.center_of_mass) {
            assert!((measured - expected).abs() < 2e-8);
        }
        // Independent maximum/minimum law contacts, not values from preparation.
        let hi = if constant { 0.0005 } else { 0.0015 };
        for contact in [
            [0.01 - hi, 0., 0.02],
            [-0.0095, 0., 0.02],
            [0.01, 0., 0.02 - hi],
            [-0.01, 0., 0.0195],
        ] {
            let q = p.transform_point(contact);
            let best = mesh
                .positions
                .iter()
                .map(|v| {
                    v.iter()
                        .zip(q)
                        .map(|(a, b)| (f64::from(*a) - b).powi(2))
                        .sum::<f64>()
                        .sqrt()
                })
                .fold(f64::INFINITY, f64::min);
            assert!(best < 2e-8, "contact discrepancy {best}");
        }
        let key = report.trace.output_hashes_sha256["feature:fillet"].clone();
        let mut entry = cache.get(&key).unwrap().clone();
        assert_eq!(entry.face_discoveries.len(), mesh.triangle_count());
        assert!(entry
            .face_discoveries
            .iter()
            .all(|f| f.feature_id.as_deref() == Some("feature:fillet")
                && f.role == "faceted_surface"));
        assert!(entry.edge_discoveries.is_empty());
        let face = opencad_geometry::TopoRef::face(
            opencad_core::TopoRefId::new("ref:face:ambiguous").unwrap(),
            "feature:fillet",
            "faceted_surface",
        );
        assert!(
            opencad_geometry::resolve_kernel_face_id_for_topo_ref_with_discoveries(
                &[face],
                &[],
                "ref:face:ambiguous",
                Some(&entry.face_discoveries)
            )
            .is_err()
        );
        entry.face_discoveries.clear();
        cache.insert(key.clone(), entry.clone());
        let warm = m
            .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
            .unwrap();
        assert!(warm.cached_nodes.contains(&"feature:fillet".into()));
        if !constant && !moved {
            // A proper rotation about the actual centroid preserves volume, area and COM;
            // the requested authored phase still must reject the actual cached shell.
            let wrong = k
                .transform_body(
                    body.clone(),
                    RigidTransform {
                        rotation: [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
                        translation_m: [
                            mass.center_of_mass[0] + mass.center_of_mass[1],
                            mass.center_of_mass[1] - mass.center_of_mass[0],
                            0.,
                        ],
                    },
                )
                .unwrap();
            assert!(
                (k.mass_properties(&wrong, 1.).unwrap().volume_m3 - mass.volume_m3).abs() < 1e-15
            );
            entry.output.body = Some(wrong);
            cache.insert(key, entry);
            let before = m.outputs.clone();
            let error = m
                .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
                .unwrap_err();
            assert!(error.to_string().contains("phase/grid"), "{error}");
            assert_eq!(m.outputs, before);
        }
    }
}
#[test]
fn mock_source_contacts_placement_cache_and_wrong_phase() {
    physical(&MockGeometryKernel::with_analytic_primitives());
}
#[test]
fn native_source_contacts_placement_cache_and_wrong_phase() {
    physical(&OcctGeometryKernel::new());
}
fn refusals<K: GeometryKernel>(k: &K) {
    let (mut base, params, refs) = fixture::fixture();
    let registry = FeatureRegistry::with_defaults();
    let mut baseline_cache = RegenerationCache::new();
    base.regenerate_with_cache(
        k,
        &registry,
        Some(&params),
        Some(&refs),
        &mut baseline_cache,
    )
    .unwrap();
    for case in 0..16 {
        let mut m = base.clone();
        let mut r = refs.clone();
        let mut cache = baseline_cache.clone();
        match case {
            0 => r.clear(),
            1 => r[0].semantic.created_by = "feature:sketch_stock".into(),
            2 => r[0].semantic.role = Some("extrusion_start".into()),
            3 => {
                let mut alias = r[0].clone();
                alias.ref_id = opencad_core::TopoRefId::new("ref:edge:alias").unwrap();
                r.push(alias);
            }
            4 => r[0].semantic.normal_hint = Some([1., 0., 0.]),
            5 => m.nodes.get_mut("feature:stock").unwrap().suppressed = true,
            6 => {
                if let FeatureDefinition::Extrude(e) =
                    &mut m.nodes.get_mut("feature:stock").unwrap().definition
                {
                    e.extent = opencad_geometry::ExtrudeExtent::Distance {
                        length: Length::from_meters(-1.),
                    };
                    e.length_expr = Some("20 mm".into());
                }
            }
            7 => {
                if let FeatureDefinition::FacetedCircularFillet(f) =
                    &mut m.nodes.get_mut("feature:fillet").unwrap().definition
                {
                    f.quality.max_integration_cells = 1;
                }
            }
            8 => {
                if let FeatureDefinition::FacetedCircularFillet(f) =
                    &mut m.nodes.get_mut("feature:fillet").unwrap().definition
                {
                    f.quality.maximum_volume_difference_m3 = 1e-20;
                }
            }
            9 => {
                if let FeatureDefinition::FacetedCircularFillet(f) =
                    &mut m.nodes.get_mut("feature:fillet").unwrap().definition
                {
                    f.quality.max_triangles = 4;
                }
            }
            10 => {
                m.sketches.get_mut("sketch:stock").unwrap().workplane =
                    opencad_sketch::Workplane::Custom {
                        origin: [0.; 3],
                        normal: [0., 0., 1.],
                        x_axis: [1. + 1e-10, 0., 0.],
                    }
            }
            11 => {
                if let FeatureDefinition::FacetedCircularFillet(f) =
                    &mut m.nodes.get_mut("feature:fillet").unwrap().definition
                {
                    f.maximum_radius = Length::from_meters(0.03);
                }
            }
            12 => {
                m.add_node(FeatureNode::new(
                    "feature:competing",
                    "Other consumer",
                    FeatureDefinition::Fillet(FilletFeature::top_perimeter(
                        "feature:stock",
                        Length::from_meters(0.0001),
                        None,
                    )),
                ))
                .unwrap();
            }
            13 => {
                let stock = base.outputs["feature:stock"].body.as_ref().unwrap();
                let rims = k.cylindrical_stock_rims(stock, [0., 0., 1.]).unwrap();
                r[0].geometric_fingerprint = Some(opencad_geometry::GeometricFingerprint {
                    surface_type: "circle".into(),
                    kernel_face_id: None,
                    kernel_edge_id: Some(rims.start_edge_id),
                    area_range: None,
                    bbox_hint: None,
                    adjacent_feature_ids: Vec::new(),
                });
            }
            14 => r[0].fallback_query = Some("nearest end".into()),
            15 => {
                if let opencad_sketch::SketchEntity::Circle(c) =
                    &mut m.sketches.get_mut("sketch:stock").unwrap().entities[1]
                {
                    c.radius = opencad_sketch::Coord::Expr(
                        opencad_core::Expression::new("1 deg").unwrap(),
                    );
                }
            }
            _ => unreachable!(),
        }
        let before = m.outputs.clone();
        assert!(
            m.regenerate_with_cache(k, &registry, Some(&params), Some(&r), &mut cache)
                .is_err(),
            "case {case}"
        );
        assert_eq!(m.outputs, before, "case {case}");
    }
}
#[test]
fn mock_source_quality_refusals_preserve_outputs() {
    refusals(&MockGeometryKernel::with_analytic_primitives());
}
#[test]
fn native_source_quality_refusals_preserve_outputs() {
    refusals(&OcctGeometryKernel::new());
}
