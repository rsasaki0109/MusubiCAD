use opencad_core::{Angle, Length};
use opencad_feature::*;
use opencad_geometry::{GeometryKernel, MockGeometryKernel, TessellationSettings};
use opencad_kernel_occt::OcctGeometryKernel;
use std::f64::consts::PI;
#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod evidence;
#[path = "support/countersink_fixture.rs"]
mod fixture;
fn physical<K: GeometryKernel>(k: &K, mock: bool) {
    let (mut model, mut params, refs) = fixture::fixture();
    let source = serde_json::to_value(&model.nodes).unwrap();
    let registry = FeatureRegistry::with_defaults();
    let mut cache = RegenerationCache::new();
    for (angle, diameter) in [(90_f64, 0.012_f64), (60., 0.012), (120., 0.014)] {
        params
            .set_expr("param:seat_angle", format!("{angle} deg"))
            .unwrap();
        params
            .set_expr("param:seat_diameter", format!("{} mm", diameter * 1000.))
            .unwrap();
        let report = model
            .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
            .unwrap();
        assert!(
            report
                .reference_provenance
                .iter()
                .all(|p| !p.status.is_unresolved()),
            "{:?}",
            report.reference_provenance
        );
        let r = 0.0033_f64;
        let cap = diameter / 2.;
        let d = (cap - r) / (angle.to_radians() / 2.).tan();
        let bore = PI * r * r * 0.01;
        let extra = PI * d * (cap * cap + cap * r - 2. * r * r) / 3.;
        let moment = PI * d * d * (cap * cap + 2. * cap * r - 3. * r * r) / 12.;
        let volume = 12e-6 - bore - extra;
        let center = [
            (12e-6 * 0.02 - (bore + extra) * 0.012) / volume,
            (12e-6 * 0.015 - (bore + extra) * 0.009) / volume,
            (12e-6 * 0.005 - bore * 0.005 - extra * 0.01 + moment) / volume,
        ];
        let body = model.active_body().unwrap();
        let mass = k.mass_properties(body, 1.).unwrap();
        assert!((mass.volume_m3 - volume).abs() < 2e-13);
        for (a, b) in mass.center_of_mass.into_iter().zip(center) {
            assert!((a - b).abs() < 2e-9);
        }
        let mesh = k
            .tessellate(body, &TessellationSettings::default())
            .unwrap();
        let (v, c) = evidence::integrated(&mesh, mock);
        assert!((v - volume).abs() < 3e-9);
        for (a, b) in c.into_iter().zip(center) {
            assert!((a - b).abs() < 3e-6);
        }
        // Contact circles and cone slope come from exported geometry, not metadata.
        let mut contact = 0;
        for p in &mesh.positions {
            let [x, y, z] = p.map(f64::from);
            let radial = (x - 0.012).hypot(y - 0.009);
            if z >= 0.01 - d - 3e-9 && radial <= cap + 3e-9 && radial >= r - 3e-9 {
                let expected = cap - (0.01 - z) * (cap - r) / d;
                assert!(
                    (radial - expected).abs() < 2e-8,
                    "{p:?} {expected} {radial}"
                );
                contact += 1;
            }
        }
        assert!(contact > 20);
        let warm = model
            .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
            .unwrap();
        assert!(warm.cached_nodes.contains(&"feature:seat".into()));
        // Successful in-memory regeneration resolves numeric fallback fields;
        // authored expressions and all other source identities remain intact.
        let mut resolved = source.clone();
        resolved["feature:seat"]["definition"]["seat"]["diameter"]["value_si"] = diameter.into();
        resolved["feature:seat"]["definition"]["seat"]["included_angle"]["value_si"] =
            angle.to_radians().into();
        assert_eq!(resolved, serde_json::to_value(&model.nodes).unwrap());
    }
    let before = model.outputs.clone();
    params.set_expr("param:seat_angle", "0 deg").unwrap();
    assert!(model
        .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
        .is_err());
    assert_eq!(before, model.outputs);
}
#[test]
fn both_kernels_match_derived_depth_volume_moments_contact_mesh_and_cache() {
    physical(&MockGeometryKernel::with_analytic_primitives(), true);
    physical(&OcctGeometryKernel::new(), false);
}
#[test]
fn closed_countersink_source_and_counterbore_only_reader_refuse_malformed_wire() {
    let (model, _, _) = fixture::fixture();
    let definition = &model.nodes["feature:seat"].definition;
    let source = serde_json::to_value(definition).unwrap();
    assert_eq!(
        *definition,
        serde_json::from_value::<FeatureDefinition>(source.clone()).unwrap()
    );
    for field in ["depth", "extra", "angle_unit", "missing_angle", "seat_type"] {
        let mut v = source.clone();
        match field {
            "angle_unit" => v["seat"]["included_angle"]["unit"] = "deg".into(),
            "missing_angle" => {
                v["seat"].as_object_mut().unwrap().remove("included_angle");
            }
            "seat_type" => v["seat"]["type"] = "future_seat".into(),
            _ => v["seat"][field] = serde_json::json!({"value_si":0.004}),
        }
        assert!(serde_json::from_value::<FeatureDefinition>(v).is_err());
    }
    #[derive(serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
    enum CounterboreOnlySeat {
        Counterbore {
            #[serde(flatten)]
            _fields: serde_json::Value,
        },
    }
    assert!(serde_json::from_value::<CounterboreOnlySeat>(source["seat"].clone()).is_err());
}
fn invalid<K: GeometryKernel>(k: &K) {
    for expr in [
        "0 deg",
        "180 deg",
        "181 deg",
        "-10 deg",
        "20 deg",
        "30 mm",
        "bore_depth",
        "unknown_angle_rad",
    ] {
        let (mut model, params, refs) = fixture::fixture();
        let FeatureDefinition::SeatedStandardHole(def) =
            &mut model.nodes.get_mut("feature:seat").unwrap().definition
        else {
            panic!("seat")
        };
        let StandardHoleSeat::Countersink {
            included_angle_expr,
            ..
        } = &mut def.seat
        else {
            panic!("countersink")
        };
        *included_angle_expr = Some(expr.into());
        let before = model.outputs.clone();
        assert!(
            model
                .regenerate(
                    k,
                    &FeatureRegistry::with_defaults(),
                    Some(&params),
                    Some(&refs)
                )
                .is_err(),
            "{expr}"
        );
        assert_eq!(before, model.outputs);
    }
    let (mut model, params, mut refs) = fixture::fixture();
    let registry = FeatureRegistry::with_defaults();
    let mut cache = RegenerationCache::new();
    model
        .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
        .unwrap();
    let before = model.outputs.clone();
    refs[0].semantic.normal_hint = Some([0., 0., -1.]);
    assert!(model
        .regenerate_with_cache(k, &registry, Some(&params), Some(&refs), &mut cache)
        .is_err());
    assert_eq!(before, model.outputs);
    let (model, _, _) = fixture::fixture();
    let FeatureDefinition::SeatedStandardHole(mut def) =
        model.nodes["feature:seat"].definition.clone()
    else {
        panic!("seat")
    };
    for angle in [f64::NAN, f64::INFINITY, 0., PI, 2. * PI] {
        def.seat = StandardHoleSeat::Countersink {
            diameter: Length::from_meters(0.012),
            included_angle: Angle::from_radians(angle),
            diameter_expr: None,
            included_angle_expr: None,
        };
        assert!(def.validate_dimensions().is_err());
    }
}
#[test]
fn invalid_angles_units_and_owned_refs_preserve_outputs_in_both_kernels() {
    invalid(&MockGeometryKernel::with_analytic_primitives());
    invalid(&OcctGeometryKernel::new());
}

