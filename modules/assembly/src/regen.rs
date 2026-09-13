//! Assembly regeneration: resolve child parts, apply placements, build compound scene.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use opencad_core::{DocumentId, InstanceId, OpenCadError, Result};
use opencad_feature::{FeatureRegistry, PartModel};
use opencad_geometry::{
    BoundingBox, GeometryKernel, KernelBody, MassProperties, MeshSet, RigidTransform,
    TessellationSettings,
};
use opencad_graph::ParamGraph;
use serde::{Deserialize, Serialize};

use crate::component::{Component, ComponentSourceKind};
use crate::model::AssemblyModel;
use crate::pattern::expand_patterns;

const DEFAULT_DENSITY_KG_PER_M3: f64 = 2700.0;
pub const DEFAULT_INTERFERENCE_BOUNDS_TOLERANCE_M: f64 = 1e-9;
pub const DEFAULT_INTERFERENCE_VOLUME_TOLERANCE_M3: f64 = 1e-12;

/// Child part payload resolved from disk.
#[derive(Debug, Clone)]
pub struct ChildPart {
    pub doc_id: DocumentId,
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

/// Explicit geometric tolerances for assembly interference detection.
///
/// Bounds that overlap by no more than `bounds_tolerance_m` are treated as
/// contact. An exact Boolean intersection is reported only when its volume is
/// strictly greater than `volume_tolerance_m3`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AssemblyInterferenceTolerance {
    /// Linear broad-phase tolerance in meters.
    pub bounds_tolerance_m: f64,
    /// Exact common-solid threshold in cubic meters.
    pub volume_tolerance_m3: f64,
}

impl Default for AssemblyInterferenceTolerance {
    fn default() -> Self {
        Self {
            bounds_tolerance_m: DEFAULT_INTERFERENCE_BOUNDS_TOLERANCE_M,
            volume_tolerance_m3: DEFAULT_INTERFERENCE_VOLUME_TOLERANCE_M3,
        }
    }
}

impl AssemblyInterferenceTolerance {
    pub fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("bounds_tolerance_m", self.bounds_tolerance_m),
            ("volume_tolerance_m3", self.volume_tolerance_m3),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(OpenCadError::validation(format!(
                    "assembly interference tolerance '{name}' must be finite and strictly positive"
                )));
            }
        }
        Ok(())
    }
}

/// Detect solid interference using exact kernel boolean intersection.
pub fn detect_interferences<K: GeometryKernel>(
    kernel: &K,
    scene: &AssemblyScene,
    volume_tolerance_m3: f64,
) -> Result<Vec<AssemblyInterference>> {
    detect_interferences_with_tolerance(
        kernel,
        scene,
        AssemblyInterferenceTolerance {
            volume_tolerance_m3,
            ..AssemblyInterferenceTolerance::default()
        },
    )
}

