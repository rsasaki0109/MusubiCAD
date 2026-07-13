//! Assembly regeneration: resolve child parts, apply placements, build compound scene.

use std::path::{Path, PathBuf};

use opencad_core::{DocumentId, InstanceId, OpenCadError, Result};
use opencad_feature::{FeatureRegistry, PartModel};
use opencad_geometry::{
    BooleanOp, BoundingBox, GeometryKernel, KernelBody, MassProperties, MeshSet, RigidTransform,
    TessellationSettings,
};
use opencad_graph::ParamGraph;

use crate::component::{Component, ComponentSourceKind};
use crate::model::AssemblyModel;
use crate::pattern::expand_patterns;

const DEFAULT_DENSITY_KG_PER_M3: f64 = 2700.0;

/// Child part payload resolved from disk.
#[derive(Debug, Clone)]
pub struct ChildPart {
    pub parameters: ParamGraph,
    pub part: PartModel,
    pub semantic_refs: Vec<opencad_geometry::TopoRef>,
}

/// Child document resolved from disk (part or nested assembly).
#[derive(Debug, Clone)]
pub enum ResolvedChild {
    Part(Box<ChildPart>),
    Assembly {
        model: Box<AssemblyModel>,
        doc_id: DocumentId,
    },
}

/// Per-instance regeneration outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceRegenStatus {
    Ok,
    Failed(String),
}

/// Result of one placed instance after regeneration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceRegenResult {
    pub instance_id: InstanceId,
    pub status: InstanceRegenStatus,
    pub body: Option<KernelBody>,
}

/// Aggregated assembly scene after static regeneration.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyScene {
    pub instances: Vec<InstanceRegenResult>,
    pub compound_body: Option<KernelBody>,
    pub bounding_box: Option<BoundingBox>,
    pub mass: Option<MassProperties>,
}

/// Summary returned to CLI / tests.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyRegenReport {
    pub instances: Vec<InstanceRegenResult>,
    pub instance_count: usize,
    pub successful_instances: usize,
    pub scene: AssemblyScene,
    pub mate_solve: Option<crate::solve::AssemblySolveReport>,
}

/// Pair of placed instances whose common solid volume exceeds the requested tolerance.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyInterference {
    pub first: InstanceId,
    pub second: InstanceId,
    pub common_volume_m3: f64,
}

/// Detect solid interference using exact kernel boolean intersection.
pub fn detect_interferences<K: GeometryKernel>(
    kernel: &K,
    scene: &AssemblyScene,
    volume_tolerance_m3: f64,
) -> Result<Vec<AssemblyInterference>> {
    const BOUNDS_TOLERANCE_M: f64 = 1e-9;
    if !volume_tolerance_m3.is_finite() || volume_tolerance_m3 < 0.0 {
        return Err(OpenCadError::validation(
            "interference volume tolerance must be finite and non-negative",
        ));
    }
    let bodies = scene
        .instances
        .iter()
        .filter_map(|instance| instance.body.as_ref().map(|body| (instance, body)))
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    for first_index in 0..bodies.len() {
        for second_index in (first_index + 1)..bodies.len() {
            let (first, first_body) = bodies[first_index];
            let (second, second_body) = bodies[second_index];
            let first_bounds = kernel.bounding_box(first_body)?;
            let second_bounds = kernel.bounding_box(second_body)?;
            let separated = (0..3).any(|axis| {
                first_bounds.max[axis] <= second_bounds.min[axis] + BOUNDS_TOLERANCE_M
                    || second_bounds.max[axis] <= first_bounds.min[axis] + BOUNDS_TOLERANCE_M
            });
            if separated {
                continue;
            }
            let common = kernel.boolean(
                first_body.clone(),
                second_body.clone(),
                BooleanOp::Intersect,
            )?;
            let volume = kernel.mass_properties(&common, 1.0)?.volume_m3;
            if volume > volume_tolerance_m3 {
                result.push(AssemblyInterference {
                    first: first.instance_id.clone(),
                    second: second.instance_id.clone(),
                    common_volume_m3: volume,
                });
            }
        }
    }
    Ok(result)
}

