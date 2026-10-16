//! ADR-102 source transactions and independent physical Mock/OCCT acceptance.
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_desktop::{editor::*, sketch_creation::*, sketch_editor::*};
use opencad_file::{DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_sketch::{GlobalPlane, Workplane};

fn empty() -> OcadDocument {
    OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:sketch_creation").unwrap(),
        "New sketches",
    ))
}
fn request(doc: &OcadDocument, plane: GlobalPlane) -> SketchCreate {
    let s = sketch_creation_state(doc).unwrap();
    SketchCreate {
        document_id: s.document_id,
        revision: s.revision,
        sketch_id: s.suggested_sketch_id,
        feature_id: s.suggested_feature_id,
        name: "Manufacturing sketch".into(),
        plane: Some(plane),
        datum_plane: None,
    }
}

#[test]
fn global_sketch_creation_is_atomic_and_revision_guarded() {
    let kernel = MockGeometryKernel::new();
    let mut doc = empty();
    let original = doc.clone();
    let mut history = DocumentHistory::default();
    let create = request(&doc, GlobalPlane::XY);
    assert_eq!(
        sketch_creation_state(&doc).unwrap(),
        sketch_creation_state(&doc).unwrap()
    );
    let plan = plan_sketch_create_with_kernel(&doc, &create, &kernel).unwrap();
    assert_eq!(doc, original);
    assert_eq!(plan.patch.operations.len(), 2);
    assert!(plan.state.sketches[0].source.entities.is_empty());
    assert!(plan.evidence.volume_m3.is_none());
    assert!(plan.evidence.bounds_size_m.is_none());
    assert!(!plan.evidence.no_op);
    apply_sketch_create_with_kernel(&mut doc, &create, &mut history, &kernel).unwrap();
    assert_eq!(history.undo_len(), 1);
    assert_eq!(doc.sketches.len(), 1);
    assert_eq!(doc.feature_nodes.len(), 1);
    assert_eq!(doc.feature_nodes[0].id, create.feature_id);
    assert_eq!(doc.sketches[0].id.as_str(), create.sketch_id);
    assert_eq!(
        doc.sketches[0].workplane,
        Workplane::Global {
            plane: GlobalPlane::XY
        }
    );
    assert_ne!(request(&doc, GlobalPlane::XY).sketch_id, create.sketch_id);
    let stable = doc.clone();
    let saved = history.clone();
    assert!(apply_sketch_create_with_kernel(&mut doc, &create, &mut history, &kernel).is_err());
    assert_eq!(doc, stable);
    assert_eq!(history, saved);
    // A late owner collision must not leave the earlier AddSketch behind.
    let mut collision = request(&doc, GlobalPlane::YZ);
    collision.feature_id = create.feature_id.clone();
    assert!(apply_sketch_create_with_kernel(&mut doc, &collision, &mut history, &kernel).is_err());
    assert_eq!(doc, stable);
    assert_eq!(history, saved);
    collision = request(&doc, GlobalPlane::YZ);
    collision.sketch_id = create.sketch_id;
    assert!(apply_sketch_create_with_kernel(&mut doc, &collision, &mut history, &kernel).is_err());
    assert_eq!(doc, stable);
    assert_eq!(history, saved);
    history.undo(&mut doc).unwrap();
    assert_eq!(doc, original);
    history.redo(&mut doc).unwrap();
    assert_eq!(doc, stable);
}

