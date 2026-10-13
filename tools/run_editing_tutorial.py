"""Run the documented editing tutorial against real MCP/Agent processes.

Requires Python 3.10+. No model calls or simulated client-host evidence. Source fixtures are copied to
an automatically cleaned temporary directory; supplied binaries are executed.
"""
from __future__ import annotations
import argparse
import json
import math
import os
import queue
import threading
from pathlib import Path
import shutil
import subprocess
import tempfile
import traceback

ROOT = Path(__file__).resolve().parents[1]

def report_action_error(error: Exception, title: str) -> None:
    """Expose a real runner failure in Actions check annotations, without hiding it."""
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        return
    detail = ''.join(traceback.format_exception(type(error), error, error.__traceback__))[-12000:]
    detail = detail.replace('%', '%25').replace('\r', '%0D').replace('\n', '%0A')
    print(f'::error title={title}::{detail}', flush=True)

def snapshot(path: Path) -> dict[str, bytes]:
    return {p.relative_to(path).as_posix(): p.read_bytes() for p in path.rglob('*') if p.is_file()}

def logical(path: Path) -> dict:
    return {p.relative_to(path).as_posix(): json.loads(p.read_text(encoding='utf-8')) for p in path.rglob('*.json') if p.is_file() and p.name != 'checksums.json'}

