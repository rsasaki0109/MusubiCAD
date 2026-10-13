use opencad_geometry::MeshSet;
use std::collections::BTreeMap;
pub fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
pub fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
pub fn integrated(mesh: &MeshSet, check_normals: bool) -> (f64, [f64; 3]) {
    let origin = mesh.positions[0].map(f64::from);
    let mut volume = 0.;
    let mut first = [0.; 3];
    let mut edges = BTreeMap::new();
    assert_eq!(mesh.triangle_face_ids.len(), mesh.triangle_count());
    for (i, t) in mesh.indices.chunks_exact(3).enumerate() {
        let world: [[f64; 3]; 3] =
            std::array::from_fn(|j| mesh.positions[t[j] as usize].map(f64::from));
        let p = world.map(|p| sub(p, origin));
        let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
        assert!(dot(n, n) > 0.);
        if check_normals {
            let norm = mesh.normals[t[0] as usize].map(f64::from);
            assert!(dot(n, norm) > 0.);
        }
        let v = dot(p[0], cross(p[1], p[2])) / 6.;
        volume += v;
        for j in 0..3 {
            first[j] += v * (p[0][j] + p[1][j] + p[2][j]) / 4.;
        }
        assert!(mesh.triangle_face_ids[i] > 0);
        for j in 0..3 {
            let a = world[j].map(|v| (if v == 0. { 0. } else { v as f32 }).to_bits());
            let b = world[(j + 1) % 3].map(|v| (if v == 0. { 0. } else { v as f32 }).to_bits());
            let (key, sign) = if a < b { ((a, b), 1) } else { ((b, a), -1) };
            let entry = edges.entry(key).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += sign;
        }
    }
    assert!(
        edges.values().all(|v| *v == (2, 0)),
        "every geometric edge must pair oppositely"
    );
    assert!(volume > 0.);
    (
        volume,
        std::array::from_fn(|i| origin[i] + first[i] / volume),
    )
}
