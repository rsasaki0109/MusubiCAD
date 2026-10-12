//! Backend-owned feature forms and verified part publication (ADR-058).

use opencad_ai::{
    design_state_revision, evaluate_assertions, required_assertions_pass, AssertionContext,
    DesignPatch, FeaturePosition, PatchOperation,
};
use opencad_core::{DocumentKind, Length, OpenCadError, Result};
use opencad_feature::{ExtrudeFeature, FeatureDefinition, FeatureNode, FeatureRegistry};
use opencad_file::{
    apply_patch_with_history, document_design_state, read_ocad, write_ocad, DocumentHistory,
    DocumentHistoryState, OcadDocument,
};
use opencad_geometry::{ExtrudeExtent, GeometryKernel};
use opencad_graph::{
    eval_length_expr_strict, evaluate_param_graph_strict, parameter_names_in_expr,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureField {
    pub field: String,
    pub expr: String,
    pub unit_hint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureRow {
    pub id: String,
    pub name: String,
    pub feature_type: String,
    pub suppressed: bool,
    pub inputs: Vec<String>,
    pub fields: Vec<FeatureField>,
    pub closed_profiles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureTree {
    pub revision: String,
    pub rows: Vec<FeatureRow>,
}

/// Transport intent, not persisted source. Unknown fields refuse rather than
/// turning a misspelled form input into a different mutation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum FeatureEdit {
    CreateExtrude {
        id: String,
        name: String,
        sketch_feature: String,
        profile_ref: String,
        length_expr: String,
    },
    SetExpression {
        id: String,
        field: String,
        expr: String,
    },
    Move {
        id: String,
        position: FeaturePosition,
    },
    Suppress {
        id: String,
        suppressed: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorEvidence {
    pub no_op: bool,
    pub volume_m3: Option<f64>,
    pub bounds_size_m: Option<[f64; 3]>,
    pub regenerated: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureEditPlan {
    pub revision: String,
    pub edit: FeatureEdit,
    pub patch: DesignPatch,
    pub evidence: EditorEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureEditResult {
    pub tree: FeatureTree,
    pub history: DocumentHistoryState,
    pub evidence: EditorEvidence,
}

fn part(doc: &OcadDocument) -> Result<()> {
    if doc.metadata.kind != DocumentKind::Part {
        return Err(OpenCadError::validation(
            "feature editor accepts only part documents",
        ));
    }
    Ok(())
}

fn length_field(definition: &FeatureDefinition) -> Option<(&str, String)> {
    let extent_length = |e: &ExtrudeExtent| match e {
        ExtrudeExtent::Distance { length } => Some(length.meters()),
        _ => None,
    };
    let (field, expr, value) = match definition {
        FeatureDefinition::Extrude(d) => ("length_expr", &d.length_expr, extent_length(&d.extent)?),
        FeatureDefinition::Hole(d) => ("depth_expr", &d.depth_expr, extent_length(&d.depth)?),
        FeatureDefinition::StandardHole(d) => (
            "depth_expr",
            &d.hole.depth_expr,
            extent_length(&d.hole.depth)?,
        ),
        FeatureDefinition::Fillet(d) => ("radius_expr", &d.radius_expr, d.radius.meters()),
        FeatureDefinition::Chamfer(d) => ("distance_expr", &d.distance_expr, d.distance.meters()),
        FeatureDefinition::LinearPattern(d) => {
            ("spacing_expr", &d.spacing_expr, d.spacing.meters())
        }
        FeatureDefinition::Shell(d) => ("thickness_expr", &d.thickness_expr, d.thickness.meters()),
        _ => return None,
    };
    Some((
        field,
        expr.clone()
            .unwrap_or_else(|| format!("{} mm", value * 1000.0)),
    ))
}

pub fn feature_tree(doc: &OcadDocument) -> Result<FeatureTree> {
    part(doc)?;
    let mut rows = Vec::new();
    for id in doc.feature_graph.ordered_ids() {
        let node = doc
            .feature_nodes
            .iter()
            .find(|n| &n.id == id)
            .ok_or_else(|| OpenCadError::not_found(format!("feature '{id}'")))?;
        let fields = length_field(&node.definition)
            .map(|(field, expr)| FeatureField {
                field: field.into(),
                expr,
                unit_hint: "mm, m, or length parameter names".into(),
            })
            .into_iter()
            .collect();
        let mut closed_profiles = Vec::new();
        // Creation deliberately accepts authored sketches only. Derived
        // projection editing belongs to a separate semantic binding command.
        if !node.suppressed {
            if let FeatureDefinition::Sketch(def) = &node.definition {
                if let Some(source) = doc.sketches.iter().find(|s| s.id.as_str() == def.sketch_id) {
                    let mut sketch = source.clone();
                    opencad_feature::sketch_bridge::prepare_sketch(&mut sketch)?;
                    closed_profiles = sketch
                        .profiles
                        .iter()
                        .filter(|p| p.is_closed())
                        .map(|p| p.id.clone())
                        .collect();
                }
            }
        }
        rows.push(FeatureRow {
            id: id.clone(),
            name: node.name.clone(),
            feature_type: node.definition.feature_type().into(),
            suppressed: node.suppressed,
            inputs: doc
                .feature_graph
                .dependency_edges()
                .iter()
                .filter(|e| &e.target == id)
                .map(|e| e.source.clone())
                .collect(),
            fields,
            closed_profiles,
        });
    }
    Ok(FeatureTree {
        revision: design_state_revision(&document_design_state(doc))?,
        rows,
    })
}

pub fn list_document_features(path: &str) -> Result<FeatureTree> {
    feature_tree(&read_ocad(path)?)
}

fn admitted_length(doc: &OcadDocument, expr: &str) -> Result<f64> {
    let scope = evaluate_param_graph_strict(&doc.parameters)?;
    let names = parameter_names_in_expr(expr);
    if names.is_empty()
        && !expr
            .split(|c: char| !c.is_alphabetic())
            .any(|u| matches!(u, "mm" | "m" | "cm" | "in"))
    {
        return Err(OpenCadError::validation(
            "length input requires explicit units or length parameter names",
        ));
    }
    for name in names {
        if name.ends_with("_rad") || name.ends_with("_deg") || name.contains("angle") {
            return Err(OpenCadError::validation(
                "length input cannot reference an angle parameter",
            ));
        }
    }
    let value = eval_length_expr_strict(expr, &scope)?;
    if value <= 0.0 {
        return Err(OpenCadError::validation("feature length must be positive"));
    }
    Ok(value)
}

fn edit_patch(doc: &OcadDocument, revision: &str, edit: &FeatureEdit) -> Result<DesignPatch> {
    let tree = feature_tree(doc)?;
    if tree.revision != revision {
        return Err(OpenCadError::validation(
            "stale feature editor revision; refresh before editing",
        ));
    }
    let operation = match edit {
        FeatureEdit::CreateExtrude {
            id,
            name,
            sketch_feature,
            profile_ref,
            length_expr,
        } => {
            if !tree
                .rows
                .iter()
                .any(|r| &r.id == sketch_feature && r.closed_profiles.contains(profile_ref))
            {
                return Err(OpenCadError::validation(
                    "creation requires an admitted authored closed sketch profile",
                ));
            }
            let length = admitted_length(doc, length_expr)?;
            let mut def = ExtrudeFeature::boss(
                sketch_feature,
                profile_ref,
                ExtrudeExtent::Distance {
                    length: Length::from_meters(length),
                },
            );
            def.length_expr = Some(length_expr.clone());
            PatchOperation::AddFeature {
                node: FeatureNode::new(id, name, FeatureDefinition::Extrude(def)),
                position: FeaturePosition::end(),
            }
        }
        FeatureEdit::SetExpression { id, field, expr } => {
            if !tree
                .rows
                .iter()
                .any(|r| &r.id == id && r.fields.iter().any(|f| &f.field == field))
            {
                return Err(OpenCadError::validation(
                    "feature field is not admitted by the editor",
                ));
            }
            admitted_length(doc, expr)?;
            PatchOperation::SetFeatureExpr {
                feature_id: id.clone(),
                field: field.clone(),
                expr: expr.clone(),
            }
        }
        FeatureEdit::Move { id, position } => PatchOperation::MoveFeature {
            id: id.clone(),
            position: position.clone(),
        },
        FeatureEdit::Suppress { id, suppressed } => PatchOperation::SetFeatureSuppressed {
            id: id.clone(),
            suppressed: *suppressed,
        },
    };
    DesignPatch::new(vec![operation]).with_revision_precondition(&document_design_state(doc))
}

/// Verify an unpublished candidate, including suppressed input and required
/// assertion admission. Empty trees are valid editable states, not fake solids.
pub fn verify_editor_document<K: GeometryKernel>(
    doc: &OcadDocument,
    kernel: &K,
) -> Result<EditorEvidence> {
    part(doc)?;
    opencad_ai::ensure_patch_valid(&opencad_file::dry_run_patch_document(
        doc,
        &DesignPatch::default(),
    ))?;
    for edge in doc.feature_graph.dependency_edges() {
        if doc
            .feature_nodes
            .iter()
            .any(|n| n.id == edge.source && n.suppressed)
            && doc
                .feature_nodes
                .iter()
                .any(|n| n.id == edge.target && !n.suppressed)
        {
            return Err(OpenCadError::validation(format!(
                "active feature '{}' consumes suppressed input '{}'",
                edge.target, edge.source
            )));
        }
    }
    let values = evaluate_param_graph_strict(&doc.parameters)?;
    let mut model = doc.clone().into_part_model();
    let report = model.regenerate(
        kernel,
        &FeatureRegistry::with_defaults(),
        Some(&doc.parameters),
        Some(&doc.semantic_refs),
    )?;
    let mass = model
        .active_body()
        .map(|body| kernel.mass_properties(body, crate::DEFAULT_DENSITY_KG_PER_M3))
        .transpose()?;
    let bounds = model
        .active_body()
        .map(|body| kernel.bounding_box(body))
        .transpose()?;
    let size = bounds.map(|b| {
        [
            b.max[0] - b.min[0],
            b.max[1] - b.min[1],
            b.max[2] - b.min[2],
        ]
    });
    let assertions = evaluate_assertions(
        &doc.assertions,
        &AssertionContext {
            parameter_values: values.into_iter().collect(),
            mass_kg: mass.as_ref().map(|m| m.mass_kg),
            bounding_box_size_m: size,
            // A kernel body may be a compound; unavailable solid counts must fail closed.
            body_count: model.active_body().is_none().then_some(0),
            reference_provenance: report.reference_provenance,
            ..AssertionContext::default()
        },
    );
    if !required_assertions_pass(&assertions) {
        return Err(OpenCadError::validation(
            "required editor candidate assertions failed",
        ));
    }
    Ok(EditorEvidence {
        no_op: false,
        volume_m3: mass.map(|m| m.volume_m3),
        bounds_size_m: size,
        regenerated: report.regenerated,
    })
}

pub fn plan_feature_edit_with_kernel<K: GeometryKernel>(
    doc: &OcadDocument,
    revision: &str,
    edit: FeatureEdit,
    kernel: &K,
) -> Result<FeatureEditPlan> {
    let patch = edit_patch(doc, revision, &edit)?;
    let dry_run = opencad_file::dry_run_patch_document(doc, &patch);
    opencad_ai::ensure_patch_valid(&dry_run)?;
    if dry_run.impact.no_op {
        return Ok(FeatureEditPlan {
            revision: revision.into(),
            edit,
            patch,
            evidence: EditorEvidence {
                no_op: true,
                volume_m3: None,
                bounds_size_m: None,
                regenerated: Vec::new(),
            },
        });
    }

    let mut candidate = doc.clone();
    opencad_file::apply_patch_to_document(&mut candidate, &patch)?;
    let evidence = verify_editor_document(&candidate, kernel)?;
    Ok(FeatureEditPlan {
        revision: revision.into(),
        edit,
        patch,
        evidence,
    })
}

pub fn apply_feature_edit_with_kernel<K: GeometryKernel>(
    doc: &mut OcadDocument,
    revision: &str,
    edit: FeatureEdit,
    history: &mut DocumentHistory,
    kernel: &K,
) -> Result<FeatureEditResult> {
    let plan = plan_feature_edit_with_kernel(doc, revision, edit, kernel)?;
    if plan.evidence.no_op {
        return Ok(FeatureEditResult {
            tree: feature_tree(doc)?,
            history: DocumentHistoryState::new(history.clone()),
            evidence: plan.evidence,
        });
    }

    let mut candidate = doc.clone();
    let mut next_history = history.clone();
    apply_patch_with_history(
        &mut candidate,
        &plan.patch,
        &mut next_history,
        "Edit feature tree",
    )?;
    let result = FeatureEditResult {
        tree: feature_tree(&candidate)?,
        history: DocumentHistoryState::new(next_history.clone()),
        evidence: plan.evidence,
    };
    *doc = candidate;
    *history = next_history;
    Ok(result)
}

pub fn plan_document_feature_edit(
    path: &str,
    revision: &str,
    edit: FeatureEdit,
) -> Result<FeatureEditPlan> {
    let doc = read_ocad(path)?;
    let mut plan = plan_feature_edit_with_kernel(
        &doc,
        revision,
        edit,
        &opencad_kernel_occt::OcctGeometryKernel::new(),
    )?;
    if !plan.evidence.no_op {
        let mut candidate = doc;
        opencad_file::apply_patch_to_document(&mut candidate, &plan.patch)?;
        plan.evidence = verify_editor_document_native(&candidate)?;
    }
    Ok(plan)
}

pub fn apply_document_feature_edit(
    path: &str,
    revision: &str,
    edit: FeatureEdit,
    history: Option<DocumentHistory>,
) -> Result<FeatureEditResult> {
    let mut doc = read_ocad(path)?;
    let before = feature_tree(&doc)?.revision;
    let result = apply_feature_edit_with_kernel(
        &mut doc,
        revision,
        edit,
        &mut history.unwrap_or_default(),
        &opencad_kernel_occt::OcctGeometryKernel::new(),
    )?;
    if !result.evidence.no_op {
        verify_editor_document_native(&doc)?;
    }
    if feature_tree(&read_ocad(path)?)?.revision != before {
        return Err(OpenCadError::validation(
            "document changed during editor verification",
        ));
    }
    if !result.evidence.no_op {
        write_ocad(path, &doc)?;
    }
    Ok(result)
}

/// Native publication also proves a disposable scene can be tessellated before
/// changing source. GPU rasterization remains a separate display operation.
pub fn verify_editor_document_native(doc: &OcadDocument) -> Result<EditorEvidence> {
    let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
    let evidence = verify_editor_document(doc, &kernel)?;
    let mut model = doc.clone().into_part_model();
    model.regenerate(
        &kernel,
        &FeatureRegistry::with_defaults(),
        Some(&doc.parameters),
        Some(&doc.semantic_refs),
    )?;
    if let Some(body) = model.active_body() {
        let mesh = kernel.tessellate(body, &opencad_geometry::TessellationSettings::default())?;
        if mesh.triangle_count() == 0 {
            return Err(OpenCadError::validation(
                "editor candidate has no display triangles",
            ));
        }
    }
    Ok(evidence)
}

/// Admit parameter units before the shared patch transaction. Position/offset
/// parameters may be zero or negative; feature lengths have their own bounds.
pub fn validate_editor_parameter_expression(
    doc: &OcadDocument,
    id: &str,
    expr: &str,
) -> Result<()> {
    let parameter = doc
        .parameters
        .get(id)
        .ok_or_else(|| OpenCadError::not_found(format!("parameter '{id}'")))?;
    let angle = parameter.name.ends_with("_rad")
        || parameter.name.ends_with("_deg")
        || parameter.name.contains("angle");
    let names = parameter_names_in_expr(expr);
    let has_units = expr.split(|c: char| !c.is_alphabetic()).any(|u| {
        if angle {
            matches!(
                u,
                "deg" | "rad" | "degree" | "degrees" | "radian" | "radians"
            )
        } else {
            matches!(u, "m" | "mm" | "cm" | "in")
        }
    });
    if names.is_empty() && !has_units {
        return Err(OpenCadError::validation(
            "parameter input requires explicit units or same-kind parameter names",
        ));
    }
    let scope = evaluate_param_graph_strict(&doc.parameters)?;
    if angle {
        opencad_graph::eval_angle_expr_strict(expr, &scope)?;
    } else {
        eval_length_expr_strict(expr, &scope)?;
    }
    Ok(())
}
