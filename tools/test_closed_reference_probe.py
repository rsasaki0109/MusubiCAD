"""Synthetic contract fixtures only; actual OCCT construction is a separate test."""
import copy
import math
import unittest
from run_closed_reference_probe import grade


def fixture():
    reference = {"format_version": 1, "radius_m": 0.01, "height_m": 0.02, "cases": []}
    native = {"format_version": 1, "occt_version": "8.0.0", "cases": []}
    for law in ("constant", "variable"):
        r = 0.0005
        v = math.pi*0.01**2*0.02-2*math.pi*((0.01-r)*(1-math.pi/4)*r*r+r**3/6)
        lo, hi = (math.nextafter(v, -math.inf), math.nextafter(v, math.inf)) if law == "constant" else (6.2684363e-6, 6.2684372e-6)
        ideal = {"minimum_radius_m": r, "maximum_radius_m": r if law == "constant" else 0.0015,
                 "body_lower_m3": lo, "body_upper_m3": hi, "cells": 0 if law == "constant" else 4, "absolute_width_m3": 1e-12}
        for level, n, requested in (("coarse", 1, 1e-4), ("fine", 2, 2e-5)):
            count, skirts = 4*n*n, 4*n
            ref = {"name": f"{law}-{level}", "law": law, "divisions": n, "requested_position_error_m": requested,
                   "position_error_upper_m": requested/2, "normal_error_upper": 0.5, "ideal": copy.deepcopy(ideal), "cells": []}
            actual = {"brep_valid": True, "shell_closed": True, "face_connected": True, "edge_incidence_two": True, "cap_normals_outward": True,
                      "free_edges": 0, "shells": 1, "solids": 1, "euler": 2, "planar_caps": 2,
                      "faces": count+skirts+2, "vertices": 4*n*(n+2), "edges": 4*n*(2*n+3),
                      "volume_m3": (lo+hi)/2-(1e-10 if level == "coarse" else 1e-11), "volume_refinement_delta_m3": 1e-19,
                      "max_sampled_skirt_cylinder_error_m": 1e-8 if level == "coarse" else 1e-9, "surface_cells": []}
            for c in range(count):
                chart, k = divmod(c, n*n)
                i, j = divmod(k, n)
                poles = [[[0.001*(c+a/3), 0.001*b/3, 0.02] for b in range(4)] for a in range(4)]
                ref["cells"].append({"chart": chart, "chart_domain": [i/n, (i+1)/n, j/n, (j+1)/n], "control_points_m": poles})
                row = {"band_cell": c, "skirt_index": -1, "samples": []}
                for a in range(3):
                    for b in range(3):
                        row["samples"].append({"u": a/2, "v": b/2, "point_m": [0.001*(c+a/2), 0.001*b/2, 0.02]})
                actual["surface_cells"].append(row)
            for k in range(skirts):
                row = {"band_cell": -1, "skirt_index": k, "samples": []}
                for a in range(3):
                    for b in range(3):
                        row["samples"].append({"u": a/2, "v": b/2, "point_m": [0.001*(k*n+a/2), 0., 0.02*b/2]})
                actual["surface_cells"].append(row)
            reference["cases"].append(ref)
            native["cases"].append(actual)
    return reference, native


class Contract(unittest.TestCase):
    def test_complete_report_does_not_admit_a_product_body(self):
        result = grade(*fixture())
        self.assertTrue(result["diagnostic_passed"])
        self.assertFalse(result["candidate_admitted"])
        self.assertFalse(result["product_body_created"])
        self.assertEqual(result["native_diagnostic_solids_created"], 4)

    def test_open_or_disconnected_topology_and_missing_caps_refuse(self):
        for field, value in (("shell_closed", False), ("face_connected", False), ("free_edges", 1), ("solids", 0), ("euler", 0), ("planar_caps", 1), ("edge_incidence_two", False), ("cap_normals_outward", False)):
            ref, native = fixture()
            native["cases"][0][field] = value
            with self.assertRaises(ValueError): grade(ref, native)

    def test_nonpositive_unstable_or_nonfinite_native_volume_refuses(self):
        for field, value in (("volume_m3", -1.), ("volume_m3", 0.), ("volume_m3", float("nan")), ("volume_refinement_delta_m3", 1e-9)):
            ref, native = fixture()
            native["cases"][0][field] = value
            with self.assertRaises(ValueError): grade(ref, native)

    def test_inconsistent_or_wrong_ideal_reference_refuses(self):
        for corruption in ("radius", "constant", "between_levels"):
            ref, native = fixture()
            if corruption == "radius": ref["cases"][0]["ideal"]["maximum_radius_m"] = 0.001
            elif corruption == "constant":
                ref["cases"][0]["ideal"]["body_lower_m3"] -= 1e-13
                ref["cases"][0]["ideal"]["body_upper_m3"] -= 1e-13
            else: ref["cases"][3]["ideal"]["body_lower_m3"] += 1e-14
            with self.assertRaises(ValueError): grade(ref, native)

    def test_missing_duplicate_or_corrupted_surfaces_refuse(self):
        for corruption in ("missing", "duplicate", "point"):
            ref, native = fixture()
            surfaces = native["cases"][0]["surface_cells"]
            if corruption == "missing": surfaces.pop()
            elif corruption == "duplicate": surfaces[0] = copy.deepcopy(surfaces[1])
            else: surfaces[0]["samples"][0]["point_m"][0] += 1e-4
            with self.assertRaises(ValueError): grade(ref, native)

    def test_absent_or_worsening_refinement_refuses(self):
        for corruption in ("same_level", "volume", "skirt"):
            ref, native = fixture()
            if corruption == "same_level":
                fine = ref["cases"][1]
                fine["divisions"] = ref["cases"][0]["divisions"]
                fine["cells"] = copy.deepcopy(ref["cases"][0]["cells"])
                native["cases"][1] = copy.deepcopy(native["cases"][0])
            elif corruption == "volume": native["cases"][1]["volume_m3"] = native["cases"][0]["volume_m3"]-1e-9
            else: native["cases"][1]["max_sampled_skirt_cylinder_error_m"] = 1e-7
            with self.assertRaises(ValueError): grade(ref, native)


if __name__ == "__main__": unittest.main()
