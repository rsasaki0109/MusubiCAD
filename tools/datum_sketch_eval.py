"""Independent source-association and placed-ellipse oracle; no CAD resolver."""
import copy
import json
from sketch_creation_eval import physical_problems, source_problems

# Hand-prescribed world axes and ellipse support amplitudes, in mm. In particular
# XZ's normal is -Y; the tilted root has y=(0,4/5,-3/5), n=(0,3/5,4/5).
CASES = {
    'XY': ((0,0,0),(1,0,0),(0,1,0),(0,0,1),(20,10,0),-2),
    'YZ': ((0,0,0),(0,1,0),(0,0,1),(1,0,0),(0,20,10),-2),
    'XZ': ((0,0,0),(1,0,0),(0,0,1),(0,-1,0),(20,0,10),-2),
    'tilted': ((10,-20,30),(1,0,0),(0,.8,-.6),(0,.6,.8),(20,8,6),-2),
    'seeded_XY': ((0,0,0),(1,0,0),(0,1,0),(0,0,1),(20,10,0),0),
}

def expected(case,offset_mm=15,local_mm=(2,-3)):
    origin,x,y,normal,amplitude,child=CASES[case]
    center=[(origin[i]+(offset_mm+child)*normal[i]+local_mm[0]*x[i]+local_mm[1]*y[i])/1000 for i in range(3)]
    lower=[center[i]-amplitude[i]/1000+min(0,.006*normal[i]) for i in range(3)]
    upper=[center[i]+amplitude[i]/1000+max(0,.006*normal[i]) for i in range(3)]
    centroid=[center[i]+.003*normal[i] for i in range(3)]
    return (lower,upper),centroid

def placed_problems(data,case,offset_mm=15,local_mm=(2,-3)):
    bounds,center=expected(case,offset_mm,local_mm)
    return physical_problems(data,bounds=bounds,expected_center=center)

def graph_source(document):
    return {name:json.loads((document/'graph'/f'{name}.json').read_text())
            for name in ['features','sketches','parameters','semantic_refs']}

def preservation_problems(document,baseline,desired):
    try:
        actual=graph_source(document)
        old=baseline['features'];new=actual['features']
        # Source catalogues serialize canonically by ID; authored execution order
        # is the separate feature_graph.order, checked exactly below.
        old_nodes={n['id']:n for n in old['feature_nodes']};new_nodes={n['id']:n for n in new['feature_nodes']}
        if len(new_nodes)!=len(new['feature_nodes']) or set(new_nodes)!=set(old_nodes)|{desired['feature_id'],desired['body_feature']} or any(new_nodes.get(id)!=node for id,node in old_nodes.items()):
            return ['datum editing changed unrelated feature source']
        if new['feature_graph']['order']!=old['feature_graph']['order']+[desired['feature_id'],desired['body_feature']]:
            return ['datum creation changed unrelated feature order']
        old_sketches={s['id']:s for s in baseline['sketches']['sketches']};new_sketches={s['id']:s for s in actual['sketches']['sketches']}
        if len(new_sketches)!=len(actual['sketches']['sketches']) or set(new_sketches)!=set(old_sketches)|{desired['sketch_id']} or any(new_sketches.get(id)!=s for id,s in old_sketches.items()):
            return ['datum creation changed unrelated authored sketches']
        if actual['semantic_refs']!=baseline['semantic_refs']:
            return ['datum creation changed unrelated semantic references']
        wanted=copy.deepcopy(baseline['parameters']);got=copy.deepcopy(actual['parameters'])
        wanted['parameters']['param:offset']['expr']='15 mm'
        # Dirty flags are disposable parameter evaluation state, not new intent.
        for data in [wanted,got]:
            for value in data['parameters'].values():value.pop('dirty',None)
        if wanted!=got:return ['datum parameter source changed beyond the requested offset']
        edges=new['feature_graph']['edges']
        if not any(e['source']==desired['workplane']['feature_id'] and e['target']==desired['feature_id'] for e in edges):
            return ['created owner lacks its actual datum dependency']
        return []
    except (OSError,ValueError,KeyError,TypeError):
        return ['datum source preservation evidence unavailable']

def task_source_problems(document,task,root):
    desired=task['expected_datum_sketch_creation']
    return source_problems(document,desired)+preservation_problems(document,graph_source(root/task['source']),desired)
