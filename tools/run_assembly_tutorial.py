"""Reproduce parent-context assemblies through real MCP/Agent and native exports.

Python 3.10+, no paid model or simulated client-host evidence. All edits use a
cleaned disposable copy; original fixtures and previously published files remain.
"""
from __future__ import annotations
import argparse
import copy
import json
import math
from pathlib import Path
import struct
import subprocess
import tempfile
import shutil
from run_editing_tutorial import MCP, agent, logical, snapshot

ROOT = Path(__file__).resolve().parents[1]
SCENARIO = ROOT / 'examples/agent/assembly_tutorial.json'
I = [[1,0,0],[0,1,0],[0,0,1]]
X = [[1,0,0],[0,0,-1],[0,1,0]]
Z = [[0,-1,0],[1,0,0],[0,0,1]]
HALF_TURN = [[-1,0,0],[0,-1,0],[0,0,1]]
VOLUME_M3 = 30e-6

def close(actual: float, expected: float, tolerance: float, label: str) -> None:
    if not isinstance(actual,(int,float)) or not math.isfinite(actual) or abs(actual-expected)>tolerance:
        raise AssertionError(f'{label}: {actual} differs from {expected} (SI)')

def vector(actual, expected, tolerance: float, label: str) -> None:
    if len(actual)!=len(expected): raise AssertionError(f'{label}: wrong dimensions')
    for a,b in zip(actual,expected): close(a,b,tolerance,label)

def expected_leaves(angular: bool, edited: bool) -> list[dict]:
    # Independently specified box frames; never call production pose helpers.
    return [
        {'instance_path':['instance:left','instance:anchor'],'translation_m':[.1,0,-.06] if angular else [.1,.06,0],'rotation':Z if angular else X,'bounds_size_m':[.02,.01,.03] if angular else [.01,.03,.02]},
        {'instance_path':['instance:left','instance:link'],'translation_m':[.1,0,0] if angular or edited else [.1,-.025,0],'rotation':(Z if edited else HALF_TURN) if angular else X,'bounds_size_m':([.02,.01,.03] if edited else [.01,.02,.03]) if angular else [.01,.03,.02]},
        {'instance_path':['instance:right','instance:anchor'],'translation_m':[.1,.2,-.06],'rotation':Z,'bounds_size_m':[.02,.01,.03]},
        {'instance_path':['instance:right','instance:link'],'translation_m':[.1,.2,0],'rotation':Z,'bounds_size_m':[.02,.01,.03]},
        {'instance_path':['instance:target'],'translation_m':[.2,0,0],'rotation':I if angular else X,'bounds_size_m':[.01,.02,.03] if angular else [.01,.03,.02]},
    ]

def check_regeneration(report: dict, angular: bool, edited: bool) -> None:
    if not isinstance(report.get('kernel'),str) or 'OCCT' not in report['kernel']:
        raise AssertionError('Native OCCT evidence required')
    if report.get('context_dof')!=0: raise AssertionError('Bound context DOF missing or nonzero')
    close(report['volume_m3'],VOLUME_M3,1e-11,'native total volume')
    leaves=report['assembly_leaves']; expected=expected_leaves(angular,edited)
    if [l['instance_path'] for l in leaves] != [l['instance_path'] for l in expected]:
        raise AssertionError('Physical scoped leaf inventory/order differs')
    for got,want in zip(leaves,expected):
        if got['source_doc']!='doc:art_box' or not got.get('source_revision'):
            raise AssertionError('Physical leaf source ownership missing')
        vector(got['placement']['translation_m'],want['translation_m'],1e-9,'leaf translation')
        vector([v for row in got['placement']['rotation'] for v in row],[v for row in want['rotation'] for v in row],1e-9,'leaf rotation')
        vector(got['bounds_size_m'],want['bounds_size_m'],3e-7,'native leaf dimensions')
        close(got['volume_m3'],6e-6,1e-11,'native leaf volume')

