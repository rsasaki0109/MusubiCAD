"""ADR-092: actual bounded reference triangle shells and native diagnostic solids."""
from __future__ import annotations
import argparse
from collections import Counter, defaultdict
import copy
import json
import math
from pathlib import Path
import shutil
import subprocess
import tempfile
from run_reference_bezier_probe import ROOT, distance, number, require, vector, unit_cross
from run_rational_reference_probe import envelope

POSITION_M = 1e-12
NORMAL = 1e-9
VOLUME_M3 = 1e-15
AREA_M2 = 1e-12
MOMENT_M4 = 1e-17
COM_M = 1e-10
REFUSALS = ("missing_input", "wrong_units", "missing_triangle", "reversed_winding")


def measured(mesh):
    """Independent signed tetrahedra and actual facet areas, never ideal mass."""
    positions = [vector(p) for p in mesh["vertices_m"]]
    volumes, areas, moments, normals = [], [], [[], [], []], []
    for t in mesh["triangles"]:
        require(len(t) == 3 and all(type(i) is int and 0 <= i < len(positions) for i in t) and len(set(t)) == 3, "invalid triangle IDs")
        a,b,c = [positions[i] for i in t]
        normal, size = unit_cross([b[k]-a[k] for k in range(3)], [c[k]-a[k] for k in range(3)])
        v = sum(a[k]*(b[(k+1)%3]*c[(k+2)%3]-b[(k+2)%3]*c[(k+1)%3]) for k in range(3))/6
        volumes.append(v);areas.append(size/2);normals.append(normal)
        for k in range(3): moments[k].append(v*(a[k]+b[k]+c[k])/4)
    return math.fsum(volumes), math.fsum(areas), [math.fsum(x) for x in moments], normals


def grade_mesh(mesh, law, quality):
    n=mesh["divisions"];require(type(n) is int and 1<=n<=64 and n&(n-1)==0, "invalid dyadic grid")
    k=4*n;stride=2*n+1
    require(len(mesh["vertices_m"])==k*stride+2 and len(mesh["triangles"])==len(mesh["face_groups"])==16*n*n+8*n, "incomplete closed mesh")
    p,normal_request=(4e-4,0.8) if quality=="coarse" else (1e-4,0.5)
    require(mesh["quality"]==quality and number(mesh["requested_position_error_m"])==p and number(mesh["requested_normal_error"])==normal_request and type(mesh["max_vertices"]) is int and mesh["max_vertices"]==33026 and type(mesh["max_triangles"]) is int and mesh["max_triangles"]==66048, "wrong mesh precision/work request")
    error,normal_error=number(mesh["position_error_upper_m"]),number(mesh["normal_error_upper"])
    require(0<error<=p and 0<normal_error<=normal_request, "unresolved whole mesh quality")
    volume,area,moments,normals=measured(mesh)
    require(volume>0 and area>0, "nonpositive boundary measurements")
    edges=defaultdict(list);used=set();expected=[];groups=[];witnesses=[]
    def idx(i,j): return (i%k)*stride+j
    def point(u,w,chart,band):
        theta=chart*math.pi/2+2*math.atan(u);r=.0005 if law==0 else .001+.0005*math.cos(theta)
        if band:return envelope(law,theta,2*math.atan(w))
        return [.01*math.cos(theta),.01*math.sin(theta),w*(.02-r)], [math.cos(theta),math.sin(theta),0.]
    for i in range(k):
        chart=i//n;u0=(i%n)/n
        for j in range(2*n):
            ids=[idx(i,j),idx(i+1,j),idx(i+1,j+1),idx(i,j+1)]
            for t,fu,fv in (([ids[0],ids[1],ids[2]],2/3,1/3),([ids[0],ids[2],ids[3]],1/3,2/3)):
                expected.append(t);groups.append((5 if j<n else 1)+chart)
                witnesses.append(point(u0+fu/n,((j if j<n else j-n)+fv)/n,chart,j>=n))
        theta=chart*math.pi/2+2*math.atan(u0+.5/n);r=.0005 if law==0 else .001+.0005*math.cos(theta)
        for t,g,rho,z,nz in (([idx(i,2*n),idx(i+1,2*n),k*stride],9,.01-r,.02,1.),([idx(i+1,0),idx(i,0),k*stride+1],10,.01,0.,-1.)):
            expected.append(t);groups.append(g);witnesses.append(([2/3*rho*math.cos(theta),2/3*rho*math.sin(theta),z],[0.,0.,nz]))
    require(mesh["triangles"]==expected and mesh["face_groups"]==groups and all(type(g) is int for g in mesh["face_groups"]), "shared IDs/diagonals/face groups changed")
    for face,(t,(ideal,n)) in enumerate(zip(mesh["triangles"],witnesses)):
        centre=[sum(mesh["vertices_m"][i][a] for i in t)/3 for a in range(3)]
        require(distance(centre,ideal)<=error+POSITION_M and distance(normals[face],n)<=normal_error+NORMAL and sum(x*y for x,y in zip(normals[face],n))>0, "mesh witness outside whole cell position/normal bounds")
        for a,b in zip(t,t[1:]+t[:1]):edges[min(a,b),max(a,b)].append((face,a<b));used.add(a)
    require(len(used)==len(mesh["vertices_m"]) and all(len(v)==2 and v[0][1]!=v[1][1] for v in edges.values()) and len(used)-len(edges)+len(expected)==2, "mesh not a closed oriented sphere topology")
    adjacency=defaultdict(list)
    for v in edges.values():a,b=v[0][0],v[1][0];adjacency[a].append(b);adjacency[b].append(a)
    seen=set();pending=[0]
    while pending:
        face=pending.pop()
        if face not in seen:seen.add(face);pending.extend(adjacency[face])
    require(len(seen)==len(expected), "disconnected shell")
    values=mesh["boundary_integrals"]
    for value,limits,tol in [(volume,values["volume_m3"],VOLUME_M3),(area,values["area_m2"],AREA_M2),*[(moments[i],values["first_moments_m4"][i],MOMENT_M4) for i in range(3)]]:
        require(len(limits)==2 and number(limits[0])<=value+tol and value-tol<=number(limits[1]) and 0<=limits[1]-limits[0]<=tol, "actual triangle integrals outside certified flux interval")
    return volume,area,moments,normals


