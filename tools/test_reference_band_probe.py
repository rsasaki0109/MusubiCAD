"""Synthetic report-contract fixtures only; actual native evidence runs separately."""
import copy
import math
import unittest
from run_reference_band_probe import grade


def fixture():
    reference = {"format_version": 1, "requested_position_error_m": 1e-4, "requested_normal_error": 0.8, "max_patches": 4096, "cases": []}
    native = {"format_version": 1, "occt_version": "8.0.0", "cases": []}
    for name in ("constant", "variable"):
        ref = {"name": name, "divisions": 1, "position_error_upper_m": 1e-4, "normal_error_upper": 0.5, "cells": []}
        actual = {"brep_valid": True, "shells": 1, "solids": 0, "faces": 4, "vertices": 8, "edges": 12,
                  "boundary_edges": 8, "euler": 0, "boundary_cycles": [4, 4], "boundary_degree_two": True,
                  "edge_incidence_valid": True, "face_connected": True, "sewing_tolerance_m": 1e-9, "max_boundary_support_error_m": 0.,
                  "surface_cells": [], "boundary_loops_m": []}
        for chart in range(4):
            poles = [[[i*0.001/3, j*0.001/3, 0.02] for j in range(4)] for i in range(4)]
            ref["cells"].append({"chart": chart, "chart_domain": [0., 1., 0., 1.], "control_points_m": poles, "position_error_upper_m": 1e-4, "normal_error_upper": 0.5})
            row = {"cell": chart, "poles_m": copy.deepcopy(poles), "samples": []}
            for i in range(3):
                for j in range(3):
                    row["samples"].append({"u": i/2, "v": j/2, "point_m": [0.001*i/2, 0.001*j/2, 0.02]})
            actual["surface_cells"].append(row)
        for side in (True, False):
            loop = []
            for i in range(4):
                theta = i*math.pi/2
                r = 0.0005 if name == "constant" else 0.001+0.0005*math.cos(theta)
                rho, z = (0.01, 0.02-r) if side else (0.01-r, 0.02)
                loop.append([rho*math.cos(theta), rho*math.sin(theta), z])
            actual["boundary_loops_m"].append(loop)
        reference["cases"].append(ref)
        native["cases"].append(actual)
    return reference, native


class Contract(unittest.TestCase):
    def test_complete_contract_passes_without_solid_admission(self):
        report = grade(*fixture())
        self.assertTrue(report["diagnostic_passed"])
        self.assertFalse(report["candidate_admitted"])
        self.assertFalse(report["solid_created"])

    def test_partial_coverage_wrong_order_and_unmet_quality_refuse(self):
        for corruption in ("missing", "domain", "quality", "budget"):
            ref, native = fixture()
            if corruption == "missing": ref["cases"][0]["cells"].pop()
            elif corruption == "domain": ref["cases"][0]["cells"][0]["chart_domain"][1] = 0.5
            elif corruption == "quality": ref["cases"][0]["normal_error_upper"] = 1.
            else: ref["max_patches"] = 3
            with self.assertRaises(ValueError): grade(ref, native)

    def test_wrong_topology_and_unexpected_solid_refuse(self):
        for field, value in (("faces", 3), ("solids", 1), ("shells", 2), ("euler", 1), ("boundary_cycles", [2, 2, 4]), ("edge_incidence_valid", False), ("boundary_degree_two", False), ("face_connected", False)):
            ref, native = fixture()
            native["cases"][0][field] = value
            with self.assertRaises(ValueError): grade(ref, native)

    def test_missing_face_or_corrupted_native_poles_and_values_refuse(self):
        for corruption in ("missing", "duplicate", "pole", "point", "nonfinite"):
            ref, native = fixture()
            cells = native["cases"][0]["surface_cells"]
            if corruption == "missing": cells.pop()
            elif corruption == "duplicate": cells[0]["cell"] = 1
            elif corruption == "pole": cells[0]["poles_m"][0][0][0] += 1e-4
            elif corruption == "point": cells[0]["samples"][0]["point_m"][0] += 1e-4
            else: cells[0]["samples"][0]["point_m"][0] = float("nan")
            with self.assertRaises(ValueError): grade(ref, native)

    def test_wrong_phase_or_missing_boundary_geometry_refuse(self):
        for corruption in ("phase", "missing", "same_support"):
            ref, native = fixture()
            loops = native["cases"][1]["boundary_loops_m"]
            if corruption == "phase": native["cases"][1]["boundary_loops_m"] = copy.deepcopy(native["cases"][0]["boundary_loops_m"])
            elif corruption == "missing": loops.pop()
            else: loops[1] = copy.deepcopy(loops[0])
            with self.assertRaises(ValueError): grade(ref, native)


if __name__ == "__main__": unittest.main()
