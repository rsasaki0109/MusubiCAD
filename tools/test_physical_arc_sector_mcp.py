"""ADR-103 real MCP/Agent arc-sector source, history and physical evidence."""
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
from arc_sector_eval import graph_source, placed_problems, source_problems
from sketch_creation_eval import triangles, volume
from test_cylindrical_draft_mcp import Client, canonical_digest, digest

ROOT = Path(__file__).resolve().parents[1]
VOLUME = 150 * math.pi * 1e-9


def agent_result(agent, method, params):
    response = agent.call('opencad.' + method, params)
    assert 'error' not in response, response
    return response['result']


def wrong_circle(mcp, root, baseline_doc):
    """A genuine native r=5 circle has the same exact mass, but wrong shape."""
    doc = root / 'wrong_circle.ocad.d'
    shutil.copytree(baseline_doc, doc)
    state = mcp.tool('inspect_sketch_creation', {'path': str(doc)})
    create = {'document_id': state['document_id'], 'revision': state['revision'],
              'sketch_id': state['suggested_sketch_id'],
              'feature_id': state['suggested_feature_id'], 'name': 'Wrong circle',
              'datum_plane': 'feature:datum'}
    mcp.tool('apply_sketch_create', {'path': str(doc), 'create': create})
    state = mcp.tool('inspect_sketches', {'path': str(doc)})
    edit = {'document_id': state['document_id'], 'revision': state['revision'],
            'sketch_id': create['sketch_id'], 'operations': [
                {'operation': 'point', 'id': 'ent:decoy_center', 'create': True,
                 'x': '2 mm', 'y': '-3 mm'},
                {'operation': 'circle', 'id': 'ent:decoy_circle', 'create': True,
                 'center': 'ent:decoy_center', 'radius': '5 mm', 'construction': False}]}
    mcp.tool('apply_sketch_edit', {'path': str(doc), 'edit': edit})
    tree = mcp.tool('list_features', {'path': str(doc)})
    owner = next(row for row in tree['rows'] if row['id'] == create['feature_id'])
    body = {'operation': 'create_extrude', 'id': 'feature:decoy', 'name': 'Wrong circle',
            'sketch_feature': owner['id'], 'profile_ref': owner['closed_profiles'][0],
            'length_expr': '6 mm'}
    response = mcp.tool('apply_feature_edit', {'path': str(doc), 'revision': tree['revision'], 'edit': body})
    assert abs(response['evidence']['volume_m3'] - VOLUME) < 1e-12
    stl = root / 'wrong_circle.stl'
    mcp.tool('export_document', {'path': str(doc), 'output': str(stl)})
    assert placed_problems(stl.read_bytes(), 'seeded_XY')


