"""Independent analytic/mutation checks for the packaged assembly tutorial.

These synthetic meshes test grading failures; only the real tutorial proves MCP
or native execution. No paid model or client-host integration is inferred.
"""
from __future__ import annotations
import struct
import unittest
import json
import tempfile
import os
import subprocess
import sys
import io
from contextlib import redirect_stdout
from unittest.mock import patch
from pathlib import Path
from run_assembly_tutorial import check_mesh, check_regeneration, mesh_properties, run
from run_editing_tutorial import report_action_error

FACES=[(0,2,1),(0,3,2),(4,5,6),(4,6,7),(0,1,5),(0,5,4),(1,2,6),(1,6,5),(2,3,7),(2,7,6),(3,0,4),(3,4,7)]

def box(origin, size):
    x,y,z=origin;w,h,d=size
    points=[(x,y,z),(x+w,y,z),(x+w,y+h,z),(x,y+h,z),(x,y,z+d),(x+w,y,z+d),(x+w,y+h,z+d),(x,y+h,z+d)]
    return [[points[v] for v in face] for face in FACES]

def encode(triangles):
    return b'analytic-metre-boxes'.ljust(80,b'\0')+struct.pack('<I',len(triangles))+b''.join(struct.pack('<12fH',0.,0.,0.,*(v for p in triangle for v in p),0) for triangle in triangles)

def slider(shift=0.):
    # Direct world-space solids, independently derived from authored box frames.
    return sum([box(origin,size) for origin,size in [
        ((.1,.03,0),(.01,.03,.02)),
        ((.1+shift,-.03,0),(.01,.03,.02)),
        ((.08,.2,-.06),(.02,.01,.03)),
        ((.08,.2,0),(.02,.01,.03)),
        ((.2,-.03,0),(.01,.03,.02)),
    ]],[])

class GraderTests(unittest.TestCase):
    def test_action_annotation_escapes_data_and_preserves_real_child_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            missing=Path(directory)/'missing-native-binary'
            result=subprocess.run([sys.executable,str(Path(__file__).with_name('run_assembly_tutorial.py')),'--binary',str(missing)],capture_output=True,text=True,env={**os.environ,'GITHUB_ACTIONS':'true'})
            self.assertNotEqual(result.returncode,0)
            self.assertIn('::error title=Packaged assembly tutorial::',result.stdout)
            self.assertIn('FileNotFoundError',result.stdout)
            self.assertIn('FileNotFoundError',result.stderr)
        with patch.dict(os.environ,{'GITHUB_ACTIONS':'true'}),redirect_stdout(io.StringIO()) as output:
            report_action_error(ValueError('percent%\n::warning::injected\rnext'),'Packaged assembly tutorial')
        self.assertEqual(output.getvalue().count('\n'),1)
        self.assertIn('percent%25%0A::warning::injected%0Dnext',output.getvalue())
    def test_local_failure_does_not_emit_actions_command(self):
        with patch.dict(os.environ,{'GITHUB_ACTIONS':'false'}),redirect_stdout(io.StringIO()) as output:
            report_action_error(ValueError('local failure'),'Packaged assembly tutorial')
        self.assertEqual(output.getvalue(),'')
    def test_empty_scenario_cannot_report_success_without_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);scenario=root/'empty.json';scenario.write_text(json.dumps({'cases':[]}))
            with self.assertRaisesRegex(ValueError,'both supported mechanisms'):
                run(root/'binary-must-not-execute',root,scenario)
    def test_single_closed_box_volume_centroid_and_dimensions(self):
        result=mesh_properties(encode(box((.1,.2,.3),(.01,.02,.03))))
        self.assertAlmostEqual(result['volume_m3'],6e-6,delta=1e-11)
        for got,want in zip(result['center_m'],[.105,.21,.315]):self.assertAlmostEqual(got,want,delta=3e-7)
        self.assertEqual(result['components'],1)
    def test_internal_wrong_pose_rejected_even_when_volume_and_outer_bounds_match(self):
        expected=mesh_properties(encode(slider()));check_mesh(expected,False,True)
        wrong=mesh_properties(encode(slider(.025)))
        self.assertAlmostEqual(wrong['volume_m3'],expected['volume_m3'],delta=1e-11)
        self.assertEqual(wrong['minimum_m'],expected['minimum_m']);self.assertEqual(wrong['maximum_m'],expected['maximum_m'])
        with self.assertRaisesRegex(AssertionError,'centre'):check_mesh(wrong,False,True)
    def test_missing_open_reversed_and_truncated_geometry_refuse(self):
        missing=mesh_properties(encode(slider()[:-12]))
        with self.assertRaises(AssertionError):check_mesh(missing,False,True)
        triangles=box((0.,0.,0.),(.01,.02,.03))
        for data in [encode(triangles[:-1]),encode([list(reversed(t)) for t in triangles]),encode(triangles)[:-1]]:
            with self.assertRaises(AssertionError):mesh_properties(data)
    def test_non_native_missing_dof_and_nonfinite_volume_refuse(self):
        for report in [{'kernel':'Mock','context_dof':0,'volume_m3':30e-6},{'kernel':'OCCT','volume_m3':30e-6},{'kernel':'OCCT','context_dof':1,'volume_m3':30e-6},{'kernel':'OCCT','context_dof':0,'volume_m3':float('nan')}]:
            with self.assertRaises(AssertionError):check_regeneration(report,False,True)

if __name__=='__main__':unittest.main()
