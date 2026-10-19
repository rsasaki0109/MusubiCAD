"""ADR-111 independent original-source, contact and fresh SI solid oracle."""
import copy
import math
from selected_constraint_eval import graph_source
from sketch_creation_eval import physical_problems, triangles

def oracle(seat='counterbore', bottom=False, depth=10):
    r, big = 3.3, 5.5
    d = 2.2 if seat == 'countersink' else 4
    bore = math.pi * r*r * depth
    if seat == 'none':
        extra, first = 0, 0
    elif seat == 'counterbore':
        extra = math.pi * (big*big-r*r)*d
        first = extra*d/2
    else:
        extra = math.pi*d*(big*big+big*r-2*r*r)/3
        first = math.pi*d*d*(big*big+2*big*r-3*r*r)/12
    removed = bore + extra
    v = 12000-removed
    z = bore*depth/2+first if bottom else removed*10-bore*depth/2-first
    center = [(12000*20-removed*12)/v/1000, (12000*15-removed*9)/v/1000, (12000*5-z)/v/1000]
    return v*1e-9, center

def placed_problems(data, seat='counterbore', bottom=False, depth=10):
    v, center = oracle(seat, bottom, depth)
    problems = physical_problems(data, bounds=([0,0,0],[.04,.03,.01]), expected_center=center, expected_volume=v)
    if problems:
        return problems
    try:
        vertices = [p for t in triangles(data) for p in t]
        entry, inward = (0,1) if bottom else (.01,-1)
        contacts = [(entry, .0033 if seat == 'none' else .0055), (entry+inward*depth/1000,.0033)]
        if seat != 'none':
            contacts.append((entry+inward*(2.2 if seat=='countersink' else 4)/1000,.0033))
        for z, radius in contacts:
            ring = {p for p in vertices if abs(p[2]-z)<3e-7 and abs(math.hypot(p[0]-.012,p[1]-.009)-radius)<3e-7}
            if len(ring)<8:
                return ['hole contact ring has wrong entry, axis, radius or depth']
        return []
    except (ValueError,OverflowError):
        return ['hole physical contacts unavailable']

def source_problems(document, desired, baseline):
    try:
        actual = graph_source(document)
        old_nodes = {n['id']:n for n in baseline['features']['feature_nodes']}
        nodes = {n['id']:n for n in actual['features']['feature_nodes']}
        identity = desired.get('feature_id','feature:hole_1')
        if len(nodes)!=len(actual['features']['feature_nodes']) or set(nodes)!=set(old_nodes)|{identity} or any(nodes[id]!=n for id,n in old_nodes.items()):
            return ['hole rewrote original stock, owners or unrelated source']
        seat = desired.get('seat','counterbore')
        bottom = desired.get('bottom',False)
        depth = desired.get('depth','bore_depth')
        definition = {
            'type':'standard_hole' if seat=='none' else 'seated_standard_hole',
            'sketch_feature':'feature:sketch_bottom' if bottom else 'feature:sketch_seat',
            'profile_ref':'sketch:bottom_axis/profile:outer' if bottom else 'sketch:seat/profile:outer',
            'target_feature':'feature:extrude_base',
            'face_ref':'ref:face:plate_bottom' if bottom else 'ref:face:plate_entry',
            'standard':{'type':'iso273','size':'M6','fit':'normal'},
        }
        if seat=='none':
            definition.update(depth={'type':'distance','length':{'value_si':.01}},depth_expr=depth)
        else:
            definition.update(bore_depth={'value_si':.01},bore_depth_expr=depth)
            definition['seat']={'type':seat,'diameter':{'value_si':.011},'diameter_expr':'seat_diameter'}
            if seat=='counterbore':
                definition['seat'].update(depth={'value_si':.004},depth_expr='seat_depth')
            else:
                definition['seat'].update(included_angle={'value_si':math.pi/2},included_angle_expr='seat_angle')
        node=nodes[identity]
        if node['definition']!=definition or node.get('suppressed',False):
            return ['hole type, source axis/entry, standard or explicit dimensions differ']
        # All original sketch expressions/constraints/planes and semantic owners
        # remain exact, even when an unused wrong source would leave solid V unchanged.
        if actual['sketches']!=baseline['sketches'] or actual['semantic_refs']!=baseline['semantic_refs']:
            return ['hole changed original axis sketches or owned entry references']
        old, new = copy.deepcopy(baseline['parameters']), copy.deepcopy(actual['parameters'])
        for graph in [old,new]:
            for p in graph['parameters'].values():
                p.pop('dirty',None)
        if old!=new:
            return ['hole changed original parameter intent']
        a,b=actual['features']['feature_graph'],baseline['features']['feature_graph']
        if a['order']!=b['order']+[identity]:
            return ['hole changed original feature order']
        edges={(e['source'],e['target']) for e in a['edges']}
        wanted={(e['source'],e['target']) for e in b['edges']}|{(definition['sketch_feature'],identity),('feature:extrude_base',identity)}
        if edges!=wanted:
            return ['hole dependency ownership differs']
        return []
    except (OSError,ValueError,KeyError,TypeError,StopIteration,OverflowError):
        return ['hole source evidence unavailable']
