//! ADR-111 independent physical, source, refusal and file/history acceptance.
use opencad_ai::DesignPatch;
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_desktop::{editor::*, hole_creation::*};
use opencad_feature::{FeatureDefinition, FeatureRegistry};
use opencad_file::{apply_patch_to_document, document_design_state, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::{json, Value};
use std::f64::consts::PI;

#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod mesh_evidence;

fn fixture() -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:native_hole_stock").unwrap(),
        "Native hole stock",
    ));
    let patch: DesignPatch = serde_json::from_str(include_str!(
        "../../../examples/agent/create_native_hole_stock_patch.json"
    ))
    .unwrap();
    apply_patch_to_document(&mut doc, &patch).unwrap();
    doc
}
fn intent(doc: &OcadDocument) -> HoleCreate {
    let recipe: Value = serde_json::from_str(include_str!(
        "../../../examples/agent/create_native_hole.json"
    ))
    .unwrap();
    let mut create: HoleCreate = serde_json::from_value(recipe["create"].clone()).unwrap();
    create.revision = feature_tree(doc).unwrap().revision;
    create
}
fn near(a: [f64; 3], b: [f64; 3], eps: f64) {
    for (a, b) in a.into_iter().zip(b) {
        assert!((a - b).abs() < eps, "{a} != {b}");
    }
}

fn measure<K: GeometryKernel>(doc: &OcadDocument, kernel: &K, bottom: bool, seat: u8, depth: f64) {
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            Some(&doc.semantic_refs),
        )
        .unwrap();
    let body = model.active_body().unwrap();
    let mass = kernel.mass_properties(body, 1.).unwrap();
    // Millimetre oracle, independent of the plan and kernel dimensions.
    let (r, big) = (3.3_f64, 5.5_f64);
    let d = if seat == 2 { 2.2 } else { 4. };
    let bore = PI * r * r * depth;
    let (extra, first) = match seat {
        0 => (0., 0.),
        1 => {
            let v = PI * (big * big - r * r) * d;
            (v, v * d / 2.)
        }
        _ => (
            PI * d * (big * big + big * r - 2. * r * r) / 3.,
            PI * d * d * (big * big + 2. * big * r - 3. * r * r) / 12.,
        ),
    };
    let volume = 12000. - bore - extra;
    let z_removed = if bottom {
        bore * depth / 2. + first
    } else {
        (bore + extra) * 10. - bore * depth / 2. - first
    };
    let center = [
        (12000. * 20. - (bore + extra) * 12.) / volume / 1000.,
        (12000. * 15. - (bore + extra) * 9.) / volume / 1000.,
        (12000. * 5. - z_removed) / volume / 1000.,
    ];
    assert!((mass.volume_m3 * 1e9 - volume).abs() < 0.0001, "{mass:?}");
    near(mass.center_of_mass, center, 1e-9);
    let bounds = kernel.bounding_box(body).unwrap();
    near(bounds.min, [0., 0., 0.], 3e-7);
    near(bounds.max, [0.04, 0.03, 0.01], 3e-7);
    let mesh = kernel
        .tessellate(
            body,
            &opencad_geometry::TessellationSettings {
                linear_deflection: 1e-5,
                angular_deflection_deg: 4.,
            },
        )
        .unwrap();
    let (v, c) = mesh_evidence::integrated(&mesh, false);
    assert!((v * 1e9 - volume).abs() < 0.5);
    near(c, center, 3e-6);
    assert!(mesh.triangle_face_ids.iter().all(|id| *id > 0));
    // Actual mesh ring contacts: entry circle at r or R, shoulder/frustum join
    // at r, and blind cap at the requested depth. Check world pose as well.
    let entry = if bottom { 0. } else { 0.01 };
    let inward = if bottom { 1. } else { -1. };
    let contacts = [
        (entry, if seat == 0 { 0.0033 } else { 0.0055 }),
        (entry + inward * depth / 1000., 0.0033),
    ];
    for (z, radius) in contacts {
        let ring: Vec<_> = mesh
            .positions
            .iter()
            .filter(|p| {
                (f64::from(p[2]) - z).abs() < 3e-7
                    && ((f64::from(p[0]) - 0.012).hypot(f64::from(p[1]) - 0.009) - radius).abs()
                        < 3e-7
            })
            .collect();
        assert!(ring.len() >= 8, "missing contact z={z},r={radius}");
    }
    if seat > 0 {
        let z = entry + inward * d / 1000.;
        assert!(
            mesh.positions
                .iter()
                .filter(|p| (f64::from(p[2]) - z).abs() < 3e-7
                    && ((f64::from(p[0]) - 0.012).hypot(f64::from(p[1]) - 0.009) - 0.0033).abs()
                        < 3e-7)
                .count()
                >= 8
        );
    }
}

