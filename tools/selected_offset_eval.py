"""ADR-110 independent literal-copy intent and fresh absolute SI physical oracle."""
import copy
import math
from selected_constraint_eval import graph_source
from sketch_creation_eval import physical_problems

CASES = {
    'XY': (([-.011,-.016,0],[.015,.010,.006]),[.002,-.003,.003]),
    'YZ': (([0,-.011,-.016],[.006,.015,.010]),[.003,.002,-.003]),
    'XZ': (([-.011,-.006,-.016],[.015,0,.010]),[.002,-.003,-.003]),
    'tilted': (([-.001,-.0298,.028],[.025,-.0054,.0484]),[.012,-.0176,.0382]),
    'seeded_XY': (([-.011,-.016,.005],[.015,.010,.011]),[.002,-.003,.008]),
}

def placed_problems(data, case):
    bounds, center = CASES[case]
    return physical_problems(data, bounds=bounds, expected_center=center,
                             expected_volume=1014*math.pi*1e-9)

def source_problems(document, desired, baseline):
    try:
        actual=graph_source(document); graph=actual['features']
        sketch=next(s for s in actual['sketches']['sketches'] if s['id']==desired['copy_sketch'])
        if sketch['workplane']!=desired['workplane'] or sketch['constraints']:
            return ['offset copy lost workplane or copied constraints/association']
        point='ent:offset_1_point_1';curve='ent:offset_1_curve_1'
        expected=[{'type':'point','id':point,'construction':False,'x':.002,'y':-.003},
                  {'type':'circle','id':curve,'construction':False,'center':point,'radius':.013}]
        if len(sketch['entities'])!=2:return ['offset copy must contain exactly its literal point/circle']
        for actual_entity,wanted in zip(sketch['entities'],expected):
            if set(actual_entity)!=set(wanted):return ['offset literal source fields differ']
            for key,value in wanted.items():
                if key in ['x','y','radius']:
                    v=actual_entity[key]
                    if isinstance(v,bool) or not isinstance(v,(int,float)) or not math.isfinite(v) or abs(v-value)>1e-12:
                        return ['offset literal centre/radius differs; wrong side or associative source']
                elif actual_entity[key]!=value:return ['offset literal identity/reference/flag differs']
        owners=[n for n in graph['feature_nodes'] if n['definition'].get('type')=='sketch'
                and n['definition'].get('sketch_id')==desired['copy_sketch']]
        if len(owners)!=1 or owners[0]['id']!=desired['copy_owner'] or owners[0].get('suppressed',False):
            return ['offset copy requires its one active authored owner']
        body=next(n for n in graph['feature_nodes'] if n['id']==desired['body_feature'])
        if body.get('suppressed',False) or any(body['definition'].get(k)!=v for k,v in {
            'type':'extrude','sketch_feature':desired['copy_owner'],'profile_ref':f'profile:circle:{curve}',
            'operation':'new_body','length_expr':'6 mm'}.items()):
            return ['offset extrusion consumes wrong copy/profile/length']
        for key,collection,added in [('features','feature_nodes',{desired['copy_owner'],desired['body_feature']}),
                                    ('sketches','sketches',{desired['copy_sketch']})]:
            old={x['id']:x for x in baseline[key][collection]};new={x['id']:x for x in actual[key][collection]}
            if len(new)!=len(actual[key][collection]) or set(new)!=set(old)|added or any(new[id]!=value for id,value in old.items()):
                return ['offset changed original curves/constraints/owners/consumers or unrelated source']
        if graph['feature_graph']['order']!=baseline['features']['feature_graph']['order']+[desired['copy_owner'],desired['body_feature']]:
            return ['offset changed original feature order']
        edges={(e['source'],e['target']) for e in graph['feature_graph']['edges']}
        wanted={(e['source'],e['target']) for e in baseline['features']['feature_graph']['edges']}
        wanted.add((desired['copy_owner'],desired['body_feature']))
        if desired['workplane']['type']=='datum_plane':wanted.add((desired['workplane']['feature_id'],desired['copy_owner']))
        if edges!=wanted:return ['offset dependency/independence differs']
        old=copy.deepcopy(baseline['parameters']);new=copy.deepcopy(actual['parameters'])
        for parameters in [old,new]:
            for p in parameters['parameters'].values():p.pop('dirty',None)
        if old!=new or actual['semantic_refs']!=baseline['semantic_refs']:
            return ['offset changed parameters or semantic references']
        return []
    except (OSError,ValueError,KeyError,TypeError,StopIteration,OverflowError):
        return ['selected offset source evidence unavailable']
