use opencad_geometry::*;
#[path = "support/countersink_cases.rs"]
mod cases;
#[test]
fn analytic_countersink_has_exact_union_and_closed_physical_mesh() {
    cases::physical(&MockGeometryKernel::with_analytic_primitives(), true);
}
#[test]
fn unsupported_or_invalid_countersinks_preserve_original_stock() {
    let k = MockGeometryKernel::with_analytic_primitives();
    cases::refusals(&k);
    let legacy = MockGeometryKernel::new();
    assert!(legacy
        .cut_countersunk_stock(cases::stock(&legacy), &cases::input(false, false))
        .is_err());
    let base = cases::stock(&k);
    let cut = k
        .cut_countersunk_stock(base.clone(), &cases::input(false, false))
        .unwrap();
    assert!(k.boolean(cut, base, BooleanOp::Subtract).is_err());
}
#[test]
fn conical_mesh_quality_and_face_ownership_are_bounded() {
    let k = MockGeometryKernel::with_analytic_primitives();
    let base = cases::stock(&k);
    let c = cases::input(false, false);
    let cut = k.cut_countersunk_stock(base, &c).unwrap();
    for q in [
        TessellationSettings {
            linear_deflection: 0.,
            angular_deflection_deg: 6.,
        },
        TessellationSettings {
            linear_deflection: 1e-20,
            angular_deflection_deg: 6.,
        },
        TessellationSettings {
            linear_deflection: 1e-6,
            angular_deflection_deg: 181.,
        },
        TessellationSettings {
            linear_deflection: 1e-6,
            angular_deflection_deg: 0.001,
        },
    ] {
        assert!(k.tessellate(&cut, &q).is_err());
    }
    let q = TessellationSettings {
        linear_deflection: 1e-6,
        angular_deflection_deg: 6.,
    };
    let mesh = k.tessellate(&cut, &q).unwrap();
    let mut cone = 0;
    for p in &mesh.positions {
        let [x, y, z] = p.map(f64::from);
        if z >= 0.01 - c.seat_depth_m - 3e-9 && x > 0.006 && x < 0.018 && y > 0.003 && y < 0.015 {
            let radius = (x - c.entry.point_m[0]).hypot(y - c.entry.point_m[1]);
            if radius < 0.0061 {
                let expected = c.seat_radius_m
                    - (0.01 - z) * (c.seat_radius_m - c.bore_radius_m) / c.seat_depth_m;
                assert!((radius - expected).abs() < 8e-9);
                cone += 1;
            }
        }
    }
    assert!(cone > 100);
}

#[test]
fn countersink_negative_extrusion_remains_closed_and_physical() {
    cases::negative_extrusion(&MockGeometryKernel::with_analytic_primitives(), true);
}
