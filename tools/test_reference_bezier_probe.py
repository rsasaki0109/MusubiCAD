"""Report-contract tests using an analytic plane; not native execution evidence."""
import copy
import unittest

from run_reference_bezier_probe import NAMES, grade


def fixture():
    reference = {"format_version": 1, "cases": []}
    native = {"format_version": 1, "occt_version": "8.0.0", "cases": []}
    for name in sorted(NAMES):
        # Analytic affine plane, independent of the production pole constructor.
        offset = -1. if name.endswith("seam-end") else 0.
        poles = [[[0.001*(i/3+offset), 0.001*j/3, 0.02] for j in range(4)] for i in range(4)]
        ref = {"name": name, "control_points_m": poles, "position_error_upper_m": 2e-8,
               "jacobian_area_lower_m2": 0.999999e-6, "jacobian_area_upper_m2": 1.000001e-6,
               "ideal_normal_alignment_lower": 1-1e-14, "normal_error_upper": 1e-8,
               "normal_lower": [0., 0., 1-1e-14], "normal_upper": [0., 0., 1+1e-14],
               "reference_to_bilinear_error_upper_m": 1e-8, "control_net_deviation_upper_m": 1e-8, "samples": []}
        actual = {"u_degree": 3, "v_degree": 3, "poles_m": copy.deepcopy(poles), "samples": []}
        for i in range(5):
            for j in range(5):
                u, v = i/4, j/4
                p = [0.001*(u+offset), 0.001*v, 0.02]
                ref["samples"].append({"u": u, "v": v, "reference_point_m": p.copy(), "reference_normal": [0., 0., 1.]})
                actual["samples"].append({"u": u, "v": v, "point_m": p.copy(), "du_m": [0.001, 0., 0.], "dv_m": [0., 0.001, 0.], "normal": [0., 0., 1.], "jacobian_area_m2": 1e-6})
        reference["cases"].append(ref)
        native["cases"].append(actual)
    return reference, native


class Contract(unittest.TestCase):
    def test_analytic_plane_passes_without_feature_admission(self):
        report = grade(*fixture())
        self.assertTrue(report["diagnostic_passed"])
        self.assertFalse(report["candidate_admitted"])
        self.assertEqual(len(report["findings"]), 40)

    def test_missing_or_duplicate_cases_and_wrong_sdk_refuse(self):
        for mutation in ("missing", "duplicate", "sdk"):
            ref, native = fixture()
            if mutation == "missing":
                native["cases"].pop()
            elif mutation == "duplicate":
                ref["cases"][0]["name"] = ref["cases"][1]["name"]
            else:
                native["occt_version"] = "7.8.0"
            with self.assertRaises(ValueError):
                grade(ref, native)

    def test_invalid_or_insufficient_bound_refuses(self):
        for bound in (float("nan"), float("inf"), True, -1., 1e-9, 1.):
            ref, native = fixture()
            ref["cases"][0]["position_error_upper_m"] = bound
            with self.assertRaises(ValueError):
                grade(ref, native)

    def test_corrupted_pole_evaluation_derivative_and_parameter_refuse(self):
        for field in ("pole", "point_m", "du_m", "dv_m", "parameter", "nonfinite"):
            ref, native = fixture()
            row = native["cases"][0]
            if field == "pole":
                row["poles_m"][1][1][0] += 1e-4
            elif field == "parameter":
                row["samples"][0]["u"] = 0.5
            elif field == "nonfinite":
                row["samples"][0]["point_m"][0] = float("nan")
            else:
                row["samples"][0][field][0] += 1e-4
            with self.assertRaises(ValueError):
                grade(ref, native)

    def test_coherent_displaced_native_surface_fails_reference_bound(self):
        ref, native = fixture()
        for poles in (ref["cases"][0]["control_points_m"], native["cases"][0]["poles_m"]):
            for row in poles:
                for p in row:
                    p[2] += 1e-4
        for sample in native["cases"][0]["samples"]:
            sample["point_m"][2] += 1e-4
        with self.assertRaisesRegex(ValueError, "outside reference bound"):
            grade(ref, native)

    def test_invalid_normal_certificate_refuses(self):
        for field, value in (("jacobian_area_lower_m2", 0.), ("jacobian_area_upper_m2", 1e-9), ("ideal_normal_alignment_lower", -1.), ("normal_error_upper", float("nan")), ("normal_error_upper", -0.1), ("normal_lower", [0., 0., 2.])):
            ref, native = fixture()
            ref["cases"][0][field] = value
            with self.assertRaises(ValueError):
                grade(ref, native)

    def test_false_native_normal_area_and_orientation_refuse(self):
        for corruption in ("normal", "area", "orientation"):
            ref, native = fixture()
            if corruption == "normal":
                native["cases"][0]["samples"][0]["normal"] = [0., 0., -1.]
            elif corruption == "area":
                native["cases"][0]["samples"][0]["jacobian_area_m2"] = 1e-4
            else:
                ref["cases"][0]["samples"][0]["reference_normal"] = [0., 0., -1.]
            with self.assertRaises(ValueError):
                grade(ref, native)


if __name__ == "__main__":
    unittest.main()
