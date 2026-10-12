//! Bounded closed triangle shells in metres (ADR-049).

use std::collections::{BTreeMap, BTreeSet};

use opencad_core::{LengthUnit, OpenCadError, Result};

use crate::{BoundingBox, MassProperties};

pub const MAX_STL_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_STL_TRIANGLES: usize = 10_000;
pub const VERTEX_GRID_M: f64 = 1e-9;

/// Validated connected, closed, consistently oriented triangle shell.
/// Fields are private so callers cannot bypass admission checks.
#[derive(Debug, Clone)]
pub struct TriangleSolid {
    vertices_m: Vec<[f64; 3]>,
    triangles: Vec<[u32; 3]>,
}

impl TriangleSolid {
    pub fn transformed(&self, transform: crate::RigidTransform) -> Result<Self> {
        transform.inverse()?;
        let [a, b, c] = transform.rotation;
        if (dot(a, cross(b, c)) - 1.0).abs() > 1e-9
            || transform
                .translation_m
                .iter()
                .chain(transform.rotation.iter().flatten())
                .any(|v| !v.is_finite())
        {
            return Err(invalid(
                "placement requires a finite proper rigid transform",
            ));
        }
        let mut result = self.clone();
        for point in &mut result.vertices_m {
            *point = std::array::from_fn(|i| {
                dot(transform.rotation[i], *point) + transform.translation_m[i]
            });
            if point.iter().any(|v| !v.is_finite() || v.abs() > 1e6) {
                return Err(invalid("placed coordinates exceed bounds"));
            }
        }
        Ok(result)
    }

    pub fn tessellate(&self) -> crate::MeshSet {
        let mut mesh = crate::MeshSet {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            triangle_face_ids: Vec::new(),
        };
        for (face, triangle) in self.triangles.iter().enumerate() {
            let [a, b, c] = triangle.map(|i| self.vertices_m[i as usize]);
            let normal = cross(sub(b, a), sub(c, a));
            let normal = normal.map(|v| (v / norm(normal)) as f32);
            for p in [a, b, c] {
                mesh.indices.push(mesh.positions.len() as u32);
                mesh.positions.push(p.map(|v| v as f32));
                mesh.normals.push(normal);
            }
            mesh.triangle_face_ids.push(face as u64 + 1);
        }
        mesh
    }
    pub fn vertices_m(&self) -> &[[f64; 3]] {
        &self.vertices_m
    }

    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    pub fn from_triangles(triangles_m: &[[[f64; 3]; 3]]) -> Result<Self> {
        if triangles_m.len() < 4 || triangles_m.len() > MAX_STL_TRIANGLES {
            return Err(invalid("triangle count must be between 4 and 10000"));
        }
        let mut vertices_m = Vec::new();
        let mut vertices = BTreeMap::new();
        let mut triangles = Vec::new();
        let mut edges: BTreeMap<(u32, u32), Vec<(usize, bool)>> = BTreeMap::new();
        for (face, points) in triangles_m.iter().enumerate() {
            let mut triangle = [0; 3];
            for (index, point) in points.iter().enumerate() {
                if point.iter().any(|v| !v.is_finite() || v.abs() > 1e6) {
                    return Err(invalid("coordinates must be finite and within 1e6 metres"));
                }
                let key = point.map(|v| (v / VERTEX_GRID_M).round() as i64);
                triangle[index] = *vertices.entry(key).or_insert_with(|| {
                    let id = vertices_m.len() as u32;
                    vertices_m.push(key.map(|v| v as f64 * VERTEX_GRID_M));
                    id
                });
            }
            let [a, b, c] = triangle.map(|i| vertices_m[i as usize]);
            if norm(cross(sub(b, a), sub(c, a))) <= VERTEX_GRID_M.powi(2) {
                return Err(invalid("degenerate triangle after vertex welding"));
            }
            for [a, b] in [
                [triangle[0], triangle[1]],
                [triangle[1], triangle[2]],
                [triangle[2], triangle[0]],
            ] {
                edges
                    .entry((a.min(b), a.max(b)))
                    .or_default()
                    .push((face, a < b));
            }
            triangles.push(triangle);
        }
        let mut neighbours = vec![Vec::new(); triangles.len()];
        for uses in edges.values() {
            if uses.len() != 2 || uses[0].1 == uses[1].1 {
                return Err(invalid(
                    "shell must have exactly two opposite uses of every edge",
                ));
            }
            neighbours[uses[0].0].push(uses[1].0);
            neighbours[uses[1].0].push(uses[0].0);
        }
        let mut visited = BTreeSet::new();
        let mut pending = vec![0];
        while let Some(face) = pending.pop() {
            if visited.insert(face) {
                pending.extend(&neighbours[face]);
            }
        }
        if visited.len() != triangles.len() {
            return Err(invalid("disconnected shells are unsupported"));
        }
        // Edge manifoldness alone permits pinched vertices. Each vertex's
        // incident faces must form one connected fan through its incident edges.
        let mut incident_faces = vec![BTreeSet::new(); vertices_m.len()];
        for (face, triangle) in triangles.iter().enumerate() {
            for vertex in triangle {
                incident_faces[*vertex as usize].insert(face);
            }
        }
        for incident in incident_faces {
            let mut pending = vec![*incident.first().ok_or_else(|| invalid("unused vertex"))?];
            let mut reached = BTreeSet::new();
            while let Some(face) = pending.pop() {
                if reached.insert(face) {
                    pending.extend(neighbours[face].iter().filter(|i| incident.contains(i)));
                }
            }
            if reached != incident {
                return Err(invalid("nonmanifold vertex fan"));
            }
        }
        validate_intersections(&vertices_m, &triangles)?;
        let solid = Self {
            vertices_m,
            triangles,
        };
        let mass = solid.mass_properties(1.0)?;
        if mass.volume_m3 <= VERTEX_GRID_M.powi(3) {
            return Err(invalid(
                "shell must have positive volume and outward winding",
            ));
        }
        Ok(solid)
    }

