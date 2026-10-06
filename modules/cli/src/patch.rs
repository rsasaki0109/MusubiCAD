//! `musubicad patch` command (Task-126+).

use std::fs;
use std::path::Path;

use opencad_ai::ensure_patch_valid;
use opencad_core::Result;
use opencad_file::{
    apply_patch_to_document, dry_run_patch_document, read_ocad, write_ocad, OcadDocument,
};

use crate::diff::{self, DiffOptions};

/// Options for `musubicad patch`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PatchOptions {
    pub dry_run: bool,
    pub json: bool,
    pub geometry: bool,
}

/// Parsed CLI arguments for `musubicad patch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchArgs {
    pub doc_path: String,
    pub patch_path: String,
    pub options: PatchOptions,
}

/// Read a DesignPatch JSON file from disk.
pub fn read_patch_file(patch_path: &str) -> Result<opencad_ai::DesignPatch> {
    let patch_json = fs::read_to_string(patch_path).map_err(|err| {
        opencad_core::OpenCadError::Other(format!(
            "failed to read patch '{}': {err}",
            Path::new(patch_path).display()
        ))
    })?;
    serde_json::from_str(&patch_json)
        .map_err(|err| opencad_core::OpenCadError::validation(format!("invalid patch JSON: {err}")))
}

/// Apply a DesignPatch JSON file to a document in memory.
pub fn apply_patch_file(doc: &mut OcadDocument, patch_path: &str) -> Result<()> {
    let patch = read_patch_file(patch_path)?;
    apply_patch_to_document(doc, &patch)
}

/// Apply or dry-run a patch against a `.ocad` document.
pub fn patch_document_with_options(args: &PatchArgs) -> Result<()> {
    let before = read_ocad(&args.doc_path)?;
    let patch = read_patch_file(&args.patch_path)?;
    let report = dry_run_patch_document(&before, &patch);
    ensure_patch_valid(&report)?;

    if args.options.dry_run {
        if args.options.json && !args.options.geometry {
            println!("{}", serde_json::to_string_pretty(&report)?);
            return Ok(());
        }
        println!(
            "impact: {} directly affected, {} predicted dirty",
            report.impact.directly_affected_nodes.len(),
            report.impact.predicted_dirty_nodes.len()
        );
        let mut diff = report.diff;
        if args.options.geometry {
            diff = diff::diff_patch_on_document(
                &before,
                &args.patch_path,
                DiffOptions {
                    json: false,
                    geometry: true,
                },
            )?;
        }
        println!("dry-run: ok");
        diff::print_diff(
            &diff,
            DiffOptions {
                json: args.options.json,
                geometry: args.options.geometry,
            },
        )?;
        return Ok(());
    }

    let mut doc = before;
    apply_patch_to_document(&mut doc, &patch)?;
    write_ocad(&args.doc_path, &doc)?;
    println!("patched: {}", args.doc_path);
    Ok(())
}

