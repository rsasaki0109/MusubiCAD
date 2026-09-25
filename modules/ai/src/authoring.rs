//! Express a part design as one structural `DesignPatch` (ADR-013).
//!
//! Applying the result to an empty part document reproduces the source
//! Design Graph: parameters, sketches, features in display order, semantic
//! references, and assertions.  Derived data (sketch profiles and solve
//! state, the Feature Graph) is recomputed by the patch layer, not copied.

use opencad_core::{OpenCadError, Result};

use crate::feature_patch::FeaturePosition;
use crate::{DesignPatch, DesignState, PatchOperation};

/// Build the structural patch that authors `state` from an empty part.
///
/// Fails for content that structural operations cannot author yet: assembly
/// and drawing models, parameter roles, sketch dimensions, or a state without
/// a complete feature display order.
pub fn authoring_patch(state: &DesignState) -> Result<DesignPatch> {
    if state.assembly.is_some() || state.drawing.is_some() {
        return Err(OpenCadError::validation(
            "authoring patches cover part documents only; assembly and drawing operations are not structural yet",
        ));
    }
    if state.feature_order.len() != state.feature_nodes.len() {
        return Err(OpenCadError::validation(
            "authoring patch requires a feature order listing every feature",
        ));
    }

    let mut operations = Vec::new();
    for entry in state.parameters.entries() {
        if entry.role.is_some() {
            return Err(OpenCadError::validation(format!(
                "parameter '{}' has a role, which add_parameter cannot author yet",
                entry.id
            )));
        }
        operations.push(PatchOperation::AddParameter {
            id: entry.id.clone(),
            name: entry.name.clone(),
            expr: entry.expr.clone(),
        });
    }
    for sketch in &state.sketches {
        if !sketch.dimensions.is_empty() {
            return Err(OpenCadError::validation(format!(
                "sketch '{}' has dimensions, which structural operations cannot author yet",
                sketch.id
            )));
        }
        let sketch_id = sketch.id.as_str().to_string();
        operations.push(PatchOperation::AddSketch {
            id: sketch_id.clone(),
            name: sketch.name.clone(),
            workplane: sketch.workplane.clone(),
        });
        for entity in &sketch.entities {
            operations.push(PatchOperation::AddSketchEntity {
                sketch_id: sketch_id.clone(),
                entity: entity.clone(),
            });
        }
        for constraint in &sketch.constraints {
            operations.push(PatchOperation::AddSketchConstraint {
                sketch_id: sketch_id.clone(),
                constraint: constraint.clone(),
            });
        }
    }
    for id in &state.feature_order {
        let node = state
            .feature_nodes
            .iter()
            .find(|node| node.id == *id)
            .ok_or_else(|| OpenCadError::validation(format!("unknown feature '{id}'")))?;
        operations.push(PatchOperation::AddFeature {
            node: node.clone(),
            position: FeaturePosition::end(),
        });
    }
    for topo_ref in &state.semantic_refs {
        operations.push(PatchOperation::AddSemanticRef {
            topo_ref: topo_ref.clone(),
        });
    }
    for assertion in &state.assertions {
        operations.push(PatchOperation::AddAssertion {
            assertion: assertion.clone(),
        });
    }
    Ok(DesignPatch::new(operations))
}
