"""Synthetic report-contract data only; live OCCT construction runs separately."""
import copy
from fractions import Fraction as F
import math
import unittest
from test_rational_contact_probe import fixture as contact_fixture
from run_rational_reference_probe import cubic_pcurve, edge_fraction, envelope, grade, tensor
from run_reference_bezier_probe import unit_cross
from run_rational_contact_probe import rational


def fixture():
    """Build mathematical samples and invented native flags for parser tests."""
    contacts, _ = contact_fixture()
    def add(a, b):
        out = dict(a)
        for key, value in b.items(): out[key] = out.get(key, F(0))+value
        return out
    def scale(a, s): return {k: v*s for k, v in a.items()}
    def mul(a, b):
        out = {}
        for (i, j), v in a.items():
            for (k, l), w in b.items(): out[i+k, j+l] = out.get((i+k, j+l), F(0))+v*w
        return out
    def bcoef(a, i, j):
        return sum((value*F(math.comb(i, p), math.comb(8, p))*F(math.comb(j, q), math.comb(2, q)) for (p, q), value in a.items() if p <= i and q <= j), F(0))
    reference = {"format_version": 1, "radius_m": .01, "height_m": .02, "normal_error_request": 1e-9, "normal_max_leaves": 4096, "cases": []}
    native = {"format_version": 1, "occt_version": "8.0.0", "cases": []}
    for law in range(2):
        minimum, maximum = F(.0005), F(.0005 if law == 0 else .0015)
        m, a, r, h = (minimum+maximum)/2, (maximum-minimum)/2, F(.01), F(.02)
        constant = math.pi*.01**2*.02-2*math.pi*((.01-.0005)*(1-math.pi/4)*.0005**2+.0005**3/6)
        volume = constant if law == 0 else 6.2684367805e-6
        ref = {"law": "constant" if law == 0 else "variable", "minimum_radius_m": float(minimum), "maximum_radius_m": float(maximum), "patches": [], "contacts": contacts["curves"][law*8:law*8+8], "side_pcurves": [], "volume_lower_m3": volume-(1e-20 if law == 0 else 4e-13), "volume_upper_m3": volume+(1e-20 if law == 0 else 4e-13), "constant_limit_volume_m3": constant if law == 0 else None}
        actual = {"valid": True, "closed": True, "connected": True, "edge_incidence_two": True, "faces": 10, "edges": 20, "vertices": 12, "shells": 1, "solids": 1, "euler": 2, "free_edges": 0, "multiple_edges": 0, "volume_m3": volume, "volume_refinement_delta_m3": 0., "rotated_volume_m3": volume, "center_of_mass_m": [0, 0, .01], "vertex_tolerances_m": [1e-10]*12, "surface_faces": [], "shared_edges": [], "classifications": []}
        for chart in range(4):
            d, c, s = {(0, 0): F(1), (2, 0): F(1)}, {(0, 0): F(1), (2, 0): F(-1)}, {(1, 0): F(2)}
            if chart == 1: c, s = scale(s, -1), c
            elif chart == 2: c, s = scale(c, -1), scale(s, -1)
            elif chart == 3: c, s = s, scale(c, -1)
            n = add(scale(d, m), scale(c, a));l = add(scale(d, r), scale(n, -1));k = scale(s, -a)
            l2, k2 = mul(l, l), mul(k, k)
            wc = add(mul(l2, {(0, 0): F(1), (0, 2): F(1)}), mul(k2, {(0, 2): F(2)}))
            nr = mul(l2, {(0, 0): F(1), (0, 2): F(-1)})
            nt = mul(mul(k, l), {(0, 1): F(2), (0, 2): F(-2)})
            nz = add(mul(l2, {(0, 1): F(2)}), mul(k2, {(0, 2): F(2)}))
            w = mul(mul(d, d), wc)
            nums = [add(mul(mul(l, c), wc), mul(n, add(mul(nr, c), scale(mul(nt, s), -1)))), add(mul(mul(l, s), wc), mul(n, add(mul(nr, s), mul(nt, c)))), add(mul(mul(add(scale(d, h), scale(n, -1)), d), wc), mul(mul(n, d), nz))]
            weights = [[float(bcoef(w, i, j)) for j in range(3)] for i in range(9)]
            poles = [[[float(bcoef(p, i, j)/F(weights[i][j])) for p in nums] for j in range(3)] for i in range(9)]
            patch = {"chart": chart, "u_degree": 8, "v_degree": 2, "control_points_m": poles, "weights": weights, "position_error_upper_m": 5e-13, "normal_bounds": {"jacobian_area_lower_m2": 1e-6, "jacobian_area_upper_m2": 1e-3, "ideal_normal_alignment_lower": .999999999, "normal_error_upper": 1e-9, "du_error_upper_m": 1e-13, "dv_error_upper_m": 1e-13, "leaves": 64}}
            ref["patches"].append(patch)
            face = {"role": {"kind": 0, "chart": chart}, "forward": True, "u_degree": 8, "v_degree": 2, "poles_m": copy.deepcopy(poles), "weights": copy.deepcopy(weights), "samples": []}
            for i in range(9):
                for j in range(9):
                    p, du, dv = tensor(poles, weights, i/8, j/8);normal, _ = unit_cross(du, dv)
                    face["samples"].append({"u": i/8, "v": j/8, "point_m": p, "du_m": du, "dv_m": dv, "normal": normal})
            actual["surface_faces"].append(face)
            pc = {"chart": chart, "divisions": 256, "control_points_rad_m": [[0., 0.] for _ in range(769)], "angle_error_upper_rad": 5e-11, "height_error_upper_m": 1e-16 if law == 0 else 3e-13, "position_error_upper_m": 1e-12}
            def endpoint(t):
                theta = chart*math.pi/2+2*math.atan(t)
                return [theta, .02-float(m)-float(a)*math.cos(theta)], [2/(1+t*t), float(a)*math.sin(theta)*2/(1+t*t)]
            for i in range(256):
                p, dp = endpoint(i/256);q, dq = endpoint((i+1)/256)
                pc["control_points_rad_m"][3*i:3*i+4] = [p, [p[k]+dp[k]/768 for k in range(2)], [q[k]-dq[k]/768 for k in range(2)], q]
            ref["side_pcurves"].append(pc)
        for kind in (1, 1, 1, 1, 2, 3):
            actual["surface_faces"].append({"role": {"kind": kind, "chart": -1}, "forward": kind != 3, "support_origin_m": [0, 0, .02 if kind == 2 else 0], "support_axis": [0, 0, 1], "support_x": [1, 0, 0], "support_radius_m": .01 if kind == 1 else 0.})
        def side_sample(kind, chart, uv):
            u, v = uv
            if kind == 0:
                patch = ref["patches"][chart];p, du, dv = tensor(patch["control_points_m"], patch["weights"], u, v);normal, _ = unit_cross(du, dv)
            elif kind == 1: p, normal = [.01*math.cos(u), .01*math.sin(u), v], [math.cos(u), math.sin(u), 0.]
            else: p, normal = [u, v, .02 if kind == 2 else 0.], [0., 0., 1. if kind == 2 else -1.]
            return {"uv": uv, "point_m": p, "normal": normal}
        for category in range(5):
            for chart in range(4):
                pair = [(0, chart), (0, (chart+1)%4)] if category == 0 else [(0, chart), (1, -1)] if category == 1 else [(0, chart), (2, -1)] if category == 2 else [(1, -1), (1, -1)] if category == 3 else [(1, -1), (3, -1)]
                edge = {"roles": [{"kind": k, "chart": c} for k, c in pair], "same_parameter": True, "same_range": True, "tolerance_m": 1e-10, "range": [0., 1.], "samples": []}
                if category in (1, 2):
                    contact = ref["contacts"][(0 if category == 1 else 4)+chart]
                    edge["contact_curve"] = {"degree": contact["degree"], "poles_m": copy.deepcopy(contact["control_points_m"]), "weights": contact["weights"][:], "knots": [0, 1], "multiplicities": [contact["degree"]+1]*2}
                    if category == 1: edge["cylindrical_pcurve"] = {"degree": 3, "rational": False, "periodic": False, "poles_rad_m": copy.deepcopy(ref["side_pcurves"][chart]["control_points_rad_m"]), "knots": [i/256 for i in range(257)], "multiplicities": [4]+[3]*255+[4]}
                for j in range(9):
                    t = edge_fraction(j)
                    if category == 0: p, _, _ = tensor(ref["patches"][chart]["control_points_m"], ref["patches"][chart]["weights"], 1., t);uvs = [[1., t], [0., t]]
                    elif category in (1, 2):
                        p, _ = rational(contact["control_points_m"], contact["weights"], t)
                        uvs = [[t, 0. if category == 1 else 1.], cubic_pcurve(edge["cylindrical_pcurve"]["poles_rad_m"], t) if category == 1 else p[:2]]
                    elif category == 3:
                        theta = chart*math.pi/2;p = [.01*math.cos(theta), .01*math.sin(theta), .018*t];uvs = [[theta, p[2]]]*2
                    else:
                        theta = (chart+t)*math.pi/2;p = [.01*math.cos(theta), .01*math.sin(theta), 0.];uvs = [[theta, 0.], p[:2]]
                    edge["samples"].append({"t": t, "point_m": p, "sides": [side_sample(k, c, uv) for (k, c), uv in zip(pair, uvs)]})
                actual["shared_edges"].append(edge)
        for j in range(8):
            theta, offset = j//2*math.pi/2, -.000025 if j%2 == 0 else .000025
            p, n = envelope(law, theta, math.pi/4);state = 1 if j%2 == 0 else 0
            actual["classifications"].append({"theta_rad": theta, "offset_m": offset, "point_m": [x+offset*y for x, y in zip(p, n)], "state": state, "rotated_state": 1-state if law == 1 and j in (1, 4) else state})
        reference["cases"].append(ref);native["cases"].append(actual)
    return reference, native


