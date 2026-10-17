"""Actual MCP/Agent flag-only controls, profiles, refusal/history and independent grading."""
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
from selected_construction_eval import graph_source, placed_problems, source_problems
from sketch_creation_eval import triangles, volume
from test_cylindrical_draft_mcp import Client, digest, canonical_digest

ROOT=Path(__file__).resolve().parents[1]

def exercise(binary,root,case):
    mcp=Client(binary,'mcp');agent=Client(binary,'agent');doc=root/'part.ocad.d';other=root/'other.ocad.d'
    try:
        mcp.call('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'selected-construction','version':'1'}})
        datum=case in ['tilted','seeded_XY']
        if case=='seeded_XY':shutil.copytree(ROOT/'examples/datum_plate.ocad.d',doc)
        else:
            mcp.tool('new_document',{'path':str(doc),'kind':'part','document_id':f'doc:construction_{case}','name':'Selected construction'})
            if datum:
                node={'id':'feature:datum','name':'Tilted datum','definition':{'type':'datum_plane','base':{'type':'custom','origin':[.01,-.02,.03],'normal':[0,.6,.8],'x_axis':[1,0,0]},'offset':{'value_si':.005}}}
                mcp.tool('patch_apply',{'path':str(doc),'patch':{'operations':[{'type':'add_feature','node':node,'position':{'at':'end'}}]}})
        baseline=graph_source(doc);shutil.copytree(doc,other)
        def parity(tool,args):
            result=mcp.tool(tool,{'path':str(doc),**args});response=agent.call(f'opencad.{tool}',{'path':str(other),**args})
            assert response.get('result')==result,response
            assert canonical_digest(doc)==canonical_digest(other)
            return result
        state=parity('inspect_sketch_creation',{})
        create={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':state['suggested_sketch_id'],'feature_id':state['suggested_feature_id'],'name':'Flag-only circle',**({'datum_plane':'feature:datum'} if datum else {'plane':case})}
        response=parity('apply_sketch_create',{'create':create});history=response['history']['history']
        example=json.loads((ROOT/'examples/agent/toggle_selected_construction.json').read_text())
        def edit(operations):
            state=parity('inspect_sketches',{})
            return {'document_id':state['document_id'],'revision':state['revision'],'sketch_id':create['sketch_id'],'operations':operations}
        response=parity('apply_sketch_edit',{'edit':edit(example['primitives']),'history':history});history=response['history']['history']
        def toggle(id,construction,before_count,after_count):
            nonlocal history
            request=edit([{'operation':'construction','id':id,'construction':construction}]);stable=digest(doc)
            plan=parity('plan_sketch_edit',{'edit':request});assert digest(doc)==stable
            assert plan['patch']['operations']==[{'type':'set_sketch_construction','sketch_id':create['sketch_id'],'entity_id':id,'construction':construction}]
            assert len(plan['profile_effect']['before'])==before_count and len(plan['profile_effect']['after'])==after_count
            response=parity('apply_sketch_edit',{'edit':request,'history':history});history=response['history']['history']
            return plan
        row=next(r for r in parity('inspect_sketches',{})['sketches'] if r['source']['id']==create['sketch_id'])
        assert [(t['id'],t['kind'],t['construction']) for t in row['construction_targets']]==[('ent:a','point',False),('ent:b','point',False),('ent:guide','line',True),('ent:disc','circle',True),('ent:ellipse','ellipse',True)]
        assert row['constraint_targets']['circles']==['ent:disc']
        toggle('ent:disc',False,0,1)
        tree=parity('list_features',{})
        response=parity('apply_feature_edit',{'revision':tree['revision'],'edit':{'operation':'create_extrude','id':'feature:disc_body','name':'Ordinary disc','sketch_feature':create['feature_id'],'profile_ref':'profile:circle:ent:disc','length_expr':'6 mm'},'history':history});history=response['history']['history']
        circle_only=canonical_digest(doc)
        plan=toggle('ent:ellipse',False,1,2)
        assert plan['profile_effect']['consumers']==[{'feature_id':'feature:disc_body','profile_ref':'profile:circle:ent:disc','suppressed':False}]
        plan=toggle('ent:a',True,2,2);assert plan['profile_effect']['before']==plan['profile_effect']['after']
        toggle('ent:guide',False,2,2);toggle('ent:guide',True,2,2)
        desired={'sketch_id':create['sketch_id'],'feature_id':create['feature_id'],'body_feature':'feature:disc_body','workplane':{'type':'datum_plane','feature_id':'feature:datum'} if datum else {'type':'global','plane':case},'case':case}
        assert source_problems(doc,desired,baseline)==[],source_problems(doc,desired,baseline)
        final=canonical_digest(doc);stable=digest(doc);peer=digest(other);saved_history=copy.deepcopy(history)
        bads=[edit([{'operation':'construction','id':'ent:guide','construction':False},{'operation':'construction','id':'ent:disc','construction':True}]),edit([{'operation':'construction','id':'ent:missing','construction':True}])]
        valid=edit([{'operation':'construction','id':'ent:b','construction':True}])
        bads.extend([{**valid,'document_id':'doc:foreign'},{**valid,'revision':'stale'}])
        for invalid in bads:
            mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':invalid,'history':history},error=True)
            assert 'error' in agent.call('opencad.apply_sketch_edit',{'path':str(other),'edit':invalid,'history':history})
            assert digest(doc)==stable and digest(other)==peer and history==saved_history
        noop=parity('apply_sketch_edit',{'edit':edit([{'operation':'construction','id':'ent:a','construction':True}]),'history':history})
        assert noop['evidence']['no_op'] and noop['history']['history']==history and digest(doc)==stable
        def history_step(operation):
            nonlocal history
            response=agent.call(f'opencad.history_{operation}_document',{'path':str(doc),'history':history})['result']
            twin=agent.call(f'opencad.history_{operation}_document',{'path':str(other),'history':history})['result']
            assert response==twin and canonical_digest(doc)==canonical_digest(other);history=response['history']
        for operation,wanted in [('undo',circle_only),('redo',final)]:
            for _ in range(4):history_step(operation)
            assert canonical_digest(doc)==wanted
        stl=root/'actual.stl';mcp.tool('export_document',{'path':str(doc),'output':str(stl)});data=stl.read_bytes()
        assert placed_problems(data,case)==[],placed_problems(data,case)
        translated=bytearray(data)
        for i,face in enumerate(triangles(data)):
            for j,p in enumerate(face):struct.pack_into('<3f',translated,84+50*i+12+12*j,p[0]+.005,p[1],p[2])
        assert math.isclose(volume(triangles(translated)),volume(triangles(data)),rel_tol=1e-6,abs_tol=1e-12) and placed_problems(translated,case)
        wrong=root/'wrong.ocad.d';shutil.copytree(doc,wrong);path=wrong/'graph/sketches.json';authored=json.loads(path.read_text())
        for mutation in ['unconsumed_flag','radius_expression','point_flag','extra_constraint']:
            candidate=copy.deepcopy(authored);sketch=next(s for s in candidate['sketches'] if s['id']==create['sketch_id'])
            if mutation=='unconsumed_flag':next(e for e in sketch['entities'] if e['id']=='ent:ellipse')['construction']=True
            elif mutation=='radius_expression':next(e for e in sketch['entities'] if e['id']=='ent:disc')['radius']='10 mm'
            elif mutation=='point_flag':next(e for e in sketch['entities'] if e['id']=='ent:a')['construction']=False
            else:sketch['constraints'].append({'type':'horizontal','id':'con:unrequested','line':'ent:guide'})
            path.write_text(json.dumps(candidate));assert source_problems(wrong,desired,baseline),mutation
        if case=='seeded_XY':
            task=next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id']=='toggle_selected_construction')
            regen=subprocess.run([str(binary),'regen',str(doc)],capture_output=True,text=True,check=True)
            assert mcp_eval.check(task,doc,regen.stdout,binary,root)==(True,[])
            assert not mcp_eval.check(task,doc,regen.stdout)[0]
        return {'case':case,'mcp_requests':mcp.count,'agent_requests':agent.count,'triangles':len(triangles(data))}
    finally:mcp.close();agent.close()

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--musubicad',type=Path,required=True);args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='musubicad-selected-construction-') as folder:
        cases=[]
        for case in ['XY','YZ','XZ','tilted','seeded_XY']:
            root=Path(folder)/case;root.mkdir();cases.append(exercise(args.musubicad.resolve(),root,case))
        print(json.dumps({'cases':cases,'model_turns':0,'typed_mcp_agent_parity':True,'flag_only_expression_retention':True,'real_profile_effects_and_consumers':True,'readonly_atomic_refusal_history':True,'independent_source_physics_and_adversaries':True}))

if __name__=='__main__':main()
