use std::fs;

use opencad_ai::{rebase_patch, semantic_three_way_merge, DesignState};
use opencad_core::{OpenCadError, Result};
use opencad_file::{document_design_state, read_ocad, write_ocad, OcadDocument};

fn state(doc: &OcadDocument) -> DesignState {
    document_design_state(doc)
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
    let result = semantic_three_way_merge(&state(&base), &state(&ours), &state(&theirs));
    if !result.conflicts.is_empty() {
        println!("{}", serde_json::to_string_pretty(&result.conflicts)?);
        return Err(OpenCadError::validation(format!(
            "semantic merge has {} conflict(s)",
            result.conflicts.len()
        )));
    }
    let merged = result
        .merged
        .ok_or_else(|| OpenCadError::Other("missing merged state".into()))?;
    let ours_state = state(&ours);
    let mut output = ours;
    // The persisted Feature Graph is re-derived only when its inputs changed,
    // so merges that leave features alone never rewrite its edge order.
    if merged.feature_nodes != ours_state.feature_nodes
        || merged.feature_order != ours_state.feature_order
        || merged.semantic_refs != ours_state.semantic_refs
        || merged.sketches != ours_state.sketches
    {
        output.feature_graph = merged.derive_feature_graph()?;
    }
    output.parameters = merged.parameters;
    output.feature_nodes = merged.feature_nodes;
    output.semantic_refs = merged.semantic_refs;
    output.sketches = merged.sketches;
    output.assertions = merged.assertions;
    output.assembly = merged.assembly;
    output.drawing = merged.drawing;
    write_ocad(&args[3], &output)?;
    println!("merged: {}", args[3]);
    Ok(())
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