class Contract(unittest.TestCase):
    def test_complete_report_keeps_product_admission_false(self):
        result = grade(*fixture())
        self.assertTrue(result["diagnostic_passed"])
        self.assertFalse(result["candidate_admitted"])
        self.assertFalse(result["product_body_created"])
        self.assertEqual(result["native_diagnostic_solids_created"], 2)
        self.assertEqual(result["rotated_classifier_mismatches"], [0, 2])

    def test_invalid_topology_inventory_and_orientation_refuse(self):
        ref, base = fixture()
        for key, value in (("valid", False), ("closed", False), ("connected", False), ("edge_incidence_two", False), ("faces", 9), ("edges", 21), ("vertices", 13), ("euler", 1), ("shells", 2), ("solids", True), ("free_edges", 1), ("multiple_edges", 1)):
            native = copy.deepcopy(base);native["cases"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key in ("surface_faces", "shared_edges", "vertex_tolerances_m", "classifications"):
            native = copy.deepcopy(base);native["cases"][0][key].pop()
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["cases"][0]["surface_faces"][0]["forward"] = False
        with self.assertRaises(ValueError): grade(ref, native)

    def test_current_control_net_derivatives_and_normal_refuse(self):
        ref, base = fixture()
        for key, value in (("u_degree", 7), ("v_degree", 3), ("weights", [[0.]*3]*9), ("poles_m", [[[0., 0., 0.]]*3]*9)):
            native = copy.deepcopy(base);native["cases"][0]["surface_faces"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key in ("point_m", "du_m", "dv_m", "normal"):
            native = copy.deepcopy(base);native["cases"][0]["surface_faces"][0]["samples"][40][key][0] += .001
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["cases"][0]["surface_faces"][0]["samples"].pop()
        with self.assertRaises(ValueError): grade(ref, native)

    def test_analytic_support_and_pcurve_relations_refuse(self):
        ref, base = fixture()
        for key, value in (("support_radius_m", .01001), ("support_origin_m", [0, 0, .0001]), ("support_axis", [0, 1, 0]), ("support_x", [0, 1, 0])):
            native = copy.deepcopy(base);native["cases"][0]["surface_faces"][4][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("uv", [2., 0.]), ("point_m", [0, 0, 0]), ("normal", [0, 0, 1])):
            native = copy.deepcopy(base);native["cases"][0]["shared_edges"][4]["samples"][3]["sides"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)

    def test_contact_representation_and_bounded_pcurve_controls_refuse(self):
        ref, base = fixture()
        for key, value in (("degree", 8), ("weights", [0, 1, 2]), ("knots", [0, 2]), ("multiplicities", [2, 2]), ("poles_m", [[0, 0, 0]]*3)):
            native = copy.deepcopy(base);native["cases"][0]["shared_edges"][4]["contact_curve"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("degree", 2), ("rational", True), ("periodic", True), ("knots", [0, 1]), ("multiplicities", [4, 4]), ("poles_rad_m", [[0, 0]]*769)):
            native = copy.deepcopy(base);native["cases"][0]["shared_edges"][4]["cylindrical_pcurve"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("divisions", 128), ("angle_error_upper_rad", 1e-15), ("height_error_upper_m", 0.), ("position_error_upper_m", 1e-15)):
            changed = copy.deepcopy(ref);changed["cases"][0]["side_pcurves"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(changed, base)

    def test_wrong_join_adjacency_parameters_and_missing_witnesses_refuse(self):
        ref, base = fixture()
        for key, value in (("roles", [{"kind": 0, "chart": 0}, {"kind": 0, "chart": 0}]), ("same_parameter", False), ("same_range", False), ("range", [0, 0])):
            native = copy.deepcopy(base);native["cases"][0]["shared_edges"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["cases"][0]["shared_edges"][0]["samples"].pop()
        with self.assertRaises(ValueError): grade(ref, native)

    def test_nonfinite_inflated_tolerances_and_unresolved_bounds_refuse(self):
        ref, base = fixture()
        for value in (-1., 1e-8, float("nan"), float("inf"), True):
            for site in ("vertex", "edge"):
                native = copy.deepcopy(base)
                if site == "vertex": native["cases"][0]["vertex_tolerances_m"][0] = value
                else: native["cases"][0]["shared_edges"][0]["tolerance_m"] = value
                with self.subTest(value=value, site=site), self.assertRaises(ValueError): grade(ref, native)
        changed = copy.deepcopy(ref);changed["cases"][0]["patches"][0]["position_error_upper_m"] = 1e-9
        with self.assertRaises(ValueError): grade(changed, base)

    def test_independent_volume_and_phase_controls_refuse(self):
        ref, base = fixture()
        for key, value in (("volume_m3", 6.28e-6), ("rotated_volume_m3", 6.28e-6), ("volume_refinement_delta_m3", 1e-10), ("center_of_mass_m", [0, 0, float("nan")])):
            native = copy.deepcopy(base);native["cases"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)

        native = copy.deepcopy(base)
        for row in native["cases"][1]["classifications"]: row["rotated_state"] = row["state"]
        with self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("state", 0), ("rotated_state", 0), ("point_m", [0, 0, .01])):
            native = copy.deepcopy(base);native["cases"][0]["classifications"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)

    def test_invalid_global_normal_certificates_and_work_refuse(self):
        ref, native = fixture()
        for key, value in (("jacobian_area_lower_m2", 0.), ("jacobian_area_upper_m2", 1e-10), ("ideal_normal_alignment_lower", -1.), ("normal_error_upper", 2e-9), ("du_error_upper_m", float("nan")), ("dv_error_upper_m", 1e-8), ("leaves", 0), ("leaves", 4097), ("leaves", True)):
            changed = copy.deepcopy(ref);changed["cases"][0]["patches"][0]["normal_bounds"][key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError): grade(changed, native)
        for key, value in (("normal_error_request", 1e-7), ("normal_max_leaves", True), ("normal_max_leaves", 1)):
            changed = copy.deepcopy(ref);changed[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(changed, native)
        changed = copy.deepcopy(ref);del changed["cases"][0]["patches"][0]["normal_bounds"]
        with self.assertRaises(KeyError): grade(changed, native)

    def test_native_jacobian_outside_claimed_global_bounds_refuses(self):
        ref, native = fixture()
        ref["cases"][0]["patches"][0]["normal_bounds"].update(jacobian_area_lower_m2=1e-8, jacobian_area_upper_m2=2e-8)
        with self.assertRaisesRegex(ValueError, "outside rounded rational certificate"): grade(ref, native)

    def test_positionally_close_coarse_pcurve_still_fails_normal_gate(self):
        ref, native = fixture()
        edge = native["cases"][0]["shared_edges"][4]
        # A 2e-9 rad angular offset is only 2e-11 m on this cylinder, within
        # the 1e-9 m join gate. All changed witnesses agree with their changed
        # pcurve/support, but the paired tangent-plane gate must still refuse.
        for control in edge["cylindrical_pcurve"]["poles_rad_m"]: control[0] += 2e-9
        ref["cases"][0]["side_pcurves"][0]["control_points_rad_m"] = copy.deepcopy(edge["cylindrical_pcurve"]["poles_rad_m"])
        for sample in edge["samples"]:
            side = sample["sides"][1]
            u, v = cubic_pcurve(edge["cylindrical_pcurve"]["poles_rad_m"], sample["t"])
            side.update(uv=[u, v], point_m=[.01*math.cos(u), .01*math.sin(u), v], normal=[math.cos(u), math.sin(u), 0.])
        with self.assertRaisesRegex(ValueError, "tangent-plane witness"):
            grade(ref, native)


if __name__ == "__main__": unittest.main()
