//! MCAD-P12-007: native drawing DXF units, annotation geometry and atomic export.
use opencad_file::{read_expanded_dir, write_expanded_dir};
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
fn records(data: &str) -> Vec<Vec<(i32, &str)>> {
    let lines: Vec<_> = data.lines().collect();
    assert_eq!(lines.len() % 2, 0);
    let mut result = Vec::new();
    let mut current = Vec::new();
    for pair in lines.chunks_exact(2) {
        let code = pair[0].parse().expect("group code");
        if code == 0 && !current.is_empty() {
            result.push(current);
            current = Vec::new();
        }
        current.push((code, pair[1]));
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}
fn value<'a>(r: &[(i32, &'a str)], code: i32) -> &'a str {
    r.iter()
        .find(|(c, _)| *c == code)
        .map(|(_, v)| *v)
        .expect("tag")
}
fn number(r: &[(i32, &str)], code: i32) -> f64 {
    value(r, code).parse().expect("number")
}
#[test]
fn native_annotations_are_deterministic_utf8_millimetre_dxf() {
    for (example, expected) in [
        ("title_plate_drawing", "Inspection plate"),
        ("angular_plate_drawing", "90.00°"),
        ("radial_plate_drawing", "⌀ 20.00 mm"),
        ("section_plate_drawing", ""),
        ("detail_plate_drawing", "DETAIL 2.00×"),
    ] {
        let source = root().join(format!("examples/{example}.ocad.d"));
        let before = read_expanded_dir(&source).expect("source");
        let dir = tempfile::tempdir().expect("temp");
        let output = dir.path().join("drawing.dxf");
        let args = [
            "export",
            source.to_str().expect("path"),
            output.to_str().expect("path"),
        ];
        let summary = success(cli(&args));
        assert!(summary.contains("format: dxf"));
        let first = std::fs::read_to_string(&output).expect("DXF");
        success(cli(&args));
        assert_eq!(
            first,
            std::fs::read_to_string(&output).expect("deterministic")
        );
        assert!(first.contains("$INSUNITS\n70\n4\n"));
        assert!(first.contains("$ACADVER\n1\nAC1027\n"));
        assert!(first.ends_with("0\nEOF\n"));
        let entries = records(&first);
        assert!(entries
            .iter()
            .any(|r| r[0] == (0, "LWPOLYLINE") && value(r, 8) == "PAPER"));
        if !expected.is_empty() {
            assert!(entries
                .iter()
                .any(|r| r[0] == (0, "TEXT") && value(r, 1) == expected));
        }
        if example == "angular_plate_drawing" {
            let arc = entries.iter().find(|r| r[0] == (0, "ARC")).expect("arc");
            assert!((number(arc, 40) - 10.).abs() < 1e-6);
            assert!(((number(arc, 51) - number(arc, 50)).rem_euclid(360.) - 90.).abs() < 1e-6);
        }
        if example == "title_plate_drawing" {
            let lines: Vec<_> = entries
                .iter()
                .filter(|r| r[0] == (0, "LINE") && ["VISIBLE", "HIDDEN"].contains(&value(r, 8)))
                .collect();
            for (code, other, lo, hi) in [(10, 11, 50., 90.), (20, 21, 100., 130.)] {
                let coords: Vec<_> = lines
                    .iter()
                    .flat_map(|r| [number(r, code), number(r, other)])
                    .collect();
                assert!((coords.iter().copied().fold(f64::INFINITY, f64::min) - lo).abs() < 1e-6);
                assert!(
                    (coords.iter().copied().fold(f64::NEG_INFINITY, f64::max) - hi).abs() < 1e-6
                );
            }
        }
        if example == "section_plate_drawing" {
            let hatch: Vec<_> = entries
                .iter()
                .filter(|r| r[0] == (0, "LINE") && value(r, 8) == "HATCH")
                .collect();
            assert!(!hatch.is_empty());
            for r in hatch {
                let x = (number(r, 10) + number(r, 11)) / 2.;
                let y = (number(r, 20) + number(r, 21)) / 2.;
                assert!(
                    !(x > 65. + 1e-6 && x < 75. - 1e-6 && y > 60. + 1e-6 && y < 70. - 1e-6),
                    "hole midpoint {x},{y}"
                );
                assert!(
                    ((number(r, 11) - number(r, 10)) - (number(r, 21) - number(r, 20))).abs()
                        < 1e-6
                );
            }
        }
        assert_eq!(
            before,
            read_expanded_dir(&source).expect("unchanged source")
        );
    }
}
#[test]
fn multi_sheet_bad_reference_and_text_codes_preserve_existing_output() {
    let dir = tempfile::tempdir().expect("temp");
    let output = dir.path().join("drawing.dxf");
    std::fs::write(&output, b"previous").expect("sentinel");
    assert!(!cli(&[
        "export",
        root()
            .join("examples/printable_plate_drawing.ocad.d")
            .to_str()
            .expect("path"),
        output.to_str().expect("path")
    ])
    .status
    .success());
    assert_eq!(std::fs::read(&output).expect("previous"), b"previous");
    let path = dir.path().join("source.ocad.d");
    copy_dir(&root().join("examples/title_plate_drawing.ocad.d"), &path);
    let mut doc = read_expanded_dir(&path).expect("doc");
    doc.drawing.as_mut().expect("drawing").sheets[0]
        .title_block
        .as_mut()
        .expect("block")
        .title = "Encoded %%c".into();
    write_expanded_dir(&path, &doc).expect("write");
    assert!(!cli(&[
        "export",
        path.to_str().expect("path"),
        output.to_str().expect("path")
    ])
    .status
    .success());
    assert_eq!(std::fs::read(&output).expect("previous"), b"previous");
    assert_eq!(doc, read_expanded_dir(&path).expect("source unchanged"));
    doc.drawing.as_mut().expect("drawing").sheets[0]
        .title_block
        .as_mut()
        .expect("block")
        .title = "日本語 ⌀20 50%".into();
    write_expanded_dir(&path, &doc).expect("write");
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
    assert_eq!(response["result"]["format"], "dxf");
    assert!(std::fs::read_to_string(&output)
        .expect("UTF8")
        .contains("日本語 ⌀20 50%"));
    std::fs::write(&output, b"previous").expect("sentinel");
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
    assert_eq!(std::fs::read(&output).expect("previous"), b"previous");
}

#[test]
fn native_dxf_render_preserves_analytic_solid_volume_and_sheet_units() {
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
    let data = opencad_drawing::render_sheet_dxf(&sheet, &meshes, &[], &[]).expect("DXF");
    assert!(data.contains("$INSUNITS\n70\n4\n"));
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
