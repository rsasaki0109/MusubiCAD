"""ADR-109 independent source intent plus fresh absolute SI physical evidence."""
import copy
import math
from selected_constraint_eval import graph_source, placed_problems
from sketch_source_lengths_eval import PARAMETERS

def source_problems(document, desired, baseline):
    try:
        actual = graph_source(document)
        sketch = next(s for s in actual['sketches']['sketches'] if s['id'] == desired['sketch_id'])
        expected = [
            {'type':'point','id':'ent:center','construction':False,'x':'origin_x + 1 mm','y':'origin_y - 1 mm'},
            {'type':'point','id':'ent:left','construction':False,'x':'origin_x - 19 mm','y':'origin_y + 5 mm'},
            {'type':'point','id':'ent:right','construction':False,'x':'origin_x + 21 mm','y':'origin_y + 5 mm'},
            {'type':'line','id':'ent:guide','construction':True,'start':'ent:left','end':'ent:trim_end_1'},
            {'type':'circle','id':'ent:disc','construction':False,'center':'ent:center','radius':'diameter / 4 + 5 mm'},
            {'type':'point','id':'ent:trim_end_1','construction':True,'x':.010,'y':.003},
        ]
        # Source endpoint floats have a 1 nm geometric tolerance. Expressions,
        # original unconsumed/shared points and IDs must still match exactly.
        entities = copy.deepcopy(sketch['entities'])
        endpoint = next(e for e in entities if e['id'] == 'ent:trim_end_1')
        if any(not isinstance(endpoint[k],(int,float)) or not math.isfinite(endpoint[k]) or abs(endpoint[k]-v)>1e-9 for k,v in [('x',.010),('y',.003)]):
            return ['selected curve branch endpoint differs']
        endpoint.update(x=.010,y=.003)
        if entities != expected or sketch['constraints'] != [{'type':'horizontal','id':'con:guide_horizontal','line':'ent:guide'}] or sketch['workplane'] != desired['workplane']:
            return ['selected curve source, explicit removals or workplane differs']
        graph = actual['features']
        owners = [n for n in graph['feature_nodes'] if n['definition'].get('type') == 'sketch' and n['definition'].get('sketch_id') == desired['sketch_id']]
        if len(owners)!=1 or owners[0]['id']!=desired['feature_id'] or owners[0].get('suppressed',False):
            return ['selected curve source lacks its active owner']
        body = next(n for n in graph['feature_nodes'] if n['id']==desired['body_feature'])
        if body.get('suppressed',False) or any(body['definition'].get(k)!=v for k,v in {
            'type':'extrude','sketch_feature':desired['feature_id'],'profile_ref':'profile:circle:ent:disc',
            'operation':'new_body','length_expr':'6 mm'}.items()):
            return ['selected curve solid consumer differs']
        for key,collection,added in [('features','feature_nodes',{desired['feature_id'],desired['body_feature']}),('sketches','sketches',{desired['sketch_id']})]:
            old={x['id']:x for x in baseline[key][collection]};new={x['id']:x for x in actual[key][collection]}
            if len(new)!=len(actual[key][collection]) or set(new)!=set(old)|added or any(new[id]!=value for id,value in old.items()):
                return ['curve edit changed unrelated authored source']
        if graph['feature_graph']['order'] != baseline['features']['feature_graph']['order']+[desired['feature_id'],desired['body_feature']]:
            return ['curve edit changed feature order']
        if desired['workplane']['type']=='datum_plane' and not any(e['source']==desired['workplane']['feature_id'] and e['target']==desired['feature_id'] for e in graph['feature_graph']['edges']):
            return ['curve edit lost datum dependency']
        old=copy.deepcopy(baseline['parameters']);new=copy.deepcopy(actual['parameters'])
        for params in [old,new]:
            for p in params['parameters'].values():p.pop('dirty',None)
        if new['parameters']!={**old['parameters'],**PARAMETERS} or new['edges']!=old['edges'] or actual['semantic_refs']!=baseline['semantic_refs']:
            return ['curve edit changed parameters or semantic references']
        return []
    except (OSError,ValueError,KeyError,TypeError,StopIteration,OverflowError):
        return ['selected curve source evidence unavailable']
