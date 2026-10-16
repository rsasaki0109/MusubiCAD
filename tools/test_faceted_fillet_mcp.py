"""Actual MCP authoring/edit/review, mandatory native checks, independent STL phase."""
from __future__ import annotations
import argparse
import copy
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import mcp_eval
from faceted_fillet_eval import physical_problems, triangles
from test_cylindrical_draft_mcp import Client, digest, canonical_digest, regenerate
ROOT = Path(__file__).resolve().parents[1]


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--musubicad',type=Path,default=ROOT/'target/debug/musubicad');parser.add_argument('--out',type=Path);args=parser.parse_args();binary=args.musubicad.resolve()
    task=next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id']=='faceted_circular_fillet_radius')
    with tempfile.TemporaryDirectory(prefix='musubicad-faceted-source-') as folder:
        root=Path(folder);doc=root/'fillet.ocad.d';mcp=Client(binary,'mcp');agent=Client(binary,'agent')
        try:
            mcp.call('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'faceted-fillet-regression','version':'1'}})
            mcp.tool('new_document',{'path':str(doc),'kind':'part','document_id':'doc:faceted_fillet','name':'Numerical faceted circular fillet'})
            create=json.loads((ROOT/'examples/agent/create_faceted_circular_fillet_patch.json').read_text())
            initial=digest(doc);preview=mcp.tool('patch_dry_run',{'path':str(doc),'patch':create,'verify':False});assert preview['verification']['passed'] is True;assert digest(doc)==initial
            authored=mcp.tool('patch_apply',{'path':str(doc),'patch':create,'verify':False});assert authored['verification']['passed'] is True
            before_volume=6222.97048807090
            regenerate(binary,doc,before_volume);baseline=digest(doc);before_canonical=canonical_digest(doc)
            definition=next(n for n in json.loads((doc/'graph/features.json').read_text())['feature_nodes'] if n['id']=='feature:fillet')['definition']
            edited=copy.deepcopy(definition);edited['maximum_radius']['value_si']=0.001
            change={'operations':[{'type':'replace_feature_definition','id':'feature:fillet','definition':edited}]}
            preview=mcp.tool('patch_dry_run',{'path':str(doc),'patch':change,'verify':False});assert preview['verification']['passed'] is True;assert digest(doc)==baseline
            mcp.tool('review_patch',{'path':str(doc),'patch':change,'output_dir':str(root/'review')});assert digest(doc)==baseline;assert (root/'review/review.json').exists();review_digest=digest(root/'review')
            applied=mcp.tool('patch_apply',{'path':str(doc),'patch':change,'verify':False,'history':authored['history']});assert applied['verification']['passed'] is True
            output=regenerate(binary,doc,task['expected_volume_mm3']);assert mcp_eval.check(task,doc,output,binary,root)==(True,[])
            after_canonical=canonical_digest(doc);history=applied['history'];before_failure=digest(doc)
            bad={'operations':[{'type':'replace_feature_definition','id':'feature:fillet','definition':definition},{'type':'set_feature_ref','feature_id':'feature:fillet','field':'edge_ref','ref_id':'ref:edge:missing'}]}
            refused=mcp.tool('patch_dry_run',{'path':str(doc),'patch':bad,'verify':False});assert any(m['level']=='error' for m in refused['validation']['messages']);assert 'verification' not in refused
            mcp.tool('patch_apply',{'path':str(doc),'patch':bad,'verify':False,'history':history},error=True);assert digest(doc)==before_failure;assert digest(root/'review')==review_digest
            bad_quality=copy.deepcopy(edited);bad_quality['quality']['max_integration_cells']=1
            physical_failure={'operations':[{'type':'replace_feature_definition','id':'feature:fillet','definition':definition},{'type':'replace_feature_definition','id':'feature:fillet','definition':bad_quality}]}
            refused=mcp.tool('patch_dry_run',{'path':str(doc),'patch':physical_failure,'verify':False});assert refused['verification']['passed'] is False
            mcp.tool('patch_apply',{'path':str(doc),'patch':physical_failure,'verify':False,'history':history},error=True);assert digest(doc)==before_failure;assert digest(root/'review')==review_digest
            badfile=root/'bad.json';badfile.write_text(json.dumps(physical_failure));refused=subprocess.run([str(binary),'patch',str(doc),str(badfile),'--no-verify'],capture_output=True,text=True,timeout=180);assert refused.returncode!=0;assert digest(doc)==before_failure
            for mode,expected_volume,source in [('undo',before_volume,before_canonical),('redo',task['expected_volume_mm3'],after_canonical)]:
                response=agent.call(f'opencad.history_{mode}_document',{'path':str(doc),'history':history});assert 'error' not in response,response
                history=response['result']['history'];regenerate(binary,doc,expected_volume);assert canonical_digest(doc)==source
            stl=root/'actual.stl';subprocess.run([str(binary),'export',str(doc),str(stl)],check=True,capture_output=True,timeout=180)
            actual=stl.read_bytes();assert physical_problems(actual,0.001,task['expected_volume_mm3'])==[]
            # Rotate the actual exported shell: unchanged volume, different authored phase.
            wrong=bytearray(actual)
            for i,face in enumerate(triangles(actual)):
                for j,(x,y,z) in enumerate(face):
                    struct.pack_into('<3f',wrong,84+50*i+12+12*j,-y,x,z)
            problems=physical_problems(wrong,0.001,task['expected_volume_mm3']);assert problems and 'phase' in problems[0],problems
            report={'mcp_requests':mcp.count,'agent_requests':agent.count,'model_turns':0,'forced_native_verification':True,'readonly_preview_review':True,'failed_batch_preserves_source_history_artifacts':True,'canonical_undo_redo':True,'source_and_physical_eval_passed':True,'wrong_phase_equal_volume_refused':True,'canonical_bounds_certify_native':False}
            if args.out:args.out.write_text(json.dumps(report,indent=2)+'\n')
            print(json.dumps(report))
        finally:
            mcp.close();agent.close()

if __name__=='__main__':main()
