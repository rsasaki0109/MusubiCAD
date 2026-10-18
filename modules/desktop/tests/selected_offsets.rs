//! ADR-110: independent copies, analytic physical output and protected history.
use opencad_core::{DocumentId, DocumentMetadata, Length};
use opencad_desktop::{editor::*, sketch_creation::*, sketch_editor::*, sketch_offset::*};
use opencad_feature::{datum::DatumPlaneFeature, FeatureDefinition, FeatureNode, FeatureRegistry};
use opencad_file::{document_design_state, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_sketch::{GlobalPlane, Workplane};
use serde_json::{json, Value};

fn recipe() -> Value {
    serde_json::from_str(include_str!(
        "../../../examples/agent/offset_selected_sketch.json"
    ))
    .unwrap()
}
fn intent(doc: &OcadDocument) -> SketchOffset {
    let mut value = recipe()["offset"].clone();
    value["document_id"] = json!(doc.metadata.id.as_str());
    value["revision"] = json!(feature_tree(doc).unwrap().revision);
    serde_json::from_value(value).unwrap()
}
fn fixture<K: GeometryKernel>(kernel: &K, plane: GlobalPlane, tilted: bool) -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:offset_controls").unwrap(),
        "Selected offset controls",
    ));
    let recipe = recipe();
    for p in recipe["parameters"].as_array().unwrap() {
        doc.parameters
            .add_parameter(serde_json::from_value(p.clone()).unwrap())
            .unwrap();
    }
    if tilted {
        doc.feature_nodes.push(FeatureNode::new(
            "feature:datum",
            "Tilted datum",
            FeatureDefinition::DatumPlane(DatumPlaneFeature {
                base: Workplane::Custom {
                    origin: [0.01, -0.02, 0.03],
                    normal: [0., 0.6, 0.8],
                    x_axis: [1., 0., 0.],
                },
                offset: Length::from_meters(0.005),
                offset_expr: None,
            }),
        ));
        doc.feature_graph = opencad_feature::derive_feature_graph(
            &doc.feature_nodes,
            &["feature:datum".into()],
            &[],
            &[],
        )
        .unwrap();
    }
    let state = sketch_creation_state_with_kernel(&doc, kernel).unwrap();
    let mut history = DocumentHistory::default();
    apply_sketch_create_with_kernel(
        &mut doc,
        &SketchCreate {
            document_id: state.document_id,
            revision: state.revision,
            sketch_id: state.suggested_sketch_id,
            feature_id: state.suggested_feature_id,
            name: "Offset source".into(),
            plane: (!tilted).then_some(plane),
            datum_plane: tilted.then(|| "feature:datum".into()),
        },
        &mut history,
        kernel,
    )
    .unwrap();
    let edit = SketchEdit {
        document_id: Some(doc.metadata.id.as_str().into()),
        revision: feature_tree(&doc).unwrap().revision,
        sketch_id: "sketch:new_1".into(),
        operations: serde_json::from_value(recipe["primitives"].clone()).unwrap(),
    };
    apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap();
    // Test setup only; file/protocol fixtures create these through DesignPatch.
    doc.sketches[0].constraints = serde_json::from_value(recipe["constraints"].clone()).unwrap();
    doc
}
fn near(a: [f64; 3], b: [f64; 3], tol: f64) {
    for i in 0..3 {
        assert!((a[i] - b[i]).abs() < tol, "{a:?} != {b:?}");
    }
}
#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod mesh_evidence;
fn measure<K: GeometryKernel>(
    doc: &OcadDocument,
    kernel: &K,
    volume: f64,
    area: f64,
    low: [f64; 3],
    high: [f64; 3],
    center: [f64; 3],
) {
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            None,
        )
        .unwrap();
    let solid = model.active_body().unwrap();
    let mass = kernel.mass_properties(solid, 1.).unwrap();
    assert!((mass.volume_m3 - volume).abs() < 1e-12);
    assert!((mass.area_m2 - area).abs() < 1e-10);
    near(mass.center_of_mass, center, 3e-7);
    let bounds = kernel.bounding_box(solid).unwrap();
    near(bounds.min, low, 3e-7);
    near(bounds.max, high, 3e-7);
    let mesh = kernel
        .tessellate(
            solid,
            &opencad_geometry::TessellationSettings {
                linear_deflection: 1e-5,
                angular_deflection_deg: 4.,
            },
        )
        .unwrap();
    let (actual, centroid) = mesh_evidence::integrated(&mesh, false);
    assert!((actual / volume - 1.).abs() < 0.01);
    near(centroid, center, 1e-4);
    for axis in 0..3 {
        let lo = mesh
            .positions
            .iter()
            .map(|v| f64::from(v[axis]))
            .fold(f64::INFINITY, f64::min);
        let hi = mesh
            .positions
            .iter()
            .map(|v| f64::from(v[axis]))
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((lo - low[axis]).abs() < 1e-4 && (hi - high[axis]).abs() < 1e-4);
    }
}
fn extrude<K: GeometryKernel>(doc: &mut OcadDocument, owner: &str, profile: &str, kernel: &K) {
    let revision = feature_tree(doc).unwrap().revision;
    apply_feature_edit_with_kernel(
        doc,
        &revision,
        FeatureEdit::CreateExtrude {
            id: "feature:offset_body".into(),
            name: "Independent offset body".into(),
            sketch_feature: owner.into(),
            profile_ref: profile.into(),
            length_expr: "6 mm".into(),
        },
        &mut DocumentHistory::default(),
        kernel,
    )
    .unwrap();
}
fn placed<K: GeometryKernel>(kernel: &K) {
    for (plane, tilted, low, high, center) in [
        (
            GlobalPlane::XY,
            false,
            [-0.011, -0.016, 0.],
            [0.015, 0.010, 0.006],
            [0.002, -0.003, 0.003],
        ),
        (
            GlobalPlane::YZ,
            false,
            [0., -0.011, -0.016],
            [0.006, 0.015, 0.010],
            [0.003, 0.002, -0.003],
        ),
        (
            GlobalPlane::XZ,
            false,
            [-0.011, -0.006, -0.016],
            [0.015, 0., 0.010],
            [0.002, -0.003, -0.003],
        ),
        (
            GlobalPlane::XY,
            true,
            [-0.001, -0.0298, 0.028],
            [0.025, -0.0054, 0.0484],
            [0.012, -0.0176, 0.0382],
        ),
    ] {
        let mut doc = fixture(kernel, plane, tilted);
        let before = doc.clone();
        let offset = intent(&doc);
        let plan = plan_sketch_offset_with_kernel(&doc, &offset, kernel).unwrap();
        assert_eq!(doc, before);
        assert_eq!(plan.destination_sketch_id, "sketch:offset_1");
        assert_eq!(plan.destination_feature_id, "feature:offset_1");
        assert!(plan.evidence.volume_m3.is_none());
        assert_eq!(plan.patch.operations.len(), 4);
        let repeated = plan_sketch_offset_with_kernel(&doc, &offset, kernel).unwrap();
        assert_eq!(
            serde_json::to_value(&plan).unwrap(),
            serde_json::to_value(repeated).unwrap()
        );
        let mut history = DocumentHistory::default();
        apply_sketch_offset_with_kernel(&mut doc, &offset, &mut history, kernel).unwrap();
        assert_eq!(doc.sketches[0], before.sketches[0]);
        assert_eq!(doc.parameters, before.parameters);
        let copy = doc
            .sketches
            .iter()
            .find(|s| s.id.as_str() == plan.destination_sketch_id)
            .unwrap();
        assert_eq!(copy.workplane, before.sketches[0].workplane);
        assert!(copy.constraints.is_empty());
        let copy_source = copy.entities.clone();
        let profile = plan
            .state
            .sketches
            .iter()
            .find(|s| s.source.id.as_str() == plan.destination_sketch_id)
            .unwrap()
            .solved
            .profiles[0]
            .id
            .clone();
        extrude(&mut doc, &plan.destination_feature_id, &profile, kernel);
        measure(
            &doc,
            kernel,
            1014. * std::f64::consts::PI * 1e-9,
            494. * std::f64::consts::PI * 1e-6,
            low,
            high,
            center,
        );
        doc.parameters.set_expr("param:diameter", "24 mm").unwrap();
        assert_eq!(doc.sketches[1].entities, copy_source);
        measure(
            &doc,
            kernel,
            1014. * std::f64::consts::PI * 1e-9,
            494. * std::f64::consts::PI * 1e-6,
            low,
            high,
            center,
        );
    }
}
#[test]
fn placed_independent_circle_copies_have_real_analytic_mass_mesh_and_source() {
    placed(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    placed(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn ordered_rectangles_and_inward_circles_have_independent_dimensions() {
    fn check<K: GeometryKernel>(kernel: &K) {
        for side in [OffsetSide::Outward, OffsetSide::Inward] {
            let mut doc = fixture(kernel, GlobalPlane::XY, false);
            let mut offset = intent(&doc);
            offset.curve_ids =
                serde_json::from_value(recipe()["rectangle_curve_ids"].clone()).unwrap();
            offset.side = side;
            let plan = plan_sketch_offset_with_kernel(&doc, &offset, kernel).unwrap();
            let before = doc.sketches[0].clone();
            assert_eq!(plan.destination_curve_ids.len(), 4);
            assert_eq!(plan.destination_point_ids.len(), 4);
            apply_sketch_offset_with_kernel(
                &mut doc,
                &offset,
                &mut DocumentHistory::default(),
                kernel,
            )
            .unwrap();
            assert_eq!(doc.sketches[0], before);
            let row = plan
                .state
                .sketches
                .iter()
                .find(|s| s.source.id.as_str() == plan.destination_sketch_id)
                .unwrap();
            extrude(
                &mut doc,
                &plan.destination_feature_id,
                &row.solved.profiles[0].id,
                kernel,
            );
            let (w, h, lo, hi) = if side == OffsetSide::Outward {
                (0.086, 0.066, [0.037, -0.033, 0.], [0.123, 0.033, 0.006])
            } else {
                (0.074, 0.054, [0.043, -0.027, 0.], [0.117, 0.027, 0.006])
            };
            measure(
                &doc,
                kernel,
                w * h * 0.006,
                2. * (w * h + (w + h) * 0.006),
                lo,
                hi,
                [0.080, 0., 0.003],
            );
        }
        let mut doc = fixture(kernel, GlobalPlane::XY, false);
        let mut offset = intent(&doc);
        offset.side = OffsetSide::Inward;
        let plan = plan_sketch_offset_with_kernel(&doc, &offset, kernel).unwrap();
        apply_sketch_offset_with_kernel(&mut doc, &offset, &mut DocumentHistory::default(), kernel)
            .unwrap();
        let row = plan
            .state
            .sketches
            .iter()
            .find(|s| s.source.id.as_str() == plan.destination_sketch_id)
            .unwrap();
        extrude(
            &mut doc,
            &plan.destination_feature_id,
            &row.solved.profiles[0].id,
            kernel,
        );
        measure(
            &doc,
            kernel,
            294. * std::f64::consts::PI * 1e-9,
            182. * std::f64::consts::PI * 1e-6,
            [-0.005, -0.010, 0.],
            [0.009, 0.004, 0.006],
            [0.002, -0.003, 0.003],
        );
    }
    check(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn unsupported_sources_units_topology_and_stale_intent_never_publish() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let doc = fixture(kernel, GlobalPlane::XY, true);
        let before = doc.clone();
        let good = intent(&doc);
        for (field, value) in [
            ("document_id", json!("doc:foreign")),
            ("revision", json!("stale")),
            ("source_sketch_id", json!("sketch:absent")),
            ("distance", json!("3")),
            ("distance", json!("-3 mm")),
            ("distance", json!("0 mm")),
            ("distance", json!("NaN m")),
            ("distance", json!("diameter / 2")),
            ("distance", json!("3 deg")),
            ("name", json!(" ")),
            ("curve_ids", json!([])),
            ("curve_ids", json!(["ent:offset_a"])),
            ("curve_ids", json!(["ent:offset_disc", "ent:offset_disc"])),
            (
                "curve_ids",
                json!([
                    "ent:offset_bottom",
                    "ent:offset_top",
                    "ent:offset_right",
                    "ent:offset_left"
                ]),
            ),
            ("curve_ids", json!(["ent:offset_bottom"])),
        ] {
            let mut request = serde_json::to_value(&good).unwrap();
            request[field] = value;
            let offset: SketchOffset = serde_json::from_value(request).unwrap();
            let mut changed = doc.clone();
            let mut history = DocumentHistory::default();
            assert!(
                apply_sketch_offset_with_kernel(&mut changed, &offset, &mut history, kernel)
                    .is_err(),
                "{field}"
            );
            assert_eq!(changed, before);
            assert_eq!(history, DocumentHistory::default());
        }
        let mut offset = good.clone();
        offset.side = OffsetSide::Inward;
        offset.distance = "10 mm".into();
        assert!(plan_sketch_offset_with_kernel(&doc, &offset, kernel).is_err());
        let mut offset = good.clone();
        offset.curve_ids = serde_json::from_value(recipe()["rectangle_curve_ids"].clone()).unwrap();
        offset.side = OffsetSide::Inward;
        offset.distance = "30 mm".into();
        assert!(plan_sketch_offset_with_kernel(&doc, &offset, kernel).is_err());
        for modification in [
            "construction",
            "owner",
            "custom_plane",
            "contradiction",
            "missing_datum",
        ] {
            let mut candidate = doc.clone();
            match modification {
                "construction" => candidate.sketches[0]
                    .entities
                    .iter_mut()
                    .find(|e| e.id().as_str() == "ent:offset_disc")
                    .unwrap()
                    .set_construction(true),
                "owner" => {
                    candidate
                        .feature_nodes
                        .iter_mut()
                        .find(|f| f.id == "feature:sketch_new_1")
                        .unwrap()
                        .suppressed = true
                }
                "custom_plane" => {
                    candidate.sketches[0].workplane = Workplane::Custom {
                        origin: [0., 0., 0.],
                        normal: [0., 0., 1.],
                        x_axis: [1., 0., 0.],
                    }
                }
                "missing_datum" => {
                    candidate.sketches[0].workplane = Workplane::DatumPlane {
                        feature_id: "feature:absent".into(),
                    }
                }
                _ => candidate.sketches[0].constraints.extend(
                    serde_json::from_value::<Vec<opencad_sketch::Constraint>>(json!([
                        {"type":"radius","id":"con:r1","target":"ent:offset_disc","expr":"10 mm"},
                        {"type":"radius","id":"con:r2","target":"ent:offset_disc","expr":"11 mm"}
                    ]))
                    .unwrap(),
                ),
            }
            let offset = intent(&candidate);
            let original = candidate.clone();
            let mut history = DocumentHistory::default();
            assert!(
                apply_sketch_offset_with_kernel(&mut candidate, &offset, &mut history, kernel)
                    .is_err(),
                "{modification}"
            );
            assert_eq!(candidate, original);
            assert_eq!(history, DocumentHistory::default());
        }
        let mut collision = doc.clone();
        // An entity in a different sketch occupies the first generated family.
        let mut history = DocumentHistory::default();
        apply_sketch_create_with_kernel(
            &mut collision,
            &SketchCreate {
                document_id: doc.metadata.id.as_str().into(),
                revision: feature_tree(&doc).unwrap().revision,
                sketch_id: "sketch:foreign".into(),
                feature_id: "feature:foreign".into(),
                name: "Foreign source".into(),
                plane: Some(GlobalPlane::XY),
                datum_plane: None,
            },
            &mut history,
            kernel,
        )
        .unwrap();
        let foreign = SketchEdit { document_id: Some(collision.metadata.id.as_str().into()),
            revision: feature_tree(&collision).unwrap().revision, sketch_id: "sketch:foreign".into(),
            operations: serde_json::from_value(json!([
                {"operation":"point","id":"ent:offset_1_point_1","x":"0 mm","y":"0 mm","create":true},
                {"operation":"circle","id":"ent:foreign_disc","center":"ent:offset_1_point_1","radius":"1 mm","construction":false,"create":true}
            ])).unwrap() };
        apply_sketch_edit_with_kernel(&mut collision, &foreign, &mut history, kernel).unwrap();
        let plan = plan_sketch_offset_with_kernel(&collision, &intent(&collision), kernel).unwrap();
        assert_eq!(plan.destination_sketch_id, "sketch:offset_2");
    }
    check(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[cfg(feature = "occt")]
#[test]
fn packed_expanded_readonly_reviews_and_canonical_history_preserve_exact_source() {
    use opencad_desktop::{redo_document_with_history, undo_document_with_history};
    let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
    let dir = tempfile::tempdir().unwrap();
    for name in ["offset.ocad.d", "offset.ocad"] {
        let path = dir.path().join(name);
        let path = path.to_str().unwrap();
        let doc = fixture(&kernel, GlobalPlane::XY, true);
        opencad_file::write_ocad(path, &doc).unwrap();
        let before = document_design_state(&doc);
        let offset = intent(&doc);
        let checksum = if name.ends_with(".d") {
            format!("{path}/checksums.json")
        } else {
            path.into()
        };
        let bytes = std::fs::read(&checksum).unwrap();
        plan_document_sketch_offset(path, &offset).unwrap();
        assert_eq!(std::fs::read(&checksum).unwrap(), bytes);
        let result = apply_document_sketch_offset(path, &offset, None).unwrap();
        let after = document_design_state(&opencad_file::read_ocad(path).unwrap());
        let history = undo_document_with_history(path, result.history.history).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            before
        );
        let history = redo_document_with_history(path, history.history).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            after
        );
        let bytes = std::fs::read(&checksum).unwrap();
        assert!(apply_document_sketch_offset(path, &offset, Some(history.history)).is_err());
        assert_eq!(std::fs::read(&checksum).unwrap(), bytes);
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            after
        );
    }
}

#[test]
fn source_catalogues_exclude_helpers_construction_and_ellipses_and_assertions_still_gate() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let mut doc = fixture(kernel, GlobalPlane::XY, false);
        doc.sketches[0].entities.extend(serde_json::from_value::<Vec<opencad_sketch::SketchEntity>>(json!([
            {"type":"rectangle","id":"ent:helper","origin":[-0.1,0.0],"size":[0.02,0.01],"corner_ids":["ent:h0","ent:h1","ent:h2","ent:h3"],"edge_ids":["ent:he0","ent:he1","ent:he2","ent:he3"]},
            {"type":"circle","id":"ent:construction","center":"ent:offset_center","radius":0.02,"construction":true},
            {"type":"ellipse","id":"ent:ellipse","center":"ent:offset_center","major_radius":0.03,"minor_radius":0.02,"axis_angle":0.0,"construction":true}
        ])).unwrap());
        let state = sketch_editor_state_with_kernel(&doc, kernel).unwrap();
        let catalog = &state.sketches[0].offset_targets;
        assert_eq!(
            catalog,
            &vec![
                "ent:offset_disc",
                "ent:offset_bottom",
                "ent:offset_right",
                "ent:offset_top",
                "ent:offset_left"
            ]
        );
        for id in [
            "ent:helper",
            "ent:he0",
            "ent:h0",
            "ent:construction",
            "ent:ellipse",
        ] {
            let mut offset = intent(&doc);
            offset.curve_ids = vec![id.into()];
            let mut candidate = doc.clone();
            let mut history = DocumentHistory::default();
            assert!(
                apply_sketch_offset_with_kernel(&mut candidate, &offset, &mut history, kernel)
                    .is_err()
            );
            assert_eq!(candidate, doc);
            assert_eq!(history, DocumentHistory::default());
        }
        let mut doc = fixture(kernel, GlobalPlane::XY, false);
        extrude(
            &mut doc,
            "feature:sketch_new_1",
            "profile:circle:ent:offset_disc",
            kernel,
        );
        doc.assertions.push(opencad_core::Assertion::new(
            "assertion:envelope",
            "Too small envelope",
            opencad_core::AssertionSeverity::Required,
            opencad_core::AssertionKind::BoundingBoxWithin {
                max_m: [0.019, 0.03, 0.01],
            },
        ));
        let offset = intent(&doc);
        let before = doc.clone();
        let mut history = DocumentHistory::default();
        let error = apply_sketch_offset_with_kernel(&mut doc, &offset, &mut history, kernel)
            .unwrap_err()
            .to_string();
        assert!(error.contains("assertions failed"), "{error}");
        assert_eq!(doc, before);
        assert_eq!(history, DocumentHistory::default());
    }
    check(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new());
}

