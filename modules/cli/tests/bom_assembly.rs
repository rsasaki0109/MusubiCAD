//! MCAD-P12-008 native counts, nested failures, transport and protected output.
use opencad_ai::authoring_patch;
use opencad_assembly::{
    AssemblyModel, AssemblyPattern, Component, ComponentSourceKind, Instance, Placement,
};
use opencad_core::{ComponentId, DocumentId, DocumentMetadata, InstanceId, PatternId};
use opencad_file::{
    apply_patch_to_document, document_design_state, read_ocad, write_expanded_dir, write_ocad,
    OcadDocument,
};
use opencad_geometry::RigidTransform;
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
fn success(o: std::process::Output) -> String {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).expect("utf8")
}
fn part_component(name: &str, path: &str, doc: &str) -> Component {
    Component::new(
        ComponentId::new(name).expect("id"),
        path,
        DocumentId::new(doc).expect("id"),
    )
}
fn instance(name: &str, component: &str, x: f64) -> Instance {
    let mut i = Instance::new(
        InstanceId::new(name).expect("id"),
        ComponentId::new(component).expect("id"),
        Placement::new(RigidTransform::from_translation([x, 0., 0.])),
        name,
    );
    i.fixed = true;
    i
}
fn assembly(doc: &str, name: &str, model: AssemblyModel) -> OcadDocument {
    let mut source = OcadDocument::new(DocumentMetadata::new_assembly(
        DocumentId::new(doc).expect("id"),
        name,
    ));
    source.assembly = Some(model);
    let mut result = OcadDocument::new(source.metadata.clone());
    apply_patch_to_document(
        &mut result,
        &authoring_patch(&document_design_state(&source)).expect("authoring"),
    )
    .expect("DesignPatch");
    result
}
fn fixture(path: &Path) -> (OcadDocument, OcadDocument) {
    let mut sub = AssemblyModel::new();
    sub.components.push(part_component(
        "component:plate",
        "plate.ocad.d",
        "doc:title_plate",
    ));
    sub.instances
        .push(instance("instance:fixed", "component:plate", 0.));
    sub.patterns.push(AssemblyPattern {
        id: PatternId::new("pattern:row").expect("id"),
        component: ComponentId::new("component:plate").expect("id"),
        count: 2,
        spacing_m: 0.1,
        direction_m: [1., 0., 0.],
        base_placement: Placement::new(RigidTransform::from_translation([0.1, 0., 0.])),
        name_prefix: "copy".into(),
    });
    let sub = assembly("doc:bom_sub", "Patterned plates", sub);
    let mut model = AssemblyModel::new();
    let mut component = part_component("component:sub", "parts/sub.ocad", "doc:bom_sub");
    component.source_kind = ComponentSourceKind::Assembly;
    model.components = vec![
        component,
        part_component("component:plate", "parts/plate.ocad.d", "doc:title_plate"),
    ];
    model.instances = vec![
        instance("instance:left", "component:sub", 0.),
        instance("instance:right", "component:sub", 0.4),
        instance("instance:direct", "component:plate", 0.8),
    ];
    let doc = assembly("doc:bom_nested", "Nested plate bill of materials", model);
    write_expanded_dir(path, &doc).expect("write root");
    std::fs::create_dir_all(path.join("parts")).expect("parts");
    write_ocad(path.join("parts/sub.ocad"), &sub).expect("packed sub");
    let part = read_ocad(root().join("examples/title_plate_drawing.ocad.d/parts/plate.ocad.d"))
        .expect("plate");
    write_expanded_dir(path.join("parts/plate.ocad.d"), &part).expect("plate write");
    (doc, sub)
}
fn request(method: &str, params: Value, mcp: bool) -> Value {
    let (mode, message) = if mcp {
        (
            "mcp",
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":method,"arguments":params}}),
        )
    } else {
        (
            "agent",
            json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}),
        )
    };
    let mut p = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("transport");
    writeln!(p.stdin.take().expect("stdin"), "{message}").expect("request");
    let o = p.wait_with_output().expect("wait");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice(&o.stdout).expect("response")
}
#[test]
fn native_nested_pattern_counts_units_transports_and_determinism() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("nested.ocad.d");
    let (before, sub) = fixture(&path);
    let output = dir.path().join("bom.json");
    let args = [
        "bom",
        path.to_str().expect("path"),
        "--output",
        output.to_str().expect("path"),
    ];
    let text = success(cli(&args));
    let bom: Value = serde_json::from_str(&text).expect("BOM");
    assert_eq!(bom["declared_component_count"], 2);
    assert_eq!(bom["top_level_instance_count"], 3);
    assert_eq!(bom["leaf_part_count"], 1);
    assert_eq!(bom["leaf_instance_count"], 7);
    assert_eq!(bom["rows"][0]["quantity"], 7);
    assert_eq!(bom["rows"][0]["source_path"], "parts/plate.ocad.d");
    assert_eq!(
        bom["rows"][0]["instance_paths"]
            .as_array()
            .expect("paths")
            .len(),
        7
    );
    let first = std::fs::read(&output).expect("JSON");
    success(cli(&args));
    assert_eq!(first, std::fs::read(&output).expect("deterministic bytes"));
    let agent = request("opencad.bom_document", json!({"path":path}), false);
    assert!(agent["error"].is_null(), "{agent}");
    assert_eq!(agent["result"], bom);
    let mcp = request("bom_document", json!({"path":path}), true);
    assert_eq!(mcp["result"]["isError"], false, "{mcp}");
    let mcp_bom: Value =
        serde_json::from_str(mcp["result"]["content"][0]["text"].as_str().expect("text"))
            .expect("BOM text");
    assert_eq!(mcp_bom, bom);
    let csv = dir.path().join("bom.csv");
    success(cli(&[
        "bom",
        path.to_str().expect("path"),
        "--output",
        csv.to_str().expect("path"),
    ]));
    let csv = std::fs::read_to_string(csv).expect("CSV");
    assert!(csv.starts_with("source_doc,name,source_path,quantity_count\r\n"));
    assert!(csv.ends_with(",7\r\n"));
    let regen = success(cli(&["regen", path.to_str().expect("path")]));
    let volume: f64 = regen
        .lines()
        .find_map(|line| line.strip_prefix("volume_m3:"))
        .expect("volume")
        .trim()
        .parse()
        .expect("number");
    assert!((volume - 7. * 0.08 * 0.06 * 0.006).abs() < 1e-12);
    assert_eq!(before, read_ocad(&path).expect("unchanged root"));
    assert_eq!(
        sub,
        read_ocad(path.join("parts/sub.ocad")).expect("unchanged child")
    );
}
#[test]
fn partial_nested_geometry_missing_identity_and_cycles_refuse_prior_output() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("nested.ocad.d");
    let (_, mut sub) = fixture(&path);
    let empty = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:empty").expect("id"),
        "Empty part",
    ));
    write_ocad(path.join("parts/empty.ocad"), &empty).expect("empty");
    let model = sub.assembly.as_mut().expect("model");
    model
        .components
        .push(part_component("component:empty", "empty.ocad", "doc:empty"));
    model
        .instances
        .push(instance("instance:empty", "component:empty", 0.3));
    write_ocad(path.join("parts/sub.ocad"), &sub).expect("sub");
    // Localized regeneration can return a root compound despite an empty nested
    // part; the BOM must verify the nested source separately and refuse it.
    success(cli(&["regen", path.to_str().expect("path")]));
    let output = dir.path().join("prior.json");
    std::fs::write(&output, b"previous BOM").expect("sentinel");
    for failure in 0..4 {
        let (doc, mut sub) = if failure == 0 {
            (read_ocad(&path).expect("root"), sub.clone())
        } else {
            fixture(&path)
        };
        match failure {
            1 => sub.metadata.id = DocumentId::new("doc:wrong").expect("id"),
            2 => {
                sub.assembly.as_mut().expect("model").components[0].source_path =
                    "missing.ocad.d".into()
            }
            3 => {
                let mut component = part_component("component:cycle", "sub.ocad", "doc:bom_sub");
                component.source_kind = ComponentSourceKind::Assembly;
                let model = sub.assembly.as_mut().expect("model");
                model.components.push(component);
                model
                    .instances
                    .push(instance("instance:cycle", "component:cycle", 0.));
            }
            _ => {}
        }
        write_ocad(path.join("parts/sub.ocad"), &sub).expect("write");
        let result = cli(&[
            "bom",
            path.to_str().expect("path"),
            "--output",
            output.to_str().expect("path"),
        ]);
        assert!(!result.status.success());
        assert_eq!(std::fs::read(&output).expect("preserved"), b"previous BOM");
        assert_eq!(doc, read_ocad(&path).expect("root unchanged"));
        assert_eq!(
            sub,
            read_ocad(path.join("parts/sub.ocad")).expect("child unchanged")
        );
    }
}
#[test]
fn output_inside_source_is_refused_and_atomic_replace_preserves_hardlinks() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("nested.ocad.d");
    let (doc, sub) = fixture(&path);
    let metadata = path.join("document.ocad.json");
    let before = std::fs::read(&metadata).expect("metadata");
    assert!(!cli(&[
        "bom",
        path.to_str().expect("path"),
        "--output",
        metadata.to_str().expect("path")
    ])
    .status
    .success());
    assert_eq!(before, std::fs::read(&metadata).expect("preserved"));
    let output = dir.path().join("linked.json");
    let unrelated = dir.path().join("unrelated.ocad.d");
    let unrelated_doc = read_ocad(path.join("parts/plate.ocad.d")).expect("plate");
    write_expanded_dir(&unrelated, &unrelated_doc).expect("other model");
    assert!(!cli(&[
        "bom",
        path.to_str().expect("path"),
        "--output",
        unrelated.join("document.ocad.json").to_str().expect("path")
    ])
    .status
    .success());
    assert_eq!(
        unrelated_doc,
        read_ocad(&unrelated).expect("other model preserved")
    );
    std::fs::hard_link(&metadata, &output).expect("hard link");
    success(cli(&[
        "bom",
        path.to_str().expect("path"),
        "--output",
        output.to_str().expect("path"),
    ]));
    assert_eq!(
        before,
        std::fs::read(&metadata).expect("input hardlink preserved")
    );
    assert!(std::fs::read_to_string(&output)
        .expect("output")
        .contains("leaf_instance_count"));
    assert_eq!(doc, read_ocad(&path).expect("root unchanged"));
    assert_eq!(
        sub,
        read_ocad(path.join("parts/sub.ocad")).expect("child unchanged")
    );
    assert!(std::fs::read_dir(dir.path()).expect("directory").all(|e| !e
        .expect("entry")
        .file_name()
        .to_string_lossy()
        .starts_with(".tmp")));
}
#[test]
#[ignore = "generate patch-authored nested BOM example"]
fn write_bom_example() {
    let path = root().join("examples/bom_nested_assembly.ocad.d");
    let (doc, sub) = fixture(&path);
    for (name, doc) in [
        ("create_bom_assembly_patch.json", doc),
        ("create_bom_subassembly_patch.json", sub),
    ] {
        let patch = authoring_patch(&document_design_state(&doc)).expect("patch");
        std::fs::write(
            root().join("examples/agent").join(name),
            serde_json::to_string_pretty(&patch).expect("json") + "\n",
        )
        .expect("patch");
    }
    success(cli(&[
        "bom",
        path.to_str().expect("path"),
        "--output",
        root()
            .join("examples/bom_nested_assembly.bom.json")
            .to_str()
            .expect("path"),
    ]));
    success(cli(&[
        "bom",
        path.to_str().expect("path"),
        "--output",
        root()
            .join("examples/bom_nested_assembly.bom.csv")
            .to_str()
            .expect("path"),
    ]));
}