#[test]
fn creation_refuses_foreign_identity_kinds_names_ids_and_unknown_workplanes() {
    let mut doc = empty();
    let before = doc.clone();
    let mut history = DocumentHistory::default();
    for field in ["document", "revision", "sketch", "feature", "name"] {
        let mut c = request(&doc, GlobalPlane::XY);
        match field {
            "document" => c.document_id = "doc:another".into(),
            "revision" => c.revision = "stale".into(),
            "sketch" => c.sketch_id = "feature:wrong_namespace".into(),
            "feature" => c.feature_id = "sketch:wrong_namespace".into(),
            _ => c.name = "  ".into(),
        }
        assert!(
            apply_sketch_create_with_kernel(&mut doc, &c, &mut history, &MockGeometryKernel::new())
                .is_err(),
            "{field}"
        );
        assert_eq!(doc, before);
        assert_eq!(history.undo_len(), 0);
    }
    let mut other = doc.clone();
    other.metadata.id = DocumentId::new("doc:identical_source").unwrap();
    assert_eq!(
        feature_tree(&doc).unwrap().revision,
        feature_tree(&other).unwrap().revision
    );
    assert!(plan_sketch_create_with_kernel(
        &other,
        &request(&doc, GlobalPlane::XY),
        &MockGeometryKernel::new()
    )
    .is_err());
    for kind in [
        opencad_core::DocumentKind::Assembly,
        opencad_core::DocumentKind::Drawing,
    ] {
        other = doc.clone();
        other.metadata.kind = kind;
        assert!(sketch_creation_state(&other).is_err());
    }
    let good = serde_json::to_value(request(&doc, GlobalPlane::XY)).unwrap();
    for plane in [
        serde_json::json!("xy"),
        serde_json::json!("datum_plane"),
        serde_json::json!({"type":"custom"}),
    ] {
        let mut v = good.clone();
        v["plane"] = plane;
        assert!(serde_json::from_value::<SketchCreate>(v).is_err());
    }
    let mut v = good;
    v["origin_m"] = serde_json::json!([0, 0, 0]);
    assert!(serde_json::from_value::<SketchCreate>(v).is_err());
}

#[test]
fn sketch_only_review_preserves_source_and_required_assertions_still_refuse() {
    use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
    let kernel = MockGeometryKernel::new();
    let mut doc = empty();
    let mut history = DocumentHistory::default();
    let create = request(&doc, GlobalPlane::XY);
    apply_sketch_create_with_kernel(&mut doc, &create, &mut history, &kernel).unwrap();
    let edit = SketchEdit {
        document_id: None,
        revision: feature_tree(&doc).unwrap().revision,
        sketch_id: create.sketch_id.clone(),
        operations: vec![SketchForm::Point {
            id: "ent:open".into(),
            x: "2 mm".into(),
            y: "3 mm".into(),
            create: true,
        }],
    };
    let before = doc.clone();
    let plan = plan_sketch_edit_with_kernel(&doc, &edit, &kernel).unwrap();
    assert_eq!(doc, before);
    assert!(plan.evidence.volume_m3.is_none());
    apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, &kernel).unwrap();
    assert!(doc.sketches[0].profiles.is_empty());
    let mut model = doc.clone().into_part_model();
    model.prepare_sketches().unwrap();
    // Consuming a profile-free source is still rejected by both preflight paths.
    model
        .add_node(opencad_feature::FeatureNode::new(
            "feature:consumer",
            "Invalid solid",
            opencad_feature::FeatureDefinition::Extrude(opencad_feature::ExtrudeFeature::boss(
                &create.feature_id,
                "profile:missing",
                opencad_geometry::ExtrudeExtent::Distance {
                    length: opencad_core::Length::from_meters(0.006),
                },
            )),
        ))
        .unwrap();
    model
        .add_dependency(&create.feature_id, "feature:consumer")
        .unwrap();
    assert!(model.prepare_sketches().is_err());
    assert!(model
        .regenerate(
            &kernel,
            &opencad_feature::FeatureRegistry::with_defaults(),
            None,
            None
        )
        .is_err());
    let mut required = empty();
    required.assertions.push(Assertion::new(
        "assertion:mass",
        "Mass requires a solid",
        AssertionSeverity::Required,
        AssertionKind::MassRange {
            min_kg: 0.,
            max_kg: 1.,
        },
    ));
    let stable = required.clone();
    let mut history = DocumentHistory::default();
    let request = request(&required, GlobalPlane::XY);
    let error = apply_sketch_create_with_kernel(&mut required, &request, &mut history, &kernel)
        .unwrap_err();
    assert!(error.to_string().contains("assertions failed"));
    assert_eq!(required, stable);
    assert_eq!(history.undo_len(), 0);
}