class MCP:
    def __init__(self, binary: Path):
        self.process = subprocess.Popen([str(binary), 'mcp'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, encoding='utf-8')
        self.sequence = 0
        self.lines: queue.Queue[str] = queue.Queue()
        def read_lines():
            assert self.process.stdout is not None
            for line in self.process.stdout:
                self.lines.put(line)
            self.lines.put('')
        threading.Thread(target=read_lines, daemon=True).start()
    def request(self, method: str, params: dict) -> dict:
        self.sequence += 1
        assert self.process.stdin is not None and self.process.stdout is not None
        self.process.stdin.write(json.dumps({'jsonrpc':'2.0','id':self.sequence,'method':method,'params':params})+'\n')
        self.process.stdin.flush()
        try:
            line = self.lines.get(timeout=30)
        except queue.Empty as error:
            raise RuntimeError('MCP response exceeded 30 seconds') from error
        if not line:
            raise RuntimeError('MCP process exited without a response')
        result = json.loads(line)
        if result.get('id') != self.sequence or 'error' in result:
            raise RuntimeError(f'MCP request failed: {result}')
        return result['result']
    def tool(self, name: str, args: dict, refuse: bool = False) -> dict:
        result = self.request('tools/call', {'name':name,'arguments':args})
        if bool(result.get('isError')) != refuse:
            raise RuntimeError(f'Unexpected {name} result: {result}')
        return result.get('structuredContent', result)
    def close(self):
        self.process.terminate()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()

def agent(binary: Path, method: str, params: dict) -> dict:
    request = {'jsonrpc':'2.0','id':1,'method':method,'params':params}
    completed = subprocess.run([str(binary),'agent'], input=json.dumps(request)+'\n', capture_output=True, text=True, encoding='utf-8', timeout=30, check=True)
    response = json.loads(completed.stdout)
    if 'error' in response:
        raise RuntimeError(f'Agent history refused: {response}')
    return response['result']

def run(binary: Path, fixture: Path, output: Path | None = None) -> dict:
    if output is not None:
        if output.exists() or output.is_symlink() or output.is_relative_to(fixture):
            raise ValueError('Output must be a new path outside the original fixture')
    original_bytes = snapshot(fixture)
    scenario = json.loads((ROOT/'examples/agent/editing_tutorial.json').read_text(encoding='utf-8'))
    version = subprocess.run([str(binary),'version'],capture_output=True,text=True,encoding='utf-8',timeout=10,check=True).stdout.strip()
    with tempfile.TemporaryDirectory(prefix='musubicad-tutorial-') as temporary:
        path = Path(temporary)/'ellipse.ocad.d'
        shutil.copytree(fixture,path)
        source = logical(path)
        mcp = MCP(binary)
        try:
            mcp.request('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'editing-tutorial','version':'1'}})
            tools = {t['name'] for t in mcp.request('tools/list',{})['tools']}
            required = {'inspect_sketches','plan_sketch_edit','apply_sketch_edit','list_features','plan_feature_edit','apply_feature_edit','regen_document'}
            if not required <= tools:
                raise RuntimeError('Installed release lacks tutorial commands; build current main or use the release named in the guide')
            sketches = mcp.tool('inspect_sketches',{'path':str(path)})
            edit = {'revision':sketches['revision'],'sketch_id':'sketch:ellipse','operations':scenario['sketch_operations']}
            before = snapshot(path)
            plan = mcp.tool('plan_sketch_edit',{'path':str(path),'edit':edit})
            assert snapshot(path)==before, 'Review wrote source'
            assert math.isclose(plan['evidence']['volume_m3'],math.pi*.04*.015*.006,abs_tol=1e-12,rel_tol=0)
            history = mcp.tool('apply_sketch_edit',{'path':str(path),'edit':edit})['history']
            tree = mcp.tool('list_features',{'path':str(path)})
            args = {'path':str(path),'revision':tree['revision'],'edit':scenario['feature_edit']}
            before = snapshot(path)
            mcp.tool('plan_feature_edit',args)
            assert snapshot(path)==before, 'Feature review wrote source'
            applied = mcp.tool('apply_feature_edit',{**args,'history':history['history']})
            history = applied['history']
            final = logical(path)
            for size, expected_size in zip(applied['evidence']['bounds_size_m'], [.08,.03,.01]):
                assert abs(size-expected_size)<3e-7, 'Native dimensions differ from analytic result'
            stable = snapshot(path)
            mcp.tool('apply_feature_edit',{**args,'history':history['history']},refuse=True)
            bad = {'revision':applied['tree']['revision'],'sketch_id':'sketch:ellipse','operations':[{'operation':'point','id':'ent:center','x':'10','y':'0 mm','create':False}]}
            mcp.tool('apply_sketch_edit',{'path':str(path),'edit':bad,'history':history['history']},refuse=True)
            assert snapshot(path)==stable, 'Refusal changed source/checksums'
            # MCP currently exposes editing; opaque Undo/Redo is on the Agent API.
            # Separate real Agent processes exercise that documented route.
            for _ in range(2):
                history = agent(binary,'opencad.history_undo_document',{'path':str(path),'history':history['history']})
            assert logical(path)==source, 'Undo lost source identity/expressions'
            for _ in range(2):
                history = agent(binary,'opencad.history_redo_document',{'path':str(path),'history':history['history']})
            assert logical(path)==final, 'Redo changed canonical source'
            result = mcp.tool('regen_document',{'path':str(path)})
            expected = math.pi*.04*.015*.010
            assert math.isclose(result['volume_m3'],expected,abs_tol=1e-12,rel_tol=0)
            entities = final['graph/sketches.json']['sketches'][0]['entities']
            ellipse = next(e for e in entities if e['id']=='ent:ellipse')
            assert [ellipse[k] for k in ['center','major_radius','minor_radius','axis_angle']]==['ent:center','40 mm','15 mm','0 deg']
            old_points = [e for e in source['graph/sketches.json']['sketches'][0]['entities'] if e['type']=='point']
            assert [e for e in entities if e['type']=='point']==old_points
            assert final['graph/parameters.json']==source['graph/parameters.json']
            assert final['manifest.ocad.json']==source['manifest.ocad.json']
            features = final['graph/features.json']
            assert features['feature_graph']['order']==source['graph/features.json']['feature_graph']['order']
            extrusion = next(n['definition'] for n in features['feature_nodes'] if n['id']=='feature:ellipse_body')
            assert extrusion['profile_ref']=='sketch:ellipse/profile:outer' and extrusion['length_expr']=='10 mm'
            assert snapshot(fixture)==original_bytes, 'Original fixture was mutated'
            if output is not None:
                shutil.copytree(path, output)
            return {'output':str(output) if output is not None else None,'passed':True,'version':version,'transport':'real MCP stdio + Agent history','actual_client_host':False,'volume_m3':result['volume_m3'],'expected_volume_mm3':expected*1e9,'checks':['read-only reviews','typed sketch and feature commands','stale/unit refusal preserves files','opaque undo/redo canonical identity','unchanged parameters/profile/document identity','fresh native analytic volume/dimensions','original fixture unchanged']}
        finally:
            mcp.close()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/musubicad')
    parser.add_argument('--fixture',type=Path,default=ROOT/'examples/ellipse_plate.ocad.d')
    parser.add_argument('--output',type=Path,help='Save a verified final copy to a new path; never overwrites')
    args=parser.parse_args()
    print(json.dumps(run(args.binary.resolve(),args.fixture.resolve(),args.output.resolve() if args.output is not None else None),indent=2))

if __name__=='__main__':
    try:
        main()
    except Exception as error:
        report_action_error(error, 'Packaged editing tutorial')
        raise