fn physical<K: GeometryKernel>(kernel: &K) {
    let base = fixture();
    let initial = document_design_state(&base);
    let state = hole_creation_state_with_kernel(&base, kernel).unwrap();
    assert_eq!(state.choices.len(), 2);
    assert_eq!(state.suggested_feature_id, "feature:hole_1");
    for bottom in [false, true] {
        for seat in 0..3 {
            for depth in [8., 10.] {
                let mut doc = base.clone();
                let mut create = intent(&doc);
                if bottom {
                    create.face_ref = "ref:face:plate_bottom".into();
                    create.sketch_feature = "feature:sketch_bottom".into();
                    create.profile_ref = "sketch:bottom_axis/profile:outer".into();
                }
                create.depth = format!("{depth} mm");
                create.seat = match seat {
                    0 => HoleSeatInput::None,
                    1 => HoleSeatInput::Counterbore {
                        diameter: "11 mm".into(),
                        depth: "4 mm".into(),
                    },
                    _ => HoleSeatInput::Countersink {
                        diameter: "11 mm".into(),
                        included_angle: "90 deg".into(),
                    },
                };
                let before = doc.clone();
                let plan = plan_hole_create_with_kernel(&doc, &create, kernel).unwrap();
                assert_eq!(doc, before);
                assert_eq!(plan.patch.operations.len(), 1);
                assert_eq!(
                    plan.choice.inward_direction,
                    if bottom { [0., 0., 1.] } else { [0., 0., -1.] }
                );
                let mut history = DocumentHistory::default();
                apply_hole_create_with_kernel(&mut doc, &create, &mut history, kernel).unwrap();
                assert_eq!(doc.sketches, base.sketches);
                assert_eq!(doc.semantic_refs, base.semantic_refs);
                assert_eq!(
                    doc.feature_nodes
                        .iter()
                        .filter(|n| n.id != "feature:hole_1")
                        .collect::<Vec<_>>(),
                    base.feature_nodes.iter().collect::<Vec<_>>()
                );
                measure(&doc, kernel, bottom, seat, depth);
                let edited = document_design_state(&doc);
                assert!(hole_creation_state_with_kernel(&doc, kernel)
                    .unwrap()
                    .choices
                    .is_empty());
                history.undo(&mut doc).unwrap();
                assert_eq!(document_design_state(&doc), initial);
                history.redo(&mut doc).unwrap();
                assert_eq!(document_design_state(&doc), edited);
            }
        }
    }
}

#[test]
fn analytic_mock_and_native_hole_seat_contacts_volume_com_and_both_entries() {
    physical(&MockGeometryKernel::with_analytic_primitives());
    physical(&OcctGeometryKernel::new());
}

