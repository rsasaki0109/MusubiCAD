//! Read-only offset planning; application uses the existing verified patch API.
use opencad_ai::{plan_offset, OffsetPlan, OffsetRequest};
use opencad_core::{DocumentKind, OpenCadError, Result};
use opencad_file::{document_design_state, read_ocad};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanOffsetRequest {
    pub path: String,
    pub edit: OffsetRequest,
}

pub fn plan(request: &PlanOffsetRequest) -> Result<OffsetPlan> {
    let doc = read_ocad(&request.path)?;
    if doc.metadata.kind != DocumentKind::Part {
        return Err(OpenCadError::validation(
            "offset planning requires a part document",
        ));
    }
    plan_offset(&document_design_state(&doc), &request.edit)
}

pub fn command(args: Vec<String>) -> Result<()> {
    if args.len() != 2 {
        return Err(OpenCadError::validation(
            "usage: musubicad plan-offset <part> <edit.json>",
        ));
    }
    let edit = serde_json::from_slice(
        &std::fs::read(&args[1]).map_err(|e| OpenCadError::Other(e.to_string()))?,
    )?;
    let result = plan(&PlanOffsetRequest {
        path: args[0].clone(),
        edit,
    })?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
