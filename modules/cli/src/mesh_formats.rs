//! 3MF and binary glTF (GLB) encoders for regenerated meshes (ADR-029).
//!
//! Both encoders are pure functions of a [`MeshSet`] (kernel metres) and are
//! deterministic: no timestamps, fixed element order, fixed number format.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Cursor, Write};

use opencad_core::{OpenCadError, Result};
use opencad_geometry::MeshSet;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Quaternion (x, y, z, w) of -90° about X: model +Z maps to glTF +Y.
const Z_UP_TO_Y_UP: [f64; 4] = [
    -std::f64::consts::FRAC_1_SQRT_2,
    0.0,
    0.0,
    std::f64::consts::FRAC_1_SQRT_2,
];

/// Positions closer than this are one vertex when welding for 3MF (metres).
const WELD_TOLERANCE_M: f64 = 1e-9;

/// Indexed triangle mesh whose coincident vertices are shared.
#[derive(Debug, Clone, PartialEq)]
pub struct WeldedMesh {
    pub vertices_m: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

/// Merge vertices that coincide within [`WELD_TOLERANCE_M`] and drop the
/// triangles that collapse.  Tessellation duplicates vertices along B-rep face
/// boundaries (each face carries its own normals); printers and slicers need
/// them shared to see a closed, manifold surface.
pub fn weld(mesh: &MeshSet) -> WeldedMesh {
    let mut index_of: HashMap<[i64; 3], u32> = HashMap::new();
    let mut vertices_m = Vec::new();
    let mut remap = Vec::with_capacity(mesh.positions.len());
    for position in &mesh.positions {
        let key = position.map(|value| (f64::from(value) / WELD_TOLERANCE_M).round() as i64);
        let index = *index_of.entry(key).or_insert_with(|| {
            vertices_m.push(*position);
            (vertices_m.len() - 1) as u32
        });
        remap.push(index);
    }
    let triangles = mesh
        .indices
        .chunks_exact(3)
        .map(|triangle| {
            [
                remap[triangle[0] as usize],
                remap[triangle[1] as usize],
                remap[triangle[2] as usize],
            ]
        })
        .filter(|[a, b, c]| a != b && b != c && a != c)
        .collect();
    WeldedMesh {
        vertices_m,
        triangles,
    }
}

/// Encode named meshes as a 3MF package: millimetres, one welded object and
/// one build item per mesh.  An assembly passes one mesh per placed instance,
/// so parts that touch stay separate closed solids instead of fusing into a
/// non-manifold surface.
pub fn encode_3mf(title: &str, objects: &[(String, MeshSet)]) -> Result<Vec<u8>> {
    let mut model = String::new();
    model.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    model.push_str(
        "<model unit=\"millimeter\" xml:lang=\"en-US\" \
         xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n",
    );
    let _ = writeln!(
        model,
        "  <metadata name=\"Title\">{}</metadata>",
        xml_escape(title)
    );
    model.push_str("  <metadata name=\"Application\">MusubiCAD</metadata>\n");
    model.push_str("  <resources>\n");
    let mut written = 0;
    for (name, mesh) in objects {
        let welded = weld(mesh);
        if welded.triangles.is_empty() {
            continue;
        }
        written += 1;
        let _ = writeln!(
            model,
            "    <object id=\"{written}\" type=\"model\" name=\"{}\">",
            xml_escape(name)
        );
        model.push_str("      <mesh>\n        <vertices>\n");
        for vertex in &welded.vertices_m {
            let _ = writeln!(
                model,
                "          <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>",
                millimetres(vertex[0]),
                millimetres(vertex[1]),
                millimetres(vertex[2])
            );
        }
        model.push_str("        </vertices>\n        <triangles>\n");
        for [v1, v2, v3] in &welded.triangles {
            let _ = writeln!(
                model,
                "          <triangle v1=\"{v1}\" v2=\"{v2}\" v3=\"{v3}\"/>"
            );
        }
        model.push_str("        </triangles>\n      </mesh>\n    </object>\n");
    }
    if written == 0 {
        return Err(OpenCadError::validation(
            "mesh must contain at least one triangle",
        ));
    }
    model.push_str("  </resources>\n  <build>\n");
    for id in 1..=written {
        let _ = writeln!(model, "    <item objectid=\"{id}\"/>");
    }
    model.push_str("  </build>\n</model>\n");

    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n\
  <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n\
  <Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/>\n\
</Types>\n";
    let relationships = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n\
  <Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" \
Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>\n\
</Relationships>\n";

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (path, body) in [
        ("[Content_Types].xml", content_types),
        ("_rels/.rels", relationships),
        ("3D/3dmodel.model", model.as_str()),
    ] {
        zip.start_file(path, options).map_err(zip_error)?;
        zip.write_all(body.as_bytes())
            .map_err(|error| OpenCadError::Other(error.to_string()))?;
    }
    Ok(zip.finish().map_err(zip_error)?.into_inner())
}

/// Encode a mesh as binary glTF 2.0 in metres: one node, one mesh, indexed
/// triangles with per-vertex normals.  Vertex data keeps the model's +Z-up
/// axes; the node rotates -90° about X so viewers that follow glTF's +Y-up
/// convention show the part upright.
pub fn encode_glb(mesh: &MeshSet, name: &str) -> Result<Vec<u8>> {
    if mesh.indices.len() < 3 || mesh.indices.len() % 3 != 0 {
        return Err(OpenCadError::validation(
            "mesh must contain at least one triangle",
        ));
    }
    if mesh.normals.len() != mesh.positions.len() {
        return Err(OpenCadError::validation(
            "mesh needs one normal per position",
        ));
    }
    let vertex_count = mesh.positions.len();
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut binary = Vec::with_capacity(vertex_count * 24 + mesh.indices.len() * 4);
    for position in &mesh.positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
            binary.extend_from_slice(&position[axis].to_le_bytes());
        }
    }
    let normals_offset = binary.len();
    for normal in &mesh.normals {
        for value in normal {
            binary.extend_from_slice(&value.to_le_bytes());
        }
    }
    let indices_offset = binary.len();
    for index in &mesh.indices {
        binary.extend_from_slice(&index.to_le_bytes());
    }
    let indices_length = binary.len() - indices_offset;
    while binary.len() % 4 != 0 {
        binary.push(0);
    }

    let json = serde_json::json!({
        "asset": { "version": "2.0", "generator": "MusubiCAD" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0, "name": name, "rotation": Z_UP_TO_Y_UP }],
        "meshes": [{
            "name": name,
            "primitives": [{
                "attributes": { "POSITION": 0, "NORMAL": 1 },
                "indices": 2,
                "material": 0,
                "mode": 4
            }]
        }],
        "materials": [{
            "name": "MusubiCAD default",
            "pbrMetallicRoughness": {
                "baseColorFactor": [0.22, 0.55, 0.86, 1.0],
                "metallicFactor": 0.1,
                "roughnessFactor": 0.6
            }
        }],
        "buffers": [{ "byteLength": binary.len() }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": normals_offset, "byteStride": 12, "target": 34962 },
            { "buffer": 0, "byteOffset": normals_offset, "byteLength": indices_offset - normals_offset, "byteStride": 12, "target": 34962 },
            { "buffer": 0, "byteOffset": indices_offset, "byteLength": indices_length, "target": 34963 }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": vertex_count, "type": "VEC3", "min": min, "max": max },
            { "bufferView": 1, "componentType": 5126, "count": vertex_count, "type": "VEC3" },
            { "bufferView": 2, "componentType": 5125, "count": mesh.indices.len(), "type": "SCALAR" }
        ]
    });
    let mut json = serde_json::to_vec(&json)?;
    while json.len() % 4 != 0 {
        json.push(b' ');
    }

    let total = 12 + 8 + json.len() + 8 + binary.len();
    let total = u32::try_from(total)
        .map_err(|_| OpenCadError::validation("mesh is too large for a GLB file"))?;
    let mut glb = Vec::with_capacity(total as usize);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&total.to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(&binary);
    Ok(glb)
}