#[test]
fn placed_stock_axis_entries_and_generated_identity_preserve_source() {
    fn run<K: GeometryKernel>(k: &K) {
        for (origin, u, v, normal) in [
            ([0., 0., 0.], [0., 1., 0.], [0., 0., 1.], [1., 0., 0.]),
            ([0., 0., 0.], [1., 0., 0.], [0., 0., 1.], [0., -1., 0.]),
            (
                [0.01, -0.02, 0.03],
                [1., 0., 0.],
                [0., 0.8, -0.6],
                [0., 0.6, 0.8],
            ),
        ] {
            let at = |p: [f64; 3]| {
                std::array::from_fn(|i| origin[i] + u[i] * p[0] + v[i] * p[1] + normal[i] * p[2])
            };
            let mut source = fixture();
            let base = opencad_sketch::Workplane::Custom {
                origin,
                normal,
                x_axis: u,
            };
            source
                .sketches
                .iter_mut()
                .find(|s| s.id.as_str() == "sketch:base")
                .unwrap()
                .workplane = base.clone();
            let FeatureDefinition::DatumPlane(d) = &mut source
                .feature_nodes
                .iter_mut()
                .find(|n| n.id == "feature:entry_plane")
                .unwrap()
                .definition
            else {
                panic!("datum")
            };
            d.base = base;
            source
                .sketches
                .iter_mut()
                .find(|s| s.id.as_str() == "sketch:bottom_axis")
                .unwrap()
                .workplane = opencad_sketch::Workplane::Custom {
                origin,
                normal: normal.map(|x| -x),
                x_axis: u,
            };
            for r in &mut source.semantic_refs {
                let n = if r.ref_id.as_str() == "ref:face:plate_entry" {
                    normal
                } else {
                    normal.map(|x| -x)
                };
                r.semantic.normal_hint = Some(n);
                let role = if n[0] > 0.9 {
                    "+x"
                } else if n[0] < -0.9 {
                    "-x"
                } else if n[1] > 0.9 {
                    "+y"
                } else if n[1] < -0.9 {
                    "-y"
                } else if n[2] > 0. {
                    "top"
                } else {
                    "bottom"
                };
                r.semantic.role = Some(role.into());
            }
            let mut occupied = source
                .feature_nodes
                .iter()
                .find(|n| n.id == "feature:entry_plane")
                .unwrap()
                .clone();
            occupied.id = "feature:hole_1".into();
            source.feature_nodes.push(occupied);
            let mut order = source.feature_graph.ordered_ids().to_vec();
            order.push("feature:hole_1".into());
            source.feature_graph = opencad_feature::derive_feature_graph(
                &source.feature_nodes,
                &order,
                &source.sketches,
                &source.semantic_refs,
            )
            .unwrap();
            let state = hole_creation_state_with_kernel(&source, k).unwrap();
            assert_eq!(state.choices.len(), 2);
            assert_eq!(state.suggested_feature_id, "feature:hole_2");
            for bottom in [false, true] {
                let mut doc = source.clone();
                let mut create = intent(&doc);
                if bottom {
                    create.face_ref = "ref:face:plate_bottom".into();
                    create.sketch_feature = "feature:sketch_bottom".into();
                    create.profile_ref = "sketch:bottom_axis/profile:outer".into();
                }
                let plan = plan_hole_create_with_kernel(&doc, &create, k).unwrap();
                near(
                    plan.choice.entry_center_m,
                    at([0.012, 0.009, if bottom { 0. } else { 0.01 }]),
                    1e-12,
                );
                near(
                    plan.choice.inward_direction,
                    normal.map(|x| x * if bottom { 1. } else { -1. }),
                    1e-12,
                );
                apply_hole_create_with_kernel(
                    &mut doc,
                    &create,
                    &mut DocumentHistory::default(),
                    k,
                )
                .unwrap();
                assert_eq!(doc.sketches, source.sketches);
                let mut model = doc.clone().into_part_model();
                model
                    .regenerate(
                        k,
                        &FeatureRegistry::with_defaults(),
                        Some(&doc.parameters),
                        Some(&doc.semantic_refs),
                    )
                    .unwrap();
                let body = model.active_body().unwrap();
                let mass = k.mass_properties(body, 1.).unwrap();
                let bore = PI * 3.3_f64.powi(2) * 10.;
                let extra = PI * (5.5_f64.powi(2) - 3.3_f64.powi(2)) * 4.;
                let volume = 12000. - bore - extra;
                let z_removed = if bottom {
                    bore * 5. + extra * 2.
                } else {
                    bore * 5. + extra * 8.
                };
                let expected = at([
                    (12000. * 20. - (bore + extra) * 12.) / volume / 1000.,
                    (12000. * 15. - (bore + extra) * 9.) / volume / 1000.,
                    (12000. * 5. - z_removed) / volume / 1000.,
                ]);
                assert!((mass.volume_m3 * 1e9 - volume).abs() < 0.0001);
                near(mass.center_of_mass, expected, 1e-9);
                let corners: Vec<_> = [0., 0.04]
                    .into_iter()
                    .flat_map(|x| {
                        [0., 0.03]
                            .into_iter()
                            .flat_map(move |y| [0., 0.01].into_iter().map(move |z| at([x, y, z])))
                    })
                    .collect();
                let bounds = k.bounding_box(body).unwrap();
                let lo = std::array::from_fn(|i| {
                    corners.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min)
                });
                let hi = std::array::from_fn(|i| {
                    corners
                        .iter()
                        .map(|p| p[i])
                        .fold(f64::NEG_INFINITY, f64::max)
                });
                near(bounds.min, lo, 3e-7);
                near(bounds.max, hi, 3e-7);
                let mesh = k
                    .tessellate(
                        body,
                        &opencad_geometry::TessellationSettings {
                            linear_deflection: 1e-5,
                            angular_deflection_deg: 4.,
                        },
                    )
                    .unwrap();
                let (actual, center) = mesh_evidence::integrated(&mesh, false);
                assert!((actual * 1e9 - volume).abs() < 0.8);
                near(center, expected, 3e-6);
                let entry = at([0.012, 0.009, if bottom { 0. } else { 0.01 }]);
                assert!(
                    mesh.positions
                        .iter()
                        .filter(|p| {
                            let delta: [f64; 3] =
                                std::array::from_fn(|i| f64::from(p[i]) - entry[i]);
                            let axial = delta
                                .into_iter()
                                .zip(normal)
                                .map(|(a, b)| a * b)
                                .sum::<f64>();
                            let radial = delta.into_iter().map(|x| x * x).sum::<f64>().sqrt();
                            axial.abs() < 3e-7 && (radial - 0.0055).abs() < 3e-7
                        })
                        .count()
                        >= 8
                );
            }
        }
    }
    run(&MockGeometryKernel::with_analytic_primitives());
    run(&OcctGeometryKernel::new());
}

