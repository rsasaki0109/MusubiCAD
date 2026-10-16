#pragma once
#include "rust/cxx.h"
namespace musubicad {
rust::Vec<std::uint8_t> tapered_rib_stock(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> frame);
rust::Vec<std::uint8_t> cylindrical_frustum(rust::Slice<const double> frame);
rust::Vec<double> cylindrical_stock(rust::Slice<const std::uint8_t> brep);
rust::Vec<std::uint64_t> cylindrical_rim_ids(rust::Slice<const std::uint8_t> brep, std::uint64_t start_face, std::uint64_t end_face, std::uint64_t side_face);
rust::Vec<double> rectangular_stock_faces(rust::Slice<const std::uint8_t> brep);
rust::Vec<double> tight_bounds(rust::Slice<const std::uint8_t> brep);
rust::Vec<std::uint8_t> cubic_prism(rust::Slice<const double> controls, rust::Slice<const double> delta);
rust::Vec<std::uint8_t> ellipse_prism(rust::Slice<const double> frame, rust::Slice<const double> delta);
rust::Vec<std::uint8_t> place_faceted_solid(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> placement, double expected_volume);
rust::Vec<std::uint8_t> triangle_solid(rust::Slice<const double> vertices, rust::Slice<const std::uint32_t> indices, double expected_volume);
rust::Vec<double> section_edges(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> plane);
rust::Vec<double> circular_edge_geometry(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> endpoints);
bool is_straight_edge(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> endpoints);
rust::Vec<std::uint8_t> variable_fillet(rust::Slice<const std::uint8_t> brep,
                                      rust::Slice<const double> endpoints,
                                      double start_radius, double end_radius);
}
