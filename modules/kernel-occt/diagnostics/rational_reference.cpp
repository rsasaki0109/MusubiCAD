// ADR-084: rational envelope + unchanged supports, diagnostic solids only.
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepGProp.hxx>
#include <BRepLib.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <Geom_BezierSurface.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_Circle.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom2d_Curve.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <GeomProjLib.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_Array2.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <NCollection_IndexedMap.hxx>
#include <NCollection_List.hxx>
#include <Standard_Failure.hxx>
#include <Standard_Version.hxx>
#include <TopExp.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec2d.hxx>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <map>
#include <sstream>
#include <stdexcept>
#include <vector>
#if OCC_VERSION_HEX != 0x080000
#error Pinned OCCT 8.0.0 required
#endif
namespace {
const double pi=std::acos(-1.);
constexpr double maximum_mm=1e-6, projection_mm=1e-9;
void require(bool condition,const char* message) {if (!condition) throw std::runtime_error(message);}
template<class T> void xyz(std::ostream& out,const T& p,double scale=1e-3) {
  require(std::isfinite(p.X()) && std::isfinite(p.Y()) && std::isfinite(p.Z()),"nonfinite native vector");
  out << '[' << p.X()*scale << ',' << p.Y()*scale << ',' << p.Z()*scale << ']';
}
struct Role {int chart;int kind;}; // blend0 cylinder1 top2 bottom3
void role(std::ostream& out,Role r) {out << "{\"kind\":" << r.kind << ",\"chart\":" << r.chart << '}';}
Role identify(const TopoDS_Face& face,const std::map<const Geom_Surface*,Role>& records) {
  TopLoc_Location location;auto surface=BRep_Tool::Surface(face,location);
  require(location.IsIdentity() && records.count(surface.get()),"native sewing replaced an analytic/reference support");
  return records.at(surface.get());
}
void attach(const TopoDS_Edge& edge,const Handle(Geom_Surface)& surface,int chart,bool cylinder,const Handle(Geom2d_Curve)& supplied={}) {
  double f,l;auto curve=BRep_Tool::Curve(edge,f,l);double achieved=projection_mm;
  auto pc=supplied;
  if (pc.IsNull()) pc=GeomProjLib::Curve2d(curve,f,l,surface,cylinder ? chart*pi/2 : -10.,cylinder ? (chart+1)*pi/2 : 10.,cylinder ? 0. : -10.,cylinder ? 20. : 10.,achieved);
  require(!pc.IsNull() && std::isfinite(achieved) && achieved>=0 && achieved<=maximum_mm,"contact projection failed/exceeded budget");
  if (cylinder) {const double periods=std::round(((chart+.5)*pi/2-pc->Value((f+l)/2).X())/(2*pi));pc->Translate(gp_Vec2d(periods*2*pi,0));}
  BRep_Builder builder;builder.UpdateEdge(edge,pc,surface,TopLoc_Location(),projection_mm);builder.SameParameter(edge,false);BRepLib::SameParameter(edge,projection_mm);
}
int state(const TopoDS_Solid& solid,const gp_Pnt& p) {
  const auto s=BRepClass3d_SolidClassifier(solid,p,projection_mm).State();return s==TopAbs_IN ? 1 : s==TopAbs_OUT ? 0 : -1;
}
void measure(std::ostream& out,int law) {
  BRepBuilderAPI_Sewing sewing(maximum_mm);
  std::map<const Geom_Surface*,Role> records;
  std::array<TopoDS_Edge,8> contacts;
  for (int c=0;c<8;++c) {
    int degree;require(bool(std::cin>>degree) && degree==(c<4 ? 2 : 4),"wrong contact degree");
    NCollection_Array1<gp_Pnt> poles(1,degree+1);NCollection_Array1<double> weights(1,degree+1),knots(1,2);NCollection_Array1<int> mults(1,2);
    for (int i=1;i<=degree+1;++i) {double x,y,z,w;require(bool(std::cin>>x>>y>>z>>w) && std::isfinite(x) && std::isfinite(y) && std::isfinite(z) && std::isfinite(w) && w>0,"invalid contact input");poles(i)=gp_Pnt(x*1e3,y*1e3,z*1e3);weights(i)=w;}
    knots(1)=0;knots(2)=1;mults(1)=mults(2)=degree+1;
    BRepBuilderAPI_MakeEdge made(new Geom_BSplineCurve(poles,weights,knots,mults,degree));require(made.IsDone(),"contact edge failed");contacts[c]=made.Edge();
  }
  std::array<Handle(Geom2d_Curve),4> side_pcurves;
  for (int chart=0;chart<4;++chart) {
    int n;require(bool(std::cin>>n) && n==256,"wrong pcurve span inventory");
    NCollection_Array1<gp_Pnt2d> poles(1,3*n+1);NCollection_Array1<double> knots(1,n+1);NCollection_Array1<int> mults(1,n+1);
    for (int i=1;i<=3*n+1;++i) {double u,z;require(bool(std::cin>>u>>z) && std::isfinite(u) && std::isfinite(z),"invalid pcurve control");poles(i)=gp_Pnt2d(u,z*1e3);}
    for (int i=0;i<=n;++i) {knots(i+1)=i/double(n);mults(i+1)=(i==0 || i==n) ? 4 : 3;}
    side_pcurves[chart]=new Geom2d_BSplineCurve(poles,knots,mults,3);
  }
  for (int chart=0;chart<4;++chart) {
    NCollection_Array2<gp_Pnt> poles(1,9,1,3);NCollection_Array2<double> weights(1,9,1,3);
    for (int i=1;i<=9;++i) for (int j=1;j<=3;++j) {
      double x,y,z,w;require(bool(std::cin>>x>>y>>z>>w) && std::isfinite(x) && std::isfinite(y) && std::isfinite(z) && std::isfinite(w) && w>0,"invalid patch input");poles(i,j)=gp_Pnt(x*1e3,y*1e3,z*1e3);weights(i,j)=w;
    }
    Handle(Geom_BezierSurface) surface=new Geom_BezierSurface(poles,weights);
    // Retain the supplied low-degree contact geometry rather than replacing
    // it with the surface builder's newly generated high-degree isocurves.
    std::array<TopoDS_Edge,4> boundary={contacts[chart],BRepBuilderAPI_MakeEdge(surface->UIso(1)).Edge(),contacts[chart+4],BRepBuilderAPI_MakeEdge(surface->UIso(0)).Edge()};
    const std::array<gp_Pnt2d,4> origins={gp_Pnt2d(0,0),gp_Pnt2d(1,0),gp_Pnt2d(0,1),gp_Pnt2d(0,0)};
    BRep_Builder builder;
    for (int i=0;i<4;++i) {
      Handle(Geom2d_Curve) pc=new Geom2d_Line(origins[i],i%2 ? gp_Dir2d(0,1) : gp_Dir2d(1,0));
      builder.UpdateEdge(boundary[i],pc,surface,TopLoc_Location(),projection_mm);builder.Range(boundary[i],surface,TopLoc_Location(),0,1);
      builder.SameParameter(boundary[i],false);BRepLib::SameParameter(boundary[i],projection_mm);
    }
    boundary[2].Reverse();boundary[3].Reverse();BRepBuilderAPI_MakeWire wire;for (const auto& edge:boundary) wire.Add(edge);require(wire.IsDone(),"blend contact wire failed");
    BRepBuilderAPI_MakeFace made(surface,wire.Wire(),true);require(made.IsDone(),"rational blend face failed");records.emplace(surface.get(),Role{chart,0});sewing.Add(made.Face());
  }
  Handle(Geom_CylindricalSurface) cylinder=new Geom_CylindricalSurface(gp_Ax3(gp_Pnt(0,0,0),gp_Dir(0,0,1),gp_Dir(1,0,0)),10);
  records.emplace(cylinder.get(),Role{-1,1});BRepBuilderAPI_MakeWire bottom_wire;
  for (int chart=0;chart<4;++chart) {
    double f,l;auto contact=BRep_Tool::Curve(contacts[chart],f,l);const auto p=contact->Value(f),q=contact->Value(l);
    const gp_Pnt bp(p.X(),p.Y(),0),bq(q.X(),q.Y(),0);
    Handle(Geom_Circle) circle=new Geom_Circle(gp_Ax2(gp_Pnt(0,0,0),gp_Dir(0,0,1),gp_Dir(1,0,0)),10);
    std::array<TopoDS_Edge,4> boundary={BRepBuilderAPI_MakeEdge(circle,chart*pi/2,(chart+1)*pi/2).Edge(),BRepBuilderAPI_MakeEdge(bq,q).Edge(),contacts[chart],BRepBuilderAPI_MakeEdge(p,bp).Edge()};
    for (int i=0;i<4;++i) attach(boundary[i],cylinder,chart,true,i==2 ? side_pcurves[chart] : Handle(Geom2d_Curve)());
    bottom_wire.Add(boundary[0]);boundary[2].Reverse();BRepBuilderAPI_MakeWire wire;for (const auto& edge:boundary) wire.Add(edge);require(wire.IsDone(),"cylinder boundary failed");
    BRepBuilderAPI_MakeFace made(cylinder,wire.Wire(),true);require(made.IsDone(),"unchanged cylinder trim failed");sewing.Add(made.Face());
  }
  Handle(Geom_Plane) top=new Geom_Plane(gp_Ax3(gp_Pnt(0,0,20),gp_Dir(0,0,1),gp_Dir(1,0,0)));
  BRepBuilderAPI_MakeWire top_wire;for (int i=4;i<8;++i) {attach(contacts[i],top,0,false);top_wire.Add(contacts[i]);}
  require(top_wire.IsDone() && bottom_wire.IsDone(),"planar boundaries failed");
  BRepBuilderAPI_MakeFace top_face(top,top_wire.Wire(),true);require(top_face.IsDone(),"top trim failed");records.emplace(top.get(),Role{-1,2});sewing.Add(top_face.Face());
  Handle(Geom_Plane) bottom=new Geom_Plane(gp_Ax3(gp_Pnt(0,0,0),gp_Dir(0,0,1),gp_Dir(1,0,0)));
  BRepBuilderAPI_MakeFace bottom_face(bottom,bottom_wire.Wire(),true);require(bottom_face.IsDone(),"bottom face failed");auto reversed=bottom_face.Face();reversed.Reverse();records.emplace(bottom.get(),Role{-1,3});sewing.Add(reversed);
  sewing.Perform();auto shape=sewing.SewedShape();
  require(!shape.IsNull() && shape.ShapeType()==TopAbs_SHELL && BRepCheck_Analyzer(shape).IsValid() && BRep_Tool::IsClosed(shape),"native reference shell invalid/open");
  BRepBuilderAPI_MakeSolid made(TopoDS::Shell(shape));require(made.IsDone(),"diagnostic solid failed");const auto solid=made.Solid();require(BRepCheck_Analyzer(solid).IsValid(),"native diagnostic solid invalid");
  NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> faces,edges,vertices,shells,solids;
  TopExp::MapShapes(solid,TopAbs_FACE,faces);TopExp::MapShapes(solid,TopAbs_EDGE,edges);TopExp::MapShapes(solid,TopAbs_VERTEX,vertices);TopExp::MapShapes(solid,TopAbs_SHELL,shells);TopExp::MapShapes(solid,TopAbs_SOLID,solids);
  require(faces.Extent()==10 && edges.Extent()==20 && vertices.Extent()==12 && shells.Extent()==1 && solids.Extent()==1,"wrong reference topology inventory");
  NCollection_IndexedDataMap<TopoDS_Shape,NCollection_List<TopoDS_Shape>,TopTools_ShapeMapHasher> incidence;TopExp::MapShapesAndAncestors(solid,TopAbs_EDGE,TopAbs_FACE,incidence);
  std::vector<std::vector<int>> adjacent(11);
  for (int e=1;e<=20;++e) {const auto& owners=incidence.FindFromKey(edges(e));require(owners.Size()==2,"free/nonmanifold reference edge");const int a=faces.FindIndex(owners.First()),b=faces.FindIndex(owners.Last());require(a>0 && b>0 && a!=b,"invalid reference incidence");adjacent[a].push_back(b);adjacent[b].push_back(a);}
  std::array<bool,11> seen{};std::vector<int> stack{1};seen[1]=true;int reached=0;while (!stack.empty()) {const int a=stack.back();stack.pop_back();++reached;for (int b:adjacent[a]) if (!seen[b]) {seen[b]=true;stack.push_back(b);}}require(reached==10,"disconnected reference faces");
  GProp_GProps mass,coarse;BRepGProp::VolumeProperties(solid,mass,1e-11);BRepGProp::VolumeProperties(solid,coarse,1e-9);require(std::isfinite(mass.Mass()) && mass.Mass()>0,"nonpositive diagnostic volume; no orientation repair");
  out << "{\"valid\":true,\"closed\":true,\"connected\":true,\"edge_incidence_two\":true,\"faces\":10,\"edges\":20,\"vertices\":12,\"shells\":1,\"solids\":1,\"euler\":2,\"free_edges\":" << sewing.NbFreeEdges() << ",\"multiple_edges\":" << sewing.NbMultipleEdges() << ",\"volume_m3\":" << mass.Mass()*1e-9 << ",\"volume_refinement_delta_m3\":" << std::abs(mass.Mass()-coarse.Mass())*1e-9 << ",\"center_of_mass_m\":";xyz(out,mass.CentreOfMass());
  out << ",\"vertex_tolerances_m\":[";for (int i=1;i<=12;++i) {if (i>1) out << ',';out << BRep_Tool::Tolerance(TopoDS::Vertex(vertices(i)))*1e-3;}
  out << "],\"surface_faces\":[";
  for (int index=1;index<=10;++index) {
    if (index>1) out << ',';const auto face=TopoDS::Face(faces(index));const auto r=identify(face,records);const auto surface=BRep_Tool::Surface(face);
    out << "{\"role\":";role(out,r);out << ",\"location_identity\":true,\"forward\":" << (face.Orientation()==TopAbs_FORWARD ? "true" : "false");
    if (r.kind==0) {
      auto patch=Handle(Geom_BezierSurface)::DownCast(surface);require(!patch.IsNull(),"rational surface type lost");
      out << ",\"u_degree\":" << patch->UDegree() << ",\"v_degree\":" << patch->VDegree() << ",\"poles_m\":[";
      for (int i=1;i<=patch->NbUPoles();++i) {if (i>1) out << ',';out << '[';for (int j=1;j<=patch->NbVPoles();++j) {if (j>1) out << ',';xyz(out,patch->Pole(i,j));}out << ']';}
      out << "],\"poles_mm\":[" << std::showpoint;
      for (int i=1;i<=patch->NbUPoles();++i) {if (i>1) out << ',';out << '[';for (int j=1;j<=patch->NbVPoles();++j) {if (j>1) out << ',';xyz(out,patch->Pole(i,j),1);}out << ']';}
      out << std::noshowpoint; // Preserve signed zero as a JSON float in raw controls.
      double uf,ul,vf,vl;patch->Bounds(uf,ul,vf,vl);
      out << "],\"u_bounds\":[" << uf << ',' << ul << "],\"v_bounds\":[" << vf << ',' << vl << "],\"u_periodic\":" << (patch->IsUPeriodic() ? "true" : "false") << ",\"v_periodic\":" << (patch->IsVPeriodic() ? "true" : "false");
      out << ",\"weights\":[";for (int i=1;i<=patch->NbUPoles();++i) {if (i>1) out << ',';out << '[';for (int j=1;j<=patch->NbVPoles();++j) {if (j>1) out << ',';out << patch->Weight(i,j);}out << ']';}
      out << "],\"samples\":[";
      for (int i=0;i<=8;++i) for (int j=0;j<=8;++j) {
        if (i || j) out << ',';gp_Pnt p;gp_Vec du,dv;patch->D1(i/8.,j/8.,p,du,dv);auto normal=du.Crossed(dv);normal.Normalize();
        out << "{\"u\":" << i/8. << ",\"v\":" << j/8. << ",\"point_m\":";xyz(out,p);out << ",\"du_m\":";xyz(out,du);out << ",\"dv_m\":";xyz(out,dv);out << ",\"normal\":";xyz(out,normal,1);out << '}';
      }
      out << ']';
    } else {
      const auto cylinder=Handle(Geom_CylindricalSurface)::DownCast(surface);const auto plane=Handle(Geom_Plane)::DownCast(surface);require(r.kind==1 ? !cylinder.IsNull() : !plane.IsNull(),"analytic support type lost");
      out << ",\"support_origin_m\":";xyz(out,r.kind==1 ? cylinder->Location() : plane->Location());out << ",\"support_axis\":";xyz(out,r.kind==1 ? cylinder->Axis().Direction() : plane->Axis().Direction(),1);
      out << ",\"support_x\":";xyz(out,r.kind==1 ? cylinder->Position().XDirection() : plane->Position().XDirection(),1);out << ",\"support_radius_m\":" << (r.kind==1 ? cylinder->Radius()*1e-3 : 0.);
      if (r.kind==2) {
        out << ",\"plane_origin_mm\":" << std::showpoint;xyz(out,plane->Location(),1);
        out << ",\"plane_x_direction\":";xyz(out,plane->Position().XDirection(),1);
        out << ",\"plane_y_direction\":";xyz(out,plane->Position().YDirection(),1);out << std::noshowpoint;
      }
    }
    out << '}';
  }
  out << "],\"shared_edges\":[";
  for (int e=1;e<=20;++e) {
    if (e>1) out << ',';const auto edge=TopoDS::Edge(edges(e));const auto& owners=incidence.FindFromKey(edge);const std::array<TopoDS_Face,2> pair={TopoDS::Face(owners.First()),TopoDS::Face(owners.Last())};
    TopLoc_Location curve_location;double first,last;auto curve=BRep_Tool::Curve(edge,curve_location,first,last);require(!curve.IsNull() && curve_location.IsIdentity(),"missing or placed sewn edge curve");
    out << "{\"tolerance_m\":" << BRep_Tool::Tolerance(edge)*1e-3 << ",\"same_parameter\":" << (BRep_Tool::SameParameter(edge) ? "true" : "false") << ",\"same_range\":" << (BRep_Tool::SameRange(edge) ? "true" : "false") << ",\"roles\":[";role(out,identify(pair[0],records));out << ',';role(out,identify(pair[1],records));out << "],\"range\":[" << first << ',' << last << ']';
    const auto r0=identify(pair[0],records),r1=identify(pair[1],records);
    if ((r0.kind==0 && r1.kind==1) || (r0.kind==1 && r1.kind==0)) {
      const auto& cylinder_face=pair[r0.kind==1 ? 0 : 1];double pf,pl;auto pc=Handle(Geom2d_BSplineCurve)::DownCast(BRep_Tool::CurveOnSurface(edge,cylinder_face,pf,pl));require(!pc.IsNull(),"bounded cylindrical pcurve replaced by unknown type");
      auto cylinder=Handle(Geom_CylindricalSurface)::DownCast(BRep_Tool::Surface(cylinder_face));require(!cylinder.IsNull(),"owning cylinder type lost");
      out << ",\"cylinder_frame\":{\"origin_mm\":" << std::showpoint;xyz(out,cylinder->Location(),1);
      out << ",\"directions\":[";xyz(out,cylinder->Position().XDirection(),1);out << ',';xyz(out,cylinder->Position().YDirection(),1);out << ',';xyz(out,cylinder->Position().Direction(),1);
      out << "],\"radius_mm\":" << cylinder->Radius() << '}';
      const auto& blend_face=pair[r0.kind==0 ? 0 : 1];double bf,bl;auto line=Handle(Geom2d_Line)::DownCast(BRep_Tool::CurveOnSurface(edge,blend_face,bf,bl));require(!line.IsNull(),"attached side blend line replaced");
      out << ",\"blend_side_line\":{\"kind\":\"geom2d_line\",\"origin\":[" << line->Location().X() << ',' << line->Location().Y() << "],\"direction\":[" << line->Direction().X() << ',' << line->Direction().Y() << "],\"range\":[" << bf << ',' << bl << "]}" << std::noshowpoint;
      out << ",\"cylindrical_pcurve\":{\"degree\":" << pc->Degree() << ",\"rational\":" << (pc->IsRational() ? "true" : "false") << ",\"periodic\":" << (pc->IsPeriodic() ? "true" : "false") << ",\"poles_rad_m\":[";
      for (int i=1;i<=pc->NbPoles();++i) {if (i>1) out << ',';out << '[' << pc->Pole(i).X() << ',' << pc->Pole(i).Y()*1e-3 << ']';}
      out << "],\"poles_rad_mm\":[" << std::showpoint;for (int i=1;i<=pc->NbPoles();++i) {if (i>1) out << ',';out << '[' << pc->Pole(i).X() << ',' << pc->Pole(i).Y() << ']';}
      out << "],\"weights\":[";for (int i=1;i<=pc->NbPoles();++i) {if (i>1) out << ',';out << pc->Weight(i);}out << std::noshowpoint;
      out << "],\"range\":[" << pf << ',' << pl;
      out << "],\"knots\":[";for (int i=1;i<=pc->NbKnots();++i) {if (i>1) out << ',';out << pc->Knot(i);}
      out << "],\"multiplicities\":[";for (int i=1;i<=pc->NbKnots();++i) {if (i>1) out << ',';out << pc->Multiplicity(i);}out << "]}";
    }
    if ((r0.kind==0 && (r1.kind==1 || r1.kind==2)) || (r1.kind==0 && (r0.kind==1 || r0.kind==2))) {
      auto contact=Handle(Geom_BSplineCurve)::DownCast(curve);require(!contact.IsNull(),"contact representation replaced");
      out << ",\"contact_curve\":{\"location_identity\":true,\"periodic\":" << (contact->IsPeriodic() ? "true" : "false") << ",\"degree\":" << contact->Degree() << ",\"poles_m\":[";
      for (int i=1;i<=contact->NbPoles();++i) {if (i>1) out << ',';xyz(out,contact->Pole(i));}
      out << "],\"poles_mm\":[" << std::showpoint;for (int i=1;i<=contact->NbPoles();++i) {if (i>1) out << ',';xyz(out,contact->Pole(i),1);}out << std::noshowpoint;
      out << "],\"weights\":[";for (int i=1;i<=contact->NbPoles();++i) {if (i>1) out << ',';out << contact->Weight(i);}
      out << "],\"knots\":[";for (int i=1;i<=contact->NbKnots();++i) {if (i>1) out << ',';out << contact->Knot(i);}
      out << "],\"multiplicities\":[";for (int i=1;i<=contact->NbKnots();++i) {if (i>1) out << ',';out << contact->Multiplicity(i);}out << "]}";
    }
    if ((r0.kind==0 && r1.kind==2) || (r0.kind==2 && r1.kind==0)) {
      const auto& plane_face=pair[r0.kind==2 ? 0 : 1];const auto& blend_face=pair[r0.kind==0 ? 0 : 1];
      double pf,pl,bf,bl;auto pc=Handle(Geom2d_BSplineCurve)::DownCast(BRep_Tool::CurveOnSurface(edge,plane_face,pf,pl));
      auto line=Handle(Geom2d_Line)::DownCast(BRep_Tool::CurveOnSurface(edge,blend_face,bf,bl));
      require(!pc.IsNull() && !line.IsNull(),"end contact pcurve representation replaced");
      out << ",\"planar_pcurve\":{\"degree\":" << pc->Degree() << ",\"periodic\":" << (pc->IsPeriodic() ? "true" : "false") << ",\"range\":[" << pf << ',' << pl << "],\"poles_mm\":[" << std::showpoint;
      for (int i=1;i<=pc->NbPoles();++i) {if (i>1) out << ',';out << '[' << pc->Pole(i).X() << ',' << pc->Pole(i).Y() << ']';}
      out << "],\"weights\":[";for (int i=1;i<=pc->NbPoles();++i) {if (i>1) out << ',';out << pc->Weight(i);}
      out << "],\"knots\":[";for (int i=1;i<=pc->NbKnots();++i) {if (i>1) out << ',';out << pc->Knot(i);}
      out << "],\"multiplicities\":[";for (int i=1;i<=pc->NbKnots();++i) {if (i>1) out << ',';out << pc->Multiplicity(i);}out << "]}";
      out << ",\"blend_end_line\":{\"kind\":\"geom2d_line\",\"origin\":[" << line->Location().X() << ',' << line->Location().Y() << "],\"direction\":[" << line->Direction().X() << ',' << line->Direction().Y() << "],\"range\":[" << bf << ',' << bl << "]}" << std::noshowpoint;
    }
    out << ",\"samples\":[";
    for (int j=0;j<=8;++j) {
      if (j) out << ',';const double fraction=j==0 ? 0. : j==8 ? 1. : (j+.37)/9.;const double t=first+(last-first)*fraction;out << "{\"t\":" << t << ",\"point_m\":";xyz(out,curve->Value(t));out << ",\"sides\":[";
      for (int s=0;s<2;++s) {
        if (s) out << ',';double pf,pl;auto pc=BRep_Tool::CurveOnSurface(edge,pair[s],pf,pl);require(!pc.IsNull() && std::abs(pf-first)<1e-12 && std::abs(pl-last)<1e-12,"missing/range-mismatched sewn pcurve");const auto uv=pc->Value(t);auto surface=BRep_Tool::Surface(pair[s]);gp_Pnt p;gp_Vec du,dv;surface->D1(uv.X(),uv.Y(),p,du,dv);auto normal=du.Crossed(dv);normal.Normalize();if (pair[s].Orientation()==TopAbs_REVERSED) normal.Reverse();const auto r=identify(pair[s],records);
        out << "{\"uv\":[" << uv.X()*(r.kind>=2 ? 1e-3 : 1.) << ',' << uv.Y()*(r.kind==0 ? 1. : 1e-3) << "],\"point_m\":";xyz(out,p);out << ",\"normal\":";xyz(out,normal,1);out << '}';
      }
      out << "]}";
    }
    out << "]}";
  }
  gp_Trsf turn;turn.SetRotation(gp_Ax1(gp_Pnt(0,0,0),gp_Dir(0,0,1)),pi/2);BRepBuilderAPI_Transform moved(solid,turn,true);require(moved.IsDone(),"native solid phase transform failed");const auto rotated=TopoDS::Solid(moved.Shape());GProp_GProps rotated_mass;BRepGProp::VolumeProperties(rotated,rotated_mass,1e-11);
  out << "],\"rotated_volume_m3\":" << rotated_mass.Mass()*1e-9 << ",\"classifications\":[";
  for (int q=0;q<4;++q) {
    const double theta=q*pi/2,r=law==0 ? .5 : 1.+.5*std::cos(theta),k=law==0 ? 0. : -.5*std::sin(theta)/(10-r),arc=std::sqrt(2.);const double factor=(k*k*arc+std::sqrt(1+k*k))/(1+2*k*k);const double nr=factor/arc,nz=nr,nt=k*(factor*arc-1);
    gp_Vec normal(nr*std::cos(theta)-nt*std::sin(theta),nr*std::sin(theta)+nt*std::cos(theta),nz);gp_Pnt center((10-r)*std::cos(theta),(10-r)*std::sin(theta),20-r);
    for (int s=0;s<2;++s) {if (q || s) out << ',';const auto p=center.Translated(normal*(r+(s ? .025 : -.025)));out << "{\"theta_rad\":" << theta << ",\"offset_m\":" << (s ? .000025 : -.000025) << ",\"point_m\":";xyz(out,p);out << ",\"state\":" << state(solid,p) << ",\"rotated_state\":" << state(rotated,p) << '}';}
  }
  out << "]}";
}
void run() {int count;require(bool(std::cin>>count) && count==2,"expected two laws");std::ostringstream out;out << std::setprecision(17) << "{\"format_version\":1,\"occt_version\":\"" << OCC_VERSION_COMPLETE << "\",\"cases\":[";for (int law=0;law<2;++law) {if (law) out << ',';measure(out,law);}std::string extra;require(!(std::cin>>extra),"unexpected native input");out << "]}\n";std::cout << out.str();}
}
int main() {try {run();} catch (const Standard_Failure& e) {std::cerr << e.what() << '\n';return 1;} catch (const std::exception& e) {std::cerr << e.what() << '\n';return 1;}}
