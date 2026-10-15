// ADR-092: actual triangular native diagnostic solids, never a product feature.
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <NCollection_IndexedMap.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <NCollection_List.hxx>
#include <TopExp.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Solid.hxx>
#include <Standard_Failure.hxx>
#include <Standard_Version.hxx>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <map>
#include <set>
#include <sstream>
#include <stdexcept>
#include <vector>
#if OCC_VERSION_HEX != 0x080000
#error Pinned OCCT 8.0.0 required
#endif
namespace {
void require(bool b,const char* m){if(!b)throw std::runtime_error(m);}
template<class T>void xyz(std::ostream& o,const T& p,double scale=1e-3){o<<'['<<p.X()*scale<<','<<p.Y()*scale<<','<<p.Z()*scale<<']';}
void measure(std::ostream& out,bool grid){
 int n,nv,nt;require(bool(std::cin>>n>>nv>>nt)&&n>=1&&n<=64&&(n&(n-1))==0&&nv==4*n*(2*n+1)+2&&nt==16*n*n+8*n,"wrong mesh inventory");
 std::vector<gp_Pnt> p;for(int i=0;i<nv;++i){double x,y,z;require(bool(std::cin>>x>>y>>z)&&std::isfinite(x)&&std::isfinite(y)&&std::isfinite(z)&&std::hypot(x,y)<=.01+(grid?1e-9:1e-12)&&z>=-(grid?1e-9:1e-12)&&z<=.02+(grid?1e-9:1e-12),"invalid mesh point/metre units");p.emplace_back(x*1000,y*1000,z*1000);}
 std::vector<std::array<int,3>> triangles;std::map<std::pair<int,int>,std::vector<bool>> uses;std::map<std::array<int,3>,int> records;
 for(int i=0;i<nt;++i){std::array<int,3> t;require(bool(std::cin>>t[0]>>t[1]>>t[2]),"missing triangle");for(int v:t)require(v>=0&&v<nv,"bad vertex ID");require(t[0]!=t[1]&&t[1]!=t[2]&&t[0]!=t[2],"degenerate IDs");triangles.push_back(t);auto key=t;std::sort(key.begin(),key.end());require(records.emplace(key,i).second,"duplicate triangle");for(int j=0;j<3;++j){int a=t[j],b=t[(j+1)%3];uses[{std::min(a,b),std::max(a,b)}].push_back(a<b);}}
 for(const auto& e:uses)require(e.second.size()==2&&e.second[0]!=e.second[1],"input shell winding/incidence invalid; no repair");
 BRepBuilderAPI_Sewing sewing(1e-9); // mm, explicit native numerical tolerance.
 for(const auto& t:triangles){BRepBuilderAPI_MakePolygon wire(p[t[0]],p[t[1]],p[t[2]],true);require(wire.IsDone(),"native triangle wire failed");BRepBuilderAPI_MakeFace face(wire.Wire());require(face.IsDone(),"native triangle face failed");sewing.Add(face.Face());}
 sewing.Perform();auto shell=sewing.SewedShape();require(!shell.IsNull()&&shell.ShapeType()==TopAbs_SHELL&&BRep_Tool::IsClosed(shell)&&BRepCheck_Analyzer(shell).IsValid(),"native shell invalid/open");
 BRepBuilderAPI_MakeSolid made(TopoDS::Shell(shell));require(made.IsDone(),"native solid failed");auto solid=made.Solid();require(BRepCheck_Analyzer(solid).IsValid(),"native solid invalid");
 NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> faces,edges,vertices;
 TopExp::MapShapes(solid,TopAbs_FACE,faces);TopExp::MapShapes(solid,TopAbs_EDGE,edges);TopExp::MapShapes(solid,TopAbs_VERTEX,vertices);
 require(faces.Extent()==nt&&vertices.Extent()==nv&&edges.Extent()*2==3*nt&&nv-edges.Extent()+nt==2,"sewing changed mesh topology");
 NCollection_IndexedDataMap<TopoDS_Shape,NCollection_List<TopoDS_Shape>,TopTools_ShapeMapHasher> incidence;TopExp::MapShapesAndAncestors(solid,TopAbs_EDGE,TopAbs_FACE,incidence);
 std::vector<std::vector<int>> adjacent(nt+1);for(int i=1;i<=edges.Extent();++i){const auto& f=incidence.FindFromKey(edges(i));require(f.Size()==2,"native incidence not two");int a=faces.FindIndex(f.First()),b=faces.FindIndex(f.Last());require(a>0&&b>0&&a!=b,"bad face incidence");adjacent[a].push_back(b);adjacent[b].push_back(a);}
 std::set<int> seen;std::vector<int> pending{1};while(!pending.empty()){int i=pending.back();pending.pop_back();if(seen.insert(i).second)for(int j:adjacent[i])pending.push_back(j);}require(int(seen.size())==nt,"disconnected native shell");
 std::vector<int> matched(nv+1,-1);std::set<int> used;
 for(int i=1;i<=nv;++i){auto point=BRep_Tool::Pnt(TopoDS::Vertex(vertices(i)));int match=-1;for(int j=0;j<nv;++j)if(point.Distance(p[j])<=1e-9){require(match<0,"ambiguous current vertex");match=j;}require(match>=0&&used.insert(match).second,"native current vertices changed");matched[i]=match;}
 GProp_GProps mass,area;BRepGProp::VolumeProperties(solid,mass,1e-11);BRepGProp::SurfaceProperties(solid,area);require(std::isfinite(mass.Mass())&&mass.Mass()>0&&std::isfinite(area.Mass())&&area.Mass()>0,"nonpositive mass; no orientation repair");
 out<<"{\"valid\":true,\"closed\":true,\"connected\":true,\"edge_incidence_two\":true,\"faces\":"<<nt<<",\"vertices\":"<<nv<<",\"edges\":"<<edges.Extent()<<",\"euler\":2,\"free_edges\":"<<sewing.NbFreeEdges()<<",\"volume_m3\":"<<mass.Mass()*1e-9<<",\"area_m2\":"<<area.Mass()*1e-6<<",\"center_of_mass_m\":";xyz(out,mass.CentreOfMass());out<<",\"current_faces\":[";
 std::set<int> face_matches;
 for(int i=1;i<=nt;++i){auto face=TopoDS::Face(faces(i));NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> v;TopExp::MapShapes(face,TopAbs_VERTEX,v);require(v.Extent()==3,"native face no longer triangular");std::array<int,3> ids;for(int j=1;j<=3;++j)ids[j-1]=matched[vertices.FindIndex(v(j))];std::sort(ids.begin(),ids.end());require(records.count(ids),"current native face connectivity changed");int index=records.at(ids);require(face_matches.insert(index).second,"duplicated native face match");
 BRepAdaptor_Surface surface(face);require(surface.GetType()==GeomAbs_Plane,"current native mesh face is not planar");gp_Pnt q;gp_Vec du,dv;surface.D1(0,0,q,du,dv);auto normal=du.Crossed(dv);normal.Normalize();if(face.Orientation()==TopAbs_REVERSED)normal.Reverse();if(i>1)out<<',';out<<"{\"triangle\":"<<index<<",\"normal\":";xyz(out,normal,1);out<<'}';}
 out<<"]}";
}
void run(bool grid){int count;require(bool(std::cin>>count)&&count==2,"expected two actual mesh cases");std::ostringstream out;out<<std::setprecision(17)<<"{\"format_version\":1,\"occt_version\":\""<<OCC_VERSION_COMPLETE<<"\",\"cases\":[";for(int i=0;i<2;++i){if(i)out<<',';measure(out,grid);}std::string extra;require(!(std::cin>>extra),"extra native input");out<<"]}\n";std::cout<<out.str();}
}
int main(int argc,char** argv){try{require(argc==1||(argc==2&&std::string(argv[1])=="--grid"),"unsupported transport mode");run(argc==2);}catch(const Standard_Failure& e){std::cerr<<e.what()<<'\n';return 1;}catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
