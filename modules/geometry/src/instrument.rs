//! Deterministic call-count instrumentation for geometry-kernel operations.

use std::cell::Cell;

use opencad_core::{Result, TopoRefId};

use crate::{
    BooleanOp, BoundingBox, EdgeRefDiscovery, ExtrudeExtent, ExtrudeOperation, FaceDerivation,
    FilletEdgeSelector, GeometryKernel, HelixSpec, KernelBody, KernelWire, MassProperties, MeshSet,
    RevolveInput, RigidTransform, SolvedSketch, TessellationSettings,
};

/// Borrowing kernel adapter that counts every backend-neutral kernel call.
///
/// The counter is observation only. It does not change inputs, outputs, call
/// ordering, or persisted document state.
pub struct CountingGeometryKernel<'a, K: GeometryKernel> {
    inner: &'a K,
    calls: Cell<u64>,
}

impl<'a, K: GeometryKernel> CountingGeometryKernel<'a, K> {
    pub fn new(inner: &'a K) -> Self {
        Self {
            inner,
            calls: Cell::new(0),
        }
    }

    pub fn call_count(&self) -> u64 {
        self.calls.get()
    }

    fn record(&self) {
        self.calls.set(self.calls.get().saturating_add(1));
    }
}

impl<K: GeometryKernel> GeometryKernel for CountingGeometryKernel<'_, K> {
    fn import_triangle_solid(&self, solid: &crate::TriangleSolid) -> Result<KernelBody> {
        self.record();
        self.inner.import_triangle_solid(solid)
    }
    fn make_wire_from_sketch(&self, sketch: &SolvedSketch) -> Result<KernelWire> {
        self.record();
        self.inner.make_wire_from_sketch(sketch)
    }

    fn extrude(
        &self,
        profile: KernelWire,
        extent: ExtrudeExtent,
        operation: ExtrudeOperation,
        target: Option<KernelBody>,
        direction_m: [f64; 3],
    ) -> Result<KernelBody> {
        self.record();
        self.inner
            .extrude(profile, extent, operation, target, direction_m)
    }

    fn revolve(&self, input: &RevolveInput) -> Result<KernelBody> {
        self.record();
        self.inner.revolve(input)
    }

    fn boolean(&self, lhs: KernelBody, rhs: KernelBody, op: BooleanOp) -> Result<KernelBody> {
        self.record();
        self.inner.boolean(lhs, rhs, op)
    }

    fn body_boolean(
        &self,
        target: KernelBody,
        source: KernelBody,
        op: BooleanOp,
    ) -> Result<KernelBody> {
        self.record();
        self.inner.body_boolean(target, source, op)
    }

    fn intersection_volume(&self, lhs: &KernelBody, rhs: &KernelBody) -> Result<f64> {
        self.record();
        self.inner.intersection_volume(lhs, rhs)
    }

    fn tessellate(&self, body: &KernelBody, settings: &TessellationSettings) -> Result<MeshSet> {
        self.record();
        self.inner.tessellate(body, settings)
    }

    fn cylindrical_stock_geometry(
        &self,
        body: &KernelBody,
        axis_hint: [f64; 3],
    ) -> Result<crate::CylindricalStockGeometry> {
        self.record();
        self.inner.cylindrical_stock_geometry(body, axis_hint)
    }
    fn cylindrical_stock_rims(
        &self,
        body: &KernelBody,
        axis_hint: [f64; 3],
    ) -> Result<crate::CylindricalRimsGeometry> {
        self.record();
        self.inner.cylindrical_stock_rims(body, axis_hint)
    }
    fn draft_cylindrical_stock(
        &self,
        body: KernelBody,
        input: &crate::CylindricalDraftInput,
    ) -> Result<KernelBody> {
        self.record();
        self.inner.draft_cylindrical_stock(body, input)
    }

    fn add_tapered_rib_stock(
        &self,
        body: KernelBody,
        input: &crate::TaperedRibInput,
    ) -> Result<KernelBody> {
        self.record();
        self.inner.add_tapered_rib_stock(body, input)
    }

    fn cut_nominal_metric_thread(
        &self,
        body: KernelBody,
        input: &crate::NominalThreadInput,
    ) -> Result<KernelBody> {
        self.record();
        self.inner.cut_nominal_metric_thread(body, input)
    }

    fn cut_countersunk_stock(
        &self,
        body: KernelBody,
        cut: &crate::CountersinkCut,
    ) -> Result<KernelBody> {
        self.record();
        self.inner.cut_countersunk_stock(body, cut)
    }

    fn rectangular_stock_geometry(
        &self,
        body: &KernelBody,
        pick: crate::FacePick,
    ) -> Result<crate::RectangularStockGeometry> {
        self.record();
        self.inner.rectangular_stock_geometry(body, pick)
    }

    fn face_derivation_history(&self, body: &KernelBody) -> Vec<FaceDerivation> {
        self.record();
        self.inner.face_derivation_history(body)
    }

    fn mass_properties(&self, body: &KernelBody, density_kg_per_m3: f64) -> Result<MassProperties> {
        self.record();
        self.inner.mass_properties(body, density_kg_per_m3)
    }

    fn inertia_about_com(
        &self,
        body: &KernelBody,
        density_kg_per_m3: f64,
    ) -> Result<[[f64; 3]; 3]> {
        self.record();
        self.inner.inertia_about_com(body, density_kg_per_m3)
    }

    fn bounding_box(&self, body: &KernelBody) -> Result<BoundingBox> {
        self.record();
        self.inner.bounding_box(body)
    }

    fn assign_face_ref(
        &self,
        body: &KernelBody,
        kernel_face_id: u64,
        ref_id: TopoRefId,
    ) -> Result<()> {
        self.record();
        self.inner.assign_face_ref(body, kernel_face_id, ref_id)
    }

    fn discover_body_edges(&self, body: &KernelBody) -> Result<Vec<EdgeRefDiscovery>> {
        self.record();
        self.inner.discover_body_edges(body)
    }

    fn edge_endpoints(&self, body: &KernelBody, kernel_edge_id: u64) -> Result<[[f64; 3]; 2]> {
        self.record();
        self.inner.edge_endpoints(body, kernel_edge_id)
    }

    fn draft_body(&self, body: KernelBody, input: &crate::DraftInput) -> Result<KernelBody> {
        self.record();
        self.inner.draft_body(body, input)
    }

    fn section_body(
        &self,
        body: &KernelBody,
        plane: &crate::SectionPlane,
    ) -> Result<crate::SectionGeometry> {
        self.record();
        self.inner.section_body(body, plane)
    }
    fn circular_edge_geometry(
        &self,
        body: &KernelBody,
        edge_id: u64,
    ) -> Result<crate::CircularEdgeGeometry> {
        self.record();
        self.inner.circular_edge_geometry(body, edge_id)
    }

    fn straight_edge_endpoints(&self, body: &KernelBody, edge_id: u64) -> Result<[[f64; 3]; 2]> {
        self.record();
        self.inner.straight_edge_endpoints(body, edge_id)
    }

    fn fillet_edges(
        &self,
        body: KernelBody,
        radius_m: f64,
        selector: FilletEdgeSelector,
    ) -> Result<KernelBody> {
        self.record();
        self.inner.fillet_edges(body, radius_m, selector)
    }

    fn variable_fillet_edge(
        &self,
        body: KernelBody,
        start_radius_m: f64,
        end_radius_m: f64,
        selector: FilletEdgeSelector,
    ) -> Result<KernelBody> {
        self.record();
        self.inner
            .variable_fillet_edge(body, start_radius_m, end_radius_m, selector)
    }

    fn chamfer_edges(
        &self,
        body: KernelBody,
        distance_m: f64,
        selector: FilletEdgeSelector,
    ) -> Result<KernelBody> {
        self.record();
        self.inner.chamfer_edges(body, distance_m, selector)
    }

    fn translate_body(&self, body: KernelBody, translation_m: [f64; 3]) -> Result<KernelBody> {
        self.record();
        self.inner.translate_body(body, translation_m)
    }

    fn transform_body(&self, body: KernelBody, transform: RigidTransform) -> Result<KernelBody> {
        self.record();
        self.inner.transform_body(body, transform)
    }

    fn make_compound(&self, bodies: &[KernelBody]) -> Result<KernelBody> {
        self.record();
        self.inner.make_compound(bodies)
    }

    // Methods with default implementations must be forwarded explicitly, or
    // the wrapper would silently answer with the default.
    fn export_step(&self, body: &KernelBody) -> Result<Vec<u8>> {
        self.inner.export_step(body)
    }

    fn import_step(&self, step: &[u8]) -> Result<KernelBody> {
        self.record();
        self.inner.import_step(step)
    }

    fn sweep(&self, profile: &SolvedSketch, path: &SolvedSketch) -> Result<KernelBody> {
        self.record();
        self.inner.sweep(profile, path)
    }

    fn helix_sweep(&self, profile: &SolvedSketch, helix: &HelixSpec) -> Result<KernelBody> {
        self.record();
        self.inner.helix_sweep(profile, helix)
    }

    fn loft(&self, sections: &[SolvedSketch]) -> Result<KernelBody> {
        self.record();
        self.inner.loft(sections)
    }

    fn shell_body(
        &self,
        body: KernelBody,
        thickness_m: f64,
        open_faces: &[crate::FacePick],
    ) -> Result<KernelBody> {
        self.record();
        self.inner.shell_body(body, thickness_m, open_faces)
    }

    fn rotate_body(
        &self,
        body: KernelBody,
        axis_origin_m: [f64; 3],
        axis_direction_m: [f64; 3],
        angle_rad: f64,
    ) -> Result<KernelBody> {
        self.record();
        self.inner
            .rotate_body(body, axis_origin_m, axis_direction_m, angle_rad)
    }

    fn mirror_body(
        &self,
        body: KernelBody,
        plane_origin_m: [f64; 3],
        plane_normal_m: [f64; 3],
    ) -> Result<KernelBody> {
        self.record();
        self.inner.mirror_body(body, plane_origin_m, plane_normal_m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProfilePlane, SketchPlacement};

    #[test]
    fn defaulted_methods_are_forwarded_to_the_inner_kernel() {
        let inner = crate::MockGeometryKernel::new();
        let kernel = CountingGeometryKernel::new(&inner);
        // Mock implements import_step; the trait default would refuse it.
        assert_eq!(
            kernel.import_step(b"ISO-10303-21;").expect("forwarded"),
            inner.import_step(b"ISO-10303-21;").expect("mock")
        );
        let top = [crate::FacePick {
            point_m: [0.0, 0.0, 0.02],
            normal: [0.0, 0.0, 1.0],
        }];
        assert_eq!(
            kernel
                .shell_body(KernelBody::new(7), 0.002, &top)
                .expect("forwarded"),
            inner
                .shell_body(KernelBody::new(7), 0.002, &top)
                .expect("mock")
        );
        assert_eq!(kernel.call_count(), 2);
    }

    #[test]
    fn counts_calls_without_changing_mock_results() {
        let inner = crate::MockGeometryKernel::new();
        let kernel = CountingGeometryKernel::new(&inner);
        let wire = kernel
            .make_wire_from_sketch(&SolvedSketch {
                profile_ref: "profile:test".into(),
                points: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
                closed: true,
                ellipse: None,
                cubic_bezier: None,
                circle: None,
                segments: Vec::new(),
                placement: Some(SketchPlacement::global_xy()),
            })
            .expect("wire");
        let body = kernel
            .extrude(
                wire,
                ExtrudeExtent::ThroughAll,
                ExtrudeOperation::NewBody,
                None,
                ProfilePlane::Xy.map_point(0.0, 1.0),
            )
            .expect("body");
        kernel.bounding_box(&body).expect("bounds");
        assert_eq!(kernel.call_count(), 3);
    }
}
