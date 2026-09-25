//! Express a design as one structural `DesignPatch` (ADR-013).
//!
//! Applying the result to an empty document of the same kind reproduces the
//! source Design Graph: parameters, sketches, features in display order,
//! semantic references, assertions, and the assembly and drawing models.
//! Derived data (sketch profiles and solve state, the Feature Graph) is
//! recomputed by the patch layer, not copied.

use opencad_core::{OpenCadError, Result};

use crate::feature_patch::FeaturePosition;
use crate::{DesignPatch, DesignState, PatchOperation};

/// Build the structural patch that authors `state` from an empty document
/// of the same kind.
///
/// Fails for content that structural operations cannot author yet:
/// parameter roles, sketch dimensions, or a state without a complete feature
/// display order.
pub fn authoring_patch(state: &DesignState) -> Result<DesignPatch> {
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
    for (path, bytes) in &state.attachments {
        use base64::Engine as _;
        operations.push(PatchOperation::AddAttachment {
            path: path.clone(),
            sha256: opencad_core::sha256_hex(bytes),
            content_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        });
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
            topo_ref: Box::new(topo_ref.clone()),
        });
    }
    for assertion in &state.assertions {
        operations.push(PatchOperation::AddAssertion {
            assertion: assertion.clone(),
        });
    }
    if let Some(assembly) = &state.assembly {
        for component in &assembly.components {
            operations.push(PatchOperation::AddComponent {
                component: component.clone(),
            });
        }
        for instance in &assembly.instances {
            operations.push(PatchOperation::AddInstance {
                instance: instance.clone(),
            });
        }
        for connector in &assembly.connectors {
            operations.push(PatchOperation::AddConnector {
                id: connector.id.as_str().to_string(),
                name: connector.name.clone(),
                instance_id: connector.instance.as_str().to_string(),
                transform: connector.transform,
            });
        }
        for mate in &assembly.mates {
            operations.push(PatchOperation::AddMate {
                mate: Box::new(mate.clone()),
            });
        }
        for pattern in &assembly.patterns {
            operations.push(PatchOperation::AddAssemblyPattern {
                pattern: pattern.clone(),
            });
        }
    }
    if let Some(drawing) = &state.drawing {
        for sheet in &drawing.sheets {
            let mut empty = sheet.clone();
            empty.views.clear();
            empty.dimensions.clear();
            let sheet_id = sheet.id.as_str().to_string();
            operations.push(PatchOperation::AddSheet { sheet: empty });
            for view in &sheet.views {
                operations.push(PatchOperation::AddDrawingView {
                    sheet_id: sheet_id.clone(),
                    view: view.clone(),
                });
            }
            for dimension in &sheet.dimensions {
                operations.push(PatchOperation::AddDrawingDimension {
                    sheet_id: sheet_id.clone(),
                    dimension: dimension.clone(),
                });
            }
        }
    }
    Ok(DesignPatch::new(operations))
}
