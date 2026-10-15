// ADR-078 isolated SDK diagnostic: actual sewn open band, never a product body.
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRep_Tool.hxx>
#include <Geom_BezierSurface.hxx>
#include <NCollection_Array2.hxx>
#include <Standard_Failure.hxx>
#include <Standard_Version.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <NCollection_IndexedMap.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <NCollection_List.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Vertex.hxx>
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
constexpr double sewing_mm=1e-6;
void require(bool b,const char* message) {if(!b) throw std::runtime_error(message);}
void point(std::ostream& out,const gp_Pnt& p) {
  require(std::isfinite(p.X())&&std::isfinite(p.Y())&&std::isfinite(p.Z()),"nonfinite point");
  out<<'['<<p.X()*1e-3<<','<<p.Y()*1e-3<<','<<p.Z()*1e-3<<']';
}
void band(std::ostream& out) {
  int count; require(bool(std::cin>>count)&&count>=4&&count<=4096,"invalid cell count");
  BRepBuilderAPI_Sewing sewing(sewing_mm);
  std::map<const Geom_Surface*,int> indices;
  for(int c=0;c<count;++c) {
    NCollection_Array2<gp_Pnt> poles(1,4,1,4);
    for(int i=1;i<=4;++i) for(int j=1;j<=4;++j) {
      double x,y,z;require(bool(std::cin>>x>>y>>z)&&std::isfinite(x)&&std::isfinite(y)&&std::isfinite(z),"invalid SI pole");
      poles(i,j)=gp_Pnt(x*1e3,y*1e3,z*1e3);
    }
    Handle(Geom_BezierSurface) s=new Geom_BezierSurface(poles);
    indices.emplace(s.get(),c);
    BRepBuilderAPI_MakeFace face(s,1e-9);
    require(face.IsDone(),"native face failed");sewing.Add(face.Face());
  }
  sewing.Perform();auto shape=sewing.SewedShape();
  require(!shape.IsNull()&&BRepCheck_Analyzer(shape).IsValid(),"invalid sewn band");
  NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> faces,edges,vertices,shells,solids;
  TopExp::MapShapes(shape,TopAbs_FACE,faces);TopExp::MapShapes(shape,TopAbs_EDGE,edges);
  TopExp::MapShapes(shape,TopAbs_VERTEX,vertices);TopExp::MapShapes(shape,TopAbs_SHELL,shells);TopExp::MapShapes(shape,TopAbs_SOLID,solids);
  require(faces.Extent()==count&&shells.Extent()==1&&solids.Extent()==0,"wrong native shell inventory");
  NCollection_IndexedDataMap<TopoDS_Shape,NCollection_List<TopoDS_Shape>,TopTools_ShapeMapHasher> incidence;
  TopExp::MapShapesAndAncestors(shape,TopAbs_EDGE,TopAbs_FACE,incidence);
  std::vector<std::vector<int>> adjacency(vertices.Extent()+1);
  std::vector<std::vector<int>> face_adjacency(faces.Extent()+1);
  int boundary_edges=0;
  for(int e=1;e<=edges.Extent();++e) {
    const auto& edge=edges(e);const int uses=incidence.FindFromKey(edge).Size();
    require(uses==1||uses==2,"nonmanifold edge incidence");
    if(uses==2) {
      const auto& owners=incidence.FindFromKey(edge);
      const int a=faces.FindIndex(owners.First()),b=faces.FindIndex(owners.Last());
      require(a>0&&b>0&&a!=b,"invalid interior face incidence");
      face_adjacency[a].push_back(b);face_adjacency[b].push_back(a);
    }
    if(uses==1) {
      auto native=TopoDS::Edge(edge);
      int a=vertices.FindIndex(TopExp::FirstVertex(native)),b=vertices.FindIndex(TopExp::LastVertex(native));
      require(a>0&&b>0&&a!=b,"invalid boundary edge endpoints");
      adjacency[a].push_back(b);adjacency[b].push_back(a);++boundary_edges;
    }
  }
  std::vector<bool> face_seen(faces.Extent()+1,false);
  std::vector<int> face_stack{1};face_seen[1]=true;int reached=0;
  while(!face_stack.empty()) {const int a=face_stack.back();face_stack.pop_back();++reached;for(int b:face_adjacency[a]) if(!face_seen[b]) {face_seen[b]=true;face_stack.push_back(b);}}
  require(reached==faces.Extent(),"disconnected native face graph");
  std::vector<bool> seen(vertices.Extent()+1,false);
  std::vector<int> cycles;
  std::vector<std::vector<int>> loops;
  double max_contact_m=0;
  for(int v=1;v<=vertices.Extent();++v) if(!adjacency[v].empty()) {
    require(adjacency[v].size()==2,"boundary is not degree two");
    const auto p=BRep_Tool::Pnt(TopoDS::Vertex(vertices(v)));
    const double rho=std::hypot(p.X(),p.Y());
    // Side or top support residual; full radius-law grading is external.
    max_contact_m=std::max(max_contact_m,std::min(std::abs(rho-10.),std::abs(p.Z()-20.))*1e-3);
    if(!seen[v]) {
      int size=0;std::vector<int> stack{v},loop;seen[v]=true;
      while(!stack.empty()) {int a=stack.back();stack.pop_back();++size;loop.push_back(a);for(int b:adjacency[a]) if(!seen[b]) {seen[b]=true;stack.push_back(b);}}
      cycles.push_back(size);loops.push_back(loop);
    }
  }
  require(cycles.size()==2&&vertices.Extent()-edges.Extent()+faces.Extent()==0,"wrong annular topology");
  out<<"{\"brep_valid\":true,\"shells\":1,\"solids\":0,\"faces\":"<<faces.Extent()<<",\"edges\":"<<edges.Extent()<<",\"vertices\":"<<vertices.Extent()
     <<",\"boundary_edges\":"<<boundary_edges<<",\"boundary_cycles\":["<<cycles[0]<<','<<cycles[1]<<"],\"boundary_degree_two\":true,\"edge_incidence_valid\":true,\"face_connected\":true,\"euler\":0,\"sewing_tolerance_m\":"<<sewing_mm*1e-3<<",\"max_boundary_support_error_m\":"<<max_contact_m<<",\"surface_cells\":[";
  for(int f=1;f<=faces.Extent();++f) {
    if(f>1)out<<',';auto face=TopoDS::Face(faces(f));
    require(face.Orientation()==TopAbs_FORWARD,"unexpected face orientation");
    TopLoc_Location location;auto surface=BRep_Tool::Surface(face,location);
    require(location.IsIdentity()&&indices.count(surface.get()),"sewing replaced an unrecognized surface");
    auto bezier=Handle(Geom_BezierSurface)::DownCast(surface);require(!bezier.IsNull(),"non-Bezier sewn face");
    out<<"{\"cell\":"<<indices.at(surface.get())<<",\"poles_m\":[";
    for(int i=1;i<=4;++i) {if(i>1)out<<',';out<<'[';for(int j=1;j<=4;++j) {if(j>1)out<<',';point(out,bezier->Pole(i,j));}out<<']';}
    out<<"],\"samples\":[";BRepAdaptor_Surface adaptor(face);
    for(int i=0;i<3;++i)for(int j=0;j<3;++j) {if(i||j)out<<',';out<<"{\"u\":"<<i/2.<<",\"v\":"<<j/2.<<",\"point_m\":";point(out,adaptor.Value(i/2.,j/2.));out<<'}';}
    out<<"]}";
  }
  out<<"],\"boundary_loops_m\":[";
  for(std::size_t i=0;i<loops.size();++i) {if(i)out<<',';out<<'[';for(std::size_t j=0;j<loops[i].size();++j) {if(j)out<<',';point(out,BRep_Tool::Pnt(TopoDS::Vertex(vertices(loops[i][j]))));}out<<']';}
  out<<"]}";
}
}
int main() {
  try {
    int count;require(bool(std::cin>>count)&&count==2,"expected constant and variable covers");
    std::ostringstream out;out<<std::setprecision(17)<<"{\"format_version\":1,\"occt_version\":\""<<OCC_VERSION_COMPLETE<<"\",\"cases\":[";
    band(out);out<<',';band(out);std::string extra;require(!(std::cin>>extra),"unexpected trailing input");out<<"]}\n";std::cout<<out.str();
  } catch(const Standard_Failure& e) {std::cerr<<e.what()<<'\n';return 1;}
    catch(const std::exception& e) {std::cerr<<e.what()<<'\n';return 1;}
}
