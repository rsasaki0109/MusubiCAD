"""Real MCP/Agent selected-constraint parity; independent source and SI grading."""
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
from selected_constraint_eval import VOLUME,graph_source,placed_problems,source_problems,solved_problems
from sketch_creation_eval import triangles,volume
from test_cylindrical_draft_mcp import Client,digest,canonical_digest

ROOT=Path(__file__).resolve().parents[1]

def dof(row):
    state=row['solved']['solve_state']
    return 0 if state['status']=='fully_constrained' else state['dof']

def exercise(binary,root,case):
    mcp=Client(binary,'mcp');agent=Client(binary,'agent');doc=root/'part.ocad.d';other=root/'other.ocad.d'
    try:
        mcp.call('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'selected-constraints','version':'1'}})
        datum=case in ['tilted','seeded_XY']
        if case=='seeded_XY':shutil.copytree(ROOT/'examples/datum_plate.ocad.d',doc)
        else:
            mcp.tool('new_document',{'path':str(doc),'kind':'part','document_id':f'doc:constraints_{case}','name':'Selected constraints'})
            if datum:
                node={'id':'feature:datum','name':'Tilted datum','definition':{'type':'datum_plane','base':{'type':'custom','origin':[.01,-.02,.03],'normal':[0,.6,.8],'x_axis':[1,0,0]},'offset':{'value_si':.005}}}
                mcp.tool('patch_apply',{'path':str(doc),'patch':{'operations':[{'type':'add_feature','node':node,'position':{'at':'end'}}]}})
        baseline=graph_source(doc);shutil.copytree(doc,other)
        def parity(tool,args):
            result=mcp.tool(tool,{'path':str(doc),**args})
            response=agent.call(f'opencad.{tool}',{'path':str(other),**args})
            assert response.get('result')==result,response
            assert canonical_digest(doc)==canonical_digest(other)
            return result
        state=parity('inspect_sketch_creation',{})
        create={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':state['suggested_sketch_id'],'feature_id':state['suggested_feature_id'],'name':'Constraint circle',**({'datum_plane':'feature:datum'} if datum else {'plane':case})}
        response=parity('apply_sketch_create',{'create':create});history=response['history']['history']
        example=json.loads((ROOT/'examples/agent/constrain_selected_circle.json').read_text())
        def edit(operations):
            state=parity('inspect_sketches',{})
            return {'document_id':state['document_id'],'revision':state['revision'],'sketch_id':create['sketch_id'],'operations':operations}
        response=parity('apply_sketch_edit',{'edit':edit(example['primitives']),'history':history});history=response['history']['history']
        tree=parity('list_features',{});owner=next(r for r in tree['rows'] if r['id']==create['feature_id'])
        body={'operation':'create_extrude','id':'feature:disc_body','name':'Constrained circle prism','sketch_feature':owner['id'],'profile_ref':owner['closed_profiles'][0],'length_expr':'6 mm'}
        response=parity('apply_feature_edit',{'revision':tree['revision'],'edit':body,'history':history});history=response['history']['history']
        assert abs(response['evidence']['volume_m3']-150*math.pi*1e-9)<1e-12
        unconstrained=canonical_digest(doc)
        for i,form in enumerate(example['constraints']):
            state=parity('inspect_sketches',{});row=next(r for r in state['sketches'] if r['source']['id']==create['sketch_id'])
            assert row['constraint_targets']=={'points':['ent:a','ent:b'],'lines':['ent:guide'],'circles':['ent:disc']}
            assert row['suggested_constraint_id']==form['id'] and dof(row)==3-i
            request=edit([form]);before=digest(doc);plan=parity('plan_sketch_edit',{'edit':request})
            assert digest(doc)==before and len(plan['patch']['operations'])==1
            assert dof(next(r for r in plan['state']['sketches'] if r['source']['id']==create['sketch_id']))==2-i
            response=parity('apply_sketch_edit',{'edit':request,'history':history});history=response['history']['history']
        desired={'sketch_id':create['sketch_id'],'feature_id':owner['id'],'body_feature':'feature:disc_body','workplane':{'type':'datum_plane','feature_id':'feature:datum'} if datum else {'type':'global','plane':case},'case':case}
        assert source_problems(doc,desired,baseline)==[],source_problems(doc,desired,baseline)
        assert solved_problems(parity('inspect_sketches',{}),create['sketch_id'])==[]
        assert abs(response['evidence']['volume_m3']-VOLUME)<1e-12
        constrained=canonical_digest(doc);stable=digest(doc);peer_stable=digest(other)
        for form,reason in [
            ({'operation':'radius','id':'con:bad','target':'ent:disc','expr':'11 mm'},'contradictory'),
            ({'operation':'horizontal','id':'con:bad','line':'ent:guide'},'redundant constraints'),
            *[({'operation':'radius','id':'con:bad','target':'ent:disc','expr':expr},None) for expr in ['10','10 deg','0 mm','-1 mm','1e999 mm','missing_parameter']],
            ({'operation':'radius','id':'con:bad','target':'ent:guide','expr':'10 mm'},None),
        ]:
            invalid=edit([form])
            refused=mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':invalid,'history':history},error=True)
            twin=agent.call('opencad.apply_sketch_edit',{'path':str(other),'edit':invalid,'history':history})
            assert 'error' in twin and digest(other)==peer_stable,twin
            if reason:assert reason in json.dumps(refused),refused
            if reason:assert reason in json.dumps(twin),twin
            assert digest(doc)==stable
        collision=edit([{'operation':'remove_constraint','id':'con:control_1'},example['constraints'][0]])
        refused=mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':collision,'history':history},error=True)
        assert 'cannot be reused' in json.dumps(refused) and digest(doc)==stable
        stale={**request,'operations':[{'operation':'remove_constraint','id':'con:control_3'}]}
        for bad in [stale,{**edit(stale['operations']),'document_id':'doc:foreign'}]:
            mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':bad,'history':history},error=True);assert digest(doc)==stable
        remove=edit([{'operation':'remove_constraint','id':'con:control_3'}]);plan=parity('plan_sketch_edit',{'edit':remove})
        assert dof(next(r for r in plan['state']['sketches'] if r['source']['id']==create['sketch_id']))==1 and digest(doc)==stable
        response=parity('apply_sketch_edit',{'edit':remove,'history':history});history=response['history']['history']
        assert abs(response['evidence']['volume_m3']-150*math.pi*1e-9)<1e-12
        # MCP exposes the same opaque history through Agent document commands.
        def history_step(operation):
            nonlocal history
            response=agent.call(f'opencad.history_{operation}_document',{'path':str(doc),'history':history})['result']
            twin=agent.call(f'opencad.history_{operation}_document',{'path':str(other),'history':history})['result']
            assert response==twin and canonical_digest(doc)==canonical_digest(other)
            history=response['history']
        history_step('undo');assert canonical_digest(doc)==constrained
        history_step('redo');assert len(next(s for s in graph_source(doc)['sketches']['sketches'] if s['id']==create['sketch_id'])['constraints'])==2
        history_step('undo');assert canonical_digest(doc)==constrained
        for operation,wanted in [('undo',unconstrained),('redo',constrained)]:
            for _ in range(3):history_step(operation)
            assert canonical_digest(doc)==wanted
        stl=root/'actual.stl';mcp.tool('export_document',{'path':str(doc),'output':str(stl)});data=stl.read_bytes()
        assert placed_problems(data,case)==[],placed_problems(data,case)
        translated=bytearray(data)
        for i,face in enumerate(triangles(data)):
            for j,p in enumerate(face):struct.pack_into('<3f',translated,84+50*i+12+12*j,p[0]+.005,p[1],p[2])
        assert math.isclose(volume(triangles(translated)),volume(triangles(data)),rel_tol=1e-6,abs_tol=1e-12) and placed_problems(translated,case)
        wrong=root/'wrong_source.ocad.d';shutil.copytree(doc,wrong);path=wrong/'graph/sketches.json';authored=json.loads(path.read_text())
        for mutation in ['direct_radius','distance','horizontal','extra_point']:
            candidate=copy.deepcopy(authored);sketch=next(s for s in candidate['sketches'] if s['id']==create['sketch_id'])
            if mutation=='direct_radius':
                next(e for e in sketch['entities'] if e['id']=='ent:disc')['radius']='10 mm';sketch['constraints'].pop()
            elif mutation=='distance':sketch['constraints'][1]['expr']='19 mm'
            elif mutation=='horizontal':sketch['constraints'][0]['line']='ent:disc'
            else:sketch['entities'].append({'type':'point','id':'ent:unused','construction':False,'x':'0 mm','y':'0 mm'})
            path.write_text(json.dumps(candidate));assert source_problems(wrong,desired,baseline),mutation
        if case=='seeded_XY':
            task=next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id']=='constrain_selected_circle')
            regen=subprocess.run([str(binary),'regen',str(doc)],capture_output=True,text=True,check=True)
            assert mcp_eval.check(task,doc,regen.stdout,binary,root)==(True,[])
            assert not mcp_eval.check(task,doc,regen.stdout)[0]
        return {'case':case,'mcp_requests':mcp.count,'agent_requests':agent.count,'triangles':len(triangles(data))}
    finally:mcp.close();agent.close()

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--musubicad',type=Path,required=True);parser.add_argument('--out',type=Path);args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='musubicad-selected-constraints-') as folder:
        cases=[]
        for case in ['XY','YZ','XZ','tilted','seeded_XY']:
            root=Path(folder)/case;root.mkdir();cases.append(exercise(args.musubicad.resolve(),root,case))
        report={'cases':cases,'model_turns':0,'typed_mcp_agent_parity':True,'readonly_atomic_refusal':True,'actual_backend_dof_and_dimensions':True,'constraint_remove_and_undo_redo':True,'independent_source_and_physical_eval':True,'equal_volume_wrong_placement_refused':True,'direct_radius_source_decoy_refused':True}
        if args.out:args.out.write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(report))

if __name__=='__main__':main()
