//! Read-only inspection of graph-derived geometry, without saving coordinates.
use opencad_core::{DocumentKind, OpenCadError, Result};
use opencad_feature::{
    projected_sketch::resolve_design_sketch, CurveProjection, FeatureDefinition,
};
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectProjectedSketchRequest {
    pub path: String,
    pub feature_id: String,
}
#[derive(Serialize)]
pub struct ProjectedSketchInspection {
    pub feature_id: String,
    pub derived: bool,
    pub sketch: opencad_sketch::Sketch,
    pub bindings: Vec<CurveProjection>,
}
pub fn inspect(request: &InspectProjectedSketchRequest) -> Result<ProjectedSketchInspection> {
    let doc = opencad_file::read_ocad(&request.path)?;
    if doc.metadata.kind != DocumentKind::Part {
        return Err(OpenCadError::validation(
            "projected sketch inspection requires a part",
        ));
    }
    let node = doc
        .feature_nodes
        .iter()
        .find(|n| n.id == request.feature_id)
        .ok_or_else(|| OpenCadError::not_found("projected sketch feature"))?;
    let FeatureDefinition::ProjectedSketch(def) = &node.definition else {
        return Err(OpenCadError::validation("feature must be projected_sketch"));
    };
    let sketch =
        resolve_design_sketch(&doc.feature_nodes, &doc.sketches, &doc.parameters, &node.id)?;
    Ok(ProjectedSketchInspection {
        feature_id: node.id.clone(),
        derived: true,
        sketch,
        bindings: def.projections.clone(),
    })
}
pub fn command(args: Vec<String>) -> Result<()> {
    if args.len() != 2 {
        return Err(OpenCadError::validation(
            "usage: musubicad inspect-projected-sketch <part> <feature_id>",
        ));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&inspect(&InspectProjectedSketchRequest {
            path: args[0].clone(),
            feature_id: args[1].clone()
        })?)?
    );
    Ok(())
}