/// Detect solid interference with explicit linear and volumetric tolerances.
pub fn detect_interferences_with_tolerance<K: GeometryKernel>(
    kernel: &K,
    scene: &AssemblyScene,
    tolerance: AssemblyInterferenceTolerance,
) -> Result<Vec<AssemblyInterference>> {
    tolerance.validate()?;
    let mut bodies = scene
        .instances
        .iter()
        .filter_map(|instance| instance.body.as_ref().map(|body| (instance, body)))
        .collect::<Vec<_>>();
    bodies.sort_by(|(first, _), (second, _)| {
        first.instance_id.as_str().cmp(second.instance_id.as_str())
    });
    let mut result = Vec::new();
    for first_index in 0..bodies.len() {
        for second_index in (first_index + 1)..bodies.len() {
            let (first, first_body) = bodies[first_index];
            let (second, second_body) = bodies[second_index];
            let first_bounds = kernel.bounding_box(first_body)?;
            let second_bounds = kernel.bounding_box(second_body)?;
            let separated = (0..3).any(|axis| {
                first_bounds.max[axis] <= second_bounds.min[axis] + tolerance.bounds_tolerance_m
                    || second_bounds.max[axis]
                        <= first_bounds.min[axis] + tolerance.bounds_tolerance_m
            });
            if separated {
                continue;
            }
            let volume = kernel.intersection_volume(first_body, second_body)?;
            if volume > tolerance.volume_tolerance_m3 {
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

/// Resolve and validate a component path against an existing assembly root.
///
/// The lexical path must be relative, and the canonical child must remain
/// inside the canonical assembly root so symlinked children cannot escape the
/// document directory.
pub fn validate_component_path(assembly_root: &Path, source_path: &str) -> Result<PathBuf> {
    crate::component::Component::validate_source_path(source_path)?;
    let canonical_root = fs::canonicalize(assembly_root).map_err(|err| {
        OpenCadError::not_found(format!(
            "assembly root '{}' is not accessible: {err}",
            assembly_root.display()
        ))
    })?;
    let joined = resolve_component_path(assembly_root, source_path);
    let canonical_child = fs::canonicalize(&joined).map_err(|err| {
        OpenCadError::not_found(format!(
            "child document '{}' not found at '{}': {err}",
            source_path,
            joined.display()
        ))
    })?;
    if !canonical_child.starts_with(&canonical_root) {
        return Err(OpenCadError::validation(format!(
            "child document '{}' resolves outside assembly root '{}'",
            source_path,
            canonical_root.display()
        )));
    }
    Ok(canonical_child)
}

pub fn regenerate_assembly<K: GeometryKernel>(
    model: &AssemblyModel,
    assembly_doc_id: &DocumentId,
    assembly_root: &Path,
    kernel: &K,
    registry: &FeatureRegistry,
    load_child: &mut dyn FnMut(&Path) -> Result<ResolvedChild>,
) -> Result<AssemblyRegenReport> {
    let mut stack = Vec::new();
    regenerate_assembly_with_stack(
        model,
        assembly_doc_id,
        AssemblyPaths {
            root: assembly_root,
            document: assembly_root,
        },
        kernel,
        registry,
        load_child,
        &mut stack,
    )
}

#[derive(Debug, Clone)]
struct AssemblyStackEntry {
    doc_id: DocumentId,
    canonical_path: PathBuf,
}

struct AssemblyPaths<'a> {
    root: &'a Path,
    document: &'a Path,
}

fn regenerate_assembly_with_stack<K: GeometryKernel>(
    model: &AssemblyModel,
    assembly_doc_id: &DocumentId,
    paths: AssemblyPaths<'_>,
    kernel: &K,
    registry: &FeatureRegistry,
    load_child: &mut dyn FnMut(&Path) -> Result<ResolvedChild>,
    stack: &mut Vec<AssemblyStackEntry>,
) -> Result<AssemblyRegenReport> {
    let expanded = expand_patterns(model)?;
    expanded.validate(assembly_doc_id)?;
    let canonical_root = fs::canonicalize(paths.root).map_err(|err| {
        OpenCadError::not_found(format!(
            "assembly root '{}' is not accessible: {err}",
            paths.root.display()
        ))
    })?;
    let canonical_path = fs::canonicalize(paths.document).map_err(|err| {
        OpenCadError::not_found(format!(
            "assembly document '{}' is not accessible: {err}",
            paths.document.display()
        ))
    })?;
    if let Some(previous) = stack
        .iter()
        .find(|entry| entry.doc_id == *assembly_doc_id || entry.canonical_path == canonical_path)
    {
        return Err(OpenCadError::validation(format!(
            "assembly cycle detected at document '{}' (path '{}', already in '{}')",
            assembly_doc_id,
            canonical_path.display(),
            previous.canonical_path.display()
        )));
    }
    validate_duplicate_component_paths(&expanded, &canonical_root)?;
    stack.push(AssemblyStackEntry {
        doc_id: assembly_doc_id.clone(),
        canonical_path,
    });

    let result = (|| {
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
                paths.root,
                kernel,
                registry,
                load_child,
                stack,
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
    })();
    stack.pop();
    result
}

fn validate_duplicate_component_paths(model: &AssemblyModel, canonical_root: &Path) -> Result<()> {
    let mut paths = BTreeMap::new();
    for component in &model.components {
        let canonical_path = match validate_component_path(canonical_root, &component.source_path) {
            Ok(path) => path,
            Err(OpenCadError::NotFound(_)) => {
                // Missing children are reported per instance during regeneration.
                continue;
            }
            Err(error) => return Err(error),
        };
        if let Some(previous_id) = paths.insert(canonical_path.clone(), component.id.clone()) {
            if previous_id != component.id {
                return Err(OpenCadError::validation(format!(
                    "components '{}' and '{}' resolve to the same child document '{}'",
                    previous_id,
                    component.id,
                    canonical_path.display()
                )));
            }
        }
    }
    Ok(())
}

fn regenerate_instance<K: GeometryKernel>(
    component: &Component,
    transform: RigidTransform,
    assembly_root: &Path,
    kernel: &K,
    registry: &FeatureRegistry,
    load_child: &mut dyn FnMut(&Path) -> Result<ResolvedChild>,
    stack: &mut Vec<AssemblyStackEntry>,
) -> Result<KernelBody> {
    let child_path = validate_component_path(assembly_root, &component.source_path)?;

    let child_root = assembly_root_for_path(&child_path);
    let body = match load_child(&child_path)? {
        ResolvedChild::Part(mut child) => {
            if child.doc_id != component.source_doc {
                return Err(OpenCadError::validation(format!(
                    "component '{}' expects document '{}' but loaded part '{}'",
                    component.id, component.source_doc, child.doc_id
                )));
            }
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
            if doc_id != component.source_doc {
                return Err(OpenCadError::validation(format!(
                    "component '{}' expects document '{}' but loaded assembly '{}'",
                    component.id, component.source_doc, doc_id
                )));
            }
            if component.source_kind == ComponentSourceKind::Part {
                return Err(OpenCadError::validation(format!(
                    "component '{}' expects part but '{}' is an assembly document",
                    component.id,
                    child_path.display()
                )));
            }
            let report = regenerate_assembly_with_stack(
                &model,
                &doc_id,
                AssemblyPaths {
                    root: &child_root,
                    document: &child_path,
                },
                kernel,
                registry,
                load_child,
                stack,
            )?;
            report.scene.compound_body.ok_or_else(|| {
                let detail = report
                    .instances
                    .iter()
                    .find_map(|instance| match &instance.status {
                        InstanceRegenStatus::Failed(message) => Some(message.as_str()),
                        InstanceRegenStatus::Ok => None,
                    })
                    .unwrap_or("no geometry");
                OpenCadError::validation(format!(
                    "child assembly '{}' failed: {detail}",
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
    use std::cell::Cell;
    use tempfile::tempdir;

    fn child_part() -> ChildPart {
        ChildPart {
            doc_id: DocumentId::new("doc:bracket_001").expect("id"),
            parameters: opencad_graph::ParamGraph::new(),
            part: bracket_base_plate().expect("model"),
            semantic_refs: Vec::new(),
        }
    }

    fn assembly_component(
        id: &str,
        source_path: &str,
        source_doc: &str,
        source_kind: ComponentSourceKind,
    ) -> Result<Component> {
        let mut component = Component::new(
            ComponentId::new(id)?,
            source_path,
            DocumentId::new(source_doc)?,
        );
        component.source_kind = source_kind;
        Ok(component)
    }

    fn one_instance(component: &str, id: &str) -> Result<Instance> {
        Ok(Instance::new(
            InstanceId::new(id)?,
            ComponentId::new(component)?,
            Placement::identity(),
            id,
        ))
    }

    fn empty_assembly(component: Component, instance: Instance) -> AssemblyModel {
        AssemblyModel {
            components: vec![component],
            instances: vec![instance],
            mates: Vec::new(),
            connectors: Vec::new(),
            patterns: Vec::new(),
        }
    }

    fn touch(path: &Path) {
        std::fs::write(path, b"fixture").expect("touch fixture");
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
    fn indirect_nested_cycle_is_localized_and_reports_cycle() -> Result<()> {
        let root = tempdir().expect("tempdir");
        let a_path = root.path().join("a.ocad");
        let b_path = root.path().join("b.ocad");
        touch(&a_path);
        touch(&b_path);

        let a_id = DocumentId::new("doc:a")?;
        let b_id = DocumentId::new("doc:b")?;
        let model_a = empty_assembly(
            assembly_component(
                "component:b",
                "b.ocad",
                "doc:b",
                ComponentSourceKind::Assembly,
            )?,
            one_instance("component:b", "instance:b")?,
        );
        let model_b = empty_assembly(
            assembly_component(
                "component:a",
                "a.ocad",
                "doc:a",
                ComponentSourceKind::Assembly,
            )?,
            one_instance("component:a", "instance:a")?,
        );
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let mut loader = |path: &Path| {
            if path.file_name().and_then(|name| name.to_str()) == Some("b.ocad") {
                Ok(ResolvedChild::Assembly {
                    model: Box::new(model_b.clone()),
                    doc_id: b_id.clone(),
                })
            } else {
                Ok(ResolvedChild::Assembly {
                    model: Box::new(model_a.clone()),
                    doc_id: a_id.clone(),
                })
            }
        };

        let report = regenerate_assembly(
            &model_a,
            &a_id,
            root.path(),
            &kernel,
            &registry,
            &mut loader,
        )?;
        assert_eq!(report.successful_instances, 0);
        assert!(matches!(
            &report.instances[0].status,
            InstanceRegenStatus::Failed(message) if message.contains("cycle")
        ));
        Ok(())
    }

    #[test]
    fn sibling_nested_assembly_reuse_is_allowed() -> Result<()> {
        let root = tempdir().expect("tempdir");
        let nested_path = root.path().join("nested.ocad");
        let part_path = root.path().join("part.ocad");
        touch(&nested_path);
        touch(&part_path);
        let nested_id = DocumentId::new("doc:nested")?;
        let part_id = DocumentId::new("doc:bracket_001")?;
        let nested_model = empty_assembly(
            assembly_component(
                "component:part",
                "part.ocad",
                "doc:bracket_001",
                ComponentSourceKind::Part,
            )?,
            one_instance("component:part", "instance:part")?,
        );
        let parent = AssemblyModel {
            components: vec![assembly_component(
                "component:nested",
                "nested.ocad",
                "doc:nested",
                ComponentSourceKind::Assembly,
            )?],
            instances: vec![
                one_instance("component:nested", "instance:first")?,
                one_instance("component:nested", "instance:second")?,
            ],
            mates: Vec::new(),
            connectors: Vec::new(),
            patterns: Vec::new(),
        };
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let mut loader = |path: &Path| {
            if path.file_name().and_then(|name| name.to_str()) == Some("nested.ocad") {
                Ok(ResolvedChild::Assembly {
                    model: Box::new(nested_model.clone()),
                    doc_id: nested_id.clone(),
                })
            } else {
                Ok(ResolvedChild::Part(Box::new(ChildPart {
                    doc_id: part_id.clone(),
                    ..child_part()
                })))
            }
        };
        let report = regenerate_assembly(
            &parent,
            &DocumentId::new("doc:parent")?,
            root.path(),
            &kernel,
            &registry,
            &mut loader,
        )?;
        assert_eq!(report.successful_instances, 2);
        Ok(())
    }

    #[test]
    fn source_document_mismatch_is_reported_for_part_and_assembly() -> Result<()> {
        let root = tempdir().expect("tempdir");
        let part_path = root.path().join("part.ocad");
        let assembly_path = root.path().join("nested.ocad");
        touch(&part_path);
        touch(&assembly_path);
        let part_component = Component::new(
            ComponentId::new("component:part")?,
            "part.ocad",
            DocumentId::new("doc:expected_part")?,
        );
        let assembly_component = assembly_component(
            "component:assembly",
            "nested.ocad",
            "doc:expected_assembly",
            ComponentSourceKind::Assembly,
        )?;
        let model = AssemblyModel {
            components: vec![part_component, assembly_component],
            instances: vec![
                one_instance("component:part", "instance:part")?,
                one_instance("component:assembly", "instance:assembly")?,
            ],
            mates: Vec::new(),
            connectors: Vec::new(),
            patterns: Vec::new(),
        };
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let actual_part_id = DocumentId::new("doc:actual_part")?;
        let actual_assembly_id = DocumentId::new("doc:actual_assembly")?;
        let mut loader = |path: &Path| {
            if path.file_name().and_then(|name| name.to_str()) == Some("part.ocad") {
                Ok(ResolvedChild::Part(Box::new(ChildPart {
                    doc_id: actual_part_id.clone(),
                    ..child_part()
                })))
            } else {
                Ok(ResolvedChild::Assembly {
                    model: Box::new(AssemblyModel::default()),
                    doc_id: actual_assembly_id.clone(),
                })
            }
        };
        let report = regenerate_assembly(
            &model,
            &DocumentId::new("doc:parent")?,
            root.path(),
            &kernel,
            &registry,
            &mut loader,
        )?;
        assert_eq!(report.successful_instances, 0);
        assert!(report.instances.iter().all(|instance| {
            matches!(&instance.status, InstanceRegenStatus::Failed(message) if message.contains("expects document"))
        }));
        Ok(())
    }

    #[test]
    fn failed_child_can_be_retried_without_mutating_the_model() -> Result<()> {
        let root = tempdir().expect("tempdir");
        let child_path = root.path().join("part.ocad");
        touch(&child_path);
        let model = empty_assembly(
            Component::new(
                ComponentId::new("component:part")?,
                "part.ocad",
                DocumentId::new("doc:bracket_001")?,
            ),
            one_instance("component:part", "instance:part")?,
        );
        let original = model.clone();
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let attempts = Cell::new(0);
        let mut loader = |_path: &Path| {
            let attempt = attempts.get();
            attempts.set(attempt + 1);
            if attempt == 0 {
                Err(OpenCadError::validation("transient child failure"))
            } else {
                Ok(ResolvedChild::Part(Box::new(child_part())))
            }
        };
        let first = regenerate_assembly(
            &model,
            &DocumentId::new("doc:parent")?,
            root.path(),
            &kernel,
            &registry,
            &mut loader,
        )?;
        assert_eq!(first.successful_instances, 0);
        let second = regenerate_assembly(
            &model,
            &DocumentId::new("doc:parent")?,
            root.path(),
            &kernel,
            &registry,
            &mut loader,
        )?;
        assert_eq!(second.successful_instances, 1);
        assert_eq!(model, original);
        Ok(())
    }

    #[test]
    fn duplicate_canonical_component_paths_are_rejected() -> Result<()> {
        let root = tempdir().expect("tempdir");
        let parts = root.path().join("parts");
        std::fs::create_dir_all(&parts).expect("parts directory");
        touch(&parts.join("shared.ocad"));
        let model = AssemblyModel {
            components: vec![
                Component::new(
                    ComponentId::new("component:first")?,
                    "parts/shared.ocad",
                    DocumentId::new("doc:first")?,
                ),
                Component::new(
                    ComponentId::new("component:second")?,
                    "parts/./shared.ocad",
                    DocumentId::new("doc:second")?,
                ),
            ],
            instances: Vec::new(),
            mates: Vec::new(),
            connectors: Vec::new(),
            patterns: Vec::new(),
        };
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let mut loader = |_path: &Path| Ok(ResolvedChild::Part(Box::new(child_part())));
        let error = regenerate_assembly(
            &model,
            &DocumentId::new("doc:parent")?,
            root.path(),
            &kernel,
            &registry,
            &mut loader,
        )
        .expect_err("duplicate canonical path");
        assert!(error.to_string().contains("same child document"));
        Ok(())
    }

    #[test]
    fn symlink_escape_is_rejected_when_supported() -> Result<()> {
        let root = tempdir().expect("tempdir");
        let outside = tempdir().expect("outside tempdir");
        let outside_file = outside.path().join("outside.ocad");
        let link = root.path().join("link.ocad");
        touch(&outside_file);
        #[cfg(unix)]
        let link_result = std::os::unix::fs::symlink(&outside_file, &link);
        #[cfg(windows)]
        let link_result = std::os::windows::fs::symlink_file(&outside_file, &link);
        if link_result.is_err() {
            return Ok(());
        }
        let error = validate_component_path(root.path(), "link.ocad").expect_err("symlink escape");
        assert!(error.to_string().contains("outside assembly root"));
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

    #[test]
    fn interference_policy_requires_positive_explicit_units() {
        assert!(AssemblyInterferenceTolerance::default().validate().is_ok());
        assert!(AssemblyInterferenceTolerance {
            bounds_tolerance_m: 0.0,
            ..AssemblyInterferenceTolerance::default()
        }
        .validate()
        .is_err());
        assert!(AssemblyInterferenceTolerance {
            volume_tolerance_m3: f64::NAN,
            ..AssemblyInterferenceTolerance::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn interference_pairs_are_ordered_and_threshold_is_exclusive() -> Result<()> {
        let scene = AssemblyScene {
            instances: vec![
                InstanceRegenResult {
                    instance_id: InstanceId::new("instance:z")?,
                    status: InstanceRegenStatus::Ok,
                    body: Some(KernelBody::new(7)),
                },
                InstanceRegenResult {
                    instance_id: InstanceId::new("instance:a")?,
                    status: InstanceRegenStatus::Ok,
                    body: Some(KernelBody::new(3)),
                },
                InstanceRegenResult {
                    instance_id: InstanceId::new("instance:m")?,
                    status: InstanceRegenStatus::Ok,
                    body: Some(KernelBody::new(5)),
                },
            ],
            compound_body: None,
            bounding_box: None,
            mass: None,
        };
        let kernel = MockGeometryKernel::new();
        let hits = detect_interferences_with_tolerance(
            &kernel,
            &scene,
            AssemblyInterferenceTolerance::default(),
        )?;
        let pairs = hits
            .iter()
            .map(|hit| (hit.first.as_str(), hit.second.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            pairs,
            vec![
                ("instance:a", "instance:m"),
                ("instance:a", "instance:z"),
                ("instance:m", "instance:z")
            ]
        );

        let exact_common_volume_m3 = (3.0_f64 * 0.001).powi(3);
        let at_threshold = detect_interferences_with_tolerance(
            &kernel,
            &AssemblyScene {
                instances: scene.instances[0..2].to_vec(),
                compound_body: None,
                bounding_box: None,
                mass: None,
            },
            AssemblyInterferenceTolerance {
                volume_tolerance_m3: exact_common_volume_m3,
                ..AssemblyInterferenceTolerance::default()
            },
        )?;
        assert!(at_threshold.is_empty());
        Ok(())
    }
}
