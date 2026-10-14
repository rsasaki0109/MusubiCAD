"""Report-contract tests only: synthetic values are not native geometry evidence."""
import copy
import unittest
from run_periodic_fillet_probe import grade, ERRORS


def synthetic_reports():
    reference = {
        "format_version": 1, "radius_m": 0.01, "height_m": 0.02, "absolute_width_m3": 1e-12,
        "references": {
            "constant": {"minimum_radius_m": 0.001, "maximum_radius_m": 0.001, "body_lower_m3": 6.2699999e-6, "body_upper_m3": 6.2700001e-6, "cells": 0},
            "variable": {"minimum_radius_m": 0.0005, "maximum_radius_m": 0.0015, "body_lower_m3": 6.2684362e-6, "body_upper_m3": 6.2684368e-6, "cells": 1000},
        },
    }
    profiles = []
    for name in ("constant", "baseline", "dense", "tight", "c2", "wrong_phase"):
        constant = name == "constant"
        row = {"name": name, "brep_valid": True, "solids": 1, "top_points": 257, "side_points": 257, "interior_points": 585,
               "native_volume_m3": 6.27e-6 if constant else 6.268441e-6,
               "volume_refinement_delta_m3": 1e-20, "law_points": 0 if constant else 513, "max_sampled_law_error_m": 1e-10}
        row.update({key: 1e-17 if constant else 1e-6 for key in ERRORS})
        if name == "wrong_phase":
            row["max_sampled_top_error_m"] = row["max_sampled_side_error_m"] = 0.0007
        profiles.append(row)
    return {"format_version": 1, "occt_version": "8.0.0", "radius_m": 0.01, "height_m": 0.02, "profiles": profiles}, reference


class ReportContract(unittest.TestCase):
    def assert_rejected(self, native, reference):
        with self.assertRaises((ValueError, KeyError, TypeError)):
            grade(native, reference)

    def test_complete_controls_report_rejection_not_admission(self):
        verdict = grade(*synthetic_reports())
        self.assertTrue(verdict["diagnostic_passed"])
        self.assertFalse(verdict["candidate_admitted"])

    def test_nonfinite_boolean_wrong_units_and_invalid_brep_refuse(self):
        for field, value in (("native_volume_m3", float("nan")), ("max_sampled_top_error_m", float("inf")), ("solids", True), ("brep_valid", False), ("native_volume_m3", 6268.441)):
            with self.subTest(field=field, value=value):
                native, reference = synthetic_reports()
                native["profiles"][1][field] = value
                self.assert_rejected(native, reference)
        native, reference = synthetic_reports()
        native["radius_m"] = 10
        self.assert_rejected(native, reference)
        native, reference = synthetic_reports()
        for row in native["profiles"][1:]:
            row["native_volume_m3"] *= 1e9
        self.assert_rejected(native, reference)
        native, reference = synthetic_reports()
        native["format_version"] = True
        self.assert_rejected(native, reference)

    def test_missing_duplicate_and_empty_measurements_refuse(self):
        for mutation in (lambda rows: rows.pop(), lambda rows: rows.append(copy.deepcopy(rows[0])), lambda rows: rows[1].update(name="constant"), lambda rows: rows[1].update(top_points=0), lambda rows: rows[1].update(interior_points=0), lambda rows: rows[1].pop("max_sampled_support_error_m")):
            native, reference = synthetic_reports()
            mutation(native["profiles"])
            self.assert_rejected(native, reference)

    def test_constant_control_and_mass_only_or_law_only_claims_refuse(self):
        for mutation in (lambda rows: rows[0].update(max_sampled_top_error_m=1e-6), lambda rows: rows[0].update(native_volume_m3=6.26e-6), lambda rows: rows[1].update(native_volume_m3=6.2684365e-6), lambda rows: rows[1].update(max_sampled_top_error_m=0.), lambda rows: rows[1].update(max_sampled_law_error_m=1e-4), lambda rows: rows[1].update(volume_refinement_delta_m3=1e-10)):
            native, reference = synthetic_reports()
            mutation(native["profiles"])
            self.assert_rejected(native, reference)

    def test_wrong_phase_requires_equal_volume_and_changed_physical_contacts(self):
        for mutation in (lambda row: row.update(native_volume_m3=6.2e-6), lambda row: row.update(max_sampled_top_error_m=1e-6), lambda row: row.update(max_sampled_side_error_m=1e-6)):
            native, reference = synthetic_reports()
            mutation(native["profiles"][-1])
            self.assert_rejected(native, reference)

    def test_reference_must_be_bounded_and_genuinely_nonconstant(self):
        for mutation in (lambda ref: ref.update(body_upper_m3=6.3e-6), lambda ref: ref.update(body_upper_m3=6.2e-6), lambda ref: ref.update(cells=0), lambda ref: ref.update(minimum_radius_m=0.001), lambda ref: ref.update(body_lower_m3=float("nan"))):
            native, reference = synthetic_reports()
            mutation(reference["references"]["variable"])
            self.assert_rejected(native, reference)


if __name__ == "__main__":
    unittest.main()
