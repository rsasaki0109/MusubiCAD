//! ADR-063 independent source/physical counterbore acceptance.
use opencad_core::Length;
use opencad_feature::*;
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
use std::f64::consts::PI;
#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod evidence;
#[path = "support/seated_fixture.rs"]
mod fixture;
fn physical<K: GeometryKernel>(kernel: &K, check_normals: bool) {
    let (mut model, mut params, refs) = fixture::fixture();
    let registry = FeatureRegistry::with_defaults();
    let mut cache = RegenerationCache::new();
    for (depth, diameter) in [(0.004, 0.011), (0.006, 0.011), (0.006, 0.012)] {
        params
            .set_expr("param:seat_depth", format!("{} mm", depth * 1000.))
            .unwrap();
        params
            .set_expr("param:seat_diameter", format!("{} mm", diameter * 1000.))
            .unwrap();
        let report = model
            .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
            .unwrap();
        assert!(
            report
                .reference_provenance
                .iter()
                .all(|p| !p.status.is_unresolved()),
            "{:?}",
            report.reference_provenance
        );
        let body = model.active_body().unwrap();
        let mass = kernel.mass_properties(body, 1.).unwrap();
        let bore = PI * 0.0033_f64.powi(2) * 0.01;
        let extra = PI * ((diameter / 2_f64).powi(2) - 0.0033_f64.powi(2)) * depth;
        let expected = 12e-6 - bore - extra;
        assert!((mass.volume_m3 - expected).abs() < 1e-12, "{mass:?}");
        let center = [
            (12e-6 * 0.020 - (bore + extra) * 0.012) / expected,
            (12e-6 * 0.015 - (bore + extra) * 0.009) / expected,
            (12e-6 * 0.005 - bore * 0.005 - extra * (0.01 - depth / 2.)) / expected,
        ];
        for (a, b) in mass.center_of_mass.into_iter().zip(center) {
            assert!((a - b).abs() < 1e-10);
        }
        let mesh = kernel
            .tessellate(body, &opencad_geometry::TessellationSettings::default())
            .unwrap();
        let (v, c) = evidence::integrated(&mesh, check_normals);
        assert!((v - expected).abs() < 3e-9);
        for (a, b) in c.into_iter().zip(center) {
            assert!((a - b).abs() < 3e-6);
        }
        let warm = model
            .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
            .unwrap();
        assert!(warm.cached_nodes.contains(&"feature:seat".into()));
    }
    let outputs = model.outputs.clone();
    params.set_expr("param:seat_depth", "12 mm").unwrap();
    assert!(model
        .regenerate_with_cache(kernel, &registry, Some(&params), Some(&refs), &mut cache)
        .is_err());
    assert_eq!(outputs, model.outputs);
}
#[test]
fn mock_and_native_counterbores_match_analytic_volume_first_moments_mesh_and_parameter_edits() {
    physical(&MockGeometryKernel::with_analytic_primitives(), true);
    physical(&OcctGeometryKernel::new(), false);
}
#[test]
fn strict_new_tag_and_closed_seat_refuse_unknown_and_legacy_decoders() {
    let (model, _, _) = fixture::fixture();
    let source = serde_json::to_value(&model.nodes["feature:seat"].definition).unwrap();
    let round: FeatureDefinition = serde_json::from_value(source.clone()).unwrap();
    assert_eq!(round, model.nodes["feature:seat"].definition);
    for path in [
        "seat_kind",
        "extra",
        "missing_face",
        "extra_seat",
        "length_unit",
    ] {
        let mut v = source.clone();
        match path {
            "seat_kind" => v["seat"]["type"] = "countersink".into(),
            "extra" => v["ignored"] = true.into(),
            "length_unit" => v["bore_depth"]["unit"] = "mm".into(),
            "extra_seat" => v["seat"]["ignored"] = true.into(),
            _ => {
                v.as_object_mut().unwrap().remove("face_ref");
            }
        }
        assert!(serde_json::from_value::<FeatureDefinition>(v).is_err());
    }
    #[derive(serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum OldFeature {
        Hole {
            #[serde(flatten)]
            _fields: serde_json::Value,
        },
        StandardHole {
            #[serde(flatten)]
            _fields: serde_json::Value,
        },
    }
    assert!(serde_json::from_value::<OldFeature>(source).is_err());
}
fn invalid_entries<K: GeometryKernel>(kernel: &K) {
    let registry = FeatureRegistry::with_defaults();
    for bad in [
        "missing",
        "foreign",
        "normal",
        "opposite",
        "side",
        "diameter",
        "seat_depth",
        "target",
        "profile",
        "plane",
        "footprint",
        "near_oblique",
    ] {
        let (mut model, params, mut refs) = fixture::fixture();
        model
            .regenerate(kernel, &registry, Some(&params), Some(&refs))
            .unwrap();
        let old = model.outputs.clone();
        let FeatureDefinition::SeatedStandardHole(def) =
            &mut model.nodes.get_mut("feature:seat").unwrap().definition
        else {
            panic!("seat")
        };
        match bad {
            "missing" => refs.clear(),
            "foreign" => refs[0].semantic.created_by = "feature:sketch_base".into(),
            "normal" => refs[0].semantic.normal_hint = Some([0., 0., -1.]),
            "diameter" => {
                def.seat = StandardHoleSeat::Counterbore {
                    diameter: Length::from_meters(0.0066),
                    depth: Length::from_meters(0.004),
                    diameter_expr: None,
                    depth_expr: None,
                };
            }
            "seat_depth" => {
                let StandardHoleSeat::Counterbore {
                    depth, depth_expr, ..
                } = &mut def.seat;
                *depth = Length::from_meters(0.01);
                *depth_expr = None;
            }
            "target" => def.target_feature = "feature:sketch_base".into(),
            "plane" => {
                refs[0].geometric_fingerprint.as_mut().unwrap().bbox_hint =
                    Some([[0., 0., 0.006], [0.04, 0.03, 0.006]]);
            }
            "footprint" => {
                let StandardHoleSeat::Counterbore {
                    diameter,
                    diameter_expr,
                    ..
                } = &mut def.seat;
                *diameter = Length::from_meters(0.04);
                *diameter_expr = None;
            }
            "near_oblique" => {
                model.sketches.get_mut("sketch:seat").unwrap().workplane =
                    opencad_sketch::Workplane::Custom {
                        origin: [0., 0., 0.01],
                        normal: [1e-8, 0., 1.],
                        x_axis: [1., 0., -1e-8],
                    };
            }
            "profile" => def.profile_ref = "sketch:seat/profile:missing".into(),
            "opposite" => {
                model.sketches.get_mut("sketch:seat").unwrap().workplane =
                    opencad_sketch::Workplane::Custom {
                        origin: [0., 0., 0.01],
                        normal: [0., 0., -1.],
                        x_axis: [1., 0., 0.],
                    }
            }
            _ => {
                model.sketches.get_mut("sketch:seat").unwrap().workplane =
                    opencad_sketch::Workplane::Custom {
                        origin: [0., 0., 0.01],
                        normal: [1., 0., 0.],
                        x_axis: [0., 1., 0.],
                    }
            }
        }
        assert!(
            model
                .regenerate(kernel, &registry, Some(&params), Some(&refs))
                .is_err(),
            "{bad}"
        );
        assert_eq!(old, model.outputs);
    }
    let (mut model, params, refs) = fixture::fixture();
    assert!(model
        .regenerate(
            &MockGeometryKernel::new(),
            &registry,
            Some(&params),
            Some(&refs)
        )
        .is_err());
}

