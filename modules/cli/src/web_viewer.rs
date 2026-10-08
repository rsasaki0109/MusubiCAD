//! `musubicad export <assembly> viewer.html`: a self-contained web page that
//! shows the assembly in 3D and moves it with one slider per joint.
//!
//! Everything comes from the Design Graph: meshes are the regenerated parts,
//! and the sliders run the same forward kinematics as `animate-joints`,
//! `reach`, and the URDF export (joint origins, axes, and limits from the
//! kinematic tree, after solving mates).  The page has no external scripts or
//! network access; a small WebGL renderer is inlined.  Output is
//! deterministic: meshes are little-endian `f32`/`u32` arrays in base64 and
//! numbers have fixed formatting.

use std::fmt::Write as _;
use std::fs;

use base64::Engine as _;
use opencad_assembly::{kinematic_tree, JointKind};
use opencad_core::{OpenCadError, Result};
use opencad_file::read_ocad;
use opencad_geometry::{MeshSet, RigidTransform};

use crate::export::ExportSummary;
use crate::joint_animation::{movable_joints, solved_assembly};

/// Part colours in instance order, matching `musubicad preview`.
const PALETTE: [[f32; 3]; 6] = [
    [0.13, 0.47, 0.84],
    [0.95, 0.55, 0.15],
    [0.20, 0.70, 0.45],
    [0.62, 0.40, 0.82],
    [0.85, 0.30, 0.35],
    [0.30, 0.33, 0.38],
];

const TEMPLATE: &str = include_str!("web_viewer.html");

/// Column-major 4 × 4 matrix of a rigid transform.
fn matrix(transform: RigidTransform) -> [f64; 16] {
    let r = transform.rotation;
    let t = transform.translation_m;
    [
        r[0][0], r[1][0], r[2][0], 0.0, r[0][1], r[1][1], r[2][1], 0.0, r[0][2], r[1][2], r[2][2],
        0.0, t[0], t[1], t[2], 1.0,
    ]
}

