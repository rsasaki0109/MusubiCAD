"""ADR-107 exact flag-only design intent and independent placed SI physics."""
import copy
from selected_constraint_eval import graph_source, placed_problems  # same independently derived disc

def source_problems(document, desired, baseline):
    try:
        actual=graph_source(document);graph=actual['features']
        sketch=next(s for s in actual['sketches']['sketches'] if s['id']==desired['sketch_id'])
        if sketch['workplane']!=desired['workplane']:return ['construction edit lost its source workplane']
        if sketch['entities'] != [
            {'type':'point','id':'ent:a','construction':True,'x':'2 mm','y':'-3 mm'},
            {'type':'point','id':'ent:b','construction':False,'x':'26 mm','y':'15 mm'},
            {'type':'line','id':'ent:guide','construction':True,'start':'ent:a','end':'ent:b'},
            {'type':'circle','id':'ent:disc','construction':False,'center':'ent:a','radius':'1 cm'},
            {'type':'ellipse','id':'ent:ellipse','construction':False,'center':'ent:a','major_radius':'15 mm + 15 mm','minor_radius':'20 mm','axis_angle':'30 deg'},
        ]:return ['intended source flags/expressions differ; solid mass alone cannot prove this edit']
        if sketch['constraints']:return ['flag-only edit introduced constraints']
        owners=[n for n in graph['feature_nodes'] if n['definition'].get('type')=='sketch' and n['definition'].get('sketch_id')==desired['sketch_id'] and not n.get('suppressed',False)]
        if len(owners)!=1 or owners[0]['id']!=desired['feature_id']:return ['construction sketch lacks one active intended owner']
        if desired['workplane']['type']=='datum_plane' and not any(e['source']==desired['workplane']['feature_id'] and e['target']==desired['feature_id'] for e in graph['feature_graph']['edges']):return ['construction sketch lacks its source datum dependency']
        body=next(n for n in graph['feature_nodes'] if n['id']==desired['body_feature'])
        if body.get('suppressed',False) or any(body['definition'].get(k)!=v for k,v in {'type':'extrude','sketch_feature':desired['feature_id'],'profile_ref':'profile:circle:ent:disc','operation':'new_body','length_expr':'6 mm'}.items()):return ['flag-only task changed the solid consumer']
        for key,collection,added in [('features','feature_nodes',{desired['feature_id'],desired['body_feature']}),('sketches','sketches',{desired['sketch_id']})]:
            old={x['id']:x for x in baseline[key][collection]};new={x['id']:x for x in actual[key][collection]}
            if len(new)!=len(actual[key][collection]) or set(new)!=set(old)|added or any(new[id]!=value for id,value in old.items()):return ['construction edit changed unrelated source']
        if graph['feature_graph']['order']!=baseline['features']['feature_graph']['order']+[desired['feature_id'],desired['body_feature']]:return ['construction edit changed feature order']
        if actual['semantic_refs']!=baseline['semantic_refs']:return ['construction edit changed semantic references']
        old=copy.deepcopy(baseline['parameters']);new=copy.deepcopy(actual['parameters'])
        for values in [old,new]:
            for value in values['parameters'].values():value.pop('dirty',None)
        if old!=new:return ['construction edit changed parameter intent']
        return []
    except (OSError,ValueError,KeyError,TypeError,StopIteration):
        return ['selected construction source evidence unavailable']
