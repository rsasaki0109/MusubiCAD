//! Read-only, flattened bill of materials from verified source snapshots (ADR-047).
use std::collections::BTreeMap;

use crate::{expand_patterns, AssemblyModel, ComponentSourceKind};
use opencad_core::{DocumentId, InstanceId, OpenCadError, Result};
use serde::{Deserialize, Serialize};

pub const BOM_MAX_DEPTH: usize = 64;
pub const BOM_MAX_SOURCES: usize = 10_000;
pub const BOM_MAX_OCCURRENCES: usize = 100_000;

/// Caller-verified source. Keys are canonical root-relative paths.
/// `component_sources` maps local component IDs to resolved catalog keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BomSource {
    pub doc_id: DocumentId,
    pub name: String,
    pub assembly: Option<AssemblyModel>,
    pub component_sources: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BomRow {
    pub source_doc: DocumentId,
    pub name: String,
    pub source_path: String,
    /// Dimensionless count of placed leaf-part occurrences.
    pub quantity: usize,
    /// Root-to-leaf IDs retain their local namespaces.
    pub instance_paths: Vec<Vec<InstanceId>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BillOfMaterials {
    pub assembly_doc: DocumentId,
    pub declared_component_count: usize,
    pub top_level_instance_count: usize,
    pub leaf_part_count: usize,
    pub leaf_instance_count: usize,
    pub rows: Vec<BomRow>,
}

/// Check expansion budgets before allocating pattern instances.
pub fn bom_expanded_model(model: &AssemblyModel, id: &DocumentId) -> Result<AssemblyModel> {
    model.validate(id)?;
    let count = model
        .patterns
        .iter()
        .try_fold(model.instances.len(), |n, p| n.checked_add(p.count));
    let Some(count) = count else {
        return Err(invalid("pattern quantity overflow"));
    };
    if count > BOM_MAX_OCCURRENCES {
        return Err(invalid("pattern expansion exceeds 100000 instances"));
    }
    let expanded = expand_patterns(model)?.sorted_deterministic();
    expanded.validate(id)?;
    Ok(expanded)
}

/// Count used leaves without filesystem/kernel access or model mutation.
pub fn build_bill_of_materials(
    root_key: &str,
    catalog: &BTreeMap<String, BomSource>,
) -> Result<BillOfMaterials> {
    if catalog.len() > BOM_MAX_SOURCES {
        return Err(invalid("source catalog exceeds 10000 documents"));
    }
    let root = catalog
        .get(root_key)
        .ok_or_else(|| invalid("root source missing"))?;
    let model = root
        .assembly
        .as_ref()
        .ok_or_else(|| invalid("root must be an assembly"))?;
    let top = bom_expanded_model(model, &root.doc_id)?;
    let mut state = BomState::default();
    walk(root_key, catalog, &mut state, &mut Vec::new())?;
    let mut rows: Vec<_> = state.rows.into_values().collect();
    rows.sort_by(|a, b| {
        (a.source_doc.as_str(), &a.source_path).cmp(&(b.source_doc.as_str(), &b.source_path))
    });
    for row in &mut rows {
        row.instance_paths.sort_by(|a, b| {
            a.iter()
                .map(InstanceId::as_str)
                .cmp(b.iter().map(InstanceId::as_str))
        });
    }
    let total = rows
        .iter()
        .try_fold(0_usize, |n, r| n.checked_add(r.quantity))
        .ok_or_else(|| invalid("quantity overflow"))?;
    Ok(BillOfMaterials {
        assembly_doc: root.doc_id.clone(),
        declared_component_count: model.components.len(),
        top_level_instance_count: top.instances.len(),
        leaf_part_count: rows.len(),
        leaf_instance_count: total,
        rows,
    })
}

#[derive(Default)]
struct BomState {
    rows: BTreeMap<String, BomRow>,
    active: Vec<(String, DocumentId)>,
    visits: usize,
}

fn walk(
    key: &str,
    catalog: &BTreeMap<String, BomSource>,
    state: &mut BomState,
    path: &mut Vec<InstanceId>,
) -> Result<()> {
    if path.len() > BOM_MAX_DEPTH || state.visits >= BOM_MAX_OCCURRENCES {
        return Err(invalid("depth/occurrence budget exceeded"));
    }
    if !path.is_empty() {
        state.visits += 1;
    }
    let source = catalog
        .get(key)
        .ok_or_else(|| invalid(format!("missing source '{key}'")))?;
    if state
        .active
        .iter()
        .any(|(p, id)| p == key || id == &source.doc_id)
    {
        return Err(invalid(format!("cycle at '{}' ({key})", source.doc_id)));
    }
    let Some(model) = &source.assembly else {
        if !source.component_sources.is_empty() {
            return Err(invalid("part source contains assembly mappings"));
        }
        let row = state.rows.entry(key.into()).or_insert_with(|| BomRow {
            source_doc: source.doc_id.clone(),
            name: source.name.clone(),
            source_path: key.into(),
            quantity: 0,
            instance_paths: Vec::new(),
        });
        row.quantity = row
            .quantity
            .checked_add(1)
            .ok_or_else(|| invalid("quantity overflow"))?;
        row.instance_paths.push(path.clone());
        return Ok(());
    };
    let expanded = bom_expanded_model(model, &source.doc_id)?;
    state.active.push((key.into(), source.doc_id.clone()));
    let result = (|| {
        for instance in &expanded.instances {
            let component = expanded.component(&instance.component).ok_or_else(|| {
                invalid(format!(
                    "instance '{}' has unknown component '{}'",
                    instance.id, instance.component
                ))
            })?;
            let child_key = source
                .component_sources
                .get(component.id.as_str())
                .ok_or_else(|| {
                    invalid(format!(
                        "component '{}' has no verified source",
                        component.id
                    ))
                })?;
            let child = catalog
                .get(child_key)
                .ok_or_else(|| invalid(format!("missing source '{child_key}'")))?;
            if child.doc_id != component.source_doc
                || (child.assembly.is_some()
                    != (component.source_kind == ComponentSourceKind::Assembly))
            {
                return Err(invalid(format!(
                    "component '{}' source identity/kind mismatch",
                    component.id
                )));
            }
            path.push(instance.id.clone());
            let result = walk(child_key, catalog, state, path);
            path.pop();
            result?;
        }
        Ok(())
    })();
    state.active.pop();
    result
}

fn invalid(message: impl Into<String>) -> OpenCadError {
    OpenCadError::validation(format!("BOM: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AssemblyPattern, Component, Instance, Placement};
    use opencad_core::{ComponentId, PatternId};
    fn id(s: &str) -> DocumentId {
        DocumentId::new(s).expect("id")
    }
    fn component(s: &str, path: &str, doc: &str, assembly: bool) -> Component {
        let mut c = Component::new(ComponentId::new(s).expect("id"), path, id(doc));
        if assembly {
            c.source_kind = ComponentSourceKind::Assembly;
        }
        c
    }
    fn instance(s: &str, c: &str) -> Instance {
        Instance::new(
            InstanceId::new(s).expect("id"),
            ComponentId::new(c).expect("id"),
            crate::Placement::identity(),
            s,
        )
    }
    fn catalog() -> BTreeMap<String, BomSource> {
        let mut sub = AssemblyModel::new();
        sub.components.push(component(
            "component:plate",
            "parts/plate.ocad.d",
            "doc:plate",
            false,
        ));
        let mut fixed = instance("instance:fixed", "component:plate");
        fixed.fixed = true;
        sub.instances.push(fixed);
        sub.patterns.push(AssemblyPattern {
            id: PatternId::new("pattern:row").expect("id"),
            component: ComponentId::new("component:plate").expect("id"),
            count: 2,
            spacing_m: 0.1,
            direction_m: [1., 0., 0.],
            base_placement: Placement::identity(),
            name_prefix: "copy".into(),
        });
        let mut root = AssemblyModel::new();
        root.components = vec![
            component("component:sub", "sub.ocad", "doc:sub", true),
            component("component:direct", "parts/plate.ocad.d", "doc:plate", false),
            component("component:unused", "missing.ocad", "doc:unused", false),
        ];
        root.instances = vec![
            instance("instance:right", "component:sub"),
            instance("instance:left", "component:sub"),
            instance("instance:direct", "component:direct"),
        ];
        BTreeMap::from([
            (
                ".".into(),
                BomSource {
                    doc_id: id("doc:root"),
                    name: "Root".into(),
                    assembly: Some(root),
                    component_sources: BTreeMap::from([
                        ("component:sub".into(), "sub.ocad".into()),
                        ("component:direct".into(), "parts/plate.ocad.d".into()),
                    ]),
                },
            ),
            (
                "sub.ocad".into(),
                BomSource {
                    doc_id: id("doc:sub"),
                    name: "Sub".into(),
                    assembly: Some(sub),
                    component_sources: BTreeMap::from([(
                        "component:plate".into(),
                        "parts/plate.ocad.d".into(),
                    )]),
                },
            ),
            (
                "parts/plate.ocad.d".into(),
                BomSource {
                    doc_id: id("doc:plate"),
                    name: "Plate".into(),
                    assembly: None,
                    component_sources: BTreeMap::new(),
                },
            ),
        ])
    }
    #[test]
    fn analytic_nested_pattern_quantities_paths_round_trip_and_permutations() {
        let mut catalog = catalog();
        let before = catalog.clone();
        let bom = build_bill_of_materials(".", &catalog).expect("BOM");
        assert_eq!(
            (
                bom.declared_component_count,
                bom.top_level_instance_count,
                bom.leaf_part_count,
                bom.leaf_instance_count
            ),
            (3, 3, 1, 7)
        );
        assert_eq!(bom.rows[0].quantity, 7);
        assert_eq!(bom.rows[0].instance_paths.len(), 7);
        assert_eq!(
            bom.rows[0]
                .instance_paths
                .iter()
                .filter(|p| p.len() == 2)
                .count(),
            6
        );
        assert_eq!(
            bom,
            serde_json::from_str::<BillOfMaterials>(&serde_json::to_string(&bom).expect("json"))
                .expect("round trip")
        );
        assert_eq!(catalog, before, "counting is read-only");
        for source in catalog.values_mut() {
            if let Some(model) = &mut source.assembly {
                model.components.reverse();
                model.instances.reverse();
            }
        }
        assert_eq!(
            bom,
            build_bill_of_materials(".", &catalog).expect("canonical")
        );
    }
    #[test]
    fn changed_copies_with_same_uuid_remain_distinct_sources() {
        let mut catalog = catalog();
        let copy = BomSource {
            name: "Edited copy".into(),
            ..catalog["parts/plate.ocad.d"].clone()
        };
        catalog.insert("other/plate.ocad.d".into(), copy);
        let root = catalog.get_mut(".").expect("root");
        let model = root.assembly.as_mut().expect("model");
        model.components.push(component(
            "component:copy",
            "other/plate.ocad.d",
            "doc:plate",
            false,
        ));
        model
            .instances
            .push(instance("instance:copy", "component:copy"));
        root.component_sources
            .insert("component:copy".into(), "other/plate.ocad.d".into());
        let bom = build_bill_of_materials(".", &catalog).expect("BOM");
        assert_eq!(bom.leaf_part_count, 2);
        assert_eq!(bom.leaf_instance_count, 8);
        assert_eq!(bom.rows.iter().map(|r| r.quantity).sum::<usize>(), 8);
    }
    #[test]
    fn missing_identity_kind_cycle_and_pattern_overflow_fail_closed() {
        for failure in 0..4 {
            let mut catalog = catalog();
            match failure {
                0 => {
                    catalog.remove("parts/plate.ocad.d");
                }
                1 => catalog.get_mut("parts/plate.ocad.d").expect("part").doc_id = id("doc:wrong"),
                2 => {
                    catalog
                        .get_mut("parts/plate.ocad.d")
                        .expect("part")
                        .assembly = Some(AssemblyModel::new())
                }
                _ => {
                    let sub = catalog.get_mut("sub.ocad").expect("sub");
                    let model = sub.assembly.as_mut().expect("model");
                    model.components.push(component(
                        "component:root",
                        "root.ocad",
                        "doc:root",
                        true,
                    ));
                    model
                        .instances
                        .push(instance("instance:root", "component:root"));
                    sub.component_sources
                        .insert("component:root".into(), ".".into());
                }
            }
            assert!(build_bill_of_materials(".", &catalog).is_err());
        }
        let mut catalog = catalog();
        catalog
            .get_mut("sub.ocad")
            .expect("sub")
            .assembly
            .as_mut()
            .expect("model")
            .patterns[0]
            .count = usize::MAX;
        assert!(build_bill_of_materials(".", &catalog).is_err());
    }
}
