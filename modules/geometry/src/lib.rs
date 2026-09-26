//! Kernel-neutral B-Rep abstractions, topology references, and mass properties.
//!
//! OCCT types must not leak outside `opencad-kernel-occt`.

pub mod brep;
pub mod instrument;
pub mod kernel;
pub mod mass;
pub mod nurbs;
pub mod provenance;
pub mod refs;
pub mod stl;
pub mod tessellation;
pub mod topo_sync;
pub mod topology;
pub mod transform;

pub use instrument::CountingGeometryKernel;
pub use kernel::{
    BooleanOp, ExtrudeExtent, ExtrudeOperation, FacePick, FilletEdgeSelector, GeometryKernel,
    KernelBody, KernelWire, MockGeometryKernel, ProfilePlane, RevolveInput, RevolveOperation,
    SketchPlacement, SolvedCircle, SolvedSketch,
};
pub use mass::{BoundingBox, MassProperties};
pub use nurbs::NurbsSurface;
pub use provenance::{
    mark_consumed_references, resolve_all_reference_provenance, resolve_edge_ref_with_provenance,
    resolve_face_ref_with_provenance, CandidateEvidence, ReferenceProvenance, ReferenceResolution,
    ReferenceStatus,
};
pub use refs::{
    GeometricFingerprint, TopoRef, TopoRefIdentity, TopoRefKind, TopoRefSemantic,
    TopoRefTolerancePolicy, DEFAULT_EDGE_MIDPOINT_TOLERANCE_M, DEFAULT_FACE_CENTROID_TOLERANCE_M,
    DEFAULT_NORMAL_ALIGNMENT_MIN_DOT, DEFAULT_TANGENT_ALIGNMENT_MIN_DOT,
    DEFAULT_VECTOR_NORM_EPSILON,
};
pub use stl::write_binary_stl;
pub use tessellation::{MeshSet, TessellationSettings};
pub use topo_sync::{
    assign_face_ref_to_refs, assign_named_face_ref, build_src_to_post_map,
    compose_face_derivation_histories, kernel_topo_ref_id, match_edge_discovery_for_topo_ref,
    match_edge_discovery_for_topo_ref_with_policy, match_face_discovery_for_topo_ref,
    match_face_discovery_for_topo_ref_with_policy, rebind_kernel_face_ids,
    resolve_kernel_edge_id_for_topo_ref, resolve_kernel_edge_id_for_topo_ref_with_policy,
    resolve_kernel_face_id_for_topo_ref, resolve_kernel_face_id_for_topo_ref_with_discoveries,
    resolve_kernel_face_id_for_topo_ref_with_discoveries_and_policy,
    resolve_kernel_face_id_for_topo_ref_with_policy, resolve_topo_ref_id,
    resolve_topo_ref_id_with_history, sync_semantic_refs, sync_semantic_refs_with_history,
    validate_kernel_face_on_mesh, EdgeRefDiscovery, FaceDerivation, FaceRefDiscovery,
};
pub use transform::{RigidTransform, POSITION_TOLERANCE_M, ROTATION_TOLERANCE};
