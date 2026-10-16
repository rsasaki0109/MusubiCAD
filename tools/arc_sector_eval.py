"""Independent source and SI mesh oracle for the typed arc-sector intent."""
import json
import math
import copy
from sketch_creation_eval import physical_problems

# Independent quarter-disc r=10 mm, centre=(2,-3) mm, depth=6 mm.
D=.04/(3*math.pi)
CASES={
 'XY': (([.002,-.003,0],[.012,.007,.006]),[.002+D,-.003+D,.003]),
 'YZ': (([0,.002,-.003],[.006,.012,.007]),[.003,.002+D,-.003+D]),
 'XZ': (([.002,-.006,-.003],[.012,0,.007]),[.002+D,-.003,-.003+D]),
 'tilted': (([.012,-.0194,.0298],[.022,-.0078,.0406]),[.012+D,-.0176+.8*D,.0382-.6*D]),
 'seeded_XY': (([.002,-.003,.005],[.012,.007,.011]),[.002+D,-.003+D,.008]),
}
def placed_problems(data,case):
 bounds,center=CASES[case]
 return physical_problems(data,bounds=bounds,expected_center=center,expected_volume=150*math.pi*1e-9)

def source_problems(document,desired,baseline=None):
    try:
        sketches=json.loads((document/'graph/sketches.json').read_text())['sketches']
        graph=json.loads((document/'graph/features.json').read_text())
        sketch=next(s for s in sketches if s['id']==desired['sketch_id'])
        if sketch['workplane']!=desired['workplane']:return ['arc sector lost authored workplane']
        wanted=[{'type': 'point', 'id': 'ent:sector_center', 'construction': False, 'x': '2.000 mm', 'y': '-3.000 mm'}, {'type': 'point', 'id': 'ent:sector_start', 'construction': False, 'x': '12.000 mm', 'y': '-3.000 mm'}, {'type': 'point', 'id': 'ent:sector_end', 'construction': False, 'x': '2.000 mm', 'y': '7.000 mm'}, {'type': 'arc', 'id': 'ent:sector_arc', 'construction': False, 'center': 'ent:sector_center', 'radius': '10.000 mm', 'start_angle': '0 deg', 'end_angle': '90 deg', 'start_point': 'ent:sector_start', 'end_point': 'ent:sector_end'}, {'type': 'line', 'id': 'ent:sector_start_ray', 'construction': False, 'start': 'ent:sector_center', 'end': 'ent:sector_start'}, {'type': 'line', 'id': 'ent:sector_end_ray', 'construction': False, 'start': 'ent:sector_end', 'end': 'ent:sector_center'}]
        if sorted(sketch['entities'],key=lambda e:e['id'])!=sorted(wanted,key=lambda e:e['id']) or sketch['constraints']:
            return ['arc-sector source differs from the exact quarter-sector source intent']
        owners=[n for n in graph['feature_nodes'] if n['definition'].get('type')=='sketch' and n['definition'].get('sketch_id')==desired['sketch_id'] and not n.get('suppressed',False)]
        if len(owners)!=1 or owners[0]['id']!=desired['feature_id']:return ['arc-sector sketch lacks one active source owner']
        if desired['workplane']['type']=='datum_plane' and not any(e['source']==desired['workplane']['feature_id'] and e['target']==desired['feature_id'] for e in graph['feature_graph']['edges']):
            return ['arc-sector owner lacks authored datum dependency']
        body=next(n for n in graph['feature_nodes'] if n['id']==desired['body_feature'])
        if body.get('suppressed',False) or any(body['definition'].get(k)!=v for k,v in {'type':'extrude','sketch_feature':desired['feature_id'],'operation':'new_body','length_expr':'6 mm'}.items()):
            return ['sector prism consumes the wrong source or dimensions']
        if baseline is not None:
            actual=graph_source(document)
            for key,collection,added in [('features','feature_nodes',{desired['feature_id'],desired['body_feature']}),('sketches','sketches',{desired['sketch_id']})]:
                old={item['id']:item for item in baseline[key][collection]};new={item['id']:item for item in actual[key][collection]}
                if len(new)!=len(actual[key][collection]) or set(new)!=set(old)|added or any(new[id]!=item for id,item in old.items()):
                    return ['arc-sector creation changed unrelated source']
            if graph['feature_graph']['order']!=baseline['features']['feature_graph']['order']+[desired['feature_id'],desired['body_feature']]:
                return ['arc-sector creation changed authored feature order']
            if actual['semantic_refs']!=baseline['semantic_refs']:return ['arc-sector creation changed semantic references']
            old=copy.deepcopy(baseline['parameters']);new=copy.deepcopy(actual['parameters'])
            for values in [old,new]:
                for value in values['parameters'].values():value.pop('dirty',None)
            if old!=new:return ['arc-sector creation changed parameter intent']
        return []
    except (OSError,ValueError,KeyError,TypeError,StopIteration):return ['arc-sector source evidence unavailable']

def graph_source(document):
    return {name:json.loads((document/'graph'/f'{name}.json').read_text()) for name in ['features','sketches','parameters','semantic_refs']}
