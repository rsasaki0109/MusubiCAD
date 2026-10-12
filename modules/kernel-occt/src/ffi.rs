//! Private native OCCT operation bridge; opaque BREP bytes never leave the kernel.
#[cfg(feature = "occt")]
#[allow(unsafe_code)] // cxx-generated FFI only; see ADR-038.
#[cxx::bridge(namespace = "musubicad")]
pub(crate) mod native {
    unsafe extern "C++" {
        include!("cpp/variable_fillet.h");
        fn ellipse_prism(frame: &[f64], delta: &[f64]) -> Result<Vec<u8>>;
        fn triangle_solid(
            vertices: &[f64],
            indices: &[u32],
            expected_volume: f64,
        ) -> Result<Vec<u8>>;
        fn section_edges(brep: &[u8], plane: &[f64]) -> Result<Vec<f64>>;
        fn circular_edge_geometry(brep: &[u8], endpoints: &[f64]) -> Result<Vec<f64>>;
        fn is_straight_edge(brep: &[u8], endpoints: &[f64]) -> Result<bool>;
        fn variable_fillet(
            brep: &[u8],
            endpoints: &[f64],
            start_radius: f64,
            end_radius: f64,
        ) -> Result<Vec<u8>>;
    }
}