#[test]
fn stale_units_dimensions_ownership_and_assertions_refuse_atomically_in_both_kernels() {
    fn run<K: GeometryKernel>(k: &K) {
        let base = fixture();
        for bad in [
            "document",
            "revision",
            "target",
            "entry",
            "axis",
            "profile",
            "standard",
            "unitless",
            "angle_length",
            "wrong_parameter",
            "depth",
            "seat_diameter",
            "seat_depth",
            "angle",
            "blank",
        ] {
            let mut create = intent(&base);
            match bad {
                "document" => create.document_id = "doc:foreign".into(),
                "revision" => create.revision = "stale".into(),
                "target" => create.target_feature = "feature:entry_plane".into(),
                "entry" => create.face_ref = "ref:face:plate_bottom".into(),
                "axis" => create.sketch_feature = "feature:sketch_bottom".into(),
                "profile" => create.profile_ref = "profile:circle:ent:seat_circle".into(),
                "standard" => {
                    create.standard = opencad_feature::HoleStandard::Iso273 {
                        size: opencad_feature::MetricSize::M8,
                        fit: opencad_feature::ClearanceFit::Normal,
                    }
                }
                "unitless" => create.depth = "8".into(),
                "angle_length" => create.depth = "8 deg".into(),
                "wrong_parameter" => create.depth = "seat_angle".into(),
                "depth" => create.depth = "12 mm".into(),
                "seat_diameter" => {
                    create.seat = HoleSeatInput::Counterbore {
                        diameter: "30 mm".into(),
                        depth: "4 mm".into(),
                    }
                }
                "seat_depth" => {
                    create.seat = HoleSeatInput::Counterbore {
                        diameter: "11 mm".into(),
                        depth: "10 mm".into(),
                    }
                }
                "angle" => {
                    create.seat = HoleSeatInput::Countersink {
                        diameter: "11 mm".into(),
                        included_angle: "180 deg".into(),
                    }
                }
                _ => create.name = " ".into(),
            }
            let mut doc = base.clone();
            let mut history = DocumentHistory::default();
            let prior = history.clone();
            assert!(
                apply_hole_create_with_kernel(&mut doc, &create, &mut history, k).is_err(),
                "{bad}"
            );
            assert_eq!(doc, base);
            assert_eq!(history, prior);
        }
        let mut doc = base.clone();
        doc.assertions.push(opencad_core::Assertion::new(
            "assertion:impossible",
            "Too small envelope",
            opencad_core::AssertionSeverity::Required,
            opencad_core::AssertionKind::BoundingBoxWithin {
                max_m: [0.001, 0.001, 0.001],
            },
        ));
        let mut history = DocumentHistory::default();
        let source = doc.clone();
        assert!(
            apply_hole_create_with_kernel(&mut doc, &intent(&source), &mut history, k).is_err()
        );
        assert_eq!(doc, source);
        assert_eq!(history, DocumentHistory::default());
        let mut wire = serde_json::to_value(intent(&base)).unwrap();
        wire["seat"]["ignored"] = json!(true);
        assert!(serde_json::from_value::<HoleCreate>(wire).is_err());
    }
    run(&MockGeometryKernel::with_analytic_primitives());
    run(&OcctGeometryKernel::new());
}

