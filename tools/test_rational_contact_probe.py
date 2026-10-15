"""Synthetic report-contract controls only; live OCCT construction runs separately."""
import copy
from fractions import Fraction as F
import math
import unittest
from run_rational_contact_probe import grade, rational


def fixture():
    reference = {"format_version": 1, "radius_m": 0.01, "height_m": 0.02, "curves": []}
    native = {"format_version": 1, "occt_version": "8.0.0", "curves": [], "wires": []}
    def product(a, b):
        result = [F(0)]*(len(a)+len(b)-1)
        for i, x in enumerate(a):
            for j, y in enumerate(b): result[i+j] += x*y
        return result
    def subtract(a, b): return [x-y for x, y in zip(a, b)]
    def scale(a, k): return [x*k for x in a]
    def bernstein(a, n):
        return [sum((a[k]*F(math.comb(i, k), math.comb(n, k)) for k in range(i+1)), F(0)) for i in range(n+1)]
    for law, maximum in (("constant", 0.0005), ("variable", 0.0015)):
        r, h, minimum, maximum_f = map(F, (0.01, 0.02, 0.0005, maximum))
        m, a = (minimum+maximum_f)/2, (maximum_f-minimum)/2
        for kind in ("side", "end"):
            for chart in range(4):
                c, s, d = [F(1), F(0), F(-1)], [F(0), F(2), F(0)], [F(1), F(0), F(1)]
                if chart == 1: c, s = scale(s, -1), c
                elif chart == 2: c, s = scale(c, -1), scale(s, -1)
                elif chart == 3: c, s = s, scale(c, -1)
                degree = 2 if kind == "side" else 4
                weights = [1., 1., 2.] if kind == "side" else [1., 1., 4/3, 2., 4.]
                if kind == "side": nums = [scale(c, r), scale(s, r), subtract(scale(d, h-m), scale(c, a))]
                else: nums = [subtract(scale(product(c, d), r-m), scale(product(c, c), a)), subtract(scale(product(s, d), r-m), scale(product(c, s), a)), scale(product(d, d), h)]
                controls = [bernstein(num, degree) for num in nums]
                poles = [[float(controls[k][i]/F(weights[i])) for k in range(3)] for i in range(degree+1)]
                def rotate(x, y, q): return ([x, y], [-y, x], [-x, -y], [y, -x])[q]
                if kind == "side":
                    for pole, (x, y) in zip(poles, [(0.01, 0), (0.01, 0.01), (0, 0.01)]): pole[:2] = rotate(x, y, chart)
                else:
                    for pole in poles: pole[2] = 0.02
                radii = [maximum, 0.0005+(maximum-0.0005)/2, 0.0005, 0.0005+(maximum-0.0005)/2]
                for index, q in ((0, chart), (degree, (chart+1)%4)):
                    poles[index] = [*rotate(0.01 if kind == "side" else 0.01-radii[q], 0., q), 0.02-radii[q] if kind == "side" else 0.02]
                ref = {"law": law, "kind": kind, "chart": chart, "degree": degree, "control_points_m": poles,
                       "weights": weights, "position_error_upper_m": 1e-14, "minimum_radius_m": 0.0005, "maximum_radius_m": maximum}
                actual = {"degree": degree, "rational": True, "edge_valid": True, "poles_m": copy.deepcopy(poles), "weights": weights[:],
                          "knots": [0, 1], "multiplicities": [degree+1]*2, "samples": [], "rotated_samples": []}
                for i in range(9):
                    t = i/8
                    p, v = rational(poles, weights, t)
                    actual["samples"].append({"u": t, "point_m": p, "tangent_m": v})
                    actual["rotated_samples"].append({"u": t, "point_m": [-p[1], p[0], p[2]], "tangent_m": [-v[1], v[0], v[2]]})
                reference["curves"].append(ref)
                native["curves"].append(actual)
    native["wires"] = [{"valid": True, "closed": True, "edges": 4, "vertices": 4, "length_m": 0.06, "rotated_length_m": 0.06} for _ in range(4)]
    return reference, native


class Contract(unittest.TestCase):
    def test_complete_report_keeps_body_and_feature_unsupported(self):
        result = grade(*fixture())
        self.assertTrue(result["diagnostic_passed"])
        self.assertFalse(result["candidate_admitted"])
        self.assertFalse(result["product_body_created"])
        self.assertEqual(result["native_contact_edges_created"], 16)
        self.assertEqual(result["native_closed_contact_wires_created"], 4)
        self.assertLess(result["rotated_constant_law_error_m"], 1e-14)
        self.assertGreater(result["rotated_variable_law_error_m"], 1e-4)

    def test_invalid_native_representation_and_missing_curves_refuse(self):
        for error in ("rational", "degree", "weight", "poles", "knots", "inventory"):
            ref, native = fixture()
            c = native["curves"][0]
            if error == "rational": c["rational"] = False
            elif error == "degree": c["degree"] = 4
            elif error == "weight": c["weights"][0] = 0
            elif error == "poles": c["poles_m"].pop()
            elif error == "knots": c["knots"] = [0, 2]
            else: native["curves"].pop()
            with self.assertRaises(ValueError): grade(ref, native)

    def test_missing_corrupted_or_nonfinite_native_values_refuse(self):
        for error in ("missing", "point", "derivative", "rotated_derivative", "nan", "rotation"):
            ref, native = fixture()
            c = native["curves"][8]
            if error == "missing": c["samples"].pop()
            elif error == "point": c["samples"][1]["point_m"][0] += 1e-5
            elif error == "derivative": c["samples"][2]["tangent_m"][1] += 1e-5
            elif error == "rotated_derivative": c["rotated_samples"][2]["tangent_m"][1] += 1e-5
            elif error == "nan": c["samples"][0]["point_m"][0] = float("nan")
            else: c["rotated_samples"] = copy.deepcopy(c["samples"])
            with self.assertRaises(ValueError): grade(ref, native)

    def test_unclosed_or_wrong_native_wire_and_length_controls_refuse(self):
        for field, value in (("valid", False), ("closed", False), ("edges", 3), ("vertices", 3), ("rotated_length_m", 0.07), ("length_m", 0)):
            ref, native = fixture()
            native["wires"][0][field] = value
            with self.assertRaises(ValueError): grade(ref, native)

    def test_wrong_phase_reference_or_unresolved_bound_refuses(self):
        for error in ("chart", "law", "bound", "endpoint", "radius"):
            ref, native = fixture()
            c = ref["curves"][8]
            if error == "chart": c["chart"] = 1
            elif error == "law": c["law"] = "constant"
            elif error == "bound": c["position_error_upper_m"] = 1e-3
            elif error == "endpoint": c["control_points_m"][-1][2] += 1e-5
            else: c["maximum_radius_m"] = 0.002
            with self.assertRaises(ValueError): grade(ref, native)


if __name__ == "__main__": unittest.main()
