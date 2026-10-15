"""Synthetic parser fixtures only; real native construction is a separate probe."""
import copy
import math
import unittest
from run_closed_reference_mesh_probe import grade, grade_grid, measured, REFUSALS
from run_rational_reference_probe import envelope


def fixture():
    ref={"format_version":1,"radius_m":.01,"height_m":.02,"global_embedding_certified":False,"native_floating_evaluation_certified":False,"product_body_created":False,"cases":[]}
    native={"format_version":1,"occt_version":"8.0.0","process_refusals":{k:True for k in REFUSALS},"cases":[]}
    for law in range(2):
        meshes=[]
        for quality,n in (("coarse",8),("fine",16)):
            k=4*n;stride=2*n+1;v=[];triangles=[];groups=[]
            for i in range(k):
                theta=i//n*math.pi/2+2*math.atan((i%n)/n);r=.0005 if not law else .001+.0005*math.cos(theta)
                for j in range(2*n+1):
                    v.append([.01*math.cos(theta),.01*math.sin(theta),(.02-r)*j/n] if j<n else envelope(law,theta,2*math.atan((j-n)/n))[0])
            v.extend([[0.,0.,.02],[0.,0.,0.]])
            def idx(i,j):return (i%k)*stride+j
            for i in range(k):
                for j in range(2*n):
                    a,b,c,d=idx(i,j),idx(i+1,j),idx(i+1,j+1),idx(i,j+1)
                    triangles.extend([[a,b,c],[a,c,d]]);groups.extend([(5 if j<n else 1)+i//n]*2)
                triangles.extend([[idx(i,2*n),idx(i+1,2*n),k*stride],[idx(i+1,0),idx(i,0),k*stride+1]]);groups.extend([9,10])
            m={"quality":quality,"divisions":n,"vertices_m":v,"triangles":triangles,"face_groups":groups,"requested_position_error_m":4e-4 if n==8 else 1e-4,"requested_normal_error":.8 if n==8 else .5,"max_vertices":33026,"max_triangles":66048,"position_error_upper_m":2e-4 if n==8 else 5e-5,"normal_error_upper":.4 if n==8 else .2}
            volume,area,moments,normals=measured(m)
            interval=lambda x:[math.nextafter(x,-math.inf),math.nextafter(x,math.inf)]
            m["boundary_integrals"]={"volume_m3":interval(volume),"area_m2":interval(area),"first_moments_m4":[interval(x) for x in moments]};meshes.append(m)
        ideal=math.pi*.01**2*.02-2*math.pi*((.01-.0005)*(1-math.pi/4)*.0005**2+.0005**3/6) if law==0 else 6.2684367805e-6
        ref["cases"].append({"law":"variable" if law else "constant","minimum_radius_m":.0005,"maximum_radius_m":.0015 if law else .0005,"meshes":meshes,"ideal_volume_m3":[ideal-4e-13,ideal+4e-13]})
        n=16;nt=len(meshes[1]["triangles"]);nv=len(v)
        native["cases"].append({"valid":True,"closed":True,"connected":True,"edge_incidence_two":True,"faces":nt,"vertices":nv,"edges":nt*3//2,"euler":2,"free_edges":0,"volume_m3":volume,"area_m2":area,"center_of_mass_m":[x/volume for x in moments],"current_faces":[{"triangle":i,"normal":p} for i,p in enumerate(normals)]})
    return ref,native


class Contract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):cls.reference,cls.native=fixture()
    def test_complete_synthetic_contract(self):
        out=grade(self.reference,self.native);self.assertTrue(out["diagnostic_passed"]);self.assertFalse(out["global_embedding_certified"])
    def test_missing_topology_current_faces_and_mass_refuse(self):
        for key,value in (("faces",1),("closed",False),("volume_m3",1e-6),("area_m2",1e-5),("center_of_mass_m",[0.,0.,0.]),("current_faces",[])):
            actual=copy.deepcopy(self.native);actual["cases"][0][key]=value
            with self.subTest(key=key),self.assertRaises(ValueError):grade(self.reference,actual)
        actual=copy.deepcopy(self.native);actual["cases"][0]["current_faces"][0]["normal"]=[0.,0.,-1.]
        with self.assertRaises(ValueError):grade(self.reference,actual)
    def test_quality_scope_layout_units_and_false_phase_refuse(self):
        for key,value in (("position_error_upper_m",0.),("position_error_upper_m",1.),("normal_error_upper",float("nan")),("max_vertices",True),("divisions",False)):
            ref=copy.deepcopy(self.reference);ref["cases"][0]["meshes"][0][key]=value
            with self.subTest(key=key),self.assertRaises(ValueError):grade(ref,self.native)
        ref=copy.deepcopy(self.reference);ref["cases"][0]["meshes"][0]["triangles"][0].reverse()
        with self.assertRaises(ValueError):grade(ref,self.native)
        ref=copy.deepcopy(self.reference);ref["cases"][0]["meshes"][0]["face_groups"][0]=10
        with self.assertRaises(ValueError):grade(ref,self.native)
        for key in ("global_embedding_certified","native_floating_evaluation_certified","product_body_created"):
            ref=copy.deepcopy(self.reference);ref[key]=True
            with self.subTest(key=key),self.assertRaises(ValueError):grade(ref,self.native)
        ref=copy.deepcopy(self.reference)
        for m in ref["cases"][1]["meshes"]:m["vertices_m"]=[[-p[1],p[0],p[2]] for p in m["vertices_m"]]
        with self.assertRaisesRegex(ValueError,"mesh witness"):grade(ref,self.native)
    def test_missing_real_refusal_and_unresolved_intervals_refuse(self):
        native=copy.deepcopy(self.native);native["process_refusals"]["reversed_winding"]=False
        with self.assertRaises(ValueError):grade(self.reference,native)
        ref=copy.deepcopy(self.reference);ref["cases"][0]["meshes"][0]["boundary_integrals"]["volume_m3"]=[0.,1.]
        with self.assertRaises(ValueError):grade(ref,self.native)

class GridContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        ref,native=fixture()
        for case_index,case in enumerate(ref['cases']):
            for raw in case['meshes']:
                def rounded(v):
                    x=abs(v/1e-9);base=math.floor(x)
                    return math.copysign(float(base+(x-base>=.5))*1e-9,v)
                g={'vertices_m':[[rounded(x) for x in p] for p in raw['vertices_m']], 'triangles':copy.deepcopy(raw['triangles']),
                   'numeric_triangle_solid_admitted':True,'exact_global_embedding_certified':False,'vertex_grid_m':1e-9,'triangle_limit':10000,
                   'grid_displacement_upper_m':9e-10,'grid_normal_change_upper':.001,
                   'position_error_upper_m':raw['position_error_upper_m']+1e-9,'normal_error_upper':raw['normal_error_upper']+.002}
                v,a,m,n=measured(g);g.update(volume_m3=v,area_m2=a,center_of_mass_m=[x/v for x in m]);raw['grid_solid']=g
            actual=native['cases'][case_index]
            actual.update(volume_m3=v,area_m2=a,center_of_mass_m=[x/v for x in m],current_faces=[{'triangle':i,'normal':p} for i,p in enumerate(n)])
        cls.reference,cls.native=ref,native
    def test_complete_synthetic_grid_contract(self):
        self.assertEqual(len(grade_grid(self.reference,self.native)),2)
    def test_grid_binding_and_final_quality_refuse(self):
        for key,value in [('numeric_triangle_solid_admitted',False),('exact_global_embedding_certified',True),('vertex_grid_m',1e-6),('triangle_limit',True),('grid_displacement_upper_m',0.),('grid_normal_change_upper',0.),('position_error_upper_m',1.),('normal_error_upper',0.),('volume_m3',1.)]:
            ref=copy.deepcopy(self.reference);ref['cases'][0]['meshes'][0]['grid_solid'][key]=value
            with self.subTest(key=key),self.assertRaises(ValueError):grade_grid(ref,self.native)
        ref=copy.deepcopy(self.reference);ref['cases'][0]['meshes'][0]['grid_solid']['vertices_m'][0][0]+=1e-9
        with self.assertRaisesRegex(ValueError,'rounded input'):grade_grid(ref,self.native)
        ref=copy.deepcopy(self.reference);t=ref['cases'][0]['meshes'][0]['grid_solid']['triangles'][0];t[:]=t[1:]+t[:1]
        with self.assertRaisesRegex(ValueError,'rounded input'):grade_grid(ref,self.native)
    def test_grid_native_mass_normals_and_refusals_cannot_be_borrowed(self):
        _, raw_native=fixture()
        with self.assertRaises(ValueError):grade_grid(self.reference,raw_native)
        for key,value in [('volume_m3',1.),('faces',True),('closed',False),('current_faces',[])]:
            native=copy.deepcopy(self.native);native['cases'][0][key]=value
            with self.subTest(key=key),self.assertRaises(ValueError):grade_grid(self.reference,native)
        native=copy.deepcopy(self.native);native['cases'][0]['current_faces'][0]['normal']=[0.,0.,-1.]
        with self.assertRaises(ValueError):grade_grid(self.reference,native)
        native=copy.deepcopy(self.native);native['process_refusals']['wrong_units']=False
        with self.assertRaises(ValueError):grade_grid(self.reference,native)

if __name__=='__main__':unittest.main()
