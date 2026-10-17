"""Independent exact source and fresh placed physical evidence for ADR-108."""
import copy
from selected_constraint_eval import graph_source, placed_problems

PARAMETERS = {
    'param:origin_x': {'id':'param:origin_x','name':'origin_x','expr':'1 mm'},
    'param:origin_y': {'id':'param:origin_y','name':'origin_y','expr':'-2 mm'},
    'param:diameter': {'id':'param:diameter','name':'diameter','expr':'20 mm'},
}

def source_problems(document, desired, baseline):
    try:
        actual = graph_source(document)
        sketch = next(s for s in actual['sketches']['sketches'] if s['id'] == desired['sketch_id'])
        expected = [
            {'type':'point','id':'ent:center','construction':False,'x':'origin_x + 1 mm','y':'origin_y - 1 mm'},
            {'type':'point','id':'ent:left','construction':False,'x':'origin_x - 19 mm','y':'origin_y + 5 mm'},
            {'type':'point','id':'ent:right','construction':False,'x':'origin_x + 21 mm','y':'origin_y + 5 mm'},
            {'type':'line','id':'ent:guide','construction':True,'start':'ent:left','end':'ent:right'},
            {'type':'circle','id':'ent:disc','construction':False,'center':'ent:center','radius':'diameter / 4 + 5 mm'},
        ]
        if sketch['entities'] != expected or sketch['constraints'] or sketch['workplane'] != desired['workplane']:
            return ['source length intent/constraints/workplane differs']
        graph = actual['features']
        owners = [n for n in graph['feature_nodes'] if n['definition'].get('type') == 'sketch'
                  and n['definition'].get('sketch_id') == desired['sketch_id']]
        if len(owners) != 1 or owners[0]['id'] != desired['feature_id'] or owners[0].get('suppressed',False):
            return ['source length sketch lacks its active owner']
        body = next(n for n in graph['feature_nodes'] if n['id'] == desired['body_feature'])
        if body.get('suppressed',False) or any(body['definition'].get(k) != v for k,v in {
            'type':'extrude','sketch_feature':desired['feature_id'],'profile_ref':'profile:circle:ent:disc',
            'operation':'new_body','length_expr':'6 mm'}.items()):
            return ['source length solid consumer differs']
        for key, collection, added in [('features','feature_nodes',{desired['feature_id'],desired['body_feature']}),
                                      ('sketches','sketches',{desired['sketch_id']})]:
            old = {x['id']:x for x in baseline[key][collection]}
            new = {x['id']:x for x in actual[key][collection]}
            if len(new) != len(actual[key][collection]) or set(new) != set(old)|added or any(new[id] != value for id,value in old.items()):
                return ['source length task changed unrelated source']
        if graph['feature_graph']['order'] != baseline['features']['feature_graph']['order']+[desired['feature_id'],desired['body_feature']]:
            return ['source length task changed feature order']
        if desired['workplane']['type'] == 'datum_plane' and not any(
            e['source'] == desired['workplane']['feature_id'] and e['target'] == desired['feature_id'] for e in graph['feature_graph']['edges']):
            return ['source length task lost its datum dependency']
        if actual['semantic_refs'] != baseline['semantic_refs']:
            return ['source length task changed semantic references']
        old = copy.deepcopy(baseline['parameters']);new = copy.deepcopy(actual['parameters'])
        for graph_parameters in [old,new]:
            for value in graph_parameters['parameters'].values():value.pop('dirty',None)
        expected_parameters = {**old['parameters'],**PARAMETERS}
        if new['parameters'] != expected_parameters or new['edges'] != old['edges']:
            return ['source length task changed parameter definitions/dependencies']
        return []
    except (OSError,ValueError,KeyError,TypeError,StopIteration):
        return ['source length evidence unavailable']
