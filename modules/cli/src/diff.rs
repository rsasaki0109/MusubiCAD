//! `opencad diff` command (Task-127+).

use opencad_core::Result;
use opencad_file::{diff_documents, read_ocad, OcadDocument};
use opencad_graph::{format_mass_kg, DesignDiff, GeometricDiff, SemanticChange};

use crate::patch;
use crate::regen;

/// Options controlling diff output and geometry enrichment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffOptions {
    pub json: bool,
    pub geometry: bool,
}

/// Parsed CLI arguments for `opencad diff`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffArgs {
    pub before_path: String,
    pub after_path: Option<String>,
    pub patch_path: Option<String>,
    pub options: DiffOptions,
}

/// Compare two in-memory documents, optionally enriching with geometry deltas.
pub fn build_document_diff(
    before: &OcadDocument,
    after: &OcadDocument,
    options: DiffOptions,
) -> Result<DesignDiff> {
    let mut diff = diff_documents(before, after);
    if options.geometry {
        enrich_geometry(&mut diff, before, after)?;
    }
    Ok(diff)
}

/// Apply a patch to a document and optionally enrich the diff with geometry.
pub fn diff_patch_on_document(
    before: &OcadDocument,
    patch_path: &str,
    options: DiffOptions,
) -> Result<DesignDiff> {
    let mut after = before.clone();
    patch::apply_patch_file(&mut after, patch_path)?;
    build_document_diff(before, &after, options)
}

/// Compare two documents or a document against a patch preview.
pub fn diff_documents_at_paths(args: &DiffArgs) -> Result<DesignDiff> {
    let before = read_ocad(&args.before_path)?;
    let after = match (&args.after_path, &args.patch_path) {
        (Some(after_path), None) => read_ocad(after_path)?,
        (None, Some(patch_path)) => {
            let mut after = before.clone();
            patch::apply_patch_file(&mut after, patch_path)?;
            after
        }
        _ => {
            return Err(opencad_core::OpenCadError::validation(
                "usage: opencad diff <before> <after> | opencad diff <doc> --patch <patch.json>",
            ));
        }
    };

    build_document_diff(&before, &after, args.options)
}

/// Regenerate a document and return mass/volume summary.
pub fn regen_document_summary(doc: &OcadDocument) -> Result<regen::RegenSummary> {
    let mut model = doc.clone().into_part_model();
    regen::regenerate_part(&mut model, Some(&doc.parameters), Some(&doc.semantic_refs))
}

fn enrich_geometry(
    diff: &mut DesignDiff,
    before: &OcadDocument,
    after: &OcadDocument,
) -> Result<()> {
    let before_summary = regen_document_summary(before)?;
    let after_summary = regen_document_summary(after)?;

    let geometry = GeometricDiff {
        volume_before: before_summary.volume_m3,
        volume_after: after_summary.volume_m3,
        mass_before: before_summary.mass_kg,
        mass_after: after_summary.mass_kg,
    };

    if let (Some(before_mass), Some(after_mass)) = (geometry.mass_before, geometry.mass_after) {
        if (before_mass - after_mass).abs() > 1e-12 {
            diff.changes.push(SemanticChange::MassChanged {
                before: format_mass_kg(before_mass),
                after: format_mass_kg(after_mass),
            });
            if diff.summary == "No changes" {
                diff.summary = format!(
                    "mass changed from {} to {}",
                    format_mass_kg(before_mass),
                    format_mass_kg(after_mass)
                );
            }
        }
    }

    diff.geometry = Some(geometry);
    Ok(())
}

pub fn print_diff(diff: &DesignDiff, options: DiffOptions) -> Result<()> {
    if options.json {
        println!("{}", serde_json::to_string_pretty(diff)?);
        return Ok(());
    }

    println!("summary: {}", diff.summary);
    if diff.changes.is_empty() {
        println!("changes: none");
    } else {
        println!("changes:");
        for change in &diff.changes {
            print_change(change);
        }
    }

    if let Some(geometry) = &diff.geometry {
        println!("geometry:");
        if let (Some(before), Some(after)) = (geometry.volume_before, geometry.volume_after) {
            println!("  volume_m3: {before} -> {after}");
        }
        if let (Some(before), Some(after)) = (geometry.mass_before, geometry.mass_after) {
            println!("  mass: {before} -> {after}");
        }
    }
    Ok(())
}

