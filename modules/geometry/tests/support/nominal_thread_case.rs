//! Independent runtime oracle: authored dimensions, piece/surface/triangle integrals.
use opencad_core::{Length, Result, TopoRefId};
use opencad_geometry::{
    BoundingBox, ExtrudeExtent, ExtrudeOperation, GeometryKernel, KernelBody, MetricThreadSize,
    NominalMetricThread, NominalThreadInput, SketchPlacement, SolvedCircle, SolvedSketch,
    TessellationSettings, ThreadHandedness,
};
use std::{
    collections::BTreeMap,
    f64::consts::{PI, TAU},
};

pub fn near(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a:e} != {b:e} tol {t:e}");
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
pub fn dimensions(size: MetricThreadSize) -> (f64, f64) {
    let (d, p) = match size {
        MetricThreadSize::M3 => (3., 0.5),
        MetricThreadSize::M4 => (4., 0.7),
        MetricThreadSize::M5 => (5., 0.8),
        MetricThreadSize::M6 => (6., 1.),
        MetricThreadSize::M8 => (8., 1.25),
        MetricThreadSize::M10 => (10., 1.5),
        MetricThreadSize::M12 => (12., 1.75),
    };
    (d * 1e-3, p * 1e-3)
}
pub fn oracle_radius(d: f64, p: f64, z: f64, theta: f64, hand: ThreadHandedness) -> f64 {
    let sign = if hand == ThreadHandedness::Right {
        1.
    } else {
        -1.
    };
    let q = (z / p - sign * theta / TAU).rem_euclid(1.);
    let t = q.min(1. - q);
    let depth = p * 0.541_265_877_365_274_1;
    if t <= 0.0625 {
        d / 2.
    } else if t >= 0.375 {
        d / 2. - depth
    } else {
        d / 2. - p * 1.732_050_807_568_877_2 * (t - 0.0625)
    }
}
pub fn placement(tilted: bool) -> SketchPlacement {
    if !tilted {
        return SketchPlacement::global_xy();
    }
    let c = 0.5_f64.sqrt();
    SketchPlacement {
        origin_m: [0.013, -0.021, 0.007],
        x_axis_m: [c, c, 0.],
        y_axis_m: [-0.5, 0.5, c],
    }
}
pub fn stock(
    kernel: &dyn GeometryKernel,
    size: MetricThreadSize,
    turns: u32,
    plane: SketchPlacement,
) -> Result<KernelBody> {
    let (d, p) = dimensions(size);
    let profile = SolvedSketch {
        profile_ref: "thread-stock".into(),
        points: Vec::new(),
        closed: true,
        circle: Some(SolvedCircle {
            center_m: [0., 0.],
            radius_m: d / 2.,
        }),
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: Some(plane),
    };
    kernel.extrude(
        kernel.make_wire_from_sketch(&profile)?,
        ExtrudeExtent::Distance {
            length: Length::from_meters(f64::from(turns) * p),
        },
        ExtrudeOperation::NewBody,
        None,
        plane.extrude_direction_m(),
    )
}
fn expected_bounds(
    d: f64,
    p: f64,
    turns: u32,
    hand: ThreadHandedness,
    origin: [f64; 3],
    axes: [[f64; 3]; 3],
) -> BoundingBox {
    let sign = if hand == ThreadHandedness::Right {
        1.
    } else {
        -1.
    };
    let length = f64::from(turns) * p;
    let mut b = BoundingBox {
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };
    let mut visit = |r: f64, theta: f64, z: f64| {
        let local = [r * theta.cos(), r * theta.sin(), z];
        for i in 0..3 {
            let v = origin[i] + (0..3).map(|j| axes[j][i] * local[j]).sum::<f64>();
            b.min[i] = b.min[i].min(v);
            b.max[i] = b.max[i].max(v);
        }
    };
    // Dense independent knot-helices/cap boundaries, not the runtime critical-point helper.
    for i in 0..=4096 {
        let theta = TAU * f64::from(i) / 4096.;
        for q in [0., 1. / 16., 3. / 8., 5. / 8., 15. / 16., 1.] {
            let r = oracle_radius(d, p, q * p, 0., ThreadHandedness::Right);
            for k in -2..=turns as i32 + 2 {
                let z = p * (q + sign * theta / TAU + f64::from(k));
                if z >= 0. && z <= length {
                    visit(r, theta, z);
                }
            }
        }
        for z in [0., length] {
            visit(oracle_radius(d, p, z, theta, hand), theta, z);
        }
    }
    b
}

