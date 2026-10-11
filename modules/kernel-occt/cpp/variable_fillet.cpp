#include "cpp/variable_fillet.h"
#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <BRep_Builder.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <NCollection_Array1.hxx>
#include <gp_Pnt2d.hxx>
#include <TopExp.hxx>
#include <NCollection_IndexedMap.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <Standard_Failure.hxx>
#include <sstream>
#include <stdexcept>

namespace musubicad {
bool is_straight_edge(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> endpoints) {
    try {
        if (endpoints.size() != 6) throw std::runtime_error("invalid edge endpoints");
        std::istringstream stream(std::string(reinterpret_cast<const char*>(brep.data()), brep.size()));
        TopoDS_Shape shape; BRep_Builder builder; BRepTools::Read(shape, stream, builder);
        if (shape.IsNull()) throw std::runtime_error("edge query BREP read failed");
        gp_Pnt first(endpoints[0], endpoints[1], endpoints[2]);
        gp_Pnt last(endpoints[3], endpoints[4], endpoints[5]);
        NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> edges; TopExp::MapShapes(shape, TopAbs_EDGE, edges);
        int matches = 0; bool straight = false;
        for (int i = 1; i <= edges.Extent(); ++i) {
            auto edge = TopoDS::Edge(edges(i));
            auto v1 = TopExp::FirstVertex(edge, true), v2 = TopExp::LastVertex(edge, true);
            if (v1.IsNull() || v2.IsNull()) continue;
            auto p1 = BRep_Tool::Pnt(v1), p2 = BRep_Tool::Pnt(v2);
            if ((p1.Distance(first) <= 1e-8 && p2.Distance(last) <= 1e-8) ||
                (p1.Distance(last) <= 1e-8 && p2.Distance(first) <= 1e-8)) {
                ++matches; straight = BRepAdaptor_Curve(edge).GetType() == GeomAbs_Line;
            }
        }
        if (matches != 1) throw std::runtime_error("straight edge query is missing or ambiguous");
        return straight;
    } catch (const Standard_Failure& failure) { throw std::runtime_error(failure.what()); }
}

rust::Vec<double> circular_edge_geometry(rust::Slice<const std::uint8_t> brep,
                                        rust::Slice<const double> endpoints) {
    try {
        if (endpoints.size() != 6) throw std::runtime_error("invalid circular edge endpoints");
        std::istringstream stream(std::string(reinterpret_cast<const char*>(brep.data()), brep.size()));
        TopoDS_Shape shape; BRep_Builder builder; BRepTools::Read(shape, stream, builder);
        if (shape.IsNull()) throw std::runtime_error("circle query BREP read failed");
        gp_Pnt first(endpoints[0], endpoints[1], endpoints[2]), last(endpoints[3], endpoints[4], endpoints[5]);
        NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> edges; TopExp::MapShapes(shape, TopAbs_EDGE, edges);
        TopoDS_Edge selected; int matches = 0;
        for (int i = 1; i <= edges.Extent(); ++i) {
            auto edge = TopoDS::Edge(edges(i));
            auto v1 = TopExp::FirstVertex(edge, true), v2 = TopExp::LastVertex(edge, true);
            if (v1.IsNull() || v2.IsNull()) continue;
            auto p1 = BRep_Tool::Pnt(v1), p2 = BRep_Tool::Pnt(v2);
            if ((p1.Distance(first) <= 1e-8 && p2.Distance(last) <= 1e-8) ||
                (p1.Distance(last) <= 1e-8 && p2.Distance(first) <= 1e-8)) { selected = edge; ++matches; }
        }
        if (matches != 1) throw std::runtime_error("circle query is missing or ambiguous");
        BRepAdaptor_Curve curve(selected);
        if (curve.GetType() != GeomAbs_Circle ||
            std::abs((curve.LastParameter() - curve.FirstParameter()) - 2.0 * std::acos(-1.0)) > 1e-9)
            throw std::runtime_error("radial dimension requires an exact complete circle");
        auto circle = curve.Circle(); auto center = circle.Location(); auto normal = circle.Axis().Direction();
        rust::Vec<double> result;
        for (double value : {center.X(), center.Y(), center.Z(), normal.X(), normal.Y(), normal.Z(), circle.Radius()}) result.push_back(value);
        return result;
    } catch (const Standard_Failure& failure) { throw std::runtime_error(failure.what()); }
}

rust::Vec<std::uint8_t> variable_fillet(rust::Slice<const std::uint8_t> brep,
                                      rust::Slice<const double> endpoints,
                                      double start_radius, double end_radius) {
    try {
        if (endpoints.size() != 6) throw std::runtime_error("invalid edge endpoints");
        std::string input(reinterpret_cast<const char*>(brep.data()), brep.size());
        std::istringstream stream(input);
        TopoDS_Shape shape;
        BRep_Builder builder;
        BRepTools::Read(shape, stream, builder);
        if (shape.IsNull())
            throw std::runtime_error("variable fillet BREP read failed");
        gp_Pnt first(endpoints[0], endpoints[1], endpoints[2]);
        gp_Pnt last(endpoints[3], endpoints[4], endpoints[5]);
        TopoDS_Edge selected;
        int matches = 0;
        for (TopExp_Explorer it(shape, TopAbs_EDGE); it.More(); it.Next()) {
            auto edge = TopoDS::Edge(it.Current());
            BRepAdaptor_Curve curve(edge);
            if (curve.GetType() != GeomAbs_Line) continue;
            // Endpoint matching is independent of the edge's orientation.
            auto v1 = TopExp::FirstVertex(edge, true);
            auto v2 = TopExp::LastVertex(edge, true);
            if (v1.IsNull() || v2.IsNull()) continue;
            auto p1 = BRep_Tool::Pnt(v1);
            auto p2 = BRep_Tool::Pnt(v2);
            bool forward = p1.Distance(first) <= 1e-5 && p2.Distance(last) <= 1e-5;
            bool backward = p1.Distance(last) <= 1e-5 && p2.Distance(first) <= 1e-5;
            if (!forward && !backward) continue;
            // An edge occurs in both adjacent face wires; count unique TShapes.
            if (!selected.IsNull() && selected.IsSame(edge)) continue;
            selected = edge; ++matches;
        }
        if (matches != 1) throw std::runtime_error("variable fillet requires one unique straight edge");
        BRepFilletAPI_MakeFillet fillet(shape);
        fillet.SetParams(1e-6, 1e-6, 1e-8, 1e-6, 1e-8, 1e-6);
        fillet.Add(selected);
        if (fillet.NbContours() != 1 || fillet.NbEdges(1) != 1)
            throw std::runtime_error("variable fillet rejects tangent edge chains");
        bool spine_reversed = BRep_Tool::Pnt(fillet.FirstVertex(1)).Distance(first) > 1e-5;
        // ADR-038: fixed 64 intervals approximate the explicit linear law
        // through OCCT's point/radius API. This avoids the broken Law_Function
        // overload and limits zero-slope interpolation to the two end intervals.
        constexpr int intervals = 64;
        NCollection_Array1<gp_Pnt2d> samples(1, intervals + 1);
        for (int i = 0; i <= intervals; ++i) {
            double t = static_cast<double>(i) / intervals;
            double position = spine_reversed ? 1.0 - t : t;
            samples(i + 1) = gp_Pnt2d(t, start_radius + position * (end_radius - start_radius));
        }
        fillet.SetRadius(samples, 1, 1);
        fillet.Build();
        if (!fillet.IsDone() || fillet.Shape().IsNull())
            throw std::runtime_error("OCCT variable fillet builder failed");
        if (!BRepCheck_Analyzer(fillet.Shape()).IsValid())
            throw std::runtime_error("OCCT variable fillet produced invalid topology");
        std::ostringstream output;
        BRepTools::Write(fillet.Shape(), output);
        auto text = output.str();
        rust::Vec<std::uint8_t> result;
        result.reserve(text.size());
        for (unsigned char byte : text) result.push_back(byte);
        return result;
    } catch (const Standard_Failure& failure) {
        throw std::runtime_error(failure.what());
    }
}
}
