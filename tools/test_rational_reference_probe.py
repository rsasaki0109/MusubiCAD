"""Synthetic report-contract data only; live OCCT construction runs separately."""
import copy
from fractions import Fraction as F
import math
import unittest
from test_rational_contact_probe import fixture as contact_fixture
from run_rational_reference_probe import cubic_pcurve, edge_fraction, envelope, grade, tensor, snapshot_request, contact_request, contact_edges, end_boundary_request, side_boundary_request, seam_boundary_request, seam_edges, SEAM_REFUSALS, SEAM_GAPS, boundary_normal_request, NORMAL_REFUSALS, NORMAL_GAPS
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
            face = {"role": {"kind": 0, "chart": chart}, "forward": True, "location_identity": True, "u_degree": 8, "v_degree": 2, "poles_m": copy.deepcopy(poles), "poles_mm": [[[x*1000 for x in p] for p in row] for row in poles], "u_bounds": [0., 1.], "v_bounds": [0., 1.], "u_periodic": False, "v_periodic": False, "weights": copy.deepcopy(weights), "samples": []}
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
            actual["surface_faces"].append({"role": {"kind": kind, "chart": -1}, "forward": kind != 3, "location_identity": True, "support_origin_m": [0, 0, .02 if kind == 2 else 0], "support_axis": [0, 0, 1], "support_x": [1, 0, 0], "support_radius_m": .01 if kind == 1 else 0.})
            if kind == 2: actual["surface_faces"][-1].update(plane_origin_mm=[0., 0., 20.], plane_x_direction=[1., 0., 0.], plane_y_direction=[0., 1., 0.])
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
                if category == 0:
                    patch = ref["patches"][chart]
                    poles = patch["control_points_m"][-1]
                    edge["seam_curve"] = {"kind": "geom_bezier_curve", "degree": 2, "location_identity": True, "periodic": False, "poles_mm": [[x*1000 for x in p] for p in poles], "poles_m": copy.deepcopy(poles), "weights": patch["weights"][-1][:]}
                    edge["seam_lines"] = [{"kind": "geom2d_line", "origin": [end,0.], "direction": [0.,1.], "range": [0.,1.]} for end in (1.,0.)]
                if category in (1, 2):
                    contact = ref["contacts"][(0 if category == 1 else 4)+chart]
                    edge["contact_curve"] = {"degree": contact["degree"], "location_identity": True, "periodic": False, "poles_mm": [[x*1000 for x in p] for p in contact["control_points_m"]], "poles_m": copy.deepcopy(contact["control_points_m"]), "weights": contact["weights"][:], "knots": [0, 1], "multiplicities": [contact["degree"]+1]*2}
                    if category == 1:
                        poles = copy.deepcopy(ref["side_pcurves"][chart]["control_points_rad_m"])
                        edge["cylindrical_pcurve"] = {"degree": 3, "rational": False, "periodic": False, "poles_rad_m": poles, "poles_rad_mm": [[p[0],p[1]*1000] for p in poles], "weights": [1.]*769, "range": [0.,1.], "knots": [i/256 for i in range(257)], "multiplicities": [4]+[3]*255+[4]}
                        edge["cylinder_frame"] = {"origin_mm": [0.,0.,0.], "directions": [[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]], "radius_mm": 10.}
                        edge["blend_side_line"] = {"kind": "geom2d_line", "origin": [0.,0.], "direction": [1.,0.], "range": [0.,1.]}
                    else:
                        edge["planar_pcurve"] = {"degree": 4, "periodic": False, "range": [0., 1.], "poles_mm": [p[:2] for p in edge["contact_curve"]["poles_mm"]], "weights": contact["weights"][:], "knots": [0., 1.], "multiplicities": [5, 5]}
                        edge["blend_end_line"] = {"kind": "geom2d_line", "origin": [0., 1.], "direction": [1., 0.], "range": [0., 1.]}
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
    # Invented certificate records test parsing only. Live execution calls the
    # Rust proof on post-sewing OCCT raw controls, never this fixture generator.
    native["surface_definition_proof_refusals"] = {"input_limit": True, "wrong_chart": True, "last_request_precision": True}
    native["surface_definition_certificates"] = {"format_version": 1, "native_floating_evaluation_certified": False, "whole_brep_certified": False, "certificates": [
        {"request": copy.deepcopy(snapshot_request(reference, case, face)), "position_error_upper_m": 5e-13,
         "normal_bounds": copy.deepcopy(case["patches"][face["role"]["chart"]]["normal_bounds"])}
        for case, actual in zip(reference["cases"], native["cases"]) for face in actual["surface_faces"] if face["role"]["kind"] == 0]}
    native["contact_definition_proof_refusals"] = {key: True for key in ("input_limit", "wrong_chart", "wrong_units", "nonuniform_weight", "missing_control", "last_request_precision")}
    native["contact_definition_certificates"] = {"format_version": 1, "native_floating_evaluation_certified": False, "attached_pcurve_lifts_certified": False, "whole_brep_certified": False, "certificates": [
        {"request": copy.deepcopy(contact_request(reference, case, edge)), "position_error_upper_m": 5e-13}
        for case, actual in zip(reference["cases"], native["cases"]) for edge in contact_edges(actual)]}
    native["end_boundary_proof_refusals"] = {key: True for key in ("input_limit", "wrong_line", "wrong_units", "changed_frame", "changed_pcurve", "missing_control", "last_request_precision", "exhausted_work")}
    native["end_boundary_certificates"] = {"format_version": 1, "native_floating_evaluation_certified": False, "side_contact_boundaries_certified": False, "whole_brep_certified": False, "certificates": [
        {"request": copy.deepcopy(end_boundary_request(reference, case, actual, edge)),
         "edge_position_error_upper_m": 1e-13, "plane_lift_position_error_upper_m": 1e-13, "blend_position_error_upper_m": 5e-13,
         "edge_to_plane_gap_upper_m": 3e-13, "edge_to_blend_gap_upper_m": 7e-13, "plane_to_blend_gap_upper_m": 7e-13,
         "blend_normal_bounds": copy.deepcopy(case["patches"][next(r["chart"] for r in edge["roles"] if r["kind"] == 0)]["normal_bounds"])}
        for case, actual in zip(reference["cases"], native["cases"]) for edge in contact_edges(actual) if any(r["kind"] == 2 for r in edge["roles"])]}
    native["side_boundary_proof_refusals"] = {key: True for key in ("input_limit", "wrong_line", "nonunit_weight", "wrong_units", "changed_frame", "changed_radius", "changed_angle", "changed_height", "missing_control", "last_request_precision", "exhausted_work")}
    native["side_boundary_certificates"] = {"format_version": 1, "native_floating_evaluation_certified": False, "end_contact_boundaries_certified": False, "whole_brep_certified": False, "certificates": [
        {"request": copy.deepcopy(side_boundary_request(reference, case, actual, edge)),
         "edge_position_error_upper_m": 1e-13, "cylindrical_lift_position_error_upper_m": 7e-13, "blend_position_error_upper_m": 1e-13,
         "angle_error_upper_rad": 5e-11, "height_error_upper_m": 1e-16 if case["law"] == "constant" else 3e-13, "support_frame_error_upper_m": 1e-18,
         "edge_to_cylinder_gap_upper_m": 9e-13, "edge_to_blend_gap_upper_m": 3e-13, "cylinder_to_blend_gap_upper_m": 9e-13,
         "blend_normal_bounds": copy.deepcopy(case["patches"][next(r["chart"] for r in edge["roles"] if r["kind"] == 0)]["normal_bounds"])}
        for case, actual in zip(reference["cases"], native["cases"]) for edge in contact_edges(actual) if any(r["kind"] == 1 for r in edge["roles"])]}
    native["seam_boundary_proof_refusals"] = {key: True for key in SEAM_REFUSALS}
    native["seam_boundary_certificates"] = {"format_version": 1, "native_floating_evaluation_certified": False, "paired_normals_certified": False, "whole_brep_certified": False, "certificates": [
        {"request": copy.deepcopy(seam_boundary_request(actual, edge)), **{key: 1e-13 for key in SEAM_GAPS}}
        for actual in native["cases"] for edge in seam_edges(actual)]}
    native["boundary_normal_proof_refusals"] = {key: True for key in NORMAL_REFUSALS}
    paired = []
    for case, actual in zip(reference["cases"], native["cases"]):
        surfaces = []
        for patch in case["patches"]:
            normal = copy.deepcopy(patch["normal_bounds"]);normal["normal_error_upper"] = 1e-10
            surfaces.append({"position_error_upper_m": 5e-13, "normal_bounds": normal})
        paired.append({"request": copy.deepcopy(boundary_normal_request(reference, case, actual)), "surfaces": surfaces,
                       "cylindrical_lifts": [{"position_error_upper_m": 7e-13, "angle_error_upper_rad": 5e-11, "height_error_upper_m": 1e-16 if case["law"] == "constant" else 3e-13, "support_frame_error_upper_m": 1e-18} for _ in range(4)],
                       "plane_frame_normal_error_upper": 1e-18, "cylinder_frame_normal_error_upper": [1e-18]*4, "cylinder_normal_error_upper": [5.1e-11]*4,
                       "side_normal_gap_upper": [2e-10]*4, "end_normal_gap_upper": [1.1e-10]*4, "seam_normal_gap_upper": [2.1e-10]*4})
    native["boundary_normal_certificates"] = {"format_version": 1, "native_floating_evaluation_certified": False, "exact_g1_certified": False, "whole_brep_certified": False, "certificates": paired}
    return reference, native