/// Kernel metres as millimetre text: four decimals (0.1 µm, finer than f32
/// resolves at part scale), no `-0`.
fn millimetres(value_m: f32) -> String {
    let rounded = (f64::from(value_m) * 1000.0 * 1e4).round() / 1e4;
    if rounded == 0.0 {
        return "0".into();
    }
    let text = format!("{rounded:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn zip_error(error: zip::result::ZipError) -> OpenCadError {
    OpenCadError::Other(format!("3MF package: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    /// Unit cube in metres, one quad per face with its own vertices and
    /// normals, as a tessellator emits it.
    fn cube(size_m: f32) -> MeshSet {
        let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
            (
                [0.0, 0.0, -1.0],
                [
                    [0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [1.0, 0.0, 0.0],
                ],
            ),
            (
                [0.0, 0.0, 1.0],
                [
                    [0.0, 0.0, 1.0],
                    [1.0, 0.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [0.0, 1.0, 1.0],
                ],
            ),
            (
                [0.0, -1.0, 0.0],
                [
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 0.0, 1.0],
                    [0.0, 0.0, 1.0],
                ],
            ),
            (
                [0.0, 1.0, 0.0],
                [
                    [0.0, 1.0, 0.0],
                    [0.0, 1.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [1.0, 1.0, 0.0],
                ],
            ),
            (
                [-1.0, 0.0, 0.0],
                [
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0],
                    [0.0, 1.0, 1.0],
                    [0.0, 1.0, 0.0],
                ],
            ),
            (
                [1.0, 0.0, 0.0],
                [
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [1.0, 1.0, 1.0],
                    [1.0, 0.0, 1.0],
                ],
            ),
        ];
        let mut mesh = MeshSet {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            triangle_face_ids: Vec::new(),
        };
        for (normal, corners) in faces {
            let base = mesh.positions.len() as u32;
            for corner in corners {
                mesh.positions.push(corner.map(|v| v * size_m));
                mesh.normals.push(normal);
            }
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        mesh
    }

    #[test]
    fn weld_shares_face_boundary_vertices_into_a_closed_surface() {
        let welded = weld(&cube(0.01));
        assert_eq!(welded.vertices_m.len(), 8);
        assert_eq!(welded.triangles.len(), 12);
        // Closed and manifold: every undirected edge is used by exactly two triangles.
        let mut edges: HashMap<(u32, u32), usize> = HashMap::new();
        for [a, b, c] in &welded.triangles {
            for (p, q) in [(a, b), (b, c), (c, a)] {
                *edges.entry((*p.min(q), *p.max(q))).or_default() += 1;
            }
        }
        assert!(edges.values().all(|count| *count == 2), "{edges:?}");
    }

    #[test]
    fn three_mf_package_holds_a_millimetre_model_and_is_deterministic() {
        let objects = [("Cube & co".to_string(), cube(0.01))];
        let bytes = encode_3mf("Cube & co", &objects).expect("3mf");
        assert_eq!(bytes, encode_3mf("Cube & co", &objects).expect("again"));
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
        let names: Vec<String> = archive.file_names().map(str::to_string).collect();
        assert_eq!(
            names,
            ["[Content_Types].xml", "_rels/.rels", "3D/3dmodel.model"]
        );
        let mut model = String::new();
        archive
            .by_name("3D/3dmodel.model")
            .expect("model")
            .read_to_string(&mut model)
            .expect("read");
        assert!(model.contains("unit=\"millimeter\""));
        assert!(
            model.contains("<vertex x=\"10\" y=\"10\" z=\"10\"/>"),
            "{model}"
        );
        assert_eq!(model.matches("<vertex ").count(), 8);
        assert_eq!(model.matches("<triangle ").count(), 12);
        assert!(model.contains("Cube &amp; co"));
    }

    #[test]
    fn glb_has_valid_chunks_bounds_and_counts() {
        let mesh = cube(0.02);
        let glb = encode_glb(&mesh, "cube").expect("glb");
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(
            u32::from_le_bytes(glb[4..8].try_into().expect("version")),
            2
        );
        assert_eq!(
            u32::from_le_bytes(glb[8..12].try_into().expect("length")) as usize,
            glb.len()
        );
        let json_length = u32::from_le_bytes(glb[12..16].try_into().expect("json length")) as usize;
        assert_eq!(&glb[16..20], b"JSON");
        assert_eq!(json_length % 4, 0);
        let json: serde_json::Value =
            serde_json::from_slice(&glb[20..20 + json_length]).expect("json");
        assert_eq!(json["accessors"][0]["count"], 24);
        assert_eq!(json["accessors"][2]["count"], 36);
        assert_eq!(
            json["nodes"][0]["rotation"][0].as_f64(),
            Some(-std::f64::consts::FRAC_1_SQRT_2)
        );
        assert_eq!(
            json["accessors"][0]["max"][0].as_f64(),
            Some(f64::from(0.02_f32))
        );
        let bin_header = 20 + json_length;
        assert_eq!(&glb[bin_header + 4..bin_header + 8], b"BIN\0");
        let bin_length =
            u32::from_le_bytes(glb[bin_header..bin_header + 4].try_into().expect("bin")) as usize;
        assert_eq!(json["buffers"][0]["byteLength"], bin_length);
        assert_eq!(glb, encode_glb(&mesh, "cube").expect("again"));
    }

    #[test]
    fn millimetre_text_is_short_and_never_negative_zero() {
        assert_eq!(millimetres(0.096), "96");
        assert_eq!(millimetres(-0.0), "0");
        assert_eq!(millimetres(0.0055), "5.5");
    }
}
