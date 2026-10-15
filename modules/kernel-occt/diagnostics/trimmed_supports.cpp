// ADR-083: retained analytic support faces only; no blend, shell or solid.
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepGProp.hxx>
#include <BRepLib.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_Circle.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Plane.hxx>
#include <GeomProjLib.hxx>
#include <Geom2d_Curve.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_IndexedMap.hxx>
#include <Standard_Failure.hxx>
#include <Standard_Version.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec2d.hxx>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <stdexcept>
#if OCC_VERSION_HEX != 0x080000
#error Pinned OCCT 8.0.0 required
#endif
namespace {
constexpr double request_mm=1e-7, maximum_mm=1e-6;
const double pi=std::acos(-1.);
void require(bool condition,const char* message) {if (!condition) throw std::runtime_error(message);}
template<class T> void xyz(std::ostream& out,const T& p,double scale=1e-3) {
  require(std::isfinite(p.X()) && std::isfinite(p.Y()) && std::isfinite(p.Z()),"nonfinite native measurement");
  out << '[' << p.X()*scale << ',' << p.Y()*scale << ',' << p.Z()*scale << ']';
}
int state(const TopoDS_Face& face,const gp_Pnt& p) {
  const auto s=BRepClass_FaceClassifier(face,p,request_mm).State();
  return s==TopAbs_IN ? 1 : s==TopAbs_OUT ? 0 : -1;
}
double attach(const TopoDS_Edge& edge,const Handle(Geom_Surface)& surface,int chart,bool side) {
  double first,last;auto curve=BRep_Tool::Curve(edge,first,last);double achieved=request_mm;
  auto pc=GeomProjLib::Curve2d(curve,first,last,surface,side ? chart*pi/2 : -10.,side ? (chart+1)*pi/2 : 10.,side ? 0. : -10.,side ? 20. : 10.,achieved);
  require(!pc.IsNull() && std::isfinite(achieved) && achieved>=0 && achieved<=maximum_mm,"native pcurve projection failed or exceeded budget");
  if (side) {
    const double center=(chart+.5)*pi/2;
    const double periods=std::round((center-pc->Value((first+last)/2).X())/(2*pi));
    pc->Translate(gp_Vec2d(periods*2*pi,0));
  }
  BRep_Builder builder;builder.UpdateEdge(edge,pc,surface,TopLoc_Location(),request_mm);
  builder.SameParameter(edge,false);BRepLib::SameParameter(edge,request_mm);
  return achieved*1e-3;
}
void describe(std::ostream& out,const TopoDS_Face& face,const std::array<TopoDS_Edge,4>& contacts,bool side,int chart,double projection,int law) {
  auto surface=BRep_Tool::Surface(face);
  const auto cylinder=Handle(Geom_CylindricalSurface)::DownCast(surface);
  const auto plane=Handle(Geom_Plane)::DownCast(surface);
  require(side ? !cylinder.IsNull() : !plane.IsNull(),"native analytic support lost");
  require(BRepCheck_Analyzer(face).IsValid(),"invalid trimmed support face");
  NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> wires,edges,vertices,shells,solids;
  TopExp::MapShapes(face,TopAbs_WIRE,wires);TopExp::MapShapes(face,TopAbs_EDGE,edges);TopExp::MapShapes(face,TopAbs_VERTEX,vertices);
  TopExp::MapShapes(face,TopAbs_SHELL,shells);TopExp::MapShapes(face,TopAbs_SOLID,solids);
  require(wires.Extent()==1 && BRep_Tool::IsClosed(wires(1)) && edges.Extent()==4 && vertices.Extent()==4 && shells.IsEmpty() && solids.IsEmpty(),"unexpected trimmed face topology");
  GProp_GProps properties;BRepGProp::SurfaceProperties(face,properties,1e-11);
  out << "{\"kind\":\"" << (side ? "side" : "end") << "\",\"chart\":" << chart << ",\"valid\":true,\"forward\":" << (face.Orientation()==TopAbs_FORWARD ? "true" : "false")
      << ",\"wires\":1,\"edges\":4,\"vertices\":4,\"closed_wire\":true,\"shells\":0,\"solids\":0,\"support_type\":\"" << (side ? "cylinder" : "plane") << "\",\"support_origin_m\":";
  xyz(out,side ? cylinder->Location() : plane->Location());
  out << ",\"support_axis\":";xyz(out,side ? cylinder->Axis().Direction() : plane->Axis().Direction(),1);
  out << ",\"support_x\":";xyz(out,side ? cylinder->Position().XDirection() : plane->Position().XDirection(),1);
  out << ",\"support_radius_m\":" << (side ? cylinder->Radius()*1e-3 : 0.) << ",\"area_m2\":" << properties.Mass()*1e-6 << ",\"projection_achieved_m\":" << projection << ",\"vertex_tolerances_m\":[";
  for (int i=1;i<=vertices.Extent();++i) {
    const double tol=BRep_Tool::Tolerance(TopoDS::Vertex(vertices(i)));require(std::isfinite(tol) && tol>0 && tol<=maximum_mm,"inflated vertex tolerance");
    if (i>1) out << ',';out << tol*1e-3;
  }
  out << "],\"boundaries\":[";
  for (int i=1;i<=edges.Extent();++i) {
    const auto edge=TopoDS::Edge(edges(i));double f,l,pf,pl;auto curve=BRep_Tool::Curve(edge,f,l);auto pc=BRep_Tool::CurveOnSurface(edge,face,pf,pl);
    require(!curve.IsNull() && !pc.IsNull(),"missing current native pcurve");
    // MakeWire can copy edges when merging vertices. Match the actual current
    // curve, then independently check its representation in the report grader.
    int contact=-1;for (int j=0;j<4;++j) {double cf,cl;if (curve==BRep_Tool::Curve(contacts[j],cf,cl)) contact=j;}
    const double tol=BRep_Tool::Tolerance(edge);require(std::isfinite(tol) && tol>0 && tol<=maximum_mm,"inflated edge tolerance");
    if (i>1) out << ',';
    out << "{\"contact_chart\":" << contact << ",\"tolerance_m\":" << tol*1e-3 << ",\"same_parameter\":" << (BRep_Tool::SameParameter(edge) ? "true" : "false") << ",\"same_range\":" << (BRep_Tool::SameRange(edge) ? "true" : "false")
        << ",\"range\":[" << f << ',' << l << "],\"pcurve_range\":[" << pf << ',' << pl << ']';
    if (contact>=0) {
      auto spline=Handle(Geom_BSplineCurve)::DownCast(curve);require(!spline.IsNull(),"contact curve replaced");
      out << ",\"degree\":" << spline->Degree() << ",\"poles_m\":[";
      for (int j=1;j<=spline->NbPoles();++j) {if (j>1) out << ',';xyz(out,spline->Pole(j));}
      out << "],\"weights\":[";for (int j=1;j<=spline->NbPoles();++j) {if (j>1) out << ',';out << spline->Weight(j);}
      out << "],\"knots\":[";for (int j=1;j<=spline->NbKnots();++j) {if (j>1) out << ',';out << spline->Knot(j);}
      out << "],\"multiplicities\":[";for (int j=1;j<=spline->NbKnots();++j) {if (j>1) out << ',';out << spline->Multiplicity(j);}out << ']';
    }
    out << ",\"samples\":[";
    for (int j=0;j<=16;++j) {
      if (j) out << ',';const double t=f+(l-f)*j/16.;const auto uv=pc->Value(t);gp_Pnt p;gp_Vec du,dv;surface->D1(uv.X(),uv.Y(),p,du,dv);auto normal=du.Crossed(dv);normal.Normalize();
      out << "{\"t\":" << t << ",\"uv\":[" << uv.X()*(side ? 1. : 1e-3) << ',' << uv.Y()*1e-3 << "],\"point_m\":";xyz(out,curve->Value(t));
      out << ",\"lifted_m\":";xyz(out,p);out << ",\"normal\":";xyz(out,normal,1);out << '}';
    }
    out << "]}";
  }
  out << "],\"classifications\":[";
  // Current face classification at independently gradeable physical positions.
  auto emit=[&](const TopoDS_Face& current,const gp_Pnt& p) {out << "{\"point_m\":";xyz(out,p);out << ",\"state\":" << state(current,p) << '}';};
  if (side) {
    for (int j=0;j<3;++j) {
      const double theta=(chart+(j+1)/4.)*pi/2;const double radius=law==0 ? .5 : 1.+.5*std::cos(theta);
      for (int s=0;s<2;++s) {if (j || s) out << ',';emit(face,gp_Pnt(10*std::cos(theta),10*std::sin(theta),20-radius+(s ? .1 : -.1)));}
    }
  } else {
    emit(face,gp_Pnt(0,0,20));
    for (int j=0;j<4;++j) {
      const double theta=(j+.5)*pi/2;const double radius=law==0 ? .5 : 1.+.5*std::cos(theta);
      for (int s=0;s<2;++s) {out << ',';const double rho=10-radius+(s ? .1 : -.1);emit(face,gp_Pnt(rho*std::cos(theta),rho*std::sin(theta),20));}
    }
  }
  out << "],\"rotated_classifications\":[";
  if (side) {
    gp_Trsf turn;turn.SetRotation(gp_Ax1(gp_Pnt(0,0,0),gp_Dir(0,0,1)),pi/2);
    BRepBuilderAPI_Transform moved(face,turn,true);require(moved.IsDone(),"native support phase transform failed");const auto rotated=TopoDS::Face(moved.Shape());
    const double theta=(chart+1.5)*pi/2;const double radius=law==0 ? .5 : 1.+.5*std::cos(theta);
    for (int s=0;s<2;++s) {if (s) out << ',';emit(rotated,gp_Pnt(10*std::cos(theta),10*std::sin(theta),20-radius+(s ? .1 : -.1)));}
  }
  out << "]}";
}
void run() {
  int count;require(bool(std::cin>>count) && count==16,"expected sixteen contacts");
  std::array<TopoDS_Edge,16> edges;
  for (int c=0;c<16;++c) {
    int degree;require(bool(std::cin>>degree) && degree==(c%8<4 ? 2 : 4),"wrong contact degree");
    NCollection_Array1<gp_Pnt> poles(1,degree+1);NCollection_Array1<double> weights(1,degree+1),knots(1,2);NCollection_Array1<int> mults(1,2);
    for (int i=1;i<=degree+1;++i) {double x,y,z,w;require(bool(std::cin>>x>>y>>z>>w) && std::isfinite(x) && std::isfinite(y) && std::isfinite(z) && std::isfinite(w) && w>0,"invalid contact data");poles(i)=gp_Pnt(x*1e3,y*1e3,z*1e3);weights(i)=w;}
    knots(1)=0;knots(2)=1;mults(1)=mults(2)=degree+1;
    Handle(Geom_BSplineCurve) curve=new Geom_BSplineCurve(poles,weights,knots,mults,degree);BRepBuilderAPI_MakeEdge made(curve);require(made.IsDone(),"contact edge failed");edges[c]=made.Edge();
  }
  std::string extra;require(!(std::cin>>extra),"unexpected native input");
  std::ostringstream out;out << std::setprecision(17) << "{\"format_version\":1,\"occt_version\":\"" << OCC_VERSION_COMPLETE << "\",\"faces\":[";
  for (int law=0;law<2;++law) {
    Handle(Geom_CylindricalSurface) cylinder=new Geom_CylindricalSurface(gp_Ax3(gp_Pnt(0,0,0),gp_Dir(0,0,1),gp_Dir(1,0,0)),10);
    Handle(Geom_Plane) plane=new Geom_Plane(gp_Ax3(gp_Pnt(0,0,20),gp_Dir(0,0,1),gp_Dir(1,0,0)));
    std::array<TopoDS_Edge,4> side,end;for (int j=0;j<4;++j) {side[j]=edges[law*8+j];end[j]=edges[law*8+4+j];}
    for (int chart=0;chart<4;++chart) {
      double f,l;auto contact=BRep_Tool::Curve(side[chart],f,l);const auto p=contact->Value(f),q=contact->Value(l);
      const gp_Pnt bottom_p(p.X(),p.Y(),0),bottom_q(q.X(),q.Y(),0);
      Handle(Geom_Circle) circle=new Geom_Circle(gp_Ax2(gp_Pnt(0,0,0),gp_Dir(0,0,1),gp_Dir(1,0,0)),10);
      std::array<TopoDS_Edge,4> boundary={BRepBuilderAPI_MakeEdge(circle,chart*pi/2,(chart+1)*pi/2).Edge(),BRepBuilderAPI_MakeEdge(bottom_q,q).Edge(),side[chart],BRepBuilderAPI_MakeEdge(p,bottom_p).Edge()};
      double achieved=0;for (const auto& edge:boundary) achieved=std::max(achieved,attach(edge,cylinder,chart,true));
      boundary[2].Reverse();BRepBuilderAPI_MakeWire wire;for (const auto& edge:boundary) wire.Add(edge);require(wire.IsDone(),"side wire failed");
      BRepBuilderAPI_MakeFace made(cylinder,wire.Wire(),true);require(made.IsDone(),"cylinder trim failed");
      if (law || chart) out << ',';describe(out,made.Face(),side,true,chart,achieved,law);
    }
    BRepBuilderAPI_MakeWire wire;double achieved=0;for (const auto& edge:end) {achieved=std::max(achieved,attach(edge,plane,0,false));wire.Add(edge);}require(wire.IsDone(),"end wire failed");
    BRepBuilderAPI_MakeFace made(plane,wire.Wire(),true);require(made.IsDone(),"end plane trim failed");out << ',';describe(out,made.Face(),end,false,-1,achieved,law);
  }
  out << "]}\n";std::cout << out.str();
}
}
int main() {try {run();} catch (const Standard_Failure& e) {std::cerr << e.what() << '\n';return 1;} catch (const std::exception& e) {std::cerr << e.what() << '\n';return 1;}}
