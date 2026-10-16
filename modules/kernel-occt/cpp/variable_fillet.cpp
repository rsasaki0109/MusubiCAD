#include "cpp/variable_fillet.h"
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <gp_Trsf.hxx>
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
#include <BRepBndLib.hxx>
#include <Bnd_Box.hxx>
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
#include <BRepAlgoAPI_Fuse.hxx>
#include <NCollection_List.hxx>
#include <gp_Pln.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <gp_Elips.hxx>
#include <Geom_BezierCurve.hxx>
#include <gp_Ax2.hxx>
#include <algorithm>
#include <vector>
#include <array>
#include <cmath>
#include <sstream>
#include <stdexcept>
#include <limits>

namespace musubicad {

// ADR-066: exact trapezoidal prism and connected union, no synthetic stock IDs.
rust::Vec<std::uint8_t> tapered_rib_stock(rust::Slice<const std::uint8_t> brep,rust::Slice<const double> p) {
    try {
        if(p.size()!=16) throw std::runtime_error("invalid tapered rib frame");
        for(double v:p) if(!std::isfinite(v)) throw std::runtime_error("nonfinite tapered rib input");
        const gp_Pnt center(p[0],p[1],p[2]);
        const gp_Vec along(p[3],p[4],p[5]),width(p[6],p[7],p[8]),normal(p[9],p[10],p[11]);
        const double length=p[12],base=p[13],top=p[14],height=p[15];
        if(std::abs(along.Magnitude()-1)>1e-9||std::abs(width.Magnitude()-1)>1e-9||std::abs(normal.Magnitude()-1)>1e-9
            ||std::abs(along.Dot(width))>1e-9||std::abs(along.Dot(normal))>1e-9||std::abs(width.Dot(normal))>1e-9
            ||std::abs(along.Crossed(width).Dot(normal)-1)>1e-9||std::abs(base-top)<=1e-9)
            throw std::runtime_error("invalid tapered rib axes/taper");
        for(double v:{length,base,top,height}) if(v<=1e-9||v>1000) throw std::runtime_error("invalid tapered rib dimensions");
        std::istringstream source(std::string(reinterpret_cast<const char*>(brep.data()),brep.size()));
        TopoDS_Shape stock;BRep_Builder builder;BRepTools::Read(stock,source,builder);
        if(stock.IsNull()||!BRepCheck_Analyzer(stock,true).IsValid()) throw std::runtime_error("invalid tapered rib stock");
        // Repeat the actual cuboid type/topology proof at the private boundary.
        (void)rectangular_stock_faces(brep);
        BRepBuilderAPI_MakePolygon polygon;
        const auto start=center.Translated(along*(-length/2));
        polygon.Add(start.Translated(width*(-base/2)));
        polygon.Add(start.Translated(width*(base/2)));
        polygon.Add(start.Translated(width*(top/2)+normal*height));
        polygon.Add(start.Translated(width*(-top/2)+normal*height));polygon.Close();
        if(!polygon.IsDone()) throw std::runtime_error("invalid tapered rib profile");
        BRepBuilderAPI_MakeFace profile(polygon.Wire(),true);
        if(!profile.IsDone()) throw std::runtime_error("invalid tapered rib plane");
        BRepPrimAPI_MakePrism prism(profile.Face(),along*length);prism.Build();
        if(!prism.IsDone()||!BRepCheck_Analyzer(prism.Shape(),true).IsValid()) throw std::runtime_error("invalid tapered rib prism");
        NCollection_List<TopoDS_Shape> arguments,tools;arguments.Append(stock);tools.Append(prism.Shape());
        BRepAlgoAPI_Fuse fuse;fuse.SetArguments(arguments);fuse.SetTools(tools);fuse.SetRunParallel(false);fuse.Build();
        if(!fuse.IsDone()||!BRepCheck_Analyzer(fuse.Shape(),true).IsValid()) throw std::runtime_error("invalid tapered rib union");
        const auto result=fuse.Shape();
        NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> solids;TopExp::MapShapes(result,TopAbs_SOLID,solids);
        if(solids.Extent()!=1) throw std::runtime_error("tapered rib must form one connected solid");
        NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> faces,edges,vertices,stockVertices;
        TopExp::MapShapes(result,TopAbs_FACE,faces);TopExp::MapShapes(result,TopAbs_EDGE,edges);TopExp::MapShapes(result,TopAbs_VERTEX,vertices);
        TopExp::MapShapes(stock,TopAbs_VERTEX,stockVertices);
        if(faces.Extent()!=11||edges.Extent()!=24||vertices.Extent()!=16||stockVertices.Extent()!=8)
            throw std::runtime_error("unresolvable tapered rib union topology");
        for(int i=1;i<=faces.Extent();++i) if(BRepAdaptor_Surface(TopoDS::Face(faces(i))).GetType()!=GeomAbs_Plane)
            throw std::runtime_error("nonplanar tapered rib union face");
        std::vector<gp_Pnt> expectedVertices;
        for(int i=1;i<=stockVertices.Extent();++i) expectedVertices.push_back(BRep_Tool::Pnt(TopoDS::Vertex(stockVertices(i))));
        for(int end:{-1,1}) for(int side:{-1,1}) for(int layer:{0,1})
            expectedVertices.push_back(center.Translated(along*(end*length/2)+width*(side*(layer?top:base)/2)+normal*(layer?height:0)));
        for(const auto& expectedPoint:expectedVertices) {
            int matches=0;const double coordinateScale=std::max({1.0,std::abs(expectedPoint.X()),std::abs(expectedPoint.Y()),std::abs(expectedPoint.Z())});
            const double tolerance=1e-9+64*std::numeric_limits<double>::epsilon()*coordinateScale;
            for(int i=1;i<=vertices.Extent();++i) if(expectedPoint.Distance(BRep_Tool::Pnt(TopoDS::Vertex(vertices(i))))<=tolerance) ++matches;
            if(matches!=1) throw std::runtime_error("tapered rib union changed an exact source/contact vertex");
        }
        GProp_GProps before,after,oldArea,newArea;BRepGProp::VolumeProperties(stock,before);BRepGProp::VolumeProperties(result,after);
        BRepGProp::SurfaceProperties(stock,oldArea);BRepGProp::SurfaceProperties(result,newArea);
        const double added=length*height*(base+top)/2,expected=before.Mass()+added;
        const double area=oldArea.Mass()-length*base+length*top+height*(base+top)+2*length*std::hypot(height,(top-base)/2);
        if(expected<=0||!std::isfinite(after.Mass())||std::abs(after.Mass()-expected)>expected*1e-8
            ||!std::isfinite(newArea.Mass())||std::abs(newArea.Mass()-area)>area*1e-8)
            throw std::runtime_error("tapered rib union measurements disagree");
        std::ostringstream stream;BRepTools::Write(result,stream);const auto bytes=stream.str();rust::Vec<std::uint8_t> output;
        for(unsigned char b:bytes) {output.push_back(b);}return output;
    } catch(const Standard_Failure& e) {throw std::runtime_error(e.what());}
}
// Exception-catching constructor; no OCCT exception crosses an infallible FFI.
rust::Vec<std::uint8_t> cylindrical_frustum(rust::Slice<const double> p) {
    try {
        if(p.size()!=9) throw std::runtime_error("invalid frustum frame");
        for(double v:p) if(!std::isfinite(v)) throw std::runtime_error("nonfinite frustum input");
        gp_Vec axis(p[3],p[4],p[5]);
        if(std::abs(axis.Magnitude()-1)>1e-9||p[6]<=1e-9||p[7]<=1e-9||p[8]<=1e-9
            ||p[6]>1000||p[7]>1000||p[8]>1000||std::abs(p[6]-p[7])<=1e-9)
            throw std::runtime_error("invalid physical frustum dimensions");
        BRepPrimAPI_MakeCone maker(gp_Ax2(gp_Pnt(p[0],p[1],p[2]),gp_Dir(axis)),p[6],p[7],p[8]);
        maker.Build();
        if(!maker.IsDone()||!BRepCheck_Analyzer(maker.Shape(),true).IsValid()) throw std::runtime_error("invalid drafted frustum");
        std::ostringstream stream;BRepTools::Write(maker.Shape(),stream);auto bytes=stream.str();rust::Vec<std::uint8_t> result;
        for(unsigned char b:bytes) {result.push_back(b);}
        return result;
    } catch(const Standard_Failure& e) {throw std::runtime_error(e.what());}
}

// ADR-065: one exact full cylinder; private topology/type proof, never bounds.
rust::Vec<double> cylindrical_stock(rust::Slice<const std::uint8_t> brep) {
    try {
        constexpr double tol=1e-9,pi=3.14159265358979323846;
        std::istringstream stream(std::string(reinterpret_cast<const char*>(brep.data()),brep.size()));
        TopoDS_Shape shape; BRep_Builder builder; BRepTools::Read(shape,stream,builder);
        if(shape.IsNull()||!BRepCheck_Analyzer(shape,true).IsValid()) throw std::runtime_error("invalid cylinder BREP");
        NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> solids,faces,edges,vertices;
        TopExp::MapShapes(shape,TopAbs_SOLID,solids);TopExp::MapShapes(shape,TopAbs_FACE,faces);
        TopExp::MapShapes(shape,TopAbs_EDGE,edges);TopExp::MapShapes(shape,TopAbs_VERTEX,vertices);
        if(solids.Extent()!=1||faces.Extent()!=3||edges.Extent()!=3||vertices.Extent()!=2)
            throw std::runtime_error("requires one full cylinder with two caps and seam");
        int circles=0,lines=0;
        for(int i=1;i<=edges.Extent();++i) {
            BRepAdaptor_Curve curve(TopoDS::Edge(edges(i)));
            if(curve.GetType()==GeomAbs_Circle) {
                if(std::abs(curve.LastParameter()-curve.FirstParameter()-2*pi)>tol)
                    throw std::runtime_error("partial circular edge");
                ++circles;
            } else if(curve.GetType()==GeomAbs_Line) ++lines;
            else throw std::runtime_error("unsupported cylinder boundary");
        }
        if(circles!=2||lines!=1) throw std::runtime_error("invalid cylinder boundary types");
        int side=0,index=0;double radius=0,v0=0,v1=0;gp_Pnt axisOrigin;gp_Dir axis(0,0,1);
        std::vector<std::pair<int,gp_Pln>> caps;std::vector<gp_Vec> capNormals;
        for(TopExp_Explorer ex(shape,TopAbs_FACE);ex.More();ex.Next()) {
            ++index;auto face=TopoDS::Face(ex.Current());BRepAdaptor_Surface surface(face);
            int wires=0;for(TopExp_Explorer w(face,TopAbs_WIRE);w.More();w.Next())++wires;
            if(wires!=1) throw std::runtime_error("cylinder face has extra wires");
            if(surface.GetType()==GeomAbs_Cylinder) {
                if(side||(face.Orientation()!=TopAbs_FORWARD&&face.Orientation()!=TopAbs_REVERSED)) throw std::runtime_error("ambiguous cylinder side");
                side=index;auto cylinder=surface.Cylinder();radius=cylinder.Radius();axisOrigin=cylinder.Location();axis=cylinder.Axis().Direction();
                double u0,u1;BRepTools::UVBounds(face,u0,u1,v0,v1);
                if(std::abs(u1-u0-2*pi)>tol||!std::isfinite(radius)||radius<=tol||radius>1000
                    ||!std::isfinite(v0)||!std::isfinite(v1)||v1-v0<=tol||v1-v0>1000)
                    throw std::runtime_error("partial or empty cylinder surface");
                gp_Pnt point;gp_Vec du,dv;surface.D1((u0+u1)/2,(v0+v1)/2,point,du,dv);
                auto normal=du.Crossed(dv);if(face.Orientation()==TopAbs_REVERSED)normal.Reverse();
                const gp_Vec offset(axisOrigin,point),direction(axis);
                const auto radial=offset-direction*offset.Dot(direction);
                if(normal.Magnitude()<=0||radial.Magnitude()<=tol||normal.Normalized().Dot(radial.Normalized())<=0
                    ||normal.Normalized().Crossed(radial.Normalized()).Magnitude()>tol)
                    throw std::runtime_error("cylinder side normal is not outward");
            } else if(surface.GetType()==GeomAbs_Plane) {
                NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> fe;
                TopExp::MapShapes(face,TopAbs_EDGE,fe);
                if(fe.Extent()!=1) throw std::runtime_error("cap is not one disk");
                gp_Pnt point;gp_Vec du,dv;surface.D1(0,0,point,du,dv);auto normal=du.Crossed(dv);
                if(face.Orientation()==TopAbs_REVERSED)normal.Reverse();
                else if(face.Orientation()!=TopAbs_FORWARD)throw std::runtime_error("unsupported cap orientation");
                if(normal.Magnitude()<=0)throw std::runtime_error("undefined cap normal");
                caps.emplace_back(index,surface.Plane());capNormals.push_back(normal.Normalized());
            } else throw std::runtime_error("noncylindrical stock surface");
        }
        if(!side||caps.size()!=2||!std::isfinite(radius)||radius<=tol) throw std::runtime_error("invalid full cylinder");
        gp_Vec direction(axis);int start=0,end=0;
        for(std::size_t ci=0;ci<caps.size();++ci) {
            const auto& cap=caps[ci];gp_Vec normal=capNormals[ci];
            if(normal.Crossed(direction).Magnitude()>tol) throw std::runtime_error("oblique cylinder cap");
            const double z=direction.Dot(gp_Vec(axisOrigin,cap.second.Location()));
            if(std::abs(z-v0)<=tol&&!start) {
                if(normal.Dot(direction)>=0) {throw std::runtime_error("start cap is not outward");}
                start=cap.first;
            } else if(std::abs(z-v1)<=tol&&!end) {
                if(normal.Dot(direction)<=0) {throw std::runtime_error("end cap is not outward");}
                end=cap.first;
            }
            else throw std::runtime_error("cylinder cap height disagreement");
        }
        if(!start||!end) throw std::runtime_error("missing cylinder endpoints");
        for(int i=1;i<=edges.Extent();++i) {
            BRepAdaptor_Curve curve(TopoDS::Edge(edges(i)));
            if(curve.GetType()==GeomAbs_Circle) {
                auto circle=curve.Circle();gp_Vec offset(axisOrigin,circle.Location());
                const double z=direction.Dot(offset);
                if(std::abs(circle.Radius()-radius)>tol||gp_Vec(circle.Axis().Direction()).Crossed(direction).Magnitude()>tol
                    ||(offset-direction*z).Magnitude()>tol||(std::abs(z-v0)>tol&&std::abs(z-v1)>tol))
                    throw std::runtime_error("cap circle does not match cylinder");
            }
        }
        const double h=v1-v0;GProp_GProps props;BRepGProp::VolumeProperties(shape,props);
        const double expected=pi*radius*radius*h;
        if(!std::isfinite(props.Mass())||props.Mass()<=0||std::abs(props.Mass()-expected)>expected*1e-8)
            throw std::runtime_error("cylinder volume disagreement");
        auto origin=axisOrigin.Translated(direction*v0);rust::Vec<double> result;
        for(double value:{origin.X(),origin.Y(),origin.Z(),axis.X(),axis.Y(),axis.Z(),radius,h,
            double(start),double(end),double(side)}) {
            if(!std::isfinite(value)) {throw std::runtime_error("nonfinite cylinder witness");}
            result.push_back(value);
        }
        return result;
    } catch(const Standard_Failure& e) {throw std::runtime_error(e.what());}
}

// ADR-071: shared edge identity includes Location, not only a TShape pointer.
rust::Vec<std::uint64_t> cylindrical_rim_ids(rust::Slice<const std::uint8_t> brep,
        std::uint64_t start_face, std::uint64_t end_face, std::uint64_t side_face) {
    try {
        const auto stock=cylindrical_stock(brep);
        const auto a=static_cast<std::uint64_t>(stock[8]),b=static_cast<std::uint64_t>(stock[9]);
        if(side_face!=static_cast<std::uint64_t>(stock[10])
            ||!((start_face==a&&end_face==b)||(start_face==b&&end_face==a)))
            throw std::runtime_error("rim faces do not belong to the full cylinder");
        std::istringstream stream(std::string(reinterpret_cast<const char*>(brep.data()),brep.size()));
        TopoDS_Shape shape;BRep_Builder builder;BRepTools::Read(shape,stream,builder);
        NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> faces,edges;
        TopExp::MapShapes(shape,TopAbs_FACE,faces);TopExp::MapShapes(shape,TopAbs_EDGE,edges);
        if(faces.Extent()!=3||edges.Extent()!=3) throw std::runtime_error("invalid rim topology");
        auto owns=[&](std::uint64_t id,const TopoDS_Shape& edge) {
            for(TopExp_Explorer ex(faces(static_cast<int>(id)),TopAbs_EDGE);ex.More();ex.Next())
                if(ex.Current().IsSame(edge)) return true;
            return false;
        };
        rust::Vec<std::uint64_t> result;
        for(auto cap:{start_face,end_face}) {
            const auto other=cap==start_face?end_face:start_face;
            int found=0;
            for(int i=1;i<=edges.Extent();++i) {
                if(owns(cap,edges(i))&&owns(side_face,edges(i))&&!owns(other,edges(i))) {
                    if(found) throw std::runtime_error("ambiguous cylinder rim incidence");
                    found=i;
                }
            }
            if(!found) throw std::runtime_error("missing cylinder rim incidence");
            result.push_back(static_cast<std::uint64_t>(found));
        }
        if(result[0]==result[1]) throw std::runtime_error("cylinder rims must be distinct");
        return result;
    } catch(const Standard_Failure& e) {throw std::runtime_error(e.what());}
}

// ADR-063: exact OCCT type/topology admission, not mesh or bounding-box inference.
rust::Vec<double> rectangular_stock_faces(rust::Slice<const std::uint8_t> brep) {
    try {
        constexpr double tol=1e-9;
        std::istringstream stream(std::string(reinterpret_cast<const char*>(brep.data()),brep.size()));
        TopoDS_Shape shape; BRep_Builder builder; BRepTools::Read(shape,stream,builder);
        if (shape.IsNull() || !BRepCheck_Analyzer(shape,true).IsValid()) throw std::runtime_error("invalid stock BREP");
        NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> solids,faces,edges,vertices;
        TopExp::MapShapes(shape,TopAbs_SOLID,solids); TopExp::MapShapes(shape,TopAbs_FACE,faces);
        TopExp::MapShapes(shape,TopAbs_EDGE,edges); TopExp::MapShapes(shape,TopAbs_VERTEX,vertices);
        if (solids.Extent()!=1 || faces.Extent()!=6 || edges.Extent()!=12 || vertices.Extent()!=8) throw std::runtime_error("stock is not a six-face cuboid");
        for (int i=1;i<=edges.Extent();++i) {
            BRepAdaptor_Curve curve(TopoDS::Edge(edges(i)));
            if (curve.GetType()!=GeomAbs_Line) throw std::runtime_error("stock edge is not straight");
        }
        gp_XYZ center(0,0,0);
        for (int i=1;i<=8;++i) center+=BRep_Tool::Pnt(TopoDS::Vertex(vertices(i))).XYZ()/8.;
        const auto corner=BRep_Tool::Pnt(TopoDS::Vertex(vertices(1)));
        std::vector<gp_Vec> directions;
        for (int i=1;i<=12;++i) {
            TopoDS_Vertex a,b;TopExp::Vertices(TopoDS::Edge(edges(i)),a,b);
            auto p=BRep_Tool::Pnt(a),q=BRep_Tool::Pnt(b);
            if (corner.Distance(p)<=tol) directions.emplace_back(p,q);
            else if (corner.Distance(q)<=tol) directions.emplace_back(q,p);
        }
        if (directions.size()!=3) throw std::runtime_error("stock corner is not trivalent");
        for (int i=0;i<3;++i) {
            if (directions[i].Magnitude()<=tol) throw std::runtime_error("stock dimension is ambiguous");
            for (int j=0;j<i;++j) if (std::abs(directions[i].Normalized().Dot(directions[j].Normalized()))>tol) throw std::runtime_error("stock dimensions are not orthogonal");
        }
        for (int mask=0;mask<8;++mask) {
            auto point=corner;
            for (int j=0;j<3;++j) if (mask&(1<<j)) point.Translate(directions[j]);
            int count=0;for(int j=1;j<=8;++j) if (point.Distance(BRep_Tool::Pnt(TopoDS::Vertex(vertices(j))))<=tol) ++count;
            if (count!=1) throw std::runtime_error("stock corners do not form a unique cuboid");
        }
        rust::Vec<double> result;
        // Same TopExp_Explorer enumeration as cadrum::shape_faces and native mesh IDs.
        for (TopExp_Explorer ex(shape,TopAbs_FACE);ex.More();ex.Next()) {
            auto face=TopoDS::Face(ex.Current());BRepAdaptor_Surface surface(face);
            if (surface.GetType()!=GeomAbs_Plane) throw std::runtime_error("stock face is not planar");
            int wires=0;for(TopExp_Explorer w(face,TopAbs_WIRE);w.More();w.Next()) ++wires;
            NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> fv,fe;
            TopExp::MapShapes(face,TopAbs_VERTEX,fv);TopExp::MapShapes(face,TopAbs_EDGE,fe);
            if(wires!=1 || fv.Extent()!=4 || fe.Extent()!=4) throw std::runtime_error("stock face has holes or nonrectangular boundary");
            auto origin=BRep_Tool::Pnt(TopoDS::Vertex(fv(1)));std::vector<gp_Vec> axes;
            for(int i=1;i<=4;++i) {
                TopoDS_Vertex a,b;TopExp::Vertices(TopoDS::Edge(fe(i)),a,b);
                auto p=BRep_Tool::Pnt(a),q=BRep_Tool::Pnt(b);
                if(origin.Distance(p)<=tol) axes.emplace_back(p,q);
                else if(origin.Distance(q)<=tol) axes.emplace_back(q,p);
            }
            if(axes.size()!=2 || axes[0].Magnitude()<=tol || axes[1].Magnitude()<=tol || std::abs(axes[0].Normalized().Dot(axes[1].Normalized()))>tol) throw std::runtime_error("stock face axes are ambiguous");
            auto normal=gp_Vec(surface.Plane().Axis().Direction());
            if(face.Orientation()==TopAbs_REVERSED) normal.Reverse();
            else if(face.Orientation()!=TopAbs_FORWARD) throw std::runtime_error("stock face orientation unsupported");
            if(normal.Dot(gp_Vec(gp_Pnt(center),origin))<=tol) throw std::runtime_error("stock face normal is not outward");
            double thickness=0.;
            for(int i=1;i<=8;++i) {
                double depth=-normal.Dot(gp_Vec(origin,BRep_Tool::Pnt(TopoDS::Vertex(vertices(i)))));
                if(depth < -tol) throw std::runtime_error("stock crosses outward face plane");
                thickness=std::max(thickness,depth);
            }
            for(int i=1;i<=4;++i) if(std::abs(normal.Dot(gp_Vec(origin,BRep_Tool::Pnt(TopoDS::Vertex(fv(i))))))>tol) throw std::runtime_error("stock face plane disagreement");
            const auto u=axes[0].Normalized(),v=axes[1].Normalized();
            for(double value:{origin.X(),origin.Y(),origin.Z(),u.X(),u.Y(),u.Z(),v.X(),v.Y(),v.Z(),normal.X(),normal.Y(),normal.Z(),axes[0].Magnitude(),axes[1].Magnitude(),thickness}) {
                if(!std::isfinite(value)) throw std::runtime_error("nonfinite stock witness");
                result.push_back(value);
            }
        }
        return result;
    } catch (const Standard_Failure& failure) {throw std::runtime_error(failure.what());}
}

rust::Vec<double> tight_bounds(rust::Slice<const std::uint8_t> brep) {
    try {
        std::istringstream stream(std::string(reinterpret_cast<const char*>(brep.data()),brep.size()));
        TopoDS_Shape shape; BRep_Builder builder; BRepTools::Read(shape,stream,builder);
        if (shape.IsNull()) throw std::runtime_error("cubic bounds BREP read failed");
        Bnd_Box box; BRepBndLib::AddOptimal(shape,box,false,true);
        if (box.IsVoid() || box.IsOpen()) throw std::runtime_error("unbounded cubic body");
        double xmin,ymin,zmin,xmax,ymax,zmax; box.Get(xmin,ymin,zmin,xmax,ymax,zmax);
        rust::Vec<double> result;
        for (auto value:{xmin,ymin,zmin,xmax,ymax,zmax}) {
            if (!std::isfinite(value)) throw std::runtime_error("non-finite cubic bounds");
            result.push_back(value);
        }
        return result;
    } catch (const Standard_Failure& failure) { throw std::runtime_error(failure.what()); }
}
rust::Vec<std::uint8_t> cubic_prism(rust::Slice<const double> controls, rust::Slice<const double> delta) {
    try {
        if (controls.size()!=12 || delta.size()!=3) throw std::runtime_error("invalid cubic prism input");
        for (auto v:controls) if (!std::isfinite(v)) throw std::runtime_error("non-finite cubic control");
        for (auto v:delta) if (!std::isfinite(v)) throw std::runtime_error("non-finite cubic extrusion");
        NCollection_Array1<gp_Pnt> poles(1,4);
        for (int i=0;i<4;++i) poles.SetValue(i+1,gp_Pnt(controls[3*i],controls[3*i+1],controls[3*i+2]));
        if (poles.Value(1).Distance(poles.Value(4))>1e-12) throw std::runtime_error("cubic span must be explicitly closed");
        gp_Vec a(poles.Value(1),poles.Value(2)), b(poles.Value(1),poles.Value(3)), direction(delta[0],delta[1],delta[2]);
        auto cross=a.Crossed(b); auto area=0.15*cross.Magnitude();
        if (area<=1e-18 || direction.Magnitude()<=1e-9) throw std::runtime_error("degenerate cubic prism");
        auto height=std::abs(direction.Dot(cross.Normalized()));
        if (std::abs(height-direction.Magnitude())>1e-9*direction.Magnitude()) throw std::runtime_error("cubic extrusion must be normal");
        Handle(Geom_BezierCurve) curve=new Geom_BezierCurve(poles);
        BRepBuilderAPI_MakeEdge edge(curve); if (!edge.IsDone()) throw std::runtime_error("cubic edge failed");
        BRepBuilderAPI_MakeWire wire(edge.Edge()); if (!wire.IsDone()) throw std::runtime_error("cubic wire failed");
        BRepBuilderAPI_MakeFace face(wire.Wire()); if (!face.IsDone()) throw std::runtime_error("cubic face failed");
        BRepPrimAPI_MakePrism prism(face.Face(),direction); if (!prism.IsDone()) throw std::runtime_error("cubic prism failed");
        auto solid=prism.Shape(); if (!BRepCheck_Analyzer(solid,true).IsValid()) throw std::runtime_error("invalid cubic prism");
        GProp_GProps mass; BRepGProp::VolumeProperties(solid,mass); auto expected=area*height;
        if (!std::isfinite(mass.Mass()) || mass.Mass()<=0 || std::abs(mass.Mass()-expected)>std::max(1e-18,expected*1e-8)) throw std::runtime_error("cubic prism volume mismatch");
        std::ostringstream stream; BRepTools::Write(solid,stream); auto bytes=stream.str(); rust::Vec<std::uint8_t> result;
        for (auto byte:bytes) result.push_back(static_cast<std::uint8_t>(byte));
        return result;
    } catch (const Standard_Failure& failure) { throw std::runtime_error(failure.what()); }
}
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

// ADR-100: matrix placement of the already admitted actual faceted BREP.
// All checks precede insertion into the Rust body store.
rust::Vec<std::uint8_t> place_faceted_solid(rust::Slice<const std::uint8_t> brep, rust::Slice<const double> p, double expected_volume) {
    try {
        if (p.size()!=12 || !std::isfinite(expected_volume) || expected_volume<=0)
            throw std::runtime_error("invalid placed faceted solid input");
        for (double x:p) if (!std::isfinite(x)) throw std::runtime_error("non-finite faceted placement");
        for (std::size_t i=0;i<3;++i) for (std::size_t j=0;j<=i;++j) {
            double dot=0;for (std::size_t k=0;k<3;++k) dot+=p[4*i+k]*p[4*j+k];
            if (std::abs(dot-(i==j?1.:0.))>1e-12) throw std::runtime_error("non-rigid faceted placement");
        }
        double determinant=p[0]*(p[5]*p[10]-p[6]*p[9])-p[1]*(p[4]*p[10]-p[6]*p[8])+p[2]*(p[4]*p[9]-p[5]*p[8]);
        if (std::abs(determinant-1.)>1e-12) throw std::runtime_error("improper faceted placement");
        std::istringstream stream(std::string(reinterpret_cast<const char*>(brep.data()),brep.size()));
        TopoDS_Shape shape;BRep_Builder builder;BRepTools::Read(shape,stream,builder);
        if (shape.IsNull() || shape.ShapeType()!=TopAbs_SOLID) throw std::runtime_error("faceted placement requires one solid");
        gp_Trsf transform;transform.SetValues(p[0],p[1],p[2],p[3],p[4],p[5],p[6],p[7],p[8],p[9],p[10],p[11]);
        BRepBuilderAPI_Transform conversion(shape,transform,true);
        if (!conversion.IsDone()) throw std::runtime_error("faceted matrix placement failed");
        auto result=conversion.Shape();
        if (result.IsNull() || result.ShapeType()!=TopAbs_SOLID || !BRepCheck_Analyzer(result,true).IsValid())
            throw std::runtime_error("invalid placed faceted solid");
        BOPAlgo_ArgumentAnalyzer check;check.SetShape1(result);check.SelfInterMode()=true;check.StopOnFirstFaulty()=true;check.Perform();
        if (check.HasFaulty()) throw std::runtime_error("self-intersecting placed faceted solid");
        GProp_GProps mass;BRepGProp::VolumeProperties(result,mass);
        if (!std::isfinite(mass.Mass()) || mass.Mass()<=0 || std::abs(mass.Mass()-expected_volume)>std::max(1e-18,expected_volume*1e-8))
            throw std::runtime_error("placed faceted volume differs from actual grid");
        std::ostringstream output;BRepTools::Write(result,output);auto bytes=output.str();rust::Vec<std::uint8_t> out;
        for (auto byte:bytes) out.push_back(static_cast<std::uint8_t>(byte));
        return out;
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
