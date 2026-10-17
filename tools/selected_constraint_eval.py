"""ADR-106 independent authored-constraint and placed physical-result oracles."""
import copy
import json
import math
from sketch_creation_eval import physical_problems

VOLUME = 600 * math.pi * 1e-9
CASES = {
    'XY': (([-.008,-.013,0],[.012,.007,.006]),[.002,-.003,.003]),
    'YZ': (([0,-.008,-.013],[.006,.012,.007]),[.003,.002,-.003]),
    'XZ': (([-.008,-.006,-.013],[.012,0,.007]),[.002,-.003,-.003]),
    'tilted': (([.002,-.0274,.0298],[.022,-.0078,.0466]),[.012,-.0176,.0382]),
    'seeded_XY': (([-.008,-.013,.005],[.012,.007,.011]),[.002,-.003,.008]),
}

def placed_problems(data, case):
    bounds, center = CASES[case]
    return physical_problems(data, bounds=bounds, expected_center=center, expected_volume=VOLUME)

def graph_source(document):
    return {name:json.loads((document/'graph'/f'{name}.json').read_text())
            for name in ['features','sketches','parameters','semantic_refs']}

def source_problems(document, desired, baseline=None):
    try:
        actual=graph_source(document); graph=actual['features']
        sketch=next(s for s in actual['sketches']['sketches'] if s['id']==desired['sketch_id'])
        if sketch['workplane']!=desired['workplane']:
            return ['constrained sketch lost its authored workplane']
        # Keep literal authored expressions. A direct radius edit can yield the
        # same solid but is not the requested constraint-driven design intent.
        if sketch['entities'] != [
            {'type':'point','id':'ent:a','construction':False,'x':'2 mm','y':'-3 mm'},
            {'type':'point','id':'ent:b','construction':False,'x':'26 mm','y':'15 mm'},
            {'type':'line','id':'ent:guide','construction':True,'start':'ent:a','end':'ent:b'},
            {'type':'circle','id':'ent:disc','construction':False,'center':'ent:a','radius':'5 mm'},
        ]:
            return ['authored points/guide/circle changed instead of adding the intended constraints']
        if sketch['constraints'] != [
            {'type':'horizontal','id':'con:control_1','line':'ent:guide'},
            {'type':'distance','id':'con:control_2','target':{'a':'ent:a','b':'ent:b'},'expr':'20 mm'},
            {'type':'radius','id':'con:control_3','target':'ent:disc','expr':'10 mm'},
        ]:
            return ['intended horizontal, point-distance and circle-radius source constraints differ']
        owners=[n for n in graph['feature_nodes'] if n['definition'].get('type')=='sketch'
                and n['definition'].get('sketch_id')==desired['sketch_id'] and not n.get('suppressed',False)]
        if len(owners)!=1 or owners[0]['id']!=desired['feature_id']:
            return ['constrained sketch lacks one active source owner']
        if desired['workplane']['type']=='datum_plane' and not any(
            e['source']==desired['workplane']['feature_id'] and e['target']==desired['feature_id']
            for e in graph['feature_graph']['edges']):
            return ['constrained sketch lacks its authored datum dependency']
        body=next(n for n in graph['feature_nodes'] if n['id']==desired['body_feature'])
        if body.get('suppressed',False) or any(body['definition'].get(k)!=v for k,v in {
            'type':'extrude','sketch_feature':desired['feature_id'],
            'profile_ref':'profile:circle:ent:disc','operation':'new_body','length_expr':'6 mm'}.items()):
            return ['constrained circle extrusion consumes the wrong source/dimensions']
        if baseline is not None:
            for key,collection,added in [('features','feature_nodes',{desired['feature_id'],desired['body_feature']}),
                                         ('sketches','sketches',{desired['sketch_id']})]:
                old={x['id']:x for x in baseline[key][collection]};new={x['id']:x for x in actual[key][collection]}
                if len(new)!=len(actual[key][collection]) or set(new)!=set(old)|added or any(new[id]!=value for id,value in old.items()):
                    return ['constraint task changed unrelated source']
            if graph['feature_graph']['order']!=baseline['features']['feature_graph']['order']+[desired['feature_id'],desired['body_feature']]:
                return ['constraint task changed feature order']
            if actual['semantic_refs']!=baseline['semantic_refs']:
                return ['constraint task changed semantic references']
            old=copy.deepcopy(baseline['parameters']);new=copy.deepcopy(actual['parameters'])
            for values in [old,new]:
                for value in values['parameters'].values():value.pop('dirty',None)
            if old!=new:return ['constraint task changed parameter intent']
        return []
    except (OSError,ValueError,KeyError,TypeError,StopIteration):
        return ['selected constraint source evidence unavailable']

def solved_problems(state, sketch_id):
    """Compare actual backend inspection against a hand-derived local solution.

    The construction guide is absent from STL, so physical grading alone cannot
    prove its point-distance or its remaining DOF. No frontend solver is used.
    """
    try:
        row=next(r for r in state['sketches'] if r['source']['id']==sketch_id)
        if row['solved']['solve_state']!={'status':'fully_constrained'}:
            return ['backend does not report the intended zero remaining DOF']
        entities={e['id']:e for e in row['solved']['entities']}
        if any(abs(entities[id][axis]-value)>1e-8 for id,axis,value in [
            ('ent:a','x',.002),('ent:a','y',-.003),('ent:b','x',.022),('ent:b','y',-.003),('ent:disc','radius',.01)]):
            return ['actual backend solved dimensions differ from the independent solution']
        return []
    except (ValueError,KeyError,TypeError,StopIteration):
        return ['actual backend constraint solution unavailable']
