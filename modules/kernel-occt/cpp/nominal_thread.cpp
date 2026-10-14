#include "cpp/nominal_thread.h"
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <gp_Trsf.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepLib.hxx>
#include <NCollection_List.hxx>
#include <BRepGProp.hxx>
#include <GProp_GProps.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom2d_Line.hxx>
#include <gp_Ax3.hxx>
#include <gp_Ax2.hxx>
#include <gp_Dir2d.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Solid.hxx>
#include <TopExp_Explorer.hxx>
#include <Standard_Failure.hxx>
#include <cmath>
#include <sstream>
#include <stdexcept>
#include <algorithm>
#include <limits>
namespace musubicad {
rust::Vec<std::uint8_t> nominal_metric_thread(rust::Slice<const double> frame,double major,double p,std::uint32_t turns,bool left){try{
 if(frame.size()!=12 || turns<1 || turns>32 || !std::isfinite(major) || !std::isfinite(p) || major<=0 || major>0.006 || p<0.0005 || p>0.00175)throw std::runtime_error("invalid nominal thread inputs");
 for(double value:frame)if(!std::isfinite(value))throw std::runtime_error("nonfinite nominal thread frame");
 major*=1000;p*=1000;const double l=p*turns;
 const double r=major-5*std::sqrt(3.)*p/16,overlap=0.002,half=7*p/16+overlap/std::sqrt(3.);
 const double spineR=(major+r+overlap)/2;
 BRepBuilderAPI_MakeWire spineMaker;
 const double turnAngle=2*std::acos(-1.);
 for(int turn=0;turn<static_cast<int>(turns)+2;++turn){
 const gp_Ax3 axis(gp_Pnt(0,0,(turn-0.5)*p),gp_Dir(0,0,1),gp_Dir(1,0,0));
 Handle(Geom_CylindricalSurface) surface=new Geom_CylindricalSurface(axis,spineR);
 Handle(Geom2d_Line) uvline=new Geom2d_Line(gp_Pnt2d(0,0),gp_Dir2d(turnAngle,p));
 const TopoDS_Edge edge=BRepBuilderAPI_MakeEdge(uvline,surface,0,std::hypot(turnAngle,p));
 if(!BRepLib::BuildCurve3d(edge,1e-10,GeomAbs_C1,14,2048))throw std::runtime_error("curve approximation failed");
 spineMaker.Add(edge);
 }
 const TopoDS_Wire spine=spineMaker.Wire();
 BRepBuilderAPI_MakePolygon profile;profile.Add(gp_Pnt(major+overlap,0,-p/2+half));profile.Add(gp_Pnt(r,0,-p/2+p/8));profile.Add(gp_Pnt(r,0,-p/2-p/8));profile.Add(gp_Pnt(major+overlap,0,-p/2-half));profile.Close();
 BRepOffsetAPI_MakePipeShell pipe(spine);pipe.SetMode(true);pipe.SetTolerance(1e-9,1e-9,1e-8);pipe.SetMaxDegree(14);pipe.SetMaxSegments(2048);auto wire=profile.Wire();wire.Reverse();pipe.Add(wire,false,false);pipe.Build();
 if(!pipe.IsDone()||!pipe.MakeSolid())throw std::runtime_error("sweep failed");
 TopoDS_Solid ridge=TopoDS::Solid(pipe.Shape());if(!BRepLib::OrientClosedSolid(ridge)||!BRepCheck_Analyzer(ridge).IsValid())throw std::runtime_error("invalid ridge");
 BRepLib::SameParameter(ridge,1e-7,true);auto root=BRepPrimAPI_MakeCylinder(major,l).Shape();BRepAlgoAPI_Cut fuse;NCollection_List<TopoDS_Shape> args,tools;args.Append(root);tools.Append(ridge);fuse.SetArguments(args);fuse.SetTools(tools);fuse.SetNonDestructive(true);fuse.SetRunParallel(false);fuse.Build();if(!fuse.IsDone()||!BRepCheck_Analyzer(fuse.Shape()).IsValid())throw std::runtime_error("invalid fused thread");
 auto guard=BRepPrimAPI_MakeBox(gp_Pnt(-2*major,-2*major,0),4*major,4*major,l).Shape();BRepAlgoAPI_Common common(fuse.Shape(),guard);common.SetRunParallel(false);common.Build();if(!common.IsDone()||!BRepCheck_Analyzer(common.Shape()).IsValid())throw std::runtime_error("invalid clipped thread");
 int count=0;TopoDS_Solid solid;for(TopExp_Explorer e(common.Shape(),TopAbs_SOLID);e.More();e.Next()){++count;solid=TopoDS::Solid(e.Current());}if(count!=1)throw std::runtime_error("thread must be one solid");
 if(!BRepLib::OrientClosedSolid(solid))throw std::runtime_error("thread winding invalid");
 if(left){gp_Trsf mirror;mirror.SetMirror(gp_Ax2(gp_Pnt(0,0,0),gp_Dir(0,1,0)));solid=TopoDS::Solid(BRepBuilderAPI_Transform(solid,mirror,true).Shape());if(!BRepLib::OrientClosedSolid(solid)||!BRepCheck_Analyzer(solid).IsValid())throw std::runtime_error("mirrored thread invalid");}
 gp_Trsf units;units.SetScale(gp_Pnt(0,0,0),0.001);const auto local=BRepBuilderAPI_Transform(solid,units,true).Shape();
 gp_Trsf placement;placement.SetValues(frame[3],frame[6],frame[9],frame[0],frame[4],frame[7],frame[10],frame[1],frame[5],frame[8],frame[11],frame[2]);
 const auto si=BRepBuilderAPI_Transform(local,placement,true).Shape();if(!BRepCheck_Analyzer(si).IsValid())throw std::runtime_error("scaled output invalid");GProp_GProps properties,area;BRepGProp::VolumeProperties(si,properties,1e-9,true,false);BRepGProp::SurfaceProperties(si,area,1e-9,false);
 const double rp=r/1000,mp=major/1000,pp=p/1000,lp=l/1000,a=pp/(2*std::acos(-1.));
 const double section=std::acos(-1.)*(11*rp*rp+5*rp*mp+8*mp*mp)/24,expectedVolume=section*lp;
 const auto integral=[&](double x){return x*std::sqrt(4*x*x+3*a*a)/2+3*a*a/4*std::asinh(2*x/(std::sqrt(3.)*a));};
 const double expectedArea=2*std::acos(-1.)*turns*(pp*(rp/4+mp/8)+2/std::sqrt(3.)*(integral(mp)-integral(rp)))+2*section;
 const gp_Pnt expectedCenter(frame[0]+frame[9]*lp/2,frame[1]+frame[10]*lp/2,frame[2]+frame[11]*lp/2);
 const double coordinateScale=std::max({1.0,std::abs(frame[0]),std::abs(frame[1]),std::abs(frame[2])});
 if(!std::isfinite(properties.Mass())||std::abs(properties.Mass()-expectedVolume)>expectedVolume*2e-5
 ||!std::isfinite(area.Mass())||std::abs(area.Mass()-expectedArea)>expectedArea*2e-5
 ||properties.CentreOfMass().Distance(expectedCenter)>2e-8+64*std::numeric_limits<double>::epsilon()*coordinateScale)
 {std::ostringstream message;message<<"nominal thread properties disagree: turns="<<turns<<" volume relative="<<std::abs(properties.Mass()/expectedVolume-1)<<" area relative="<<std::abs(area.Mass()/expectedArea-1)<<" COM error m="<<properties.CentreOfMass().Distance(expectedCenter);throw std::runtime_error(message.str());}
 std::ostringstream out;BRepTools::Write(si,out);rust::Vec<std::uint8_t> bytes;for(unsigned char c:out.str())bytes.push_back(c);return bytes;
 }catch(const Standard_Failure& e){throw std::runtime_error(e.what());}}

rust::Vec<double> nominal_thread_properties(rust::Slice<const std::uint8_t> brep) {
 try {
 std::istringstream stream(std::string(reinterpret_cast<const char*>(brep.data()),brep.size()));TopoDS_Shape shape;BRep_Builder builder;BRepTools::Read(shape,stream,builder);
 if(shape.IsNull()||!BRepCheck_Analyzer(shape).IsValid())throw std::runtime_error("invalid thread property body");
 GProp_GProps volume,area;BRepGProp::VolumeProperties(shape,volume,1e-9,true,false);BRepGProp::SurfaceProperties(shape,area,1e-9,false);
 if(!std::isfinite(volume.Mass())||volume.Mass()<=0||!std::isfinite(area.Mass())||area.Mass()<=0)throw std::runtime_error("invalid measured thread properties");
 rust::Vec<double> out;out.push_back(volume.Mass());out.push_back(area.Mass());const auto c=volume.CentreOfMass();
 for(double value:{c.X(),c.Y(),c.Z()}){if(!std::isfinite(value))throw std::runtime_error("nonfinite thread center");out.push_back(value);}return out;
 }catch(const Standard_Failure& e){throw std::runtime_error(e.what());}
}

}
