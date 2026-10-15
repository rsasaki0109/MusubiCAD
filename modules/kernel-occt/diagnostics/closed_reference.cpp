// ADR-079: isolated closed polynomial approximation, not a product fillet.
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <Geom_BezierCurve.hxx>
#include <Geom_BezierSurface.hxx>
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
#include <algorithm>
#include <cmath>
#include <gp_Pln.hxx>
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
void require(bool condition, const char* message) {
  if (!condition) throw std::runtime_error(message);
}
void point(std::ostream& out, const gp_Pnt& p) {
  require(std::isfinite(p.X()) && std::isfinite(p.Y()) && std::isfinite(p.Z()), "nonfinite native point");
  out << '[' << p.X()*1e-3 << ',' << p.Y()*1e-3 << ',' << p.Z()*1e-3 << ']';
}
struct SurfaceRecord { int band_cell; int skirt_index; };
TopoDS_Edge boundary(const Handle(Geom_BezierSurface)& s, int v) {
  NCollection_Array1<gp_Pnt> poles(1,4);
  for (int i=1;i<=4;++i) poles(i)=s->Pole(i,v);
  return BRepBuilderAPI_MakeEdge(new Geom_BezierCurve(poles)).Edge();
}
void measure(std::ostream& out) {
  int n; require(bool(std::cin>>n) && (n==16 || n==32), "unexpected fixture divisions");
  const int count=4*n*n, skirts=4*n;
  BRepBuilderAPI_Sewing sewing(1e-6); // millimetres, diagnostic only.
  std::map<const Geom_Surface*,SurfaceRecord> records;
  BRepBuilderAPI_MakeWire top_wire, bottom_wire;
  auto add_surface=[&](const Handle(Geom_BezierSurface)& s, int cell, int skirt) {
    BRepBuilderAPI_MakeFace face(s,1e-9);
    require(face.IsDone(),"native face construction failed");
    records.emplace(s.get(),SurfaceRecord{cell,skirt});sewing.Add(face.Face());
  };
  for (int c=0;c<count;++c) {
    NCollection_Array2<gp_Pnt> poles(1,4,1,4);
    for (int i=1;i<=4;++i) for (int j=1;j<=4;++j) {
      double x,y,z;
      require(bool(std::cin>>x>>y>>z) && std::isfinite(x) && std::isfinite(y) && std::isfinite(z),"invalid SI poles");
      poles(i,j)=gp_Pnt(x*1e3,y*1e3,z*1e3);
    }
    Handle(Geom_BezierSurface) band=new Geom_BezierSurface(poles);
    add_surface(band,c,-1);
    if (c%n==0) {
      NCollection_Array2<gp_Pnt> side(1,4,1,4);
      for (int i=1;i<=4;++i) for (int j=1;j<=4;++j) {
        const auto p=poles(i,1);
        // Preserve the actual shared upper pole, without multiply/divide drift.
        side(i,j)=gp_Pnt(p.X(),p.Y(),j==4 ? p.Z() : p.Z()*(j-1)/3.);
      }
      Handle(Geom_BezierSurface) skirt=new Geom_BezierSurface(side);
      add_surface(skirt,-1,c/n);bottom_wire.Add(boundary(skirt,1));
    }
    if (c%n==n-1) top_wire.Add(boundary(band,4));
  }
  require(top_wire.IsDone() && bottom_wire.IsDone(),"native cap wires failed");
  BRepBuilderAPI_MakeFace top(gp_Pln(gp_Pnt(0,0,20),gp_Dir(0,0,1)),top_wire.Wire(),true);
  BRepBuilderAPI_MakeFace bottom(gp_Pln(gp_Pnt(0,0,0),gp_Dir(0,0,1)),bottom_wire.Wire(),true);
  require(top.IsDone() && bottom.IsDone(),"native cap construction failed");
  auto bottom_face=bottom.Face();bottom_face.Reverse();
  sewing.Add(top.Face());sewing.Add(bottom_face);sewing.Perform();
  auto shell=sewing.SewedShape();
  require(!shell.IsNull() && shell.ShapeType()==TopAbs_SHELL && BRepCheck_Analyzer(shell).IsValid(),"invalid sewn shell");
  require(BRep_Tool::IsClosed(shell),"native shell is not closed");
  BRepBuilderAPI_MakeSolid make_solid(TopoDS::Shell(shell));
  require(make_solid.IsDone(),"native solid construction failed");
  const auto solid=make_solid.Solid();
  require(BRepCheck_Analyzer(solid).IsValid(),"invalid native solid");
  NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> faces,edges,vertices,shells,solids;
  TopExp::MapShapes(solid,TopAbs_FACE,faces);TopExp::MapShapes(solid,TopAbs_EDGE,edges);
  TopExp::MapShapes(solid,TopAbs_VERTEX,vertices);TopExp::MapShapes(solid,TopAbs_SHELL,shells);TopExp::MapShapes(solid,TopAbs_SOLID,solids);
  require(faces.Extent()==count+skirts+2 && shells.Extent()==1 && solids.Extent()==1,"wrong closed topology inventory");
  NCollection_IndexedDataMap<TopoDS_Shape,NCollection_List<TopoDS_Shape>,TopTools_ShapeMapHasher> incidence;
  TopExp::MapShapesAndAncestors(solid,TopAbs_EDGE,TopAbs_FACE,incidence);
  std::vector<std::vector<int>> adjacency(faces.Extent()+1);
  for (int e=1;e<=edges.Extent();++e) {
    const auto& owners=incidence.FindFromKey(edges(e));
    require(owners.Size()==2,"free or nonmanifold closed-solid edge");
    const int a=faces.FindIndex(owners.First()),b=faces.FindIndex(owners.Last());
    require(a>0 && b>0 && a!=b,"invalid closed-solid incidence");
    adjacency[a].push_back(b);adjacency[b].push_back(a);
  }
  std::vector<bool> seen(faces.Extent()+1,false);std::vector<int> stack{1};seen[1]=true;int reached=0;
  while (!stack.empty()) {int a=stack.back();stack.pop_back();++reached;for (int b:adjacency[a]) if (!seen[b]) {seen[b]=true;stack.push_back(b);}}
  require(reached==faces.Extent() && vertices.Extent()-edges.Extent()+faces.Extent()==2,"invalid connected closed surface topology");
  GProp_GProps coarse,fine;BRepGProp::VolumeProperties(solid,coarse,1e-9);BRepGProp::VolumeProperties(solid,fine,1e-11);
  require(std::isfinite(fine.Mass()) && fine.Mass()>0,"nonpositive native volume; no orientation repair permitted");
  out << "{\"brep_valid\":true,\"shell_closed\":true,\"face_connected\":true,\"edge_incidence_two\":true,\"free_edges\":0,\"shells\":1,\"solids\":1,\"euler\":2,\"faces\":" << faces.Extent()
      << ",\"vertices\":" << vertices.Extent() << ",\"edges\":" << edges.Extent() << ",\"volume_m3\":" << fine.Mass()*1e-9
      << ",\"volume_refinement_delta_m3\":" << std::abs(fine.Mass()-coarse.Mass())*1e-9;
  const auto center=fine.CentreOfMass();
  out << ",\"center_of_mass_m\":";point(out,center);
  out << ",\"first_moments_m4\":[" << fine.Mass()*center.X()*1e-12 << ',' << fine.Mass()*center.Y()*1e-12 << ',' << fine.Mass()*center.Z()*1e-12 << "],\"surface_cells\":[";
  int planes=0, emitted=0;double radial_error=0, curved_area=0;double cap_areas[2]={0,0};
  GProp_GProps total_area;BRepGProp::SurfaceProperties(solid,total_area,1e-11);
  for (int f=1;f<=faces.Extent();++f) {
    auto face=TopoDS::Face(faces(f));BRepAdaptor_Surface adaptor(face);
    GProp_GProps face_area;BRepGProp::SurfaceProperties(face,face_area,1e-11);
    const double area=face_area.Mass()*1e-6;
    require(std::isfinite(area) && area>0,"nonpositive native face area");
    if (adaptor.GetType()==GeomAbs_Plane) {
      ++planes;
      const auto plane=adaptor.Plane();const double z=plane.Location().Z();
      require(std::abs(z)<1e-7 || std::abs(z-20)<1e-7,"wrong cap plane");
      auto normal=plane.Axis().Direction();
      if (face.Orientation()==TopAbs_REVERSED) normal.Reverse();
      require((z<10 && normal.Z()<-0.99) || (z>10 && normal.Z()>0.99),"wrong cap orientation");
      cap_areas[z<10 ? 0 : 1]=area;
      continue;
    }
    require(face.Orientation()==TopAbs_FORWARD,"wrong band/skirt orientation");
    TopLoc_Location location;auto s=BRep_Tool::Surface(face,location);
    require(location.IsIdentity() && records.count(s.get()),"unrecognized sewn surface");
    const auto record=records.at(s.get());curved_area+=area;if (emitted++) out << ',';
    out << "{\"band_cell\":" << record.band_cell << ",\"skirt_index\":" << record.skirt_index << ",\"area_m2\":" << area << ",\"samples\":[";
    for (int i=0;i<3;++i) for (int j=0;j<3;++j) {
      if (i || j) out << ',';const auto p=adaptor.Value(i/2.,j/2.);
      out << "{\"u\":" << i/2. << ",\"v\":" << j/2. << ",\"point_m\":";point(out,p);out << '}';
      if (record.skirt_index>=0) radial_error=std::max(radial_error,std::abs(std::hypot(p.X(),p.Y())-10)*1e-3);
    }
    out << "]}";
  }
  require(planes==2,"wrong native cap count");
  out << "],\"planar_caps\":2,\"cap_normals_outward\":true,\"max_sampled_skirt_cylinder_error_m\":" << radial_error
      << ",\"curved_surface_area_m2\":" << curved_area << ",\"planar_cap_areas_m2\":[" << cap_areas[0] << ',' << cap_areas[1]
      << "],\"total_surface_area_m2\":" << total_area.Mass()*1e-6 << '}';
}
}
int main() {
  try {
    int count;require(bool(std::cin>>count) && count==4,"expected two laws and two refinement levels");
    std::ostringstream out;out << std::setprecision(17) << "{\"format_version\":1,\"occt_version\":\"" << OCC_VERSION_COMPLETE << "\",\"cases\":[";
    for (int i=0;i<count;++i) {if (i) out << ',';measure(out);}
    std::string extra;require(!(std::cin>>extra),"unexpected trailing input");out << "]}\n";std::cout << out.str();
  } catch (const Standard_Failure& e) {std::cerr << e.what() << '\n';return 1;}
    catch (const std::exception& e) {std::cerr << e.what() << '\n';return 1;}
}