fn placed<K: GeometryKernel>(k: &K, mock: bool) {
    let (mut model, params, mut refs) = fixture::fixture();
    let a = 25_f64.to_radians();
    let (sin, cos) = a.sin_cos();
    let transform = opencad_geometry::RigidTransform {
        rotation: [[1., 0., 0.], [0., cos, -sin], [0., sin, cos]],
        translation_m: [0.025, -0.013, 0.042],
    };
    let normal = [0., -sin, cos];
    for (id, z) in [("sketch:base", 0.), ("sketch:seat", 0.01)] {
        model.sketches.get_mut(id).unwrap().workplane = opencad_sketch::Workplane::Custom {
            origin: transform.transform_point([0., 0., z]),
            normal,
            x_axis: [1., 0., 0.],
        };
    }
    refs[0].semantic.normal_hint = Some(normal);
    let corners = [
        [0., 0., 0.01],
        [0.04, 0., 0.01],
        [0.04, 0.03, 0.01],
        [0., 0.03, 0.01],
    ]
    .map(|p| transform.transform_point(p));
    refs[0].geometric_fingerprint.as_mut().unwrap().bbox_hint = Some([
        std::array::from_fn(|i| corners.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min)),
        std::array::from_fn(|i| {
            corners
                .iter()
                .map(|p| p[i])
                .fold(f64::NEG_INFINITY, f64::max)
        }),
    ]);
    model
        .regenerate(
            k,
            &FeatureRegistry::with_defaults(),
            Some(&params),
            Some(&refs),
        )
        .unwrap();
    let r = 0.0033_f64;
    let cap = 0.006_f64;
    let d = cap - r;
    let bore = PI * r * r * 0.01;
    let extra = PI * d * (cap * cap + cap * r - 2. * r * r) / 3.;
    let moment = PI * d * d * (cap * cap + 2. * cap * r - 3. * r * r) / 12.;
    let v = 12e-6 - bore - extra;
    let center = transform.transform_point([
        (12e-6 * 0.02 - (bore + extra) * 0.012) / v,
        (12e-6 * 0.015 - (bore + extra) * 0.009) / v,
        (12e-6 * 0.005 - bore * 0.005 - extra * 0.01 + moment) / v,
    ]);
    let body = model.active_body().unwrap();
    let mass = k.mass_properties(body, 1.).unwrap();
    assert!((mass.volume_m3 - v).abs() < 2e-13);
    for (a, b) in mass.center_of_mass.into_iter().zip(center) {
        assert!((a - b).abs() < 2e-9);
    }
    let (mv, mc) = evidence::integrated(
        &k.tessellate(body, &TessellationSettings::default())
            .unwrap(),
        mock,
    );
    assert!((mv - v).abs() < 3e-9);
    for (a, b) in mc.into_iter().zip(center) {
        assert!((a - b).abs() < 3e-6);
    }
}
#[test]
fn non_axis_aligned_source_stock_and_owned_entry_work_in_both_kernels() {
    placed(&MockGeometryKernel::with_analytic_primitives(), true);
    placed(&OcctGeometryKernel::new(), false);
}
