"""Independent authored-plane and numerical SI STL checks for ADR-102.

Exact native mass is checked separately. Mesh bounds allow the existing 0.1 mm
display deflection; mesh volume allows 1%. No tessellation certificate is claimed.
"""
from collections import Counter
import json
import math
import struct

BOUNDS = {
    "XY": ((-.02,-.01,0),(.02,.01,.006)),
    "YZ": ((0,-.02,-.01),(.006,.02,.01)),
    "XZ": ((-.02,-.006,-.01),(.02,0,.01)),
}

def triangles(data):
    if len(data)<84:
        raise ValueError("missing binary STL")
    n=struct.unpack_from('<I',data,80)[0]
    if not 4<=n<=20000 or len(data)!=84+50*n:
        raise ValueError("invalid bounded STL")
    faces=[]
    for i in range(n):
        fields=struct.unpack_from('<12fH',data,84+50*i)
        if not all(math.isfinite(x) for x in fields[:12]):
            raise ValueError("nonfinite STL")
        faces.append([tuple(fields[j:j+3]) for j in (3,6,9)])
    return faces

def volume(faces):
    return math.fsum((a[0]*(b[1]*c[2]-b[2]*c[1])+a[1]*(b[2]*c[0]-b[0]*c[2])+a[2]*(b[0]*c[1]-b[1]*c[0]))/6 for a,b,c in faces)

def centroid(faces):
    signed=[(a[0]*(b[1]*c[2]-b[2]*c[1])+a[1]*(b[2]*c[0]-b[0]*c[2])+a[2]*(b[0]*c[1]-b[1]*c[0])) for a,b,c in faces]
    total=math.fsum(signed)
    return tuple(math.fsum(s*(a[i]+b[i]+c[i])/4 for s,(a,b,c) in zip(signed,faces))/total for i in range(3))

def physical_problems(data,plane=None,*,bounds=None,expected_center=None,expected_volume=None):
    try:
        faces=triangles(data);points=[p for face in faces for p in face]
        if bounds is None and plane not in BOUNDS:
            return ['unsupported expected global plane']
        lower,upper=BOUNDS[plane] if bounds is None else bounds
        for axis in range(3):
            actual=(min(p[axis] for p in points),max(p[axis] for p in points))
            if any(abs(a-b)>1e-4 for a,b in zip(actual,(lower[axis],upper[axis]))):
                return ['actual world bounds or signed extrusion direction differs']
        ideal=math.pi*.02*.01*.006 if expected_volume is None else expected_volume
        if abs(volume(faces)-ideal)>ideal*.01:
            return ['independent STL volume differs from analytic prism']
        if expected_center is not None and any(abs(a-b)>1e-4 for a,b in zip(centroid(faces),expected_center)):
            return ['independent STL centroid differs from placed analytic prism']
        # Geometric closure at 1 nm; distinct from exact topological certification.
        edges=Counter()
        for face in faces:
            a,b,c=[tuple(round(x/1e-9) for x in p) for p in face]
            for p,q in [(a,b),(b,c),(c,a)]:
                if p==q:
                    return ['degenerate STL triangle']
                edges[p,q]+=1
        if any(count!=1 or edges[q,p]!=1 for (p,q),count in edges.items()):
            return ['STL is not a numerically closed oriented shell']
        return []
    except (ValueError,OverflowError,ZeroDivisionError,struct.error):
        return ['actual SI STL absent or malformed']

def source_problems(document,desired):
    try:
        sketches=json.loads((document/'graph/sketches.json').read_text())['sketches']
        features=json.loads((document/'graph/features.json').read_text())
        selected=[s for s in sketches if s['id']==desired['sketch_id']]
        owners=[n for n in features['feature_nodes'] if n['definition'].get('type')=='sketch' and n['definition'].get('sketch_id')==desired['sketch_id'] and not n.get('suppressed',False)]
        workplane=desired.get('workplane',{'type':'global','plane':desired.get('plane')})
        if len(selected)!=1 or selected[0]['workplane']!=workplane:
            return ['new sketch lacks exact authored workplane identity']
        if len(owners)!=1 or owners[0]['id']!=desired['feature_id']:
            return ['new sketch lacks its unique active source owner']
        entities={e['id']:e for e in selected[0]['entities']}
        ellipse=entities.get('ent:new_ellipse',{});point=entities.get('ent:new_center',{})
        if ellipse.get('type')!='ellipse' or any(ellipse.get(k)!=v for k,v in {'center':'ent:new_center','major_radius':'20 mm','minor_radius':'10 mm','axis_angle':'0 deg','construction':False}.items()):
            return ['new ellipse source differs from the requested dimensions']
        center=desired.get('center',['0 mm','0 mm'])
        if point.get('type')!='point' or point.get('x')!=center[0] or point.get('y')!=center[1]:
            return ['new ellipse center lost explicit local-unit source']
        body=next(n for n in features['feature_nodes'] if n['id']==desired['body_feature'])
        definition=body['definition']
        if body.get('suppressed',False) or any(definition.get(k)!=v for k,v in {'type':'extrude','sketch_feature':desired['feature_id'],'operation':'new_body','length_expr':'6 mm'}.items()):
            return ['new extrusion source does not consume the created sketch owner']
        return []
    except (OSError,ValueError,KeyError,TypeError,StopIteration):
        return ['new sketch creation source absent or malformed']