    pub fn bounding_box(&self) -> BoundingBox {
        let mut result = BoundingBox {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        };
        for point in &self.vertices_m {
            for (axis, value) in point.iter().enumerate() {
                result.min[axis] = result.min[axis].min(*value);
                result.max[axis] = result.max[axis].max(*value);
            }
        }
        result
    }

    pub fn mass_properties(&self, density_kg_per_m3: f64) -> Result<MassProperties> {
        if !density_kg_per_m3.is_finite() || density_kg_per_m3 < 0.0 {
            return Err(invalid("density must be finite and nonnegative"));
        }
        let origin = self.vertices_m[0];
        let mut volume = 0.0;
        let mut area = 0.0;
        let mut moment = [0.0; 3];
        for triangle in &self.triangles {
            let [a, b, c] = triangle.map(|i| sub(self.vertices_m[i as usize], origin));
            let signed_volume = dot(a, cross(b, c)) / 6.0;
            volume += signed_volume;
            area += norm(cross(sub(b, a), sub(c, a))) / 2.0;
            for axis in 0..3 {
                moment[axis] += signed_volume * (a[axis] + b[axis] + c[axis]) / 4.0;
            }
        }
        if !volume.is_finite() || volume.abs() <= VERTEX_GRID_M.powi(3) {
            return Err(invalid("shell volume is zero or non-finite"));
        }
        let result = MassProperties {
            volume_m3: volume,
            area_m2: area,
            mass_kg: volume * density_kg_per_m3,
            center_of_mass: std::array::from_fn(|i| origin[i] + moment[i] / volume),
        };
        if !result.mass_kg.is_finite()
            || !result.area_m2.is_finite()
            || result.center_of_mass.iter().any(|v| !v.is_finite())
        {
            return Err(invalid("non-finite mass properties"));
        }
        Ok(result)
    }
}

