//! Explicit presentation translations, independent of mechanical state (ADR-055).
use crate::AssemblyModel;
use opencad_core::{InstanceId, OpenCadError, Result, ViewId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplodedOffset {
    pub instance_id: InstanceId,
    pub translation_m: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplodedView {
    pub id: ViewId,
    pub name: String,
    pub offsets: Vec<ExplodedOffset>,
}

pub fn validate_exploded_views(model: &AssemblyModel) -> Result<()> {
    let mut ids = BTreeSet::new();
    for view in &model.exploded_views {
        ViewId::new(view.id.as_str())?;
        if !ids.insert(view.id.as_str())
            || view.name.trim().is_empty()
            || view.offsets.is_empty()
            || view.offsets.len() > 256
        {
            return Err(OpenCadError::validation(
                "exploded view requires unique ID, name and 1..256 offsets",
            ));
        }
        let mut targets = BTreeSet::new();
        for offset in &view.offsets {
            InstanceId::new(offset.instance_id.as_str())?;
            if !targets.insert(offset.instance_id.as_str())
                || model
                    .instances
                    .iter()
                    .filter(|i| i.id == offset.instance_id)
                    .count()
                    != 1
                || offset
                    .translation_m
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 1e6)
            {
                return Err(OpenCadError::validation(format!("invalid or unresolved exploded target '{}' (declared top-level instances only; finite SI metres required)", offset.instance_id)));
            }
        }
    }
    Ok(())
}

/// Translate detached tessellations. Mechanical bodies and reports are never touched.
pub fn exploded_meshes(
    model: &AssemblyModel,
    mechanical: &[crate::InstanceMesh],
    view_id: &str,
) -> Result<Vec<crate::InstanceMesh>> {
    validate_exploded_views(model)?;
    let view = model
        .exploded_views
        .iter()
        .find(|v| v.id.as_str() == view_id)
        .ok_or_else(|| OpenCadError::validation(format!("unknown exploded view '{view_id}'")))?;
    let mut seen = BTreeSet::new();
    let mut meshes = mechanical.to_vec();
    for instance in &mut meshes {
        if instance.mesh_set.positions.is_empty() || instance.mesh_set.triangle_count() == 0 {
            return Err(OpenCadError::validation(
                "exploded presentation requires nonempty instance geometry",
            ));
        }
        if !seen.insert(instance.instance_id.as_str().to_owned()) {
            return Err(OpenCadError::validation(
                "exploded presentation requires unique mechanical instances",
            ));
        }
        let delta = view
            .offsets
            .iter()
            .find(|o| o.instance_id == instance.instance_id)
            .map(|o| o.translation_m)
            .unwrap_or([0.; 3]);
        for point in &mut instance.mesh_set.positions {
            for axis in 0..3 {
                let value = f64::from(point[axis]) + delta[axis];
                if !value.is_finite() || value.abs() > f64::from(f32::MAX) {
                    return Err(OpenCadError::validation(
                        "exploded mesh coordinate is non-finite or outside rendering range",
                    ));
                }
                point[axis] = value as f32;
            }
        }
    }
    if view
        .offsets
        .iter()
        .any(|o| !seen.contains(o.instance_id.as_str()))
        || meshes.is_empty()
        || model
            .instances
            .iter()
            .any(|i| !seen.contains(i.id.as_str()))
    {
        return Err(OpenCadError::validation(
            "exploded presentation requires every declared instance mesh",
        ));
    }
    Ok(meshes)
}
