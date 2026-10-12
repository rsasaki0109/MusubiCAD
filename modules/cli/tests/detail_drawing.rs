//! MCAD-P12-004: bounded native details, drawing files and verified Agent edits.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentKind, DocumentMetadata, Length, ViewId};
use opencad_drawing::{ProjectionKind, Sheet, ViewMesh};
use opencad_file::{
    apply_patch_to_document, document_design_state, read_expanded_dir, write_expanded_dir,
    OcadDocument,
};
use opencad_geometry::{
    ExtrudeExtent, ExtrudeOperation, GeometryKernel, KernelBody, MockGeometryKernel, SolvedSketch,
    TessellationSettings,
};
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn authored(kind: DocumentKind, id: &str, patch_path: &str) -> OcadDocument {
    let mut metadata = DocumentMetadata::new(DocumentId::new(id).expect("id"), "Detail plate");
    metadata.kind = kind;
    let mut doc = OcadDocument::new(metadata);
    let patch: DesignPatch =
        serde_json::from_str(&std::fs::read_to_string(root().join(patch_path)).expect("patch"))
            .expect("json");
    apply_patch_to_document(&mut doc, &patch).expect("apply");
    doc
}
fn drawing() -> OcadDocument {
    authored(
        DocumentKind::Drawing,
        "doc:detail_drawing",
        "examples/agent/create_detail_drawing_patch.json",
    )
}
fn part() -> OcadDocument {
    authored(
        DocumentKind::Part,
        "doc:detail_plate",
        "examples/agent/create_detail_plate_patch.json",
    )
}
#[test]
#[ignore = "generate patch-authored detail drawing and referenced part"]
fn write_detail_drawing_example() {
    let path = root().join("examples/detail_plate_drawing.ocad.d");
    write_expanded_dir(&path, &drawing()).expect("drawing");
    write_expanded_dir(path.join("parts/plate.ocad.d"), &part()).expect("part");
}
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args(args)
        .output()
        .expect("CLI")
}
fn success(output: std::process::Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8")
}
fn fixture(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("drawing.ocad.d");
    write_expanded_dir(&path, &drawing()).expect("write");
    write_expanded_dir(path.join("parts/plate.ocad.d"), &part()).expect("part");
    path
}
#[test]
fn detail_example_round_trip_and_legacy_migration() {
    let expected = drawing();
    let actual =
        read_expanded_dir(root().join("examples/detail_plate_drawing.ocad.d")).expect("read");
    assert_eq!(
        document_design_state(&actual),
        document_design_state(&expected)
    );
    let patch = authoring_patch(&document_design_state(&actual)).expect("authoring");
    let mut rebuilt = OcadDocument::new(actual.metadata.clone());
    apply_patch_to_document(&mut rebuilt, &patch).expect("rebuild");
    assert_eq!(
        document_design_state(&rebuilt),
        document_design_state(&actual)
    );
    let dir = tempfile::tempdir().expect("temp");
    write_expanded_dir(dir.path(), &rebuilt).expect("write");
    assert_eq!(rebuilt, read_expanded_dir(dir.path()).expect("round trip"));
    let legacy =
        read_expanded_dir(root().join("examples/semantic_bracket_drawing.ocad.d")).expect("legacy");
    assert!(legacy
        .drawing
        .as_ref()
        .expect("drawing")
        .sheets
        .iter()
        .all(|s| s.views.iter().all(|v| v.detail.is_none())));
    assert_eq!(
        legacy,
        opencad_file::migrate::migrate_to_current(legacy.clone()).expect("migration")
    );
}
fn sheet() -> Sheet {
    drawing().drawing.expect("drawing").sheets[0].clone()
}
fn analytic_detail(kernel: &impl GeometryKernel, body: &KernelBody) {
    let sheet = sheet();
    let child = sheet
        .views
        .iter()
        .find(|v| v.detail.is_some())
        .expect("child");
    let mesh = kernel
        .tessellate(body, &TessellationSettings::default())
        .expect("mesh");
    let projected = opencad_drawing::project_mesh_wireframe(&mesh, ProjectionKind::Front);
    let segments =
        opencad_drawing::layout_detail_segments(child, &sheet, &projected).expect("detail");
    assert_eq!(segments.len(), 2);
    let expected = [
        ([0.135, 0.105], [0.155, 0.105]),
        ([0.155, 0.085], [0.155, 0.105]),
    ];
    for (a, b) in expected {
        assert!(
            segments.iter().any(|s| {
                let near =
                    |p: [f64; 2], q: [f64; 2]| p.iter().zip(q).all(|(a, b)| (a - b).abs() < 1e-9);
                (near(s.start_m, a) && near(s.end_m, b)) || (near(s.start_m, b) && near(s.end_m, a))
            }),
            "{segments:?}"
        );
    }
    let meshes = sheet
        .views
        .iter()
        .map(|v| ViewMesh {
            view_id: v.id.clone(),
            mesh_set: mesh.clone(),
        })
        .collect::<Vec<_>>();
    let svg = opencad_drawing::render_sheet_svg(&sheet, &meshes).expect("svg");
    assert!(svg.contains("DETAIL 2.00×"));
    assert!(svg.contains("class=\"detail-source\""));
}
#[test]
fn native_and_mock_crops_match_analytic_plate_corner_dimensions_and_volume() {
    let native = OcctGeometryKernel::new();
    let wire = native
        .make_wire_from_sketch(&SolvedSketch {
            profile_ref: "plate".into(),
            points: vec![[0., 0.], [0.08, 0.], [0.08, 0.06], [0., 0.06]],
            closed: true,
            ellipse: None,
            cubic_bezier: None,
            circle: None,
            segments: vec![],
            placement: None,
        })
        .expect("wire");
    let body = native
        .extrude(
            wire,
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.006),
            },
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.],
        )
        .expect("plate");
    assert!(
        (native.mass_properties(&body, 1.).expect("volume").volume_m3 - 0.08 * 0.06 * 0.006).abs()
            < 1e-12
    );
    analytic_detail(&native, &body);
    // Mock is a cube, so choose the upper-right corner for its 80 × 80 mm XY face.
    let mock = MockGeometryKernel::new();
    let body = KernelBody(80);
    let mut sheet = sheet();
    let child = sheet
        .views
        .iter_mut()
        .find(|v| v.detail.is_some())
        .expect("detail");
    child.detail.as_mut().expect("detail").region_min_m = [0.06, 0.06];
    child.detail.as_mut().expect("detail").region_max_m = [0.09, 0.09];
    let mesh = mock
        .tessellate(&body, &TessellationSettings::default())
        .expect("mesh");
    let projected = opencad_drawing::project_mesh_wireframe(&mesh, ProjectionKind::Front);
    let child = sheet
        .views
        .iter()
        .find(|v| v.detail.is_some())
        .expect("detail");
    let actual = opencad_drawing::layout_detail_segments(child, &sheet, &projected).expect("crop");
    assert_eq!(actual.len(), 2);
    for segment in actual {
        assert!(
            ((segment.end_m[0] - segment.start_m[0]).hypot(segment.end_m[1] - segment.start_m[1])
                - 0.02)
                .abs()
                < 1e-9
        );
    }
    assert!(
        (mock.mass_properties(&body, 1.).expect("volume").volume_m3 - 0.08_f64.powi(3)).abs()
            < 1e-12
    );
}
fn agent(path: &std::path::Path, operations: Value) -> Value {
    let request = json!({"jsonrpc":"2.0","id":1,"method":"opencad.patch_apply_document","params":{"path":path,"patch":{"operations":operations}}});
    let mut process = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .arg("agent")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("agent");
    writeln!(process.stdin.take().expect("stdin"), "{request}").expect("write");
    let result = process.wait_with_output().expect("wait");
    assert!(result.status.success());
    serde_json::from_slice(&result.stdout).expect("json")
}
#[test]
fn parent_scale_region_removal_and_verified_edits_are_atomic() {
    let dir = tempfile::tempdir().expect("temp");
    let path = fixture(dir.path());
    let input = path.to_str().expect("path");
    let text = success(cli(&["regen", input]));
    assert!(
        text.contains(
            "detail: view:detail parent: view:front magnification: 2 scale: 1 segments: 2"
        ),
        "{text}"
    );
    let before = read_expanded_dir(&path).expect("before");
    let svg = dir.path().join("drawing.svg");
    let output = svg.to_str().expect("path");
    success(cli(&["export", input, output]));
    let first = std::fs::read(&svg).expect("svg");
    success(cli(&["export", input, output]));
    assert_eq!(first, std::fs::read(&svg).expect("repeat"));
    assert_eq!(before, read_expanded_dir(&path).expect("no caches"));
    for operations in [
        json!([{"type":"remove_drawing_view","view_id":"view:front"}]),
        json!([{"type":"set_drawing_view_scale","view_id":"view:front","scale":1.}]),
        json!([{"type":"set_drawing_view_scale","view_id":"view:detail","scale":2.}]),
    ] {
        let result = agent(&path, operations);
        assert!(result["error"].is_object(), "{result}");
        assert_eq!(before, read_expanded_dir(&path).expect("atomic"));
    }
    let child = before.drawing.as_ref().expect("drawing").sheets[0]
        .views
        .iter()
        .find(|v| v.detail.is_some())
        .expect("child")
        .clone();
    for case in 0..4 {
        let mut bad = child.clone();
        bad.id = ViewId::new("view:bad").expect("id");
        match case {
            0 => {
                bad.detail.as_mut().expect("detail").parent_view_id =
                    ViewId::new("view:missing").expect("id")
            }
            1 => {
                bad.detail.as_mut().expect("detail").region_min_m = [1., 1.];
                bad.detail.as_mut().expect("detail").region_max_m = [1.01, 1.01];
            }
            2 => {
                bad.detail.as_mut().expect("detail").parent_view_id =
                    ViewId::new("view:detail").expect("id")
            }
            _ => bad.model.source_doc = DocumentId::new("doc:other").expect("id"),
        };
        let result = agent(
            &path,
            json!([{"type":"add_drawing_view","sheet_id":"sheet:a4","view":bad}]),
        );
        assert!(result["error"].is_object(), "{result}");
        assert_eq!(before, read_expanded_dir(&path).expect("atomic"));
    }
    let result = agent(
        &path,
        json!([{"type":"set_drawing_view_scale","view_id":"view:front","scale":1.},{"type":"set_drawing_view_scale","view_id":"view:detail","scale":2.}]),
    );
    assert!(result["error"].is_null(), "{result}");
    let text = success(cli(&["regen", input]));
    assert!(
        text.contains("magnification: 2 scale: 2 segments: 2"),
        "{text}"
    );
    let result = agent(
        &path,
        json!([{"type":"remove_drawing_view","view_id":"view:detail"},{"type":"remove_drawing_view","view_id":"view:front"}]),
    );
    assert!(result["error"].is_null(), "{result}");
    assert!(read_expanded_dir(&path)
        .expect("empty")
        .drawing
        .expect("drawing")
        .sheets[0]
        .views
        .is_empty());
}
