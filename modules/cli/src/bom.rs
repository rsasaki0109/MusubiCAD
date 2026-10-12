//! Snapshot-backed BOM query and optional JSON/CSV output (MCAD-P12-008).
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use opencad_assembly::{
    bom_expanded_model, build_bill_of_materials, regenerate_assembly, validate_component_path,
    BillOfMaterials, BomSource, ChildPart, ComponentSourceKind, InstanceRegenStatus, ResolvedChild,
    BOM_MAX_DEPTH, BOM_MAX_SOURCES,
};
use opencad_core::{DocumentId, DocumentKind, OpenCadError, Result};
use opencad_feature::FeatureRegistry;
use opencad_file::{read_ocad, OcadDocument};
use opencad_geometry::GeometryKernel;

pub fn query_bom(path: &str, output: Option<&str>) -> Result<BillOfMaterials> {
    #[cfg(feature = "occt")]
    {
        query_bom_with_kernel(
            path,
            output,
            &opencad_kernel_occt::OcctGeometryKernel::new(),
        )
    }
    #[cfg(not(feature = "occt"))]
    {
        let _ = (path, output);
        Err(OpenCadError::validation("BOM verification requires OCCT"))
    }
}

fn query_bom_with_kernel(
    path: &str,
    output: Option<&str>,
    kernel: &impl GeometryKernel,
) -> Result<BillOfMaterials> {
    if output.is_some_and(|p| {
        !matches!(
            Path::new(p).extension().and_then(|x| x.to_str()),
            Some("json" | "csv")
        )
    }) {
        return Err(OpenCadError::validation(
            "BOM output must use .json or .csv",
        ));
    }
    let root_path = std::fs::canonicalize(path).map_err(io_error)?;
    let root_dir = document_root(&root_path);
    let root = read_ocad(&root_path)?;
    if root.metadata.kind != DocumentKind::Assembly {
        return Err(OpenCadError::validation(
            "BOM requires an assembly document",
        ));
    }
    let mut snapshots = Snapshots {
        root: root_dir,
        documents: BTreeMap::new(),
        catalog: BTreeMap::new(),
        paths: BTreeMap::new(),
        active: Vec::new(),
    };
    let root_key = snapshots.collect(&root_path, &root)?;
    let bom = build_bill_of_materials(&root_key, &snapshots.catalog)?;

    // Verify every assembly independently: a parent compound can conceal a nested
    // partial failure in the existing localized regeneration contract.
    for (key, source) in &snapshots.catalog {
        let Some(model) = &source.assembly else {
            continue;
        };
        let source_path = snapshots
            .paths
            .get(key)
            .ok_or_else(|| OpenCadError::validation("BOM snapshot path missing"))?;
        let report = regenerate_assembly(
            model,
            &source.doc_id,
            &document_root(source_path),
            kernel,
            &FeatureRegistry::with_defaults(),
            &mut |p| snapshots.child(p),
        )?;
        if report.successful_instances != report.instance_count {
            let detail = report
                .instances
                .iter()
                .find_map(|i| match &i.status {
                    InstanceRegenStatus::Failed(message) => Some(message.as_str()),
                    InstanceRegenStatus::Ok => None,
                })
                .unwrap_or("incomplete assembly regeneration");
            return Err(OpenCadError::validation(format!(
                "BOM source '{key}' failed: {detail}"
            )));
        }
    }
    if let Some(output) = output {
        let destination = Path::new(output);
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = std::fs::canonicalize(parent).map_err(io_error)?;
        let filename = destination
            .file_name()
            .ok_or_else(|| OpenCadError::validation("BOM output filename missing"))?;
        let resolved = std::fs::canonicalize(destination).unwrap_or_else(|_| parent.join(filename));
        if parent
            .ancestors()
            .chain(resolved.ancestors())
            .any(opencad_file::expanded_dir::is_expanded_dir)
            || snapshots
                .documents
                .keys()
                .any(|p| resolved == *p || (p.is_dir() && resolved.starts_with(p)))
        {
            return Err(OpenCadError::validation(
                "BOM output must be outside every source document",
            ));
        }
        let data = if destination.extension().and_then(|e| e.to_str()) == Some("csv") {
            bom_csv(&bom)
        } else {
            serde_json::to_string_pretty(&bom)? + "\n"
        };
        let mut temp = tempfile::NamedTempFile::new_in(&parent).map_err(io_error)?;
        temp.write_all(data.as_bytes()).map_err(io_error)?;
        temp.as_file().sync_all().map_err(io_error)?;
        temp.persist(destination).map_err(|e| io_error(e.error))?;
    }
    Ok(bom)
}

