//! Backend-owned source admission and disposable regenerated sketch frames.
use opencad_core::{OpenCadError, Result};
use opencad_feature::{datum::DatumGeometry, FeatureDefinition, FeatureRegistry};
use opencad_file::OcadDocument;
use opencad_geometry::GeometryKernel;
use opencad_sketch::Workplane;
use serde::{Deserialize, Serialize};

/// Origin is SI metres; axes and normal are dimensionless unit vectors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedSketchFrame {
    pub origin_m: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub normal: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatumPlaneChoice {
    pub feature_id: String,
    pub name: String,
    pub frame: ResolvedSketchFrame,
}

/// Only global constants or the datum executor's validated regenerated output
/// reach this function. No caller-supplied cached frame is accepted as intent.
pub(crate) fn resolved_frame(workplane: &Workplane) -> Result<ResolvedSketchFrame> {
    let p = opencad_feature::placement_from_workplane(workplane)?;
    Ok(ResolvedSketchFrame {
        origin_m: p.origin_m,
        x_axis: p.x_axis_m,
        y_axis: p.y_axis_m,
        normal: p.extrude_direction_m(),
    })
}

pub fn admitted_datum_planes_with_kernel<K: GeometryKernel>(
    doc: &OcadDocument,
    kernel: &K,
) -> Result<Vec<DatumPlaneChoice>> {
    crate::editor::feature_tree(doc)?;
    if !doc
        .feature_nodes
        .iter()
        .any(|n| matches!(n.definition, FeatureDefinition::DatumPlane(_)))
    {
        return Ok(Vec::new());
    }
    opencad_ai::ensure_patch_valid(&opencad_file::dry_run_patch_document(
        doc,
        &opencad_ai::DesignPatch::default(),
    ))?;
    opencad_feature::derive_feature_graph(
        &doc.feature_nodes,
        doc.feature_graph.ordered_ids(),
        &doc.sketches,
        &doc.semantic_refs,
    )?;
    let mut model = doc.clone().into_part_model();
    model.regenerate(
        kernel,
        &FeatureRegistry::with_defaults(),
        Some(&doc.parameters),
        Some(&doc.semantic_refs),
    )?;
    let mut choices = Vec::new();
    for id in doc.feature_graph.ordered_ids() {
        let node = &model.nodes[id];
        if node.suppressed
            || !matches!(node.definition, FeatureDefinition::DatumPlane(_))
            || opencad_feature::datum::admitted_sketch_datum_chain(id, model.nodes.values())
                .is_err()
        {
            continue;
        }
        let Some(DatumGeometry::Plane { workplane }) =
            model.outputs.get(id).and_then(|o| o.datum.as_ref())
        else {
            return Err(OpenCadError::validation(
                "datum frame unavailable after regeneration",
            ));
        };
        choices.push(DatumPlaneChoice {
            feature_id: id.clone(),
            name: node.name.clone(),
            frame: resolved_frame(workplane)?,
        });
    }
    Ok(choices)
}
