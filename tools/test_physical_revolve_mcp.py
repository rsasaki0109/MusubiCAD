"""ADR-112 actual MCP/Agent parity, atomic source history and independent native physics."""
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
from rectangular_revolve_eval import graph_source, physical, patch_source_problems, source_problems
from sketch_creation_eval import triangles, volume
from test_cylindrical_draft_mcp import Client, canonical_digest, digest
ROOT = Path(__file__).resolve().parents[1]


def exercise(binary, root, plane, degrees, side=False, reverse=False, write_example=None):
    mcp, agent = Client(binary, 'mcp'), Client(binary, 'agent')
    doc, peer = root/'part.ocad.d', root/'peer.ocad.d'
    try:
        mcp.call('initialize', {'protocolVersion':'2025-06-18', 'capabilities':{}, 'clientInfo':{'name':'physical-revolve', 'version':'1'}})
        mcp.tool('new_document', {'path':str(doc), 'kind':'part', 'document_id':'doc:physical_rectangular_revolve', 'name':'Physical rectangular revolve'})
        shutil.copytree(doc, peer)
        initial, stable = canonical_digest(doc), digest(doc)
        patch = json.loads((ROOT/'examples/agent/physical_rectangular_revolve_patch.json').read_text())
        for o in patch['operations']:
            if o['type']=='add_parameter' and o['id']=='param:turn_angle': o['expr']=f'{degrees} deg'
            if o['type']=='add_sketch': o['workplane']['plane']=plane
            if o['type']=='add_sketch_entity' and o['entity']['type']=='point' and side:
                o['entity']['x']=f"{10-float(o['entity']['x'].split()[0]):g} mm"
            if o['type']=='add_feature':
                d=o['node']['definition']
                if d['type']=='datum_axis':
                    d['direction']={'XY':[0,1,0], 'YZ':[0,0,1], 'XZ':[0,0,1]}[plane]
                    if reverse:d['direction']=[-v for v in d['direction']]
                    d['origin_m']={'XY':[.005,0,0], 'YZ':[0,.005,0], 'XZ':[.005,0,0]}[plane]
                    d['origin_expr']=[None,'axis_u',None] if plane=='YZ' else ['axis_u',None,None]
                if d['type']=='revolve':d['angle_rad']=math.radians(degrees)
        def parity(tool, args, agent_method=None):
            result=mcp.tool(tool, {'path':str(doc), **args})
            twin=agent.call('opencad.'+(agent_method or tool), {'path':str(peer), **args})
            assert 'error' not in twin, twin
            other=twin['result']
            if tool=='patch_apply':
                assert result['patched']==str(doc) and other['patched']==str(peer)
                result={k:v for k,v in result.items() if k!='patched'}
                other={k:v for k,v in other.items() if k!='patched'}
            assert result==other, (result,other)
            assert canonical_digest(doc)==canonical_digest(peer)
            return result
        preview=parity('patch_dry_run', {'patch':patch,'verify':True}, 'patch_dry_run_document')
        assert preview['verification']['passed'] and digest(doc)==stable
        for mutate in ['unit', 'missing_axis', 'duplicate', 'zero', 'stale']:
            bad=copy.deepcopy(patch)
            if mutate=='unit':bad['operations'][1]['expr']='90 mm'
            elif mutate=='missing_axis':bad['operations'][-1]['node']['definition']['axis_feature']='feature:missing'
            elif mutate=='duplicate':bad['operations'][-1]['node']['id']='feature:revolve_axis'
            elif mutate=='zero':bad['operations'][1]['expr']='0 deg'
            else:bad['preconditions']=[{'type':'revision_equals','algorithm':'sha256','version':'musubicad.design-state.v2','digest':'0'*64}]
            mcp.tool('patch_apply', {'path':str(doc),'patch':bad,'verify':True}, error=True)
            assert 'error' in agent.call('opencad.patch_apply_document', {'path':str(peer),'patch':bad,'verify':True})
            assert digest(doc)==stable and canonical_digest(peer)==initial
        result=parity('patch_apply', {'patch':patch,'verify':True}, 'patch_apply_document')
        history=result['history']; created=canonical_digest(doc)
        assert result['verification']['passed']
        assert patch_source_problems(doc, patch)==[], patch_source_problems(doc, patch)
        if write_example is not None:
            assert not write_example.exists(), 'refuse overwriting a fixture'
            shutil.copytree(doc, write_example)
        for op,wanted in [('undo',initial),('redo',created)]:
            result=agent.call('opencad.history_'+op+'_document',{'path':str(doc),'history':history})['result']
            twin=agent.call('opencad.history_'+op+'_document',{'path':str(peer),'history':history})['result']
            assert result==twin;history=result['history'];assert canonical_digest(doc)==wanted==canonical_digest(peer)
        stl=root/'actual.stl';mcp.tool('export_document',{'path':str(doc),'output':str(stl)})
        data=stl.read_bytes();assert physical(data,plane,degrees,negative_side=side,negative_axis=reverse)==[], physical(data,plane,degrees,negative_side=side,negative_axis=reverse)
        shifted=bytearray(data)
        for i,face in enumerate(triangles(data)):
            for j,p in enumerate(face):struct.pack_into('<3f', shifted,84+50*i+12+12*j,p[0]+.005,p[1],p[2])
        assert abs(volume(triangles(data))-volume(triangles(shifted)))<1e-10
        assert physical(shifted,plane,degrees,negative_side=side,negative_axis=reverse)
        if plane=='XY' and degrees==90 and not side:
            baseline=graph_source(doc)
            edit={'operations':[{'type':'set_parameter','id':'param:axis_u','expr':'10 mm'},{'type':'set_parameter','id':'param:turn_angle','expr':'180 deg'}]}
            result=parity('patch_apply',{'patch':edit,'history':history,'verify':True},'patch_apply_document');history=result['history']
            desired={'parameters':{'param:axis_u':'10 mm','param:turn_angle':'180 deg'}}
            assert source_problems(doc,desired,baseline)==[]
            mcp.tool('export_document',{'path':str(doc),'output':str(stl)})
            assert physical(stl.read_bytes(),'XY',180,10)==[],physical(stl.read_bytes(),'XY',180,10)
            task=next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id']=='edit_physical_rectangular_revolve')
            regen=subprocess.run([str(binary),'regen',str(doc)],capture_output=True,text=True,check=True,timeout=60)
            assert mcp_eval.check(task,doc,regen.stdout,binary,root)==(True,[])
            wrong=root/'wrong.ocad.d';shutil.copytree(doc,wrong)
            for mutation in ['axis','angle','profile','unrelated']:
                current=copy.deepcopy(graph_source(doc))
                if mutation=='axis':next(n for n in current['features']['feature_nodes'] if n['id']=='feature:physical_revolve')['definition'].pop('axis_feature')
                elif mutation=='angle':next(n for n in current['features']['feature_nodes'] if n['id']=='feature:physical_revolve')['definition']['angle_expr']='180 deg'
                elif mutation=='profile':current['sketches']['sketches'][0]['entities'][0]['x']='16 mm'
                else:current['parameters']['parameters']['param:axis_u']['name']='wrong'
                for name,value in current.items():(wrong/'graph'/f'{name}.json').write_text(json.dumps(value))
                assert source_problems(wrong,desired,baseline),mutation
            result=agent.call('opencad.history_undo_document',{'path':str(doc),'history':history})['result'];assert canonical_digest(doc)==created
        return {'plane':plane,'degrees':degrees,'negative_side':side,'negative_axis':reverse,'mcp_requests':mcp.count,'agent_requests':agent.count,'triangles':len(triangles(data))}
    finally:
        mcp.close();agent.close()


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--musubicad',type=Path,required=True);parser.add_argument('--write-example',type=Path);parser.add_argument('--out',type=Path);args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='musubicad-physical-revolve-') as folder:
        root=Path(folder);cases=[]
        for i,(plane,degrees,side,reverse) in enumerate([('XY',90,False,False),('YZ',90,False,False),('XZ',90,False,False),('XY',180,True,False),('XY',270,False,True),('XY',360,False,False)]):
            path=root/str(i);path.mkdir();cases.append(exercise(args.musubicad.resolve(),path,plane,degrees,side,reverse,args.write_example if i==0 else None))
        report={'cases':cases,'model_turns':0,'mcp_agent_parity':True,'readonly_atomic_refusal_history':True,'independent_source_physics':True,'equal_volume_wrong_placement_refused':True,'axis_association_and_source_decoys_refused':True}
        if args.out:args.out.write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(report))
if __name__=='__main__':main()
