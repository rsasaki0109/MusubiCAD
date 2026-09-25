//! ADR-013 slice 6: structural assembly and drawing operations.

use std::path::PathBuf;

use opencad_ai::{build_patch_candidate, rebase_patch, ConflictKind, DesignPatch, DesignState};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_file::expanded_dir::serialize_document_files;
use opencad_file::{
    apply_patch_to_document, document_design_state, dry_run_patch_document, read_ocad,
    write_expanded_dir, OcadDocument,
};
use opencad_graph::SemanticChange;
use serde_json::json;

fn example(name: &str) -> OcadDocument {
    read_ocad(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .join(name),
    )
    .expect("read example")
}

fn patch(operations: serde_json::Value) -> DesignPatch {
    serde_json::from_value(json!({ "operations": operations })).expect("patch json")
}

fn candidate(
    doc: &OcadDocument,
    operations: serde_json::Value,
) -> opencad_core::Result<DesignState> {
    build_patch_candidate(&document_design_state(doc), &patch(operations))
}

fn rejection(doc: &OcadDocument, operations: serde_json::Value) -> String {
    candidate(doc, operations)
        .expect_err("patch must be rejected")
        .to_string()
}

fn identity() -> serde_json::Value {
    json!([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
}

fn third_bracket() -> serde_json::Value {
    json!({ "type": "add_instance", "instance": {
        "id": "instance:third",
        "component": "component:bracket",
        "placement": { "transform": { "translation_m": [0.3, 0.0, 0.0], "rotation": identity() } },
        "fixed": false,
        "name": "Third Bracket"
    } })
}

#[test]
fn instances_and_mates_are_added_diffed_and_persisted() {
    let doc = example("assembly_two_brackets.ocad.d");
    let operations = json!([
        third_bracket(),
        { "type": "add_mate", "mate": { "id": "mate:ground_third", "type": "ground",
                                         "instance": "instance:third" } }
    ]);
    let report = dry_run_patch_document(&doc, &patch(operations.clone()));
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert!(report
        .diff
        .changes
        .contains(&SemanticChange::AssemblyInstanceAdded {
            id: "instance:third".into()
        }));
    assert!(report
        .diff
        .changes
        .contains(&SemanticChange::AssemblyMateAdded {
            id: "mate:ground_third".into()
        }));

    let mut edited = doc.clone();
    apply_patch_to_document(&mut edited, &patch(operations)).expect("apply");
    let dir = tempfile::tempdir().expect("tempdir");
    write_expanded_dir(dir.path(), &edited).expect("write");
    let restored = read_ocad(dir.path()).expect("read back");
    assert_eq!(restored.assembly, edited.assembly);
    assert_eq!(restored.assembly.as_ref().unwrap().instances.len(), 3);
}

#[test]
fn assembly_removals_fail_closed_and_list_consumers() {
    let doc = example("assembly_two_brackets.ocad.d");
    let instance = rejection(
        &doc,
        json!([{ "type": "remove_instance", "id": "instance:right" }]),
    );
    assert!(
        instance.contains("cannot remove 'instance:right': still used by mate mate:spacing"),
        "{instance}"
    );
    let component = rejection(
        &doc,
        json!([{ "type": "remove_component", "id": "component:bracket" }]),
    );
    assert!(
        component.contains("still used by instance instance:left, instance instance:right"),
        "{component}"
    );

    // Removing the mate first makes the instance removable.
    let after = candidate(
        &doc,
        json!([
            { "type": "remove_mate", "id": "mate:spacing" },
            { "type": "remove_instance", "id": "instance:right" }
        ]),
    )
    .expect("remove mate then instance");
    assert_eq!(after.assembly.unwrap().instances.len(), 1);

    let robot = example("robot_arm_assembly.ocad.d");
    let connector = rejection(
        &robot,
        json!([{ "type": "remove_connector", "id": "connector:forearm_elbow" }]),
    );
    assert!(
        connector
            .contains("cannot remove 'connector:forearm_elbow': still used by mate mate:elbow"),
        "{connector}"
    );
}

#[test]
fn assembly_references_and_ids_are_validated() {
    let doc = example("assembly_two_brackets.ocad.d");
    let mut orphan = third_bracket();
    orphan["instance"]["component"] = json!("component:missing");
    assert!(rejection(&doc, json!([orphan]))
        .contains("instance 'instance:third' references unknown component 'component:missing'"));

    let mut duplicate = third_bracket();
    duplicate["instance"]["id"] = json!("instance:left");
    assert!(rejection(&doc, json!([duplicate])).contains("already exists"));

    let escaping = rejection(
        &doc,
        json!([{ "type": "add_component", "component": {
            "id": "component:outside", "source_path": "../outside.ocad.d", "source_doc": "doc:outside" } }]),
    );
    assert!(
        escaping.contains("contains a forbidden path component"),
        "{escaping}"
    );

    // Self-reference needs the document ID, so the file layer rejects it.
    let mut edited = doc.clone();
    let error = apply_patch_to_document(
        &mut edited,
        &patch(json!([{ "type": "add_component", "component": {
            "id": "component:self", "source_path": "parts/self.ocad.d",
            "source_doc": "doc:assembly_two_brackets" } }])),
    )
    .expect_err("self reference");
    assert!(
        error
            .to_string()
            .contains("references the assembly document itself"),
        "{error}"
    );
    assert_eq!(edited, doc);

    let part = example("bracket.ocad.d");
    assert!(rejection(&part, json!([third_bracket()])).contains("requires an assembly model"));
}

#[test]
fn an_assembly_is_authored_from_an_empty_document() {
    let metadata = DocumentMetadata::new_assembly(
        DocumentId::new("doc:new_assembly").expect("id"),
        "New Assembly",
    );
    let mut doc = OcadDocument::new(metadata);
    apply_patch_to_document(
        &mut doc,
        &patch(json!([
            { "type": "add_component", "component": {
                "id": "component:bracket", "source_path": "parts/bracket.ocad.d",
                "source_doc": "doc:bracket_001" } },
            third_bracket(),
            { "type": "add_mate", "mate": { "id": "mate:ground", "type": "ground",
                                             "instance": "instance:third" } }
        ])),
    )
    .expect("author assembly");
    let assembly = doc.assembly.as_ref().expect("assembly");
    assert_eq!(assembly.components.len(), 1);
    assert_eq!(assembly.instances.len(), 1);
    assert_eq!(assembly.mates.len(), 1);
}

#[test]
fn drawing_sheets_views_and_dimensions_are_structural() {
    let doc = example("bracket_front_view.ocad.d");
    let view = json!({
        "id": "view:top", "name": "Top",
        "model": { "source_path": "parts/bracket.ocad.d", "source_doc": "doc:bracket_001" },
        "projection": "top", "scale": 1.0, "origin_on_sheet_m": [0.05, 0.15]
    });
    let operations = json!([
        { "type": "add_sheet", "sheet": { "id": "sheet:details", "name": "Details",
                                          "width_m": 0.297, "height_m": 0.21, "views": [] } },
        { "type": "add_drawing_view", "sheet_id": "sheet:details", "view": view },
        { "type": "add_drawing_dimension", "sheet_id": "sheet:details", "dimension": {
            "id": "dim:top_depth", "view_id": "view:top",
            "start_model_m": [0.0, 0.0, 0.0], "end_model_m": [0.0, 0.06, 0.0], "offset_m": 0.01 } }
    ]);
    let report = dry_run_patch_document(&doc, &patch(operations.clone()));
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert!(report
        .diff
        .changes
        .contains(&SemanticChange::DrawingSheetAdded {
            id: "sheet:details".into()
        }));
    let mut edited = doc.clone();
    apply_patch_to_document(&mut edited, &patch(operations)).expect("apply");
    assert_eq!(edited.drawing.as_ref().unwrap().sheets.len(), 2);

    let used_view = rejection(
        &doc,
        json!([{ "type": "remove_drawing_view", "view_id": "view:front" }]),
    );
    assert!(
        used_view.contains("cannot remove 'view:front': still used by dimension dim:overall_width"),
        "{used_view}"
    );
    let cross_sheet = rejection(
        &doc,
        json!([
            { "type": "add_sheet", "sheet": { "id": "sheet:other", "name": "Other",
                                              "width_m": 0.21, "height_m": 0.297, "views": [] } },
            { "type": "add_drawing_dimension", "sheet_id": "sheet:other", "dimension": {
                "id": "dim:stray", "view_id": "view:front",
                "start_model_m": [0.0, 0.0, 0.0], "end_model_m": [0.01, 0.0, 0.0], "offset_m": 0.01 } }
        ]),
    );
    assert!(
        cross_sheet.contains("outside sheet 'sheet:other'"),
        "{cross_sheet}"
    );

    let populated = rejection(
        &doc,
        json!([{ "type": "add_sheet", "sheet": { "id": "sheet:full", "name": "Full",
            "width_m": 0.21, "height_m": 0.297, "views": [view] } }]),
    );
    assert!(populated.contains("must be added empty"));

    let mut bad_scale = view.clone();
    bad_scale["id"] = json!("view:bad");
    bad_scale["scale"] = json!(0.0);
    assert!(rejection(
        &doc,
        json!([{ "type": "add_drawing_view", "sheet_id": "sheet:a4", "view": bad_scale }])
    )
    .contains("scale must be finite and > 0"));

    // A sheet owns its views and dimensions: removing it removes them too.
    let after = candidate(&doc, json!([{ "type": "remove_sheet", "id": "sheet:a4" }]))
        .expect("remove sheet");
    assert!(after.drawing.unwrap().sheets.is_empty());
}

#[test]
fn rebase_detects_concurrent_structural_model_edits() {
    let doc = example("assembly_two_brackets.ocad.d");
    let base = document_design_state(&doc);
    let ours = patch(json!([third_bracket()]));
    let mut theirs_instance = third_bracket();
    theirs_instance["instance"]["name"] = json!("Another Third");
    let theirs = candidate(&doc, json!([theirs_instance])).expect("theirs");
    let conflicts = rebase_patch(&ours, &base, &theirs).expect_err("conflict");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].kind, ConflictKind::Assembly);
    assert_eq!(conflicts[0].id, "instance:third");
}

#[test]
fn failed_model_patches_leave_the_document_unchanged() {
    let mut doc = example("assembly_two_brackets.ocad.d");
    let snapshot = serialize_document_files(&doc).expect("serialize");
    let failing = patch(json!([
        third_bracket(),
        { "type": "remove_component", "id": "component:bracket" }
    ]));
    assert!(apply_patch_to_document(&mut doc, &failing).is_err());
    assert_eq!(serialize_document_files(&doc).expect("serialize"), snapshot);
}