#[test]
fn suppressed_alias_foreign_ref_and_oblique_axes_are_not_advertised() {
    let k = MockGeometryKernel::with_analytic_primitives();
    for mode in [
        "suppressed",
        "alias",
        "foreign",
        "oblique",
        "construction",
        "noncircle",
    ] {
        let mut doc = fixture();
        match mode {
            "suppressed" => {
                doc.feature_nodes
                    .iter_mut()
                    .find(|n| n.id == "feature:sketch_seat")
                    .unwrap()
                    .suppressed = true
            }
            "alias" => {
                let mut n = doc
                    .feature_nodes
                    .iter()
                    .find(|n| n.id == "feature:sketch_seat")
                    .unwrap()
                    .clone();
                n.id = "feature:alias".into();
                doc.feature_nodes.push(n);
            }
            "foreign" => doc.semantic_refs[0].semantic.created_by = "feature:sketch_base".into(),
            "oblique" => {
                doc.sketches
                    .iter_mut()
                    .find(|s| s.id.as_str() == "sketch:seat")
                    .unwrap()
                    .workplane = opencad_sketch::Workplane::Custom {
                    origin: [0., 0., 0.01],
                    normal: [1e-8, 0., 1.],
                    x_axis: [0., 1., 0.],
                };
            }
            "construction" => {
                if let opencad_sketch::SketchEntity::Circle(c) = doc
                    .sketches
                    .iter_mut()
                    .find(|s| s.id.as_str() == "sketch:seat")
                    .unwrap()
                    .entities
                    .iter_mut()
                    .find(|e| e.id().as_str() == "ent:seat_circle")
                    .unwrap()
                {
                    c.base.construction = true;
                }
            }
            _ => doc
                .sketches
                .iter_mut()
                .find(|s| s.id.as_str() == "sketch:seat")
                .unwrap()
                .entities
                .retain(|e| e.id().as_str() != "ent:seat_circle"),
        }
        let order = doc
            .feature_nodes
            .iter()
            .map(|n| n.id.clone())
            .collect::<Vec<_>>();
        if let Ok(graph) = opencad_feature::derive_feature_graph(
            &doc.feature_nodes,
            &order,
            &doc.sketches,
            &doc.semantic_refs,
        ) {
            doc.feature_graph = graph;
        }
        if let Ok(state) = hole_creation_state_with_kernel(&doc, &k) {
            assert!(
                !state
                    .choices
                    .iter()
                    .any(|c| c.sketch_feature == "feature:sketch_seat"),
                "{mode}"
            );
        }
    }
    let legacy = MockGeometryKernel::new();
    assert!(hole_creation_state_with_kernel(&fixture(), &legacy)
        .unwrap()
        .choices
        .is_empty());
    assert!(plan_hole_create_with_kernel(&fixture(), &intent(&fixture()), &legacy).is_err());
}

