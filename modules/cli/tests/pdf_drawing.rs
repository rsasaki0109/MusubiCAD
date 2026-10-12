//! MCAD-P12-006: native multi-page PDF, units, deterministic text and atomic refusal.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_file::{
    apply_patch_to_document, document_design_state, read_expanded_dir, write_expanded_dir,
    OcadDocument,
};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args(args)
        .output()
        .expect("CLI")
}
fn success(result: std::process::Output) -> String {
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).expect("utf8")
}
fn copy_dir(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).expect("directory");
    for entry in std::fs::read_dir(source).expect("entries") {
        let entry = entry.expect("entry");
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), target.join(entry.file_name())).expect("copy");
        }
    }
}
fn boxes(data: &[u8]) -> Vec<[f64; 2]> {
    String::from_utf8_lossy(data)
        .split("/MediaBox [")
        .skip(1)
        .map(|s| {
            let n: Vec<f64> = s
                .split(']')
                .next()
                .expect("box")
                .split_whitespace()
                .map(|v| v.parse().expect("number"))
                .collect();
            [n[2], n[3]]
        })
        .collect()
}
#[test]
fn all_native_annotations_export_as_deterministic_a4_vector_pdf() {
    for example in [
        "title_plate_drawing",
        "angular_plate_drawing",
        "radial_plate_drawing",
        "section_plate_drawing",
        "detail_plate_drawing",
    ] {
        let source = root().join(format!("examples/{example}.ocad.d"));
        let before = read_expanded_dir(&source).expect("before");
        let dir = tempfile::tempdir().expect("temp");
        let output = dir.path().join("drawing.pdf");
        let summary = success(cli(&[
            "export",
            source.to_str().expect("path"),
            output.to_str().expect("path"),
        ]));
        assert!(summary.contains("pages: 1"), "{summary}");
        let first = std::fs::read(&output).expect("PDF");
        success(cli(&[
            "export",
            source.to_str().expect("path"),
            output.to_str().expect("path"),
        ]));
        assert_eq!(
            first,
            std::fs::read(&output).expect("deterministic across processes")
        );
        assert!(first.starts_with(b"%PDF-1.7"));
        let b = boxes(&first);
        assert_eq!(b.len(), 1);
        assert!((b[0][0] - 210. * 72. / 25.4).abs() < 1e-3);
        assert!((b[0][1] - 297. * 72. / 25.4).abs() < 1e-3);
        if example != "section_plate_drawing" {
            assert!(
                String::from_utf8_lossy(&first).contains("/FontFile2"),
                "embedded subset font"
            );
        }
        assert_eq!(
            before,
            read_expanded_dir(&source).expect("unchanged source")
        );
    }
}
fn multipage() -> OcadDocument {
    let source =
        read_expanded_dir(root().join("examples/title_plate_drawing.ocad.d")).expect("drawing");
    let mut metadata = opencad_core::DocumentMetadata::new(
        opencad_core::DocumentId::new("doc:printable_drawing").expect("id"),
        "Printable plate",
    );
    metadata.kind = opencad_core::DocumentKind::Drawing;
    let mut doc = OcadDocument::new(metadata);
    apply_patch_to_document(
        &mut doc,
        &authoring_patch(&document_design_state(&source)).expect("authoring"),
    )
    .expect("new drawing");
    let mut sheet = doc.drawing.as_ref().expect("drawing").sheets[0].clone();
    sheet.id = opencad_core::SheetId::new("sheet:landscape").expect("id");
    sheet.name = "Landscape".into();
    sheet.width_m = 0.297;
    sheet.height_m = 0.210;
    sheet.views[0].id = opencad_core::ViewId::new("view:landscape").expect("id");
    // AddSheet must be empty; author the second view through its normal operation.
    let view = sheet.views.remove(0);
    let patch:DesignPatch=serde_json::from_value(json!({"operations":[{"type":"add_sheet","sheet":sheet},{"type":"add_drawing_view","sheet_id":"sheet:landscape","view":view}]})).expect("patch");
    apply_patch_to_document(&mut doc, &patch).expect("apply");
    doc
}
#[test]
#[ignore = "generate patch-authored multi-page printable drawing"]
fn write_pdf_example() {
    let path = root().join("examples/printable_plate_drawing.ocad.d");
    let doc = multipage();
    write_expanded_dir(&path, &doc).expect("write");
    copy_dir(
        &root().join("examples/title_plate_drawing.ocad.d/parts"),
        &path.join("parts"),
    );
    let patch = authoring_patch(&document_design_state(&doc)).expect("authoring");
    std::fs::write(
        root().join("examples/agent/create_printable_drawing_patch.json"),
        serde_json::to_string_pretty(&patch).expect("json") + "\n",
    )
    .expect("patch");
}
#[test]
fn native_multi_page_export_and_mcp_summary_preserve_units() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("drawing.ocad.d");
    let doc = multipage();
    write_expanded_dir(&path, &doc).expect("write");
    copy_dir(
        &root().join("examples/title_plate_drawing.ocad.d/parts"),
        &path.join("parts"),
    );
    let output = dir.path().join("multi.pdf");
    let request = json!({"jsonrpc":"2.0","id":1,"method":"opencad.export","params":{"path":path,"output":output}});
    let mut process = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .arg("agent")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("agent");
    writeln!(process.stdin.take().expect("stdin"), "{request}").expect("request");
    let result = process.wait_with_output().expect("wait");
    let response: Value = serde_json::from_slice(&result.stdout).expect("JSON");
    assert!(response["error"].is_null(), "{response}");
    assert_eq!(response["result"]["pages"], json!(2));
    let data = std::fs::read(&output).expect("PDF");
    let actual = boxes(&data);
    assert_eq!(actual.len(), 2);
    for (a, b) in actual.iter().zip([[210., 297.], [297., 210.]]) {
        assert!((a[0] - b[0] * 72. / 25.4).abs() < 1e-3);
        assert!((a[1] - b[1] * 72. / 25.4).abs() < 1e-3);
    }
    assert_eq!(
        document_design_state(&doc),
        document_design_state(&read_expanded_dir(&path).expect("unchanged"))
    );
}
#[test]
fn missing_glyph_and_bad_reference_preserve_existing_pdf_and_source() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("drawing.ocad.d");
    copy_dir(&root().join("examples/title_plate_drawing.ocad.d"), &path);
    let mut doc = read_expanded_dir(&path).expect("doc");
    doc.drawing.as_mut().expect("drawing").sheets[0]
        .title_block
        .as_mut()
        .expect("block")
        .title = "日本語".into();
    write_expanded_dir(&path, &doc).expect("write");
    let output = dir.path().join("existing.pdf");
    std::fs::write(&output, b"prior output").expect("sentinel");
    let result = cli(&[
        "export",
        path.to_str().expect("path"),
        output.to_str().expect("path"),
    ]);
    assert!(!result.status.success());
    assert_eq!(std::fs::read(&output).expect("prior"), b"prior output");
    assert_eq!(doc, read_expanded_dir(&path).expect("source unchanged"));
    doc.drawing.as_mut().expect("drawing").sheets[0]
        .title_block
        .as_mut()
        .expect("block")
        .title = "Plate".into();
    doc.drawing.as_mut().expect("drawing").sheets[0].views[0]
        .model
        .source_path = "parts/missing.ocad.d".into();
    write_expanded_dir(&path, &doc).expect("write");
    assert!(!cli(&[
        "export",
        path.to_str().expect("path"),
        output.to_str().expect("path")
    ])
    .status
    .success());
    assert_eq!(std::fs::read(&output).expect("prior"), b"prior output");
}

