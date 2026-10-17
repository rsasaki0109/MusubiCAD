"""Actual three-point ellipse parity, history and independent oriented SI grading."""
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
from pointer_ellipse_eval import CASES,VOLUME,graph_source,placed_problems,source_problems
from sketch_creation_eval import physical_problems,triangles,volume
from test_cylindrical_draft_mcp import Client,digest,canonical_digest

ROOT=Path(__file__).resolve().parents[1]

def exercise(binary,root,case):
    mcp=Client(binary,'mcp');agent=Client(binary,'agent');doc=root/'part.ocad.d';other=root/'other.ocad.d'
    try:
        mcp.call('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'pointer-ellipse','version':'1'}})
        catalog=mcp.call('tools/list',{})['result']['tools']
        schema=next(t for t in catalog if t['name']=='plan_sketch_edit')['inputSchema']
        variants=schema['properties']['edit']['properties']['operations']['items']['oneOf']
        form=next(v for v in variants if v['properties']['operation'].get('const')=='ellipse_axes')
        assert set(form['required'])=={'operation','id','center_x','center_y','major_x','major_y','through_x','through_y','construction'} and not form['additionalProperties']
        datum=case in ['tilted','seeded_XY']
        if case=='seeded_XY':shutil.copytree(ROOT/'examples/datum_plate.ocad.d',doc)
        else:
            mcp.tool('new_document',{'path':str(doc),'kind':'part','document_id':f'doc:ellipse_{case}','name':'Pointer ellipse'})
            if datum:
                node={'id':'feature:datum','name':'Tilted datum','definition':{'type':'datum_plane','base':{'type':'custom','origin':[.01,-.02,.03],'normal':[0,.6,.8],'x_axis':[1,0,0]},'offset':{'value_si':.005}}}
                mcp.tool('patch_apply',{'path':str(doc),'patch':{'operations':[{'type':'add_feature','node':node,'position':{'at':'end'}}]}})
        baseline=graph_source(doc);initial=canonical_digest(doc);shutil.copytree(doc,other)
        state=mcp.tool('inspect_sketch_creation',{'path':str(doc)})
        assert state==agent.call('opencad.inspect_sketch_creation',{'path':str(other)})['result']
        create={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':state['suggested_sketch_id'],'feature_id':state['suggested_feature_id'],'name':'Pointer ellipse',**({'datum_plane':'feature:datum'} if datum else {'plane':case})}
        response=mcp.tool('apply_sketch_create',{'path':str(doc),'create':create})
        assert response==agent.call('opencad.apply_sketch_create',{'path':str(other),'create':create})['result']
        history=response['history']['history'];other_history=copy.deepcopy(history)
        empty=canonical_digest(doc);state=mcp.tool('inspect_sketches',{'path':str(doc)})
        operations=json.loads((ROOT/'examples/agent/create_pointer_ellipse.json').read_text())['operations']
        edit={'document_id':state['document_id'],'revision':state['revision'],'sketch_id':create['sketch_id'],'operations':operations}
        before=digest(doc);plan=mcp.tool('plan_sketch_edit',{'path':str(doc),'edit':edit})
        assert plan==agent.call('opencad.plan_sketch_edit',{'path':str(other),'edit':edit})['result']
        assert digest(doc)==before and len(plan['patch']['operations'])==2
        for mutation in [{'major_x':'2 mm','major_y':'-3 mm'},{'through_x':'26 mm','through_y':'15 mm'},
                         {'through_x':'18 mm','through_y':'9 mm'},{'through_x':'82 mm','through_y':'57 mm'},
                         {'through_x':'-34 mm','through_y':'45 mm'},{'center_x':'2'},{'major_y':'15 deg'},
                         {'major_x':'1e999 mm'},{'major_x':'named_length'},{'minor_radius':'20 mm'}]:
            bad=copy.deepcopy(edit);bad['operations'][0].update(mutation)
            mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':bad,'history':history},error=True);assert digest(doc)==before
        for bad in [{**edit,'document_id':'doc:foreign'},{k:v for k,v in edit.items() if k!='document_id'}]:
            mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':bad,'history':history},error=True);assert digest(doc)==before
        response=mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':edit,'history':history})
        parity=agent.call('opencad.apply_sketch_edit',{'path':str(other),'edit':edit,'history':other_history})['result']
        assert response==parity and canonical_digest(doc)==canonical_digest(other)
        history=response['history']['history'];other_history=parity['history']['history'];ellipse=canonical_digest(doc)
        for operation,wanted in [('undo',empty),('redo',ellipse)]:
            result=agent.call(f'opencad.history_{operation}_document',{'path':str(doc),'history':history})['result'];history=result['history']
            other_history=agent.call(f'opencad.history_{operation}_document',{'path':str(other),'history':other_history})['result']['history']
            assert canonical_digest(doc)==wanted and canonical_digest(doc)==canonical_digest(other)
        stable=digest(doc);mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':edit,'history':history},error=True);assert digest(doc)==stable
        current=mcp.tool('inspect_sketches',{'path':str(doc)});collision={**edit,'revision':current['revision']}
        mcp.tool('apply_sketch_edit',{'path':str(doc),'edit':collision,'history':history},error=True);assert digest(doc)==stable
        tree=mcp.tool('list_features',{'path':str(doc)});owner=next(r for r in tree['rows'] if r['id']==create['feature_id'])
        body={'operation':'create_extrude','id':'feature:pointer_body','name':'Pointer ellipse prism','sketch_feature':owner['id'],'profile_ref':owner['closed_profiles'][0],'length_expr':'6 mm'}
        response=mcp.tool('apply_feature_edit',{'path':str(doc),'revision':tree['revision'],'edit':body,'history':history})
        parity=agent.call('opencad.apply_feature_edit',{'path':str(other),'revision':tree['revision'],'edit':body,'history':other_history})['result']
        assert response==parity and canonical_digest(doc)==canonical_digest(other)
        history=response['history']['history'];assert abs(response['evidence']['volume_m3']-VOLUME)<1e-12
        desired={'sketch_id':create['sketch_id'],'feature_id':owner['id'],'body_feature':'feature:pointer_body','workplane':{'type':'datum_plane','feature_id':'feature:datum'} if datum else {'type':'global','plane':case},'case':case}
        assert source_problems(doc,desired,baseline)==[],source_problems(doc,desired,baseline)
        wrong_source=root/'wrong_source.ocad.d';shutil.copytree(doc,wrong_source);source_path=wrong_source/'graph/sketches.json';authored=json.loads(source_path.read_text())
        for mutation in ['projected_width','orientation','construction','extra_point','center']:
            candidate=copy.deepcopy(authored);sketch=next(s for s in candidate['sketches'] if s['id']==create['sketch_id']);e=next(x for x in sketch['entities'] if x['type']=='ellipse')
            if mutation=='projected_width':e['minor_radius']='12.000000000000 mm'
            elif mutation=='orientation':e['axis_angle']='5.63968419838630200 rad'
            elif mutation=='construction':e['construction']=True
            elif mutation=='extra_point':sketch['entities'].append({'type':'point','id':'ent:unused_axis','construction':False,'x':'0 mm','y':'0 mm'})
            else:next(x for x in sketch['entities'] if x['type']=='point')['x']='3.000 mm'
            source_path.write_text(json.dumps(candidate));assert source_problems(wrong_source,desired,baseline),mutation
        source_path.write_text(json.dumps(authored));assert source_problems(wrong_source,desired,baseline)==[]
        stl=root/'actual.stl';mcp.tool('export_document',{'path':str(doc),'output':str(stl)});data=stl.read_bytes()
        assert placed_problems(data,case)==[],placed_problems(data,case)
        # A proper 180-degree rotation about the local X axis through the COM
        # flips ellipse orientation while preserving volume, COM and world bounds.
        bounds,center,_,_,axis=CASES[case];reflected=bytearray(data);translated=bytearray(data)
        for i,face in enumerate(triangles(data)):
            normal=struct.unpack_from('<3f',data,84+50*i)
            along_normal=sum(normal[k]*axis[k] for k in range(3))
            struct.pack_into('<3f',reflected,84+50*i,*[2*along_normal*axis[k]-normal[k] for k in range(3)])
            for j,p in enumerate(face):
                d=[p[k]-center[k] for k in range(3)];along=sum(d[k]*axis[k] for k in range(3))
                q=[center[k]+2*along*axis[k]-d[k] for k in range(3)]
                struct.pack_into('<3f',reflected,84+50*i+12+12*j,*q)
                struct.pack_into('<3f',translated,84+50*i+12+12*j,p[0]+.005,p[1],p[2])
        assert physical_problems(reflected,bounds=bounds,expected_center=center,expected_volume=VOLUME)==[],physical_problems(reflected,bounds=bounds,expected_center=center,expected_volume=VOLUME)
        assert placed_problems(reflected,case), 'opposite ellipse orientation must fail even with matching mass/COM/world bounds'
        assert math.isclose(volume(triangles(translated)),volume(triangles(data)),rel_tol=1e-6,abs_tol=1e-12) and placed_problems(translated,case)
        final=canonical_digest(doc)
        for operation,wanted in [('undo',initial),('redo',final)]:
            for _ in range(3):history=agent.call(f'opencad.history_{operation}_document',{'path':str(doc),'history':history})['result']['history']
            assert canonical_digest(doc)==wanted
        if case=='seeded_XY':
            task=next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id']=='create_pointer_ellipse')
            regen=subprocess.run([str(binary),'regen',str(doc)],capture_output=True,text=True,check=True)
            assert mcp_eval.check(task,doc,regen.stdout,binary,root)==(True,[])
        return {'case':case,'mcp_requests':mcp.count,'agent_requests':agent.count,'triangles':len(triangles(data))}
    finally:mcp.close();agent.close()

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--musubicad',type=Path,required=True);parser.add_argument('--out',type=Path);args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='musubicad-pointer-ellipse-') as folder:
        root=Path(folder);cases=[]
        for case in ['XY','YZ','XZ','tilted','seeded_XY']:
            path=root/case;path.mkdir();cases.append(exercise(args.musubicad.resolve(),path,case))
        report={'cases':cases,'model_turns':0,'typed_mcp_agent_parity':True,'readonly_atomic_refusal':True,'single_batch_undo_redo':True,'independent_source_and_physical_eval':True,'equal_volume_wrong_placement_refused':True,'invalid_source_decoys_refused':True,'matching_mass_bounds_wrong_orientation_refused':True}
        if args.out:args.out.write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(report))

if __name__=='__main__':main()
