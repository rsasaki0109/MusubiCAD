use opencad_desktop::sketch_editor::*;
use opencad_file::{read_ocad, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
fn fixture() -> OcadDocument {
    read_ocad(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/ellipse_plate.ocad.d"),
    )
    .unwrap()
}
fn edit(doc: &OcadDocument, operations: Vec<SketchForm>) -> SketchEdit {
    SketchEdit {
        document_id: None,
        revision: sketch_editor_state(doc).unwrap().revision,
        sketch_id: "sketch:ellipse".into(),
        operations,
    }
}
fn ellipse() -> SketchForm {
    SketchForm::Ellipse {
        id: "ent:ellipse".into(),
        center: "ent:center".into(),
        major_radius: "40 mm".into(),
        minor_radius: "15 mm".into(),
        axis_angle: "0 deg".into(),
        construction: false,
        create: false,
    }
}
fn analytic<K: GeometryKernel>(kernel: &K) {
    let mut doc = fixture();
    let original = doc.clone();
    let mut history = DocumentHistory::default();
    let request = edit(
        &doc,
        vec![
            ellipse(),
            SketchForm::Point {
                id: "ent:center".into(),
                x: "10 mm".into(),
                y: "5 mm".into(),
                create: false,
            },
        ],
    );
    let plan = plan_sketch_edit_with_kernel(&doc, &request, kernel).unwrap();
    assert_eq!(doc, original);
    assert_eq!(plan.patch.operations.len(), 2);
    assert!(
        (plan.evidence.volume_m3.unwrap() - std::f64::consts::PI * 0.04 * 0.015 * 0.006).abs()
            < 1e-12
    );
    let sizes = plan.evidence.bounds_size_m.unwrap();
    for (actual, expected) in sizes.into_iter().zip([0.08, 0.03, 0.006]) {
        assert!(
            (actual - expected).abs() < 3e-7,
            "bounds {sizes:?} expected {expected}"
        );
    }
    apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).unwrap();
    assert_eq!(doc.parameters, original.parameters);
    assert_eq!(doc.feature_nodes, original.feature_nodes);
    assert_eq!(doc.sketches[0].id, original.sketches[0].id);
    let line_edit = edit(
        &doc,
        vec![
            SketchForm::Point {
                id: "ent:a".into(),
                x: "0 mm".into(),
                y: "20 mm".into(),
                create: true,
            },
            SketchForm::Point {
                id: "ent:b".into(),
                x: "20 mm".into(),
                y: "22 mm".into(),
                create: true,
            },
            SketchForm::Line {
                id: "ent:guide".into(),
                start: "ent:a".into(),
                end: "ent:b".into(),
                construction: true,
            },
            SketchForm::Horizontal {
                id: "con:horizontal".into(),
                line: "ent:guide".into(),
            },
            SketchForm::Distance {
                id: "con:distance".into(),
                a: "ent:a".into(),
                b: "ent:b".into(),
                expr: "25 mm".into(),
            },
        ],
    );
    let plan = plan_sketch_edit_with_kernel(&doc, &line_edit, kernel).unwrap();
    let solved = &plan.state.sketches[0].solved;
    let point = |id| {
        let opencad_sketch::SketchEntity::Point(p) = solved.find_entity(id).unwrap() else {
            panic!()
        };
        [
            opencad_sketch::solve::coord_value(&p.x).unwrap(),
            opencad_sketch::solve::coord_value(&p.y).unwrap(),
        ]
    };
    let (a, b) = (point("ent:a"), point("ent:b"));
    assert!((a[1] - b[1]).abs() < 1e-8);
    assert!(((b[0] - a[0]).hypot(b[1] - a[1]) - 0.025).abs() < 1e-8);
    assert!(
        (plan.evidence.volume_m3.unwrap() - std::f64::consts::PI * 0.04 * 0.015 * 0.006).abs()
            < 1e-12
    );
    assert_eq!(plan.state.sketches[0].source.constraints.len(), 2);
}
#[test]
fn interactive_sketch_commands_match_backend_constraints_and_analytic_profile() {
    analytic(&MockGeometryKernel::new());
    #[cfg(feature = "occt")]
    analytic(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn interactive_sketch_failures_retain_source_and_history() {
    let mut doc = fixture();
    let before = doc.clone();
    let mut history = DocumentHistory::default();
    for form in [
        SketchForm::Point {
            id: "ent:center".into(),
            x: "10".into(),
            y: "0 mm".into(),
            create: false,
        },
        SketchForm::Point {
            id: "ent:missing".into(),
            x: "10 mm".into(),
            y: "0 mm".into(),
            create: false,
        },
        SketchForm::Radius {
            id: "con:unsupported".into(),
            target: "ent:ellipse".into(),
            expr: "10 mm".into(),
        },
        SketchForm::Construction {
            id: "ent:ellipse".into(),
            construction: true,
        },
        SketchForm::Ellipse {
            id: "ent:ellipse".into(),
            center: "ent:center".into(),
            major_radius: "40 mm".into(),
            minor_radius: "50 mm".into(),
            axis_angle: "0 deg".into(),
            construction: false,
            create: false,
        },
    ] {
        let request = edit(&doc, vec![form]);
        assert!(apply_sketch_edit_with_kernel(
            &mut doc,
            &request,
            &mut history,
            &MockGeometryKernel::new()
        )
        .is_err());
        assert_eq!(doc, before);
        assert!(!DocumentHistoryState::new(history.clone()).can_undo);
    }
    let stale = SketchEdit {
        document_id: None,
        revision: "old".into(),
        ..edit(&doc, vec![ellipse()])
    };
    assert!(plan_sketch_edit_with_kernel(&doc, &stale, &MockGeometryKernel::new()).is_err());
}
#[cfg(feature = "occt")]
#[test]
fn interactive_sketch_failed_commit_and_undo_redo_preserve_source_identity() {
    use opencad_desktop::{redo_document_with_history, undo_document_with_history};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("edit.ocad.d");
    let path = path.to_str().unwrap();
    let doc = fixture();
    opencad_file::write_ocad(path, &doc).unwrap();
    let request = edit(&doc, vec![ellipse()]);
    let original = opencad_file::expanded_dir::serialize_document_files(&doc).unwrap();
    plan_document_sketch_edit(path, &request).unwrap();
    assert_eq!(
        opencad_file::expanded_dir::serialize_document_files(&read_ocad(path).unwrap()).unwrap(),
        original
    );
    let result = apply_document_sketch_edit(path, &request, None).unwrap();
    let changed = read_ocad(path).unwrap();
    let stable = opencad_file::expanded_dir::serialize_document_files(&changed).unwrap();
    let history = result.history.clone();
    assert!(apply_document_sketch_edit(path, &request, Some(history.history)).is_err());
    assert_eq!(
        opencad_file::expanded_dir::serialize_document_files(&read_ocad(path).unwrap()).unwrap(),
        stable
    );
    let undone = undo_document_with_history(path, result.history.history).unwrap();
    assert_eq!(read_ocad(path).unwrap().sketches, doc.sketches);
    redo_document_with_history(path, undone.history).unwrap();
    assert_eq!(read_ocad(path).unwrap().sketches, changed.sketches);
}
use opencad_file::DocumentHistoryState;

#[test]
fn conflicting_construction_circle_constraints_refuse_and_leave_source_unchanged() {
    let doc = fixture();
    let before = doc.clone();
    let request = edit(
        &doc,
        vec![
            SketchForm::Circle {
                id: "ent:guide_circle".into(),
                center: "ent:anchor".into(),
                radius: "5 mm".into(),
                construction: true,
                create: true,
            },
            SketchForm::Radius {
                id: "con:r1".into(),
                target: "ent:guide_circle".into(),
                expr: "5 mm".into(),
            },
            SketchForm::Radius {
                id: "con:r2".into(),
                target: "ent:guide_circle".into(),
                expr: "10 mm".into(),
            },
        ],
    );
    assert!(plan_sketch_edit_with_kernel(&doc, &request, &MockGeometryKernel::new()).is_err());
    assert_eq!(doc, before);
    let request = edit(
        &doc,
        vec![request.operations[0].clone(), request.operations[1].clone()],
    );
    let plan = plan_sketch_edit_with_kernel(&doc, &request, &MockGeometryKernel::new()).unwrap();
    assert!(
        (plan.evidence.volume_m3.unwrap() - std::f64::consts::PI * 0.03 * 0.02 * 0.006).abs()
            < 1e-12
    );
    let mut candidate = doc;
    let mut history = DocumentHistory::default();
    apply_sketch_edit_with_kernel(
        &mut candidate,
        &request,
        &mut history,
        &MockGeometryKernel::new(),
    )
    .unwrap();
    let remove = edit(
        &candidate,
        vec![SketchForm::RemoveConstraint {
            id: "con:r1".into(),
        }],
    );
    let plan =
        plan_sketch_edit_with_kernel(&candidate, &remove, &MockGeometryKernel::new()).unwrap();
    assert!(plan.state.sketches[0].source.constraints.is_empty());
}
