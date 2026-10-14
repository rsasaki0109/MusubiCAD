// ADR-074 diagnostic executable only; never linked into the product.
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepTools.hxx>
#include <GProp_GProps.hxx>
#include <Law_Function.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_IndexedMap.hxx>
#include <Standard_Failure.hxx>
#include <Standard_Version.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <algorithm>
#include <cmath>
#include <gp.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Trsf.hxx>
#include <iomanip>
#include <iostream>
#include <stdexcept>

#if OCC_VERSION_HEX != 0x080000
#error This investigation requires the pinned OCCT 8.0.0 SDK
#endif

namespace {
constexpr double radius_mm = 10, height_mm = 20;
const double pi = std::acos(-1.0);
struct Profile {
  const char *name;
  double amplitude_mm;
  int intervals;
  bool tight;
  bool c2;
  bool rotate;
};
struct Measurements {
  double volume_m3 = 0, volume_refinement_delta_m3 = 0;
  double top_error_m = 0, side_error_m = 0, support_error_m = 0, radius_error_m = 0,
         law_error_m = 0;
  int solids = 0, top_points = 0, side_points = 0, interior_points = 0, law_points = 0;
};
void require(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}
void maximum(double &result, double residual) {
  require(std::isfinite(residual), "nonfinite native measurement");
  result = std::max(result, std::abs(residual) * 1e-3);
}
TopoDS_Edge top_rim(const TopoDS_Shape &stock) {
  NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> edges;
  TopExp::MapShapes(stock, TopAbs_EDGE, edges);
  TopoDS_Edge rim;
  int matches = 0;
  for (int i = 1; i <= edges.Extent(); ++i) {
    auto edge = TopoDS::Edge(edges(i));
    BRepAdaptor_Curve curve(edge);
    if (curve.GetType() != GeomAbs_Circle)
      continue;
    const auto circle = curve.Circle();
    if (std::abs(circle.Location().Z() - height_mm) > 1e-8)
      continue;
    require(std::abs(circle.Radius() - radius_mm) < 1e-8, "wrong rim radius");
    require(std::hypot(circle.Location().X(), circle.Location().Y()) < 1e-8, "wrong rim centre");
    require(std::abs(curve.LastParameter() - curve.FirstParameter() - 2 * pi) < 1e-10,
            "partial rim");
    rim = edge;
    ++matches;
  }
  require(matches == 1, "missing or ambiguous full top rim");
  return rim;
}
Measurements measure(const Profile &profile) {
  const auto stock = BRepPrimAPI_MakeCylinder(radius_mm, height_mm).Shape();
  const auto rim = top_rim(stock);
  BRepFilletAPI_MakeFillet builder(stock, ChFi3d_Rational);
  const double eps = profile.tight ? 1e-9 : 1e-6;
  builder.SetParams(eps, eps, eps / 100, eps, eps / 100, eps);
  if (profile.c2)
    builder.SetContinuity(GeomAbs_C2, 1e-8);
  builder.Add(rim);
  NCollection_Array1<gp_Pnt2d> samples(1, profile.intervals + 1);
  for (int i = 0; i <= profile.intervals; i++)
    samples(i + 1) = gp_Pnt2d(double(i) / profile.intervals,
                              1 + profile.amplitude_mm * std::cos(2 * pi * i / profile.intervals));
  builder.SetRadius(samples, 1, 1);
  builder.Build();
  require(builder.IsDone() && !builder.Shape().IsNull(), "native builder failed");
  TopoDS_Shape shape = builder.Shape();
  if (profile.rotate) {
    gp_Trsf rotation;
    rotation.SetRotation(gp::OZ(), pi / 2);
    shape = BRepBuilderAPI_Transform(shape, rotation, true).Shape();
  }
  require(BRepCheck_Analyzer(shape).IsValid(), "invalid native BREP");
  Measurements out;
  for (TopExp_Explorer it(shape, TopAbs_SOLID); it.More(); it.Next())
    ++out.solids;
  require(out.solids == 1, "native output is not one solid");
  GProp_GProps coarse, fine;
  BRepGProp::VolumeProperties(shape, coarse, 1e-9);
  BRepGProp::VolumeProperties(shape, fine, 1e-11);
  out.volume_m3 = fine.Mass() * 1e-9;
  out.volume_refinement_delta_m3 = std::abs(fine.Mass() - coarse.Mass()) * 1e-9;
  require(std::isfinite(out.volume_m3) && out.volume_m3 > 0, "invalid native volume");
  if (profile.amplitude_mm > 0) {
    const auto law = builder.GetLaw(1, rim);
    require(!law.IsNull(), "missing radius law");
    double first, last;
    law->Bounds(first, last);
    require(std::abs(first) < 1e-8 && std::abs(last - 2 * pi * radius_mm) < 1e-8,
            "unexpected law coordinates");
    for (int i = 0; i <= 512; i++) {
      const double u = double(i) / 512;
      maximum(out.law_error_m, law->Value(first + u * (last - first)) -
                                   (1 + profile.amplitude_mm * std::cos(2 * pi * u)));
      ++out.law_points;
    }
  }
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    const auto face = TopoDS::Face(it.Current());
    BRepAdaptor_Surface surface(face);
    const bool top = surface.GetType() == GeomAbs_Plane &&
                     std::abs(surface.Plane().Location().Z() - height_mm) < 1e-7;
    const bool side = surface.GetType() == GeomAbs_Cylinder;
    if (top || side) {
      NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> edges;
      TopExp::MapShapes(face, TopAbs_EDGE, edges);
      for (int e = 1; e <= edges.Extent(); e++) {
        BRepAdaptor_Curve curve(TopoDS::Edge(edges(e)));
        if (side && curve.GetType() == GeomAbs_Line)
          continue;
        for (int i = 0; i <= 256; i++) {
          const auto point = curve.Value(
              curve.FirstParameter() + (curve.LastParameter() - curve.FirstParameter()) * i / 256.);
          const double rho = std::hypot(point.X(), point.Y());
          const double r = 1 + profile.amplitude_mm * std::cos(std::atan2(point.Y(), point.X()));
          if (top) {
            maximum(out.top_error_m, radius_mm - rho - r);
            ++out.top_points;
          }
          if (side && point.Z() > height_mm - 3) {
            require(std::abs(rho - radius_mm) < 1e-6, "contact is off cylinder");
            maximum(out.side_error_m, height_mm - point.Z() - r);
            ++out.side_points;
          }
        }
      }
      continue;
    }
    if (surface.GetType() == GeomAbs_Plane)
      continue;
    double u0, u1, v0, v1;
    BRepTools::UVBounds(face, u0, u1, v0, v1);
    for (int i = 1; i < 64; i++)
      for (int j = 1; j < 16; j++) {
        const double u = u0 + (u1 - u0) * i / 64., v = v0 + (v1 - v0) * j / 16.;
        if (BRepClass_FaceClassifier(face, gp_Pnt2d(u, v), 1e-8).State() != TopAbs_IN)
          continue;
        gp_Pnt point;
        gp_Vec du, dv;
        surface.D1(u, v, point, du, dv);
        auto normal = du.Crossed(dv);
        require(normal.Magnitude() > 1e-12, "degenerate interior normal");
        normal.Normalize();
        require(face.Orientation() == TopAbs_FORWARD || face.Orientation() == TopAbs_REVERSED,
                "unsupported face orientation");
        if (face.Orientation() == TopAbs_REVERSED)
          normal.Reverse();
        if (normal.Z() <= 0.15 || normal.Z() >= 0.9)
          continue;
        const double r = (height_mm - point.Z()) / (1 - normal.Z());
        require(std::isfinite(r) && r > 0, "invalid inferred sphere");
        const auto centre = point.Translated(-normal * r);
        maximum(out.support_error_m, std::hypot(centre.X(), centre.Y()) - (radius_mm - r));
        maximum(out.radius_error_m,
                r - (1 + profile.amplitude_mm * std::cos(std::atan2(centre.Y(), centre.X()))));
        ++out.interior_points;
      }
  }
  require(out.top_points >= 257 && out.side_points >= 257 && out.interior_points >= 100,
          "missing physical witnesses");
  return out;
}
void write(const Profile &p, const Measurements &m) {
  std::cout << "{\"name\":\"" << p.name << "\",\"brep_valid\":true,\"solids\":" << m.solids
            << ",\"native_volume_m3\":" << m.volume_m3
            << ",\"volume_refinement_delta_m3\":" << m.volume_refinement_delta_m3
            << ",\"top_points\":" << m.top_points << ",\"side_points\":" << m.side_points
            << ",\"interior_points\":" << m.interior_points << ",\"law_points\":" << m.law_points
            << ",\"max_sampled_top_error_m\":" << m.top_error_m
            << ",\"max_sampled_side_error_m\":" << m.side_error_m
            << ",\"max_sampled_support_error_m\":" << m.support_error_m
            << ",\"max_sampled_radius_error_m\":" << m.radius_error_m
            << ",\"max_sampled_law_error_m\":" << m.law_error_m << "}";
}
} // namespace
int main() {
  try {
    const Profile profiles[] = {
        {"constant", 0, 64, false, false, false}, {"baseline", .5, 64, false, false, false},
        {"dense", .5, 256, false, false, false},  {"tight", .5, 64, true, false, false},
        {"c2", .5, 64, false, true, false},       {"wrong_phase", .5, 64, false, false, true}};
    // Buffer measurements before emitting JSON: failures cannot look complete.
    Measurements results[6];
    for (int i = 0; i < 6; i++)
      results[i] = measure(profiles[i]);
    std::cout << std::setprecision(17) << "{\"format_version\":1,\"occt_version\":\""
              << OCC_VERSION_COMPLETE << "\",\"radius_m\":0.01,\"height_m\":0.02,\"profiles\":[";
    for (int i = 0; i < 6; i++) {
      if (i)
        std::cout << ",";
      write(profiles[i], results[i]);
    }
    std::cout << "]}\n";
  } catch (const Standard_Failure &e) {
    std::cerr << "OCCT diagnostic failed: " << e.what() << "\n";
    return 1;
  } catch (const std::exception &e) {
    std::cerr << "Diagnostic failed: " << e.what() << "\n";
    return 1;
  }
}