pub fn parse_patch_args<I>(args: I) -> Result<PatchArgs>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let mut positional = Vec::new();
    let mut dry_run = false;
    let mut json = false;
    let mut geometry = false;

    for arg in args {
        match arg.as_ref() {
            "--dry-run" => dry_run = true,
            "--json" => json = true,
            "--geometry" => geometry = true,
            value => positional.push(value.to_string()),
        }
    }

    let doc_path = positional.first().cloned().ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: musubicad patch <document> <patch.json> [--dry-run] [--json] [--geometry]",
        )
    })?;
    let patch_path = positional.get(1).cloned().ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: musubicad patch <document> <patch.json> [--dry-run] [--json] [--geometry]",
        )
    })?;

    Ok(PatchArgs {
        doc_path,
        patch_path,
        options: PatchOptions {
            dry_run,
            json,
            geometry,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_ai::DesignPatch;
    use opencad_core::{DocumentId, DocumentMetadata};
    use opencad_feature::bracket_with_hole;
    use opencad_file::{read_ocad, OcadDocument};
    use opencad_graph::{bracket_parameters, evaluate_param_graph};
    use tempfile::tempdir;

    #[test]
    fn patch_updates_parameter_and_persists() {
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();

        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        opencad_file::write_expanded_dir(&doc_path, &doc).expect("write");

        let patch_path = dir.path().join("width.patch.json");
        let patch = DesignPatch::set_parameter("param:width", "100 mm");
        fs::write(&patch_path, serde_json::to_string(&patch).expect("json")).expect("patch");

        let args = PatchArgs {
            doc_path: doc_path.to_str().expect("path").to_string(),
            patch_path: patch_path.to_str().expect("patch").to_string(),
            options: PatchOptions::default(),
        };
        patch_document_with_options(&args).expect("patch");

        let restored = read_ocad(&doc_path).expect("read");
        let values = evaluate_param_graph(&restored.parameters).expect("eval");
        assert!((values["width"] - 0.1).abs() < 1e-9);
    }

    #[test]
    fn dry_run_does_not_modify_document() {
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();

        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        opencad_file::write_expanded_dir(&doc_path, &doc).expect("write");

        let patch_path = dir.path().join("width.patch.json");
        fs::write(
            &patch_path,
            r#"{"operations":[{"type":"set_parameter","id":"param:width","expr":"100 mm"}]}"#,
        )
        .expect("patch");

        let args = PatchArgs {
            doc_path: doc_path.to_str().expect("path").to_string(),
            patch_path: patch_path.to_str().expect("patch").to_string(),
            options: PatchOptions {
                dry_run: true,
                json: false,
                geometry: false,
            },
        };
        patch_document_with_options(&args).expect("dry-run");

        let restored = read_ocad(&doc_path).expect("read");
        let values = evaluate_param_graph(&restored.parameters).expect("eval");
        assert!((values["width"] - 0.08).abs() < 1e-9);
    }

    #[test]
    fn dry_run_rejects_invalid_patch() {
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();

        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        opencad_file::write_expanded_dir(&doc_path, &doc).expect("write");

        let patch_path = dir.path().join("bad.patch.json");
        fs::write(
            &patch_path,
            r#"{"operations":[{"type":"set_parameter","id":"param:missing","expr":"10 mm"}]}"#,
        )
        .expect("patch");

        let args = PatchArgs {
            doc_path: doc_path.to_str().expect("path").to_string(),
            patch_path: patch_path.to_str().expect("patch").to_string(),
            options: PatchOptions {
                dry_run: true,
                json: false,
                geometry: false,
            },
        };
        patch_document_with_options(&args).expect_err("invalid patch");
    }

    #[test]
    fn patch_applies_feature_expr_operation() {
        let part = bracket_with_hole().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Hole",
        );
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();

        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        opencad_file::write_expanded_dir(&doc_path, &doc).expect("write");

        let patch_path = dir.path().join("extrude.patch.json");
        fs::write(
            &patch_path,
            r#"{"operations":[{"type":"set_feature_expr","feature_id":"feature:extrude_base","field":"length_expr","expr":"thickness * 2"}]}"#,
        )
        .expect("patch");

        let args = PatchArgs {
            doc_path: doc_path.to_str().expect("path").to_string(),
            patch_path: patch_path.to_str().expect("patch").to_string(),
            options: PatchOptions::default(),
        };
        patch_document_with_options(&args).expect("patch");

        let restored = read_ocad(&doc_path).expect("read");
        let node = restored
            .feature_nodes
            .iter()
            .find(|node| node.id == "feature:extrude_base")
            .expect("extrude");
        let opencad_feature::FeatureDefinition::Extrude(extrude) = &node.definition else {
            panic!("expected extrude");
        };
        assert_eq!(extrude.length_expr.as_deref(), Some("thickness * 2"));
    }

    #[test]
    fn parse_patch_args_reads_flags() {
        let args = parse_patch_args([
            "bracket.ocad.d",
            "width.patch.json",
            "--dry-run",
            "--geometry",
        ])
        .expect("parse");
        assert!(args.options.dry_run);
        assert!(args.options.geometry);
        assert!(!args.options.json);
    }
}
