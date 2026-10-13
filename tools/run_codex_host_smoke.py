"""Exercise real Codex app-server MCP dispatch without starting a model turn.

Python 3.10+ and Codex app-server experimental API required. Only a temporary
fixture copy is edited. Persistent Codex configuration is never written.
"""
from __future__ import annotations
import argparse
import json
import math
import queue
import shutil
import subprocess
import tempfile
import threading
import time
from pathlib import Path
from run_editing_tutorial import ROOT, logical, snapshot


class Host:
    def __init__(self, codex: str, binary: Path, cwd: Path):
        self.stderr = tempfile.TemporaryFile(mode='w+', encoding='utf-8')
        try:
            self.process = subprocess.Popen([
                codex, '-c', 'mcp_servers.musubicad_smoke.command='+json.dumps(str(binary)),
                '-c', 'mcp_servers.musubicad_smoke.args=["mcp"]',
                'app-server', '--stdio',
            ], cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=self.stderr, text=True, encoding='utf-8')
        except BaseException:
            self.stderr.close()
            raise
        self.lines: queue.Queue[str] = queue.Queue()
        self.sequence = 0
        def reader():
            assert self.process.stdout is not None
            for line in self.process.stdout:
                self.lines.put(line)
            self.lines.put('')
        threading.Thread(target=reader, daemon=True).start()

    def send(self, message: dict):
        assert self.process.stdin is not None
        self.process.stdin.write(json.dumps(message)+'\n')
        self.process.stdin.flush()

    def request(self, method: str, params: dict, timeout: float = 45) -> dict:
        self.sequence += 1
        self.send({'id':self.sequence, 'method':method, 'params':params})
        deadline = time.monotonic()+timeout
        while True:
            remaining = deadline-time.monotonic()
            if remaining <= 0:
                raise RuntimeError(f'Codex {method} response timed out')
            try:
                line = self.lines.get(timeout=remaining)
            except queue.Empty as error:
                raise RuntimeError(f'Codex {method} response timed out') from error
            if not line:
                raise RuntimeError('Owned Codex app-server exited without a response')
            response = json.loads(line)
            if response.get('id') != self.sequence:
                continue  # Ignore notifications and unrelated host traffic; never log it.
            if 'error' in response:
                raise RuntimeError(f'Codex {method} failed (host error details withheld)')
            return response['result']

    def tool(self, thread: str, name: str, arguments: dict, refuse: bool = False) -> dict:
        result = self.request('mcpServer/tool/call', {
            'threadId':thread, 'server':'musubicad_smoke', 'tool':name,
            'arguments':arguments,
        })
        require(bool(result.get('isError')) == refuse, f'Unexpected {name} refusal state')
        return result.get('structuredContent', result)

    def close(self):
        self.process.terminate()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=5)
        for pipe in (self.process.stdin, self.process.stdout):
            if pipe is not None:
                pipe.close()
        self.stderr.close()


def require(condition: bool, message: str):
    if not condition:
        raise RuntimeError(message)


