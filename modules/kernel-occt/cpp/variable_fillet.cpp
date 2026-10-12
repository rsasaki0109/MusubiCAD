#include "cpp/variable_fillet.h"
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepGProp.hxx>
#include <BOPAlgo_ArgumentAnalyzer.hxx>
#include <GProp_GProps.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <BRep_Builder.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
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
#include <BRepAlgoAPI_Section.hxx>
#include <gp_Pln.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <gp_Elips.hxx>
#include <gp_Ax2.hxx>
#include <cmath>
#include <sstream>
#include <stdexcept>

namespace musubicad {
rust::Vec<std::uint8_t> ellipse_prism(rust::Slice<const double> frame, rust::Slice<const double> delta) {
    try {
        if (frame.size()!=9 || delta.size()!=3) throw std::runtime_error("invalid ellipse prism input");
        for (auto v:frame) if (!std::isfinite(v)) throw std::runtime_error("non-finite ellipse frame");
        for (auto v:delta) if (!std::isfinite(v)) throw std::runtime_error("non-finite ellipse extrusion");
        gp_Vec major(frame[3],frame[4],frame[5]),minor(frame[6],frame[7],frame[8]);
        auto a=major.Magnitude(),b=minor.Magnitude();
        if (b<=1e-9 || a<b || a>1e6 || std::abs(major.Dot(minor)/(a*b))>1e-9) throw std::runtime_error("invalid ellipse axes");
        gp_Dir normal(major.Crossed(minor));
        gp_Elips ellipse(gp_Ax2(gp_Pnt(frame[0],frame[1],frame[2]),normal,gp_Dir(major)),a,b);
        BRepBuilderAPI_MakeEdge edge(ellipse); if (!edge.IsDone()) throw std::runtime_error("ellipse edge failed");
        BRepBuilderAPI_MakeWire wire(edge.Edge()); if (!wire.IsDone()) throw std::runtime_error("ellipse wire failed");
        BRepBuilderAPI_MakeFace face(wire.Wire()); if (!face.IsDone()) throw std::runtime_error("ellipse face failed");
        gp_Vec direction(delta[0],delta[1],delta[2]);
        auto height=std::abs(direction.Dot(gp_Vec(normal)));
        if (height<=1e-9 || std::abs(height-direction.Magnitude())>1e-9*direction.Magnitude()) throw std::runtime_error("ellipse extrusion must be normal");
        BRepPrimAPI_MakePrism prism(face.Face(),direction); if (!prism.IsDone()) throw std::runtime_error("ellipse prism failed");
        auto solid=prism.Shape();if (!BRepCheck_Analyzer(solid,true).IsValid()) throw std::runtime_error("invalid ellipse prism");
        GProp_GProps mass;BRepGProp::VolumeProperties(solid,mass);auto expected=std::acos(-1.)*a*b*height;
        if (!std::isfinite(mass.Mass()) || mass.Mass()<=0 || std::abs(mass.Mass()-expected)>std::max(1e-18,expected*1e-8)) throw std::runtime_error("ellipse prism volume mismatch");
        std::ostringstream stream;BRepTools::Write(solid,stream);auto bytes=stream.str();rust::Vec<std::uint8_t> result;
        for (auto byte:bytes) {result.push_back(static_cast<std::uint8_t>(byte));}
        return result;
    } catch (const Standard_Failure& failure) {throw std::runtime_error(failure.what());}
}

rust::Vec<std::uint8_t> triangle_solid(rust::Slice<const double> vertices, rust::Slice<const std::uint32_t> indices, double expected_volume) {
    try {
        if (vertices.empty() || vertices.size()%3 || indices.size()<12 || indices.size()%3 || indices.size()>30000 || !std::isfinite(expected_volume) || expected_volume<=0)
            throw std::runtime_error("invalid triangle solid input");
        BRepBuilderAPI_Sewing sewing(1e-8);
        for (std::size_t t=0; t<indices.size(); t+=3) {
            BRepBuilderAPI_MakePolygon polygon;
            for (std::size_t j=0; j<3; ++j) {
                auto i=static_cast<std::size_t>(indices[t+j])*3;
                if (i+2>=vertices.size()) throw std::runtime_error("triangle vertex index out of bounds");
                for (std::size_t axis=0; axis<3; ++axis) if (!std::isfinite(vertices[i+axis])) throw std::runtime_error("non-finite triangle vertex");
                polygon.Add(gp_Pnt(vertices[i],vertices[i+1],vertices[i+2]));
            }
            polygon.Close();
            if (!polygon.IsDone()) throw std::runtime_error("triangle wire failed");
            BRepBuilderAPI_MakeFace face(polygon.Wire());
            if (!face.IsDone()) throw std::runtime_error("triangle face failed");
            sewing.Add(face.Face());
        }
        sewing.Perform();
        auto shell=sewing.SewedShape();
        if (shell.IsNull() || shell.ShapeType()!=TopAbs_SHELL) throw std::runtime_error("triangles must sew to one shell");
        BRepBuilderAPI_MakeSolid builder(TopoDS::Shell(shell));
        if (!builder.IsDone()) throw std::runtime_error("triangle solid failed");
        auto solid=builder.Solid();
        if (!BRepCheck_Analyzer(solid,true).IsValid()) throw std::runtime_error("invalid faceted B-rep");
        BOPAlgo_ArgumentAnalyzer check;
        check.SetShape1(solid); check.SelfInterMode()=true; check.StopOnFirstFaulty()=true; check.Perform();
        if (check.HasFaulty()) throw std::runtime_error("self-intersecting faceted solid");
        GProp_GProps mass; BRepGProp::VolumeProperties(solid,mass);
        if (!std::isfinite(mass.Mass()) || mass.Mass()<=0 || std::abs(mass.Mass()-expected_volume)>std::max(1e-18,expected_volume*1e-8))
            throw std::runtime_error("faceted solid volume differs from mesh volume");
        std::ostringstream stream; BRepTools::Write(solid,stream);
        auto bytes=stream.str(); rust::Vec<std::uint8_t> result;
        for (auto byte:bytes) result.push_back(static_cast<std::uint8_t>(byte));
        return result;
    } catch (const Standard_Failure& failure) { throw std::runtime_error(failure.what()); }
}

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

rust::Vec<double> section_edges(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> plane) {
    try {
        if (plane.size()!=6) throw std::runtime_error("invalid section plane");
        std::istringstream stream(std::string(reinterpret_cast<const char*>(brep.data()),brep.size()));
        TopoDS_Shape shape; BRep_Builder builder; BRepTools::Read(shape,stream,builder);
        if (shape.IsNull()) throw std::runtime_error("section BREP read failed");
        gp_Pln cutting(gp_Pnt(plane[0],plane[1],plane[2]),gp_Dir(plane[3],plane[4],plane[5]));
        for (TopExp_Explorer it(shape,TopAbs_FACE);it.More();it.Next()) {
            BRepAdaptor_Surface surface(TopoDS::Face(it.Current()));
            if (surface.GetType()==GeomAbs_Plane) {
                auto face_plane=surface.Plane();
                if (std::abs(face_plane.Axis().Direction().Dot(cutting.Axis().Direction()))>=1.0-1e-9 &&
                    face_plane.Distance(cutting.Location())<=1e-8)
                    throw std::runtime_error("section cutting plane is coincident with a body face");
            }
        }
        BRepAlgoAPI_Section section(shape,cutting,false); section.Approximation(false); section.Build();
        if (!section.IsDone()) throw std::runtime_error("OCCT section failed");
        NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> edges; TopExp::MapShapes(section.Shape(),TopAbs_EDGE,edges);
        if (edges.Extent()>4096) throw std::runtime_error("section exceeds edge limit");
        rust::Vec<double> result;
        for (int i=1;i<=edges.Extent();++i) {
            auto edge=TopoDS::Edge(edges(i));BRepAdaptor_Curve curve(edge);
            if (curve.GetType()!=GeomAbs_Line) throw std::runtime_error("section contains unsupported curved edges");
            auto a=curve.Value(curve.FirstParameter()),b=curve.Value(curve.LastParameter());
            for (double value:{a.X(),a.Y(),a.Z(),b.X(),b.Y(),b.Z()}) result.push_back(value);
        }
        return result;
    } catch (const Standard_Failure& failure) {throw std::runtime_error(failure.what());}
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