pub fn resolve_component_path(assembly_root: &Path, source_path: &str) -> PathBuf {
    assembly_root.join(source_path)
}

pub fn regenerate_assembly<K: GeometryKernel>(
    model: &AssemblyModel,
    assembly_doc_id: &DocumentId,
    assembly_root: &Path,
    kernel: &K,
    registry: &FeatureRegistry,
    load_child: &mut dyn FnMut(&Path) -> Result<ResolvedChild>,
) -> Result<AssemblyRegenReport> {
    let expanded = expand_patterns(model)?;
    expanded.validate_no_self_reference(assembly_doc_id)?;

    let (model, mate_solve) = if expanded.mates.is_empty() {
        (expanded, None)
    } else {
        let (instances, report) = crate::solve::solve_assembly_mates(&expanded)?;
        let mut solved = expanded;
        solved.instances = instances;
        (solved, Some(report))
    };

    let mut instance_results = Vec::new();
    let mut placed_bodies = Vec::new();

    for instance in &model.instances {
        let Some(component) = model.component(&instance.component) else {
            instance_results.push(InstanceRegenResult {
                instance_id: instance.id.clone(),
                status: InstanceRegenStatus::Failed(format!(
                    "unknown component '{}'",
                    instance.component
                )),
                body: None,
            });
            continue;
        };

        match regenerate_instance(
            component,
            instance.placement.transform,
            assembly_root,
            kernel,
            registry,
            load_child,
        ) {
            Ok(body) => {
                placed_bodies.push(body.clone());
                instance_results.push(InstanceRegenResult {
                    instance_id: instance.id.clone(),
                    status: InstanceRegenStatus::Ok,
                    body: Some(body),
                });
            }
            Err(err) => {
                instance_results.push(InstanceRegenResult {
                    instance_id: instance.id.clone(),
                    status: InstanceRegenStatus::Failed(err.to_string()),
                    body: None,
                });
            }
        }
    }

    let successful_instances = instance_results
        .iter()
        .filter(|result| matches!(result.status, InstanceRegenStatus::Ok))
        .count();

    let compound_body = if placed_bodies.is_empty() {
        None
    } else {
        Some(kernel.make_compound(&placed_bodies)?)
    };

    let bounding_box = aggregate_bounding_box(kernel, &placed_bodies)?;
    let mass = aggregate_mass(kernel, &placed_bodies, DEFAULT_DENSITY_KG_PER_M3)?;

    let scene = AssemblyScene {
        instances: instance_results.clone(),
        compound_body,
        bounding_box,
        mass,
    };

    Ok(AssemblyRegenReport {
        instance_count: instance_results.len(),
        successful_instances,
        instances: instance_results,
        scene,
        mate_solve,
    })
}

fn regenerate_instance<K: GeometryKernel>(
    component: &Component,
    transform: RigidTransform,
    assembly_root: &Path,
    kernel: &K,
    registry: &FeatureRegistry,
    load_child: &mut dyn FnMut(&Path) -> Result<ResolvedChild>,
) -> Result<KernelBody> {
    let child_path = resolve_component_path(assembly_root, &component.source_path);
    if !child_path.exists() {
        return Err(OpenCadError::not_found(format!(
            "child document '{}' not found at '{}'",
            component.id,
            child_path.display()
        )));
    }

    let child_root = assembly_root_for_path(&child_path);
    let body = match load_child(&child_path)? {
        ResolvedChild::Part(mut child) => {
            if component.source_kind == ComponentSourceKind::Assembly {
                return Err(OpenCadError::validation(format!(
                    "component '{}' expects assembly but '{}' is a part document",
                    component.id,
                    child_path.display()
                )));
            }
            child
                .part
                .regenerate(
                    kernel,
                    registry,
                    Some(&child.parameters),
                    Some(&child.semantic_refs),
                )
                .map_err(|err| {
                    OpenCadError::Other(format!(
                        "child part '{}' regen failed: {err}",
                        component.id
                    ))
                })?;

            child
                .part
                .active_body()
                .ok_or_else(|| {
                    OpenCadError::validation(format!(
                        "child part '{}' has no active solid body",
                        component.id
                    ))
                })?
                .clone()
        }
        ResolvedChild::Assembly { model, doc_id } => {
            if component.source_kind == ComponentSourceKind::Part {
                return Err(OpenCadError::validation(format!(
                    "component '{}' expects part but '{}' is an assembly document",
                    component.id,
                    child_path.display()
                )));
            }
            let report =
                regenerate_assembly(&model, &doc_id, &child_root, kernel, registry, load_child)?;
            report.scene.compound_body.ok_or_else(|| {
                OpenCadError::validation(format!(
                    "child assembly '{}' produced no geometry",
                    component.id
                ))
            })?
        }
    };

    kernel.transform_body(body, transform)
}

