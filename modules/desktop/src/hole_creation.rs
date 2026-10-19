//! ADR-111: backend-owned stock/entry/circle choices and verified hole creation.
use crate::editor::{
    feature_tree, verify_editor_document, verify_editor_document_native, EditorEvidence,
    FeatureEditResult,
};
use opencad_ai::{DesignPatch, FeaturePosition, PatchOperation};
use opencad_core::{Angle, Length, OpenCadError, Result};
use opencad_feature::{
    regenerate::RegenSession, seated_standard_hole::admit_owned_hole_inputs, ClearanceFit,
    FeatureDefinition, FeatureNode, FeatureRegistry, HoleFeature, HoleStandard, MetricSize,
    SeatedStandardHoleFeature, StandardHoleFeature, StandardHoleSeat, ThreadRepresentation,
};
use opencad_file::{
    apply_patch_to_document, apply_patch_with_history, document_design_state, read_ocad,
    write_ocad, DocumentHistory, DocumentHistoryState, OcadDocument,
};
use opencad_geometry::{ExtrudeExtent, ExtrudeOperation, GeometryKernel};
use opencad_sketch::{SketchEntity, Workplane};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoleChoice {
    pub target_feature: String,
    pub stock_name: String,
    pub face_ref: String,
    pub entry_role: String,
    pub sketch_feature: String,
    pub axis_name: String,
    pub profile_ref: String,
    pub circle_id: String,
    pub entry_center_m: [f64; 3],
    pub inward_direction: [f64; 3],
    pub bore_diameter_m: f64,
    pub stock_thickness_m: f64,
    pub standards: Vec<HoleStandard>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoleCreationState {
    pub document_id: String,
    pub revision: String,
    pub suggested_feature_id: String,
    pub choices: Vec<HoleChoice>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum HoleSeatInput {
    None,
    Counterbore {
        diameter: String,
        depth: String,
    },
    Countersink {
        diameter: String,
        included_angle: String,
    },
}

/// Transport intent. No cached world frame or numeric kernel identity is accepted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoleCreate {
    pub document_id: String,
    pub revision: String,
    pub target_feature: String,
    pub face_ref: String,
    pub sketch_feature: String,
    pub profile_ref: String,
    pub standard: HoleStandard,
    pub depth: String,
    pub seat: HoleSeatInput,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoleCreatePlan {
    pub create: HoleCreate,
    pub feature_id: String,
    pub choice: HoleChoice,
    pub bore_depth_m: f64,
    pub seat_diameter_m: Option<f64>,
    pub seat_depth_m: Option<f64>,
    pub removed_volume_m3: f64,
    pub patch: DesignPatch,
    pub evidence: EditorEvidence,
}

fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(format!("hole creation: {message}"))
}

fn matching_standards(diameter: f64) -> Vec<HoleStandard> {
    [
        MetricSize::M3,
        MetricSize::M4,
        MetricSize::M5,
        MetricSize::M6,
        MetricSize::M8,
        MetricSize::M10,
        MetricSize::M12,
    ]
    .into_iter()
    .flat_map(|size| {
        [
            HoleStandard::Iso273 {
                size,
                fit: ClearanceFit::Close,
            },
            HoleStandard::Iso273 {
                size,
                fit: ClearanceFit::Normal,
            },
            HoleStandard::Iso273 {
                size,
                fit: ClearanceFit::Loose,
            },
            HoleStandard::IsoMetricCoarse {
                size,
                representation: ThreadRepresentation::TapDrill,
            },
        ]
    })
    .filter(|s| (s.dimensions().cut_diameter.meters() - diameter).abs() <= 1e-9)
    .collect()
}

pub fn hole_creation_state_with_kernel<K: GeometryKernel>(
    doc: &OcadDocument,
    kernel: &K,
) -> Result<HoleCreationState> {
    let tree = feature_tree(doc)?;
    let n = (1..=doc.feature_nodes.len().saturating_add(1))
        .find(|n| {
            !doc.feature_nodes
                .iter()
                .any(|f| f.id == format!("feature:hole_{n}"))
        })
        .ok_or_else(|| invalid("no free feature identity"))?;
    let mut state = HoleCreationState {
        document_id: doc.metadata.id.as_str().into(),
        revision: tree.revision,
        suggested_feature_id: format!("feature:hole_{n}"),
        choices: Vec::new(),
    };
    if !doc.feature_nodes.iter().any(|n| !n.suppressed && matches!(&n.definition,
        FeatureDefinition::Extrude(e) if e.operation == ExtrudeOperation::NewBody && e.target_feature.is_none() && matches!(e.extent, ExtrudeExtent::Distance { .. }))) {
        return Ok(state);
    }
    opencad_ai::ensure_patch_valid(&opencad_file::dry_run_patch_document(
        doc,
        &DesignPatch::default(),
    ))?;
    let mut model = doc.clone().into_part_model();
    model.regenerate(
        kernel,
        &FeatureRegistry::with_defaults(),
        Some(&doc.parameters),
        Some(&doc.semantic_refs),
    )?;
    let Some(stock) = doc
        .feature_graph
        .ordered_ids()
        .iter()
        .rev()
        .find(|id| model.outputs.get(*id).is_some_and(|o| o.body.is_some()))
    else {
        return Ok(state);
    };
    let FeatureDefinition::Extrude(extrude) = &model.nodes[stock].definition else {
        return Ok(state);
    };
    let ExtrudeExtent::Distance { length: height } = extrude.extent else {
        return Ok(state);
    };
    if extrude.operation != ExtrudeOperation::NewBody || extrude.target_feature.is_some() {
        return Ok(state);
    }
    let context = RegenSession {
        kernel,
        nodes: &model.nodes,
        sketches: &model.sketches,
        outputs: &model.outputs,
        semantic_refs: &doc.semantic_refs,
        face_history: &[],
        face_discoveries: &[],
        edge_discoveries: &[],
        attachments: &model.attachments,
    };
    for id in doc.feature_graph.ordered_ids() {
        let node = &model.nodes[id];
        let FeatureDefinition::Sketch(owner) = &node.definition else {
            continue;
        };
        if node.suppressed || doc.feature_nodes.iter().filter(|f| matches!(&f.definition, FeatureDefinition::Sketch(s) if s.sketch_id == owner.sketch_id)).count() != 1 { continue; }
        let sketch = &model.sketches[&owner.sketch_id];
        if !matches!(
            sketch.workplane,
            Workplane::Custom { .. } | Workplane::FaceRef { .. } | Workplane::DatumPlane { .. }
        ) {
            continue;
        }
        for profile in &sketch.profiles {
            let Some(profile_ref) = profile.profile_ref.as_deref() else {
                continue;
            };
            if profile.entity_ids.len() != 1 {
                continue;
            }
            let Some(SketchEntity::Circle(circle)) =
                sketch.find_entity(profile.entity_ids[0].as_str())
            else {
                continue;
            };
            if circle.base.construction {
                continue;
            }
            let opencad_sketch::Coord::Literal(radius) = circle.radius else {
                continue;
            };
            let standards = matching_standards(2. * radius);
            let Some(standard) = standards.first() else {
                continue;
            };
            for reference in &doc.semantic_refs {
                if reference.semantic.created_by != *stock {
                    continue;
                }
                let def = StandardHoleFeature {
                    hole: HoleFeature::on_face_ref(
                        id,
                        profile_ref,
                        ExtrudeExtent::Distance {
                            length: Length::from_meters(height.meters() / 2.),
                        },
                        stock,
                        reference.ref_id.as_str(),
                    ),
                    standard: standard.clone(),
                };
                let Ok(admitted) =
                    admit_owned_hole_inputs(&def, standard.dimensions().cut_diameter, &context)
                else {
                    continue;
                };
                let Some(plane) = admitted.profile.placement else {
                    continue;
                };
                let Some(c) = admitted.profile.circle else {
                    continue;
                };
                state.choices.push(HoleChoice {
                    target_feature: stock.clone(),
                    stock_name: model.nodes[stock].name.clone(),
                    face_ref: reference.ref_id.as_str().into(),
                    entry_role: reference.semantic.role.clone().unwrap_or_default(),
                    sketch_feature: id.clone(),
                    axis_name: node.name.clone(),
                    profile_ref: profile_ref.into(),
                    circle_id: circle.base.id.as_str().into(),
                    entry_center_m: plane.map_point(c.center_m[0], c.center_m[1]),
                    inward_direction: admitted.inward_direction,
                    bore_diameter_m: 2. * c.radius_m,
                    stock_thickness_m: admitted.stock.inward_thickness_m,
                    standards: standards.clone(),
                });
            }
        }
    }
    Ok(state)
}

pub fn inspect_document_hole_creation(path: &str) -> Result<HoleCreationState> {
    hole_creation_state_with_kernel(
        &read_ocad(path)?,
        &opencad_kernel_occt::OcctGeometryKernel::new(),
    )
}

fn dimension(doc: &OcadDocument, expr: &str, angle: bool) -> Result<f64> {
    let names = opencad_graph::parameter_names_in_expr(expr);
    let explicit = expr.split(|c: char| !c.is_alphabetic()).any(|u| {
        if angle {
            matches!(
                u,
                "deg" | "rad" | "degree" | "degrees" | "radian" | "radians"
            )
        } else {
            matches!(u, "m" | "mm" | "cm" | "in")
        }
    });
    if names.is_empty() && !explicit {
        return Err(invalid(
            "dimension needs explicit units or same-kind parameter names",
        ));
    }
    if names
        .iter()
        .any(|n| opencad_graph::param_eval::is_angle_parameter(n) != angle)
    {
        return Err(invalid("dimension parameter kind mismatch"));
    }
    let scope = opencad_graph::evaluate_param_graph_strict(&doc.parameters)?;
    let value = if angle {
        opencad_graph::eval_angle_expr_strict(expr, &scope)?
    } else {
        opencad_graph::eval_length_expr_strict(expr, &scope)?
    };
    if !value.is_finite()
        || value <= 1e-9
        || value > if angle { std::f64::consts::PI } else { 1000. }
    {
        return Err(invalid("dimension outside admitted positive range"));
    }
    Ok(value)
}

pub fn plan_hole_create_with_kernel<K: GeometryKernel>(
    doc: &OcadDocument,
    create: &HoleCreate,
    kernel: &K,
) -> Result<HoleCreatePlan> {
    if create.document_id != doc.metadata.id.as_str()
        || create.revision != feature_tree(doc)?.revision
    {
        return Err(invalid("stale or foreign document"));
    }
    if create.name.trim().is_empty() {
        return Err(invalid("name is required"));
    }
    let state = hole_creation_state_with_kernel(doc, kernel)?;
    let choice = state
        .choices
        .into_iter()
        .find(|c| {
            c.target_feature == create.target_feature
                && c.face_ref == create.face_ref
                && c.sketch_feature == create.sketch_feature
                && c.profile_ref == create.profile_ref
                && c.standards.contains(&create.standard)
        })
        .ok_or_else(|| {
            invalid(
                "choose a current admitted stock, owned entry, circle axis and matching standard",
            )
        })?;
    let bore_depth_m = dimension(doc, &create.depth, false)?;
    if bore_depth_m
        > choice.stock_thickness_m + 64. * f64::EPSILON * choice.stock_thickness_m.max(1.)
    {
        return Err(invalid("bore extends outside original stock"));
    }
    let hole = HoleFeature {
        sketch_feature: create.sketch_feature.clone(),
        profile_ref: create.profile_ref.clone(),
        target_feature: create.target_feature.clone(),
        face_ref: Some(create.face_ref.clone()),
        depth: ExtrudeExtent::Distance {
            length: Length::from_meters(bore_depth_m),
        },
        depth_expr: Some(create.depth.clone()),
    };
    let seat = match &create.seat {
        HoleSeatInput::None => None,
        HoleSeatInput::Counterbore { diameter, depth } => Some(StandardHoleSeat::Counterbore {
            diameter: Length::from_meters(dimension(doc, diameter, false)?),
            depth: Length::from_meters(dimension(doc, depth, false)?),
            diameter_expr: Some(diameter.clone()),
            depth_expr: Some(depth.clone()),
        }),
        HoleSeatInput::Countersink {
            diameter,
            included_angle,
        } => Some(StandardHoleSeat::Countersink {
            diameter: Length::from_meters(dimension(doc, diameter, false)?),
            included_angle: Angle::from_radians(dimension(doc, included_angle, true)?),
            diameter_expr: Some(diameter.clone()),
            included_angle_expr: Some(included_angle.clone()),
        }),
    };
    let (definition, seat_diameter_m, seat_depth_m, extra, first) = if let Some(seat) = seat {
        let def = SeatedStandardHoleFeature {
            sketch_feature: hole.sketch_feature,
            profile_ref: hole.profile_ref,
            target_feature: hole.target_feature,
            face_ref: create.face_ref.clone(),
            bore_depth: Length::from_meters(bore_depth_m),
            bore_depth_expr: Some(create.depth.clone()),
            standard: create.standard.clone(),
            seat,
        };
        def.validate_dimensions()?;
        let (diameter, depth) = def
            .seat
            .dimensions(create.standard.dimensions().cut_diameter)?;
        let (r, big, d) = (
            choice.bore_diameter_m / 2.,
            diameter.meters() / 2.,
            depth.meters(),
        );
        let (extra, first) = match def.seat {
            StandardHoleSeat::Counterbore { .. } => {
                let v = std::f64::consts::PI * (big * big - r * r) * d;
                (v, v * d / 2.)
            }
            StandardHoleSeat::Countersink { .. } => (
                std::f64::consts::PI * d * (big * big + big * r - 2. * r * r) / 3.,
                std::f64::consts::PI * d * d * (big * big + 2. * big * r - 3. * r * r) / 12.,
            ),
        };
        (
            FeatureDefinition::SeatedStandardHole(def),
            Some(diameter.meters()),
            Some(depth.meters()),
            extra,
            first,
        )
    } else {
        (
            FeatureDefinition::StandardHole(StandardHoleFeature {
                hole,
                standard: create.standard.clone(),
            }),
            None,
            None,
            0.,
            0.,
        )
    };
    let patch = DesignPatch::new(vec![PatchOperation::AddFeature {
        node: FeatureNode::new(&state.suggested_feature_id, &create.name, definition),
        position: FeaturePosition::end(),
    }])
    .with_revision_precondition(&document_design_state(doc))?;
    opencad_ai::ensure_patch_valid(&opencad_file::dry_run_patch_document(doc, &patch))?;
    let mut candidate = doc.clone();
    apply_patch_to_document(&mut candidate, &patch)?;
    let evidence = verify_editor_document(&candidate, kernel)?;
    // Query the actual original stock and candidate, including ordinary holes.
    let mut model = candidate.clone().into_part_model();
    model.regenerate(
        kernel,
        &FeatureRegistry::with_defaults(),
        Some(&candidate.parameters),
        Some(&candidate.semantic_refs),
    )?;
    let stock = model
        .outputs
        .get(&create.target_feature)
        .and_then(|o| o.body.as_ref())
        .ok_or_else(|| invalid("missing original physical stock"))?;
    let output = model
        .outputs
        .get(&state.suggested_feature_id)
        .and_then(|o| o.body.as_ref())
        .ok_or_else(|| invalid("missing hole physical output"))?;
    let before = kernel.mass_properties(stock, 1.)?;
    let after = kernel.mass_properties(output, 1.)?;
    let bore = std::f64::consts::PI * (choice.bore_diameter_m / 2.).powi(2) * bore_depth_m;
    let removed_volume_m3 = bore + extra;
    let expected = before.volume_m3 - removed_volume_m3;
    if !expected.is_finite()
        || expected <= 0.
        || removed_volume_m3 <= 64. * f64::EPSILON * before.volume_m3
        || !after.volume_m3.is_finite()
        || (after.volume_m3 - expected).abs() > expected * 1e-8
    {
        return Err(invalid("physical removal volume disagrees"));
    }
    for i in 0..3 {
        let removed_first = removed_volume_m3 * choice.entry_center_m[i]
            + choice.inward_direction[i] * (bore * bore_depth_m / 2. + first);
        if !after.center_of_mass[i].is_finite()
            || (after.center_of_mass[i]
                - (before.volume_m3 * before.center_of_mass[i] - removed_first) / expected)
                .abs()
                > 1e-9
        {
            return Err(invalid("physical first moment disagrees"));
        }
    }
    let a = kernel.bounding_box(stock)?;
    let b = kernel.bounding_box(output)?;
    if a.min
        .into_iter()
        .chain(a.max)
        .zip(b.min.into_iter().chain(b.max))
        .any(|(a, b)| (a - b).abs() > 3e-7)
    {
        return Err(invalid("hole changed outer stock bounds"));
    }
    let mesh = kernel.tessellate(output, &opencad_geometry::TessellationSettings::default())?;
    opencad_feature::seated_standard_hole::validate_owned_hole_mesh(&mesh)?;
    Ok(HoleCreatePlan {
        create: create.clone(),
        feature_id: state.suggested_feature_id,
        choice,
        bore_depth_m,
        seat_diameter_m,
        seat_depth_m,
        removed_volume_m3,
        patch,
        evidence,
    })
}

pub fn apply_hole_create_with_kernel<K: GeometryKernel>(
    doc: &mut OcadDocument,
    create: &HoleCreate,
    history: &mut DocumentHistory,
    kernel: &K,
) -> Result<FeatureEditResult> {
    let plan = plan_hole_create_with_kernel(doc, create, kernel)?;
    let mut candidate = doc.clone();
    let mut next = history.clone();
    apply_patch_with_history(
        &mut candidate,
        &plan.patch,
        &mut next,
        "Create admitted hole",
    )?;
    let result = FeatureEditResult {
        tree: feature_tree(&candidate)?,
        history: DocumentHistoryState::new(next.clone()),
        evidence: plan.evidence,
    };
    *doc = candidate;
    *history = next;
    Ok(result)
}

pub fn plan_document_hole_create(path: &str, create: &HoleCreate) -> Result<HoleCreatePlan> {
    let doc = read_ocad(path)?;
    let mut plan = plan_hole_create_with_kernel(
        &doc,
        create,
        &opencad_kernel_occt::OcctGeometryKernel::new(),
    )?;
    let mut candidate = doc;
    apply_patch_to_document(&mut candidate, &plan.patch)?;
    plan.evidence = verify_editor_document_native(&candidate)?;
    Ok(plan)
}

pub fn apply_document_hole_create(
    path: &str,
    create: &HoleCreate,
    history: Option<DocumentHistory>,
) -> Result<FeatureEditResult> {
    let mut doc = read_ocad(path)?;
    let result = apply_hole_create_with_kernel(
        &mut doc,
        create,
        &mut history.unwrap_or_default(),
        &opencad_kernel_occt::OcctGeometryKernel::new(),
    )?;
    verify_editor_document_native(&doc)?;
    let current = read_ocad(path)?;
    if current.metadata.id.as_str() != create.document_id
        || feature_tree(&current)?.revision != create.revision
    {
        return Err(invalid("document changed during verification"));
    }
    write_ocad(path, &doc)?;
    Ok(result)
}
