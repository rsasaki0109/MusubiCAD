//! MCAD-P12-001: analytic native angles, drawing files and verified Agent edits.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{
    DimensionId, DocumentId, DocumentKind, DocumentMetadata, Length, TopoRefId, ViewId,
};
use opencad_drawing::{AngularDimension, AngularMeasurement, RayEnd};
use opencad_file::{
    apply_patch_to_document, document_design_state, read_expanded_dir, write_expanded_dir,
    OcadDocument,
};
use opencad_geometry::{
    ExtrudeExtent, ExtrudeOperation, GeometryKernel, KernelBody, SolvedCircle, SolvedSketch,
    TopoRef,
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
    let mut metadata = DocumentMetadata::new(DocumentId::new(id).expect("id"), "Angular plate");
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
        "doc:angular_drawing",
        "examples/agent/create_angular_drawing_patch.json",
    )
}
fn part() -> OcadDocument {
    authored(
        DocumentKind::Part,
        "doc:angular_plate",
        "examples/agent/create_angular_plate_part_patch.json",
    )
}
#[test]
#[ignore = "generate patch-authored angular drawing and referenced part"]
fn write_angular_drawing_example() {
    let path = root().join("examples/angular_plate_drawing.ocad.d");
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
fn angular_example_round_trip_and_legacy_migration() {
    let expected = drawing();
    let actual =
        read_expanded_dir(root().join("examples/angular_plate_drawing.ocad.d")).expect("read");
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
        .all(|s| s.angular_dimensions.is_empty()));
    assert_eq!(
        legacy,
        opencad_file::migrate::migrate_to_current(legacy.clone()).expect("migration")
    );
}
fn ref_for(
    kernel: &OcctGeometryKernel,
    body: &KernelBody,
    id: &str,
    ends: [[f64; 3]; 2],
) -> TopoRef {
    let discoveries = kernel.discover_body_edges(body).expect("edges");
    let discovery = discoveries
        .iter()
        .find(|d| {
            let points = kernel
                .edge_endpoints(body, d.kernel_edge_id)
                .expect("endpoints");
            ends.iter().all(|expected| {
                points
                    .iter()
                    .any(|p| p.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-9))
            })
        })
        .expect("matching edge");
    TopoRef::kernel_edge(
        TopoRefId::new(id).expect("id"),
        "feature:triangle",
        &discovery.role,
        discovery.kernel_edge_id,
        discovery.midpoint_m,
        discovery.tangent_m,
    )
}
#[test]
fn occt_analytic_triangle_angles_and_curved_edge_refusal() {
    let kernel = OcctGeometryKernel::new();
    for degrees in [30_f64, 45., 60., 90.] {
        let x = if degrees == 90. { 0. } else { 0.04 };
        let y = if degrees == 90. {
            0.04
        } else {
            x * degrees.to_radians().tan()
        };
        let wire = kernel
            .make_wire_from_sketch(&SolvedSketch {
                profile_ref: "triangle".into(),
                points: vec![[0., 0.], [0.08, 0.], [x, y]],
                closed: true,
                ellipse: None,
                circle: None,
                segments: vec![],
                placement: None,
            })
            .expect("wire");
        let body = kernel
            .extrude(
                wire,
                ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                ExtrudeOperation::NewBody,
                None,
                [0., 0., 1.],
            )
            .expect("body");
        let actual = kernel.mass_properties(&body, 1.).expect("mass").volume_m3;
        assert!((actual - 0.5 * 0.08 * y * 0.006).abs() < 1e-11);
        let refs = vec![
            ref_for(
                &kernel,
                &body,
                "ref:edge:base",
                [[0., 0., 0.006], [0.08, 0., 0.006]],
            ),
            ref_for(
                &kernel,
                &body,
                "ref:edge:side",
                [[0., 0., 0.006], [x, y, 0.006]],
            ),
        ];
        let dim = AngularDimension {
            id: DimensionId::new("dim:triangle").expect("id"),
            view_id: ViewId::new("view:front").expect("id"),
            measurement: AngularMeasurement::Edges {
                edge_refs: [refs[0].ref_id.clone(), refs[1].ref_id.clone()],
                ray_ends: [RayEnd::End; 2],
            },
            arc_radius_m: 0.01,
        };
        let (resolved, proof) = opencad_drawing::resolve_angular_dimension(
            &dim,
            &refs,
            &kernel.discover_body_edges(&body).expect("edges"),
            &kernel,
            &body,
        )
        .expect("resolve");
        assert!(
            (resolved.measured_angle_rad().expect("angle") - degrees.to_radians()).abs() < 1e-10
        );
        assert_eq!(proof.len(), 2);
        let moved = kernel
            .transform_body(
                body.clone(),
                opencad_geometry::RigidTransform::from_translation([0.003, 0.004, 0.005]),
            )
            .expect("geometry edit");
        let (updated, _) = opencad_drawing::resolve_angular_dimension(
            &dim,
            &refs,
            &kernel.discover_body_edges(&moved).expect("moved edges"),
            &kernel,
            &moved,
        )
        .expect("same binding after geometry edit");
        let AngularMeasurement::Points { vertex_model_m, .. } = updated.measurement else {
            panic!("resolved rays")
        };
        for (actual, expected) in vertex_model_m.into_iter().zip([0.003, 0.004, 0.011]) {
            assert!((actual - expected).abs() < 1e-9);
        }
        assert!(
            (updated.measured_angle_rad().expect("angle after edit") - degrees.to_radians()).abs()
                < 1e-10
        );
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
    let body = kernel
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
    let discoveries = kernel.discover_body_edges(&body).expect("edges");
    let circle = discoveries
        .iter()
        .find(|d| d.length_m < 1e-9)
        .expect("circular edge");
    assert!(kernel
        .straight_edge_endpoints(&body, circle.kernel_edge_id)
        .is_err());
}
#[test]
fn angular_native_regen_svg_and_patch_failures_are_atomic() {
    let dir = tempfile::tempdir().expect("temp");
    let path = fixture(dir.path());
    let input = path.to_str().expect("path");
    let text = success(cli(&["regen", input]));
    assert!(
        text.contains("dimension: dim:plate_corner measured_angle_rad: 1.570796"),
        "{text}"
    );
    let before = read_expanded_dir(&path).expect("before");
    let svg = dir.path().join("drawing.svg");
    let output = svg.to_str().expect("path");
    success(cli(&["export", input, output]));
    let first = std::fs::read(&svg).expect("svg");
    success(cli(&["export", input, output]));
    assert_eq!(first, std::fs::read(&svg).expect("svg"));
    let first = String::from_utf8(first).expect("utf8");
    assert!(first.contains("90.00°"));
    assert!(first.contains("class=\"angular-dimension\""));
    assert_eq!(
        before,
        read_expanded_dir(&path).expect("authored refs unchanged")
    );
    for operation in [
        json!({"type":"remove_drawing_view","view_id":"view:front"}),
        json!({"type":"add_angular_dimension","sheet_id":"sheet:a4","dimension":{"id":"dim:missing","view_id":"view:front","edge_refs":["ref:edge:absent","ref:edge:plate_left"],"arc_radius_m":0.01}}),
    ] {
        let request = json!({"jsonrpc":"2.0","id":1,"method":"opencad.patch_apply_document","params":{"path":path,"patch":{"operations":[operation]}}});
        let mut process = Command::new(env!("CARGO_BIN_EXE_musubicad"))
            .arg("agent")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("agent");
        {
            let mut stdin = process.stdin.take().expect("stdin");
            writeln!(stdin, "{request}").expect("write");
        }
        let result = process.wait_with_output().expect("wait");
        assert!(result.status.success());
        let response: Value = serde_json::from_slice(&result.stdout).expect("json");
        assert!(response["error"].is_object(), "{response}");
        assert_eq!(before, read_expanded_dir(&path).expect("no persistence"));
    }
    let mut doc = before.clone();
    let patch:DesignPatch=serde_json::from_value(json!({"operations":[{"type":"remove_drawing_dimension","id":"dim:plate_corner"},{"type":"remove_drawing_view","view_id":"view:front"}]})).expect("patch");
    apply_patch_to_document(&mut doc, &patch).expect("remove annotation then view");
    assert!(doc.drawing.expect("drawing").sheets[0]
        .angular_dimensions
        .is_empty());
}
