use std::cell::RefCell;

use opencad_core::{OpenCadError, Result, TopoRefId};
use opencad_geometry::{
    BooleanOp, BoundingBox, ExtrudeExtent, ExtrudeOperation, FilletEdgeSelector, GeometryKernel,
    KernelBody, KernelWire, MassProperties, MeshSet, SolvedSketch, TessellationSettings,
};

use crate::convert::{map_occt_error, sketch_to_edges};
use crate::store::KernelStore;

#[cfg(feature = "occt")]
use cadrum::{DVec3, Edge, Solid};

#[cfg(feature = "occt")]
fn collect_edges(solid: &Solid, selector: FilletEdgeSelector) -> Vec<&Edge> {
    match selector {
        FilletEdgeSelector::All => solid.iter_edge().collect(),
        FilletEdgeSelector::TopPerimeter => {
            let bb = solid.bounding_box();
            let z_max = bb[1].z;
            let eps = 1e-6;
            solid
                .iter_edge()
                .filter(|e| {
                    [e.start_point(), e.end_point()]
                        .iter()
                        .all(|p| (p.z - z_max).abs() < eps)
                })
                .collect()
        }
        FilletEdgeSelector::FacePerimeter { kernel_face_id } => solid
            .iter_face()
            .find(|face| face.id() == kernel_face_id)
            .map(|face| face.iter_edge().collect())
            .unwrap_or_default(),
    }
}

/// OpenCASCADE backend via statically linked OCCT (cadrum).
pub struct OcctGeometryKernel {
    store: RefCell<KernelStore>,
}

impl Default for OcctGeometryKernel {
    fn default() -> Self {
        Self::new()
    }
}

impl OcctGeometryKernel {
    pub fn new() -> Self {
        Self {
            store: RefCell::new(KernelStore::new()),
        }
    }

    pub fn occt_version() -> &'static str {
        "OCCT 8.0.0 (cadrum static)"
    }
}

