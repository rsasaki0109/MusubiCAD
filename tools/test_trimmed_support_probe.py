"""Synthetic report-contract tests only; these faces are not native evidence."""
import copy
import math
import unittest
from test_rational_contact_probe import fixture as contacts_fixture
from run_rational_contact_probe import rational
from run_trimmed_support_probe import expected_queries, grade


def fixture():
    reference, _ = contacts_fixture()
    native = {"format_version": 1, "occt_version": "8.0.0", "faces": []}
    for law in range(2):
        for slot in range(5):
            side, chart = slot < 4, slot if slot < 4 else -1
            m, a = (.0005, 0.) if law == 0 else (.001, .0005)
            area = .01*((.02-m)*math.pi/2-a*(math.sin((chart+1)*math.pi/2)-math.sin(chart*math.pi/2))) if side else math.pi*((.01-m)**2+a*a/2)
            face = {"kind": "side" if side else "end", "chart": chart, "valid": True, "forward": True,
                    "closed_wire": True, "wires": 1, "edges": 4, "vertices": 4, "shells": 0, "solids": 0,
                    "support_type": "cylinder" if side else "plane", "support_origin_m": [0., 0., 0. if side else .02],
                    "support_axis": [0., 0., 1.], "support_x": [1., 0., 0.], "support_radius_m": .01 if side else 0.,
                    "area_m2": area, "projection_achieved_m": 1e-10, "vertex_tolerances_m": [1e-10]*4, "boundaries": []}
            for j in range(4):
                contact = chart if side and j == 0 else -1 if side else j
                edge = {"contact_chart": contact, "tolerance_m": 1e-10, "same_parameter": True, "same_range": True,
                        "range": [0., 1.], "pcurve_range": [0., 1.], "samples": []}
                ref = None
                if contact >= 0:
                    ref = reference["curves"][law*8+(0 if side else 4)+contact]
                    edge.update(degree=ref["degree"], poles_m=copy.deepcopy(ref["control_points_m"]), weights=ref["weights"][:], knots=[0, 1], multiplicities=[ref["degree"]+1]*2)
                for k in range(17):
                    t = k/16
                    if ref is not None: point, _ = rational(ref["control_points_m"], ref["weights"], t)
                    else:
                        # Synthetic bottom arc / axial generators; no native construction.
                        angle = (chart+t)*math.pi/2 if j == 1 else (chart+(1 if j == 2 else 0))*math.pi/2
                        point = [.01*math.cos(angle), .01*math.sin(angle), 0. if j == 1 else .018*t]
                    if side:
                        u = math.atan2(point[1], point[0])
                        if u < chart*math.pi/2-1e-12: u += 2*math.pi
                        uv, normal = [u, point[2]], [math.cos(u), math.sin(u), 0.]
                        lifted = [.01*math.cos(u), .01*math.sin(u), point[2]]
                    else: uv, normal, lifted = point[:2], [0., 0., 1.], point[:]
                    edge["samples"].append({"t": t, "uv": uv, "point_m": point, "lifted_m": lifted, "normal": normal})
                face["boundaries"].append(edge)
            face["classifications"] = [{"point_m": p, "state": s} for p, s in expected_queries(law, chart)]
            face["rotated_classifications"] = [{"point_m": p, "state": s} for p, s in expected_queries(law, chart, True)] if side else []
            if law == 1 and side and chart in (0, 2):
                face["rotated_classifications"][0 if chart == 0 else 1]["state"] ^= 1
            native["faces"].append(face)
    return reference, native


