//! Revision-guarded authored sketch forms, shared by Tauri and Agent API.
use crate::editor::{
    feature_tree, verify_editor_document, verify_editor_document_native, EditorEvidence,
    FeatureEditResult,
};
use crate::sketch_workplanes::{
    admitted_datum_planes_with_kernel, resolved_frame, ResolvedSketchFrame,
};
use opencad_ai::{DesignPatch, PatchOperation};
use opencad_core::{ConstraintId, EntityId, Expression, OpenCadError, Result};
use opencad_feature::FeatureDefinition;
use opencad_file::{
    apply_patch_with_history, document_design_state, read_ocad, write_ocad, DocumentHistory,
    DocumentHistoryState, OcadDocument,
};
use opencad_geometry::GeometryKernel;
use opencad_sketch::{
    CircleEntity, Constraint, Coord, DistanceTarget, EllipseEntity, EntityBase, EntityRef,
    LineEntity, PointEntity, Sketch, SketchEntity, Workplane,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchEditorState {
    pub document_id: String,
    pub revision: String,
    pub sketches: Vec<SketchEditorRow>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchEditorRow {
    pub source: Sketch,
    pub solved: Sketch,
    pub workplane_label: String,
    pub resolved_frame: ResolvedSketchFrame,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SketchEdit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    pub revision: String,
    pub sketch_id: String,
    pub operations: Vec<SketchForm>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum SketchForm {
    Point {
        id: String,
        x: String,
        y: String,
        create: bool,
    },
    Line {
        id: String,
        start: String,
        end: String,
        construction: bool,
    },
    Circle {
        id: String,
        center: String,
        radius: String,
        construction: bool,
        create: bool,
    },
    Ellipse {
        id: String,
        center: String,
        major_radius: String,
        minor_radius: String,
        axis_angle: String,
        construction: bool,
        create: bool,
    },
    Construction {
        id: String,
        construction: bool,
    },
    Horizontal {
        id: String,
        line: String,
    },
    Vertical {
        id: String,
        line: String,
    },
    Coincident {
        id: String,
        a: String,
        b: String,
    },
    Distance {
        id: String,
        a: String,
        b: String,
        expr: String,
    },
    Radius {
        id: String,
        target: String,
        expr: String,
    },
    RemoveConstraint {
        id: String,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchEditPlan {
    pub edit: SketchEdit,
    pub patch: DesignPatch,
    pub state: SketchEditorState,
    pub evidence: EditorEvidence,
}

fn authored<'a>(doc: &'a OcadDocument, id: &str) -> Result<&'a Sketch> {
    let sketch = doc
        .sketches
        .iter()
        .find(|s| s.id.as_str() == id)
        .ok_or_else(|| OpenCadError::validation("unknown authored sketch"))?;
    let owners = doc
        .feature_nodes
        .iter()
        .filter(|n| {
            !n.suppressed
                && matches!(&n.definition, FeatureDefinition::Sketch(s) if s.sketch_id==id)
        })
        .count();
    if owners != 1 {
        return Err(OpenCadError::validation(
            "sketch editor requires one active authored owner",
        ));
    }
    match &sketch.workplane {
        Workplane::Global { .. } => {}
        Workplane::DatumPlane { feature_id } => {
            opencad_feature::datum::admitted_sketch_datum_chain(
                feature_id,
                doc.feature_nodes.iter(),
            )?;
        }
        _ => {
            return Err(OpenCadError::validation(
                "unsupported sketch editor workplane",
            ))
        }
    }
    Ok(sketch)
}
pub fn sketch_editor_state(doc: &OcadDocument) -> Result<SketchEditorState> {
    sketch_editor_state_with_kernel(doc, &opencad_kernel_occt::OcctGeometryKernel::new())
}
pub fn sketch_editor_state_with_kernel<K: GeometryKernel>(
    doc: &OcadDocument,
    kernel: &K,
) -> Result<SketchEditorState> {
    let values = opencad_graph::evaluate_param_graph_strict(&doc.parameters)?;
    let datums = admitted_datum_planes_with_kernel(doc, kernel)?;
    let mut sketches = Vec::new();
    for sketch in &doc.sketches {
        if authored(doc, sketch.id.as_str()).is_ok() {
            let mut solved = opencad_feature::param_apply::solved_sketch_copy(sketch, &values)?;
            for entity in &mut solved.entities {
                if let SketchEntity::Ellipse(e) = entity {
                    let [a, b, angle] = opencad_sketch::solve::ellipse_parameters(e)?;
                    e.major_radius = Coord::Literal(a);
                    e.minor_radius = Coord::Literal(b);
                    e.axis_angle = Coord::Literal(angle);
                }
            }
            let (workplane_label, resolved_frame) = match &sketch.workplane {
                Workplane::Global { plane } => {
                    (format!("{plane:?}"), resolved_frame(&sketch.workplane)?)
                }
                Workplane::DatumPlane { feature_id } => {
                    let plane = datums
                        .iter()
                        .find(|p| &p.feature_id == feature_id)
                        .ok_or_else(|| {
                            OpenCadError::validation("sketch datum frame unavailable")
                        })?;
                    (format!("Datum · {}", plane.name), plane.frame.clone())
                }
                _ => {
                    return Err(OpenCadError::validation(
                        "unsupported sketch editor workplane",
                    ))
                }
            };
            sketches.push(SketchEditorRow {
                source: sketch.clone(),
                solved,
                workplane_label,
                resolved_frame,
            });
        }
    }
    Ok(SketchEditorState {
        document_id: doc.metadata.id.as_str().into(),
        revision: feature_tree(doc)?.revision,
        sketches,
    })
}
pub fn inspect_document_sketches(path: &str) -> Result<SketchEditorState> {
    sketch_editor_state(&read_ocad(path)?)
}
fn coord(doc: &OcadDocument, expr: &str, angle: bool) -> Result<Coord> {
    let names = opencad_graph::parameter_names_in_expr(expr);
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
            "sketch inputs require explicit units or same-kind parameters",
        ));
    }
    for name in names {
        let is_angle = name.ends_with("_rad") || name.ends_with("_deg") || name.contains("angle");
        if is_angle != angle {
            return Err(OpenCadError::validation(
                "sketch input parameter unit kind mismatch",
            ));
        }
    }
    let values = opencad_graph::evaluate_param_graph_strict(&doc.parameters)?;
    let value = if angle {
        opencad_graph::eval_angle_expr_strict(expr, &values)?
    } else {
        opencad_graph::eval_length_expr_strict(expr, &values)?
    };
    if !value.is_finite() || value.abs() > 1e6 {
        return Err(OpenCadError::validation(
            "sketch coordinate outside finite bounded SI range",
        ));
    }
    Coord::expr(expr)
}
fn base(id: &str, construction: bool) -> Result<EntityBase> {
    Ok(EntityBase {
        id: EntityId::new(id)?,
        construction,
    })
}
fn sketch_patch(doc: &OcadDocument, edit: &SketchEdit) -> Result<DesignPatch> {
    if feature_tree(doc)?.revision != edit.revision {
        return Err(OpenCadError::validation("stale sketch source revision"));
    }
    let sketch = authored(doc, &edit.sketch_id)?;
    if edit
        .document_id
        .as_deref()
        .is_some_and(|id| id != doc.metadata.id.as_str())
        || (matches!(sketch.workplane, Workplane::DatumPlane { .. })
            && edit.document_id.as_deref() != Some(doc.metadata.id.as_str()))
    {
        return Err(OpenCadError::validation(
            "sketch edit document identity required or mismatched",
        ));
    }
    if edit.operations.is_empty() || edit.operations.len() > 64 {
        return Err(OpenCadError::validation(
            "sketch form batch must contain 1..64 operations",
        ));
    }
    let mut ops = Vec::new();
    for form in &edit.operations {
        let sketch = authored(doc, &edit.sketch_id)?;
        if let SketchForm::Point { id, .. } | SketchForm::Construction { id, .. } = form {
            if sketch.entities.iter().any(|e| matches!(e, SketchEntity::CubicBezier(c) if c.base.id.as_str()==id || c.control_points.iter().any(|p| p.as_str()==id))) {
                return Err(OpenCadError::validation("cubic/NURBS editing remains separately gated"));
            }
        }
        if let SketchForm::Construction { id, .. } = form {
            if !matches!(
                sketch.find_entity(id),
                Some(
                    SketchEntity::Point(_)
                        | SketchEntity::Line(_)
                        | SketchEntity::Circle(_)
                        | SketchEntity::Ellipse(_)
                )
            ) {
                return Err(OpenCadError::validation(
                    "unsupported construction entity in sketch editor",
                ));
            }
        }
        if let SketchForm::Circle { radius, .. }
        | SketchForm::Radius { expr: radius, .. }
        | SketchForm::Distance { expr: radius, .. } = form
        {
            coord(doc, radius, false)?;
            let values = opencad_graph::evaluate_param_graph_strict(&doc.parameters)?;
            if opencad_graph::eval_length_expr_strict(radius, &values)? <= 0.0 {
                return Err(OpenCadError::validation("radius/distance must be positive"));
            }
        }
        let entity = match form {
            SketchForm::Point { id, x, y, .. } => Some(SketchEntity::Point(PointEntity {
                base: base(id, false)?,
                x: coord(doc, x, false)?,
                y: coord(doc, y, false)?,
            })),
            SketchForm::Line {
                id,
                start,
                end,
                construction,
            } => Some(SketchEntity::Line(LineEntity {
                base: base(id, *construction)?,
                start: EntityId::new(start)?,
                end: EntityId::new(end)?,
            })),
            SketchForm::Circle {
                id,
                center,
                radius,
                construction,
                ..
            } => Some(SketchEntity::Circle(CircleEntity {
                base: base(id, *construction)?,
                center: EntityId::new(center)?,
                radius: coord(doc, radius, false)?,
            })),
            SketchForm::Ellipse {
                id,
                center,
                major_radius,
                minor_radius,
                axis_angle,
                construction,
                ..
            } => Some(SketchEntity::Ellipse(EllipseEntity {
                base: base(id, *construction)?,
                center: EntityId::new(center)?,
                major_radius: coord(doc, major_radius, false)?,
                minor_radius: coord(doc, minor_radius, false)?,
                axis_angle: coord(doc, axis_angle, true)?,
                resolved_parameters: None,
            })),
            _ => None,
        };
        if let Some(mut entity) = entity {
            let create = match form {
                SketchForm::Point { create, .. }
                | SketchForm::Circle { create, .. }
                | SketchForm::Ellipse { create, .. } => *create,
                _ => true,
            };
            if !create {
                let old = authored(doc, &edit.sketch_id)?
                    .find_entity(entity.id().as_str())
                    .ok_or_else(|| OpenCadError::validation("missing authored entity"))?;
                if matches!(entity, SketchEntity::Point(_)) {
                    entity.set_construction(old.is_construction());
                }
            }
            ops.push(if create {
                PatchOperation::AddSketchEntity {
                    sketch_id: edit.sketch_id.clone(),
                    entity,
                }
            } else {
                PatchOperation::ReplaceSketchEntity {
                    sketch_id: edit.sketch_id.clone(),
                    entity,
                }
            });
            continue;
        }
        let constraint = match form {
            SketchForm::Horizontal { id, line } => Some(Constraint::Horizontal {
                id: ConstraintId::new(id)?,
                line: EntityId::new(line)?,
            }),
            SketchForm::Vertical { id, line } => Some(Constraint::Vertical {
                id: ConstraintId::new(id)?,
                line: EntityId::new(line)?,
            }),
            SketchForm::Coincident { id, a, b } => Some(Constraint::Coincident {
                id: ConstraintId::new(id)?,
                a: EntityRef::Entity(EntityId::new(a)?),
                b: EntityRef::Entity(EntityId::new(b)?),
            }),
            SketchForm::Distance { id, a, b, expr } => {
                coord(doc, expr, false)?;
                Some(Constraint::Distance {
                    id: ConstraintId::new(id)?,
                    target: DistanceTarget::PointToPoint {
                        a: EntityId::new(a)?,
                        b: EntityId::new(b)?,
                    },
                    expr: Expression::new(expr)?,
                })
            }
            SketchForm::Radius { id, target, expr } => {
                coord(doc, expr, false)?;
                Some(Constraint::Radius {
                    id: ConstraintId::new(id)?,
                    target: EntityId::new(target)?,
                    expr: Expression::new(expr)?,
                })
            }
            _ => None,
        };
        if let Some(constraint) = constraint {
            ops.push(PatchOperation::AddSketchConstraint {
                sketch_id: edit.sketch_id.clone(),
                constraint,
            });
        } else {
            ops.push(match form {
                SketchForm::Construction { id, construction } => {
                    PatchOperation::SetSketchConstruction {
                        sketch_id: edit.sketch_id.clone(),
                        entity_id: id.clone(),
                        construction: *construction,
                    }
                }
                SketchForm::RemoveConstraint { id } => PatchOperation::RemoveSketchConstraint {
                    sketch_id: edit.sketch_id.clone(),
                    constraint_id: id.clone(),
                },
                _ => return Err(OpenCadError::validation("unsupported sketch form")),
            });
        }
    }
    DesignPatch::new(ops).with_revision_precondition(&document_design_state(doc))
}
pub fn plan_sketch_edit_with_kernel<K: GeometryKernel>(
    doc: &OcadDocument,
    edit: &SketchEdit,
    kernel: &K,
) -> Result<SketchEditPlan> {
    let patch = sketch_patch(doc, edit)?;
    let dry = opencad_file::dry_run_patch_document(doc, &patch);
    opencad_ai::ensure_patch_valid(&dry)?;
    let mut candidate = doc.clone();
    opencad_file::apply_patch_to_document(&mut candidate, &patch)?;
    let source = authored(&candidate, &edit.sketch_id)?;
    let point_allowed = |id: &str| {
        matches!(source.find_entity(id), Some(SketchEntity::Point(_))) && !source.entities.iter().any(|e| matches!(e, SketchEntity::CubicBezier(c) if c.control_points.iter().any(|p| p.as_str()==id)))
    };
    for form in &edit.operations {
        let allowed = match form {
            SketchForm::Horizontal { line, .. } | SketchForm::Vertical { line, .. } => {
                matches!(source.find_entity(line), Some(SketchEntity::Line(_)))
            }
            SketchForm::Coincident { a, b, .. } | SketchForm::Distance { a, b, .. } => {
                point_allowed(a) && point_allowed(b)
            }
            SketchForm::Radius { target, .. } => {
                matches!(source.find_entity(target), Some(SketchEntity::Circle(_)))
            }
            _ => true,
        };
        if !allowed {
            return Err(OpenCadError::validation(
                "constraint requires admitted authored point/line/circle targets",
            ));
        }
    }
    // Explicitly solve every admitted sketch, including unconsumed construction geometry.
    let state = sketch_editor_state_with_kernel(&candidate, kernel)?;
    let evidence = if dry.impact.no_op {
        EditorEvidence {
            no_op: true,
            volume_m3: None,
            bounds_size_m: None,
            regenerated: Vec::new(),
        }
    } else {
        verify_editor_document(&candidate, kernel)?
    };
    Ok(SketchEditPlan {
        edit: edit.clone(),
        patch,
        state,
        evidence,
    })
}
pub fn apply_sketch_edit_with_kernel<K: GeometryKernel>(
    doc: &mut OcadDocument,
    edit: &SketchEdit,
    history: &mut DocumentHistory,
    kernel: &K,
) -> Result<FeatureEditResult> {
    let plan = plan_sketch_edit_with_kernel(doc, edit, kernel)?;
    let mut candidate = doc.clone();
    let mut next = history.clone();
    if !plan.evidence.no_op {
        apply_patch_with_history(&mut candidate, &plan.patch, &mut next, "Edit sketch")?;
    }
    let result = FeatureEditResult {
        tree: feature_tree(&candidate)?,
        history: DocumentHistoryState::new(next.clone()),
        evidence: plan.evidence,
    };
    *doc = candidate;
    *history = next;
    Ok(result)
}
pub fn plan_document_sketch_edit(path: &str, edit: &SketchEdit) -> Result<SketchEditPlan> {
    let doc = read_ocad(path)?;
    let mut plan =
        plan_sketch_edit_with_kernel(&doc, edit, &opencad_kernel_occt::OcctGeometryKernel::new())?;
    if !plan.evidence.no_op {
        let mut candidate = doc;
        opencad_file::apply_patch_to_document(&mut candidate, &plan.patch)?;
        plan.evidence = verify_editor_document_native(&candidate)?;
    }
    Ok(plan)
}
pub fn apply_document_sketch_edit(
    path: &str,
    edit: &SketchEdit,
    history: Option<DocumentHistory>,
) -> Result<FeatureEditResult> {
    let mut doc = read_ocad(path)?;
    let before = feature_tree(&doc)?.revision;
    let result = apply_sketch_edit_with_kernel(
        &mut doc,
        edit,
        &mut history.unwrap_or_default(),
        &opencad_kernel_occt::OcctGeometryKernel::new(),
    )?;
    if !result.evidence.no_op {
        verify_editor_document_native(&doc)?;
    }
    let current = read_ocad(path)?;
    if current.metadata.id != doc.metadata.id || feature_tree(&current)?.revision != before {
        return Err(OpenCadError::validation(
            "document changed during sketch verification",
        ));
    }
    if !result.evidence.no_op {
        write_ocad(path, &doc)?;
    }
    Ok(result)
}
