use opencad_core::Length;
use opencad_geometry::*;
use std::f64::consts::PI;
#[path = "support/arc_sector_cases.rs"]
mod cases;
#[test]
fn physical_mock_sector_minor_half_major_and_reversed_arcs_match_analytic_measurements() {
    cases::physical(&MockGeometryKernel::with_analytic_primitives());
}
#[test]
fn physical_mock_sector_placement_symmetric_extent_mesh_and_face_history_are_genuine() {
    cases::placement_mesh(&MockGeometryKernel::with_analytic_primitives(), true);
}
#[test]
fn unsupported_sector_inputs_and_queries_refuse_without_changing_existing_bodies() {
    let k = MockGeometryKernel::with_analytic_primitives();
    let p = cases::profile(PI / 2., false);
    let base = cases::body(
        &k,
        &p,
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.006),
        },
        [0., 0., 1.],
    );
    let saved = k.mass_properties(&base, 1.).unwrap();
    let bounds = k.bounding_box(&base).unwrap();
    for case in ["gap", "wrong_center", "nan", "duplicate", "extra", "chord"] {
        let mut bad = p.clone();
        match case {
            "gap" => {
                if let ProfileSegment::Line { end_m, .. } = &mut bad.segments[1] {
                    end_m[0] += 1e-10;
                }
            }
            "wrong_center" => {
                for s in &mut bad.segments[1..] {
                    if let ProfileSegment::Line { start_m, end_m } = s {
                        if (start_m[0] - 0.002).abs() < 1e-12 && (start_m[1] + 0.003).abs() < 1e-12
                        {
                            start_m[0] += 0.001;
                        }
                        if (end_m[0] - 0.002).abs() < 1e-12 && (end_m[1] + 0.003).abs() < 1e-12 {
                            end_m[0] += 0.001;
                        }
                    }
                }
            }
            "nan" => {
                if let ProfileSegment::Arc { mid_m, .. } = &mut bad.segments[0] {
                    mid_m[0] = f64::NAN;
                }
            }
            "duplicate" => {
                if let ProfileSegment::Arc { start_m, mid_m, .. } = &mut bad.segments[0] {
                    *mid_m = *start_m;
                }
            }
            "extra" => bad.segments.push(bad.segments[1]),
            _ => {
                bad.segments.remove(1);
            }
        }
        assert!(k.make_wire_from_sketch(&bad).is_err(), "{case}");
        assert_eq!(k.mass_properties(&base, 1.).unwrap(), saved);
        assert_eq!(k.bounding_box(&base).unwrap(), bounds);
    }
    let wire = k.make_wire_from_sketch(&p).unwrap();
    assert!(k
        .extrude(
            wire,
            ExtrudeExtent::ThroughAll,
            ExtrudeOperation::NewBody,
            None,
            [0., 0., 1.]
        )
        .is_err());
    let wire = k.make_wire_from_sketch(&p).unwrap();
    assert!(k
        .extrude(
            wire,
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.006)
            },
            ExtrudeOperation::NewBody,
            None,
            [1., 0., 1.]
        )
        .is_err());
    assert!(k
        .boolean(base.clone(), base.clone(), BooleanOp::Subtract)
        .is_err());
    assert!(k.cylindrical_stock_geometry(&base, [0., 0., 1.]).is_err());
    assert!(k
        .tessellate(
            &base,
            &TessellationSettings {
                linear_deflection: 1e-20,
                angular_deflection_deg: 1e-8
            }
        )
        .is_err());
    let near_full = cases::body(
        &k,
        &cases::profile(2. * PI - 1e-5, false),
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.006),
        },
        [0., 0., 1.],
    );
    assert!(k
        .tessellate(
            &near_full,
            &TessellationSettings {
                linear_deflection: 1e-5,
                angular_deflection_deg: 720. / 8192.
            }
        )
        .unwrap_err()
        .to_string()
        .contains("angular"));
    assert_eq!(k.mass_properties(&base, 1.).unwrap(), saved);
}