struct Snapshots {
    root: PathBuf,
    documents: BTreeMap<PathBuf, OcadDocument>,
    catalog: BTreeMap<String, BomSource>,
    paths: BTreeMap<String, PathBuf>,
    active: Vec<(PathBuf, DocumentId)>,
}
impl Snapshots {
    fn collect(&mut self, path: &Path, doc: &OcadDocument) -> Result<String> {
        if self.active.len() > BOM_MAX_DEPTH
            || self
                .active
                .iter()
                .any(|(p, id)| p == path || id == &doc.metadata.id)
        {
            return Err(OpenCadError::validation(
                "BOM cycle/depth limit in source references",
            ));
        }
        let relative = path
            .strip_prefix(&self.root)
            .map_err(|_| OpenCadError::validation("BOM source escapes root"))?;
        let relative = relative
            .to_str()
            .ok_or_else(|| OpenCadError::validation("BOM paths must be UTF-8"))?
            .replace('\\', "/");
        let key = if relative.is_empty() {
            ".".into()
        } else {
            relative
        };
        if self.catalog.contains_key(&key) {
            return Ok(key);
        }
        if self.documents.len() >= BOM_MAX_SOURCES {
            return Err(OpenCadError::validation("BOM source budget exceeded"));
        }
        match doc.metadata.kind {
            DocumentKind::Part if doc.assembly.is_none() => {}
            DocumentKind::Assembly if doc.assembly.is_some() => {}
            _ => return Err(OpenCadError::validation("BOM source kind/model mismatch")),
        }
        self.documents.insert(path.to_path_buf(), doc.clone());
        self.paths.insert(key.clone(), path.to_path_buf());
        self.active
            .push((path.to_path_buf(), doc.metadata.id.clone()));
        let result = (|| {
            let mut mappings = BTreeMap::new();
            if let Some(model) = &doc.assembly {
                let expanded = bom_expanded_model(model, &doc.metadata.id)?;
                let used: BTreeSet<_> = expanded
                    .instances
                    .iter()
                    .map(|i| i.component.as_str())
                    .collect();
                for id in used {
                    let component = expanded
                        .components
                        .iter()
                        .find(|c| c.id.as_str() == id)
                        .ok_or_else(|| {
                            OpenCadError::validation(format!("BOM unknown component '{id}'"))
                        })?;
                    let child_path =
                        validate_component_path(&document_root(path), &component.source_path)?;
                    let child = match self.documents.get(&child_path) {
                        Some(d) => d.clone(),
                        None => read_ocad(&child_path)?,
                    };
                    let kind = match component.source_kind {
                        ComponentSourceKind::Part => DocumentKind::Part,
                        ComponentSourceKind::Assembly => DocumentKind::Assembly,
                    };
                    if child.metadata.id != component.source_doc || child.metadata.kind != kind {
                        return Err(OpenCadError::validation(format!(
                            "BOM component '{id}' source identity/kind mismatch"
                        )));
                    }
                    let child_key = self.collect(&child_path, &child)?;
                    if mappings.values().any(|k| k == &child_key) {
                        return Err(OpenCadError::validation(
                            "BOM duplicate component source path",
                        ));
                    }
                    mappings.insert(id.to_string(), child_key);
                }
            }
            Ok(mappings)
        })();
        self.active.pop();
        let mappings = result?;
        self.catalog.insert(
            key.clone(),
            BomSource {
                doc_id: doc.metadata.id.clone(),
                name: doc.metadata.name.clone(),
                assembly: doc.assembly.clone(),
                component_sources: mappings,
            },
        );
        Ok(key)
    }
    fn child(&self, path: &Path) -> Result<ResolvedChild> {
        let doc = self
            .documents
            .get(path)
            .ok_or_else(|| {
                OpenCadError::validation(format!(
                    "BOM source '{}' was not snapshotted",
                    path.display()
                ))
            })?
            .clone();
        if let Some(model) = &doc.assembly {
            return Ok(ResolvedChild::Assembly {
                model: Box::new(model.clone()),
                doc_id: doc.metadata.id,
            });
        }
        let doc_id = doc.metadata.id.clone();
        let parameters = doc.parameters.clone();
        let semantic_refs = doc.semantic_refs.clone();
        Ok(ResolvedChild::Part(Box::new(ChildPart {
            doc_id,
            parameters,
            semantic_refs,
            part: doc.into_part_model(),
        })))
    }
}

