use serde::{Deserialize, Serialize};

use opencad_core::{Length, OpenCadError, Result, TopoRefId};

use crate::mass::{BoundingBox, MassProperties};
use crate::tessellation::{MeshSet, TessellationSettings};

/// Opaque backend-neutral solid body handle.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KernelBody(pub u64);

/// Opaque backend-neutral wire/profile handle.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KernelWire(pub u64);

impl KernelBody {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

impl KernelWire {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilePlane {
    Xy,
    Yz,
    Xz,
}

impl ProfilePlane {
    pub fn map_point(self, u: f64, v: f64) -> [f64; 3] {
        match self {
            Self::Xy => [u, v, 0.0],
            Self::Yz => [0.0, u, v],
            Self::Xz => [u, 0.0, v],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolveOperation {
    NewBody,
    Cut,
    Join,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BooleanOp {
    Union,
    Subtract,
    Intersect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtrudeOperation {
    NewBody,
    Cut,
    Join,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExtrudeExtent {
    Distance { length: Length },
    ThroughAll,
    Symmetric { length: Length },
}

/// Which edges to fillet on a solid body.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilletEdgeSelector {
    /// Every edge on the body.
    All,
    /// Edges on the top face (maximum Z in the bounding box).
    #[default]
    TopPerimeter,
    /// Edges on the face with the given kernel B-Rep face id.
    FacePerimeter { kernel_face_id: u64 },
    /// Specific kernel B-Rep edges by id.
    KernelEdges { kernel_edge_ids: Vec<u64> },
    /// Edges matched by semantic role on the current body (longest match wins).
    EdgeRole { role: String },
}

/// Local UV placement for a sketch profile in world coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SketchPlacement {
    pub origin_m: [f64; 3],
    pub x_axis_m: [f64; 3],
    pub y_axis_m: [f64; 3],
}

impl SketchPlacement {
    pub fn global_xy() -> Self {
        Self {
            origin_m: [0.0, 0.0, 0.0],
            x_axis_m: [1.0, 0.0, 0.0],
            y_axis_m: [0.0, 1.0, 0.0],
        }
    }

    pub fn map_point(self, u: f64, v: f64) -> [f64; 3] {
        [
            self.origin_m[0] + self.x_axis_m[0] * u + self.y_axis_m[0] * v,
            self.origin_m[1] + self.x_axis_m[1] * u + self.y_axis_m[1] * v,
            self.origin_m[2] + self.x_axis_m[2] * u + self.y_axis_m[2] * v,
        ]
    }

    pub fn extrude_direction_m(self) -> [f64; 3] {
        let cross = [
            self.x_axis_m[1] * self.y_axis_m[2] - self.x_axis_m[2] * self.y_axis_m[1],
            self.x_axis_m[2] * self.y_axis_m[0] - self.x_axis_m[0] * self.y_axis_m[2],
            self.x_axis_m[0] * self.y_axis_m[1] - self.x_axis_m[1] * self.y_axis_m[0],
        ];
        let len = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
        if len <= 1e-12 {
            return [0.0, 0.0, 1.0];
        }
        [cross[0] / len, cross[1] / len, cross[2] / len]
    }
}

/// 2D profile input for wire creation (sketch solver output).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolvedSketch {
    pub profile_ref: String,
    pub points: Vec<[f64; 2]>,
    pub closed: bool,
    #[serde(skip)]
    pub placement: Option<SketchPlacement>,
}

/// Input for a solid-of-revolution operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevolveInput {
    pub sketch: SolvedSketch,
    pub profile_plane: ProfilePlane,
    pub axis_origin_m: [f64; 3],
    pub axis_direction_m: [f64; 3],
    pub angle_rad: f64,
    pub operation: RevolveOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<KernelBody>,
}

/// Kernel-neutral geometry operations.
pub trait GeometryKernel {
    fn make_wire_from_sketch(&self, sketch: &SolvedSketch) -> Result<KernelWire>;

    fn extrude(
        &self,
        profile: KernelWire,
        extent: ExtrudeExtent,
        operation: ExtrudeOperation,
        target: Option<KernelBody>,
        direction_m: [f64; 3],
    ) -> Result<KernelBody>;

    fn revolve(&self, input: &RevolveInput) -> Result<KernelBody>;

    fn boolean(&self, lhs: KernelBody, rhs: KernelBody, op: BooleanOp) -> Result<KernelBody>;

    fn tessellate(&self, body: &KernelBody, settings: &TessellationSettings) -> Result<MeshSet>;

    /// Face derivation pairs `(post_id, src_id)` from the kernel's last modifying op.
    fn face_derivation_history(&self, body: &KernelBody) -> Vec<(u64, u64)>;

    fn mass_properties(&self, body: &KernelBody, density_kg_per_m3: f64) -> Result<MassProperties>;

    fn bounding_box(&self, body: &KernelBody) -> Result<BoundingBox>;

    fn assign_face_ref(
        &self,
        body: &KernelBody,
        kernel_face_id: u64,
        ref_id: TopoRefId,
    ) -> Result<()>;

    fn discover_body_edges(
        &self,
        body: &KernelBody,
    ) -> Result<Vec<crate::topo_sync::EdgeRefDiscovery>>;

    fn fillet_edges(
        &self,
        body: KernelBody,
        radius_m: f64,
        selector: FilletEdgeSelector,
    ) -> Result<KernelBody>;

    fn chamfer_edges(
        &self,
        body: KernelBody,
        distance_m: f64,
        selector: FilletEdgeSelector,
    ) -> Result<KernelBody>;

    fn translate_body(&self, body: KernelBody, translation_m: [f64; 3]) -> Result<KernelBody>;

    fn rotate_body(
        &self,
        body: KernelBody,
        axis_origin_m: [f64; 3],
        axis_direction_m: [f64; 3],
        angle_rad: f64,
    ) -> Result<KernelBody>;

    fn mirror_body(
        &self,
        body: KernelBody,
        plane_origin_m: [f64; 3],
        plane_normal_m: [f64; 3],
    ) -> Result<KernelBody>;
}

/// In-memory mock kernel for tests and headless pipelines without OCCT.
#[derive(Debug, Default)]
pub struct MockGeometryKernel;

impl MockGeometryKernel {
    pub fn new() -> Self {
        Self
    }
}

impl GeometryKernel for MockGeometryKernel {
    fn make_wire_from_sketch(&self, sketch: &SolvedSketch) -> Result<KernelWire> {
        if sketch.points.len() < 2 {
            return Err(OpenCadError::validation(
                "sketch profile needs at least two points",
            ));
        }
        Ok(KernelWire::new(sketch.points.len() as u64))
    }

    fn extrude(
        &self,
        profile: KernelWire,
        extent: ExtrudeExtent,
        _operation: ExtrudeOperation,
        _target: Option<KernelBody>,
        _direction_m: [f64; 3],
    ) -> Result<KernelBody> {
        let length = match extent {
            ExtrudeExtent::Distance { length } => length.meters(),
            ExtrudeExtent::Symmetric { length } => length.meters() * 2.0,
            ExtrudeExtent::ThroughAll => 1.0,
        };
        if length <= 0.0 {
            return Err(OpenCadError::validation("extrude length must be positive"));
        }
        Ok(KernelBody::new(profile.0 + (length * 1000.0) as u64))
    }

    fn revolve(&self, input: &RevolveInput) -> Result<KernelBody> {
        let sketch = &input.sketch;
        let axis_direction_m = input.axis_direction_m;
        let angle_rad = input.angle_rad;
        let operation = input.operation;
        let target = input.target.clone();
        if sketch.points.len() < 2 {
            return Err(OpenCadError::validation(
                "sketch profile needs at least two points",
            ));
        }
        if angle_rad <= 0.0 {
            return Err(OpenCadError::validation("revolve angle must be positive"));
        }
        if angle_rad > std::f64::consts::TAU + 1e-6 {
            return Err(OpenCadError::validation(
                "revolve angle must not exceed 360° (2π rad)",
            ));
        }
        let axis_sum = axis_direction_m[0].abs()
            + axis_direction_m[1].abs()
            + axis_direction_m[2].abs();
        if axis_sum <= 1e-12 {
            return Err(OpenCadError::validation(
                "revolve axis direction must be a non-zero vector",
            ));
        }
        let body = KernelBody::new(
            (sketch.points.len() as u64)
                .wrapping_add((angle_rad * 1000.0) as u64)
                .max(1),
        );
        match operation {
            RevolveOperation::NewBody => Ok(body),
            RevolveOperation::Cut => {
                let Some(target) = target else {
                    return Err(OpenCadError::validation(
                        "cut revolve requires target body",
                    ));
                };
                self.boolean(target, body, BooleanOp::Subtract)
            }
            RevolveOperation::Join => {
                let Some(target) = target else {
                    return Err(OpenCadError::validation(
                        "join revolve requires target body",
                    ));
                };
                self.boolean(target, body, BooleanOp::Union)
            }
        }
    }

    fn boolean(&self, lhs: KernelBody, rhs: KernelBody, op: BooleanOp) -> Result<KernelBody> {
        let id = match op {
            BooleanOp::Union => lhs.0 ^ rhs.0,
            BooleanOp::Subtract => lhs.0.wrapping_sub(rhs.0),
            BooleanOp::Intersect => lhs.0 & rhs.0,
        };
        Ok(KernelBody::new(id))
    }

    fn tessellate(&self, body: &KernelBody, settings: &TessellationSettings) -> Result<MeshSet> {
        Ok(MeshSet::box_prism(
            body.0 as f64 * 0.001,
            settings.linear_deflection,
        ))
    }

    fn face_derivation_history(&self, _body: &KernelBody) -> Vec<(u64, u64)> {
        Vec::new()
    }

    fn mass_properties(&self, body: &KernelBody, density_kg_per_m3: f64) -> Result<MassProperties> {
        let side = body.0 as f64 * 0.001;
        let volume = side * side * side;
        Ok(MassProperties {
            volume_m3: volume,
            area_m2: 6.0 * side * side,
            mass_kg: volume * density_kg_per_m3,
            center_of_mass: [side / 2.0, side / 2.0, side / 2.0],
        })
    }

    fn bounding_box(&self, body: &KernelBody) -> Result<BoundingBox> {
        let side = body.0 as f64 * 0.001;
        Ok(BoundingBox {
            min: [0.0, 0.0, 0.0],
            max: [side, side, side],
        })
    }

    fn assign_face_ref(
        &self,
        body: &KernelBody,
        kernel_face_id: u64,
        ref_id: TopoRefId,
    ) -> Result<()> {
        let mesh = self.tessellate(body, &TessellationSettings::default())?;
        crate::topo_sync::validate_kernel_face_on_mesh(&mesh, kernel_face_id)?;
        let _ = ref_id;
        Ok(())
    }

    fn discover_body_edges(
        &self,
        _body: &KernelBody,
    ) -> Result<Vec<crate::topo_sync::EdgeRefDiscovery>> {
        Ok(Vec::new())
    }

    fn fillet_edges(
        &self,
        body: KernelBody,
        radius_m: f64,
        _selector: FilletEdgeSelector,
    ) -> Result<KernelBody> {
        if radius_m <= 0.0 {
            return Err(OpenCadError::validation("fillet radius must be positive"));
        }
        Ok(body)
    }

    fn chamfer_edges(
        &self,
        body: KernelBody,
        distance_m: f64,
        _selector: FilletEdgeSelector,
    ) -> Result<KernelBody> {
        if distance_m <= 0.0 {
            return Err(OpenCadError::validation(
                "chamfer distance must be positive",
            ));
        }
        Ok(body)
    }

    fn translate_body(&self, body: KernelBody, translation_m: [f64; 3]) -> Result<KernelBody> {
        if translation_m[0] == 0.0 && translation_m[1] == 0.0 && translation_m[2] == 0.0 {
            return Ok(body);
        }
        let delta = ((translation_m[0].abs() + translation_m[1].abs() + translation_m[2].abs())
            * 1000.0) as u64;
        Ok(KernelBody::new(body.0.wrapping_add(delta.max(1))))
    }

    fn rotate_body(
        &self,
        body: KernelBody,
        _axis_origin_m: [f64; 3],
        _axis_direction_m: [f64; 3],
        angle_rad: f64,
    ) -> Result<KernelBody> {
        let delta = (angle_rad.abs() * 1000.0) as u64;
        Ok(KernelBody::new(body.0.wrapping_add(delta.max(1))))
    }

    fn mirror_body(
        &self,
        body: KernelBody,
        plane_origin_m: [f64; 3],
        plane_normal_m: [f64; 3],
    ) -> Result<KernelBody> {
        let flip = plane_normal_m[0].abs() + plane_normal_m[1].abs() + plane_normal_m[2].abs();
        if flip <= 1e-12 {
            return Err(OpenCadError::validation(
                "mirror plane normal must be a non-zero vector",
            ));
        }
        let delta = ((plane_origin_m[0].abs() + plane_origin_m[1].abs() + plane_origin_m[2].abs())
            * 1000.0) as u64;
        Ok(KernelBody::new(body.0.wrapping_add(delta.max(1))))
    }
}

#[cfg(test)]
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
    fn mock_kernel_extrude_produces_body() {
        let kernel = MockGeometryKernel::new();
        let wire = kernel
            .make_wire_from_sketch(&rectangle_sketch())
            .expect("wire");
        let body = kernel
            .extrude(
                wire,
                ExtrudeExtent::Distance {
                    length: Length::from_unit(6.0, opencad_core::LengthUnit::Millimeter),
                },
                ExtrudeOperation::NewBody,
                None,
                [0.0, 0.0, 1.0],
            )
            .expect("extrude");
        assert!(body.0 > 0);
    }

    #[test]
    fn mock_kernel_mass_properties() {
        let kernel = MockGeometryKernel::new();
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
        assert!(mass.volume_m3 > 0.0);
        assert!(mass.mass_kg > 0.0);
    }
}
