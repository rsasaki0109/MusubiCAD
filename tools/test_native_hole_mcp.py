"""ADR-111 actual typed MCP/Agent creation, refusal, history and independent eval."""
import argparse
import copy
import json
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import mcp_eval
from native_hole_eval import graph_source, oracle, source_problems, placed_problems
from sketch_creation_eval import triangles, volume
from test_cylindrical_draft_mcp import Client, digest, canonical_digest

ROOT=Path(__file__).resolve().parents[1]

def exercise(binary,root,seat,bottom,write_example=None,depth_mm=10):
    mcp,agent=Client(binary,'mcp'),Client(binary,'agent')
    doc,peer=root/'part.ocad.d',root/'peer.ocad.d'
    try:
        mcp.call('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'native-hole-creation','version':'1'}})
        mcp.tool('new_document',{'path':str(doc),'kind':'part','document_id':'doc:native_hole_stock','name':'Native hole stock'})
        patch=json.loads((ROOT/'examples/agent/create_native_hole_stock_patch.json').read_text())
        seed=mcp.tool('patch_apply',{'path':str(doc),'patch':patch})
        assert seed['verification']['passed'] is True
        if write_example is not None:
            assert not write_example.exists(), 'refuse overwriting an example'
            shutil.copytree(doc,write_example)
        shutil.copytree(doc,peer)
        baseline=graph_source(doc);before=canonical_digest(doc);stable=digest(doc)
        def parity(tool,args):
            result=mcp.tool(tool,{'path':str(doc),**args})
            twin=agent.call(f'opencad.{tool}',{'path':str(peer),**args})
            assert twin.get('result')==result,twin
            assert canonical_digest(doc)==canonical_digest(peer)
            return result
        state=parity('inspect_hole_creation',{})
        assert len(state['choices'])==2 and state['suggested_feature_id']=='feature:hole_1'
        choice=next(c for c in state['choices'] if c['face_ref']==('ref:face:plate_bottom' if bottom else 'ref:face:plate_entry'))
        assert choice['bore_diameter_m']==.0066 and choice['stock_thickness_m']==.01
        create=json.loads((ROOT/'examples/agent/create_native_hole.json').read_text())['create']
        create.update(document_id=state['document_id'],revision=state['revision'],face_ref=choice['face_ref'],sketch_feature=choice['sketch_feature'],profile_ref=choice['profile_ref'])
        if depth_mm!=10:create['depth']=f'{depth_mm} mm'
        create['seat']={'type':'none'} if seat=='none' else ({'type':'counterbore','diameter':'seat_diameter','depth':'seat_depth'} if seat=='counterbore' else {'type':'countersink','diameter':'seat_diameter','included_angle':'seat_angle'})
        for key,value in [('document_id','doc:foreign'),('revision','stale'),('target_feature','feature:entry_plane'),('face_ref','ref:face:missing'),('sketch_feature','feature:sketch_base'),('profile_ref','profile:circle:ent:seat_circle'),('depth','8'),('depth','12 mm'),('depth','seat_angle'),('cached_frame',{}),('name',' ')]:
            bad={**create,key:value}
            mcp.tool('apply_hole_create',{'path':str(doc),'create':bad},error=True)
            assert 'error' in agent.call('opencad.apply_hole_create',{'path':str(peer),'create':bad})
            assert digest(doc)==stable and canonical_digest(peer)==before
        plan=parity('plan_hole_create',{'create':create});expected,_=oracle(seat,bottom,depth_mm)
        assert plan['feature_id']=='feature:hole_1' and len(plan['patch']['operations'])==1
        assert plan['entry_reference']['ref_id']==create['face_ref'] and plan['entry_reference']['status'] not in ['missing','ambiguous'] and plan['entry_reference'].get('resolved_kernel_id') is not None
        assert abs(plan['evidence']['volume_m3']-expected)<1e-12 and abs(plan['removed_volume_m3']-(12e-6-expected))<1e-12
        assert plan['choice']['inward_direction']==([0,0,1] if bottom else [0,0,-1])
        assert digest(doc)==stable
        result=parity('apply_hole_create',{'create':create});history=result['history']['history'];after=canonical_digest(doc)
        for operation,wanted in [('undo',before),('redo',after)]:
            result=agent.call(f'opencad.history_{operation}_document',{'path':str(doc),'history':history})['result']
            twin=agent.call(f'opencad.history_{operation}_document',{'path':str(peer),'history':history})['result']
            assert result==twin;history=result['history'];assert canonical_digest(doc)==wanted and canonical_digest(peer)==wanted
        regenerated=subprocess.run([str(binary),'regen',str(doc),'--json'],capture_output=True,text=True,check=True)
        provenance=json.loads(regenerated.stdout)['reference_provenance']
        assert all(p['status'] not in ['missing','ambiguous'] and p.get('resolved_kernel_id') is not None for p in provenance),provenance
        desired={'seat':seat,'bottom':bottom,'depth':create['depth'],'depth_m':depth_mm/1000}
        assert source_problems(doc,desired,baseline)==[],source_problems(doc,desired,baseline)
        stl=root/'actual.stl';mcp.tool('export_document',{'path':str(doc),'output':str(stl)});data=stl.read_bytes()
        assert placed_problems(data,seat,bottom,depth_mm)==[],placed_problems(data,seat,bottom,depth_mm)
        translated=bytearray(data)
        for i,face in enumerate(triangles(data)):
            for j,p in enumerate(face):struct.pack_into('<3f',translated,84+50*i+12+12*j,p[0]+.005,p[1],p[2])
        assert abs(volume(triangles(data))-volume(triangles(translated)))<1e-11
        assert placed_problems(translated,seat,bottom,depth_mm)
        wrong=root/'wrong.ocad.d';shutil.copytree(doc,wrong)
        for mutation in ['axis','entry','standard','seat','stock','axis_source','unrelated_axis','parameter','nonfinite']:
            candidate=copy.deepcopy(graph_source(doc));nodes=candidate['features']['feature_nodes'];hole=next(n for n in nodes if n['id']=='feature:hole_1')['definition']
            if mutation=='axis':hole['sketch_feature']='feature:sketch_base'
            elif mutation=='entry':hole['face_ref']='ref:face:plate_entry' if bottom else 'ref:face:plate_bottom'
            elif mutation=='standard':hole['standard']['fit']='loose'
            elif mutation=='seat':hole['type']='standard_hole' if seat!='none' else 'seated_standard_hole'
            elif mutation=='stock':next(n for n in nodes if n['id']=='feature:extrude_base')['definition']['length_expr']='10 mm'
            elif mutation in ['axis_source','unrelated_axis','nonfinite']:
                axis=next(s for s in candidate['sketches']['sketches'] if s['id']==('sketch:bottom_axis' if mutation=='unrelated_axis' else 'sketch:seat'))
                axis['entities'][0]['x']=float('nan') if mutation=='nonfinite' else '13 mm'
            else:candidate['parameters']['parameters']['param:hole_x']['expr']='13 mm'
            for name,value in candidate.items():(wrong/'graph'/f'{name}.json').write_text(json.dumps(value))
            assert source_problems(wrong,desired,baseline),mutation
        saved=digest(doc)
        mcp.tool('apply_hole_create',{'path':str(doc),'create':create,'history':history},error=True);assert digest(doc)==saved
        assert parity('inspect_hole_creation',{})['choices']==[]
        if not bottom and write_example is None and (seat=='counterbore' or depth_mm==8):
            task_id='create_native_blind_hole' if depth_mm==8 else 'create_native_standard_hole'
            task=next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id']==task_id)
            regen=subprocess.run([str(binary),'regen',str(doc)],capture_output=True,text=True,check=True)
            assert mcp_eval.check(task,doc,regen.stdout,binary,root)==(True,[])
            assert not mcp_eval.check(task,doc,regen.stdout)[0]
        return {'seat':seat,'bottom':bottom,'depth_mm':depth_mm,'mcp_requests':mcp.count,'agent_requests':agent.count,'triangles':len(triangles(data))}
    finally:mcp.close();agent.close()

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--musubicad',type=Path,required=True);parser.add_argument('--write-example',type=Path);args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='musubicad-native-hole-') as folder:
        cases=[]
        for seat in ['none','counterbore','countersink']:
            for bottom in [False,True]:
                root=Path(folder)/f'{seat}-{bottom}';root.mkdir()
                write=args.write_example if not cases else None
                cases.append(exercise(args.musubicad.resolve(),root,seat,bottom,write))
        for bottom in [False,True]:
            root=Path(folder)/f'blind-{bottom}';root.mkdir();cases.append(exercise(args.musubicad.resolve(),root,'none',bottom,depth_mm=8))
        print(json.dumps({'cases':cases,'model_turns':0,'typed_mcp_agent_parity':True,'readonly_atomic_refusal_history':True,'independent_source_contacts_physics_and_adversaries':True}))

if __name__=='__main__':main()
