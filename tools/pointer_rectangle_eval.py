"""Independent source and SI mesh oracle for the typed pointer-rectangle intent."""
import json
import copy
from sketch_creation_eval import physical_problems

# Hand-derived 20 x 10 x 6 mm prism; opposite corners=(2,-3)..(22,7) mm.
# Tilted root: origin=(10,-20,30) mm, n=(0,.6,.8), offset=+5 mm.
CASES={
    'XY': (([.002,-.003,0],[.022,.007,.006]),[.012,.002,.003]),
    'YZ': (([0,.002,-.003],[.006,.022,.007]),[.003,.012,.002]),
    'XZ': (([.002,-.006,-.003],[.022,0,.007]),[.012,-.003,.002]),
    'tilted': (([.012,-.0194,.0298],[.032,-.0078,.0406]),[.022,-.0136,.0352]),
    'seeded_XY': (([.002,-.003,.005],[.022,.007,.011]),[.012,.002,.008]),
}

def placed_problems(data,case):
    bounds,center=CASES[case]
    return physical_problems(data,bounds=bounds,expected_center=center,expected_volume=1200e-9)

def source_problems(document,desired,baseline=None):
    try:
        sketches=json.loads((document/'graph/sketches.json').read_text())['sketches']
        graph=json.loads((document/'graph/features.json').read_text())
        sketch=next(s for s in sketches if s['id']==desired['sketch_id'])
        if sketch['workplane']!=desired['workplane']:return ['pointer rectangle lost authored workplane']
        corners=[('2.000 mm','-3.000 mm'),('22.000 mm','-3.000 mm'),('22.000 mm','7.000 mm'),('2.000 mm','7.000 mm')]
        wanted=[{'type':'point','id':f'ent:rectangle_1_p{i}','construction':False,'x':x,'y':y} for i,(x,y) in enumerate(corners)]
        wanted += [{'type':'line','id':f'ent:rectangle_1_e{i}','construction':False,'start':f'ent:rectangle_1_p{i}','end':f'ent:rectangle_1_p{(i+1)%4}'} for i in range(4)]
        if sorted(sketch['entities'],key=lambda e:e['id'])!=sorted(wanted,key=lambda e:e['id']) or sketch['constraints']:
            return ['pointer source differs from the atomic four-point/four-line rectangle intent']
        owners=[n for n in graph['feature_nodes'] if n['definition'].get('type')=='sketch' and n['definition'].get('sketch_id')==desired['sketch_id'] and not n.get('suppressed',False)]
        if len(owners)!=1 or owners[0]['id']!=desired['feature_id']:return ['pointer sketch lacks one active source owner']
        if desired['workplane']['type']=='datum_plane' and not any(e['source']==desired['workplane']['feature_id'] and e['target']==desired['feature_id'] for e in graph['feature_graph']['edges']):
            return ['pointer owner lacks authored datum dependency']
        body=next(n for n in graph['feature_nodes'] if n['id']==desired['body_feature'])
        if body.get('suppressed',False) or any(body['definition'].get(k)!=v for k,v in {'type':'extrude','sketch_feature':desired['feature_id'],'operation':'new_body','length_expr':'6 mm'}.items()):
            return ['pointer prism consumes the wrong source or dimensions']
        if baseline is not None:
            actual=graph_source(document)
            for key,collection,added in [('features','feature_nodes',{desired['feature_id'],desired['body_feature']}),('sketches','sketches',{desired['sketch_id']})]:
                old={item['id']:item for item in baseline[key][collection]};new={item['id']:item for item in actual[key][collection]}
                if len(new)!=len(actual[key][collection]) or set(new)!=set(old)|added or any(new[id]!=item for id,item in old.items()):
                    return ['pointer creation changed unrelated source']
            if graph['feature_graph']['order']!=baseline['features']['feature_graph']['order']+[desired['feature_id'],desired['body_feature']]:
                return ['pointer creation changed authored feature order']
            if actual['semantic_refs']!=baseline['semantic_refs']:return ['pointer creation changed semantic references']
            old=copy.deepcopy(baseline['parameters']);new=copy.deepcopy(actual['parameters'])
            for values in [old,new]:
                for value in values['parameters'].values():value.pop('dirty',None)
            if old!=new:return ['pointer creation changed parameter intent']
        return []
    except (OSError,ValueError,KeyError,TypeError,StopIteration):return ['pointer source evidence unavailable']

def graph_source(document):
    return {name:json.loads((document/'graph'/f'{name}.json').read_text()) for name in ['features','sketches','parameters','semantic_refs']}
