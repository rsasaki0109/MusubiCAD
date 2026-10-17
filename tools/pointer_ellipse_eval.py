"""Independent intended-source and oriented SI mesh oracle for ADR-105."""
import copy
import json
import math
import re
from sketch_creation_eval import physical_problems,triangles

RX=.012*math.sqrt(5)
RY=.002*math.sqrt(145)
VOLUME=3600*math.pi*1e-9
# Hand-derived world oracles; no production placement resolver or intent math.
CASES={
 'XY': (([.002-RX,-.003-RY,0],[.002+RX,-.003+RY,.006]),[.002,-.003,.003],[.8,.6,0],[-.6,.8,0],[1,0,0]),
 'YZ': (([0,.002-RX,-.003-RY],[.006,.002+RX,-.003+RY]),[.003,.002,-.003],[0,.8,.6],[0,-.6,.8],[0,1,0]),
 'XZ': (([.002-RX,-.006,-.003-RY],[.002+RX,0,-.003+RY]),[.002,-.003,-.003],[.8,0,.6],[-.6,0,.8],[1,0,0]),
 'tilted': (([.012-RX,-.0194-.0016*math.sqrt(145),.0358-.0012*math.sqrt(145)],
             [.012+RX,-.0158+.0016*math.sqrt(145),.0406+.0012*math.sqrt(145)]),[.012,-.0176,.0382],[.8,.48,-.36],[-.6,.64,-.48],[1,0,0]),
 'seeded_XY': (([.002-RX,-.003-RY,.005],[.002+RX,-.003+RY,.011]),[.002,-.003,.008],[.8,.6,0],[-.6,.8,0],[1,0,0]),
}
def placed_problems(data,case):
    bounds,center,major,minor,_=CASES[case]
    problems=physical_problems(data,bounds=bounds,expected_center=center,expected_volume=VOLUME)
    if problems:return problems
    points=[p for f in triangles(data) for p in f]
    for axis,radius in [(major,.03),(minor,.02)]:
        projected=[sum((p[i]-center[i])*axis[i] for i in range(3)) for p in points]
        if abs(min(projected)+radius)>1e-4 or abs(max(projected)-radius)>1e-4:
            return ['actual oriented ellipse supports differ despite equal volume/world bounds']
    return []

def graph_source(document):
    return {name:json.loads((document/'graph'/f'{name}.json').read_text()) for name in ['features','sketches','parameters','semantic_refs']}

def source_problems(document,desired,baseline=None):
    try:
        actual=graph_source(document);sketches=actual['sketches']['sketches'];graph=actual['features']
        sketch=next(s for s in sketches if s['id']==desired['sketch_id'])
        if sketch['workplane']!=desired['workplane']:return ['ellipse lost authored workplane']
        entities=sketch['entities']
        point=next(e for e in entities if e.get('id')=='ent:ellipse_1_center')
        ellipse=next(e for e in entities if e.get('id')=='ent:ellipse_1')
        if len(entities)!=2 or sketch['constraints'] or point!={'type':'point','id':'ent:ellipse_1_center','construction':False,'x':'2.000 mm','y':'-3.000 mm'}:
            return ['ellipse centre/source differs from the exact pointer intent']
        if {k:v for k,v in ellipse.items() if k!='axis_angle'}!={'type':'ellipse','id':'ent:ellipse_1','construction':False,'center':'ent:ellipse_1_center','major_radius':'30.000000000000 mm','minor_radius':'20.000000000000 mm'}:
            return ['ellipse axes differ from the three-point intent (perpendicular width is not the minor radius)']
        angle=ellipse.get('axis_angle','')
        if not isinstance(angle,str) or not re.fullmatch(r'\d+\.\d{17} rad',angle) or abs(float(angle[:-4])-math.atan2(3,4))>1e-13:
            return ['ellipse source orientation differs from the clicked major axis']
        owners=[n for n in graph['feature_nodes'] if n['definition'].get('type')=='sketch' and n['definition'].get('sketch_id')==desired['sketch_id'] and not n.get('suppressed',False)]
        if len(owners)!=1 or owners[0]['id']!=desired['feature_id']:return ['ellipse lacks one active source owner']
        if desired['workplane']['type']=='datum_plane' and not any(e['source']==desired['workplane']['feature_id'] and e['target']==desired['feature_id'] for e in graph['feature_graph']['edges']):
            return ['ellipse owner lacks authored datum dependency']
        body=next(n for n in graph['feature_nodes'] if n['id']==desired['body_feature'])
        if body.get('suppressed',False) or any(body['definition'].get(k)!=v for k,v in {'type':'extrude','sketch_feature':desired['feature_id'],'operation':'new_body','length_expr':'6 mm'}.items()):
            return ['ellipse prism consumes the wrong source or dimensions']
        if baseline is not None:
            for key,collection,added in [('features','feature_nodes',{desired['feature_id'],desired['body_feature']}),('sketches','sketches',{desired['sketch_id']})]:
                old={x['id']:x for x in baseline[key][collection]};new={x['id']:x for x in actual[key][collection]}
                if len(new)!=len(actual[key][collection]) or set(new)!=set(old)|added or any(new[id]!=value for id,value in old.items()):
                    return ['ellipse creation changed unrelated source']
            if graph['feature_graph']['order']!=baseline['features']['feature_graph']['order']+[desired['feature_id'],desired['body_feature']]:
                return ['ellipse creation changed feature order']
            if actual['semantic_refs']!=baseline['semantic_refs']:return ['ellipse creation changed semantic references']
            old=copy.deepcopy(baseline['parameters']);new=copy.deepcopy(actual['parameters'])
            for values in [old,new]:
                for value in values['parameters'].values():value.pop('dirty',None)
            if old!=new:return ['ellipse creation changed parameter intent']
        return []
    except (OSError,ValueError,KeyError,TypeError,StopIteration):return ['ellipse source evidence unavailable']