/// Parse binary or strict ASCII STL, converting the explicitly supplied unit once.
/// Facet normals and binary colour attributes are not geometric truth.
pub fn read_stl_solid(bytes: &[u8], source_unit: LengthUnit) -> Result<TriangleSolid> {
    if bytes.len() > MAX_STL_BYTES {
        return Err(invalid("STL exceeds 4 MiB input budget"));
    }
    let factor = source_unit.to_meters_factor();
    let mut triangles = Vec::new();
    if bytes.len() >= 84 {
        let count = u32::from_le_bytes(
            bytes[80..84]
                .try_into()
                .map_err(|_| invalid("invalid binary count"))?,
        ) as usize;
        if count <= MAX_STL_TRIANGLES && bytes.len() == 84 + count * 50 {
            for record in bytes[84..].chunks_exact(50) {
                let mut triangle = [[0.0; 3]; 3];
                for (index, value) in triangle.iter_mut().flatten().enumerate() {
                    let offset = 12 + index * 4;
                    *value = f32::from_le_bytes(
                        record[offset..offset + 4]
                            .try_into()
                            .map_err(|_| invalid("invalid binary vertex"))?,
                    ) as f64
                        * factor;
                }
                triangles.push(triangle);
            }
            return TriangleSolid::from_triangles(&triangles);
        }
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid("invalid binary STL length or non-UTF8 ASCII STL"))?;
    let mut lines = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::trim);
    let start = lines.next().ok_or_else(|| invalid("empty STL"))?;
    let Some(name) = start.strip_prefix("solid") else {
        return Err(invalid("ASCII STL must start with solid"));
    };
    if !name.is_empty() && !name.starts_with(char::is_whitespace) {
        return Err(invalid("invalid solid header"));
    }
    loop {
        let line = lines.next().ok_or_else(|| invalid("missing endsolid"))?;
        if let Some(end_name) = line.strip_prefix("endsolid") {
            if end_name.trim() != name.trim() || lines.next().is_some() {
                return Err(invalid("mismatched endsolid or trailing data"));
            }
            break;
        }
        let normal: Vec<_> = line.split_whitespace().collect();
        if normal.len() != 5
            || normal[..2] != ["facet", "normal"]
            || normal[2..]
                .iter()
                .any(|n| n.parse::<f64>().map_or(true, |v| !v.is_finite()))
        {
            return Err(invalid("invalid facet normal"));
        }
        if lines.next() != Some("outer loop") {
            return Err(invalid("missing outer loop"));
        }
        let mut triangle = [[0.0; 3]; 3];
        for point in &mut triangle {
            let words: Vec<_> = lines
                .next()
                .ok_or_else(|| invalid("missing vertex"))?
                .split_whitespace()
                .collect();
            if words.len() != 4 || words[0] != "vertex" {
                return Err(invalid("invalid vertex"));
            }
            for axis in 0..3 {
                point[axis] = words[axis + 1]
                    .parse::<f64>()
                    .map_err(|_| invalid("invalid vertex number"))?
                    * factor;
            }
        }
        if lines.next() != Some("endloop") || lines.next() != Some("endfacet") {
            return Err(invalid("missing endloop/endfacet"));
        }
        triangles.push(triangle);
        if triangles.len() > MAX_STL_TRIANGLES {
            return Err(invalid("STL exceeds triangle budget"));
        }
    }
    TriangleSolid::from_triangles(&triangles)
}

fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(format!("STL solid: {message}"))
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// Deterministic x-axis sweep. A fixed pair budget bounds pathological meshes.
fn validate_intersections(vertices: &[[f64; 3]], triangles: &[[u32; 3]]) -> Result<()> {
    let points: Vec<_> = triangles
        .iter()
        .map(|t| t.map(|i| vertices[i as usize]))
        .collect();
    let bounds: Vec<_> = points
        .iter()
        .map(|t| {
            let min = std::array::from_fn::<_, 3, _>(|i| {
                t.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min)
            });
            let max = std::array::from_fn::<_, 3, _>(|i| {
                t.iter().map(|p| p[i]).fold(f64::NEG_INFINITY, f64::max)
            });
            (min, max)
        })
        .collect();
    let mut order: Vec<_> = (0..triangles.len()).collect();
    order.sort_by(|a, b| bounds[*a].0[0].total_cmp(&bounds[*b].0[0]).then(a.cmp(b)));
    let mut pairs = 0;
    for (position, &a) in order.iter().enumerate() {
        for &b in &order[position + 1..] {
            if bounds[b].0[0] > bounds[a].1[0] + VERTEX_GRID_M {
                break;
            }
            pairs += 1;
            if pairs > 1_000_000 {
                return Err(invalid("intersection pair budget exceeded"));
            }
            if (1..3).any(|i| {
                bounds[b].0[i] > bounds[a].1[i] + VERTEX_GRID_M
                    || bounds[a].0[i] > bounds[b].1[i] + VERTEX_GRID_M
            }) {
                continue;
            }
            let shared: Vec<_> = triangles[a]
                .iter()
                .filter(|i| triangles[b].contains(i))
                .map(|i| vertices[*i as usize])
                .collect();
            if triangles_intersect(points[a], points[b], &shared) {
                return Err(invalid("self-intersecting triangles"));
            }
        }
    }
    Ok(())
}

fn allowed_contact(p: [f64; 3], shared: &[[f64; 3]]) -> bool {
    if shared.iter().any(|q| norm(sub(p, *q)) <= VERTEX_GRID_M) {
        return true;
    }
    if shared.len() == 2 {
        let edge = sub(shared[1], shared[0]);
        let t = dot(sub(p, shared[0]), edge) / dot(edge, edge);
        let q = std::array::from_fn(|i| shared[0][i] + t * edge[i]);
        return (0.0..=1.0).contains(&t) && norm(sub(p, q)) <= VERTEX_GRID_M;
    }
    false
}