#[test]
fn native_pdf_render_preserves_analytic_solid_volume_and_sheet_units() {
    use opencad_geometry::{
        ExtrudeExtent, ExtrudeOperation, GeometryKernel, SolvedSketch, TessellationSettings,
    };
    let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
    let wire = kernel
        .make_wire_from_sketch(&SolvedSketch {
            profile_ref: "plate".into(),
            points: vec![[0., 0.], [0.08, 0.], [0.08, 0.06], [0., 0.06]],
            closed: true,
            circle: None,
            segments: vec![],
            placement: None,
        })
        .expect("wire");
    let body = kernel
        .extrude(
            wire,
            ExtrudeExtent::Distance {
                length: opencad_core::Length::from_meters(0.006),
            },
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.],
        )
        .expect("plate");
    let before = kernel.mass_properties(&body, 1.).expect("volume").volume_m3;
    assert!((before - 0.08 * 0.06 * 0.006).abs() < 1e-12);
    let sheet = read_expanded_dir(root().join("examples/title_plate_drawing.ocad.d"))
        .expect("drawing")
        .drawing
        .expect("drawing")
        .sheets[0]
        .clone();
    let meshes = [opencad_drawing::ViewMesh {
        view_id: sheet.views[0].id.clone(),
        mesh_set: kernel
            .tessellate(&body, &TessellationSettings::default())
            .expect("mesh"),
    }];
    let data = opencad_drawing::render_drawing_pdf(&[opencad_drawing::PdfPageInput {
        sheet: &sheet,
        meshes: &meshes,
        radial: &[],
        sections: &[],
    }])
    .expect("PDF");
    assert_eq!(boxes(&data).len(), 1);
    assert!(
        (kernel
            .mass_properties(&body, 1.)
            .expect("unchanged")
            .volume_m3
            - before)
            .abs()
            < 1e-12
    );
}