fn number(value: f64) -> String {
    let rounded = (value * 1e9).round() / 1e9 + 0.0;
    let text = format!("{rounded:.9}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-" {
        "0".into()
    } else {
        text.into()
    }
}

fn numbers(values: &[f64]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|v| number(*v))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn base64_f32(values: impl Iterator<Item = f32>) -> String {
    let bytes: Vec<u8> = values.flat_map(f32::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn base64_u32(values: &[u32]) -> String {
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Positions and triangle indices; the page computes flat face normals.
fn mesh_json(mesh: &MeshSet) -> String {
    format!(
        "{{\"positions\":\"{}\",\"indices\":\"{}\"}}",
        base64_f32(mesh.positions.iter().flatten().copied()),
        base64_u32(&mesh.indices)
    )
}

/// Write `output` (a `.html`) for the assembly at `input`.
pub fn export_web_viewer(input: &str, output: &str) -> Result<ExportSummary> {
    let doc = read_ocad(input)?;
    let assembly = doc.assembly.as_ref().ok_or_else(|| {
        OpenCadError::validation("HTML viewer export needs an assembly document with joints")
    })?;
    let assembly = solved_assembly(assembly)?;
    let tree = kinematic_tree(&assembly)?;
    let named = movable_joints(&assembly, &tree);
    let meshes = crate::export::assembly_instance_meshes(input, &doc)?;

    let mut links = Vec::new();
    let mut triangles = 0;
    for link in &tree.links {
        let instance = assembly
            .instances
            .iter()
            .find(|instance| instance.id == link.instance)
            .ok_or_else(|| {
                OpenCadError::validation(format!("instance '{}' not found", link.instance))
            })?;
        let parent = match &link.joint {
            None => -1,
            Some(joint) => tree
                .links
                .iter()
                .position(|candidate| candidate.instance == joint.parent)
                .map_or(-1, |position| position as i64),
        };
        let origin = match &link.joint {
            None => link.world,
            Some(joint) => joint.origin,
        };
        let joint = match &link.joint {
            Some(joint) if joint.kind.is_movable() => {
                let name = named
                    .iter()
                    .find(|candidate| candidate.instance == link.instance.as_str())
                    .map(|candidate| (candidate.id.clone(), candidate.short_name()))
                    .unwrap_or_else(|| {
                        let id = link.instance.as_str().to_string();
                        (id.clone(), id)
                    });
                let (kind, axis) = match joint.kind {
                    JointKind::Continuous { axis } => ("continuous", axis),
                    JointKind::Revolute { axis, .. } => ("revolute", axis),
                    JointKind::Prismatic { axis, .. } => ("prismatic", axis),
                    JointKind::Fixed => ("fixed", [0.0; 3]),
                };
                let (lower, upper) = joint
                    .kind
                    .limits()
                    .unwrap_or((-std::f64::consts::PI, std::f64::consts::PI));
                format!(
                    "{{\"id\":{},\"name\":{},\"type\":\"{kind}\",\"axis\":{},\"lower\":{},\"upper\":{}}}",
                    json_string(&name.0),
                    json_string(&name.1),
                    numbers(&axis),
                    number(lower),
                    number(upper)
                )
            }
            _ => "null".into(),
        };
        // Colour by position among the regenerated instances, as `preview` does.
        let position = meshes.iter().position(|(id, _, _)| *id == link.instance);
        let mesh = position
            .map(|position| {
                let mesh = &meshes[position].2;
                triangles += mesh.triangle_count();
                mesh_json(mesh)
            })
            .unwrap_or_else(|| "null".into());
        let color = PALETTE[position.unwrap_or(0) % PALETTE.len()].map(f64::from);
        links.push(format!(
            "{{\"instance\":{},\"name\":{},\"parent\":{parent},\"origin\":{},\"frameInPartInv\":{},\"placementInv\":{},\"color\":{},\"joint\":{joint},\"mesh\":{mesh}}}",
            json_string(link.instance.as_str()),
            json_string(&instance.name),
            numbers(&matrix(origin)),
            numbers(&matrix(link.frame_in_part.inverse()?)),
            numbers(&matrix(instance.placement.transform.inverse()?)),
            numbers(&color),
        ));
    }

    let mut data = String::new();
    let _ = write!(
        data,
        "{{\"name\":{},\"document\":{},\"links\":[{}]}}",
        json_string(&doc.metadata.name),
        json_string(doc.metadata.id.as_str()),
        links.join(",")
    );
    // Keep the JSON from closing its <script> element.
    let data = data.replace("</", "<\\/");
    let html = TEMPLATE
        .replace("__TITLE__", &html_escape(&doc.metadata.name))
        .replace("__DATA__", &data);
    fs::write(output, &html)
        .map_err(|error| OpenCadError::Other(format!("cannot write '{output}': {error}")))?;
    Ok(ExportSummary {
        format: "html".into(),
        triangles,
        output: output.to_string(),
        bytes: Some(html.len()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(name: &str) -> String {
        format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn numbers_are_fixed_and_matrices_column_major() {
        assert_eq!(number(0.1 + 0.2), "0.3");
        assert_eq!(number(-1e-12), "0");
        assert_eq!(number(-2.5), "-2.5");
        assert_eq!(number(3.0), "3");
        let m = matrix(RigidTransform {
            translation_m: [1.0, 2.0, 3.0],
            rotation: [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        });
        // First column is the image of +X: (0, 1, 0).
        assert_eq!(&m[0..4], &[0.0, 1.0, 0.0, 0.0]);
        assert_eq!(&m[12..16], &[1.0, 2.0, 3.0, 1.0]);
        assert_eq!(html_escape("<a & b>"), "&lt;a &amp; b&gt;");
    }

    #[test]
    fn exports_a_self_contained_deterministic_robot_viewer() {
        let output =
            std::env::temp_dir().join(format!("musubicad-viewer-{}.html", std::process::id()));
        let path = output.to_str().expect("utf-8");
        let summary = export_web_viewer(&example("robot_arm_assembly.ocad.d"), path).expect("html");
        assert_eq!(summary.format, "html");
        assert!(summary.triangles > 1000);
        let html = std::fs::read_to_string(&output).expect("read");
        assert!(html.starts_with("<!doctype html>"));
        assert!(!html.contains("__DATA__") && !html.contains("__TITLE__"));
        // Self-contained: no external scripts, styles, or fetches.
        for forbidden in ["<script src", "<link", "http://", "https://", "fetch("] {
            assert!(!html.contains(forbidden), "{forbidden}");
        }
        for joint in ["\"joint:shoulder\"", "\"joint:elbow\"", "\"joint:wrist\""] {
            assert!(html.contains(joint), "{joint}");
        }
        // Elbow limits are -100° to 140°, in radians.
        assert!(html.contains("\"lower\":-1.745329252,\"upper\":2.443460953"));
        let again = std::env::temp_dir().join(format!(
            "musubicad-viewer-again-{}.html",
            std::process::id()
        ));
        export_web_viewer(
            &example("robot_arm_assembly.ocad.d"),
            again.to_str().expect("utf-8"),
        )
        .expect("again");
        assert_eq!(std::fs::read(&again).expect("again"), html.as_bytes());
        let _ = std::fs::remove_file(&output);
        let _ = std::fs::remove_file(&again);
        assert!(export_web_viewer(&example("bracket.ocad.d"), path).is_err());
    }
}
