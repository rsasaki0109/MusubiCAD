"""Actual stdio typed curve discovery/edit, parity, source, history and physics."""
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
from selected_curve_edit_eval import graph_source, source_problems, placed_problems
from sketch_creation_eval import triangles, volume
from test_cylindrical_draft_mcp import Client, digest, canonical_digest

ROOT=Path(__file__).resolve().parents[1]

def exercise(binary,root,case,fixture_output=None):
    mcp=Client(binary,'mcp');agent=Client(binary,'agent');doc=root/'part.ocad.d';peer=root/'peer.ocad.d'
    try:
        mcp.call('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'selected-curve-edits','version':'1'}})
        if case=='seeded_XY':shutil.copytree(ROOT/'examples/datum_plate.ocad.d',doc)
        else:
            mcp.tool('new_document',{'path':str(doc),'kind':'part','document_id':f'doc:curves_{case}','name':'Selected curves'})
            if case=='tilted':
                node={'id':'feature:datum','name':'Tilted datum','definition':{'type':'datum_plane','base':{'type':'custom','origin':[.01,-.02,.03],'normal':[0,.6,.8],'x_axis':[1,0,0]},'offset':{'value_si':.005}}}
                mcp.tool('patch_apply',{'path':str(doc),'patch':{'operations':[{'type':'add_feature','node':node,'position':{'at':'end'}}]}})
        baseline=graph_source(doc);shutil.copytree(doc,peer)
        def parity(tool,args):
            result=mcp.tool(tool,{'path':str(doc),**args})
            method={'patch_apply':'patch_apply_document','export_document':'export'}.get(tool,tool)
            twin=agent.call(f'opencad.{method}',{'path':str(peer),**args})
            if tool=='patch_apply':
                assert result['patched']==str(doc) and twin.get('result',{}).get('patched')==str(peer),twin
                assert {k:v for k,v in result.items() if k!='patched'}=={k:v for k,v in twin['result'].items() if k!='patched'},twin
            else:assert twin.get('result')==result,twin
            assert canonical_digest(doc)==canonical_digest(peer)
            return result
        example=json.loads((ROOT/'examples/agent/trim_selected_guide.json').read_text())
        parity('patch_apply',{'patch':{'operations':[{'type':'add_parameter',**p} for p in example['parameters']]}})
        state=parity('inspect_sketch_creation',{});datum=case in ['tilted','seeded_XY']
        create={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':state['suggested_sketch_id'],'feature_id':state['suggested_feature_id'],'name':'Selected guide',**({'datum_plane':'feature:datum'} if datum else {'plane':case})}
        result=parity('apply_sketch_create',{'create':create});history=result['history']['history']
        def edit(operations):
            state=parity('inspect_sketches',{})
            return {'document_id':state['document_id'],'revision':state['revision'],'sketch_id':create['sketch_id'],'operations':operations}
        result=parity('apply_sketch_edit',{'edit':edit(example['primitives']),'history':history});history=result['history']['history']
        result=parity('patch_apply',{'patch':{'operations':[{'type':'add_sketch_constraint','sketch_id':create['sketch_id'],'constraint':c} for c in example['constraints']]},'history':history});history=result['history']
        tree=parity('list_features',{})
        body={'operation':'create_extrude','id':'feature:disc_body','name':'Unchanged disc','sketch_feature':create['feature_id'],'profile_ref':'profile:circle:ent:disc','length_expr':'6 mm'}
        result=parity('apply_feature_edit',{'revision':tree['revision'],'edit':body,'history':history});history=result['history']['history']
        if fixture_output is not None:
            # Optional source fixture generation uses the same real verified backend.
            shutil.copytree(doc,fixture_output)
        before=canonical_digest(doc);stable=digest(doc);saved=copy.deepcopy(history)
        current=edit([example['curve_edit']]);query={k:current[k] for k in ['document_id','revision','sketch_id']}
        query.update({k:example['curve_edit'][k] for k in ['target_id','cutter_id','mode','end']})
        options=parity('inspect_sketch_curve_edit',{'query':query})
        assert options['new_point_id']=='ent:trim_end_1' and len(options['intersections'])==2
        for index,x in [(0,-.006),(1,.010)]:
            p=options['intersections'][index]['position_m'];assert abs(p[0]-x)<1e-10 and abs(p[1]-.003)<1e-10
        assert digest(doc)==stable
        for mutation in ['retained_length','bad_branch','existing_point','missing_removal','duplicate_removal','mixed_batch','foreign','stale','missing_identity']:
            bad=copy.deepcopy(current);form=bad['operations'][0]
            if mutation=='retained_length':form['remove_constraints']=[]
            elif mutation=='bad_branch':form['intersection_index']=99
            elif mutation=='existing_point':form['new_point_id']='ent:right'
            elif mutation=='missing_removal':form['remove_constraints']=['con:missing']
            elif mutation=='duplicate_removal':form['remove_constraints']*=2
            elif mutation=='mixed_batch':bad['operations'].insert(0,{'operation':'point','id':'ent:prefix','x':'1 mm','y':'2 mm','create':True})
            elif mutation=='foreign':bad['document_id']='doc:foreign'
            elif mutation=='stale':bad['revision']='stale'
            else:bad.pop('document_id')
            mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':bad,'history':history},error=True)
            assert 'error' in agent.call('opencad.apply_sketch_edit',{'path':str(peer),'edit':bad,'history':history}),mutation
            assert digest(doc)==stable and history==saved and canonical_digest(peer)==before,mutation
        plan=parity('plan_sketch_edit',{'edit':current})
        assert plan['curve_edit_effect']['selected_index']==1 and len(plan['profile_effect']['before'])==len(plan['profile_effect']['after'])==1
        assert abs(plan['evidence']['volume_m3']-600*math.pi*1e-9)<1e-12 and digest(doc)==stable
        result=parity('apply_sketch_edit',{'edit':current,'history':history});history=result['history']['history'];after=canonical_digest(doc)
        desired={'sketch_id':create['sketch_id'],'feature_id':create['feature_id'],'body_feature':'feature:disc_body','workplane':{'type':'datum_plane','feature_id':'feature:datum'} if datum else {'type':'global','plane':case},'case':case}
        assert source_problems(doc,desired,baseline)==[],source_problems(doc,desired,baseline)
        for operation,wanted in [('undo',before),('redo',after)]:
            result=agent.call(f'opencad.history_{operation}_document',{'path':str(doc),'history':history})['result']
            twin=agent.call(f'opencad.history_{operation}_document',{'path':str(peer),'history':history})['result']
            assert result==twin;history=result['history'];assert canonical_digest(doc)==wanted and canonical_digest(peer)==wanted
        stl=root/'actual.stl';parity('export_document',{'output':str(stl)});data=stl.read_bytes()
        assert placed_problems(data,case)==[],placed_problems(data,case)
        translated=bytearray(data)
        for i,face in enumerate(triangles(data)):
            for j,p in enumerate(face):struct.pack_into('<3f',translated,84+50*i+12+12*j,p[0]+.005,p[1],p[2])
        assert math.isclose(volume(triangles(data)),volume(triangles(translated)),rel_tol=1e-6) and placed_problems(translated,case)
        wrong=root/'wrong.ocad.d';shutil.copytree(doc,wrong);path=wrong/'graph/sketches.json';source=json.loads(path.read_text())
        for mutation in ['other_branch','old_point','extra_removal','wrong_flag','literal_radius','nan_endpoint']:
            candidate=copy.deepcopy(source);sketch=next(s for s in candidate['sketches'] if s['id']==create['sketch_id']);entities={e['id']:e for e in sketch['entities']}
            if mutation=='other_branch':entities['ent:trim_end_1']['x']=-.006
            elif mutation=='old_point':entities['ent:right']['x']='22 mm'
            elif mutation=='extra_removal':sketch['constraints']=[]
            elif mutation=='wrong_flag':entities['ent:trim_end_1']['construction']=False
            elif mutation=='nan_endpoint':entities['ent:trim_end_1']['x']=float('nan')
            else:entities['ent:disc']['radius']='10 mm'
            path.write_text(json.dumps(candidate));assert source_problems(wrong,desired,baseline),mutation
        if case=='seeded_XY':
            task=next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id']=='trim_selected_guide')
            regen=subprocess.run([str(binary),'regen',str(doc)],capture_output=True,text=True,check=True)
            assert mcp_eval.check(task,doc,regen.stdout,binary,root)==(True,[])
            assert not mcp_eval.check(task,doc,regen.stdout)[0]
        return {'case':case,'mcp_requests':mcp.count,'agent_requests':agent.count,'triangles':len(triangles(data))}
    finally:mcp.close();agent.close()

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--musubicad',type=Path,required=True);args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='musubicad-selected-curves-') as folder:
        cases=[]
        for case in ['XY','YZ','XZ','tilted','seeded_XY']:
            root=Path(folder)/case;root.mkdir();cases.append(exercise(args.musubicad.resolve(),root,case))
        print(json.dumps({'cases':cases,'model_turns':0,'explicit_branch_removal_and_verified_endpoint':True,'typed_mcp_agent_parity':True,'readonly_atomic_refusal_history':True,'independent_source_physics_and_adversaries':True}))

if __name__=='__main__':main()