def mesh_properties(data: bytes) -> dict:
    """Independent closed binary-STL tetrahedral integration, coordinates in m."""
    if len(data)<84: raise AssertionError('Truncated STL header')
    count=struct.unpack_from('<I',data,80)[0]
    if not 1<=count<=5000 or len(data)!=84+50*count: raise AssertionError('Invalid STL triangle inventory')
    volume=0.; moment=[0.,0.,0.]; minimum=[math.inf]*3; maximum=[-math.inf]*3
    edges={}; neighbors={}
    for index in range(count):
        values=struct.unpack_from('<12fH',data,84+50*index)
        a,b,c=[values[start:start+3] for start in [3,6,9]]
        if any(not math.isfinite(v) for p in [a,b,c] for v in p): raise AssertionError('Nonfinite mesh coordinates')
        ids=[tuple(round(v/1e-7) for v in p) for p in [a,b,c]]
        if len(set(ids))!=3: raise AssertionError('Degenerate mesh triangle')
        for p in [a,b,c]:
            for j in range(3): minimum[j]=min(minimum[j],p[j]);maximum[j]=max(maximum[j],p[j])
        for u,v in zip(ids,ids[1:]+ids[:1]):
            key=tuple(sorted((u,v)));used,direction=edges.get(key,(0,0));edges[key]=(used+1,direction+(1 if u<v else -1))
            neighbors.setdefault(u,set()).add(v);neighbors.setdefault(v,set()).add(u)
        cross=[b[1]*c[2]-b[2]*c[1],b[2]*c[0]-b[0]*c[2],b[0]*c[1]-b[1]*c[0]]
        tetra=sum(a[j]*cross[j] for j in range(3))/6
        volume+=tetra
        for j in range(3): moment[j]+=tetra*(a[j]+b[j]+c[j])/4
    if any(v!=(2,0) for v in edges.values()): raise AssertionError('Mesh is not closed with consistent winding')
    if not math.isfinite(volume) or volume<=0: raise AssertionError('Mesh has no positive oriented volume')
    remaining=set(neighbors);components=0
    while remaining:
        components+=1;todo=[remaining.pop()]
        while todo:
            for v in neighbors[todo.pop()] & remaining: remaining.remove(v);todo.append(v)
    return {'volume_m3':volume,'center_m':[v/volume for v in moment],'minimum_m':minimum,'maximum_m':maximum,'components':components,'triangles':count}

def check_mesh(properties: dict, angular: bool, edited: bool) -> None:
    close(properties['volume_m3'],VOLUME_M3,5e-11,'exported mesh volume')
    center=([.113,.086,-.009] if edited else [.114,.083,-.009]) if angular else ([.119,.085,0] if edited else [.119,.080,0])
    minimum=[.08,0 if edited else -.02,-.06] if angular else [.08,-.03 if edited else -.055,-.06]
    vector(properties['center_m'],center,3e-7,'exported mesh centre')
    vector(properties['minimum_m'],minimum,3e-7,'exported mesh minimum')
    vector(properties['maximum_m'],[.21,.21,.03],3e-7,'exported mesh maximum')
    if properties['components']!=5: raise AssertionError('Export omitted or merged physical members')

def export_check(mcp: MCP, path: Path, directory: Path, angular: bool, edited: bool) -> dict:
    output=directory/('edited.stl' if edited else 'original.stl')
    mcp.tool('export_document',{'path':str(path),'output':str(output)})
    result=mesh_properties(output.read_bytes());check_mesh(result,angular,edited);return result

