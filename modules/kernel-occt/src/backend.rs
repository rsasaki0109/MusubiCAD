use std::cell::RefCell;

use opencad_core::{OpenCadError, Result, TopoRefId};
use opencad_geometry::{
    BooleanOp, BoundingBox, ExtrudeExtent, ExtrudeOperation, FilletEdgeSelector, GeometryKernel,
    KernelBody, KernelWire, MassProperties, MeshSet, RevolveInput, RevolveOperation,
    SolvedSketch, TessellationSettings,
};
use opencad_geometry::topo_sync::EdgeRefDiscovery;

use crate::convert::{map_occt_error, sketch_to_edges, sketch_to_edges_on_plane};
use crate::store::KernelStore;

#[cfg(feature = "occt")]
use cadrum::{DVec3, Edge, ProfileOrient, Solid};

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
        FilletEdgeSelector::KernelEdges { kernel_edge_ids } => solid
            .iter_edge()
            .filter(|edge| kernel_edge_ids.contains(&edge.id()))
            .collect(),
        FilletEdgeSelector::EdgeRole { role } => {
            let bb = solid.bounding_box();
            let bbox = [
                [bb[0].x, bb[0].y, bb[0].z],
                [bb[1].x, bb[1].y, bb[1].z],
            ];
            let mut matches: Vec<(&Edge, f64)> = solid
                .iter_edge()
                .filter_map(|edge| {
                    let start = edge.start_point();
                    let end = edge.end_point();
                    let midpoint = [
                        (start.x + end.x) * 0.5,
                        (start.y + end.y) * 0.5,
                        (start.z + end.z) * 0.5,
                    ];
                    let tangent = normalize_vec3([
                        end.x - start.x,
                        end.y - start.y,
                        end.z - start.z,
                    ]);
                    let edge_role = top_edge_role(&midpoint, &tangent, &bbox);
                    if edge_role != role {
                        return None;
                    }
                    let length = ((end.x - start.x).powi(2)
                        + (end.y - start.y).powi(2)
                        + (end.z - start.z).powi(2))
                    .sqrt();
                    Some((edge, length))
                })
                .collect();
            matches.sort_by(|left, right| {
                right
                    .1
                    .partial_cmp(&left.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            matches.into_iter().take(1).map(|(edge, _)| edge).collect()
        }
    }
}

#[cfg(feature = "occt")]
fn discover_edges_from_solid(solid: &Solid) -> Vec<EdgeRefDiscovery> {
    let bb = solid.bounding_box();
    let z_max = bb[1].z;
    let eps = 1e-6;
    let bbox = [
        [bb[0].x, bb[0].y, bb[0].z],
        [bb[1].x, bb[1].y, bb[1].z],
    ];
    solid
        .iter_edge()
        .filter_map(|edge| {
            let start = edge.start_point();
            let end = edge.end_point();
            let midpoint = [
                (start.x + end.x) * 0.5,
                (start.y + end.y) * 0.5,
                (start.z + end.z) * 0.5,
            ];
            if (midpoint[2] - z_max).abs() > eps {
                return None;
            }
            let tangent = normalize_vec3([
                end.x - start.x,
                end.y - start.y,
                end.z - start.z,
            ]);
            let length_m = [
                end.x - start.x,
                end.y - start.y,
                end.z - start.z,
            ];
            let length = (length_m[0] * length_m[0]
                + length_m[1] * length_m[1]
                + length_m[2] * length_m[2])
                .sqrt();
            let role = top_edge_role(&midpoint, &tangent, &bbox);
            Some(EdgeRefDiscovery {
                kernel_edge_id: edge.id(),
                role,
                midpoint_m: [
                    midpoint[0] as f32,
                    midpoint[1] as f32,
                    midpoint[2] as f32,
                ],
                tangent_m: [
                    tangent[0] as f32,
                    tangent[1] as f32,
                    tangent[2] as f32,
                ],
                length_m: length as f32,
                feature_id: None,
            })
        })
        .collect()
}

#[cfg(feature = "occt")]
fn normalize_vec3(vector: [f64; 3]) -> [f64; 3] {
    let len =
        (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt();
    if len <= 1e-12 {
        return [1.0, 0.0, 0.0];
    }
    [vector[0] / len, vector[1] / len, vector[2] / len]
}

#[cfg(feature = "occt")]
fn top_edge_role(midpoint: &[f64; 3], tangent: &[f64; 3], bb: &[[f64; 3]; 2]) -> String {
    if tangent[0].abs() >= tangent[1].abs() {
        if midpoint[1] > (bb[0][1] + bb[1][1]) * 0.5 {
            "top@+y".into()
        } else {
            "top@-y".into()
        }
    } else if midpoint[0] > (bb[0][0] + bb[1][0]) * 0.5 {
        "top@+x".into()
    } else {
        "top@-x".into()
    }
}

#[cfg(feature = "occt")]
fn revolve_spine_edge(axis: DVec3, angle_rad: f64) -> opencad_core::Result<Edge> {
    if angle_rad >= std::f64::consts::TAU - 1e-6 {
        return Edge::circle(1.0, axis).map_err(map_occt_error);
    }

    let helper = if axis.z.abs() < 0.9 {
        DVec3::Z
    } else {
        DVec3::X
    };
    let u = axis.cross(helper).normalize();
    let v = axis.cross(u).normalize();
    let start = u;
    let end = u * angle_rad.cos() + v * angle_rad.sin();
    let mid = u * (angle_rad * 0.5).cos() + v * (angle_rad * 0.5).sin();
    Edge::arc_3pts(start, mid, end).map_err(map_occt_error)
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
        direction_m: [f64; 3],
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

            let dir_len = (direction_m[0].powi(2)
                + direction_m[1].powi(2)
                + direction_m[2].powi(2))
            .sqrt();
            if dir_len <= 1e-12 {
                return Err(OpenCadError::validation(
                    "extrude direction must be a non-zero vector",
                ));
            }
            let direction = DVec3::new(
                direction_m[0] / dir_len * length_m,
                direction_m[1] / dir_len * length_m,
                direction_m[2] / dir_len * length_m,
            );

            let mut solid = Solid::extrude(edges.iter(), direction).map_err(map_occt_error)?;

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
            let _ = (profile, extent, operation, target, direction_m);
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn revolve(&self, input: &RevolveInput) -> Result<KernelBody> {
        #[cfg(feature = "occt")]
        {
            let sketch = &input.sketch;
            let profile_plane = input.profile_plane;
            let axis_direction_m = input.axis_direction_m;
            let angle_rad = input.angle_rad;
            let operation = input.operation;
            let target = input.target.clone();
            if angle_rad <= 0.0 {
                return Err(OpenCadError::validation("revolve angle must be positive"));
            }
            if angle_rad > std::f64::consts::TAU + 1e-6 {
                return Err(OpenCadError::validation(
                    "revolve angle must not exceed 360° (2π rad)",
                ));
            }

            let axis_len = (axis_direction_m[0].powi(2)
                + axis_direction_m[1].powi(2)
                + axis_direction_m[2].powi(2))
            .sqrt();
            if axis_len <= 1e-12 {
                return Err(OpenCadError::validation(
                    "revolve axis direction must be a non-zero vector",
                ));
            }
            let axis = DVec3::new(
                axis_direction_m[0] / axis_len,
                axis_direction_m[1] / axis_len,
                axis_direction_m[2] / axis_len,
            );

            let edges = sketch_to_edges_on_plane(sketch, profile_plane)?;
            let spine = revolve_spine_edge(axis, angle_rad)?;
            let mut solid = Solid::sweep(&edges, std::slice::from_ref(&spine), ProfileOrient::Up(axis))
                .map_err(map_occt_error)?;

            match operation {
                RevolveOperation::NewBody => {}
                RevolveOperation::Cut => {
                    let Some(target_body) = target else {
                        return Err(OpenCadError::validation(
                            "cut revolve requires target body",
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
                RevolveOperation::Join => {
                    let new_body = solid;
                    let Some(target_body) = target else {
                        return Err(OpenCadError::validation(
                            "join revolve requires target body",
                        ));
                    };
                    let target_solid = self
                        .store
                        .borrow()
                        .body(target_body.0)
                        .ok_or_else(|| OpenCadError::not_found(format!("body {}", target_body.0)))?
                        .clone();
                    solid = (target_solid + new_body).build().map_err(map_occt_error)?;
                }
            }

            let id = self.store.borrow_mut().insert_body(solid);
            Ok(KernelBody::new(id))
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = input;
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
            self.store.borrow_mut().tag_face_ref(
                body.0,
                kernel_face_id,
                ref_id.as_str().to_string(),
            );
            Ok(())
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = (body, kernel_face_id, ref_id);
            Err(OpenCadError::Other("OCCT backend disabled".into()))
        }
    }

    fn discover_body_edges(&self, body: &KernelBody) -> Result<Vec<EdgeRefDiscovery>> {
        #[cfg(feature = "occt")]
        {
            let solid = self
                .store
                .borrow()
                .body(body.0)
                .ok_or_else(|| OpenCadError::not_found(format!("body {}", body.0)))?
                .clone();
            Ok(discover_edges_from_solid(&solid))
        }
        #[cfg(not(feature = "occt"))]
        {
            let _ = body;
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
                return Err(OpenCadError::validation(
                    "chamfer distance must be positive",
                ));
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
            let plane_origin = DVec3::new(plane_origin_m[0], plane_origin_m[1], plane_origin_m[2]);
            let plane_normal = DVec3::new(plane_normal_m[0], plane_normal_m[1], plane_normal_m[2]);
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
            placement: None,
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
                [0.0, 0.0, 1.0],
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
                [0.0, 0.0, 1.0],
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
                [0.0, 0.0, 1.0],
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
                [0.0, 0.0, 1.0],
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
                [0.0, 0.0, 1.0],
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
                [0.0, 0.0, 1.0],
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
                [0.0, 0.0, 1.0],
            )
            .expect("extrude");
        let before = kernel.mass_properties(&body, 2700.0).expect("before");
        let rotated = kernel
            .rotate_body(
                body,
                [0.04, 0.03, 0.0],
                [0.0, 0.0, 1.0],
                std::f64::consts::FRAC_PI_2,
            )
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
                [0.0, 0.0, 1.0],
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
