#pragma once
#include "rust/cxx.h"
namespace musubicad {
rust::Vec<double> circular_edge_geometry(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> endpoints);
bool is_straight_edge(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> endpoints);
rust::Vec<std::uint8_t> variable_fillet(rust::Slice<const std::uint8_t> brep,
                                      rust::Slice<const double> endpoints,
                                      double start_radius, double end_radius);
}
