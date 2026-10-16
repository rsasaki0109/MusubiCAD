"""Actual typed MCP/Agent datum creation/editing and independent fresh STL grading."""
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
from datum_sketch_eval import expected,graph_source,placed_problems,preservation_problems
from sketch_creation_eval import source_problems,triangles,volume
from test_cylindrical_draft_mcp import Client,digest,canonical_digest

ROOT=Path(__file__).resolve().parents[1]

def exercise(binary,root,case):
    mcp=Client(binary,'mcp');agent=Client(binary,'agent');doc=root/'part.ocad.d';other=root/'other.ocad.d'
    try:
        mcp.call('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'datum-sketch','version':'1'}})
        seed=case=='seeded_XY'
        if seed:
            shutil.copytree(ROOT/'examples/datum_plate.ocad.d',doc);datum='feature:datum'
        else:
            mcp.tool('new_document',{'path':str(doc),'kind':'part','document_id':f'doc:datum_{case}','name':'Datum creation'})
            base={'type':'custom','origin':[.01,-.02,.03],'normal':[0,.6,.8],'x_axis':[1,0,0]} if case=='tilted' else {'type':'global','plane':case}
            nodes=[('feature:root',{'type':'datum_plane','base':base,'offset':{'value_si':.005},'offset_expr':'offset'}),
                   ('feature:child',{'type':'datum_plane','base':{'type':'datum_plane','feature_id':'feature:root'},'offset':{'value_si':-.002}})]
            operations=[{'type':'add_parameter','id':'param:offset','name':'offset','expr':'5 mm'}]+[
                {'type':'add_feature','node':{'id':id,'name':id,'definition':definition},'position':{'at':'end'}} for id,definition in nodes]
            mcp.tool('patch_apply',{'path':str(doc),'patch':{'operations':operations}});datum='feature:child'
        baseline=graph_source(doc);initial=canonical_digest(doc);shutil.copytree(doc,other)
        state=mcp.tool('inspect_sketch_creation',{'path':str(doc)})
        assert state==agent.call('opencad.inspect_sketch_creation',{'path':str(other)})['result']
        assert any(p['feature_id']==datum for p in state['datum_planes'])
        create={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':state['suggested_sketch_id'],
                'feature_id':state['suggested_feature_id'],'name':'Placed datum ellipse','datum_plane':datum}
        before=digest(doc);plan=mcp.tool('plan_sketch_create',{'path':str(doc),'create':create})
        assert plan==agent.call('opencad.plan_sketch_create',{'path':str(other),'create':create})['result']
        assert digest(doc)==before and len(plan['patch']['operations'])==2
        response=mcp.tool('apply_sketch_create',{'path':str(doc),'create':create})
        assert response==agent.call('opencad.apply_sketch_create',{'path':str(other),'create':create})['result']
        history=response['history']['history'];other_history=copy.deepcopy(history)
        for operations in [[{'operation':'point','id':'ent:new_center','x':'0 mm','y':'0 mm','create':True}],
                           [{'operation':'ellipse','id':'ent:new_ellipse','center':'ent:new_center','major_radius':'20 mm','minor_radius':'10 mm','axis_angle':'0 deg','construction':False,'create':True}]]:
            state=mcp.tool('inspect_sketches',{'path':str(doc)})
            edit={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':create['sketch_id'],'operations':operations}
            before=digest(doc);plan=mcp.tool('plan_sketch_edit',{'path':str(doc),'edit':edit})
            assert plan==agent.call('opencad.plan_sketch_edit',{'path':str(other),'edit':edit})['result'] and digest(doc)==before
            response=mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':edit,'history':history})
            parity=agent.call('opencad.apply_sketch_edit',{'path':str(other),'edit':edit,'history':other_history})['result']
            assert response==parity and canonical_digest(doc)==canonical_digest(other)
            history=response['history']['history'];other_history=parity['history']['history']
        tree=mcp.tool('list_features',{'path':str(doc)});profile=next(r for r in tree['rows'] if r['id']==create['feature_id'])['closed_profiles'][0]
        body={'operation':'create_extrude','id':'feature:new_datum_body','name':'Datum prism','sketch_feature':create['feature_id'],'profile_ref':profile,'length_expr':'6 mm'}
        response=mcp.tool('apply_feature_edit',{'path':str(doc),'revision':tree['revision'],'edit':body,'history':history});history=response['history']['history']
        assert abs(response['evidence']['volume_m3']-math.pi*1200e-9)<1e-12
        state=mcp.tool('inspect_sketches',{'path':str(doc)})
        stale={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':create['sketch_id'],
               'operations':[{'operation':'point','id':'ent:new_center','x':'2 mm','y':'-3 mm','create':False}]}
        for bad in [{**stale,'document_id':'doc:foreign'},{k:v for k,v in stale.items() if k!='document_id'},
                    {**stale,'frame':{'origin_m':[0,0,0]}}]:
            before=digest(doc);mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':bad,'history':history},error=True);assert digest(doc)==before
        stl=root/'before.stl';mcp.tool('export_document',{'path':str(doc),'output':str(stl)})
        assert placed_problems(stl.read_bytes(),case,5,(0,0))==[]
        response=mcp.tool('patch_apply',{'path':str(doc),'patch':{'operations':[{'type':'set_parameter','id':'param:offset','expr':'15 mm'}]},'history':history});history=response['history']
        before=digest(doc);mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':stale,'history':history},error=True);assert digest(doc)==before
        state=mcp.tool('inspect_sketches',{'path':str(doc)});current={**stale,'revision':state['revision']}
        response=mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':current,'history':history});history=response['history']['history']
        desired={'sketch_id':create['sketch_id'],'feature_id':create['feature_id'],'body_feature':'feature:new_datum_body',
                 'workplane':{'type':'datum_plane','feature_id':datum},'center':['2 mm','-3 mm'],'case':case}
        assert source_problems(doc,desired)==[],source_problems(doc,desired)
        assert preservation_problems(doc,baseline,desired)==[],preservation_problems(doc,baseline,desired)
        stl=root/'after.stl';mcp.tool('export_document',{'path':str(doc),'output':str(stl)});data=stl.read_bytes()
        assert placed_problems(data,case)==[],placed_problems(data,case)
        # Translate a genuine mesh: its volume is unchanged but source placement is wrong.
        wrong=bytearray(data)
        for i,face in enumerate(triangles(data)):
            for j,(x,y,z) in enumerate(face):struct.pack_into('<3f',wrong,84+50*i+12+12*j,x+.005,y,z)
        assert abs(volume(triangles(wrong))-volume(triangles(data)))<1e-12
        assert placed_problems(wrong,case)
        altered=copy.deepcopy(desired);altered['workplane']={'type':'custom','origin':[0,0,0],'normal':[0,0,1],'x_axis':[1,0,0]}
        assert source_problems(doc,altered)
        final=canonical_digest(doc)
        for operation,wanted in [('undo',initial),('redo',final)]:
            for _ in range(6):
                result=agent.call(f'opencad.history_{operation}_document',{'path':str(doc),'history':history})
                assert 'error' not in result,result
                history=result['result']['history']
            assert canonical_digest(doc)==wanted
        if seed:
            task=next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id']=='create_datum_sketch')
            regen=subprocess.run([str(binary),'regen',str(doc)],capture_output=True,text=True,check=True)
            assert mcp_eval.check(task,doc,regen.stdout,binary,root)==(True,[])
        return {'case':case,'mcp_requests':mcp.count,'agent_requests':agent.count,'triangles':len(triangles(data))}
    finally:mcp.close();agent.close()

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--musubicad',type=Path,required=True);parser.add_argument('--out',type=Path)
    args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='musubicad-datum-sketch-') as directory:
        root=Path(directory);cases=[]
        for case in ['XY','YZ','XZ','tilted','seeded_XY']:
            folder=root/case;folder.mkdir();cases.append(exercise(args.musubicad.resolve(),folder,case))
        result={'cases':cases,'model_turns':0,'typed_mcp_agent_parity':True,'source_association_and_parameter_motion':True,
                'readonly_atomic_identity_stale_refusal':True,'canonical_undo_redo':True,'independent_fresh_source_and_physical_eval':True,
                'equal_volume_wrong_placement_refused':True}
        if args.out:args.out.write_text(json.dumps(result,indent=2)+'\n')
        print(json.dumps(result))

if __name__=='__main__':main()