class Contract(unittest.TestCase):
    def test_paired_normal_echo_frame_and_component_binding_refuse(self):
        ref, base = fixture()
        for key, value in (("max_leaves", True), ("maximum_normal_gap", 1e-8), ("maximum_surface_normal_error", 1e-9), ("plane_axes", [[0.,1.,0.],[1.,0.,0.]])):
            native = copy.deepcopy(base);native["boundary_normal_certificates"]["certificates"][0]["request"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["boundary_normal_certificates"]["certificates"][0]["request"]["blends"][0]["chart"] = False
        with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["boundary_normal_certificates"]["certificates"][0]["request"]["plane_axes"][0][1] = -0.0
        with self.assertRaises(ValueError): grade(ref, native)
        for key in ("plane_frame_normal_error_upper",):
            native = copy.deepcopy(base);native["boundary_normal_certificates"]["certificates"][0][key] = -1.
            with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base)
        plane = native["cases"][0]["surface_faces"][8];plane["plane_x_direction"][2] = 1e-7
        native["boundary_normal_certificates"]["certificates"][0]["request"]["plane_axes"][0][2] = 1e-7
        with self.assertRaisesRegex(ValueError,"underclaimed plane"): grade(ref, native)

    def test_paired_normal_inventories_bounds_outward_sums_and_scope_refuse(self):
        ref, base = fixture()
        for key in NORMAL_GAPS:
            for value in (0.,1e-8,True,float("nan"),1e-12):
                native = copy.deepcopy(base);native["boundary_normal_certificates"]["certificates"][0][key][0] = value
                with self.subTest(key=key,value=value), self.assertRaises(ValueError): grade(ref, native)
        for key in ("native_floating_evaluation_certified", "exact_g1_certified", "whole_brep_certified"):
            native = copy.deepcopy(base);native["boundary_normal_certificates"][key] = True
            with self.assertRaises(ValueError): grade(ref, native)
        for key in ("surfaces", "cylindrical_lifts", "cylinder_normal_error_upper", "seam_normal_gap_upper"):
            native = copy.deepcopy(base);native["boundary_normal_certificates"]["certificates"][0][key].pop()
            with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["boundary_normal_certificates"]["certificates"].pop()
        with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["boundary_normal_proof_refusals"]["last_request_precision"] = False
        with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["boundary_normal_certificates"]["certificates"][0]["surfaces"][0]["normal_bounds"]["normal_error_upper"] = 1e-8
        with self.assertRaises(ValueError): grade(ref, native)


    def test_current_seam_representation_lines_and_echo_binding_refuse(self):
        ref, base = fixture()
        changes = [("degree", True), ("degree", 3), ("kind", "geom_bspline_curve"), ("periodic", True), ("location_identity", False), ("weights", [1.,0.,1.]), ("poles_mm", [[0.,0.,0.]]*2)]
        for key, value in changes:
            native = copy.deepcopy(base);native["cases"][0]["shared_edges"][0]["seam_curve"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("origin", [1.,1e-16]), ("direction", [0.,-1.]), ("range", [0.,2.]), ("kind", "other"), ("origin", [True,0.])):
            native = copy.deepcopy(base);native["cases"][0]["shared_edges"][0]["seam_lines"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("left_chart", False), ("left_chart", 1), ("right_chart", 2), ("maximum_gap_m", 1e-10), ("edge_weights", [1.]*3)):
            native = copy.deepcopy(base);native["seam_boundary_certificates"]["certificates"][0]["request"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["seam_boundary_certificates"]["certificates"][0]["request"]["left_line"]["origin"][1] = -0.0
        with self.assertRaises(ValueError): grade(ref, native)

    def test_seam_owner_order_and_wrap_preserve_binding(self):
        ref, native = fixture()
        for case in native["cases"]:
            for edge in seam_edges(case):
                edge["roles"].reverse();edge["seam_lines"].reverse()
                for sample in edge["samples"]: sample["sides"].reverse()
        summary = grade(ref, native)["native_seam_boundary_certificates"]
        self.assertEqual(summary["count"], 8)
        self.assertFalse(summary["paired_normals_certified"])

    def test_missing_seam_evidence_overclaims_and_bounds_refuse(self):
        ref, base = fixture()
        for key in SEAM_GAPS:
            for value in (0.,1e-11,float("nan"),True):
                native = copy.deepcopy(base);native["seam_boundary_certificates"]["certificates"][0][key] = value
                with self.subTest(key=key,value=value), self.assertRaises(ValueError): grade(ref, native)
        for key in ("native_floating_evaluation_certified", "paired_normals_certified", "whole_brep_certified"):
            native = copy.deepcopy(base);native["seam_boundary_certificates"][key] = True
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["seam_boundary_certificates"]["certificates"].pop()
        with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["seam_boundary_proof_refusals"]["last_request_precision"] = False
        with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["cases"][0]["shared_edges"][0]["range"] = [0.,1.0000000000000002]
        with self.assertRaises(ValueError): grade(ref, native)


    def test_current_side_pcurve_frame_line_and_echo_binding_refuse(self):
        ref, base = fixture()
        for key, value in (("range", [0.,1.0000000000000002]), ("weights", [1.]*768), ("weights", [True]*769), ("poles_rad_mm", [[0.,0.]]*769)):
            native = copy.deepcopy(base);native["cases"][0]["shared_edges"][4]["cylindrical_pcurve"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["cases"][0]["shared_edges"][4]["cylindrical_pcurve"]["weights"][384] = 1.0000000000000002
        with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["cases"][0]["shared_edges"][4]["cylindrical_pcurve"]["knots"][0] = False
        with self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("origin", [0.,1e-16]), ("direction", [-1.,0.]), ("range", [0.,2.]), ("kind", "other")):
            native = copy.deepcopy(base);native["cases"][0]["shared_edges"][4]["blend_side_line"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("origin_mm", [0.,0.,0.001]), ("directions", [[0.,1.,0.],[1.,0.,0.],[0.,0.,1.]]), ("radius_mm", 10.001)):
            native = copy.deepcopy(base);native["cases"][0]["shared_edges"][4]["cylinder_frame"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("chart", False), ("chart", 1), ("max_leaves", True), ("maximum_gap_m", 1e-8), ("cylinder_radius_mm", 10.001)):
            native = copy.deepcopy(base);native["side_boundary_certificates"]["certificates"][0]["request"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for value in (False, -0.0, 0.001):
            native = copy.deepcopy(base);native["side_boundary_certificates"]["certificates"][0]["request"]["pcurve_poles_rad_mm"][0][0] = value
            with self.subTest(value=value), self.assertRaises(ValueError): grade(ref, native)

    def test_incomplete_side_component_bounds_gap_sums_and_overclaims_refuse(self):
        ref, base = fixture()
        for key in ("native_floating_evaluation_certified", "end_contact_boundaries_certified", "whole_brep_certified"):
            native = copy.deepcopy(base);native["side_boundary_certificates"][key] = True
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key in ("edge_position_error_upper_m", "cylindrical_lift_position_error_upper_m", "blend_position_error_upper_m", "edge_to_cylinder_gap_upper_m", "edge_to_blend_gap_upper_m", "cylinder_to_blend_gap_upper_m"):
            for value in (0., 1e-10, float("nan"), True):
                native = copy.deepcopy(base);native["side_boundary_certificates"]["certificates"][0][key] = value
                with self.subTest(key=key,value=value), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("angle_error_upper_rad", 1e-15), ("height_error_upper_m", 0.), ("support_frame_error_upper_m", -1.), ("support_frame_error_upper_m", 1e-10), ("edge_to_cylinder_gap_upper_m", 1e-13)):
            native = copy.deepcopy(base);native["side_boundary_certificates"]["certificates"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["side_boundary_certificates"]["certificates"].pop()
        with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);del native["side_boundary_certificates"]
        with self.assertRaises(KeyError): grade(ref, native)
        native = copy.deepcopy(base);native["side_boundary_proof_refusals"]["last_request_precision"] = False
        with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["side_boundary_certificates"]["certificates"][0]["blend_normal_bounds"]["leaves"] = 4097
        with self.assertRaises(ValueError): grade(ref, native)

    def test_current_end_pcurve_line_frame_and_echo_binding_refuse(self):
        ref, base = fixture()
        for key, value in (("degree", 3), ("periodic", True), ("range", [0, 1.0000000000000002]), ("knots", [False, True]), ("multiplicities", [4, 4]), ("weights", [0.]*5), ("poles_mm", [[0,0]]*4)):
            native = copy.deepcopy(base);native["cases"][0]["shared_edges"][8]["planar_pcurve"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("kind", "other"), ("origin", [0, 1.0000000000000002]), ("direction", [-1, 0]), ("range", [0, 2]), ("origin", [False, 1])):
            native = copy.deepcopy(base);native["cases"][0]["shared_edges"][8]["blend_end_line"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("plane_origin_mm", [0.,0.,21.]), ("plane_x_direction", [0.,1.,0.]), ("plane_y_direction", [1.,0.,0.])):
            native = copy.deepcopy(base);native["cases"][0]["surface_faces"][8][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("chart", False), ("chart", 1), ("max_leaves", True), ("max_leaves", 1), ("maximum_gap_m", 1e-8), ("plane_origin_mm", [0.,0.,21.]), ("blend_weights", [[1.]*3]*9)):
            native = copy.deepcopy(base);native["end_boundary_certificates"]["certificates"][0]["request"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for value in (False, -0.0, .001):
            native = copy.deepcopy(base);native["end_boundary_certificates"]["certificates"][0]["request"]["pcurve_poles_mm"][0][1] = value
            with self.subTest(value=value), self.assertRaises(ValueError): grade(ref, native)

    def test_incomplete_end_bounds_gap_sums_and_overclaims_refuse(self):
        ref, base = fixture()
        for key in ("native_floating_evaluation_certified", "side_contact_boundaries_certified", "whole_brep_certified"):
            native = copy.deepcopy(base);native["end_boundary_certificates"][key] = True
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key in ("edge_position_error_upper_m", "plane_lift_position_error_upper_m", "blend_position_error_upper_m", "edge_to_plane_gap_upper_m", "edge_to_blend_gap_upper_m", "plane_to_blend_gap_upper_m"):
            for value in (0., 1e-10, float("nan"), True):
                native = copy.deepcopy(base);native["end_boundary_certificates"]["certificates"][0][key] = value
                with self.subTest(key=key,value=value), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["end_boundary_certificates"]["certificates"][0]["edge_to_plane_gap_upper_m"] = 1e-13
        with self.assertRaisesRegex(ValueError, "outward component sum"): grade(ref, native)
        native = copy.deepcopy(base);native["end_boundary_certificates"]["certificates"].pop()
        with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);del native["end_boundary_certificates"]
        with self.assertRaises(KeyError): grade(ref, native)
        native = copy.deepcopy(base);native["end_boundary_proof_refusals"]["last_request_precision"] = False
        with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["end_boundary_certificates"]["certificates"][0]["blend_normal_bounds"]["leaves"] = 4097
        with self.assertRaises(ValueError): grade(ref, native)

    def test_current_contact_definition_and_transport_binding_refuse(self):
        ref, base = fixture()
        for key, value in (("location_identity", False), ("periodic", True), ("poles_mm", [[0.,0.,0.]]*3), ("knots", [False, True]), ("multiplicities", [True, True])):
            native = copy.deepcopy(base);native["cases"][0]["shared_edges"][4]["contact_curve"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["cases"][0]["shared_edges"][4]["range"][0] = 1e-16
        with self.assertRaisesRegex(ValueError, "domain unsupported"): grade(ref, native)
        for key, value in (("kind", "end"), ("chart", 1), ("chart", False), ("maximum_radius_m", .002), ("maximum_position_error_m", 1e-10)):
            native = copy.deepcopy(base);native["contact_definition_certificates"]["certificates"][0]["request"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for value in (False, -0.0, .001):
            native = copy.deepcopy(base);native["contact_definition_certificates"]["certificates"][0]["request"]["poles_mm"][0][1] = value
            with self.subTest(value=value), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["contact_definition_certificates"]["certificates"][0]["request"]["weights"][1] *= 2
        with self.assertRaises(ValueError): grade(ref, native)

    def test_missing_unresolved_or_overclaimed_contact_certificates_refuse(self):
        ref, base = fixture()
        for key in ("native_floating_evaluation_certified", "attached_pcurve_lifts_certified", "whole_brep_certified"):
            native = copy.deepcopy(base);native["contact_definition_certificates"][key] = True
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for value in (0., 1e-10, float("nan"), True):
            native = copy.deepcopy(base);native["contact_definition_certificates"]["certificates"][0]["position_error_upper_m"] = value
            with self.subTest(value=value), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["contact_definition_certificates"]["certificates"].pop()
        with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);del native["contact_definition_certificates"]
        with self.assertRaises(KeyError): grade(ref, native)
        native = copy.deepcopy(base);native["contact_definition_proof_refusals"]["last_request_precision"] = False
        with self.assertRaises(ValueError): grade(ref, native)

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

    def test_raw_native_definition_and_snapshot_binding_refuse(self):
        ref, base = fixture()
        for key, value in (("location_identity", False), ("poles_mm", [[[0., 0., 0.]]*3]*9), ("u_bounds", [0, 2]), ("v_bounds", [0, 2]), ("u_periodic", True), ("v_periodic", True)):
            native = copy.deepcopy(base);native["cases"][0]["surface_faces"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        for key, value in (("chart", 1), ("cylinder_radius_m", .02), ("maximum_position_error_m", 1e-10), ("maximum_normal_error", 1e-7), ("max_leaves", 8192)):
            native = copy.deepcopy(base);native["surface_definition_certificates"]["certificates"][0]["request"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["surface_definition_certificates"]["certificates"][0]["request"]["weights"][4][1] *= 2
        with self.assertRaises(ValueError): grade(ref, native)

        for value in (False, -0.0):
            native = copy.deepcopy(base);native["surface_definition_certificates"]["certificates"][0]["request"]["poles_mm"][0][0][1] = value
            with self.subTest(echo=value), self.assertRaises(ValueError): grade(ref, native)

    def test_missing_unresolved_or_overclaimed_native_certificates_refuse(self):
        ref, base = fixture()
        native = copy.deepcopy(base);native["surface_definition_proof_refusals"]["last_request_precision"] = False
        with self.assertRaises(ValueError): grade(ref, native)
        for key in ("native_floating_evaluation_certified", "whole_brep_certified"):
            native = copy.deepcopy(base);native["surface_definition_certificates"][key] = True
            with self.subTest(key=key), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["surface_definition_certificates"]["certificates"].pop()
        with self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);del native["surface_definition_certificates"]
        with self.assertRaises(KeyError): grade(ref, native)
        for value in (0., 1e-10, float("nan")):
            native = copy.deepcopy(base);native["surface_definition_certificates"]["certificates"][0]["position_error_upper_m"] = value
            with self.subTest(value=value), self.assertRaises(ValueError): grade(ref, native)
        native = copy.deepcopy(base);native["surface_definition_certificates"]["certificates"][0]["normal_bounds"]["jacobian_area_upper_m2"] = 2e-6
        with self.assertRaisesRegex(ValueError, "captured surface-definition"): grade(ref, native)

    def test_coarse_pcurve_inside_old_join_allowance_fails_complete_side_gap(self):
        ref, native = fixture()
        edge = native["cases"][0]["shared_edges"][4]
        # A 2e-9 rad angular offset is only 2e-11 m on this cylinder, within
        # the old 1e-9 m join allowance. Update raw capture and echo too: the
        # stronger complete-gap witness gate must reject this before the old
        # tangent-plane gate. Synthetic bounds are deliberately underclaimed;
        # the live Rust proof refuses changed angular controls before output.
        for control in edge["cylindrical_pcurve"]["poles_rad_m"]: control[0] += 2e-9
        ref["cases"][0]["side_pcurves"][0]["control_points_rad_m"] = copy.deepcopy(edge["cylindrical_pcurve"]["poles_rad_m"])
        edge["cylindrical_pcurve"]["poles_rad_mm"] = [[p[0],p[1]*1000] for p in edge["cylindrical_pcurve"]["poles_rad_m"]]
        native["side_boundary_certificates"]["certificates"][0]["request"] = copy.deepcopy(side_boundary_request(ref,ref["cases"][0],native["cases"][0],edge))
        native["boundary_normal_certificates"]["certificates"][0]["request"] = copy.deepcopy(boundary_normal_request(ref,ref["cases"][0],native["cases"][0]))
        for sample in edge["samples"]:
            side = sample["sides"][1]
            u, v = cubic_pcurve(edge["cylindrical_pcurve"]["poles_rad_m"], sample["t"])
            side.update(uv=[u, v], point_m=[.01*math.cos(u), .01*math.sin(u), v], normal=[math.cos(u), math.sin(u), 0.])
        with self.assertRaisesRegex(ValueError, "outside whole side boundary"):
            grade(ref, native)


if __name__ == "__main__": unittest.main()
