"""Actual typed point/circle batch parity, atomic history and fresh physical grading."""
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
from pointer_circle_eval import graph_source,placed_problems,source_problems
from sketch_creation_eval import triangles,volume
from test_cylindrical_draft_mcp import Client,digest,canonical_digest

ROOT=Path(__file__).resolve().parents[1]

def exercise(binary,root,case):
    mcp=Client(binary,'mcp');agent=Client(binary,'agent');doc=root/'part.ocad.d';other=root/'other.ocad.d'
    try:
        mcp.call('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'pointer-circle','version':'1'}})
        datum=case in ['tilted','seeded_XY']
        if case=='seeded_XY':shutil.copytree(ROOT/'examples/datum_plate.ocad.d',doc)
        else:
            mcp.tool('new_document',{'path':str(doc),'kind':'part','document_id':f'doc:pointer_{case}','name':'Pointer circle'})
            if datum:
                node={'id':'feature:datum','name':'Tilted datum','definition':{'type':'datum_plane','base':{'type':'custom','origin':[.01,-.02,.03],'normal':[0,.6,.8],'x_axis':[1,0,0]},'offset':{'value_si':.005}}}
                mcp.tool('patch_apply',{'path':str(doc),'patch':{'operations':[{'type':'add_feature','node':node,'position':{'at':'end'}}]}})
        baseline=graph_source(doc);initial=canonical_digest(doc);shutil.copytree(doc,other)
        state=mcp.tool('inspect_sketch_creation',{'path':str(doc)})
        assert state==agent.call('opencad.inspect_sketch_creation',{'path':str(other)})['result']
        create={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':state['suggested_sketch_id'],'feature_id':state['suggested_feature_id'],'name':'Pointer circle',**({'datum_plane':'feature:datum'} if datum else {'plane':case})}
        response=mcp.tool('apply_sketch_create',{'path':str(doc),'create':create})
        assert response==agent.call('opencad.apply_sketch_create',{'path':str(other),'create':create})['result']
        history=response['history']['history'];other_history=copy.deepcopy(history)
        empty=canonical_digest(doc);state=mcp.tool('inspect_sketches',{'path':str(doc)})
        edit={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':create['sketch_id'],'operations':[
            {'operation':'point','id':'ent:circle_1_center','x':'2.000 mm','y':'-3.000 mm','create':True},
            {'operation':'circle','id':'ent:circle_1','center':'ent:circle_1_center','radius':'5.000000 mm','construction':False,'create':True}]}
        before=digest(doc);plan=mcp.tool('plan_sketch_edit',{'path':str(doc),'edit':edit})
        assert plan==agent.call('opencad.plan_sketch_edit',{'path':str(other),'edit':edit})['result']
        assert digest(doc)==before and len(plan['patch']['operations'])==2
        for radius in ['0 mm','-5 mm','5','5 deg','1e999 mm']:
            bad=copy.deepcopy(edit);bad['operations'][1]['radius']=radius
            mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':bad,'history':history},error=True);assert digest(doc)==before
        bad={**edit,'document_id':'doc:foreign'}
        mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':bad,'history':history},error=True);assert digest(doc)==before
        response=mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':edit,'history':history})
        parity=agent.call('opencad.apply_sketch_edit',{'path':str(other),'edit':edit,'history':other_history})['result']
        assert response==parity and canonical_digest(doc)==canonical_digest(other)
        history=response['history']['history'];other_history=parity['history']['history'];circle=canonical_digest(doc)
        for operation,wanted in [('undo',empty),('redo',circle)]:
            result=agent.call(f'opencad.history_{operation}_document',{'path':str(doc),'history':history})['result'];history=result['history']
            other_history=agent.call(f'opencad.history_{operation}_document',{'path':str(other),'history':other_history})['result']['history']
            assert canonical_digest(doc)==wanted
            assert canonical_digest(doc)==canonical_digest(other)
        stable=digest(doc);mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':edit,'history':history},error=True);assert digest(doc)==stable
        tree=mcp.tool('list_features',{'path':str(doc)});owner=next(r for r in tree['rows'] if r['id']==create['feature_id'])
        body={'operation':'create_extrude','id':'feature:pointer_body','name':'Pointer cylinder','sketch_feature':owner['id'],'profile_ref':owner['closed_profiles'][0],'length_expr':'6 mm'}
        response=mcp.tool('apply_feature_edit',{'path':str(doc),'revision':tree['revision'],'edit':body,'history':history})
        parity=agent.call('opencad.apply_feature_edit',{'path':str(other),'revision':tree['revision'],'edit':body,'history':other_history})['result']
        assert response==parity and canonical_digest(doc)==canonical_digest(other)
        history=response['history']['history'];assert abs(response['evidence']['volume_m3']-150*math.pi*1e-9)<1e-12
        desired={'sketch_id':create['sketch_id'],'feature_id':owner['id'],'body_feature':'feature:pointer_body','workplane':{'type':'datum_plane','feature_id':'feature:datum'} if datum else {'type':'global','plane':case},'case':case}
        assert source_problems(doc,desired,baseline)==[],source_problems(doc,desired,baseline)
        stl=root/'actual.stl';mcp.tool('export_document',{'path':str(doc),'output':str(stl)});data=stl.read_bytes()
        assert placed_problems(data,case)==[],placed_problems(data,case)
        wrong=bytearray(data)
        for i,face in enumerate(triangles(data)):
            for j,(x,y,z) in enumerate(face):struct.pack_into('<3f',wrong,84+50*i+12+12*j,x+.005,y,z)
        assert abs(volume(triangles(wrong))-volume(triangles(data)))<1e-12 and placed_problems(wrong,case)
        final=canonical_digest(doc)
        for operation,wanted in [('undo',initial),('redo',final)]:
            for _ in range(3):history=agent.call(f'opencad.history_{operation}_document',{'path':str(doc),'history':history})['result']['history']
            assert canonical_digest(doc)==wanted
        if case=='seeded_XY':
            task=next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id']=='create_pointer_circle')
            regen=subprocess.run([str(binary),'regen',str(doc)],capture_output=True,text=True,check=True)
            assert mcp_eval.check(task,doc,regen.stdout,binary,root)==(True,[])
        return {'case':case,'mcp_requests':mcp.count,'agent_requests':agent.count,'triangles':len(triangles(data))}
    finally:mcp.close();agent.close()

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--musubicad',type=Path,required=True);parser.add_argument('--out',type=Path)
    args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='musubicad-pointer-circle-') as folder:
        root=Path(folder);cases=[]
        for case in ['XY','YZ','XZ','tilted','seeded_XY']:
            path=root/case;path.mkdir();cases.append(exercise(args.musubicad.resolve(),path,case))
        report={'cases':cases,'model_turns':0,'typed_mcp_agent_parity':True,'readonly_atomic_refusal':True,'single_batch_undo_redo':True,'independent_source_and_physical_eval':True,'equal_volume_wrong_placement_refused':True}
        if args.out:args.out.write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(report))

if __name__=='__main__':main()