class Contract(unittest.TestCase):
    def test_complete_report_keeps_feature_and_body_unsupported(self):
        result = grade(*fixture())
        self.assertTrue(result["diagnostic_passed"])
        self.assertFalse(result["candidate_admitted"])
        self.assertFalse(result["product_body_created"])
        self.assertEqual(result["native_trimmed_cylinder_faces_created"], 8)
        self.assertEqual(result["native_trimmed_plane_faces_created"], 2)
        self.assertEqual(result["rotated_region_mismatches"], [0, 2])

    def test_invalid_inventory_or_topology_refuses(self):
        for key, value in (("valid", False), ("forward", False), ("closed_wire", False), ("wires", 2), ("edges", True), ("vertices", 5), ("shells", 1), ("solids", 1), ("chart", 3), ("kind", "end")):
            ref, native = fixture();native["faces"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for part in ("faces", "boundaries", "samples"):
            ref, native = fixture()
            container = native["faces"] if part == "faces" else native["faces"][0]["boundaries"] if part == "boundaries" else native["faces"][0]["boundaries"][0]["samples"]
            container.pop()
            with self.subTest(part=part), self.assertRaises(ValueError): grade(ref, native)

    def test_changed_support_or_area_refuses(self):
        for key, value in (("support_type", "bspline"), ("support_radius_m", .01001), ("support_origin_m", [0, 0, .00001]), ("support_axis", [0, 1, 0]), ("support_x", [0, 1, 0]), ("area_m2", .0001)):
            ref, native = fixture();native["faces"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        ref, native = fixture();native["faces"][4]["support_origin_m"][2] += .001
        with self.assertRaises(ValueError): grade(ref, native)

    def test_current_pcurve_range_flags_lifts_and_normal_refuse(self):
        for key, value in (("same_parameter", False), ("same_range", False), ("range", [0, 0]), ("pcurve_range", [0, 2])):
            ref, native = fixture();native["faces"][0]["boundaries"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("t", .5), ("point_m", [.02, 0, .02]), ("lifted_m", [.02, 0, .02]), ("uv", [5, .02]), ("normal", [0, 0, 1])):
            ref, native = fixture();native["faces"][0]["boundaries"][0]["samples"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)

    def test_inflated_missing_or_nonfinite_tolerances_refuse(self):
        for value in (-1., 1e-8, float("nan"), float("inf"), True):
            for location in ("projection", "vertex", "edge"):
                ref, native = fixture();face = native["faces"][0]
                if location == "projection": face["projection_achieved_m"] = value
                elif location == "vertex": face["vertex_tolerances_m"][0] = value
                else: face["boundaries"][0]["tolerance_m"] = value
                with self.subTest(value=value, location=location), self.assertRaises(ValueError): grade(ref, native)
        ref, native = fixture();native["faces"][0]["vertex_tolerances_m"].pop()
        with self.assertRaises(ValueError): grade(ref, native)

    def test_replaced_contact_controls_and_law_refuse(self):
        for key, value in (("contact_chart", -1), ("degree", 4), ("poles_m", [[0, 0, 0]]*3), ("weights", [1, 0, 2]), ("knots", [0, 2]), ("multiplicities", [2, 2])):
            ref, native = fixture();native["faces"][0]["boundaries"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        ref, native = fixture();ref["curves"][0]["maximum_radius_m"] = .0015
        with self.assertRaises(ValueError): grade(ref, native)

    def test_region_and_native_phase_controls_refuse(self):
        for index in (0, 4, 5, 9):
            ref, native = fixture();native["faces"][index]["classifications"][0]["state"] ^= 1
            with self.subTest(index=index), self.assertRaises(ValueError): grade(ref, native)
        for control in ("missing", "variable passes", "constant fails", "wrong witness"):
            ref, native = fixture()
            if control == "missing": native["faces"][0]["rotated_classifications"] = []
            elif control == "variable passes":
                for chart in range(4): native["faces"][5+chart]["rotated_classifications"] = [{"point_m": p, "state": s} for p, s in expected_queries(1, chart, True)]
            elif control == "constant fails": native["faces"][0]["rotated_classifications"][0]["state"] ^= 1
            else: native["faces"][0]["classifications"][0]["point_m"][2] += .001
            with self.subTest(control=control), self.assertRaises(ValueError): grade(ref, native)


if __name__ == "__main__": unittest.main()