def run(codex: str, binary: Path, fixture: Path) -> dict:
    host_version = subprocess.run([codex, '--version'], capture_output=True,
        text=True, encoding='utf-8', check=True, timeout=10).stdout.strip()
    version = subprocess.run([str(binary), 'version'], capture_output=True,
        text=True, encoding='utf-8', check=True, timeout=10).stdout.strip()
    original = snapshot(fixture)
    scenario = json.loads((ROOT/'examples/agent/editing_tutorial.json').read_text(encoding='utf-8'))
    with tempfile.TemporaryDirectory(prefix='musubicad-codex-smoke-') as temporary:
        cwd = Path(temporary)
        part = cwd/'ellipse.ocad.d'
        shutil.copytree(fixture, part)
        source = logical(part)
        host = Host(codex, binary, cwd)
        try:
            host.request('initialize', {'clientInfo':{'name':'musubicad_host_smoke','version':'1'},
                'capabilities':{'experimentalApi':True}})
            host.send({'method':'initialized'})
            thread = host.request('thread/start', {'cwd':str(cwd), 'ephemeral':True,
                'sandbox':'workspace-write', 'approvalPolicy':'on-request'})['thread']['id']
            servers = []
            cursor = None
            while True:
                params = {'threadId':thread, 'limit':100}
                if cursor is not None:
                    params['cursor'] = cursor
                page = host.request('mcpServerStatus/list', params)
                servers.extend(s for s in page['data'] if s['name']=='musubicad_smoke')
                next_cursor = page.get('nextCursor')
                if next_cursor is None:
                    break
                require(next_cursor != cursor, 'Host inventory cursor did not advance')
                cursor = next_cursor
            require(len(servers)==1, 'Owned MCP server missing or duplicated in real Codex inventory')
            tools = sorted(servers[0]['tools'])
            required = {'inspect_sketches','plan_sketch_edit','apply_sketch_edit','list_features',
                'plan_feature_edit','apply_feature_edit','regen_document'}
            require(required <= set(tools), 'Host did not discover required editing tools')
            def tool(name, args, refuse=False):
                return host.tool(thread, name, args, refuse)
            sketches = tool('inspect_sketches', {'path':str(part)})
            edit = {'revision':sketches['revision'],'sketch_id':'sketch:ellipse',
                'operations':scenario['sketch_operations']}
            before = snapshot(part)
            plan = tool('plan_sketch_edit', {'path':str(part),'edit':edit})
            require(snapshot(part)==before, 'Sketch review changed source')
            require(math.isclose(plan['evidence']['volume_m3'],math.pi*.04*.015*.006,
                abs_tol=1e-12, rel_tol=0), 'Sketch review volume differs from analytic value')
            history = tool('apply_sketch_edit', {'path':str(part),'edit':edit})['history']
            tree = tool('list_features', {'path':str(part)})
            args = {'path':str(part),'revision':tree['revision'],'edit':scenario['feature_edit']}
            before = snapshot(part)
            tool('plan_feature_edit', args)
            require(snapshot(part)==before, 'Feature review changed source')
            applied = tool('apply_feature_edit', {**args,'history':history['history']})
            sizes = applied['evidence']['bounds_size_m']
            require(len(sizes)==3 and all(abs(a-b)<3e-7 for a,b in zip(sizes,[.08,.03,.01])),
                'Native dimensions differ from 80 x 30 x 10 mm')
            stable = snapshot(part)
            tool('apply_feature_edit', {**args,'history':applied['history']['history']}, True)
            require(snapshot(part)==stable, 'Stale refusal changed source/checksums')
            bad = {'revision':applied['tree']['revision'],'sketch_id':'sketch:ellipse',
                'operations':[{'operation':'point','id':'ent:center','x':'10','y':'0 mm','create':False}]}
            tool('apply_sketch_edit', {'path':str(part),'edit':bad,
                'history':applied['history']['history']}, True)
            require(snapshot(part)==stable, 'Unit refusal changed source/checksums')
            regen = tool('regen_document', {'path':str(part)})
            expected = math.pi*.04*.015*.01
            require(math.isclose(regen['volume_m3'], expected, abs_tol=1e-12, rel_tol=0),
                'Fresh native volume differs from analytic value')
            final = logical(part)
            for key in ('manifest.ocad.json','graph/parameters.json'):
                require(final[key]==source[key], 'Source identity/parameters changed')
            old_entities = source['graph/sketches.json']['sketches'][0]['entities']
            entities = final['graph/sketches.json']['sketches'][0]['entities']
            require([e for e in entities if e['type']=='point']==
                [e for e in old_entities if e['type']=='point'], 'Authored points changed')
            ellipse = next(e for e in entities if e['id']=='ent:ellipse')
            require([ellipse[k] for k in ('center','major_radius','minor_radius','axis_angle')]==
                ['ent:center','40 mm','15 mm','0 deg'], 'Authored ellipse expressions lost')
            features = final['graph/features.json']
            require(features['feature_graph']['order']==source['graph/features.json']['feature_graph']['order'],
                'Feature order changed')
            extrusion = next(n['definition'] for n in features['feature_nodes'] if n['id']=='feature:ellipse_body')
            require(extrusion['profile_ref']=='sketch:ellipse/profile:outer' and extrusion['length_expr']=='10 mm',
                'Source profile reference or feature expression changed')
            require(snapshot(fixture)==original, 'Original fixture changed')
            return {'passed':True,'host_version':host_version,'musubicad_version':version,
                'transport':'actual Codex app-server MCP dispatch','ephemeral_thread':True,
                'model_turns':0,'persistent_plugin_installation_tested':False,
                'tools':tools,'volume_m3':regen['volume_m3'],'bounds_size_m':sizes,
                'checks':['live host discovery','read-only reviews','typed sketch/feature apply',
                    'stale/unit refusal preserves bytes','source identities/expressions retained',
                    'fresh native analytic volume/dimensions','original fixture unchanged']}
        finally:
            host.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--codex', default='codex')
    parser.add_argument('--binary', type=Path, default=ROOT/'target/debug/musubicad')
    parser.add_argument('--fixture', type=Path, default=ROOT/'examples/ellipse_plate.ocad.d')
    args = parser.parse_args()
    print(json.dumps(run(args.codex,args.binary.resolve(),args.fixture.resolve()), indent=2))


if __name__=='__main__':
    main()