fn physical<K: GeometryKernel>(kernel: &K) {
    for (plane, min, max) in [
        (GlobalPlane::XY, [-0.02, -0.01, 0.], [0.02, 0.01, 0.006]),
        (GlobalPlane::YZ, [0., -0.02, -0.01], [0.006, 0.02, 0.01]),
        (GlobalPlane::XZ, [-0.02, -0.006, -0.01], [0.02, 0., 0.01]),
    ] {
        let mut doc = empty();
        let mut history = DocumentHistory::default();
        let create = request(&doc, plane);
        apply_sketch_create_with_kernel(&mut doc, &create, &mut history, kernel).unwrap();
        let edit = SketchEdit {
            document_id: None,
            revision: feature_tree(&doc).unwrap().revision,
            sketch_id: create.sketch_id.clone(),
            operations: vec![
                SketchForm::Point {
                    id: "ent:center".into(),
                    x: "0 mm".into(),
                    y: "0 mm".into(),
                    create: true,
                },
                SketchForm::Ellipse {
                    id: "ent:ellipse".into(),
                    center: "ent:center".into(),
                    major_radius: "20 mm".into(),
                    minor_radius: "10 mm".into(),
                    axis_angle: "0 deg".into(),
                    construction: false,
                    create: true,
                },
            ],
        };
        apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap();
        let tree = feature_tree(&doc).unwrap();
        let profile = tree
            .rows
            .iter()
            .find(|r| r.id == create.feature_id)
            .unwrap()
            .closed_profiles[0]
            .clone();
        let edit = FeatureEdit::CreateExtrude {
            id: "feature:body".into(),
            name: "Physical ellipse".into(),
            sketch_feature: create.feature_id.clone(),
            profile_ref: profile,
            length_expr: "6 mm".into(),
        };
        apply_feature_edit_with_kernel(&mut doc, &tree.revision, edit, &mut history, kernel)
            .unwrap();
        let mut model = doc.clone().into_part_model();
        model
            .regenerate(
                kernel,
                &opencad_feature::FeatureRegistry::with_defaults(),
                Some(&doc.parameters),
                Some(&doc.semantic_refs),
            )
            .unwrap();
        let body = model.active_body().unwrap();
        let mass = kernel.mass_properties(body, 1.).unwrap();
        assert!((mass.volume_m3 - std::f64::consts::PI * 0.02 * 0.01 * 0.006).abs() < 1e-12);
        let bounds = kernel.bounding_box(body).unwrap();
        for i in 0..3 {
            assert!(
                (bounds.min[i] - min[i]).abs() < 3e-7,
                "{plane:?} min {bounds:?}"
            );
            assert!(
                (bounds.max[i] - max[i]).abs() < 3e-7,
                "{plane:?} max {bounds:?}"
            );
            assert!((mass.center_of_mass[i] - (min[i] + max[i]) * 0.5).abs() < 3e-7);
        }
        assert_eq!(doc.sketches[0].workplane, Workplane::Global { plane });
        assert_eq!(history.undo_len(), 3);
    }
}