#[test]
fn packed_expanded_review_failure_history_and_live_parameter_sources() {
    let root = tempfile::tempdir().unwrap();
    for packed in [false, true] {
        let path = root
            .path()
            .join(if packed { "part.ocad" } else { "part.ocad.d" });
        let path = path.to_str().unwrap();
        let base = fixture();
        opencad_file::write_ocad(path, &base).unwrap();
        let read = opencad_file::read_ocad(path).unwrap();
        let before = document_design_state(&read);
        let create = intent(&read);
        let bytes = std::fs::read(if packed {
            std::path::PathBuf::from(path)
        } else {
            std::path::PathBuf::from(path).join("checksums.json")
        })
        .unwrap();
        plan_document_hole_create(path, &create).unwrap();
        assert_eq!(opencad_file::read_ocad(path).unwrap(), read);
        assert_eq!(
            std::fs::read(if packed {
                std::path::PathBuf::from(path)
            } else {
                std::path::PathBuf::from(path).join("checksums.json")
            })
            .unwrap(),
            bytes
        );
        let result = apply_document_hole_create(path, &create, None).unwrap();
        let applied = opencad_file::read_ocad(path).unwrap();
        let edited = document_design_state(&applied);
        assert!(
            apply_document_hole_create(path, &create, Some(result.history.history.clone()))
                .is_err()
        );
        assert_eq!(opencad_file::read_ocad(path).unwrap(), applied);
        let h = opencad_desktop::undo_document_with_history(path, result.history.history).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            before
        );
        opencad_desktop::redo_document_with_history(path, h.history).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            edited
        );
        let source = applied.sketches.clone();
        let h = opencad_desktop::set_document_parameter_with_history(
            path,
            "param:seat_depth",
            "6 mm",
            None,
        )
        .unwrap();
        let changed = opencad_file::read_ocad(path).unwrap();
        assert_eq!(changed.sketches, source);
        assert!(
            (verify_editor_document_native(&changed)
                .unwrap()
                .volume_m3
                .unwrap()
                * 1e9
                - (12000.
                    - PI * (3.3_f64.powi(2) * 10. + (5.5_f64.powi(2) - 3.3_f64.powi(2)) * 6.)))
                .abs()
                < 0.0001
        );
        assert!(opencad_desktop::set_document_parameter_with_history(
            path,
            "param:seat_depth",
            "12 mm",
            Some(h.history)
        )
        .is_err());
        assert_eq!(opencad_file::read_ocad(path).unwrap(), changed);
        let FeatureDefinition::SeatedStandardHole(def) = &applied
            .feature_nodes
            .iter()
            .find(|n| n.id == "feature:hole_1")
            .unwrap()
            .definition
        else {
            panic!("hole")
        };
        assert_eq!(def.bore_depth_expr.as_deref(), Some("bore_depth"));
    }
}