fn document_root(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(Path::new(".")).to_path_buf()
    }
}
fn io_error(e: std::io::Error) -> OpenCadError {
    OpenCadError::Other(e.to_string())
}
fn bom_csv(bom: &BillOfMaterials) -> String {
    let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut csv = String::from("source_doc,name,source_path,quantity_count\r\n");
    for r in &bom.rows {
        csv.push_str(&format!(
            "{},{},{},{}\r\n",
            quote(r.source_doc.as_str()),
            quote(&r.name),
            quote(&r.source_path),
            r.quantity
        ));
    }
    csv
}

pub fn cmd_bom(args: Vec<String>) -> Result<()> {
    let usage =
        || OpenCadError::validation("usage: musubicad bom <assembly> [--output bom.json|bom.csv]");
    let (path, output) = match args.as_slice() {
        [path] => (path, None),
        [path, flag, output] if flag == "--output" => (path, Some(output.as_str())),
        _ => return Err(usage()),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&query_bom(path, output)?)?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_geometry::{KernelBody, MockGeometryKernel};
    #[test]
    fn mock_bom_keeps_analytic_solid_dimensions_volume_and_source() {
        let kernel = MockGeometryKernel::new();
        let body = KernelBody(80);
        let before = kernel.mass_properties(&body, 1.).expect("volume");
        assert!((before.volume_m3 - 0.08_f64.powi(3)).abs() < 1e-12);
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/assembly_two_brackets.ocad.d"
        );
        let source = read_ocad(path).expect("source");
        let bom = query_bom_with_kernel(path, None, &kernel).expect("BOM");
        assert_eq!(
            (
                bom.top_level_instance_count,
                bom.leaf_part_count,
                bom.leaf_instance_count
            ),
            (2, 1, 2)
        );
        assert_eq!(source, read_ocad(path).expect("unchanged"));
        assert!(
            (kernel.mass_properties(&body, 1.).expect("after").volume_m3 - before.volume_m3).abs()
                < 1e-12
        );
        let bounds = kernel.bounding_box(&body).expect("bounds");
        for i in 0..3 {
            assert!((bounds.max[i] - bounds.min[i] - 0.08).abs() < 1e-12);
        }
    }
    #[test]
    fn csv_quotes_metadata_and_preserves_canonical_counts() {
        let bom = BillOfMaterials {
            assembly_doc: DocumentId::new("doc:assembly").expect("id"),
            declared_component_count: 1,
            top_level_instance_count: 2,
            leaf_part_count: 1,
            leaf_instance_count: 2,
            rows: vec![opencad_assembly::BomRow {
                source_doc: DocumentId::new("doc:part").expect("id"),
                name: "Part, \"A\"\nline".into(),
                source_path: "parts/part.ocad.d".into(),
                quantity: 2,
                instance_paths: Vec::new(),
            }],
        };
        assert_eq!(bom_csv(&bom),"source_doc,name,source_path,quantity_count\r\n\"doc:part\",\"Part, \"\"A\"\"\nline\",\"parts/part.ocad.d\",2\r\n");
    }
}
