//! MCAD-P12-005: explicit title-block metadata, drawing files and verified Agent edits.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentKind, DocumentMetadata, Length};
use opencad_drawing::{TitleBlock, ViewMesh};
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
    let mut metadata = DocumentMetadata::new(DocumentId::new(id).expect("id"), "Title plate");
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
        "doc:title_drawing",
        "examples/agent/create_title_drawing_patch.json",
    )
}
fn part() -> OcadDocument {
    authored(
        DocumentKind::Part,
        "doc:title_plate",
        "examples/agent/create_detail_plate_patch.json",
    )
}
#[test]
#[ignore = "generate patch-authored title drawing and referenced part"]
fn write_title_drawing_example() {
    let path = root().join("examples/title_plate_drawing.ocad.d");
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
fn title_example_round_trip_and_legacy_migration() {
    let expected = drawing();
    let actual =
        read_expanded_dir(root().join("examples/title_plate_drawing.ocad.d")).expect("read");
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
        .all(|s| s.title_block.is_none()));
    assert_eq!(
        legacy,
        opencad_file::migrate::migrate_to_current(legacy.clone()).expect("migration")
    );
}
fn block() -> TitleBlock {
    drawing().drawing.expect("drawing").sheets[0]
        .title_block
        .clone()
        .expect("block")
}
#[test]
fn both_kernel_paths_render_metadata_with_unchanged_analytic_geometry() {
    let native = OcctGeometryKernel::new();
    let wire = native
        .make_wire_from_sketch(&SolvedSketch {
            profile_ref: "plate".into(),
            points: vec![[0., 0.], [0.08, 0.], [0.08, 0.06], [0., 0.06]],
            closed: true,
            ellipse: None,
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
    let native_volume = native.mass_properties(&body, 1.).expect("mass").volume_m3;
    assert!((native_volume - 0.08 * 0.06 * 0.006).abs() < 1e-12);
    let mesh = native
        .tessellate(&body, &TessellationSettings::default())
        .expect("mesh");
    render_metadata(mesh);
    assert!(
        (native
            .mass_properties(&body, 1.)
            .expect("unchanged")
            .volume_m3
            - native_volume)
            .abs()
            < 1e-12
    );
    let mock = MockGeometryKernel::new();
    let body = KernelBody(80);
    render_metadata(
        mock.tessellate(&body, &TessellationSettings::default())
            .expect("mock"),
    );
    assert!(
        (mock
            .mass_properties(&body, 1.)
            .expect("mock volume")
            .volume_m3
            - 0.08_f64.powi(3))
        .abs()
            < 1e-12
    );
}
fn render_metadata(mesh: opencad_geometry::MeshSet) {
    let mut sheet = drawing().drawing.expect("drawing").sheets[0].clone();
    let view = sheet.views[0].id.clone();
    let mut value = block();
    value.title = "Plate <A> & B".into();
    sheet.title_block = Some(value);
    let svg = opencad_drawing::render_sheet_svg(
        &sheet,
        &[ViewMesh {
            view_id: view,
            mesh_set: mesh,
        }],
    )
    .expect("SVG");
    assert!(svg.contains("Plate &lt;A&gt; &amp; B"));
    assert_eq!(svg.matches("class=\"title-block-cell\"").count(), 7);
    assert!(!svg.contains("Plate <A>"));
    assert!(svg.contains("width=\"180.000000\""));
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
    writeln!(process.stdin.take().expect("stdin"), "{request}").expect("request");
    let result = process.wait_with_output().expect("wait");
    assert!(result.status.success());
    serde_json::from_slice(&result.stdout).expect("json")
}
#[test]
fn verified_metadata_set_clear_and_failure_are_atomic() {
    let dir = tempfile::tempdir().expect("temp");
    let path = fixture(dir.path());
    let input = path.to_str().expect("path");
    let before = read_expanded_dir(&path).expect("before");
    let svg = dir.path().join("drawing.svg");
    let output = svg.to_str().expect("path");
    success(cli(&["export", input, output]));
    let first = std::fs::read(&svg).expect("SVG");
    success(cli(&["export", input, output]));
    assert_eq!(first, std::fs::read(&svg).expect("repeat"));
    assert_eq!(before, read_expanded_dir(&path).expect("no mutation"));
    for (id, value) in [
        ("sheet:missing", block()),
        (
            "sheet:a4",
            TitleBlock {
                title: "bad\nline".into(),
                ..block()
            },
        ),
    ] {
        let result = agent(
            &path,
            json!([{"type":"set_title_block","sheet_id":id,"title_block":value}]),
        );
        assert!(result["error"].is_object(), "{result}");
        assert_eq!(before, read_expanded_dir(&path).expect("atomic"));
    }
    assert!(serde_json::from_value::<DesignPatch>(
        json!({"operations":[{"type":"set_title_block","sheet_id":"sheet:a4"}]})
    )
    .is_err());
    let mut part = part();
    assert!(apply_patch_to_document(&mut part,&serde_json::from_value::<DesignPatch>(json!({"operations":[{"type":"set_title_block","sheet_id":"sheet:a4","title_block":block()}]})).expect("patch")).is_err());
    let edited = TitleBlock {
        revision: "B".into(),
        title: "Plate <B> & C".into(),
        ..block()
    };
    let result = agent(
        &path,
        json!([{"type":"set_title_block","sheet_id":"sheet:a4","title_block":edited}]),
    );
    assert!(result["error"].is_null(), "{result}");
    let after = read_expanded_dir(&path).expect("edited");
    assert_eq!(
        after.drawing.as_ref().expect("drawing").sheets[0]
            .title_block
            .as_ref(),
        Some(&edited)
    );
    assert_eq!(
        before.drawing.as_ref().expect("drawing").sheets[0].views,
        after.drawing.as_ref().expect("drawing").sheets[0].views
    );
    let result = agent(
        &path,
        json!([{"type":"set_title_block","sheet_id":"sheet:a4","title_block":null}]),
    );
    assert!(result["error"].is_null(), "{result}");
    assert!(read_expanded_dir(&path)
        .expect("clear")
        .drawing
        .expect("drawing")
        .sheets[0]
        .title_block
        .is_none());
}