impl GeometryKernel for OcctGeometryKernel {
    fn make_wire_from_sketch(&self, sketch: &SolvedSketch) -> Result<KernelWire> {
        #[cfg(feature = "occt")]
        {
            let edges = sketch_to_edges(sketch)?;
            let id = self.store.borrow_mut().insert_wire(edges);
            Ok(KernelWire::new(id))
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = sketch;
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn extrude(
        &self,
        profile: KernelWire,
        extent: ExtrudeExtent,
        operation: ExtrudeOperation,
        target: Option<KernelBody>,
    ) -> Result<KernelBody> {
        #[cfg(feature = "occt")]
        {
            let edges = self
                .store
                .borrow()
                .wire(profile.0)
                .ok_or_else(|| OpenCadError::not_found(format!("wire {}", profile.0)))?
                .to_vec();

            let length_m = match extent {
                ExtrudeExtent::Distance { length } => length.meters(),
                ExtrudeExtent::Symmetric { length } => length.meters() * 2.0,
                ExtrudeExtent::ThroughAll => 1.0,
            };
            if length_m <= 0.0 {
                return Err(OpenCadError::validation("extrude length must be positive"));
            }

            let mut solid =
                Solid::extrude(edges.iter(), DVec3::Z * length_m).map_err(map_occt_error)?;

            if let ExtrudeOperation::Cut = operation {
                let Some(target_body) = target else {
                    return Err(OpenCadError::validation(
                        "cut operation requires target body",
                    ));
                };
                let target_solid = self
                    .store
                    .borrow()
                    .body(target_body.0)
                    .ok_or_else(|| OpenCadError::not_found(format!("body {}", target_body.0)))?
                    .clone();
                solid = (target_solid - solid).build().map_err(map_occt_error)?;
            }

            let id = self.store.borrow_mut().insert_body(solid);
            Ok(KernelBody::new(id))
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = (profile, extent, operation, target);
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn boolean(&self, lhs: KernelBody, rhs: KernelBody, op: BooleanOp) -> Result<KernelBody> {
        #[cfg(feature = "occt")]
        {
            let store = self.store.borrow();
            let left = store
                .body(lhs.0)
                .ok_or_else(|| OpenCadError::not_found(format!("body {}", lhs.0)))?
                .clone();
            let right = store
                .body(rhs.0)
                .ok_or_else(|| OpenCadError::not_found(format!("body {}", rhs.0)))?
                .clone();
            drop(store);

            let expr = match op {
                BooleanOp::Union => left + right,
                BooleanOp::Subtract => left - right,
                BooleanOp::Intersect => left * right,
            };
            let solid = expr.build().map_err(map_occt_error)?;
            let id = self.store.borrow_mut().insert_body(solid);
            Ok(KernelBody::new(id))
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = (lhs, rhs, op);
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn face_derivation_history(&self, body: &KernelBody) -> Vec<(u64, u64)> {
        #[cfg(feature = "occt")]
        {
            let store = self.store.borrow();
            let Some(solid) = store.body(body.0) else {
                return Vec::new();
            };
            solid
                .iter_history()
                .map(|pair| (pair[0], pair[1]))
                .collect()
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = body;
            Vec::new()
        }
    }

    fn tessellate(&self, body: &KernelBody, settings: &TessellationSettings) -> Result<MeshSet> {
        #[cfg(feature = "occt")]
        {
            let solid = self
                .store
                .borrow()
                .body(body.0)
                .ok_or_else(|| OpenCadError::not_found(format!("body {}", body.0)))?
                .clone();

            let tessellation = cadrum::Tessellation {
                deflection_linear: settings.linear_deflection,
                deflection_angular: settings.angular_deflection_deg.to_radians(),
                relative_linear: false,
            };
            let mesh =
                Solid::mesh(std::slice::from_ref(&solid), tessellation).map_err(map_occt_error)?;

            let positions: Vec<[f32; 3]> = mesh
                .vertices
                .iter()
                .map(|p| [p.x as f32, p.y as f32, p.z as f32])
                .collect();
            let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
            let indices: Vec<u32> = mesh.indices.iter().map(|&i| i as u32).collect();
            let triangle_face_ids = mesh.face_ids.clone();

            Ok(MeshSet {
                positions,
                normals,
                indices,
                triangle_face_ids,
            })
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = (body, settings);
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn mass_properties(&self, body: &KernelBody, density_kg_per_m3: f64) -> Result<MassProperties> {
        #[cfg(feature = "occt")]
        {
            let solid = self
                .store
                .borrow()
                .body(body.0)
                .ok_or_else(|| OpenCadError::not_found(format!("body {}", body.0)))?
                .clone();

            let volume = solid.volume();
            let area = solid.area();
            let center = solid.center();
            Ok(MassProperties {
                volume_m3: volume,
                area_m2: area,
                mass_kg: volume * density_kg_per_m3,
                center_of_mass: [center.x, center.y, center.z],
            })
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = (body, density_kg_per_m3);
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn bounding_box(&self, body: &KernelBody) -> Result<BoundingBox> {
        #[cfg(feature = "occt")]
        {
            let solid = self
                .store
                .borrow()
                .body(body.0)
                .ok_or_else(|| OpenCadError::not_found(format!("body {}", body.0)))?
                .clone();
            let bb = solid.bounding_box();
            Ok(BoundingBox {
                min: [bb[0].x, bb[0].y, bb[0].z],
                max: [bb[1].x, bb[1].y, bb[1].z],
            })
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = body;
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn assign_face_ref(
        &self,
        body: &KernelBody,
        kernel_face_id: u64,
        ref_id: TopoRefId,
    ) -> Result<()> {
        #[cfg(feature = "occt")]
        {
            if self.store.borrow().body(body.0).is_none() {
                return Err(OpenCadError::not_found(format!("body {}", body.0)));
            }
            self.store
                .borrow_mut()
                .tag_face_ref(body.0, kernel_face_id, ref_id.as_str().to_string());
            Ok(())
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = (body, kernel_face_id, ref_id);
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn fillet_edges(
        &self,
        body: KernelBody,
        radius_m: f64,
        selector: FilletEdgeSelector,
    ) -> Result<KernelBody> {
        #[cfg(feature = "occt")]
        {
            if radius_m <= 0.0 {
                return Err(OpenCadError::validation("fillet radius must be positive"));
            }

            let solid = self
                .store
                .borrow()
                .body(body.0)
                .ok_or_else(|| OpenCadError::not_found(format!("body {}", body.0)))?
                .clone();

            let edges = collect_edges(&solid, selector);
            if edges.is_empty() {
                return Err(OpenCadError::validation(
                    "fillet selector matched no edges on the body",
                ));
            }

            let filleted = solid
                .fillet_edges(radius_m, edges)
                .map_err(map_occt_error)?;
            let id = self.store.borrow_mut().insert_body(filleted);
            Ok(KernelBody::new(id))
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = (body, radius_m, selector);
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn chamfer_edges(
        &self,
        body: KernelBody,
        distance_m: f64,
        selector: FilletEdgeSelector,
    ) -> Result<KernelBody> {
        #[cfg(feature = "occt")]
        {
            if distance_m <= 0.0 {
                return Err(OpenCadError::validation("chamfer distance must be positive"));
            }

            let solid = self
                .store
                .borrow()
                .body(body.0)
                .ok_or_else(|| OpenCadError::not_found(format!("body {}", body.0)))?
                .clone();

            let edges = collect_edges(&solid, selector);
            if edges.is_empty() {
                return Err(OpenCadError::validation(
                    "chamfer selector matched no edges on the body",
                ));
            }

            let chamfered = solid
                .chamfer_edges(distance_m, edges)
                .map_err(map_occt_error)?;
            let id = self.store.borrow_mut().insert_body(chamfered);
            Ok(KernelBody::new(id))
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = (body, distance_m, selector);
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn translate_body(&self, body: KernelBody, translation_m: [f64; 3]) -> Result<KernelBody> {
        #[cfg(feature = "occt")]
        {
            let solid = self
                .store
                .borrow()
                .body(body.0)
                .ok_or_else(|| OpenCadError::not_found(format!("body {}", body.0)))?
                .clone();
            let translated = solid.translate(DVec3::new(
                translation_m[0],
                translation_m[1],
                translation_m[2],
            ));
            let id = self.store.borrow_mut().insert_body(translated);
            Ok(KernelBody::new(id))
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = (body, translation_m);
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn rotate_body(
        &self,
        body: KernelBody,
        axis_origin_m: [f64; 3],
        axis_direction_m: [f64; 3],
        angle_rad: f64,
    ) -> Result<KernelBody> {
        #[cfg(feature = "occt")]
        {
            let solid = self
                .store
                .borrow()
                .body(body.0)
                .ok_or_else(|| OpenCadError::not_found(format!("body {}", body.0)))?
                .clone();
            let axis_direction = DVec3::new(
                axis_direction_m[0],
                axis_direction_m[1],
                axis_direction_m[2],
            );
            let axis_origin = DVec3::new(axis_origin_m[0], axis_origin_m[1], axis_origin_m[2]);
            let rotated = solid.rotate(axis_origin, axis_direction, angle_rad);
            let id = self.store.borrow_mut().insert_body(rotated);
            Ok(KernelBody::new(id))
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = (body, axis_origin_m, axis_direction_m, angle_rad);
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn mirror_body(
        &self,
        body: KernelBody,
        plane_origin_m: [f64; 3],
        plane_normal_m: [f64; 3],
    ) -> Result<KernelBody> {
        #[cfg(feature = "occt")]
        {
            let solid = self
                .store
                .borrow()
                .body(body.0)
                .ok_or_else(|| OpenCadError::not_found(format!("body {}", body.0)))?
                .clone();
            let plane_origin = DVec3::new(
                plane_origin_m[0],
                plane_origin_m[1],
                plane_origin_m[2],
            );
            let plane_normal = DVec3::new(
                plane_normal_m[0],
                plane_normal_m[1],
                plane_normal_m[2],
            );
            let mirrored = solid.mirror(plane_origin, plane_normal);
            let id = self.store.borrow_mut().insert_body(mirrored);
            Ok(KernelBody::new(id))
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = (body, plane_origin_m, plane_normal_m);
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }
}

#[cfg(all(test, feature = "occt"))]
mod tests {
    use super::*;
    use opencad_core::Length;

    fn rectangle_sketch() -> SolvedSketch {
        SolvedSketch {
            profile_ref: "sketch:base/profile:outer".into(),
            points: vec![[0.0, 0.0], [0.08, 0.0], [0.08, 0.06], [0.0, 0.06]],
            closed: true,
        }
    }

    #[test]
    fn occt_extrude_plate_volume() {
        let kernel = OcctGeometryKernel::new();
        let wire = kernel
            .make_wire_from_sketch(&rectangle_sketch())
            .expect("wire");
        let body = kernel
            .extrude(
                wire,
                ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                ExtrudeOperation::NewBody,
                None,
            )
            .expect("extrude");

        let mass = kernel.mass_properties(&body, 2700.0).expect("mass");
        let expected_volume = 0.08 * 0.06 * 0.006;
        assert!((mass.volume_m3 - expected_volume).abs() < 1e-9);
        assert!(mass.mass_kg > 0.0);
    }

    #[test]
    fn occt_tessellate_produces_mesh() {
        let kernel = OcctGeometryKernel::new();
        let wire = kernel
            .make_wire_from_sketch(&rectangle_sketch())
            .expect("wire");
        let body = kernel
            .extrude(
                wire,
                ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                ExtrudeOperation::NewBody,
                None,
            )
            .expect("extrude");
        let mesh = kernel
            .tessellate(&body, &TessellationSettings::default())
            .expect("mesh");
        assert!(mesh.triangle_count() > 0);
        assert!(mesh.has_triangle_face_ids());
        assert!(mesh.triangle_face_ids.iter().any(|&id| id > 0));
        let distinct_faces: std::collections::BTreeSet<_> =
            mesh.triangle_face_ids.iter().copied().collect();
        assert!(distinct_faces.len() > 1, "expected multiple B-Rep faces");
    }

    #[test]
    fn occt_fillet_produces_face_derivation_history() {
        let kernel = OcctGeometryKernel::new();
        let wire = kernel
            .make_wire_from_sketch(&rectangle_sketch())
            .expect("wire");
        let body = kernel
            .extrude(
                wire,
                ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                ExtrudeOperation::NewBody,
                None,
            )
            .expect("extrude");

        let filleted = kernel
            .fillet_edges(body, 0.001, FilletEdgeSelector::TopPerimeter)
            .expect("fillet");
        let history = kernel.face_derivation_history(&filleted);
        assert!(!history.is_empty());
        assert!(history.iter().any(|(post, src)| post != src));
    }

    #[test]
    fn occt_fillet_top_edges_reduces_volume() {
        let kernel = OcctGeometryKernel::new();
        let wire = kernel
            .make_wire_from_sketch(&rectangle_sketch())
            .expect("wire");
        let body = kernel
            .extrude(
                wire,
                ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                ExtrudeOperation::NewBody,
                None,
            )
            .expect("extrude");

        let before = kernel.mass_properties(&body, 2700.0).expect("before");
        let filleted = kernel
            .fillet_edges(body, 0.001, FilletEdgeSelector::TopPerimeter)
            .expect("fillet");
        let after = kernel.mass_properties(&filleted, 2700.0).expect("after");

        assert!(
            after.volume_m3 < before.volume_m3,
            "fillet should reduce volume: {} vs {}",
            after.volume_m3,
            before.volume_m3
        );
    }

    #[test]
    fn occt_chamfer_top_edges_reduces_volume() {
        let kernel = OcctGeometryKernel::new();
        let wire = kernel
            .make_wire_from_sketch(&rectangle_sketch())
            .expect("wire");
        let body = kernel
            .extrude(
                wire,
                ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                ExtrudeOperation::NewBody,
                None,
            )
            .expect("extrude");

        let before = kernel.mass_properties(&body, 2700.0).expect("before");
        let chamfered = kernel
            .chamfer_edges(body, 0.0005, FilletEdgeSelector::TopPerimeter)
            .expect("chamfer");
        let after = kernel.mass_properties(&chamfered, 2700.0).expect("after");

        assert!(
            after.volume_m3 < before.volume_m3,
            "chamfer should reduce volume: {} vs {}",
            after.volume_m3,
            before.volume_m3
        );
    }

    #[test]
    fn occt_translate_body_offsets_solid() {
        let kernel = OcctGeometryKernel::new();
        let wire = kernel
            .make_wire_from_sketch(&rectangle_sketch())
            .expect("wire");
        let body = kernel
            .extrude(
                wire,
                ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                ExtrudeOperation::NewBody,
                None,
            )
            .expect("extrude");
        let translated = kernel
            .translate_body(body, [0.1, 0.0, 0.0])
            .expect("translate");
        let mass = kernel.mass_properties(&translated, 2700.0).expect("mass");
        assert!(mass.volume_m3 > 0.0);
    }

    #[test]
    fn occt_rotate_body_preserves_volume() {
        let kernel = OcctGeometryKernel::new();
        let wire = kernel
            .make_wire_from_sketch(&rectangle_sketch())
            .expect("wire");
        let body = kernel
            .extrude(
                wire,
                ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                ExtrudeOperation::NewBody,
                None,
            )
            .expect("extrude");
        let before = kernel.mass_properties(&body, 2700.0).expect("before");
        let rotated = kernel
            .rotate_body(body, [0.04, 0.03, 0.0], [0.0, 0.0, 1.0], std::f64::consts::FRAC_PI_2)
            .expect("rotate");
        let after = kernel.mass_properties(&rotated, 2700.0).expect("after");
        assert!(
            (after.volume_m3 - before.volume_m3).abs() < 1e-9,
            "rotation should preserve volume: {} vs {}",
            after.volume_m3,
            before.volume_m3
        );
    }

    #[test]
    fn occt_mirror_body_preserves_volume() {
        let kernel = OcctGeometryKernel::new();
        let wire = kernel
            .make_wire_from_sketch(&rectangle_sketch())
            .expect("wire");
        let body = kernel
            .extrude(
                wire,
                ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                ExtrudeOperation::NewBody,
                None,
            )
            .expect("extrude");
        let before = kernel.mass_properties(&body, 2700.0).expect("before");
        let mirrored = kernel
            .mirror_body(body, [0.04, 0.0, 0.0], [1.0, 0.0, 0.0])
            .expect("mirror");
        let after = kernel.mass_properties(&mirrored, 2700.0).expect("after");
        assert!(
            (after.volume_m3 - before.volume_m3).abs() < 1e-9,
            "mirror should preserve volume: {} vs {}",
            after.volume_m3,
            before.volume_m3
        );
    }
}
