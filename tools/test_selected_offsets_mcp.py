"""Actual typed MCP/Agent offsets, parity, literal source, refusal and SI output."""
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
from selected_offset_eval import graph_source,source_problems,placed_problems
from sketch_creation_eval import triangles,volume
from test_cylindrical_draft_mcp import Client,digest,canonical_digest

ROOT=Path(__file__).resolve().parents[1]

def exercise(binary,root,case,fixture_output=None):
    mcp=Client(binary,'mcp');agent=Client(binary,'agent');doc=root/'part.ocad.d';peer=root/'peer.ocad.d'
    try:
        mcp.call('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'selected-offsets','version':'1'}})
        if case=='seeded_XY':shutil.copytree(ROOT/'examples/datum_plate.ocad.d',doc)
        else:
            mcp.tool('new_document',{'path':str(doc),'kind':'part','document_id':f'doc:offset_{case}','name':'Selected offsets'})
            if case=='tilted':
                node={'id':'feature:datum','name':'Tilted datum','definition':{'type':'datum_plane','base':{'type':'custom','origin':[.01,-.02,.03],'normal':[0,.6,.8],'x_axis':[1,0,0]},'offset':{'value_si':.005}}}
                mcp.tool('patch_apply',{'path':str(doc),'patch':{'operations':[{'type':'add_feature','node':node,'position':{'at':'end'}}]}})
        shutil.copytree(doc,peer)
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
        recipe=json.loads((ROOT/'examples/agent/offset_selected_sketch.json').read_text())
        parity('patch_apply',{'patch':{'operations':[{'type':'add_parameter',**p} for p in recipe['parameters']]}})
        state=parity('inspect_sketch_creation',{});datum=case in ['tilted','seeded_XY']
        workplane={'type':'datum_plane','feature_id':'feature:datum'} if datum else {'type':'global','plane':case}
        create={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':state['suggested_sketch_id'],'feature_id':state['suggested_feature_id'],'name':'Offset source',**({'datum_plane':'feature:datum'} if datum else {'plane':case})}
        result=parity('apply_sketch_create',{'create':create});history=result['history']['history']
        state=parity('inspect_sketches',{})
        edit={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':create['sketch_id'],'operations':recipe['primitives']}
        result=parity('apply_sketch_edit',{'edit':edit,'history':history});history=result['history']['history']
        result=parity('patch_apply',{'patch':{'operations':[{'type':'add_sketch_constraint','sketch_id':create['sketch_id'],'constraint':c} for c in recipe['constraints']]},'history':history});history=result['history']
        tree=parity('list_features',{})
        source_body={'operation':'create_extrude','id':'feature:offset_source_body','name':'Original circle body','sketch_feature':create['feature_id'],'profile_ref':'profile:circle:ent:offset_disc','length_expr':'6 mm'}
        result=parity('apply_feature_edit',{'revision':tree['revision'],'edit':source_body,'history':history});history=result['history']['history']
        if fixture_output is not None:shutil.copytree(doc,fixture_output)
        baseline=graph_source(doc);before=canonical_digest(doc);stable=digest(doc);saved=copy.deepcopy(history)
        state=parity('inspect_sketches',{});row=next(s for s in state['sketches'] if s['source']['id']==create['sketch_id'])
        assert row['offset_targets']==['ent:offset_disc',*recipe['rectangle_curve_ids']]
        offset={**recipe['offset'],'document_id':state['document_id'],'revision':state['revision']}
        for key,value in [('distance','3'),('distance','-3 mm'),('distance','0 mm'),('distance','3 deg'),('distance','diameter'),
                          ('source_sketch_id','sketch:missing'),('document_id','doc:foreign'),('revision','stale'),
                          ('association','associative'),('destination_sketch_id','sketch:forged'),('curve_ids',[]),('curve_ids',['ent:offset_disc']*2),('curve_ids',['ent:offset_a']),('curve_ids',[recipe['rectangle_curve_ids'][0]])]:
            bad={**offset,key:value};mcp.tool('apply_sketch_offset',{'path':str(doc),'offset':bad,'history':history},error=True)
            assert 'error' in agent.call('opencad.apply_sketch_offset',{'path':str(peer),'offset':bad,'history':history})
            assert digest(doc)==stable and canonical_digest(peer)==before and history==saved
        for ids,distance in [(recipe['rectangle_curve_ids'][::-1],'3 mm'),(recipe['rectangle_curve_ids'],'30 mm'),(['ent:offset_disc'],'10 mm')]:
            bad={**offset,'curve_ids':ids,'side':'inward','distance':distance}
            mcp.tool('apply_sketch_offset',{'path':str(doc),'offset':bad,'history':history},error=True);assert digest(doc)==stable
        plan=parity('plan_sketch_offset',{'offset':offset})
        assert plan['destination_sketch_id']=='sketch:offset_1' and plan['destination_feature_id']=='feature:offset_1'
        assert plan['association']=='independent_copy' and abs(plan['signed_distance_m']-.003)<1e-12 and len(plan['patch']['operations'])==4
        assert abs(plan['curves'][0]['radius_m']-.013)<1e-12 and abs(plan['evidence']['volume_m3']-600*math.pi*1e-9)<1e-12
        assert digest(doc)==stable
        inward=parity('plan_sketch_offset',{'offset':{**offset,'side':'inward'}});assert abs(inward['curves'][0]['radius_m']-.007)<1e-12
        result=parity('apply_sketch_offset',{'offset':offset,'history':history});history=result['history']['history'];after=canonical_digest(doc)
        for operation,wanted in [('undo',before),('redo',after)]:
            result=agent.call(f'opencad.history_{operation}_document',{'path':str(doc),'history':history})['result']
            twin=agent.call(f'opencad.history_{operation}_document',{'path':str(peer),'history':history})['result'];assert result==twin
            history=result['history'];assert canonical_digest(doc)==wanted and canonical_digest(peer)==wanted
        tree=parity('list_features',{});body={'operation':'create_extrude','id':'feature:offset_body','name':'Independent offset body','sketch_feature':plan['destination_feature_id'],'profile_ref':next(r for r in tree['rows'] if r['id']==plan['destination_feature_id'])['closed_profiles'][0],'length_expr':'6 mm'}
        result=parity('apply_feature_edit',{'revision':tree['revision'],'edit':body,'history':history});history=result['history']['history']
        desired={'copy_sketch':'sketch:offset_1','copy_owner':'feature:offset_1','body_feature':'feature:offset_body','workplane':workplane,'case':case}
        assert source_problems(doc,desired,baseline)==[],source_problems(doc,desired,baseline)
        stl=root/'actual.stl';parity('export_document',{'output':str(stl)});data=stl.read_bytes();assert placed_problems(data,case)==[],placed_problems(data,case)
        translated=bytearray(data)
        for i,face in enumerate(triangles(data)):
            for j,p in enumerate(face):struct.pack_into('<3f',translated,84+50*i+12+12*j,p[0]+.005,p[1],p[2])
        assert math.isclose(volume(triangles(data)),volume(triangles(translated)),rel_tol=1e-6) and placed_problems(translated,case)
        wrong=root/'wrong.ocad.d';shutil.copytree(doc,wrong);path=wrong/'graph/sketches.json';original=json.loads(path.read_text())
        for mutation in ['wrong_side','wrong_center','associated_radius','changed_source','removed_source_constraint','wrong_flag','nan_radius']:
            candidate=copy.deepcopy(original);sketches={s['id']:s for s in candidate['sketches']};copy_entities=sketches['sketch:offset_1']['entities'];source_entities=sketches[create['sketch_id']]['entities']
            if mutation=='wrong_side':copy_entities[1]['radius']=.007
            elif mutation=='wrong_center':copy_entities[0]['x']=.007
            elif mutation=='associated_radius':copy_entities[1]['radius']='diameter / 4 + 8 mm'
            elif mutation=='changed_source':source_entities[0]['x']='2 mm'
            elif mutation=='removed_source_constraint':sketches[create['sketch_id']]['constraints']=[]
            elif mutation=='wrong_flag':copy_entities[0]['construction']=True
            else:copy_entities[1]['radius']=float('nan')
            path.write_text(json.dumps(candidate));assert source_problems(wrong,desired,baseline),mutation
        if case=='seeded_XY' and fixture_output is None:
            task=next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id']=='offset_selected_sketch')
            regen=subprocess.run([str(binary),'regen',str(doc)],capture_output=True,text=True,check=True)
            assert mcp_eval.check(task,doc,regen.stdout,binary,root)==(True,[])
            assert not mcp_eval.check(task,doc,regen.stdout)[0]
        return {'case':case,'mcp_requests':mcp.count,'agent_requests':agent.count,'triangles':len(triangles(data))}
    finally:mcp.close();agent.close()

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--musubicad',type=Path,required=True);args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='musubicad-selected-offsets-') as folder:
        cases=[]
        for case in ['XY','YZ','XZ','tilted','seeded_XY']:
            root=Path(folder)/case;root.mkdir();cases.append(exercise(args.musubicad.resolve(),root,case))
        print(json.dumps({'cases':cases,'model_turns':0,'ordered_independent_copies_and_generated_owners':True,'typed_mcp_agent_parity':True,'readonly_atomic_refusal_history':True,'independent_source_physics_and_adversaries':True}))

if __name__=='__main__':main()