#[test]
fn cached_drafts_cannot_bypass_owner_workplane_or_contradiction_admission() {
    use opencad_core::{ConstraintId, EntityId, Expression};
    use opencad_feature::{
        FeatureDefinition, FeatureNode, FeatureRegistry, RegenerationCache, SketchFeatureDef,
    };
    use opencad_sketch::{CircleEntity, Constraint, Coord, EntityBase, PointEntity, SketchEntity};
    let kernel = MockGeometryKernel::new();
    let mut doc = empty();
    let mut history = DocumentHistory::default();
    let create = request(&doc, GlobalPlane::XY);
    apply_sketch_create_with_kernel(&mut doc, &create, &mut history, &kernel).unwrap();
    let mut model = doc.into_part_model();
    let mut cache = RegenerationCache::default();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate_with_cache(&kernel, &registry, None, None, &mut cache)
        .unwrap();
    let warm = model
        .regenerate_with_cache(&kernel, &registry, None, None, &mut cache)
        .unwrap();
    assert_eq!(warm.cached_nodes, vec![create.feature_id.clone()]);
    assert!(model.active_body().is_none());
    for scenario in ["custom", "ambiguous", "suppressed", "contradiction"] {
        let mut bad = model.clone();
        let outputs = bad.outputs.clone();
        let mut warm_cache = cache.clone();
        match scenario {
            "custom" => {
                bad.sketches.get_mut(&create.sketch_id).unwrap().workplane = Workplane::Custom {
                    origin: [0.; 3],
                    normal: [0., 0., 1.],
                    x_axis: [1., 0., 0.],
                }
            }
            "ambiguous" => bad
                .add_node(FeatureNode::new(
                    "feature:other_owner",
                    "Ambiguous owner",
                    FeatureDefinition::Sketch(SketchFeatureDef {
                        sketch_id: create.sketch_id.clone(),
                    }),
                ))
                .unwrap(),
            "suppressed" => bad.nodes.get_mut(&create.feature_id).unwrap().suppressed = true,
            _ => {
                let s = bad.sketches.get_mut(&create.sketch_id).unwrap();
                s.entities.push(SketchEntity::Point(PointEntity {
                    base: EntityBase {
                        id: EntityId::new("ent:c").unwrap(),
                        construction: false,
                    },
                    x: Coord::Literal(0.),
                    y: Coord::Literal(0.),
                }));
                s.entities.push(SketchEntity::Circle(CircleEntity {
                    base: EntityBase {
                        id: EntityId::new("ent:guide").unwrap(),
                        construction: true,
                    },
                    center: EntityId::new("ent:c").unwrap(),
                    radius: Coord::Literal(0.005),
                }));
                for (id, expr) in [("con:one", "5 mm"), ("con:two", "6 mm")] {
                    s.constraints.push(Constraint::Radius {
                        id: ConstraintId::new(id).unwrap(),
                        target: EntityId::new("ent:guide").unwrap(),
                        expr: Expression::new(expr).unwrap(),
                    });
                }
            }
        }
        assert!(bad.prepare_sketches().is_err(), "{scenario}");
        assert!(
            bad.regenerate_with_cache(&kernel, &registry, None, None, &mut warm_cache)
                .is_err(),
            "{scenario}"
        );
        assert_eq!(bad.outputs, outputs, "{scenario}");
    }
}

#[test]
fn created_sketch_forms_match_independent_analytic_solids_in_all_global_planes() {
    physical(&MockGeometryKernel::new());
    #[cfg(feature = "occt")]
    physical(&opencad_kernel_occt::OcctGeometryKernel::new());
}

#[cfg(feature = "occt")]
#[test]
fn creation_failure_and_history_preserve_canonical_source_and_roundtrips() {
    use opencad_file::{read_ocad, write_ocad};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("part.ocad.d");
    let path = path.to_str().unwrap();
    let original = empty();
    write_ocad(path, &original).unwrap();
    let files = opencad_file::expanded_dir::serialize_document_files(&original).unwrap();
    let create = request(&original, GlobalPlane::XZ);
    let plan = plan_document_sketch_create(path, &create).unwrap();
    assert!(plan.evidence.volume_m3.is_none());
    assert_eq!(
        opencad_file::expanded_dir::serialize_document_files(&read_ocad(path).unwrap()).unwrap(),
        files
    );
    let result = apply_document_sketch_create(path, &create, None).unwrap();
    let changed = read_ocad(path).unwrap();
    let changed_files = opencad_file::expanded_dir::serialize_document_files(&changed).unwrap();
    assert!(
        apply_document_sketch_create(path, &create, Some(result.history.history.clone())).is_err()
    );
    assert_eq!(
        opencad_file::expanded_dir::serialize_document_files(&read_ocad(path).unwrap()).unwrap(),
        changed_files
    );
    let packed = dir.path().join("part.ocad");
    write_ocad(&packed, &changed).unwrap();
    assert_eq!(read_ocad(&packed).unwrap(), changed);
    let undone = opencad_desktop::undo_document_with_history(path, result.history.history).unwrap();
    assert_eq!(read_ocad(path).unwrap(), original);
    opencad_desktop::redo_document_with_history(path, undone.history).unwrap();
    assert_eq!(read_ocad(path).unwrap(), changed);
}
