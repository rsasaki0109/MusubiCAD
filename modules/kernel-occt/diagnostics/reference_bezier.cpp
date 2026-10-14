// ADR-076 isolated SDK surface diagnostic; coordinates explicitly in metres.
#include <Geom_BezierSurface.hxx>
#include <NCollection_Array2.hxx>
#include <Standard_Failure.hxx>
#include <Standard_Version.hxx>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <stdexcept>

#if OCC_VERSION_HEX != 0x080000
#error This diagnostic requires pinned OCCT 8.0.0
#endif

namespace {
void vector(std::ostream &out, double x, double y, double z) {
  if (!std::isfinite(x) || !std::isfinite(y) || !std::isfinite(z))
    throw std::runtime_error("nonfinite native surface measurement");
  out << '[' << x << ',' << y << ',' << z << ']';
}
}
int main() {
  try {
    int count;
    if (!(std::cin >> count) || count != 40)
      throw std::runtime_error("expected all 40 reference fixtures");
    std::ostringstream out;
    out << std::setprecision(17) << "{\"format_version\":1,\"occt_version\":\""
        << OCC_VERSION_COMPLETE << "\",\"cases\":[";
    for (int c = 0; c < count; ++c) {
      if (c) out << ',';
      NCollection_Array2<gp_Pnt> poles(1, 4, 1, 4);
      for (int i = 1; i <= 4; ++i)
        for (int j = 1; j <= 4; ++j) {
          double x, y, z;
          if (!(std::cin >> x >> y >> z) ||
              !std::isfinite(x) || !std::isfinite(y) || !std::isfinite(z))
            throw std::runtime_error("invalid metre-valued control net");
          poles(i, j) = gp_Pnt(x, y, z);
        }
      Geom_BezierSurface surface(poles);
      if (surface.UDegree() != 3 || surface.VDegree() != 3)
        throw std::runtime_error("wrong native surface degree");
      out << "{\"u_degree\":3,\"v_degree\":3,\"poles_m\":[";
      for (int i = 1; i <= 4; ++i) {
        if (i > 1) out << ',';
        out << '[';
        for (int j = 1; j <= 4; ++j) {
          if (j > 1) out << ',';
          const auto p = surface.Pole(i, j);
          vector(out, p.X(), p.Y(), p.Z());
        }
        out << ']';
      }
      out << "],\"samples\":[";
      for (int i = 0; i <= 4; ++i)
        for (int j = 0; j <= 4; ++j) {
          if (i || j) out << ',';
          const double u = i / 4., v = j / 4.;
          gp_Pnt p;
          gp_Vec du, dv;
          surface.D1(u, v, p, du, dv);
          out << "{\"u\":" << u << ",\"v\":" << v << ",\"point_m\":";
          vector(out, p.X(), p.Y(), p.Z());
          out << ",\"du_m\":";
          vector(out, du.X(), du.Y(), du.Z());
          out << ",\"dv_m\":";
          vector(out, dv.X(), dv.Y(), dv.Z());
          out << '}';
        }
      out << "]}";
    }
    std::string trailing;
    if (std::cin >> trailing)
      throw std::runtime_error("unexpected trailing fixture input");
    out << "]}\n";
    std::cout << out.str();
  } catch (const Standard_Failure &e) {
    std::cerr << "Native Bezier diagnostic failed: " << e.what() << '\n';
    return 1;
  } catch (const std::exception &e) {
    std::cerr << "Bezier diagnostic failed: " << e.what() << '\n';
    return 1;
  }
}