fn assembly_root_for_path(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf())
    }
}

fn aggregate_bounding_box<K: GeometryKernel>(
    kernel: &K,
    bodies: &[KernelBody],
) -> Result<Option<BoundingBox>> {
    let mut merged: Option<BoundingBox> = None;
    for body in bodies {
        let bbox = kernel.bounding_box(body)?;
        merged = Some(match merged {
            None => bbox,
            Some(current) => merge_bounding_boxes(current, bbox),
        });
    }
    Ok(merged)
}

fn aggregate_mass<K: GeometryKernel>(
    kernel: &K,
    bodies: &[KernelBody],
    density_kg_per_m3: f64,
) -> Result<Option<MassProperties>> {
    let mut total_volume = 0.0;
    let mut total_area = 0.0;
    let mut total_mass = 0.0;
    let mut weighted_com = [0.0_f64; 3];

    for body in bodies {
        let props = kernel.mass_properties(body, density_kg_per_m3)?;
        total_volume += props.volume_m3;
        total_area += props.area_m2;
        total_mass += props.mass_kg;
        for (axis, weight) in weighted_com.iter_mut().enumerate() {
            *weight += props.center_of_mass[axis] * props.mass_kg;
        }
    }

    if bodies.is_empty() {
        return Ok(None);
    }

    let center_of_mass = if total_mass > 0.0 {
        [
            weighted_com[0] / total_mass,
            weighted_com[1] / total_mass,
            weighted_com[2] / total_mass,
        ]
    } else {
        [0.0, 0.0, 0.0]
    };

    Ok(Some(MassProperties {
        volume_m3: total_volume,
        area_m2: total_area,
        mass_kg: total_mass,
        center_of_mass,
    }))
}

fn merge_bounding_boxes(a: BoundingBox, b: BoundingBox) -> BoundingBox {
    BoundingBox {
        min: [
            a.min[0].min(b.min[0]),
            a.min[1].min(b.min[1]),
            a.min[2].min(b.min[2]),
        ],
        max: [
            a.max[0].max(b.max[0]),
            a.max[1].max(b.max[1]),
            a.max[2].max(b.max[2]),
        ],
    }
}

pub fn tessellate_assembly_scene<K: GeometryKernel>(
    kernel: &K,
    scene: &AssemblyScene,
    settings: &TessellationSettings,
) -> Result<MeshSet> {
    Ok(MeshSet::merge(
        &tessellate_assembly_instances(kernel, scene, settings)?
            .into_iter()
            .map(|instance| instance.mesh_set)
            .collect::<Vec<_>>(),
    ))
}

/// Per-instance tessellation for multi-color viewport rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceMesh {
    pub instance_id: InstanceId,
    pub mesh_set: MeshSet,
}

