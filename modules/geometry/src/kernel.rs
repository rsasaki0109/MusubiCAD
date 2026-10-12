use serde::{Deserialize, Serialize};

use opencad_core::{Length, OpenCadError, Result, TopoRefId};

use crate::mass::{BoundingBox, MassProperties};
use crate::tessellation::{MeshSet, TessellationSettings};
use crate::transform::RigidTransform;

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

/// A face picked by geometry: a point on (or in the plane of) the face and
/// its outward normal, both from the tessellated body.
///
/// Kernel face IDs are not reliable across kernels: OCCT faces can share
/// one underlying surface record (an extrusion's top and bottom faces), and
/// tessellation reads IDs from a deep copy.  Picks are matched against the
/// kernel's own faces instead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FacePick {
    /// A point on the face or in its plane, in metres.
    pub point_m: [f64; 3],
    /// Unit outward normal (dimensionless).
    pub normal: [f64; 3],
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

/// An exact circle in sketch coordinates (ADR-020).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SolvedCircle {
    /// Centre in sketch-plane coordinates, in metres.
    pub center_m: [f64; 2],
    /// Radius in metres.
    pub radius_m: f64,
}

/// One segment of a profile loop in sketch coordinates, in metres
/// (ADR-021).  Arcs are given by three points so the orientation follows
/// the loop.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileSegment {
    Line {
        start_m: [f64; 2],
        end_m: [f64; 2],
    },
    Arc {
        start_m: [f64; 2],
        mid_m: [f64; 2],
        end_m: [f64; 2],
    },
}

/// 2D profile input for wire creation (sketch solver output).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolvedSketch {
    pub profile_ref: String,
    /// Polygon vertices.  For a circle profile these are an inscribed
    /// polygon, kept for kernels without exact curves; kernels that support
    /// curves build the profile from [`Self::circle`] instead.
    pub points: Vec<[f64; 2]>,
    pub closed: bool,
    /// The exact circle when the profile is a single circle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circle: Option<SolvedCircle>,
    /// Exact loop segments when the loop contains arcs (ADR-021); `points`
    /// then holds a sampled polygon for kernels without curves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<ProfileSegment>,
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

/// A right-handed helix around an axis for helical sweeps (ADR-025).  The
/// helix passes through the profile's centre, so its radius is that centre's
/// distance from the axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HelixSpec {
    pub axis_origin_m: [f64; 3],
    pub axis_direction_m: [f64; 3],
    /// Axial advance per turn, in metres.
    pub pitch_m: f64,
    /// Total axial length, in metres.
    pub height_m: f64,
}

impl HelixSpec {
    /// Reject non-finite, non-positive, or degenerate helix parameters.
    pub fn validate(&self) -> Result<()> {
        if !(self.pitch_m.is_finite() && self.pitch_m > 0.0) {
            return Err(OpenCadError::validation("helix pitch must be positive"));
        }
        if !(self.height_m.is_finite() && self.height_m > 0.0) {
            return Err(OpenCadError::validation("helix height must be positive"));
        }
        let [x, y, z] = self.axis_direction_m;
        if !(x * x + y * y + z * z).is_normal() {
            return Err(OpenCadError::validation(
                "helix axis direction must be non-zero",
            ));
        }
        Ok(())
    }
}

