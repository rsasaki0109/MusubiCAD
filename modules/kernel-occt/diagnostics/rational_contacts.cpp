// ADR-082: actual rational contact edges/wires, not a trimmed support or body.
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <Geom_BSplineCurve.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_IndexedMap.hxx>
#include <Standard_Failure.hxx>
#include <Standard_Version.hxx>
#include <TopExp.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Vec.hxx>
#include <array>
#include <cmath>
#include <gp_Ax1.hxx>
#include <gp_Trsf.hxx>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <stdexcept>
#if OCC_VERSION_HEX != 0x080000
#error Pinned OCCT 8.0.0 required
#endif
namespace {
void require(bool value,const char* message) {if (!value) throw std::runtime_error(message);}
template<class T> void vector(std::ostream& out,const T& p) {
  require(std::isfinite(p.X()) && std::isfinite(p.Y()) && std::isfinite(p.Z()),"nonfinite native contact value");
  out << '[' << p.X()*1e-3 << ',' << p.Y()*1e-3 << ',' << p.Z()*1e-3 << ']';
}
void samples(std::ostream& out,const Handle(Geom_Curve)& curve) {
  out << '[';
  for (int j=0;j<=8;++j) {
    if (j) out << ',';
    const double u=j/8.;gp_Pnt p;gp_Vec d;curve->D1(u,p,d);
    out << "{\"u\":" << u << ",\"point_m\":";vector(out,p);
    out << ",\"tangent_m\":";vector(out,d);out << '}';
  }
  out << ']';
}
void run() {
  int count;require(bool(std::cin>>count) && count==16,"expected sixteen support curves");
  std::array<BRepBuilderAPI_MakeWire,4> wires;
  gp_Trsf turn;turn.SetRotation(gp_Ax1(gp_Pnt(0,0,0),gp_Dir(0,0,1)),std::acos(-1.)/2.);
  std::ostringstream out;out << std::setprecision(17) << "{\"format_version\":1,\"occt_version\":\"" << OCC_VERSION_COMPLETE << "\",\"curves\":[";
  for (int c=0;c<count;++c) {
    int degree;require(bool(std::cin>>degree) && degree==(c%8<4 ? 2 : 4),"wrong contact degree");
    NCollection_Array1<gp_Pnt> poles(1,degree+1);
    NCollection_Array1<double> weights(1,degree+1),knots(1,2);
    NCollection_Array1<int> mults(1,2);
    for (int i=1;i<=degree+1;++i) {
      double x,y,z,w;require(bool(std::cin>>x>>y>>z>>w) && std::isfinite(x) && std::isfinite(y) && std::isfinite(z) && std::isfinite(w) && w>0,"invalid contact poles or weight");
      poles(i)=gp_Pnt(x*1e3,y*1e3,z*1e3);weights(i)=w;
    }
    knots(1)=0;knots(2)=1;mults(1)=mults(2)=degree+1;
    Handle(Geom_BSplineCurve) made=new Geom_BSplineCurve(poles,weights,knots,mults,degree);
    BRepBuilderAPI_MakeEdge builder(made);require(builder.IsDone(),"native contact edge failed");
    const auto edge=builder.Edge();require(BRepCheck_Analyzer(edge).IsValid(),"invalid contact edge");
    double first,last;auto current=BRep_Tool::Curve(edge,first,last);
    auto curve=Handle(Geom_BSplineCurve)::DownCast(current);
    require(!curve.IsNull() && first==0 && last==1 && curve->IsRational(),"unexpected current edge representation");
    wires[c/4].Add(edge);require(wires[c/4].IsDone(),"native contact wire connection failed");
    if (c) out << ',';
    out << "{\"degree\":" << curve->Degree() << ",\"rational\":true,\"edge_valid\":true,\"poles_m\":[";
    for (int i=1;i<=curve->NbPoles();++i) {if (i>1) out << ',';vector(out,curve->Pole(i));}
    out << "],\"weights\":[";
    for (int i=1;i<=curve->NbPoles();++i) {if (i>1) out << ',';out << curve->Weight(i);}
    out << "],\"knots\":[";
    for (int i=1;i<=curve->NbKnots();++i) {if (i>1) out << ',';out << curve->Knot(i);}
    out << "],\"multiplicities\":[";
    for (int i=1;i<=curve->NbKnots();++i) {if (i>1) out << ',';out << curve->Multiplicity(i);}
    out << "],\"samples\":";samples(out,current);
    auto rotated=Handle(Geom_Curve)::DownCast(current->Transformed(turn));
    require(!rotated.IsNull(),"native contact phase transform failed");
    out << ",\"rotated_samples\":";samples(out,rotated);out << '}';
  }
  out << "],\"wires\":[";
  for (int i=0;i<4;++i) {
    const auto wire=wires[i].Wire();
    require(BRepCheck_Analyzer(wire).IsValid() && BRep_Tool::IsClosed(wire),"native contact wire invalid or open");
    NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> edges,vertices;
    TopExp::MapShapes(wire,TopAbs_EDGE,edges);TopExp::MapShapes(wire,TopAbs_VERTEX,vertices);
    require(edges.Extent()==4 && vertices.Extent()==4,"wrong contact wire inventory");
    GProp_GProps length,rotated_length;BRepGProp::LinearProperties(wire,length);
    BRepBuilderAPI_Transform rotated_wire(wire,turn,true);require(rotated_wire.IsDone(),"native wire phase transform failed");
    BRepGProp::LinearProperties(rotated_wire.Shape(),rotated_length);
    require(std::isfinite(length.Mass()) && length.Mass()>0 && std::isfinite(rotated_length.Mass()),"invalid wire length");
    if (i) out << ',';
    out << "{\"valid\":true,\"closed\":true,\"edges\":4,\"vertices\":4,\"length_m\":" << length.Mass()*1e-3 << ",\"rotated_length_m\":" << rotated_length.Mass()*1e-3 << '}';
  }
  std::string extra;require(!(std::cin>>extra),"unexpected contact input");
  out << "]}\n";std::cout << out.str();
}
}
int main() {
  try {run();}
  catch (const Standard_Failure& e) {std::cerr << e.what() << '\n';return 1;}
  catch (const std::exception& e) {std::cerr << e.what() << '\n';return 1;}
}
