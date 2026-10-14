"""ADR-066 real native MCP authoring/typed edits/refusal/history; zero model calls."""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import mcp_eval
from test_cylindrical_draft_mcp import Client, digest, canonical_digest, regenerate
ROOT=Path(__file__).resolve().parents[1]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--musubicad',type=Path,default=ROOT/'target/debug/musubicad');parser.add_argument('--out',type=Path);args=parser.parse_args();binary=args.musubicad.resolve()
    with tempfile.TemporaryDirectory(prefix='musubicad-tapered-rib-') as folder:
        root=Path(folder);doc=root/'rib.ocad.d';mcp=Client(binary,'mcp');agent=Client(binary,'agent')
        try:
            mcp.call('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'tapered-rib-regression','version':'1'}})
            mcp.tool('new_document',{'path':str(doc),'kind':'part','document_id':'doc:tapered_rib_plate','name':'Square-ended tapered rib plate'})
            create=json.loads((ROOT/'examples/agent/create_tapered_rib_plate_patch.json').read_text())
            initial=digest(doc);preview=mcp.tool('patch_dry_run',{'path':str(doc),'patch':create,'verify':False});assert preview['verification']['passed'] is True;assert digest(doc)==initial
            authored=mcp.tool('patch_apply',{'path':str(doc),'patch':create,'verify':False});assert authored['verification']['passed'] is True
            regenerate(binary,doc,13800);baseline=digest(doc);base_canonical=canonical_digest(doc)
            change={'operations':[{'type':'set_parameter','id':'param:rib_height','expr':'12 mm'}]}
            preview=mcp.tool('patch_dry_run',{'path':str(doc),'patch':change,'verify':False});assert preview['verification']['passed'] is True;assert digest(doc)==baseline
            mcp.tool('review_patch',{'path':str(doc),'patch':change,'output_dir':str(root/'review')});assert digest(doc)==baseline;assert (root/'review/review.json').exists()
            applied=mcp.tool('patch_apply',{'path':str(doc),'patch':change,'verify':False,'history':authored['history']});assert applied['verification']['passed'] is True
            output=regenerate(binary,doc,13224);edited=digest(doc);edit_canonical=canonical_digest(doc);history=applied['history']
            task=next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id']=='tapered_rib_height');assert mcp_eval.check(task,doc,output)==(True,[])
            for field,expr in [('draft_angle','-2 deg'),('base_thickness','6 mm')]:
                typed={'operations':[{'type':'set_feature_expr','feature_id':'feature:rib','field':field,'expr':expr}]}
                preview=mcp.tool('patch_dry_run',{'path':str(doc),'patch':typed,'verify':False});assert preview['verification']['passed'] is True;assert digest(doc)==edited
            suppress={'operations':[{'type':'set_feature_suppressed','id':'feature:rib','suppressed':True}]}
            preview=mcp.tool('patch_dry_run',{'path':str(doc),'patch':suppress,'verify':False});assert preview['verification']['passed'] is True;assert digest(doc)==edited
            wrong={'operations':[{'type':'add_semantic_ref','topo_ref':{'ref_id':'ref:face:wrong_end','kind':'face','semantic':{'created_by':'feature:extrude_base','role':'extrusion_start','normal_hint':[0,0,-1]}}},{'type':'set_feature_ref','feature_id':'feature:rib','field':'face_ref','ref_id':'ref:face:wrong_end'}]}
            refused=mcp.tool('patch_dry_run',{'path':str(doc),'patch':wrong,'verify':False});assert refused['verification']['passed'] is False
            mcp.tool('patch_apply',{'path':str(doc),'patch':wrong,'verify':False,'history':history},error=True);assert digest(doc)==edited
            for expr in ['100 mm','12 deg']:
                bad={'operations':[{'type':'set_parameter','id':'param:rib_height','expr':expr}]}
                mcp.tool('patch_apply',{'path':str(doc),'patch':bad,'verify':False,'history':history},error=True);assert digest(doc)==edited
            for mode,expected,source in [('undo',13800,base_canonical),('redo',13224,edit_canonical)]:
                response=agent.call(f'opencad.history_{mode}_document',{'path':str(doc),'history':history});assert 'error' not in response,response
                history=response['result']['history'];regenerate(binary,doc,expected);assert canonical_digest(doc)==source
            edited=digest(doc);badfile=root/'wrong.json';badfile.write_text(json.dumps(wrong));result=subprocess.run([str(binary),'patch',str(doc),str(badfile),'--no-verify'],capture_output=True,text=True,timeout=60);assert result.returncode!=0;assert digest(doc)==edited
            report={'mcp_requests':mcp.count,'agent_requests':agent.count,'model_turns':0,'volume_before_mm3':13800,'volume_after_mm3':13224,'forced_verification':True,'readonly_preview_review':True,'typed_angle_thickness_preview':True,'suppression_preview':True,'failure_preserved_source':True,'canonical_undo_redo':True,'independent_eval_passed':True}
            if args.out:args.out.write_text(json.dumps(report,indent=2)+'\n')
            print(json.dumps(report))
        finally:
            mcp.close();agent.close()
if __name__=='__main__':main()