fn triangles_intersect(a: [[f64; 3]; 3], b: [[f64; 3]; 3], shared: &[[f64; 3]]) -> bool {
    let normal = cross(sub(b[1], b[0]), sub(b[2], b[0]));
    let normal = normal.map(|v| v / norm(normal));
    let distances = a.map(|p| dot(sub(p, b[0]), normal));
    if distances.iter().all(|d| d.abs() <= VERTEX_GRID_M) {
        let axis = (0..3)
            .max_by(|i, j| normal[*i].abs().total_cmp(&normal[*j].abs()))
            .unwrap_or(0);
        let uv = |p: [f64; 3]| [p[(axis + 1) % 3], p[(axis + 2) % 3]];
        let orient = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
            (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
        };
        let inside = |p: [f64; 3], t: [[f64; 3]; 3]| {
            let signs = [
                orient(uv(t[0]), uv(t[1]), uv(p)),
                orient(uv(t[1]), uv(t[2]), uv(p)),
                orient(uv(t[2]), uv(t[0]), uv(p)),
            ];
            let tolerance = VERTEX_GRID_M
                * norm(sub(t[1], t[0]))
                    .max(norm(sub(t[2], t[1])))
                    .max(norm(sub(t[0], t[2])));
            signs.iter().all(|s| *s >= -tolerance) || signs.iter().all(|s| *s <= tolerance)
        };
        for (first, second) in [(a, b), (b, a)] {
            if first
                .iter()
                .any(|p| inside(*p, second) && !allowed_contact(*p, shared))
            {
                return true;
            }
        }
        for i in 0..3 {
            for j in 0..3 {
                let p = uv(a[i]);
                let q = uv(a[(i + 1) % 3]);
                let r = uv(b[j]);
                let s = uv(b[(j + 1) % 3]);
                let x = orient(p, q, r);
                let y = orient(p, q, s);
                let z = orient(r, s, p);
                let w = orient(r, s, q);
                let tolerance_a = VERTEX_GRID_M * norm(sub(a[(i + 1) % 3], a[i]));
                let tolerance_b = VERTEX_GRID_M * norm(sub(b[(j + 1) % 3], b[j]));
                if ((x > tolerance_a && y < -tolerance_a) || (y > tolerance_a && x < -tolerance_a))
                    && ((z > tolerance_b && w < -tolerance_b)
                        || (w > tolerance_b && z < -tolerance_b))
                {
                    return true;
                }
            }
        }
        return false;
    }
    for (first, second) in [(a, b), (b, a)] {
        let n = cross(sub(second[1], second[0]), sub(second[2], second[0]));
        let n = n.map(|v| v / norm(n));
        for i in 0..3 {
            let p = first[i];
            let q = first[(i + 1) % 3];
            let d = dot(sub(p, second[0]), n);
            let e = dot(sub(q, second[0]), n);
            if (d > VERTEX_GRID_M && e > VERTEX_GRID_M)
                || (d < -VERTEX_GRID_M && e < -VERTEX_GRID_M)
                || (d - e).abs() < 1e-18
            {
                continue;
            }
            let t = d / (d - e);
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let hit = std::array::from_fn(|axis| p[axis] + t * (q[axis] - p[axis]));
            let mut inside = true;
            for j in 0..3 {
                let edge = sub(second[(j + 1) % 3], second[j]);
                if dot(cross(edge, sub(hit, second[j])), n) < -VERTEX_GRID_M * norm(edge) {
                    inside = false;
                    break;
                }
            }
            if inside && !allowed_contact(hit, shared) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_checks_preserve_shared_edges_and_refuse_crossings() {
        let a = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
        let adjacent = [[1., 0., 0.], [1., 1., 0.], [0., 1., 0.]];
        assert!(!triangles_intersect(
            a,
            adjacent,
            &[[1., 0., 0.], [0., 1., 0.]]
        ));
        assert!(triangles_intersect(
            a,
            [[0.1, 0.1, 0.], [0.5, 0.1, 0.], [0.1, 0.5, 0.]],
            &[]
        ));
        assert!(triangles_intersect(
            a,
            [[0.2, 0.2, -1.], [0.2, 0.2, 1.], [0.5, 0.2, 0.]],
            &[]
        ));
        let mut triangles = box_triangles();
        for t in &mut triangles {
            for p in t {
                if norm(sub(*p, [80., 60., 6.])) < 1e-12 {
                    *p = [-20., -20., 3.];
                }
            }
        }
        assert!(read_stl_solid(&ascii(&triangles), LengthUnit::Millimeter).is_err());
    }

    fn box_triangles() -> Vec<[[f64; 3]; 3]> {
        let p = [
            [0., 0., 0.],
            [80., 0., 0.],
            [80., 60., 0.],
            [0., 60., 0.],
            [0., 0., 6.],
            [80., 0., 6.],
            [80., 60., 6.],
            [0., 60., 6.],
        ];
        [
            [0, 3, 2],
            [0, 2, 1],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ]
        .map(|t| t.map(|i| p[i]))
        .to_vec()
    }
    fn ascii(triangles: &[[[f64; 3]; 3]]) -> Vec<u8> {
        let mut text = "solid plate\n".to_string();
        for t in triangles {
            text.push_str("facet normal 0 0 0\nouter loop\n");
            for [x, y, z] in t {
                text.push_str(&format!("vertex {x} {y} {z}\n"));
            }
            text.push_str("endloop\nendfacet\n");
        }
        text.push_str("endsolid plate\n");
        text.into_bytes()
    }
    #[test]
    fn ascii_binary_analytic_properties_and_units() {
        let triangles = box_triangles();
        let mut binary = vec![0; 84];
        binary[..5].copy_from_slice(b"solid"); // A binary header may start with solid.
        binary[80..84].copy_from_slice(&12u32.to_le_bytes());
        for triangle in &triangles {
            binary.extend_from_slice(&[0; 12]);
            for v in triangle.iter().flatten() {
                binary.extend_from_slice(&(*v as f32).to_le_bytes());
            }
            binary.extend_from_slice(&[0; 2]);
        }
        for bytes in [ascii(&triangles), binary] {
            let mesh = read_stl_solid(&bytes, LengthUnit::Millimeter).expect("box");
            let m = mesh.mass_properties(1000.).expect("properties");
            assert!((m.volume_m3 - 2.88e-5).abs() < 1e-12);
            assert!((m.area_m2 - 0.01128).abs() < 1e-12);
            for (actual, expected) in m.center_of_mass.into_iter().zip([0.04, 0.03, 0.003]) {
                assert!((actual - expected).abs() < 1e-12);
            }
            for (actual, expected) in mesh
                .bounding_box()
                .size()
                .into_iter()
                .zip([0.08, 0.06, 0.006])
            {
                assert!((actual - expected).abs() < 1e-12);
            }
            let metres = read_stl_solid(&bytes, LengthUnit::Meter).expect("explicit metre");
            assert!(
                (metres.mass_properties(1.).expect("properties").volume_m3 - 28800.).abs() < 1e-6
            );
        }
    }
    #[test]
    fn refuses_open_inverted_disconnected_and_degenerate_meshes() {
        let mut triangles = box_triangles();
        assert!(read_stl_solid(&ascii(&triangles[..11]), LengthUnit::Millimeter).is_err());
        for t in &mut triangles {
            t.swap(0, 1);
        }
        assert!(read_stl_solid(&ascii(&triangles), LengthUnit::Millimeter).is_err());
        triangles = box_triangles();
        let translated: Vec<_> = triangles
            .iter()
            .map(|t| t.map(|p| [p[0] + 100., p[1], p[2]]))
            .collect();
        triangles.extend(translated);
        assert!(read_stl_solid(&ascii(&triangles), LengthUnit::Millimeter).is_err());
        triangles = box_triangles();
        triangles[0][1] = triangles[0][0];
        assert!(read_stl_solid(&ascii(&triangles), LengthUnit::Millimeter).is_err());
    }
    #[test]
    fn rejects_nonfinite_truncated_and_trailing_data() {
        let bytes = ascii(&box_triangles());
        let bad = String::from_utf8(bytes.clone()).expect("ascii").replacen(
            "vertex 0 0 0",
            "vertex NaN 0 0",
            1,
        );
        assert!(read_stl_solid(bad.as_bytes(), LengthUnit::Millimeter).is_err());
        assert!(read_stl_solid(&bytes[..bytes.len() - 20], LengthUnit::Millimeter).is_err());
        let mut trailing = bytes;
        trailing.extend_from_slice(b"garbage");
        assert!(read_stl_solid(&trailing, LengthUnit::Millimeter).is_err());
    }
}