/// Analytic full circular edge in model metres; never a sampled curve fit.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CircularEdgeGeometry {
    pub center_m: [f64; 3],
    pub normal: [f64; 3],
    pub radius_m: f64,
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

    /// Combine existing bodies; OCCT refuses empty or non-finite-volume outputs.
    fn body_boolean(
        &self,
        target: KernelBody,
        source: KernelBody,
        op: BooleanOp,
    ) -> Result<KernelBody> {
        self.boolean(target, source, op)
    }

    /// Exact common-solid volume in cubic meters.
    ///
    /// The default implementation intersects the two bodies and reads the volume
    /// of the single result. Kernels whose intersection can be empty or split
    /// into several disconnected pieces (for example OCCT) must override this to
    /// sum the piece volumes and return `0.0` for an empty intersection instead
    /// of failing.
    fn intersection_volume(&self, lhs: &KernelBody, rhs: &KernelBody) -> Result<f64> {
        let common = self.boolean(lhs.clone(), rhs.clone(), BooleanOp::Intersect)?;
        Ok(self.mass_properties(&common, 1.0)?.volume_m3)
    }

    fn tessellate(&self, body: &KernelBody, settings: &TessellationSettings) -> Result<MeshSet>;

    /// Face derivation pairs `(post_id, src_id)` from the kernel's last modifying op.
    fn face_derivation_history(&self, body: &KernelBody) -> Vec<(u64, u64)>;

    fn mass_properties(&self, body: &KernelBody, density_kg_per_m3: f64) -> Result<MassProperties>;

    /// Inertia tensor in kg·m² about the body's centre of mass, expressed in
    /// the body's coordinate axes, for a uniform `density_kg_per_m3`.
    fn inertia_about_com(
        &self,
        body: &KernelBody,
        density_kg_per_m3: f64,
    ) -> Result<[[f64; 3]; 3]> {
        let _ = (body, density_kg_per_m3);
        Err(OpenCadError::Other(
            "this geometry kernel does not compute inertia tensors".into(),
        ))
    }

    fn bounding_box(&self, body: &KernelBody) -> Result<BoundingBox>;

    /// Serialize a body as STEP (ISO 10303-21) text whose length unit is the
    /// millimetre, the STEP convention.  Kernel lengths are metres, so
    /// implementations scale by 1000 on export.  The output must be
    /// deterministic for identical geometry.
    fn export_step(&self, _body: &KernelBody) -> Result<Vec<u8>> {
        Err(OpenCadError::validation(
            "this geometry kernel does not support STEP export",
        ))
    }

    /// Import every solid of a millimetre STEP file as one body in metres.
    fn import_step(&self, _step: &[u8]) -> Result<KernelBody> {
        Err(OpenCadError::validation(
            "this geometry kernel does not support STEP import",
        ))
    }

    /// Sweep a closed `profile` along `path`, an open or closed chain of
    /// line and arc segments placed by its own sketch placement (ADR-023).
    /// The profile keeps its orientation relative to the path plane normal.
    fn sweep(&self, _profile: &SolvedSketch, _path: &SolvedSketch) -> Result<KernelBody> {
        Err(OpenCadError::validation(
            "this geometry kernel does not support sweeps",
        ))
    }

    /// Sweep a closed `profile` along a helix through its centre (ADR-025).
    /// The profile moves by a screw motion about the helix axis.
    fn helix_sweep(&self, _profile: &SolvedSketch, _helix: &HelixSpec) -> Result<KernelBody> {
        Err(OpenCadError::validation(
            "this geometry kernel does not support helical sweeps",
        ))
    }

    /// Skin a solid through two or more closed section profiles, each placed
    /// by its own sketch placement, in order (ADR-022).
    fn loft(&self, _sections: &[SolvedSketch]) -> Result<KernelBody> {
        Err(OpenCadError::validation(
            "this geometry kernel does not support lofts",
        ))
    }

    /// Tilt selected planar side faces about their neutral-plane intersections.
    /// Implementations must reject unsupported/non-convex geometry (ADR-036).
    fn draft_body(&self, _body: KernelBody, _input: &crate::DraftInput) -> Result<KernelBody> {
        Err(OpenCadError::validation(
            "this geometry kernel does not support draft",
        ))
    }

    /// Hollow `body` to a uniform inward wall of `thickness_m` metres,
    /// removing the picked faces to form openings (ADR-017).  Every pick must
    /// match exactly one face of `body`, or the call fails.
    fn shell_body(
        &self,
        _body: KernelBody,
        _thickness_m: f64,
        _open_faces: &[FacePick],
    ) -> Result<KernelBody> {
        Err(OpenCadError::validation(
            "this geometry kernel does not support shelling",
        ))
    }

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

    /// End vertices of one current body edge, in model meters (ADR-034).
    /// A linear dimension measures their chord, not curved-edge arc length.
    fn edge_endpoints(&self, _body: &KernelBody, _kernel_edge_id: u64) -> Result<[[f64; 3]; 2]> {
        Err(OpenCadError::validation(
            "this geometry kernel does not expose edge endpoints",
        ))
    }

    /// Exact polygonal slice; unsupported geometry fails closed.
    fn section_body(
        &self,
        _body: &KernelBody,
        _plane: &crate::SectionPlane,
    ) -> Result<crate::SectionGeometry> {
        Err(OpenCadError::validation(
            "kernel does not support polygonal sections",
        ))
    }

    /// Query an exact, complete circle. Arcs and noncircular curves are unsupported.
    fn circular_edge_geometry(
        &self,
        _body: &KernelBody,
        _edge_id: u64,
    ) -> Result<CircularEdgeGeometry> {
        Err(OpenCadError::validation(
            "kernel does not support analytic circular edges",
        ))
    }

    /// Exact straight-edge query for angular measurements; unsupported by default.
    fn straight_edge_endpoints(&self, _body: &KernelBody, _edge_id: u64) -> Result<[[f64; 3]; 2]> {
        Err(OpenCadError::validation(
            "kernel does not expose straight-edge geometry",
        ))
    }

    fn fillet_edges(
        &self,
        body: KernelBody,
        radius_m: f64,
        selector: FilletEdgeSelector,
    ) -> Result<KernelBody>;

    /// Linear radius evolution on one straight edge. Endpoint radii follow
    /// lexicographically increasing world coordinates, independent of BREP order.
    fn variable_fillet_edge(
        &self,
        _body: KernelBody,
        start_radius_m: f64,
        end_radius_m: f64,
        _selector: FilletEdgeSelector,
    ) -> Result<KernelBody> {
        validate_variable_fillet_radii(start_radius_m, end_radius_m)?;
        Err(OpenCadError::validation(
            "variable fillets are unsupported by this kernel",
        ))
    }

    fn chamfer_edges(
        &self,
        body: KernelBody,
        distance_m: f64,
        selector: FilletEdgeSelector,
    ) -> Result<KernelBody>;

    fn translate_body(&self, body: KernelBody, translation_m: [f64; 3]) -> Result<KernelBody>;

    fn transform_body(&self, body: KernelBody, transform: RigidTransform) -> Result<KernelBody>;

    fn make_compound(&self, bodies: &[KernelBody]) -> Result<KernelBody>;

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
    /// A deterministic stand-in body derived from the byte length, so
    /// imported-solid pipelines run without OCCT (ADR-016).  Mock export
    /// stays unsupported.
    fn import_step(&self, step: &[u8]) -> Result<KernelBody> {
        if step.is_empty() {
            return Err(OpenCadError::validation("STEP file contains no solids"));
        }
        Ok(KernelBody::new((step.len() as u64 % 97).max(1)))
    }

    /// A deterministic stand-in body derived from the inputs (ADR-023).
    fn sweep(&self, profile: &SolvedSketch, path: &SolvedSketch) -> Result<KernelBody> {
        if path.segments.is_empty() {
            return Err(OpenCadError::validation("sweep path has no segments"));
        }
        let seed = (profile.points.len() as u64)
            .wrapping_mul(1_000_003)
            .wrapping_add(path.segments.len() as u64);
        Ok(KernelBody::new(seed.max(1)))
    }

    /// A deterministic stand-in body derived from the inputs (ADR-025).
    fn helix_sweep(&self, profile: &SolvedSketch, helix: &HelixSpec) -> Result<KernelBody> {
        helix.validate()?;
        let turns = (helix.height_m / helix.pitch_m).ceil() as u64;
        let seed = (profile.points.len() as u64)
            .wrapping_mul(1_000_033)
            .wrapping_add(turns);
        Ok(KernelBody::new(seed.max(1)))
    }

    /// A deterministic stand-in body derived from the section count and
    /// point counts, so loft pipelines run without OCCT (ADR-022).
    fn loft(&self, sections: &[SolvedSketch]) -> Result<KernelBody> {
        if sections.len() < 2 {
            return Err(OpenCadError::validation("loft needs at least two sections"));
        }
        let seed = sections.iter().fold(sections.len() as u64, |acc, section| {
            acc.wrapping_mul(31)
                .wrapping_add(section.points.len() as u64)
        });
        Ok(KernelBody::new(seed.max(1)))
    }

    fn draft_body(&self, body: KernelBody, input: &crate::DraftInput) -> Result<KernelBody> {
        input.validate()?;
        let hash = input
            .neutral_origin_m
            .iter()
            .chain(&input.neutral_normal)
            .chain(std::iter::once(&input.angle_rad))
            .chain(
                input
                    .faces
                    .iter()
                    .flat_map(|f| f.point_m.iter().chain(&f.normal)),
            )
            .fold(body.0, |h, v| h.wrapping_mul(31).wrapping_add(v.to_bits()));
        Ok(KernelBody::new(hash % 1_000_000 + 1))
    }

    /// A deterministic stand-in body derived from the inputs, so shell
    /// pipelines run without OCCT (ADR-017).
    fn shell_body(
        &self,
        body: KernelBody,
        thickness_m: f64,
        open_faces: &[FacePick],
    ) -> Result<KernelBody> {
        if !thickness_m.is_finite() || thickness_m <= 0.0 {
            return Err(OpenCadError::validation("shell thickness must be positive"));
        }
        if open_faces.is_empty() {
            return Err(OpenCadError::validation(
                "shell needs at least one open face",
            ));
        }
        let micrometres = (thickness_m * 1e6).round() as u64;
        let faces = open_faces.iter().fold(0_u64, |acc, pick| {
            pick.point_m
                .iter()
                .chain(&pick.normal)
                .fold(acc, |acc, value| {
                    acc.wrapping_mul(31)
                        .wrapping_add((value * 1e6).round() as i64 as u64)
                })
        });
        Ok(KernelBody::new(
            body.0
                .wrapping_mul(1_000_003)
                .wrapping_add(micrometres)
                .wrapping_add(faces)
                .max(1),
        ))
    }

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
        let axis_sum =
            axis_direction_m[0].abs() + axis_direction_m[1].abs() + axis_direction_m[2].abs();
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
                    return Err(OpenCadError::validation("cut revolve requires target body"));
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

    fn body_boolean(
        &self,
        target: KernelBody,
        source: KernelBody,
        op: BooleanOp,
    ) -> Result<KernelBody> {
        // Mock IDs model operation plumbing, not geometric emptiness.
        self.boolean(target, source, op)
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

    /// The mock body is a solid cube of side `body.0` mm: `m·s²/6` on each axis.
    fn inertia_about_com(
        &self,
        body: &KernelBody,
        density_kg_per_m3: f64,
    ) -> Result<[[f64; 3]; 3]> {
        let mass = self.mass_properties(body, density_kg_per_m3)?;
        let side = body.0 as f64 * 0.001;
        let moment = mass.mass_kg * side * side / 6.0;
        Ok([[moment, 0.0, 0.0], [0.0, moment, 0.0], [0.0, 0.0, moment]])
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
        body: &KernelBody,
    ) -> Result<Vec<crate::topo_sync::EdgeRefDiscovery>> {
        if body.0 == 0 {
            return Ok(Vec::new());
        }
        let roles = ["top@-y", "top@+x", "top@+y", "top@-x"];
        roles
            .iter()
            .enumerate()
            .map(|(index, role)| {
                let points = self.edge_endpoints(body, index as u64 + 1)?;
                let side = body.0 as f64 * 0.001;
                Ok(crate::topo_sync::EdgeRefDiscovery {
                    kernel_edge_id: index as u64 + 1,
                    role: (*role).into(),
                    midpoint_m: std::array::from_fn(|axis| {
                        ((points[0][axis] + points[1][axis]) * 0.5) as f32
                    }),
                    tangent_m: std::array::from_fn(|axis| {
                        ((points[1][axis] - points[0][axis]) / side) as f32
                    }),
                    length_m: side as f32,
                    feature_id: None,
                })
            })
            .collect()
    }

    fn edge_endpoints(&self, body: &KernelBody, kernel_edge_id: u64) -> Result<[[f64; 3]; 2]> {
        let side = body.0 as f64 * 0.001;
        let points = [
            [0.0, 0.0, side],
            [side, 0.0, side],
            [side, side, side],
            [0.0, side, side],
        ];
        let index = kernel_edge_id
            .checked_sub(1)
            .and_then(|i| usize::try_from(i).ok())
            .filter(|i| *i < 4)
            .ok_or_else(|| OpenCadError::not_found(format!("mock edge {kernel_edge_id}")))?;
        Ok([points[index], points[(index + 1) % 4]])
    }

    fn section_body(
        &self,
        body: &KernelBody,
        plane: &crate::SectionPlane,
    ) -> Result<crate::SectionGeometry> {
        plane.validate_crossing(&self.bounding_box(body)?)?;
        let axis = (0..3)
            .find(|i| plane.normal[*i].abs() > 1. - 1e-9)
            .ok_or_else(|| {
                OpenCadError::validation("mock sections require an axis-aligned plane")
            })?;
        let axes: Vec<_> = (0..3).filter(|i| *i != axis).collect();
        let side = body.0 as f64 * 0.001;
        let points: Vec<[f64; 3]> = [[0., 0.], [side, 0.], [side, side], [0., side]]
            .into_iter()
            .map(|p| {
                let mut v = [0.; 3];
                v[axis] = plane.origin_m[axis];
                v[axes[0]] = p[0];
                v[axes[1]] = p[1];
                v
            })
            .collect();
        crate::polygonal_section(
            plane,
            (0..4).map(|i| [points[i], points[(i + 1) % 4]]).collect(),
        )
    }

    fn circular_edge_geometry(
        &self,
        body: &KernelBody,
        edge_id: u64,
    ) -> Result<CircularEdgeGeometry> {
        // Mock contract: synthetic edge 5 is a complete XY circle, radius half
        // the body's deterministic side length. Box edges 1..4 remain straight.
        if edge_id != 5 || body.0 == 0 {
            return Err(OpenCadError::validation("mock edge is not a circle"));
        }
        let side = body.0 as f64 * 0.001;
        Ok(CircularEdgeGeometry {
            center_m: [side / 2., side / 2., side],
            normal: [0., 0., 1.],
            radius_m: side / 2.,
        })
    }

    fn straight_edge_endpoints(&self, body: &KernelBody, edge_id: u64) -> Result<[[f64; 3]; 2]> {
        self.edge_endpoints(body, edge_id)
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

    fn variable_fillet_edge(
        &self,
        body: KernelBody,
        start_radius_m: f64,
        end_radius_m: f64,
        selector: FilletEdgeSelector,
    ) -> Result<KernelBody> {
        validate_variable_fillet_radii(start_radius_m, end_radius_m)?;
        match selector {
            FilletEdgeSelector::KernelEdges { kernel_edge_ids } if kernel_edge_ids.len() == 1 => {
                Ok(KernelBody::new(body.0 + 1))
            }
            FilletEdgeSelector::EdgeRole { role } if !role.trim().is_empty() => {
                Ok(KernelBody::new(body.0 + 1))
            }
            _ => Err(OpenCadError::validation(
                "variable fillet requires one edge selector",
            )),
        }
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

    fn transform_body(&self, body: KernelBody, transform: RigidTransform) -> Result<KernelBody> {
        if transform.is_identity() {
            return Ok(body);
        }
        self.translate_body(body, transform.translation_m)
    }

    fn make_compound(&self, bodies: &[KernelBody]) -> Result<KernelBody> {
        if bodies.is_empty() {
            return Err(OpenCadError::validation(
                "compound requires at least one body",
            ));
        }
        if bodies.len() == 1 {
            return Ok(bodies[0].clone());
        }
        let id = bodies
            .iter()
            .fold(0_u64, |acc, body| acc.wrapping_add(body.0))
            .max(1);
        Ok(KernelBody::new(id))
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

/// Kernel-level check shared by Mock and OCCT before topology changes.
pub fn validate_variable_fillet_radii(start_m: f64, end_m: f64) -> Result<()> {
    if [start_m, end_m].iter().any(|r| !r.is_finite() || *r < 1e-9) {
        return Err(OpenCadError::validation(
            "variable fillet radii must be finite and at least 1e-9 m",
        ));
    }
    Ok(())
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
            circle: None,
            segments: Vec::new(),
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
