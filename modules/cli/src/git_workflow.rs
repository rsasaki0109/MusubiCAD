use std::fs;
use std::path::Path;

use opencad_ai::{rebase_patch, semantic_three_way_merge, DesignState, SemanticConflict};
use opencad_core::{OpenCadError, Result};
use opencad_file::{document_design_state, read_ocad, write_ocad, OcadDocument};

fn state(doc: &OcadDocument) -> DesignState {
    document_design_state(doc)
}

/// Semantically merge three documents into a copy of `ours`.
///
/// Returns the typed conflicts instead of a document when the merge cannot
/// be completed.  The persisted Feature Graph is taken verbatim from the side
/// whose Feature Graph inputs equal the merged ones (so a graph only one
/// branch changed keeps that branch's bytes) and re-derived otherwise.
pub fn merge_documents(
    base: &OcadDocument,
    ours: &OcadDocument,
    theirs: &OcadDocument,
) -> Result<std::result::Result<OcadDocument, Vec<SemanticConflict>>> {
    let (ours_state, theirs_state) = (state(ours), state(theirs));
    let result = semantic_three_way_merge(&state(base), &ours_state, &theirs_state);
    if !result.conflicts.is_empty() {
        return Ok(Err(result.conflicts));
    }
    let merged = result
        .merged
        .ok_or_else(|| OpenCadError::Other("missing merged state".into()))?;
    let same_graph_inputs = |side: &DesignState| {
        merged.feature_nodes == side.feature_nodes
            && merged.feature_order == side.feature_order
            && merged.semantic_refs == side.semantic_refs
            && merged.sketches == side.sketches
    };
    let mut output = ours.clone();
    output.feature_graph = if same_graph_inputs(&ours_state) {
        ours.feature_graph.clone()
    } else if same_graph_inputs(&theirs_state) {
        theirs.feature_graph.clone()
    } else {
        merged.derive_feature_graph()?
    };
    output.parameters = merged.parameters;
    output.feature_nodes = merged.feature_nodes;
    output.semantic_refs = merged.semantic_refs;
    output.sketches = merged.sketches;
    output.assertions = merged.assertions;
    output.attachments = merged.attachments;
    output.assembly = merged.assembly;
    output.drawing = merged.drawing;
    Ok(Ok(output))
}

pub fn merge(args: Vec<String>) -> Result<()> {
    if args.len() != 4 {
        return Err(OpenCadError::validation(
            "usage: opencad merge <base> <ours> <theirs> <output>",
        ));
    }
    let base = read_ocad(&args[0])?;
    let ours = read_ocad(&args[1])?;
    let theirs = read_ocad(&args[2])?;
    match merge_documents(&base, &ours, &theirs)? {
        Ok(output) => {
            write_ocad(&args[3], &output)?;
            println!("merged: {}", args[3]);
            Ok(())
        }
        Err(conflicts) => {
            println!("{}", serde_json::to_string_pretty(&conflicts)?);
            Err(OpenCadError::validation(format!(
                "semantic merge has {} conflict(s)",
                conflicts.len()
            )))
        }
    }
}

/// `opencad merge-driver %O %A %B %P` and `opencad merge-driver install`.
pub fn merge_driver(args: Vec<String>) -> Result<()> {
    if args.first().map(String::as_str) == Some("install") {
        let repo = crate::git_driver::repository_root()?;
        let executable = std::env::current_exe()
            .map_err(|error| OpenCadError::Other(format!("cannot locate opencad: {error}")))?;
        let attributes = crate::git_driver::install(&repo, &executable)?;
        println!("Registered merge driver 'musubicad' in {}.", repo.display());
        println!("Add these lines to .gitattributes:");
        for line in attributes {
            println!("{line}");
        }
        return Ok(());
    }
    let [base, current, other, path] = args.as_slice() else {
        return Err(OpenCadError::validation(
            "usage: opencad merge-driver %O %A %B %P   |   opencad merge-driver install",
        ));
    };
    let repo = crate::git_driver::repository_root()?;
    match crate::git_driver::run_merge_driver(
        &repo,
        base.as_ref(),
        current.as_ref(),
        other.as_ref(),
        path,
    )? {
        crate::git_driver::DriverOutcome::Merged => Ok(()),
        crate::git_driver::DriverOutcome::Conflict(reason) => {
            eprintln!("opencad merge-driver: {reason}");
            Err(OpenCadError::validation(format!(
                "'{path}' left conflicted"
            )))
        }
    }
}

/// `path` relative to the repository root with `/` separators; paths that
/// cannot be resolved are returned as given.
fn repository_relative(repo: &Path, path: &Path) -> String {
    let resolved = std::fs::canonicalize(path).ok().and_then(|absolute| {
        let root = std::fs::canonicalize(repo).ok()?;
        absolute.strip_prefix(root).ok().map(Path::to_path_buf)
    });
    resolved
        .unwrap_or_else(|| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

/// `opencad conflicts <doc.ocad.d>`: typed conflicts of an unfinished merge.
pub fn conflicts(args: Vec<String>) -> Result<()> {
    let [document] = args.as_slice() else {
        return Err(OpenCadError::validation(
            "usage: opencad conflicts <document.ocad.d>",
        ));
    };
    let repo = crate::git_driver::repository_root()?;
    let relative = repository_relative(&repo, Path::new(document));
    let conflicts = crate::git_driver::merge_conflicts(&repo, &relative)?;
    println!("{}", serde_json::to_string_pretty(&conflicts)?);
    if conflicts.is_empty() {
        Ok(())
    } else {
        Err(OpenCadError::validation(format!(
            "{} semantic conflict(s) in '{relative}'",
            conflicts.len()
        )))
    }
}

pub fn rebase(args: Vec<String>) -> Result<()> {
    if args.len() != 4 {
        return Err(OpenCadError::validation(
            "usage: opencad rebase-patch <old-base> <new-base> <patch.json> <output.json>",
        ));
    }
    let old_base = read_ocad(&args[0])?;
    let new_base = read_ocad(&args[1])?;
    let patch = crate::patch::read_patch_file(&args[2])?;
    let rebased =
        rebase_patch(&patch, &state(&old_base), &state(&new_base)).map_err(|conflicts| {
            OpenCadError::validation(format!(
                "patch rebase conflicts:\n{}",
                serde_json::to_string_pretty(&conflicts).unwrap_or_default()
            ))
        })?;
    fs::write(&args[3], serde_json::to_vec_pretty(&rebased)?).map_err(|err| {
        OpenCadError::Other(format!(
            "failed to write rebased patch '{}': {err}",
            args[3]
        ))
    })?;
    println!("rebased: {}", args[3]);
    Ok(())
}
