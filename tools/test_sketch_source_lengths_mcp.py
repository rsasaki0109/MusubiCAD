"""Real MCP/Agent source lengths, intersections, refusal, history and physical grading."""
import argparse
import copy
import json
import math
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import mcp_eval
from sketch_source_lengths_eval import graph_source, placed_problems, source_problems
from sketch_creation_eval import triangles, volume
from test_cylindrical_draft_mcp import Client, digest, canonical_digest

ROOT = Path(__file__).resolve().parents[1]

def exercise(binary, root, case):
    mcp = Client(binary,'mcp');agent = Client(binary,'agent');doc = root/'part.ocad.d';peer = root/'peer.ocad.d'
    try:
        mcp.call('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'source-lengths','version':'1'}})
        if case == 'seeded_XY':shutil.copytree(ROOT/'examples/datum_plate.ocad.d',doc)
        else:
            mcp.tool('new_document',{'path':str(doc),'kind':'part','document_id':f'doc:lengths_{case}','name':'Source lengths'})
            if case == 'tilted':
                node = {'id':'feature:datum','name':'Tilted datum','definition':{'type':'datum_plane','base':{'type':'custom','origin':[.01,-.02,.03],'normal':[0,.6,.8],'x_axis':[1,0,0]},'offset':{'value_si':.005}}}
                mcp.tool('patch_apply',{'path':str(doc),'patch':{'operations':[{'type':'add_feature','node':node,'position':{'at':'end'}}]}})
        baseline = graph_source(doc);shutil.copytree(doc,peer)
        def parity(tool,args):
            result = mcp.tool(tool,{'path':str(doc),**args})
            method = {'patch_apply':'patch_apply_document','export_document':'export'}.get(tool,tool)
            twin = agent.call(f'opencad.{method}',{'path':str(peer),**args})
            if tool == 'patch_apply':
                assert result['patched'] == str(doc) and twin.get('result',{}).get('patched') == str(peer),twin
                assert {k:v for k,v in result.items() if k != 'patched'} == {k:v for k,v in twin['result'].items() if k != 'patched'},twin
            else:assert twin.get('result') == result,twin
            assert canonical_digest(doc) == canonical_digest(peer)
            return result
        example = json.loads((ROOT/'examples/agent/resolve_sketch_source_lengths.json').read_text())
        parity('patch_apply',{'patch':{'operations':[{'type':'add_parameter',**p} for p in example['parameters']]}})
        state = parity('inspect_sketch_creation',{});datum = case in ['tilted','seeded_XY']
        create = {'document_id':state['document_id'],'revision':state['revision'],'sketch_id':state['suggested_sketch_id'],'feature_id':state['suggested_feature_id'],'name':'Expression circle',**({'datum_plane':'feature:datum'} if datum else {'plane':case})}
        result = parity('apply_sketch_create',{'create':create});history = result['history']['history']
        def edit(operations):
            state = parity('inspect_sketches',{})
            return {'document_id':state['document_id'],'revision':state['revision'],'sketch_id':create['sketch_id'],'operations':operations}
        result = parity('apply_sketch_edit',{'edit':edit(example['primitives']),'history':history});history = result['history']['history']
        row = parity('inspect_sketches',{})['sketches'][-1];entities = {e['id']:e for e in row['solved']['entities']}
        assert abs(entities['ent:center']['x']-.002)<1e-10 and abs(entities['ent:center']['y']+.003)<1e-10 and abs(entities['ent:disc']['radius']-.01)<1e-10
        tree = parity('list_features',{})
        result = parity('apply_feature_edit',{'revision':tree['revision'],'edit':{'operation':'create_extrude','id':'feature:disc_body','name':'Expression body','sketch_feature':create['feature_id'],'profile_ref':'profile:circle:ent:disc','length_expr':'6 mm'},'history':history});history = result['history']['history']
        before = canonical_digest(doc);request = edit([example['parameter_radius']]);stable = digest(doc)
        plan = parity('plan_sketch_edit',{'edit':request});assert digest(doc) == stable
        assert abs(plan['evidence']['volume_m3']-600*math.pi*1e-9)<1e-12
        result = parity('apply_sketch_edit',{'edit':request,'history':history});history = result['history']['history'];after = canonical_digest(doc)
        desired = {'sketch_id':create['sketch_id'],'feature_id':create['feature_id'],'body_feature':'feature:disc_body','workplane':{'type':'datum_plane','feature_id':'feature:datum'} if datum else {'type':'global','plane':case},'case':case}
        assert source_problems(doc,desired,baseline) == [],source_problems(doc,desired,baseline)
        curve = {'sketch_id':create['sketch_id'],'target_id':'ent:guide','cutter_id':'ent:disc','mode':'trim','end':'end','new_point_id':'ent:cut_end'}
        for index, x in [(0,-.006),(1,.010)]:
            plan = parity('plan_curve_edit',{'edit':{**curve,'intersection_index':index}})
            assert plan['selected_index'] == index and len(plan['intersections']) == 2
            position = plan['intersections'][index]['position_m'];assert abs(position[0]-x)<1e-10 and abs(position[1]-.003)<1e-10
        stable = digest(doc);saved = copy.deepcopy(history)
        mcp.tool('plan_curve_edit',{'path':str(doc),'edit':curve},error=True)
        assert 'error' in agent.call('opencad.plan_curve_edit',{'path':str(peer),'edit':curve}) and digest(doc) == stable
        offset = {'source_sketch_id':create['sketch_id'],'curve_ids':['ent:disc'],'distance_m':.001,'association':'independent_copy','join':'mitre_tangent','destination_sketch_id':'sketch:offset','name':'Read-only offset','destination_curve_ids':['ent:offset'],'destination_point_ids':['ent:offset_center']}
        plan = parity('plan_offset',{'edit':offset})
        assert abs(plan['curves'][0]['radius_m']-.011)<1e-10 and digest(doc) == stable
        for value in ['5 deg','5 furlong','1 mm / 0','missing_radius','tool_angle']:
            invalid = edit([{**example['parameter_radius'],'radius':value}])
            mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':invalid,'history':history},error=True)
            assert 'error' in agent.call('opencad.apply_sketch_edit',{'path':str(peer),'edit':invalid,'history':history})
            assert digest(doc) == stable and history == saved and canonical_digest(doc) == canonical_digest(peer)
        for value in ['20 deg','20 furlong','1 mm / 0']:
            invalid = {'operations':[{'type':'set_parameter','id':'param:diameter','expr':value}]}
            mcp.tool('patch_apply',{'path':str(doc),'patch':invalid,'history':history},error=True)
            assert 'error' in agent.call('opencad.patch_apply_document',{'path':str(peer),'patch':invalid,'history':history})
            assert digest(doc) == stable and history == saved and canonical_digest(doc) == canonical_digest(peer)
        for operation,wanted in [('undo',before),('redo',after)]:
            response = agent.call(f'opencad.history_{operation}_document',{'path':str(doc),'history':history})['result']
            twin = agent.call(f'opencad.history_{operation}_document',{'path':str(peer),'history':history})['result']
            assert response == twin;history = response['history'];assert canonical_digest(doc) == wanted and canonical_digest(peer) == wanted
        stl = root/'actual.stl';parity('export_document',{'output':str(stl)})
        data = stl.read_bytes();assert placed_problems(data,case) == [],placed_problems(data,case)
        translated = bytearray(data)
        for i,face in enumerate(triangles(data)):
            for j,p in enumerate(face):struct.pack_into('<3f',translated,84+50*i+12+12*j,p[0]+.005,p[1],p[2])
        assert math.isclose(volume(triangles(data)),volume(triangles(translated)),rel_tol=1e-6) and placed_problems(translated,case)
        wrong = root/'wrong.ocad.d';shutil.copytree(doc,wrong);path = wrong/'graph/sketches.json';source = json.loads(path.read_text())
        for mutation in ['radius_literal','unconsumed_point','extra_constraint']:
            candidate = copy.deepcopy(source);sketch = next(s for s in candidate['sketches'] if s['id'] == create['sketch_id'])
            if mutation == 'radius_literal':next(e for e in sketch['entities'] if e['id'] == 'ent:disc')['radius'] = '10 mm'
            elif mutation == 'unconsumed_point':next(e for e in sketch['entities'] if e['id'] == 'ent:left')['x'] = '-18 mm'
            else:sketch['constraints'].append({'type':'horizontal','id':'con:extra','line':'ent:guide'})
            path.write_text(json.dumps(candidate));assert source_problems(wrong,desired,baseline),mutation
        if case == 'seeded_XY':
            task = next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id'] == 'resolve_sketch_source_lengths')
            regen = subprocess.run([str(binary),'regen',str(doc)],capture_output=True,text=True,check=True)
            assert mcp_eval.check(task,doc,regen.stdout,binary,root) == (True,[])
            assert not mcp_eval.check(task,doc,regen.stdout)[0]
        return {'case':case,'mcp_requests':mcp.count,'agent_requests':agent.count,'triangles':len(triangles(data))}
    finally:mcp.close();agent.close()

def main():
    parser = argparse.ArgumentParser();parser.add_argument('--musubicad',type=Path,required=True);args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='musubicad-source-lengths-') as folder:
        cases = []
        for case in ['XY','YZ','XZ','tilted','seeded_XY']:
            root = Path(folder)/case;root.mkdir();cases.append(exercise(args.musubicad.resolve(),root,case))
        print(json.dumps({'cases':cases,'model_turns':0,'strict_source_length_resolution':True,'actual_intersections_and_offset':True,'typed_mcp_agent_parity':True,'readonly_atomic_refusal_history':True,'independent_source_physics_and_adversaries':True}))

if __name__ == '__main__':main()