def grade(reference,native):
    for x in (reference,native):require(type(x["format_version"]) is int and x["format_version"]==1 and len(x["cases"])==2,"wrong report inventory")
    require(native["occt_version"]=="8.0.0" and number(reference["radius_m"])==.01 and number(reference["height_m"])==.02,"wrong stock/SDK")
    require(all(reference[k] is False for k in ("global_embedding_certified","native_floating_evaluation_certified","product_body_created")),"overclaimed shell admission")
    require(set(native["process_refusals"])==set(REFUSALS) and all(v is True for v in native["process_refusals"].values()),"missing real process refusals")
    summaries=[]
    for law,(case,actual) in enumerate(zip(reference["cases"],native["cases"])):
        require(case["law"]==("variable" if law else "constant") and number(case["minimum_radius_m"])==.0005 and number(case["maximum_radius_m"])==(.0015 if law else .0005) and len(case["meshes"])==2,"wrong reference law")
        lo,hi=map(number,case["ideal_volume_m3"]);require(0<lo<=hi and hi-lo<=1e-12,"invalid independent ideal volume enclosure")
        if law==0:
            ideal=math.pi*.01**2*.02-2*math.pi*((.01-.0005)*(1-math.pi/4)*.0005**2+.0005**3/6)
            require(lo-1e-20<=ideal<=hi+1e-20,"independent constant ideal volume mismatch")
        results=[grade_mesh(m,law,q) for m,q in zip(case["meshes"],("coarse","fine"))]
        coarse,fine=case["meshes"];v,a,mom,normals=results[1];n=fine["divisions"];nv=len(fine["vertices_m"]);nt=len(fine["triangles"])
        require(n>coarse["divisions"] and max(abs(results[1][0]-lo),abs(results[1][0]-hi))<max(abs(results[0][0]-lo),abs(results[0][0]-hi)),"no independent volume refinement")
        for key,value in (("faces",nt),("vertices",nv),("edges",nt*3//2),("euler",2),("free_edges",0)):
            require(type(actual[key]) is int and actual[key]==value,"wrong native mesh topology")
        require(all(actual[k] is True for k in ("valid","closed","connected","edge_incidence_two")),"invalid/open native solid")
        require(abs(number(actual["volume_m3"])-v)<=VOLUME_M3 and abs(number(actual["area_m2"])-a)<=AREA_M2 and distance(actual["center_of_mass_m"],[x/v for x in mom])<=COM_M,"native physical measurements disagree with actual triangle shell")
        require(all(abs(number(actual["volume_m3"])*vector(actual["center_of_mass_m"])[k]-mom[k])<=MOMENT_M4 for k in range(3)),"native first moments disagree with actual triangle shell")
        require(len(actual["current_faces"])==nt and Counter(f["triangle"] for f in actual["current_faces"])==Counter(range(nt)),"missing current native faces")
        for f in actual["current_faces"]:
            require(type(f["triangle"]) is int and distance(f["normal"],normals[f["triangle"]])<=NORMAL,"current native face normal changed")
        summaries.append({"law":case["law"],"divisions":n,"vertices":nv,"triangles":nt,"position_error_upper_m":fine["position_error_upper_m"],"normal_error_upper":fine["normal_error_upper"],"native_volume_m3":actual["volume_m3"]})
    return {"diagnostic_passed":True,"product_body_created":False,"global_embedding_certified":False,"cases":summaries}


def grid_reference(reference):
    """Bind the existing native transport to actual admitted vertices/face order."""
    out=copy.deepcopy(reference)
    for case in out['cases']:
        for m in case['meshes']:
            m.update({k:m['grid_solid'][k] for k in ('vertices_m','triangles')})
    return out


def grade_grid(reference,native):
    """Independent conversion witnesses/mass; no exact global intersection proof."""
    require(type(native['format_version']) is int and native['format_version']==1 and native['occt_version']=='8.0.0' and len(native['cases'])==2,'wrong grid native inventory')
    require(set(native['process_refusals'])==set(REFUSALS) and all(v is True for v in native['process_refusals'].values()),'missing grid process refusals')
    summaries=[]
    for case,actual in zip(reference['cases'],native['cases']):
        for raw in case['meshes']:
            g=raw['grid_solid'];require(g['numeric_triangle_solid_admitted'] is True and g['exact_global_embedding_certified'] is False,'overclaimed grid admission')
            require(number(g['vertex_grid_m'])==1e-9 and type(g['triangle_limit']) is int and g['triangle_limit']==10000,'changed grid/budget')
            require(len(g['vertices_m'])==len(raw['vertices_m']) and len(g['triangles'])==len(raw['triangles'])<=10000,'grid changed inventory')
            vol,area,moment,normals=measured(g);_,_,_,old_normals=measured(raw)
            forward={};reverse={};delta=0.
            for a,b in zip(raw['triangles'],g['triangles']):
                for i,j in zip(a,b):
                    require(forward.setdefault(i,j)==j and reverse.setdefault(j,i)==i,'grid correspondence not bijective')
                    p=raw['vertices_m'][i];q=g['vertices_m'][j]
                    expected=[]
                    for v in p:
                        scaled=abs(v/1e-9);base=math.floor(scaled)
                        expected.append(math.copysign(float(base+(scaled-base>=.5))*1e-9,v))
                    # Exact representational binding to documented binary64 rounding.
                    require(all(x==y for x,y in zip(q,expected)),'grid vertex not actual rounded input')
                    delta=max(delta,distance(p,q))
            require(len(forward)==len(raw['vertices_m']) and len(reverse)==len(g['vertices_m']),'incomplete grid correspondence')
            d=number(g['grid_displacement_upper_m']);e=number(g['grid_normal_change_upper'])
            require(0<d<=1e-9 and delta<=d+1e-22 and 0<e<1 and all(distance(a,b)<=e+1e-14 for a,b in zip(old_normals,normals)),'uncertified grid conversion differences')
            position=number(g['position_error_upper_m']);normal=number(g['normal_error_upper'])
            require(raw['position_error_upper_m']+d<=position<=raw['requested_position_error_m'] and raw['normal_error_upper']+e<=normal<=raw['requested_normal_error'],'final grid quality not preserved')
            require(abs(number(g['volume_m3'])-vol)<=VOLUME_M3 and abs(number(g['area_m2'])-area)<=AREA_M2 and distance(g['center_of_mass_m'],[x/vol for x in moment])<=COM_M,'grid solid mass mismatch')
        fine=case['meshes'][1];g=fine['grid_solid'];v,a,mom,normals=measured(g);nt=len(g['triangles']);nv=len(g['vertices_m'])
        for key,value in (('faces',nt),('vertices',nv),('edges',nt*3//2),('euler',2),('free_edges',0)):
            require(type(actual[key]) is int and actual[key]==value,'wrong grid native topology')
        require(all(actual[k] is True for k in ('valid','closed','connected','edge_incidence_two')),'invalid grid native solid')
        require(abs(number(actual['volume_m3'])-v)<=VOLUME_M3 and abs(number(actual['area_m2'])-a)<=AREA_M2 and distance(actual['center_of_mass_m'],[x/v for x in mom])<=COM_M,'grid native mass mismatch')
        require(all(abs(number(actual['volume_m3'])*vector(actual['center_of_mass_m'])[k]-mom[k])<=MOMENT_M4 for k in range(3)),'grid native first moments mismatch')
        require(len(actual['current_faces'])==nt and Counter(f['triangle'] for f in actual['current_faces'])==Counter(range(nt)),'missing grid native faces')
        for f in actual['current_faces']:
            require(type(f['triangle']) is int and distance(f['normal'],normals[f['triangle']])<=NORMAL,'grid native face normal mismatch')
        summaries.append({'law':case['law'],'position_error_upper_m':g['position_error_upper_m'],'normal_error_upper':g['normal_error_upper'],'grid_displacement_upper_m':g['grid_displacement_upper_m'],'grid_normal_change_upper':g['grid_normal_change_upper'],'native_volume_m3':actual['volume_m3']})
    return summaries


def native_input(reference):
    values=['2']
    for case in reference["cases"]:
        m=case["meshes"][1];values.extend(map(str,(m["divisions"],len(m["vertices_m"]),len(m["triangles"]))))
        for p in m["vertices_m"]:values.extend(format(x,'.17g') for x in p)
        for t in m["triangles"]:values.extend(map(str,t))
    return '\n'.join(values)+'\n'


def run(sdk,output):
    result=subprocess.run(['cargo','run','--quiet','-p','opencad-geometry','--example','periodic_fillet_closed_mesh'],cwd=ROOT,check=True,text=True,capture_output=True,timeout=180)
    ref=json.loads(result.stdout)
    with tempfile.TemporaryDirectory(prefix='closed-reference-mesh-') as directory:
        binary=Path(directory)/'probe';libs=['TKTopAlgo','TKShHealing','TKGeomAlgo','TKBRep','TKGeomBase','TKG3d','TKG2d','TKMath','TKernel']
        subprocess.run([shutil.which('c++') or 'c++','-O2','-std=c++17','-I',str(sdk/'include/opencascade'),str(ROOT/'modules/kernel-occt/diagnostics/closed_reference_mesh.cpp'),'-L',str(sdk/'lib'),'-Wl,--start-group',*[f'-l{x}' for x in libs],'-Wl,--end-group','-lpthread','-ldl','-o',str(binary)],check=True,timeout=180)
        def native_pass(reference, mode):
            command=[str(binary)]+(['--grid'] if mode=='grid' else [])
            payload=native_input(reference)
            result=subprocess.run(command,input=payload,text=True,capture_output=True,check=True,timeout=180)
            native=json.loads(result.stdout)
            changes={'missing_input':payload[:len(payload)//2]}
            for name in REFUSALS[1:]:
                bad=copy.deepcopy(reference);m=bad['cases'][0]['meshes'][1]
                if name=='wrong_units':m['vertices_m']=[[x*1000 for x in p] for p in m['vertices_m']]
                elif name=='missing_triangle':m['triangles'].pop()
                else:m['triangles']=[list(reversed(t)) for t in m['triangles']]
                changes[name]=native_input(bad)
            native['process_refusals']={}
            for name,payload in changes.items():
                refused=subprocess.run(command,input=payload,text=True,capture_output=True,timeout=180)
                require(refused.returncode!=0 and not refused.stdout.strip(),'invalid native input/partial stdout: '+mode+'/'+name);native['process_refusals'][name]=True
            return native
        native=native_pass(ref,'raw')
        grid_native=native_pass(grid_reference(ref),'grid')
    report={'format_version':1,'diagnostic_passed':False,'reference':ref,'native':native,'grid_native':grid_native}
    try:
        passed=grade(ref,native)
        report['grid_cases']=grade_grid(ref,grid_native)
        report.update(passed)
    finally:output.parent.mkdir(parents=True,exist_ok=True);output.write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    print('closed periodic reference mesh/native diagnostic passed; product unsupported; report='+str(output))


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--occt-root',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    try:run(a.occt_root.resolve(),a.output.resolve())
    except (KeyError,ValueError,TypeError,OSError,subprocess.SubprocessError) as e:p.exit(1,'closed reference mesh diagnostic failed: '+str(e)+'\n')
if __name__=='__main__':main()