pub fn tessellate_assembly_instances<K: GeometryKernel>(
    kernel: &K,
    scene: &AssemblyScene,
    settings: &TessellationSettings,
) -> Result<Vec<InstanceMesh>> {
    let mut meshes = Vec::new();
    for instance in &scene.instances {
        let Some(body) = instance.body.as_ref() else {
            continue;
        };
        meshes.push(InstanceMesh {
            instance_id: instance.instance_id.clone(),
            mesh_set: kernel.tessellate(body, settings)?,
        });
    }

    if meshes.is_empty() {
        return Err(OpenCadError::validation(
            "assembly scene has no tessellatable bodies",
        ));
    }

    Ok(meshes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::{Instance, Placement};
    use opencad_core::{ComponentId, DocumentId, InstanceId};
    use opencad_feature::bracket_base_plate;
    use opencad_geometry::MockGeometryKernel;
    use tempfile::tempdir;

    fn child_part() -> ChildPart {
        ChildPart {
            parameters: opencad_graph::ParamGraph::new(),
            part: bracket_base_plate().expect("model"),
            semantic_refs: Vec::new(),
        }
    }

    #[test]
    fn regenerates_two_instances() -> Result<()> {
        let model = AssemblyModel {
            components: vec![Component::new(
                ComponentId::new("component:bracket")?,
                "parts/bracket.ocad.d",
                DocumentId::new("doc:bracket_001")?,
            )],
            instances: vec![
                Instance::new(
                    InstanceId::new("instance:left")?,
                    ComponentId::new("component:bracket")?,
                    Placement::identity(),
                    "Left",
                ),
                Instance::new(
                    InstanceId::new("instance:right")?,
                    ComponentId::new("component:bracket")?,
                    Placement::new(RigidTransform::from_translation([0.2, 0.0, 0.0])),
                    "Right",
                ),
            ],
            mates: Vec::new(),
            connectors: Vec::new(),
            patterns: Vec::new(),
        };

        let dir = tempdir().expect("tempdir");
        let child_path = dir.path().join("parts/bracket.ocad.d");
        std::fs::create_dir_all(&child_path).expect("mkdir");

        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let assembly_id = DocumentId::new("doc:assembly_001")?;

        let mut loader = |_path: &Path| Ok(ResolvedChild::Part(Box::new(child_part())));
        let report = regenerate_assembly(
            &model,
            &assembly_id,
            dir.path(),
            &kernel,
            &registry,
            &mut loader,
        )?;

        assert_eq!(report.instance_count, 2);
        assert_eq!(report.successful_instances, 2);
        assert!(report.scene.compound_body.is_some());
        assert!(report.scene.mass.is_some());
        Ok(())
    }

    #[test]
    fn missing_child_reports_instance_error() -> Result<()> {
        let model = AssemblyModel {
            components: vec![Component::new(
                ComponentId::new("component:bracket")?,
                "missing.ocad.d",
                DocumentId::new("doc:bracket_001")?,
            )],
            instances: vec![Instance::new(
                InstanceId::new("instance:only")?,
                ComponentId::new("component:bracket")?,
                Placement::identity(),
                "Only",
            )],
            mates: Vec::new(),
            connectors: Vec::new(),
            patterns: Vec::new(),
        };

        let dir = tempdir().expect("tempdir");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let assembly_id = DocumentId::new("doc:assembly_001")?;

        let mut loader = |_path: &Path| Ok(ResolvedChild::Part(Box::new(child_part())));
        let report = regenerate_assembly(
            &model,
            &assembly_id,
            dir.path(),
            &kernel,
            &registry,
            &mut loader,
        )?;

        assert_eq!(report.successful_instances, 0);
        assert!(matches!(
            report.instances[0].status,
            InstanceRegenStatus::Failed(_)
        ));
        Ok(())
    }

    #[test]
    fn detects_common_solid_volume_above_tolerance() -> Result<()> {
        let scene = AssemblyScene {
            instances: vec![
                InstanceRegenResult {
                    instance_id: InstanceId::new("instance:first")?,
                    status: InstanceRegenStatus::Ok,
                    body: Some(KernelBody::new(3)),
                },
                InstanceRegenResult {
                    instance_id: InstanceId::new("instance:second")?,
                    status: InstanceRegenStatus::Ok,
                    body: Some(KernelBody::new(7)),
                },
            ],
            compound_body: None,
            bounding_box: None,
            mass: None,
        };
        let hits = detect_interferences(&MockGeometryKernel::new(), &scene, 1e-12)?;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].first.as_str(), "instance:first");
        assert!(hits[0].common_volume_m3 > 0.0);
        Ok(())
    }
}