fn print_change(change: &SemanticChange) {
    match change {
        SemanticChange::ParameterChanged { id, before, after } => {
            println!("  parameter {id}: {before} -> {after}");
        }
        SemanticChange::FeatureAdded { id, feature_type } => {
            println!("  feature added {id} ({feature_type})");
        }
        SemanticChange::FeatureRemoved { id } => {
            println!("  feature removed {id}");
        }
        SemanticChange::FeatureModified {
            id,
            field,
            before,
            after,
        } => {
            println!("  feature {id}.{field}: {before} -> {after}");
        }
        SemanticChange::ConstraintModified { id, before, after } => {
            println!("  constraint {id}: {before} -> {after}");
        }
        SemanticChange::MassChanged { before, after } => {
            println!("  mass: {before} -> {after}");
        }
        SemanticChange::BboxChanged { before, after } => {
            println!("  bbox: {before} -> {after}");
        }
        SemanticChange::TopoRefAdded {
            ref_id,
            created_by,
            role,
        } => {
            let role_suffix = role
                .as_deref()
                .map(|value| format!(", role={value}"))
                .unwrap_or_default();
            println!("  topo ref added {ref_id} ({created_by}{role_suffix})");
        }
        SemanticChange::TopoRefRemoved { ref_id } => {
            println!("  topo ref removed {ref_id}");
        }
        SemanticChange::TopoRefModified {
            ref_id,
            field,
            before,
            after,
        } => {
            println!("  topo ref {ref_id}.{field}: {before} -> {after}");
        }
        SemanticChange::AssemblyInstanceAdded { id } => {
            println!("  assembly instance added {id}");
        }
        SemanticChange::AssemblyInstanceRemoved { id } => {
            println!("  assembly instance removed {id}");
        }
        SemanticChange::AssemblyInstanceChanged {
            id,
            field,
            before,
            after,
        } => {
            println!("  assembly instance {id}.{field}: {before} -> {after}");
        }
        SemanticChange::AssemblyMateAdded { id } => {
            println!("  assembly mate added {id}");
        }
        SemanticChange::AssemblyMateRemoved { id } => {
            println!("  assembly mate removed {id}");
        }
        SemanticChange::AssemblyMateChanged { id, before, after } => {
            println!("  assembly mate {id}: {before} -> {after}");
        }
        SemanticChange::AssemblyConnectorAdded { id } => {
            println!("  assembly connector added {id}");
        }
        SemanticChange::AssemblyConnectorRemoved { id } => {
            println!("  assembly connector removed {id}");
        }
        SemanticChange::AssemblyConnectorChanged { id, before, after } => {
            println!("  assembly connector {id}: {before} -> {after}");
        }
        SemanticChange::DrawingSheetAdded { id } => println!("  drawing sheet added {id}"),
        SemanticChange::DrawingSheetRemoved { id } => println!("  drawing sheet removed {id}"),
        SemanticChange::DrawingSheetChanged { id, before, after } => {
            println!("  drawing sheet {id}: {before} -> {after}");
        }
        SemanticChange::DrawingViewAdded { id } => println!("  drawing view added {id}"),
        SemanticChange::DrawingViewRemoved { id } => println!("  drawing view removed {id}"),
        SemanticChange::DrawingViewChanged { id, before, after } => {
            println!("  drawing view {id}: {before} -> {after}");
        }
    }
}

pub fn parse_diff_args<I>(args: I) -> Result<DiffArgs>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let mut positional = Vec::new();
    let mut json = false;
    let mut geometry = false;
    let mut patch_path = None;

    let mut iter = args.into_iter().peekable();
    while let Some(arg) = iter.next() {
        match arg.as_ref() {
            "--json" => json = true,
            "--geometry" => geometry = true,
            "--patch" => {
                let value = iter.next().ok_or_else(|| {
                    opencad_core::OpenCadError::validation("missing value for --patch")
                })?;
                patch_path = Some(value.as_ref().to_string());
            }
            value if value.starts_with("--patch=") => {
                patch_path = Some(value.trim_start_matches("--patch=").to_string());
            }
            value => positional.push(value.to_string()),
        }
    }

    let before_path = positional.first().cloned().ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: opencad diff <before> <after> [--json] [--geometry]\n       opencad diff <doc> --patch <patch.json> [--json] [--geometry]",
        )
    })?;

    let after_path = if patch_path.is_some() {
        if positional.len() > 1 {
            return Err(opencad_core::OpenCadError::validation(
                "usage: opencad diff <doc> --patch <patch.json>",
            ));
        }
        None
    } else {
        Some(positional.get(1).cloned().ok_or_else(|| {
            opencad_core::OpenCadError::validation(
                "usage: opencad diff <before> <after> [--json] [--geometry]",
            )
        })?)
    };

    Ok(DiffArgs {
        before_path,
        after_path,
        patch_path,
        options: DiffOptions { json, geometry },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{DocumentId, DocumentMetadata};
    use opencad_feature::bracket_with_hole;
    use opencad_file::{write_expanded_dir, OcadDocument};
    use opencad_graph::{bracket_parameters, SemanticChange};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parse_diff_args_two_documents() {
        let args = parse_diff_args(["before.ocad.d", "after.ocad.d", "--json"]).expect("parse");
        assert_eq!(args.before_path, "before.ocad.d");
        assert_eq!(args.after_path.as_deref(), Some("after.ocad.d"));
        assert!(args.options.json);
    }

    #[test]
    fn parse_diff_args_patch_mode() {
        let args =
            parse_diff_args(["bracket.ocad.d", "--patch", "width.patch.json"]).expect("parse");
        assert_eq!(args.before_path, "bracket.ocad.d");
        assert!(args.after_path.is_none());
        assert_eq!(args.patch_path.as_deref(), Some("width.patch.json"));
    }

    #[test]
    fn diff_with_patch_detects_width_change() {
        let part = bracket_with_hole().expect("model");
        let metadata =
            DocumentMetadata::new(DocumentId::new("doc:bracket_001").expect("id"), "Bracket");
        let mut doc = OcadDocument::from_part_model(metadata, &part);
        doc.parameters = bracket_parameters();

        let dir = tempdir().expect("tempdir");
        let doc_path = dir.path().join("bracket.ocad.d");
        write_expanded_dir(&doc_path, &doc).expect("write");

        let patch_path = dir.path().join("width.patch.json");
        fs::write(
            &patch_path,
            r#"{"operations":[{"type":"set_parameter","id":"param:width","expr":"100 mm"}]}"#,
        )
        .expect("patch");

        let args = DiffArgs {
            before_path: doc_path.to_str().expect("path").to_string(),
            after_path: None,
            patch_path: Some(patch_path.to_str().expect("patch").to_string()),
            options: DiffOptions::default(),
        };
        let diff = diff_documents_at_paths(&args).expect("diff");
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(
            diff.changes[0],
            SemanticChange::ParameterChanged {
                id: "param:width".into(),
                before: "80 mm".into(),
                after: "100 mm".into(),
            }
        );
    }
}
