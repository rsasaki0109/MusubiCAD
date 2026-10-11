#pragma once
#include "rust/cxx.h"
namespace musubicad {
rust::Vec<std::uint8_t> variable_fillet(rust::Slice<const std::uint8_t> brep,
                                      rust::Slice<const double> endpoints,
                                      double start_radius, double end_radius);
}
