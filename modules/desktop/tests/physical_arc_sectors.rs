//! Exact arc sectors retain ordinary source through one guarded patch and history.
use opencad_core::{DocumentId, DocumentMetadata, Length};
use opencad_desktop::{editor::*, sketch_creation::*};
use opencad_feature::{datum::DatumPlaneFeature, FeatureDefinition, FeatureNode, FeatureRegistry};
use opencad_file::{document_design_state, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_sketch::{GlobalPlane, Workplane};

fn fixture(datum: bool) -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:arc_sector").unwrap(),
        "Arc sector",
    ));
    if datum {
        doc.feature_nodes.push(FeatureNode::new(
            "feature:datum",
            "Tilted construction plane",
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
    doc
}
fn create<K: GeometryKernel>(
    doc: &mut OcadDocument,
    plane: GlobalPlane,
    datum: bool,
    history: &mut DocumentHistory,
    kernel: &K,
) {
    let state = sketch_creation_state_with_kernel(doc, kernel).unwrap();
    let request = SketchCreate {
        document_id: state.document_id,
        revision: state.revision,
        sketch_id: state.suggested_sketch_id,
        feature_id: state.suggested_feature_id,
        name: "Arc sector".into(),
        plane: (!datum).then_some(plane),
        datum_plane: datum.then(|| "feature:datum".into()),
    };
    apply_sketch_create_with_kernel(doc, &request, history, kernel).unwrap();
}

fn source_patch(doc: &OcadDocument) -> opencad_ai::DesignPatch {
    let entities:Vec<opencad_sketch::SketchEntity>=serde_json::from_value(serde_json::json!([
        {"type":"point","id":"ent:sector_center","construction":false,"x":"2.000 mm","y":"-3.000 mm"},
        {"type":"point","id":"ent:sector_start","construction":false,"x":"12.000 mm","y":"-3.000 mm"},
        {"type":"point","id":"ent:sector_end","construction":false,"x":"2.000 mm","y":"7.000 mm"},
        {"type":"arc","id":"ent:sector_arc","construction":false,"center":"ent:sector_center","radius":"10.000 mm","start_angle":"0 deg","end_angle":"90 deg","start_point":"ent:sector_start","end_point":"ent:sector_end"},
        {"type":"line","id":"ent:sector_start_ray","construction":false,"start":"ent:sector_center","end":"ent:sector_start"},
        {"type":"line","id":"ent:sector_end_ray","construction":false,"start":"ent:sector_end","end":"ent:sector_center"}
    ])).unwrap();
    let operations = entities
        .into_iter()
        .map(|entity| opencad_ai::PatchOperation::AddSketchEntity {
            sketch_id: "sketch:new_1".into(),
            entity,
        })
        .collect();
    opencad_ai::DesignPatch::new(operations)
        .with_revision_precondition(&document_design_state(doc))
        .unwrap()
}
fn complete<K: GeometryKernel>(kernel: &K) -> (OcadDocument, DocumentHistory) {
    let mut doc = fixture(true);
    let mut history = DocumentHistory::default();
    create(&mut doc, GlobalPlane::XY, true, &mut history, kernel);
    let before = doc.clone();
    let patch = source_patch(&doc);
    opencad_ai::ensure_patch_valid(&opencad_file::dry_run_patch_document(&doc, &patch)).unwrap();
    let mut candidate = doc.clone();
    opencad_file::apply_patch_to_document(&mut candidate, &patch).unwrap();
    verify_editor_document(&candidate, kernel).unwrap();
    assert_eq!(doc, before);
    opencad_file::apply_patch_with_history(&mut doc, &patch, &mut history, "Create arc sector")
        .unwrap();
    let authored = document_design_state(&doc);
    history.undo(&mut doc).unwrap();
    assert_eq!(document_design_state(&doc), document_design_state(&before));
    history.redo(&mut doc).unwrap();
    assert_eq!(document_design_state(&doc), authored);
    let tree = feature_tree(&doc).unwrap();
    let owner = tree
        .rows
        .iter()
        .find(|r| r.id == "feature:sketch_new_1")
        .unwrap();
    assert_eq!(owner.closed_profiles.len(), 1);
    apply_feature_edit_with_kernel(
        &mut doc,
        &tree.revision,
        FeatureEdit::CreateExtrude {
            id: "feature:sector_body".into(),
            name: "Exact sector prism".into(),
            sketch_feature: owner.id.clone(),
            profile_ref: owner.closed_profiles[0].clone(),
            length_expr: "6 mm".into(),
        },
        &mut history,
        kernel,
    )
    .unwrap();
    let stable = doc.clone();
    let saved = history.clone();
    assert!(
        opencad_file::apply_patch_with_history(&mut doc, &patch, &mut history, "Stale sector")
            .is_err()
    );
    assert_eq!(doc, stable);
    assert_eq!(history, saved);
    (doc, history)
}
fn regeneration<K: GeometryKernel>(kernel: &K) {
    let (doc, history) = complete(kernel);
    assert_eq!(history.undo_len(), 3);
    let original = doc.clone();
    let mut cache = opencad_feature::RegenerationCache::new();
    for parameters in [None, Some(&doc.parameters), None] {
        let mut model = doc.clone().into_part_model();
        model
            .regenerate_with_cache(
                kernel,
                &FeatureRegistry::with_defaults(),
                parameters,
                None,
                &mut cache,
            )
            .unwrap();
        let body = model.active_body().unwrap();
        let m = kernel.mass_properties(body, 1.).unwrap();
        assert!((m.volume_m3 - 150. * std::f64::consts::PI * 1e-9).abs() < 1e-12);
        assert!((m.area_m2 - (80. * std::f64::consts::PI + 120.) * 1e-6).abs() < 1e-10);
        let bounds = kernel.bounding_box(body).unwrap();
        for (actual, wanted) in bounds
            .min
            .into_iter()
            .chain(bounds.max)
            .zip([0.012, -0.0194, 0.0298, 0.022, -0.0078, 0.0406])
        {
            assert!((actual - wanted).abs() < 3e-7, "{actual} != {wanted}");
        }
        assert_eq!(doc, original);
    }
}
#[test]
fn authored_sector_source_and_atomic_history_regenerate_in_both_kernels_with_empty_parameters() {
    regeneration(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    regeneration(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn sector_source_roundtrips_in_packed_and_expanded_documents_without_literalizing_intent() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let (doc, _) = complete(&kernel);
    for name in ["sector.ocad", "sector.ocad.d"] {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(name);
        opencad_file::write_ocad(&path, &doc).unwrap();
        let restored = opencad_file::read_ocad(&path).unwrap();
        assert_eq!(
            document_design_state(&restored),
            document_design_state(&doc)
        );
        verify_editor_document(&restored, &kernel).unwrap();
        #[cfg(feature = "occt")]
        verify_editor_document(&restored, &opencad_kernel_occt::OcctGeometryKernel::new()).unwrap();
    }
}
