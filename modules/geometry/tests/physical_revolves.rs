use opencad_geometry::*;
use std::f64::consts::{PI, TAU};
#[path = "support/revolve_cases.rs"]
mod cases;
#[test]
fn physical_mock_revolves_have_analytic_measurements_world_bounds_and_contacts() {
    cases::physical(&MockGeometryKernel::with_analytic_primitives());
}
#[test]
fn physical_mock_revolve_rigid_placement_retains_owned_face_ancestry() {
    cases::placed(&MockGeometryKernel::with_analytic_primitives(), true);
}
#[test]
fn portable_revolve_refusals_preserve_previous_physical_records() {
    let k = MockGeometryKernel::with_analytic_primitives();
    let p = cases::input(ProfilePlane::Xy, PI / 2., false, false);
    let b = k.revolve(&p).unwrap();
    let m = k.mass_properties(&b, 1.).unwrap();
    let bounds = k.bounding_box(&b).unwrap();
    for case in [
        "zero",
        "negative",
        "too_far",
        "nan",
        "infinite",
        "near_full",
        "crossing",
        "touching",
        "thin",
        "skew",
        "off_plane",
        "zero_axis",
        "nan_axis",
        "wrong_plane",
        "open",
        "extra",
        "arc",
        "gap",
        "circle",
        "cut",
        "join",
        "target",
        "placement",
    ] {
        let mut bad = p.clone();
        match case {
            "zero" => bad.angle_rad = 0.,
            "negative" => bad.angle_rad = -PI,
            "too_far" => bad.angle_rad = TAU + 1e-5,
            "nan" => bad.angle_rad = f64::NAN,
            "infinite" => bad.axis_origin_m[0] = f64::INFINITY,
            "near_full" => bad.angle_rad = TAU - 1e-7,
            "crossing" => bad.axis_origin_m[0] = 0.02,
            "touching" => bad.axis_origin_m[0] = 0.015,
            "thin" => {
                for p in &mut bad.sketch.points {
                    if p[0] > 0.02 {
                        p[0] = 0.015 + 1e-10;
                    }
                }
                bad.sketch.segments.clear();
            }
            "skew" => bad.axis_direction_m = [1., 1., 0.],
            "off_plane" => bad.axis_origin_m[2] = 1e-5,
            "zero_axis" => bad.axis_direction_m = [0.; 3],
            "nan_axis" => bad.axis_direction_m[1] = f64::NAN,
            "wrong_plane" => bad.profile_plane = ProfilePlane::Yz,
            "open" => bad.sketch.closed = false,
            "extra" => bad.sketch.points.push([0.02, 0.01]),
            "arc" => {
                bad.sketch.segments[0] = ProfileSegment::Arc {
                    start_m: bad.sketch.points[0],
                    mid_m: [0.02, -0.01],
                    end_m: bad.sketch.points[1],
                }
            }
            "gap" => {
                if let ProfileSegment::Line { end_m, .. } = &mut bad.sketch.segments[0] {
                    end_m[0] += 1e-10;
                }
            }
            "circle" => {
                bad.sketch.circle = Some(SolvedCircle {
                    center_m: [0.; 2],
                    radius_m: 0.01,
                })
            }
            "cut" => bad.operation = RevolveOperation::Cut,
            "join" => bad.operation = RevolveOperation::Join,
            "target" => bad.target = Some(b.clone()),
            _ => {
                bad.sketch.placement = Some(SketchPlacement {
                    origin_m: [0., 0., 0.001],
                    ..SketchPlacement::global_xy()
                })
            }
        }
        assert!(
            RectangularRevolveGeometry::from_input(&bad).is_err(),
            "{case}"
        );
        assert!(k.revolve(&bad).is_err(), "{case}");
        assert_eq!(k.mass_properties(&b, 1.).unwrap(), m);
        assert_eq!(k.bounding_box(&b).unwrap(), bounds);
    }
    assert!(k
        .boolean(b.clone(), b.clone(), BooleanOp::Subtract)
        .is_err());
    assert!(k.cylindrical_stock_geometry(&b, [0., 1., 0.]).is_err());
    for quality in [
        TessellationSettings {
            linear_deflection: 1e-20,
            angular_deflection_deg: 4.,
        },
        TessellationSettings {
            linear_deflection: 1e-5,
            angular_deflection_deg: 1e-9,
        },
        TessellationSettings {
            linear_deflection: f64::NAN,
            angular_deflection_deg: 4.,
        },
    ] {
        assert!(k.tessellate(&b, &quality).is_err());
    }
    assert_eq!(k.mass_properties(&b, 1.).unwrap(), m);
}
#[test]
fn physical_mock_in_plane_rotated_rectangle_and_axial_origin_shift_match_world_oracle() {
    cases::in_plane(&MockGeometryKernel::with_analytic_primitives());
}
