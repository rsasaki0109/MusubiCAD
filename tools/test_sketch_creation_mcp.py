"""Actual MCP/Agent creation, source/history/refusal and independent mesh evidence."""
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
from sketch_creation_eval import physical_problems,source_problems,triangles,volume
from test_cylindrical_draft_mcp import Client,digest,canonical_digest

ROOT=Path(__file__).resolve().parents[1]

def exercise(binary,root,plane,seed=False):
    doc=root/f'{plane}.ocad.d';other=root/f'{plane}-parity.ocad.d'
    mcp=Client(binary,'mcp');agent=Client(binary,'agent')
    try:
        mcp.call('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'sketch-creation','version':'1'}})
        if seed:
            shutil.copytree(ROOT/'examples/ellipse_plate.ocad.d',doc)
        else:
            mcp.tool('new_document',{'path':str(doc),'kind':'part','document_id':f'doc:creation_{plane}','name':'Created sketch'})
        shutil.copytree(doc,other);before=digest(doc);original=canonical_digest(doc)
        state=mcp.tool('inspect_sketch_creation',{'path':str(doc)})
        assert agent.call('opencad.inspect_sketch_creation',{'path':str(other)})['result']==state
        create={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':state['suggested_sketch_id'],'feature_id':state['suggested_feature_id'],'name':'New manufacturing sketch','plane':plane}
        plan=mcp.tool('plan_sketch_create',{'path':str(doc),'create':create})
        assert plan==agent.call('opencad.plan_sketch_create',{'path':str(other),'create':create})['result']
        assert digest(doc)==before and len(plan['patch']['operations'])==2
        if not seed:
            assert plan['evidence']['volume_m3'] is None and plan['evidence']['bounds_size_m'] is None
        applied=mcp.tool('apply_sketch_create',{'path':str(doc),'create':create})
        parity=agent.call('opencad.apply_sketch_create',{'path':str(other),'create':create})
        assert parity['result']==applied and canonical_digest(doc)==canonical_digest(other)
        history=applied['history']['history'];stable=digest(doc)
        state=mcp.tool('inspect_sketch_creation',{'path':str(doc)})
        for bad in [create,{**create,'revision':state['revision'],'document_id':'doc:another'},
                    {**create,'revision':state['revision'],'sketch_id':state['suggested_sketch_id']},
                    {**create,'revision':state['revision'],'feature_id':state['suggested_feature_id']},
                    {**create,'revision':state['revision'],'plane':'datum_plane'}]:
            mcp.tool('apply_sketch_create',{'path':str(doc),'create':bad,'history':history},error=True)
            assert digest(doc)==stable
        # A point-only stage is an actual editable draft without a closed profile.
        for operations in [[{'operation':'point','id':'ent:new_center','x':'0 mm','y':'0 mm','create':True}],
                           [{'operation':'ellipse','id':'ent:new_ellipse','center':'ent:new_center','major_radius':'20 mm','minor_radius':'10 mm','axis_angle':'0 deg','construction':False,'create':True}]]:
            state=mcp.tool('inspect_sketches',{'path':str(doc)})
            edit={'revision':state['revision'],'sketch_id':create['sketch_id'],'operations':operations}
            before=digest(doc);mcp.tool('plan_sketch_edit',{'path':str(doc),'edit':edit});assert digest(doc)==before
            response=mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':edit,'history':history});history=response['history']['history']
        tree=mcp.tool('list_features',{'path':str(doc)})
        profile=next(r for r in tree['rows'] if r['id']==create['feature_id'])['closed_profiles'][0]
        edit={'operation':'create_extrude','id':'feature:new_body','name':'New ellipse prism','sketch_feature':create['feature_id'],'profile_ref':profile,'length_expr':'6 mm'}
        args={'path':str(doc),'revision':tree['revision'],'edit':edit}
        mcp.tool('plan_feature_edit',args)
        response=mcp.tool('apply_feature_edit',{**args,'history':history});history=response['history']['history']
        expected=math.pi*1200
        assert abs(response['evidence']['volume_m3']*1e9-expected)<.0001
        desired={'sketch_id':create['sketch_id'],'feature_id':create['feature_id'],'body_feature':'feature:new_body','plane':plane}
        assert source_problems(doc,desired)==[]
        stl=root/f'{plane}.stl';mcp.tool('export_document',{'path':str(doc),'output':str(stl)})
        actual=stl.read_bytes();assert physical_problems(actual,plane)==[],physical_problems(actual,plane)
        wrong=bytearray(actual)
        for i,face in enumerate(triangles(actual)):
            for j,(x,y,z) in enumerate(face):
                struct.pack_into('<3f',wrong,84+50*i+12+12*j,y,z,x)
        assert abs(volume(triangles(wrong))-volume(triangles(actual)))<1e-15
        assert physical_problems(wrong,plane) and 'world bounds' in physical_problems(wrong,plane)[0]
        changed=canonical_digest(doc)
        for mode,expected_source in [('undo',original),('redo',changed)]:
            for _ in range(4):
                result=agent.call(f'opencad.history_{mode}_document',{'path':str(doc),'history':history})
                assert 'error' not in result,result
                history=result['result']['history']
            assert canonical_digest(doc)==expected_source
        if seed:
            task=next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id']=='create_global_sketch')
            regen=subprocess.run([str(binary),'regen',str(doc)],capture_output=True,text=True,check=True)
            assert mcp_eval.check(task,doc,regen.stdout,binary,root)==(True,[])
            # Same volume/geometry with a different authored plane must fail source grading.
            altered=copy.deepcopy(desired);altered['plane']='YZ'
            assert source_problems(doc,altered)
        return {'plane':plane,'seeded_part':seed,'mcp_requests':mcp.count,'agent_requests':agent.count,'mesh_triangles':len(triangles(actual))}
    finally:
        mcp.close();agent.close()

def main():
    p=argparse.ArgumentParser();p.add_argument('--musubicad',type=Path,required=True);p.add_argument('--out',type=Path)
    args=p.parse_args()
    with tempfile.TemporaryDirectory(prefix='musubicad-sketch-create-') as folder:
        root=Path(folder);binary=args.musubicad.resolve()
        cases=[exercise(binary,root,plane) for plane in ['XY','YZ','XZ']]
        seeded=root/'seeded';seeded.mkdir();cases.append(exercise(binary,seeded,'XZ',True))
        report={'cases':cases,'model_turns':0,'typed_mcp_agent_parity':True,'readonly_and_atomic_refusal':True,'canonical_undo_redo':True,'independent_source_and_physical_eval':True,'equal_volume_wrong_plane_refused':True}
        if args.out:args.out.write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(report))

if __name__=='__main__':main()