#[test]
fn invalid_entry_dimensions_and_default_mock_fail_without_replacing_previous_geometry() {
    invalid_entries(&MockGeometryKernel::with_analytic_primitives());
    invalid_entries(&OcctGeometryKernel::new());
}
#[test]
fn warm_cache_rechecks_reference_and_kernel_ownership() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let (mut model, params, mut refs) = fixture::fixture();
    let registry = FeatureRegistry::with_defaults();
    let mut cache = RegenerationCache::new();
    model
        .regenerate_with_cache(&kernel, &registry, Some(&params), Some(&refs), &mut cache)
        .unwrap();
    let old = model.outputs.clone();
    refs[0].semantic.normal_hint = Some([0., 0., -1.]);
    assert!(model
        .regenerate_with_cache(&kernel, &registry, Some(&params), Some(&refs), &mut cache)
        .is_err());
    assert_eq!(old, model.outputs);
    refs[0].semantic.normal_hint = Some([0., 0., 1.]);
    assert!(model
        .regenerate_with_cache(
            &MockGeometryKernel::new(),
            &registry,
            Some(&params),
            Some(&refs),
            &mut cache
        )
        .is_err());
    assert_eq!(old, model.outputs);
}
fn blind<K: GeometryKernel>(k: &K) {
    for bottom in [false, true] {
        let (mut model, mut params, mut refs) = fixture::fixture();
        params.set_expr("param:bore_depth", "8 mm").unwrap();
        if bottom {
            model.sketches.get_mut("sketch:seat").unwrap().workplane =
                opencad_sketch::Workplane::Custom {
                    origin: [0., 0., 0.],
                    normal: [0., 0., -1.],
                    x_axis: [1., 0., 0.],
                };
            for entity in &mut model.sketches.get_mut("sketch:seat").unwrap().entities {
                if let opencad_sketch::SketchEntity::Point(p) = entity {
                    p.y = opencad_sketch::entity::Coord::literal(-0.009);
                }
            }
            refs[0].semantic.role = Some("bottom".into());
            refs[0].semantic.normal_hint = Some([0., 0., -1.]);
            refs[0].geometric_fingerprint.as_mut().unwrap().bbox_hint =
                Some([[0., 0., 0.], [0.04, 0.03, 0.]]);
        }
        model
            .regenerate(
                k,
                &FeatureRegistry::with_defaults(),
                Some(&params),
                Some(&refs),
            )
            .unwrap();
        let body = model.active_body().unwrap();
        let actual = k.mass_properties(body, 1.).unwrap();
        let bore = PI * 0.0033_f64.powi(2) * 0.008;
        let seat = PI * (0.0055_f64.powi(2) - 0.0033_f64.powi(2)) * 0.004;
        let volume = 12e-6 - bore - seat;
        assert!((actual.volume_m3 - volume).abs() < 1e-12);
        let z = (12e-6 * 0.005
            - bore * if bottom { 0.004 } else { 0.006 }
            - seat * if bottom { 0.002 } else { 0.008 })
            / volume;
        assert!((actual.center_of_mass[2] - z).abs() < 1e-12);
        let mesh = k
            .tessellate(body, &opencad_geometry::TessellationSettings::default())
            .unwrap();
        let (v, c) = evidence::integrated(&mesh, false);
        assert!((v - volume).abs() < 3e-9);
        assert!((c[2] - z).abs() < 3e-6);
    }
}
#[test]
fn blind_counterbores_and_bottom_entry_have_independent_contact_and_first_moments() {
    blind(&MockGeometryKernel::with_analytic_primitives());
    blind(&OcctGeometryKernel::new());
}