def run(binary: Path, fixtures: Path, scenario: Path) -> dict:
    cases=json.loads(scenario.read_text(encoding='utf-8'))['cases'];results=[]
    expected_cases={'parent_context_slider.ocad.d':('signed_distance','0 mm'),'parent_context_hinge.ocad.d':('signed_angle','90 deg')}
    if len(cases)!=2 or {c['fixture']:(c['relation'],c['target']) for c in cases}!=expected_cases:
        raise ValueError('Scenario must execute both supported mechanisms and target edits')
    version=subprocess.run([str(binary),'version'],capture_output=True,text=True,encoding='utf-8',check=True,timeout=10).stdout.strip()
    for case in cases:
        angular=case['relation']=='signed_angle';fixture=fixtures/case['fixture'];original=snapshot(fixture)
        with tempfile.TemporaryDirectory(prefix='musubicad-assembly-tutorial-') as temporary:
            work=Path(temporary);path=work/'assembly.ocad.d';shutil.copytree(fixture,path)
            before_source=logical(path);children=snapshot(path/'parts');mcp=MCP(binary)
            try:
                mcp.request('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'assembly-tutorial','version':'1'}})
                tools={t['name'] for t in mcp.request('tools/list',{})['tools']}
                required={'inspect_articulation_source','patch_dry_run','review_patch','patch_apply','regen_document','export_document','bom_document'}
                if not required<=tools: raise AssertionError('Binary lacks parent-context tutorial API; use current main or its future release')
                source=mcp.tool('inspect_articulation_source',{'path':str(path/'parts/mechanism.ocad.d')})
                if source['independent_joints']!=['joint:hinge'] or not source['can_articulate_child']: raise AssertionError('Child driver admission differs')
                if [c['id'] for c in source['connectors']]!=['connector:link_tip']: raise AssertionError('Exact child connector not discovered')
                check_regeneration(mcp.tool('regen_document',{'path':str(path)}),angular,False)
                export_check(mcp,path,work,angular,False)
                parent=mcp.tool('inspect_articulation_source',{'path':str(path)})
                assembly=before_source['graph/assemblies.json']['assembly'];left=next(i for i in assembly['instances'] if i['id']=='instance:left')
                config=copy.deepcopy(left['articulation'])
                if config['source_doc']!=source['source_doc'] or config['source_revision']!=source['source_revision']: raise AssertionError('Child source revision/owner differs')
                relation=config['context_constraints'][0]['relation'];relation['angle' if angular else 'distance']=case['target'];
                patch={'preconditions':[parent['revision_precondition']],'operations':[{'type':'set_instance_articulation','instance_id':'instance:left','articulation':config}]}
                stable=snapshot(path)
                dry=mcp.tool('patch_dry_run',{'path':str(path),'patch':patch})
                if not dry['verification']['passed']: raise AssertionError('Parent constraint dry-run failed')
                review=mcp.tool('review_patch',{'path':str(path),'patch':patch,'output_dir':str(work/'review')})
                if review['review']['geometry']['after_interference_count']!=0 or review['review']['geometry'].get('before_regen_error'):
                    raise AssertionError('Parent review did not verify clear physical geometry')
                if not (work/'review/review.html').is_file() or not (work/'review/review.json').is_file():
                    raise AssertionError('Review artifacts missing')
                if snapshot(path)!=stable: raise AssertionError('Review mutated source')
                applied=mcp.tool('patch_apply',{'path':str(path),'patch':patch,'verify':False})
                if snapshot(path/'parts')!=children: raise AssertionError('Parent edit mutated a child')
                final=logical(path);expected_source=copy.deepcopy(before_source)
                next(i for i in expected_source['graph/assemblies.json']['assembly']['instances'] if i['id']=='instance:left')['articulation']=config
                if final!=expected_source: raise AssertionError('Parent-only edit changed unrelated source')
                report=mcp.tool('regen_document',{'path':str(path)});check_regeneration(report,angular,True)
                mesh=export_check(mcp,path,work,angular,True)
                bom=mcp.tool('bom_document',{'path':str(path)})
                if bom['leaf_instance_count']!=5: raise AssertionError('BOM quantities differ')
                stable=snapshot(path)
                mcp.tool('patch_apply',{'path':str(path),'patch':patch,'verify':False},refuse=True) # old parent revision
                for wrong in ['unitless','range','owner','missing_connector']:
                    bad=copy.deepcopy(config);constraint=bad['context_constraints'][0]
                    if wrong=='owner': constraint['target']['source_doc']='doc:wrong_owner'
                    elif wrong=='missing_connector': constraint['moving']['connector_id']='connector:missing'
                    else: constraint['relation']['angle' if angular else 'distance']='10' if wrong=='unitless' else ('999 deg' if angular else '999 mm')
                    mcp.tool('patch_apply',{'path':str(path),'patch':{'operations':[{'type':'set_instance_articulation','instance_id':'instance:left','articulation':bad}]},'verify':False},refuse=True)
                    if snapshot(path)!=stable: raise AssertionError('Refused constraint mutated files')
                for extension in ['urdf','html']:
                    protected=work/f'protected.{extension}';protected.write_bytes(b'KEEP')
                    mcp.tool('export_document',{'path':str(path),'output':str(protected)},refuse=True)
                    if protected.read_bytes()!=b'KEEP': raise AssertionError('Unsupported export replaced output')
                history=agent(binary,'opencad.history_undo_document',{'path':str(path),'history':applied['history']})
                if logical(path)!=before_source or snapshot(path/'parts')!=children: raise AssertionError('Undo lost source identity')
                history=agent(binary,'opencad.history_redo_document',{'path':str(path),'history':history['history']})
                if logical(path)!=final or snapshot(path/'parts')!=children: raise AssertionError('Redo lost source identity')
                check_regeneration(mcp.tool('regen_document',{'path':str(path)}),angular,True)
                if snapshot(fixture)!=original: raise AssertionError('Original fixture changed')
                results.append({'fixture':case['fixture'],'passed':True,'tools_discovered':len(tools),'context_dof':report['context_dof'],'volume_m3':report['volume_m3'],'mesh':mesh})
            finally: mcp.close()
    return {'passed':True,'version':version,'transport':'real MCP stdio + Agent history','actual_client_host':False,'model_turns':0,'cases':results}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/musubicad')
    parser.add_argument('--fixtures',type=Path,default=ROOT/'examples')
    parser.add_argument('--scenario',type=Path,default=SCENARIO)
    args=parser.parse_args();print(json.dumps(run(args.binary.resolve(),args.fixtures.resolve(),args.scenario.resolve()),indent=2))

if __name__=='__main__': main()
