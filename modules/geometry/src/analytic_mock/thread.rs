//! ADR-068 integer screw-band partitions; no nearest-point seam welding.
use super::mesh::{Builder, FaceKey};
use super::{cross, dot, invalid, sub};
use crate::{MeshSet, NominalThreadGeometry, TessellationSettings, ThreadHandedness};
use opencad_core::Result;
use std::{collections::BTreeMap, f64::consts::TAU};

type Pair = [i64; 2];
fn clipped(
    poly: Vec<Pair>,
    sign: i64,
    turn: i64,
    n: i64,
    limit: i64,
    lower: bool,
) -> Result<Vec<Pair>> {
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let va = a[1] + sign * a[0] + turn * n - limit;
        let vb = b[1] + sign * b[0] + turn * n - limit;
        let ina = if lower { va >= 0 } else { va <= 0 };
        let inb = if lower { vb >= 0 } else { vb <= 0 };
        if ina {
            out.push(a);
        }
        if ina != inb {
            let delta = vb - va;
            let mut p = [0; 2];
            for j in 0..2 {
                let num = (b[j] - a[j]) * (-va);
                if num % delta != 0 {
                    return Err(invalid("nonintegral thread partition"));
                }
                p[j] = a[j] + num / delta;
            }
            out.push(p);
        }
    }
    out.dedup();
    if out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    Ok(out)
}

pub(super) fn mesh(
    g: NominalThreadGeometry,
    faces: &BTreeMap<FaceKey, u64>,
    quality: &TessellationSettings,
) -> Result<MeshSet> {
    g.validate()?;
    let linear = quality.linear_deflection;
    let plan = g.mesh_plan(quality)?;
    let n = i64::from(plan.angular_segments);
    let error = plan.interpolation_bound_m;
    let profile = g.profile();
    let p = profile.pitch().meters();
    let turns = i64::from(profile.turns());
    let sign = if profile.handedness() == ThreadHandedness::Right {
        1
    } else {
        -1
    };
    let knots = [0, n / 16, 3 * n / 8, 5 * n / 8, 15 * n / 16, n];
    let mut builder = Builder::new();
    let point = |u: i64, q: i64, k: i64| {
        let angle = u.rem_euclid(n);
        let z = q + sign * u + k * n;
        let phase = (z - sign * angle).rem_euclid(n) as f64 / n as f64;
        let radius = profile.radius_for_phase(phase);
        let theta = TAU * angle as f64 / n as f64;
        g.map([
            radius * theta.cos(),
            radius * theta.sin(),
            p * z as f64 / n as f64,
        ])
    };
    let emit = |builder: &mut Builder, points: [[f64; 3]; 3], face: usize| -> Result<()> {
        let normal = cross(sub(points[1], points[0]), sub(points[2], points[0]));
        if dot(normal, normal) <= 0. {
            return Err(invalid("degenerate thread partition"));
        }
        let converted = points.map(|p| p.map(|v| v as f32 as f64));
        let float_normal = cross(
            sub(converted[1], converted[0]),
            sub(converted[2], converted[0]),
        );
        let denominator = (dot(normal, normal) * dot(float_normal, float_normal)).sqrt();
        let residual = quality.angular_deflection_deg.to_radians() - plan.normal_bound_rad;
        if !denominator.is_finite()
            || denominator <= 0.
            || dot(normal, float_normal) / denominator < residual.cos()
        {
            return Err(invalid("thread angular quality below f32 normal precision"));
        }
        builder.triangle(points, normal, faces[&FaceKey::Thread(face)])
    };
    for k in -1..=turns + 1 {
        for u in 0..n {
            for (band, range) in knots.windows(2).enumerate() {
                let poly = clipped(
                    vec![
                        [u, range[0]],
                        [u + 1, range[0]],
                        [u + 1, range[1]],
                        [u, range[1]],
                    ],
                    sign,
                    k,
                    n,
                    0,
                    true,
                )?;
                let poly = clipped(poly, sign, k, n, turns * n, false)?;
                if poly.len() < 3 {
                    continue;
                }
                let area: i64 = (0..poly.len())
                    .map(|i| {
                        let a = poly[i];
                        let b = poly[(i + 1) % poly.len()];
                        a[0] * b[1] - a[1] * b[0]
                    })
                    .sum();
                if area <= 0 {
                    continue;
                }
                let points: Vec<_> = poly.iter().map(|v| point(v[0], v[1], k)).collect();
                let face = match band {
                    0 | 4 => 2,
                    1 => 4,
                    2 => 3,
                    _ => 5,
                };
                for i in 1..points.len() - 1 {
                    emit(&mut builder, [points[0], points[i], points[i + 1]], face)?;
                }
            }
        }
    }
    for (z, face) in [(0, 0), (turns * n, 1)] {
        for u in 0..n {
            // q is recovered from the canonical (angle,axial) key.
            let a = point(u, z - sign * u, 0);
            let b = point(u + 1, z - sign * (u + 1), 0);
            let center = g.map([0., 0., p * z as f64 / n as f64]);
            let points = if face == 0 {
                [center, b, a]
            } else {
                [center, a, b]
            };
            emit(&mut builder, points, face)?;
        }
    }
    if error + builder.error > linear {
        return Err(invalid("thread deflection below f32 coordinate error"));
    }
    builder.finish()
}
