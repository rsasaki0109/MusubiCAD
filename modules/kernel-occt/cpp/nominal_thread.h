#pragma once
#include "rust/cxx.h"
namespace musubicad {
rust::Vec<std::uint8_t> nominal_metric_thread(rust::Slice<const double> frame,double major,double pitch,std::uint32_t turns,bool left);
rust::Vec<double> nominal_thread_properties(rust::Slice<const std::uint8_t> brep);
}
