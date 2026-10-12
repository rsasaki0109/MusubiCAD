//! MCAD-P12-003: exact native sections, drawing files and verified Agent edits.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentKind, DocumentMetadata, Length, ViewId};
use opencad_drawing::ProjectionKind;
use opencad_file::{
    apply_patch_to_document, document_design_state, read_expanded_dir, write_expanded_dir,
    OcadDocument,
};
use opencad_geometry::{
    ExtrudeExtent, ExtrudeOperation, GeometryKernel, KernelBody, SectionPlane, SolvedCircle,
    SolvedSketch,
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
    let mut metadata = DocumentMetadata::new(DocumentId::new(id).expect("id"), "Section plate");
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
        "doc:section_drawing",
        "examples/agent/create_section_drawing_patch.json",
    )
}
fn part() -> OcadDocument {
    authored(
        DocumentKind::Part,
        "doc:section_plate",
        "examples/agent/create_section_plate_patch.json",
    )
}
#[test]
#[ignore = "generate patch-authored section drawing and referenced part"]
fn write_section_drawing_example() {
    let path = root().join("examples/section_plate_drawing.ocad.d");
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
fn section_example_round_trip_and_legacy_migration() {
    let expected = drawing();
    let actual =
        read_expanded_dir(root().join("examples/section_plate_drawing.ocad.d")).expect("read");
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
        .all(|s| s.views.iter().all(|v| v.section.is_none())));
    assert_eq!(
        legacy,
        opencad_file::migrate::migrate_to_current(legacy.clone()).expect("migration")
    );
}
fn box_body(kernel: &OcctGeometryKernel, origin: [f64; 2], size: [f64; 2]) -> KernelBody {
    let [x, y] = origin;
    let [w, h] = size;
    let wire = kernel
        .make_wire_from_sketch(&SolvedSketch {
            profile_ref: "box".into(),
            points: vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]],
            closed: true,
            ellipse: None,
            circle: None,
            segments: vec![],
            placement: None,
        })
        .expect("wire");
    kernel
        .extrude(
            wire,
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.006),
            },
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.],
        )
        .expect("box")
}
#[test]
fn native_sections_match_analytic_areas_dimensions_and_volume() {
    let kernel = OcctGeometryKernel::new();
    let plate = box_body(&kernel, [0., 0.], [0.08, 0.06]);
    for (normal, origin, area) in [
        ([0., 0., 1.], [0., 0., 0.003], 0.0048),
        ([0., 1., 0.], [0., 0.03, 0.], 0.00048),
        ([1., 0., 0.], [0.04, 0., 0.], 0.00036),
    ] {
        let geometry = kernel
            .section_body(
                &plate,
                &SectionPlane {
                    origin_m: origin,
                    normal,
                },
            )
            .expect("section");
        assert!((geometry.area_m2 - area).abs() < 1e-12);
        assert_eq!(geometry.loops_m.len(), 1);
        assert_eq!(geometry.loops_m[0].len(), 4);
        for p in &geometry.loops_m[0] {
            let distance: f64 = p
                .iter()
                .zip(origin)
                .zip(normal)
                .map(|((a, b), n)| (a - b) * n)
                .sum();
            assert!(distance.abs() < 1e-9);
        }
    }
    for width in [0.08, 0.1] {
        let plate = box_body(&kernel, [0., 0.], [width, 0.06]);
        let hole = box_body(&kernel, [0.03, 0.02], [0.02, 0.02]);
        let ring = kernel
            .body_boolean(plate, hole, opencad_geometry::BooleanOp::Subtract)
            .expect("ring");
        let geometry = kernel
            .section_body(
                &ring,
                &SectionPlane {
                    origin_m: [0., 0., 0.003],
                    normal: [0., 0., 1.],
                },
            )
            .expect("holed slice");
        assert!((geometry.area_m2 - (width * 0.06 - 0.0004)).abs() < 1e-12);
        assert_eq!(geometry.loops_m.len(), 2);
        assert_eq!(geometry.loops_m.iter().map(Vec::len).sum::<usize>(), 8);
        let volume = kernel.mass_properties(&ring, 1.).expect("mass").volume_m3;
        assert!((volume - geometry.area_m2 * 0.006).abs() < 1e-12);
        let split = kernel
            .section_body(
                &ring,
                &SectionPlane {
                    origin_m: [0., 0.03, 0.],
                    normal: [0., 1., 0.],
                },
            )
            .expect("islands");
        assert_eq!(split.loops_m.len(), 2);
        assert!((split.area_m2 - (width - 0.02) * 0.006).abs() < 1e-12);
    }
    for z in [-0.01, 0., 0.006, 0.01] {
        assert!(kernel
            .section_body(
                &plate,
                &SectionPlane {
                    origin_m: [0., 0., z],
                    normal: [0., 0., 1.]
                }
            )
            .is_err());
    }
    let wire = kernel
        .make_wire_from_sketch(&SolvedSketch {
            profile_ref: "circle".into(),
            points: vec![],
            closed: true,
            ellipse: None,
            circle: Some(SolvedCircle {
                center_m: [0., 0.],
                radius_m: 0.01,
            }),
            segments: vec![],
            placement: None,
        })
        .expect("circle");
    let cylinder = kernel
        .extrude(
            wire,
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.006),
            },
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.],
        )
        .expect("cylinder");
    assert!(kernel
        .section_body(
            &cylinder,
            &SectionPlane {
                origin_m: [0., 0., 0.003],
                normal: [0., 0., 1.]
            }
        )
        .is_err());
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
fn verified_section_edits_regenerate_holes_and_atomic_failures() {
    let dir = tempfile::tempdir().expect("temp");
    let path = fixture(dir.path());
    let input = path.to_str().expect("path");
    let text = success(cli(&["regen", input]));
    assert_section(&text, "view:front");
    let before = read_expanded_dir(&path).expect("before");
    let svg = dir.path().join("drawing.svg");
    let output = svg.to_str().expect("path");
    success(cli(&["export", input, output]));
    let first = std::fs::read(&svg).expect("svg");
    success(cli(&["export", input, output]));
    assert_eq!(first, std::fs::read(&svg).expect("repeat"));
    let text = String::from_utf8(first).expect("utf8");
    assert!(text.contains("fill-rule=\"evenodd\""));
    assert!(text.contains("class=\"section-contour\""));
    assert!(text.contains("width=\"4.242641\""));
    assert!(text.contains("Z M "), "hole subpath: {text}");
    assert_eq!(before, read_expanded_dir(&path).expect("no caches"));
    let current = before.drawing.as_ref().expect("drawing").sheets[0].views[0].clone();
    for (projection, z) in [(ProjectionKind::Top, 0.003), (ProjectionKind::Front, 0.02)] {
        let mut bad = current.clone();
        bad.id = ViewId::new("view:bad").expect("id");
        bad.projection = projection;
        bad.section.as_mut().expect("section").plane.origin_m[2] = z;
        let result = agent(
            &path,
            json!([{"type":"add_drawing_view","sheet_id":"sheet:a4","view":bad}]),
        );
        assert!(result["error"].is_object(), "{result}");
        assert_eq!(before, read_expanded_dir(&path).expect("atomic"));
    }
    let result = agent(
        &path,
        json!([{"type":"add_drawing_dimension","sheet_id":"sheet:a4","dimension":{"id":"dim:section","view_id":"view:front","start_model_m":[0.,0.,0.003],"end_model_m":[0.08,0.,0.003],"offset_m":0.01}}]),
    );
    assert!(result["error"].is_object());
    assert_eq!(before, read_expanded_dir(&path).expect("atomic annotation"));
    let mut edited = current;
    edited.id = ViewId::new("view:section_updated").expect("id");
    edited.section.as_mut().expect("section").plane.origin_m[2] = 0.004;
    let result = agent(
        &path,
        json!([{"type":"remove_drawing_view","view_id":"view:front"},{"type":"add_drawing_view","sheet_id":"sheet:a4","view":edited}]),
    );
    assert!(result["error"].is_null(), "{result}");
    let text = success(cli(&["regen", input]));
    assert_section(&text, "view:section_updated");
}
fn assert_section(text: &str, view: &str) {
    let line = text
        .lines()
        .find(|line| line.starts_with(&format!("section: {view} ")))
        .expect("section evidence");
    let words: Vec<_> = line.split_whitespace().collect();
    let area: f64 = words[3].parse().expect("SI area");
    assert!((area - 0.0044).abs() < 1e-12);
    assert_eq!(words[5], "2");
    assert_eq!(words[7], "8");
}
