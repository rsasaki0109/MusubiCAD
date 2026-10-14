"""ADR-065 real MCP/Agent stdio regression; no model calls or credentials."""
from __future__ import annotations
import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess
import tempfile
import mcp_eval
ROOT = Path(__file__).resolve().parents[1]

class Client:
    def __init__(self, binary: Path, mode: str):
        self.process = subprocess.Popen([str(binary), mode], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8")
        self.count = 0
    def call(self, method: str, params: dict) -> dict:
        self.count += 1
        self.process.stdin.write(json.dumps({"jsonrpc":"2.0","id":self.count,"method":method,"params":params}) + "\n")
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        if not line:
            raise AssertionError("backend ended before replying: " + self.process.stderr.read()[-500:])
        response = json.loads(line)
        assert response["id"] == self.count
        return response
    def tool(self, name: str, arguments: dict, error: bool = False) -> dict:
        response = self.call("tools/call", {"name":name,"arguments":arguments})
        assert "error" not in response, response
        result = response["result"]
        assert result.get("isError", False) is error, result
        return result.get("structuredContent", result)
    def close(self):
        self.process.stdin.close()
        self.process.wait(timeout=30)
        assert self.process.returncode == 0, self.process.stderr.read()[-500:]

def digest(path: Path) -> str:
    h = hashlib.sha256()
    for p in sorted(x for x in path.rglob("*") if x.is_file()):
        h.update(p.relative_to(path).as_posix().encode() + b"\0" + p.read_bytes())
    return h.hexdigest()

def canonical_digest(path: Path) -> str:
    """Opaque history preserves JSON values/array order; object member order
    may change through serde transport (existing DocumentHistory contract)."""
    h = hashlib.sha256()
    for p in sorted(x for x in path.rglob("*") if x.is_file() and x.name!="checksums.json"):
        content=p.read_bytes()
        if not p.relative_to(path).as_posix().startswith("imports/"):
            content=json.dumps(json.loads(content),sort_keys=True,separators=(",",":" )).encode()
        h.update(p.relative_to(path).as_posix().encode()+b"\0"+content)
    return h.hexdigest()

def regenerate(binary: Path, path: Path, expected_mm3: float) -> str:
    r = subprocess.run([str(binary),"regen",str(path)], capture_output=True,text=True,check=True,timeout=60)
    actual = mcp_eval.regenerated_volume_mm3(r.stdout)
    assert actual is not None and abs(actual-expected_mm3)<0.0001, (actual,expected_mm3,r.stdout)
    return r.stdout

def main():
    parser=argparse.ArgumentParser();parser.add_argument("--musubicad",type=Path,default=ROOT/"target/debug/musubicad");parser.add_argument("--out",type=Path);args=parser.parse_args();binary=args.musubicad.resolve()
    with tempfile.TemporaryDirectory(prefix="musubicad-cylindrical_draft-") as temp:
        root=Path(temp);doc=root/"plate.ocad.d";mcp=Client(binary,"mcp");agent=Client(binary,"agent")
        try:
            mcp.call("initialize",{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"cylindrical_draft-regression","version":"1"}})
            mcp.tool("new_document",{"path":str(doc),"kind":"part","document_id":"doc:cylindrical_draft","name":"Cylindrical neutral-plane draft"})
            create=json.loads((ROOT/"examples/agent/create_cylindrical_draft_patch.json").read_text())
            before=digest(doc)
            preview=mcp.tool("patch_dry_run",{"path":str(doc),"patch":create,"verify":False});assert preview["verification"]["passed"] is True
            assert digest(doc)==before
            authored=mcp.tool("patch_apply",{"path":str(doc),"patch":create,"verify":False});assert authored["verification"]["passed"] is True
            expected_before=math.pi*(20**2*30 + math.tan(math.radians(7))**2*30**3/12)
            regenerate(binary,doc,expected_before);baseline=digest(doc);baseline_canonical=canonical_digest(doc)
            change={"operations":[{"type":"set_parameter","id":"param:draft_angle","expr":"-5 deg"}]}
            preview=mcp.tool("patch_dry_run",{"path":str(doc),"patch":change,"verify":False});assert preview["verification"]["passed"] is True;assert digest(doc)==baseline
            mcp.tool("review_patch",{"path":str(doc),"patch":change,"output_dir":str(root/"review")});assert digest(doc)==baseline;assert (root/"review/review.json").exists()
            applied=mcp.tool("patch_apply",{"path":str(doc),"patch":change,"verify":False,"history":authored["history"]});assert applied["verification"]["passed"] is True
            expected_after=math.pi*(20**2*30 + math.tan(math.radians(-5))**2*30**3/12);output=regenerate(binary,doc,expected_after)
            task=next(t for t in json.loads(mcp_eval.TASKS.read_text())["tasks"] if t["id"]=="cylindrical_draft_angle")
            assert mcp_eval.check(task,doc,output)==(True,[])
            edited=digest(doc);edited_canonical=canonical_digest(doc);history=applied["history"]
            wrong={"operations":[{"type":"add_semantic_ref","topo_ref":{"ref_id":"ref:face:opposite","kind":"face","semantic":{"created_by":"feature:stock","role":"top","normal_hint":[0,0,1]}}},{"type":"set_feature_ref","feature_id":"feature:draft","field":"face_ref","ref_id":"ref:face:opposite"}]}
            refused=mcp.tool("patch_dry_run",{"path":str(doc),"patch":wrong,"verify":False});assert refused["verification"]["passed"] is False
            mcp.tool("patch_apply",{"path":str(doc),"patch":wrong,"verify":False,"history":history},error=True);assert digest(doc)==edited
            bad_angle={"operations":[{"type":"set_parameter","id":"param:draft_angle","expr":"0 deg"}]}
            mcp.tool("patch_apply",{"path":str(doc),"patch":bad_angle,"verify":False,"history":history},error=True);assert digest(doc)==edited
            wrong_unit={"operations":[{"type":"set_parameter","id":"param:draft_angle","expr":"30 mm"}]}
            mcp.tool("patch_apply",{"path":str(doc),"patch":wrong_unit,"verify":False,"history":history},error=True);assert digest(doc)==edited
            for mode,expected,source in [("undo",expected_before,baseline_canonical),("redo",expected_after,edited_canonical)]:
                response=agent.call(f"opencad.history_{mode}_document",{"path":str(doc),"history":history});assert "error" not in response,response
                history=response["result"]["history"];regenerate(binary,doc,expected);assert canonical_digest(doc)==source
            edited=digest(doc)  # Current byte representation after canonical redo.
            badfile=root/"wrong.json";badfile.write_text(json.dumps(wrong));r=subprocess.run([str(binary),"patch",str(doc),str(badfile),"--no-verify"],capture_output=True,text=True,timeout=60);assert r.returncode!=0;assert digest(doc)==edited
            report={"mcp_requests":mcp.count,"agent_requests":agent.count,"model_turns":0,"volume_before_mm3":expected_before,"volume_after_mm3":expected_after,"forced_verification":True,"readonly_preview_review":True,"failure_preserved_source":True,"canonical_undo_redo":True,"independent_eval_passed":True}
            if args.out:args.out.write_text(json.dumps(report,indent=2)+"\n")
            print(json.dumps(report))
        finally:
            mcp.close();agent.close()
if __name__=="__main__":main()
