#pragma once
#include "rust/cxx.h"
namespace musubicad {
rust::Vec<std::uint8_t> triangle_solid(rust::Slice<const double> vertices, rust::Slice<const std::uint32_t> indices, double expected_volume);
rust::Vec<double> section_edges(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> plane);
rust::Vec<double> circular_edge_geometry(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> endpoints);
bool is_straight_edge(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> endpoints);
rust::Vec<std::uint8_t> variable_fillet(rust::Slice<const std::uint8_t> brep,
                                      rust::Slice<const double> endpoints,
                                      double start_radius, double end_radius);
}