def exercise(binary, root, case):
    mcp = Client(binary, 'mcp')
    agent = Client(binary, 'agent')
    doc, other = root / 'part.ocad.d', root / 'other.ocad.d'
    try:
        mcp.call('initialize', {'protocolVersion': '2025-06-18', 'capabilities': {},
                              'clientInfo': {'name': 'physical-arc-sector', 'version': '1'}})
        datum = case in ['tilted', 'seeded_XY']
        if case == 'seeded_XY':
            shutil.copytree(ROOT / 'examples/datum_plate.ocad.d', doc)
        else:
            mcp.tool('new_document', {'path': str(doc), 'kind': 'part',
                     'document_id': f'doc:arc_sector_{case}', 'name': 'Physical arc sector'})
            if datum:
                node = {'id': 'feature:datum', 'name': 'Tilted datum', 'definition': {
                    'type': 'datum_plane', 'base': {'type': 'custom', 'origin': [.01, -.02, .03],
                    'normal': [0, .6, .8], 'x_axis': [1, 0, 0]}, 'offset': {'value_si': .005}}}
                mcp.tool('patch_apply', {'path': str(doc), 'patch': {'operations': [
                    {'type': 'add_feature', 'node': node, 'position': {'at': 'end'}}]}, 'verify': True})
        baseline, initial = graph_source(doc), canonical_digest(doc)
        shutil.copytree(doc, other)
        baseline_doc = root / 'baseline.ocad.d'
        shutil.copytree(doc, baseline_doc)
        state = mcp.tool('inspect_sketch_creation', {'path': str(doc)})
        assert state == agent_result(agent, 'inspect_sketch_creation', {'path': str(other)})
        create = {'document_id': state['document_id'], 'revision': state['revision'],
                  'sketch_id': state['suggested_sketch_id'], 'feature_id': state['suggested_feature_id'],
                  'name': 'Physical arc sector', **({'datum_plane': 'feature:datum'} if datum else {'plane': case})}
        response = mcp.tool('apply_sketch_create', {'path': str(doc), 'create': create})
        assert response == agent_result(agent, 'apply_sketch_create', {'path': str(other), 'create': create})
        history = response['history']['history']
        other_history = copy.deepcopy(history)
        empty = canonical_digest(doc)
        state = mcp.tool('inspect_sketches', {'path': str(doc)})
        operations = json.loads((ROOT / 'examples/agent/physical_arc_sector.json').read_text())['operations']
        for operation in operations:
            operation['sketch_id'] = create['sketch_id']
        patch = {'operations': operations, 'preconditions': [{'type': 'revision_equals',
                 'algorithm': 'sha256', 'version': 'musubicad.design-state.v2', 'digest': state['revision']}]}
        before = digest(doc)
        preview = mcp.tool('patch_dry_run', {'path': str(doc), 'patch': patch, 'verify': True})
        assert preview == agent_result(agent, 'patch_dry_run_document', {'path': str(other), 'patch': patch, 'verify': True})
        assert preview['verification']['passed'] is True and digest(doc) == before
        bad_patches = []
        bad = copy.deepcopy(patch); bad['operations'][3]['entity']['radius'] = '10 deg'; bad_patches.append(bad)
        bad = copy.deepcopy(patch); bad['operations'][3]['entity']['end_angle'] = '90 mm'; bad_patches.append(bad)
        bad = copy.deepcopy(patch); bad['operations'][3]['entity']['center'] = 'ent:missing'; bad_patches.append(bad)
        bad = copy.deepcopy(patch); bad['operations'][-1]['entity']['id'] = 'ent:sector_start_ray'; bad_patches.append(bad)
        bad = copy.deepcopy(patch); bad['preconditions'][0]['digest'] = '0' * 64; bad_patches.append(bad)
        history_before = copy.deepcopy(history)
        for bad in bad_patches:
            mcp.tool('patch_apply', {'path': str(doc), 'patch': bad, 'history': history, 'verify': True}, error=True)
            failed = agent.call('opencad.patch_apply_document', {'path': str(other), 'patch': bad, 'history': other_history, 'verify': True})
            assert 'error' in failed, failed
            assert digest(doc) == before and canonical_digest(doc) == canonical_digest(other)
            assert history == history_before
        response = mcp.tool('patch_apply', {'path': str(doc), 'patch': patch, 'history': history, 'verify': True})
        parity = agent_result(agent, 'patch_apply_document', {'path': str(other), 'patch': patch, 'history': other_history, 'verify': True})
        assert response['patched'] == str(doc) and parity['patched'] == str(other)
        assert {k: v for k, v in response.items() if k != 'patched'} == {k: v for k, v in parity.items() if k != 'patched'}
        assert response['verification']['passed'] is True and canonical_digest(doc) == canonical_digest(other)
        history, other_history = response['history'], parity['history']
        sector = canonical_digest(doc)
        for operation, wanted in [('undo', empty), ('redo', sector)]:
            history = agent_result(agent, f'history_{operation}_document', {'path': str(doc), 'history': history})['history']
            other_history = agent_result(agent, f'history_{operation}_document', {'path': str(other), 'history': other_history})['history']
            assert canonical_digest(doc) == wanted == canonical_digest(other)
        stable = digest(doc)
        mcp.tool('patch_apply', {'path': str(doc), 'patch': patch, 'history': history, 'verify': True}, error=True)
        assert digest(doc) == stable
        tree = mcp.tool('list_features', {'path': str(doc)})
        owner = next(row for row in tree['rows'] if row['id'] == create['feature_id'])
        body = {'operation': 'create_extrude', 'id': 'feature:sector_body', 'name': 'Physical sector',
                'sketch_feature': owner['id'], 'profile_ref': owner['closed_profiles'][0], 'length_expr': '6 mm'}
        response = mcp.tool('apply_feature_edit', {'path': str(doc), 'revision': tree['revision'], 'edit': body, 'history': history})
        parity = agent_result(agent, 'apply_feature_edit', {'path': str(other), 'revision': tree['revision'], 'edit': body, 'history': other_history})
        assert response == parity and canonical_digest(doc) == canonical_digest(other)
        history = response['history']['history']
        assert abs(response['evidence']['volume_m3'] - VOLUME) < 1e-12
        desired = {'sketch_id': create['sketch_id'], 'feature_id': owner['id'], 'body_feature': 'feature:sector_body',
                   'workplane': {'type': 'datum_plane', 'feature_id': 'feature:datum'} if datum else {'type': 'global', 'plane': case}, 'case': case}
        assert source_problems(doc, desired, baseline) == [], source_problems(doc, desired, baseline)
        wrong_source = root / 'wrong_source.ocad.d'
        shutil.copytree(doc, wrong_source)
        source_path = wrong_source / 'graph/sketches.json'
        authored = json.loads(source_path.read_text())
        for mutation in ['wrong_branch', 'wrong_radius', 'construction', 'missing_ray']:
            candidate = copy.deepcopy(authored)
            sketch = next(s for s in candidate['sketches'] if s['id'] == create['sketch_id'])
            arc = next(e for e in sketch['entities'] if e['type'] == 'arc')
            if mutation == 'wrong_branch': arc['end_angle'] = '270 deg'
            elif mutation == 'wrong_radius': arc['radius'] = '5 mm'
            elif mutation == 'construction': arc['construction'] = True
            else: sketch['entities'] = [e for e in sketch['entities'] if e['id'] != 'ent:sector_end_ray']
            source_path.write_text(json.dumps(candidate))
            assert source_problems(wrong_source, desired, baseline), mutation
        source_path.write_text(json.dumps(authored))
        assert source_problems(wrong_source, desired, baseline) == []
        stl = root / 'actual.stl'
        mcp.tool('export_document', {'path': str(doc), 'output': str(stl)})
        data = stl.read_bytes()
        assert placed_problems(data, case) == [], placed_problems(data, case)
        wrong = bytearray(data)
        for i, face in enumerate(triangles(data)):
            for j, (x, y, z) in enumerate(face):
                struct.pack_into('<3f', wrong, 84 + 50*i + 12 + 12*j, x + .005, y, z)
        assert abs(volume(triangles(wrong)) - volume(triangles(data))) < 1e-12 and placed_problems(wrong, case)
        final = canonical_digest(doc)
        for operation, wanted in [('undo', initial), ('redo', final)]:
            for _ in range(3):
                history = agent_result(agent, f'history_{operation}_document', {'path': str(doc), 'history': history})['history']
            assert canonical_digest(doc) == wanted
        if case == 'seeded_XY':
            task = next(t for t in json.loads(mcp_eval.TASKS.read_text())['tasks'] if t['id'] == 'create_physical_arc_sector')
            regen = subprocess.run([str(binary), 'regen', str(doc)], capture_output=True, text=True, check=True, timeout=60)
            assert mcp_eval.check(task, doc, regen.stdout, binary, root) == (True, [])
            wrong_circle(mcp, root, baseline_doc)
        return {'case': case, 'mcp_requests': mcp.count, 'agent_requests': agent.count, 'triangles': len(triangles(data))}
    finally:
        mcp.close()
        agent.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--musubicad', type=Path, required=True)
    parser.add_argument('--out', type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='musubicad-physical-arc-sector-') as folder:
        root, cases = Path(folder), []
        for case in ['XY', 'YZ', 'XZ', 'tilted', 'seeded_XY']:
            path = root / case
            path.mkdir()
            cases.append(exercise(args.musubicad.resolve(), path, case))
        report = {'cases': cases, 'model_turns': 0, 'mcp_agent_parity': True,
                  'verified_readonly_atomic_refusal': True, 'single_batch_undo_redo': True,
                  'independent_source_and_physical_eval': True, 'equal_volume_wrong_placement_refused': True,
                  'equal_analytic_volume_wrong_circle_refused': True, 'invalid_source_decoys_refused': True}
        if args.out:
            args.out.write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report))


if __name__ == '__main__':
    main()