pub fn case(
    kernel: &dyn GeometryKernel,
    size: MetricThreadSize,
    hand: ThreadHandedness,
    turns: u32,
    tilted: bool,
    reversed: bool,
    native: bool,
) -> (KernelBody, NominalThreadInput) {
    let (d, p) = dimensions(size);
    let length = f64::from(turns) * p;
    let plane = placement(tilted);
    let axis = plane.extrude_direction_m();
    let body = stock(kernel, size, turns, plane).unwrap();
    let initial = kernel.mass_properties(&body, 1.).unwrap();
    near(
        initial.volume_m3,
        PI * d * d * length / 4.,
        initial.volume_m3 * 1e-8,
    );
    let profile = NominalMetricThread::new(size, hand, turns).unwrap();
    let input = NominalThreadInput {
        profile,
        axis_hint: axis.map(|v| if reversed { -v } else { v }),
        radial_axis: plane.x_axis_m,
    };
    let output = kernel
        .cut_nominal_metric_thread(body.clone(), &input)
        .unwrap_or_else(|e| {
            panic!("{size:?} {hand:?} turns{turns} tilted{tilted} reverse{reversed}: {e}")
        });
    let mass = kernel.mass_properties(&output, 2700.).unwrap();
    let major = d / 2.;
    let root = major - p * 0.541_265_877_365_274_1;
    let piece = p * (root * root / 4. + major * major / 8.)
        + 2. * (major.powi(3) - root.powi(3)) / (3. * 3_f64.sqrt());
    let section = PI * piece / p;
    let volume = section * length;
    let a = p / TAU;
    // Independent midpoint integration of the flank surface metric sqrt(4r²+3a²).
    let samples = 8192;
    let delta = (major - root) / f64::from(samples);
    let flank = (0..samples)
        .map(|i| {
            let r = root + delta * (f64::from(i) + 0.5);
            (4. * r * r + 3. * a * a).sqrt() * delta
        })
        .sum::<f64>();
    let area = TAU * f64::from(turns) * (p * (root / 4. + major / 8.) + 2. / 3_f64.sqrt() * flank)
        + 2. * section;
    let tolerance = if native { 2e-5 } else { 1e-10 };
    near(mass.volume_m3, volume, volume * tolerance);
    near(mass.area_m2, area, area * tolerance);
    near(mass.mass_kg, 2700. * volume, 2700. * volume * tolerance);
    for (i, component) in axis.into_iter().enumerate() {
        near(
            mass.center_of_mass[i],
            plane.origin_m[i] + component * length / 2.,
            if native { 2e-8 } else { 1e-12 },
        );
    }
    near(
        kernel.mass_properties(&body, 1.).unwrap().volume_m3,
        initial.volume_m3,
        initial.volume_m3 * 1e-12,
    );
    assert!(kernel.face_derivation_history(&output).is_empty());
    let origin = if reversed {
        std::array::from_fn(|i| plane.origin_m[i] + axis[i] * length)
    } else {
        plane.origin_m
    };
    let axes = [
        plane.x_axis_m,
        cross(input.axis_hint, plane.x_axis_m),
        input.axis_hint,
    ];
    let expected = expected_bounds(d, p, turns, hand, origin, axes);
    let bounds = kernel.bounding_box(&output).unwrap();
    for i in 0..3 {
        near(
            bounds.min[i],
            expected.min[i],
            if native { 2e-6 } else { 5e-9 },
        );
        near(
            bounds.max[i],
            expected.max[i],
            if native { 2e-6 } else { 5e-9 },
        );
    }
    let mesh = kernel
        .tessellate(&output, &TessellationSettings::default())
        .unwrap();
    assert!(mesh.has_triangle_face_ids());
    assert!(mesh.triangle_count() > 100);
    let mut edges = BTreeMap::<([u32; 3], [u32; 3]), i32>::new();
    let mut mesh_volume = 0.;
    let mut side_count = 0;
    for tri in mesh.indices.chunks_exact(3) {
        let points = tri
            .iter()
            .map(|i| mesh.positions[*i as usize])
            .collect::<Vec<_>>();
        for i in 0..3 {
            *edges
                .entry((
                    points[i].map(f32::to_bits),
                    points[(i + 1) % 3].map(f32::to_bits),
                ))
                .or_default() += 1;
        }
        let [a, b, c] = std::array::from_fn(|i| sub(points[i].map(f64::from), origin));
        assert!(dot(cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(c, a))) > 0.);
        mesh_volume += dot(a, cross(b, c)) / 6.;
    }
    assert!(edges
        .iter()
        .all(|(&(a, b), &count)| count == 1 && edges.get(&(b, a)) == Some(&1)));
    near(mesh_volume, volume, volume * 0.004);
    for vertex in &mesh.positions {
        let offset = sub(vertex.map(f64::from), origin);
        let local = axes.map(|a| dot(a, offset));
        if local[2] > 1e-8 && local[2] < length - 1e-8 {
            let radius = local[0].hypot(local[1]);
            let expected = oracle_radius(d, p, local[2], local[1].atan2(local[0]), hand);
            near(radius, expected, if native { 2e-8 } else { 1e-8 });
            side_count += 1;
        }
    }
    assert!(side_count > 20);
    for id in mesh
        .triangle_face_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
    {
        kernel
            .assign_face_ref(
                &output,
                id,
                TopoRefId::new(format!("ref:thread-{id}")).unwrap(),
            )
            .unwrap();
    }
    assert!(kernel
        .assign_face_ref(
            &output,
            u64::MAX / 7,
            TopoRefId::new("ref:missing-thread-face").unwrap()
        )
        .is_err());
    (output, input)
}
