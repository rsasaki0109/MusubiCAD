//! MCAD-P12-002: analytic native circles, drawing files and verified Agent edits.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{
    DimensionId, DocumentId, DocumentKind, DocumentMetadata, Length, TopoRefId, ViewId,
};
use opencad_drawing::{RadialDimension, RadialKind};
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
    let mut metadata = DocumentMetadata::new(DocumentId::new(id).expect("id"), "Radial plate");
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
        "doc:radial_drawing",
        "examples/agent/create_radial_drawing_patch.json",
    )
}
fn part() -> OcadDocument {
    authored(
        DocumentKind::Part,
        "doc:radial_cylinder",
        "examples/agent/create_radial_cylinder_patch.json",
    )
}
#[test]
#[ignore = "generate patch-authored radial drawing and referenced part"]
fn write_radial_drawing_example() {
    let path = root().join("examples/radial_plate_drawing.ocad.d");
    write_expanded_dir(&path, &drawing()).expect("drawing");
    write_expanded_dir(path.join("parts/cylinder.ocad.d"), &part()).expect("part");
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
    write_expanded_dir(path.join("parts/cylinder.ocad.d"), &part()).expect("part");
    path
}
#[test]
fn radial_example_round_trip_and_legacy_migration() {
    let expected = drawing();
    let actual =
        read_expanded_dir(root().join("examples/radial_plate_drawing.ocad.d")).expect("read");
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
        .all(|s| s.radial_dimensions.is_empty()));
    assert_eq!(
        legacy,
        opencad_file::migrate::migrate_to_current(legacy.clone()).expect("migration")
    );
}
fn circle_body(kernel: &OcctGeometryKernel, radius: f64) -> KernelBody {
    let wire = kernel
        .make_wire_from_sketch(&SolvedSketch {
            profile_ref: "circle".into(),
            points: vec![],
            closed: true,
            ellipse: None,
            circle: Some(SolvedCircle {
                center_m: [0.02, 0.03],
                radius_m: radius,
            }),
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
        .expect("cylinder")
}
#[test]
fn native_circle_radius_diameter_volume_and_fresh_transformed_witnesses() {
    let kernel = OcctGeometryKernel::new();
    let mut reference = None;
    for radius in [0.005, 0.01, 0.02] {
        let body = circle_body(&kernel, radius);
        let edges = kernel.discover_body_edges(&body).expect("edges");
        let edge = edges.iter().find(|d| d.length_m < 1e-9).expect("circle");
        let reference = reference
            .get_or_insert_with(|| {
                TopoRef::edge(
                    TopoRefId::new("ref:edge:circle").expect("id"),
                    "feature:plate",
                    &edge.role,
                )
            })
            .clone();
        let mut dim = RadialDimension {
            id: DimensionId::new("dim:radius").expect("id"),
            view_id: ViewId::new("view:front").expect("id"),
            edge_ref: reference.ref_id.clone(),
            kind: RadialKind::Radius,
            leader_direction: [1., 0.],
            leader_extension_m: 0.01,
        };
        let resolved = opencad_drawing::resolve_radial_dimension(
            &dim,
            std::slice::from_ref(&reference),
            &edges,
            &kernel,
            &body,
        )
        .expect("radius");
        assert!((resolved.measured_length_m() - radius).abs() < 1e-10);
        assert!((resolved.circle.center_m[0] - 0.02).abs() < 1e-10);
        assert!((resolved.circle.center_m[1] - 0.03).abs() < 1e-10);
        assert!((resolved.circle.center_m[2] - 0.006).abs() < 1e-10);
        let mass = kernel.mass_properties(&body, 1000.).expect("mass");
        assert!((mass.volume_m3 - std::f64::consts::PI * radius * radius * 0.006).abs() < 1e-11);
        dim.kind = RadialKind::Diameter;
        let diameter = opencad_drawing::resolve_radial_dimension(
            &dim,
            std::slice::from_ref(&reference),
            &edges,
            &kernel,
            &body,
        )
        .expect("diameter");
        assert!((diameter.measured_length_m() - 2. * radius).abs() < 1e-10);
        let moved = kernel
            .transform_body(
                body,
                opencad_geometry::RigidTransform::from_translation([0.003, 0.004, 0.005]),
            )
            .expect("translation");
        let updated = opencad_drawing::resolve_radial_dimension(
            &dim,
            std::slice::from_ref(&reference),
            &kernel.discover_body_edges(&moved).expect("edges"),
            &kernel,
            &moved,
        )
        .expect("fresh circle");
        for (a, b) in updated
            .circle
            .center_m
            .into_iter()
            .zip([0.023, 0.034, 0.011])
        {
            assert!((a - b).abs() < 1e-9);
        }
        assert!((updated.circle.radius_m - radius).abs() < 1e-10);
    }
    let wire = kernel
        .make_wire_from_sketch(&SolvedSketch {
            profile_ref: "rectangle".into(),
            points: vec![[0., 0.], [0.02, 0.], [0.02, 0.01], [0., 0.01]],
            closed: true,
            ellipse: None,
            circle: None,
            segments: vec![],
            placement: None,
        })
        .expect("rect");
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
        .expect("plate");
    for edge in kernel.discover_body_edges(&body).expect("edges") {
        assert!(kernel
            .circular_edge_geometry(&body, edge.kernel_edge_id)
            .is_err());
    }
    assert!(kernel.circular_edge_geometry(&body, u64::MAX).is_err());
    // A semicircular profile has analytic arcs, which must not be treated as full circles.
    let sketch = SolvedSketch {
        profile_ref: "semicircle".into(),
        points: vec![],
        closed: true,
        ellipse: None,
        circle: None,
        segments: vec![
            opencad_geometry::ProfileSegment::Arc {
                start_m: [0.01, 0.],
                mid_m: [0., 0.01],
                end_m: [-0.01, 0.],
            },
            opencad_geometry::ProfileSegment::Line {
                start_m: [-0.01, 0.],
                end_m: [0.01, 0.],
            },
        ],
        placement: None,
    };
    let wire = kernel.make_wire_from_sketch(&sketch).expect("arc wire");
    let arc_body = kernel
        .extrude(
            wire,
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.006),
            },
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.],
        )
        .expect("arc solid");
    let edges = kernel.discover_body_edges(&arc_body).expect("arcs");
    assert!(!edges.is_empty());
    for edge in edges {
        assert!(kernel
            .circular_edge_geometry(&arc_body, edge.kernel_edge_id)
            .is_err());
    }
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
fn native_regen_svg_positive_agent_edit_and_atomic_failures() {
    let dir = tempfile::tempdir().expect("temp");
    let path = fixture(dir.path());
    let input = path.to_str().expect("path");
    let text = success(cli(&["regen", input]));
    assert!(
        text.contains("dimension: dim:radius measured_radial_length_m: 0.01"),
        "{text}"
    );
    assert!(
        text.contains("dimension: dim:diameter measured_radial_length_m: 0.02"),
        "{text}"
    );
    let before = read_expanded_dir(&path).expect("before");
    let svg = dir.path().join("drawing.svg");
    let output = svg.to_str().expect("path");
    success(cli(&["export", input, output]));
    let first = std::fs::read(&svg).expect("svg");
    success(cli(&["export", input, output]));
    assert_eq!(first, std::fs::read(&svg).expect("repeat"));
    let first = String::from_utf8(first).expect("utf8");
    assert!(first.contains("R 10.00 mm"));
    assert!(first.contains("⌀ 20.00 mm"));
    assert!(first.contains("class=\"radial-dimension\""));
    assert_eq!(before, read_expanded_dir(&path).expect("refs unchanged"));
    for operations in [
        json!([{"type":"remove_drawing_view","view_id":"view:front"}]),
        json!([{"type":"add_radial_dimension","sheet_id":"sheet:a4","dimension":{"id":"dim:missing","view_id":"view:front","edge_ref":"ref:edge:missing","kind":"radius","leader_direction":[1.,0.],"leader_extension_m":0.01}}]),
        json!([{"type":"set_drawing_view_scale","view_id":"view:front","scale":0.}]),
        json!([{"type":"add_angular_dimension","sheet_id":"sheet:a4","dimension":{"id":"dim:radius","view_id":"view:front","edge_refs":["ref:edge:a","ref:edge:b"],"arc_radius_m":0.01}}]),
    ] {
        let result = agent(&path, operations);
        assert!(result["error"].is_object(), "{result}");
        assert_eq!(before, read_expanded_dir(&path).expect("atomic"));
    }
    let mut edited = serde_json::to_value(
        before.drawing.as_ref().expect("drawing").sheets[0]
            .radial_dimensions
            .iter()
            .find(|d| d.kind == RadialKind::Diameter)
            .expect("diameter"),
    )
    .expect("json");
    edited["id"] = json!("dim:edited");
    edited["leader_extension_m"] = json!(0.02);
    let result = agent(
        &path,
        json!([{"type":"remove_drawing_dimension","id":"dim:diameter"},{"type":"add_radial_dimension","sheet_id":"sheet:a4","dimension":edited}]),
    );
    assert!(result["error"].is_null(), "{result}");
    let after = read_expanded_dir(&path).expect("after");
    assert!(after.drawing.as_ref().expect("drawing").sheets[0]
        .radial_dimensions
        .iter()
        .any(|d| d.id.as_str() == "dim:edited"));
    let text = success(cli(&["regen", input]));
    assert!(
        text.contains("dimension: dim:edited measured_radial_length_m: 0.02"),
        "{text}"
    );
}
