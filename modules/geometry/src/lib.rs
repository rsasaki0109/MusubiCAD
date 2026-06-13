//! Kernel-neutral B-Rep abstractions, topology references, and mass properties.
//!
//! OCCT types must not leak outside `opencad-kernel-occt`.

pub mod brep;
pub mod kernel;
pub mod mass;
pub mod nurbs;
pub mod refs;
pub mod stl;
pub mod tessellation;
pub mod topo_sync;
pub mod topology;

pub use kernel::{
    BooleanOp, ExtrudeExtent, ExtrudeOperation, FilletEdgeSelector, GeometryKernel, KernelBody,
    KernelWire, MockGeometryKernel, ProfilePlane, RevolveInput, RevolveOperation, SketchPlacement,
    SolvedSketch,
};
pub use mass::{BoundingBox, MassProperties};
pub use nurbs::NurbsSurface;
pub use refs::{GeometricFingerprint, TopoRef, TopoRefKind, TopoRefSemantic};
pub use stl::write_binary_stl;
pub use tessellation::{MeshSet, TessellationSettings};
pub use topo_sync::{
    assign_face_ref_to_refs, assign_named_face_ref, build_src_to_post_map,
    compose_face_derivation_histories, kernel_topo_ref_id, match_face_discovery_for_topo_ref,
    rebind_kernel_face_ids, resolve_kernel_face_id_for_topo_ref,
    resolve_kernel_face_id_for_topo_ref_with_discoveries, resolve_topo_ref_id,
    resolve_topo_ref_id_with_history, sync_semantic_refs, sync_semantic_refs_with_history,
    validate_kernel_face_on_mesh, EdgeRefDiscovery, FaceDerivation, FaceRefDiscovery,
    match_edge_discovery_for_topo_ref, resolve_kernel_edge_id_for_topo_ref,
};