#[test]
fn existing_tangent_line_arc_admission_keeps_explicit_topology_without_sampling() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let mut doc = fixture(kernel, GlobalPlane::XY, false);
        let half = std::f64::consts::FRAC_PI_2;
        doc.sketches[0].entities.extend(serde_json::from_value::<Vec<opencad_sketch::SketchEntity>>(json!([
            {"type":"point","id":"ent:s0","x":-0.08,"y":0.08},
            {"type":"point","id":"ent:s1","x":-0.04,"y":0.08},
            {"type":"point","id":"ent:s2","x":-0.04,"y":0.12},
            {"type":"point","id":"ent:s3","x":-0.08,"y":0.12},
            {"type":"point","id":"ent:sc0","x":-0.04,"y":0.10},
            {"type":"point","id":"ent:sc1","x":-0.08,"y":0.10},
            {"type":"line","id":"ent:sb","start":"ent:s0","end":"ent:s1"},
            {"type":"arc","id":"ent:sr","center":"ent:sc0","radius":0.02,"start_angle":-half,"end_angle":half,"start_point":"ent:s1","end_point":"ent:s2"},
            {"type":"line","id":"ent:st","start":"ent:s2","end":"ent:s3"},
            {"type":"arc","id":"ent:sl","center":"ent:sc1","radius":0.02,"start_angle":half,"end_angle":3.*half,"start_point":"ent:s3","end_point":"ent:s0"}
        ])).unwrap());
        let before = doc.clone();
        let mut offset = intent(&doc);
        offset.curve_ids = vec![
            "ent:sb".into(),
            "ent:sr".into(),
            "ent:st".into(),
            "ent:sl".into(),
        ];
        let plan = plan_sketch_offset_with_kernel(&doc, &offset, kernel).unwrap();
        assert_eq!(doc, before);
        assert_eq!(plan.destination_curve_ids.len(), 4);
        assert_eq!(plan.destination_point_ids.len(), 6);
        for (i, curve) in plan.curves.iter().enumerate() {
            match curve {
                opencad_sketch::curve_edit::EditCurve::Arc {
                    center_m,
                    radius_m,
                    sweep_rad,
                    ..
                } => {
                    assert!(
                        (radius_m - 0.023).abs() < 1e-12
                            && (sweep_rad - std::f64::consts::PI).abs() < 1e-12
                    );
                    near(
                        [center_m[0], center_m[1], 0.],
                        [if i == 1 { -0.04 } else { -0.08 }, 0.10, 0.],
                        1e-12,
                    );
                }
                opencad_sketch::curve_edit::EditCurve::Line { start_m, end_m } => {
                    assert!(((end_m[0] - start_m[0]).abs() - 0.04).abs() < 1e-12);
                    assert!((start_m[1] - if i == 0 { 0.077 } else { 0.123 }).abs() < 1e-12);
                }
                _ => panic!("stadium must retain analytic line/arc identities"),
            }
        }
        let mut history = DocumentHistory::default();
        apply_sketch_offset_with_kernel(&mut doc, &offset, &mut history, kernel).unwrap();
        assert_eq!(doc.sketches[0], before.sketches[0]);
        assert_eq!(doc.sketches[1].entities.len(), 10);
        assert!(doc.sketches[1].constraints.is_empty());
    }
    check(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new());
}
