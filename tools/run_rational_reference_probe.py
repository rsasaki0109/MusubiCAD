"""ADR-084/085: actual rational envelope solids; no production geometry admission."""
from __future__ import annotations
import argparse
import copy
from collections import Counter
import json
import math
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from run_reference_bezier_probe import ROOT, distance, number, require, unit_cross, vector
from run_rational_contact_probe import rational

POINT_M = 1e-14
D1_M = 1e-12  # Metres per dimensionless local parameter.
NORMAL = 1e-9
JOIN_M = 1e-9  # Native numerical gates, not certified global native errors.
VOLUME_M3 = 1e-12
JACOBIAN_M2 = 1e-16  # Separate numerical witness allowance, not a native certificate.


def tensor(poles, weights, u, v):
    """Independent homogeneous Bernstein value and quotient-rule D1."""
    def basis(n, i, t): return math.comb(n, i)*t**i*(1-t)**(n-i)
    def derivative(n, i, t):
        return n*((basis(n-1, i-1, t) if i > 0 else 0.)-(basis(n-1, i, t) if i < n else 0.))
    h = [[0.]*4 for _ in range(3)]
    for i in range(9):
        for j in range(3):
            factors = [basis(8, i, u)*basis(2, j, v), derivative(8, i, u)*basis(2, j, v), basis(8, i, u)*derivative(2, j, v)]
            for k in range(3):
                for axis, value in enumerate([*poles[i][j], 1.]): h[k][axis] += factors[k]*weights[i][j]*value
    require(h[0][3] > 0 and all(math.isfinite(x) for row in h for x in row), "invalid homogeneous surface evaluation")
    p = [h[0][k]/h[0][3] for k in range(3)]
    return p, [(h[1][k]-p[k]*h[1][3])/h[0][3] for k in range(3)], [(h[2][k]-p[k]*h[2][3])/h[0][3] for k in range(3)]


def envelope(law, theta, alpha):
    """ADR-072 square-root reference, independently of tensor construction."""
    r = .0005 if law == 0 else .001+.0005*math.cos(theta)
    k = 0. if law == 0 else -.0005*math.sin(theta)/(.01-r)
    q = math.cos(alpha)+math.sin(alpha)
    factor = (k*k*q+math.sqrt(1+k*k*(q*q-1)))/(1+k*k*q*q)
    nr, nz, nt = factor*math.cos(alpha), factor*math.sin(alpha), k*(factor*q-1)
    normal = [nr*math.cos(theta)-nt*math.sin(theta), nr*math.sin(theta)+nt*math.cos(theta), nz]
    center = [(.01-r)*math.cos(theta), (.01-r)*math.sin(theta), .02-r]
    return [center[k]+r*normal[k] for k in range(3)], normal


def edge_fraction(j): return 0. if j == 0 else 1. if j == 8 else (j+.37)/9.


def cubic_pcurve(poles, t):
    require(0 <= t <= 1, "pcurve parameter outside contact range")
    span = min(255, int(t*256));local = t*256-span
    return [sum(math.comb(3, j)*local**j*(1-local)**(3-j)*poles[3*span+j][k] for j in range(4)) for k in range(2)]


def controls(actual, ref, degree):
    require(type(actual["degree"]) is int and actual["degree"] == degree and actual["knots"] == [0, 1] and actual["multiplicities"] == [degree+1]*2, "current contact representation changed")
    require(actual["location_identity"] is True and actual["periodic"] is False and all(type(x) is int for x in actual["multiplicities"]) and all(type(x) is not bool for x in actual["knots"]), "placed/periodic/invalid current contact representation")
    require(len(actual["poles_m"]) == len(actual["weights"]) == len(ref["control_points_m"]) == len(ref["weights"]) == degree+1, "incomplete current contact controls")
    require(all(number(w) > 0 for w in actual["weights"]+ref["weights"]), "nonpositive contact weight")
    for p, q, w, z in zip(actual["poles_m"], ref["control_points_m"], actual["weights"], ref["weights"]):
        require(distance(p, q) <= POINT_M and abs(w/actual["weights"][0]-z/ref["weights"][0]) <= 1e-13, "sewing altered supplied contact controls")


def pcurve(actual, ref, chart, law):
    require(type(ref["chart"]) is int and ref["chart"] == chart and type(ref["divisions"]) is int and ref["divisions"] == 256, "wrong bounded pcurve chart/work")
    angle, height, lift = [number(ref[k]) for k in ("angle_error_upper_rad", "height_error_upper_m", "position_error_upper_m")]
    require(48/256**4/384 <= angle <= 1e-9 and 0 < height <= 1e-12 and (768*.0005/256**4/384 if law else 0.) <= height and math.hypot(.01*angle, height)-1e-27 <= lift <= 1e-12, "unresolved pcurve interpolation/rounding bound")
    require(type(actual["degree"]) is int and actual["degree"] == 3 and actual["rational"] is False and actual["periodic"] is False and actual["knots"] == [i/256 for i in range(257)] and actual["multiplicities"] == [4]+[3]*255+[4], "bounded native pcurve parameterization changed")
    require(len(actual["poles_rad_m"]) == len(ref["control_points_rad_m"]) == 769, "bounded pcurve controls missing")
    for p, q in zip(actual["poles_rad_m"], ref["control_points_rad_m"]):
        require(len(p) == len(q) == 2 and all(abs(number(x)-number(y)) <= 1e-14 for x, y in zip(p, q)), "current bounded pcurve controls changed")


def same_numeric_data(actual, expected):
    """Exact binary64 transport identity, including zero sign; bool is not numeric."""
    if isinstance(expected, dict):
        return isinstance(actual, dict) and set(actual) == set(expected) and all(same_numeric_data(actual[k], v) for k, v in expected.items())
    if isinstance(expected, str):
        return type(actual) is str and actual == expected
    if isinstance(expected, list):
        return isinstance(actual, list) and len(actual) == len(expected) and all(same_numeric_data(a, b) for a, b in zip(actual, expected))
    return number(actual).hex() == number(expected).hex()


def snapshot_request(reference, case, face):
    """Bind a read-only proof to CURRENT raw native data, not supplied poles."""
    return {"chart": face["role"]["chart"], "cylinder_radius_m": reference["radius_m"],
            "cylinder_height_m": reference["height_m"], "minimum_radius_m": case["minimum_radius_m"],
            "maximum_radius_m": case["maximum_radius_m"], "poles_mm": face["poles_mm"],
            "weights": face["weights"], "maximum_position_error_m": 1e-12,
            "maximum_normal_error": 1e-9, "max_leaves": 4096}


def contact_request(reference, case, edge):
    roles = edge["roles"]
    return {"kind": "side" if any(r["kind"] == 1 for r in roles) else "end",
            "chart": next(r["chart"] for r in roles if r["kind"] == 0),
            "cylinder_radius_m": reference["radius_m"], "cylinder_height_m": reference["height_m"],
            "minimum_radius_m": case["minimum_radius_m"], "maximum_radius_m": case["maximum_radius_m"],
            "poles_mm": edge["contact_curve"]["poles_mm"], "weights": edge["contact_curve"]["weights"],
            "maximum_position_error_m": 1e-12}


def contact_edges(case):
    return sorted((edge for edge in case["shared_edges"] if "contact_curve" in edge),
                  key=lambda edge: (0 if any(r["kind"] == 1 for r in edge["roles"]) else 1,
                                    next(r["chart"] for r in edge["roles"] if r["kind"] == 0)))


def end_boundary_request(reference, case, actual, edge):
    base = contact_request(reference, case, edge)
    blend = next(f for f in actual["surface_faces"] if f["role"] == {"kind": 0, "chart": base["chart"]})
    plane = next(f for f in actual["surface_faces"] if f["role"]["kind"] == 2)
    return {key: base[key] for key in ("chart", "cylinder_radius_m", "cylinder_height_m", "minimum_radius_m", "maximum_radius_m")} | {
        "edge_poles_mm": base["poles_mm"], "edge_weights": base["weights"],
        "plane_origin_mm": plane["plane_origin_mm"], "plane_x_direction": plane["plane_x_direction"], "plane_y_direction": plane["plane_y_direction"],
        "pcurve_poles_mm": edge["planar_pcurve"]["poles_mm"], "pcurve_weights": edge["planar_pcurve"]["weights"],
        "blend_poles_mm": blend["poles_mm"], "blend_weights": blend["weights"], "blend_line": edge["blend_end_line"],
        "maximum_gap_m": 1e-12, "maximum_blend_normal_error": 1e-9, "max_leaves": 4096}


def grade_end_boundary(reference, case, actual, edge, proof):
    pc, line = edge["planar_pcurve"], edge["blend_end_line"]
    require(type(pc["degree"]) is int and pc["degree"] == 4 and pc["periodic"] is False and len(pc["poles_mm"]) == len(pc["weights"]) == 5 and pc["knots"] == [0, 1] and all(type(k) is not bool for k in pc["knots"]) and pc["multiplicities"] == [5, 5] and all(type(m) is int for m in pc["multiplicities"]), "unsupported planar contact representation")
    require(pc["range"] == [0, 1] and all(type(x) is not bool and math.isfinite(number(x)) for x in pc["range"]), "unsupported planar contact range")
    require(set(line) == {"kind", "origin", "direction", "range"} and line["kind"] == "geom2d_line" and line["origin"] == [0, 1] and line["direction"] == [1, 0] and line["range"] == [0, 1] and all(type(x) is not bool and math.isfinite(number(x)) for key in ("origin", "direction", "range") for x in line[key]), "unsupported attached blend end line")
    for p in pc["poles_mm"]: require(len(p) == 2 and all(math.isfinite(number(x)) for x in p), "invalid planar pcurve controls")
    require(all(number(w) > 0 for w in pc["weights"]), "nonpositive planar contact weight")
    expected = end_boundary_request(reference, case, actual, edge)
    for key in ("plane_origin_mm", "plane_x_direction", "plane_y_direction"): vector(expected[key])
    echo = proof["request"]
    require(same_numeric_data(echo, expected) and type(echo["chart"]) is int and type(echo["max_leaves"]) is int, "end boundary capture/reference/request mismatch")
    components = [number(proof[k]) for k in ("edge_position_error_upper_m", "plane_lift_position_error_upper_m", "blend_position_error_upper_m")]
    gaps = [number(proof[k]) for k in ("edge_to_plane_gap_upper_m", "edge_to_blend_gap_upper_m", "plane_to_blend_gap_upper_m")]
    require(all(0 < x <= expected["maximum_gap_m"] for x in components+gaps), "unresolved complete end boundary precision")
    e, p, b = components
    require(all(g >= math.nextafter(x+y, math.inf) for g, (x, y) in zip(gaps, ((e, p), (e, b), (p, b)))), "end gap does not enclose outward component sum")
    normal_certificate(proof["blend_normal_bounds"], expected["maximum_blend_normal_error"], expected["max_leaves"])
    return proof


def side_boundary_request(reference, case, actual, edge):
    base = contact_request(reference, case, edge)
    blend = next(f for f in actual["surface_faces"] if f["role"] == {"kind": 0, "chart": base["chart"]})
    cylinder = edge["cylinder_frame"]
    return {key: base[key] for key in ("chart", "cylinder_radius_m", "cylinder_height_m", "minimum_radius_m", "maximum_radius_m")} | {
        "edge_poles_mm": base["poles_mm"], "edge_weights": base["weights"],
        "cylinder_origin_mm": cylinder["origin_mm"], "cylinder_directions": cylinder["directions"], "cylinder_radius_mm": cylinder["radius_mm"],
        "pcurve_poles_rad_mm": edge["cylindrical_pcurve"]["poles_rad_mm"], "pcurve_weights": edge["cylindrical_pcurve"]["weights"],
        "blend_poles_mm": blend["poles_mm"], "blend_weights": blend["weights"], "blend_line": edge["blend_side_line"],
        "maximum_gap_m": 1e-12, "maximum_blend_normal_error": 1e-9, "max_leaves": 4096}


def grade_side_boundary(reference, case, actual, edge, proof):
    pc, line = edge["cylindrical_pcurve"], edge["blend_side_line"]
    # Degree/knots/multiplicities/nonperiodicity are also required by pcurve().
    require(all(type(k) is not bool for k in pc["knots"]) and all(type(m) is int for m in pc["multiplicities"]), "invalid cylindrical basis metadata types")
    require(pc["range"] == [0, 1] and all(type(x) is not bool and math.isfinite(number(x)) for x in pc["range"]), "unsupported cylindrical pcurve range")
    require(len(pc["weights"]) == len(pc["poles_rad_mm"]) == 769 and all(number(w) == 1 for w in pc["weights"]), "unsupported cylindrical pcurve weight representation")
    for raw, si in zip(pc["poles_rad_mm"], pc["poles_rad_m"]):
        require(len(raw) == len(si) == 2 and abs(number(raw[0])-number(si[0])) <= 1e-14 and abs(number(raw[1])/1000-number(si[1])) <= POINT_M, "raw cylindrical controls disagree with SI witnesses")
    require(set(line) == {"kind", "origin", "direction", "range"} and line["kind"] == "geom2d_line" and line["origin"] == [0, 0] and line["direction"] == [1, 0] and line["range"] == [0, 1] and all(type(x) is not bool and math.isfinite(number(x)) for key in ("origin", "direction", "range") for x in line[key]), "unsupported attached blend side line")
    expected = side_boundary_request(reference, case, actual, edge)
    vector(expected["cylinder_origin_mm"])
    require(len(expected["cylinder_directions"]) == 3 and number(expected["cylinder_radius_mm"]) > 0, "invalid actual cylinder frame")
    for d in expected["cylinder_directions"]: vector(d)
    echo = proof["request"]
    require(same_numeric_data(echo, expected) and type(echo["chart"]) is int and type(echo["max_leaves"]) is int, "side boundary capture/reference/request mismatch")
    components = [number(proof[k]) for k in ("edge_position_error_upper_m", "cylindrical_lift_position_error_upper_m", "blend_position_error_upper_m")]
    gaps = [number(proof[k]) for k in ("edge_to_cylinder_gap_upper_m", "edge_to_blend_gap_upper_m", "cylinder_to_blend_gap_upper_m")]
    require(all(0 < x <= expected["maximum_gap_m"] for x in components+gaps), "unresolved complete side boundary precision")
    e, p, b = components
    require(all(g >= math.nextafter(x+y, math.inf) for g, (x, y) in zip(gaps, ((e, p), (e, b), (p, b)))), "side gap does not enclose outward component sum")
    angle, height, frame = [number(proof[k]) for k in ("angle_error_upper_rad", "height_error_upper_m", "support_frame_error_upper_m")]
    supplied = case["side_pcurves"][expected["chart"]]
    require(angle >= number(supplied["angle_error_upper_rad"]) and height >= number(supplied["height_error_upper_m"]) and frame >= 0 and p >= math.nextafter(frame+math.hypot(number(reference["radius_m"])*angle, height), -math.inf), "unresolved cylindrical component bounds")
    normal_certificate(proof["blend_normal_bounds"], expected["maximum_blend_normal_error"], expected["max_leaves"])
    return proof


SEAM_REFUSALS = ("input_limit", "wrong_chart", "wrong_line", "wrong_units", "changed_edge", "changed_weight", "changed_left", "changed_right", "missing_control", "last_request_precision")
SEAM_GAPS = ("edge_to_left_gap_upper_m", "edge_to_right_gap_upper_m", "left_to_right_gap_upper_m")


def seam_layout(edge):
    roles, lines = edge["roles"], edge["seam_lines"]
    require(len(roles) == len(lines) == 2 and all(type(r["kind"]) is int and r["kind"] == 0 and type(r["chart"]) is int and 0 <= r["chart"] < 4 for r in roles), "unsupported seam owner inventory")
    ends = []
    for line in lines:
        require(set(line) == {"kind", "origin", "direction", "range"} and line["kind"] == "geom2d_line" and line["origin"] in ([0, 0], [1, 0]) and line["direction"] == [0, 1] and line["range"] == [0, 1] and all(math.isfinite(number(x)) for key in ("origin", "direction", "range") for x in line[key]), "unsupported attached seam line")
        ends.append(line["origin"][0])
    require(sorted(ends) == [0, 1], "seam must connect opposite chart boundaries")
    left = ends.index(1);right = 1-left
    require(roles[right]["chart"] == (roles[left]["chart"]+1)%4, "nonadjacent seam charts")
    return left, right


def seam_edges(actual):
    return sorted((e for e in actual["shared_edges"] if all(r["kind"] == 0 for r in e["roles"])), key=lambda e: e["roles"][seam_layout(e)[0]]["chart"])


def seam_boundary_request(actual, edge):
    left, right = seam_layout(edge)
    charts = [edge["roles"][i]["chart"] for i in (left, right)]
    faces = [next(f for f in actual["surface_faces"] if f["role"] == {"kind": 0, "chart": c}) for c in charts]
    curve = edge["seam_curve"]
    return {"left_chart": charts[0], "right_chart": charts[1], "edge_poles_mm": curve["poles_mm"], "edge_weights": curve["weights"],
            "left_poles_mm": faces[0]["poles_mm"], "left_weights": faces[0]["weights"],
            "right_poles_mm": faces[1]["poles_mm"], "right_weights": faces[1]["weights"],
            "left_line": edge["seam_lines"][left], "right_line": edge["seam_lines"][right], "maximum_gap_m": 1e-12}


def grade_seam_boundary(actual, edge, proof):
    curve = edge["seam_curve"]
    require(curve["kind"] == "geom_bezier_curve" and type(curve["degree"]) is int and curve["degree"] == 2 and curve["periodic"] is False and curve["location_identity"] is True, "unsupported current seam representation")
    require(edge["range"] == [0, 1] and all(math.isfinite(number(x)) for x in edge["range"]), "unsupported seam domain")
    require(len(curve["poles_mm"]) == len(curve["poles_m"]) == len(curve["weights"]) == 3 and all(number(w) > 0 for w in curve["weights"]), "invalid seam controls")
    for raw, si in zip(curve["poles_mm"], curve["poles_m"]):
        require(distance([x/1000 for x in vector(raw)], si) <= POINT_M, "raw seam controls disagree with SI witnesses")
    expected = seam_boundary_request(actual, edge)
    require(same_numeric_data(proof["request"], expected) and type(proof["request"]["left_chart"]) is int and type(proof["request"]["right_chart"]) is int, "seam boundary capture/request mismatch")
    require(all(0 < number(proof[k]) <= expected["maximum_gap_m"] for k in SEAM_GAPS), "unresolved complete seam boundary precision")
    return proof


def normal_certificate(certificate, maximum_error, max_leaves):
    values = [number(certificate[key]) for key in ("jacobian_area_lower_m2", "jacobian_area_upper_m2", "ideal_normal_alignment_lower", "normal_error_upper", "du_error_upper_m", "dv_error_upper_m")]
    jlo, jhi, alignment, error, eu, ev = values
    require(0 < jlo <= jhi and 0 < alignment <= 1 and 0 < error <= maximum_error and 0 < eu <= D1_M and 0 < ev <= D1_M, "unresolved whole-chart normal/regularity/derivative certificate")
    require(type(certificate["leaves"]) is int and 1 <= certificate["leaves"] <= max_leaves, "incomplete/exhausted normal certificate work")
    return values


def grade(reference, native):
    for report in (reference, native):
        require(type(report.get("format_version")) is int and report["format_version"] == 1 and len(report["cases"]) == 2, "wrong rational surface report/inventory")
    require(native["occt_version"] == "8.0.0" and abs(number(reference["radius_m"])-.01) <= 1e-15 and abs(number(reference["height_m"])-.02) <= 1e-15, "wrong SDK/stock")
    require(number(reference["normal_error_request"]) == 1e-9 and type(reference["normal_max_leaves"]) is int and reference["normal_max_leaves"] == 4096, "wrong whole-chart normal precision/work request")
    refusals = native["surface_definition_proof_refusals"]
    require(set(refusals) == {"input_limit", "wrong_chart", "last_request_precision"} and all(v is True for v in refusals.values()), "missing live snapshot proof refusals")
    captured = native["surface_definition_certificates"]
    require(type(captured["format_version"]) is int and captured["format_version"] == 1 and len(captured["certificates"]) == 8 and captured["native_floating_evaluation_certified"] is False and captured["whole_brep_certified"] is False, "missing/mislabelled native definition certificate inventory")
    native_certificates = []
    contact_refusals = native["contact_definition_proof_refusals"]
    require(set(contact_refusals) == {"input_limit", "wrong_chart", "wrong_units", "nonuniform_weight", "missing_control", "last_request_precision"} and all(v is True for v in contact_refusals.values()), "missing live contact proof refusals")
    contact_proofs = native["contact_definition_certificates"]
    require(type(contact_proofs["format_version"]) is int and contact_proofs["format_version"] == 1 and len(contact_proofs["certificates"]) == 16 and all(contact_proofs[key] is False for key in ("native_floating_evaluation_certified", "attached_pcurve_lifts_certified", "whole_brep_certified")), "missing/overclaimed contact definition certificates")
    contact_certificates = []
    end_refusals = native["end_boundary_proof_refusals"]
    require(set(end_refusals) == {"input_limit", "wrong_line", "wrong_units", "changed_frame", "changed_pcurve", "missing_control", "last_request_precision", "exhausted_work"} and all(v is True for v in end_refusals.values()), "missing live end boundary refusals")
    end_proofs = native["end_boundary_certificates"]
    require(type(end_proofs["format_version"]) is int and end_proofs["format_version"] == 1 and len(end_proofs["certificates"]) == 8 and all(end_proofs[k] is False for k in ("native_floating_evaluation_certified", "side_contact_boundaries_certified", "whole_brep_certified")), "missing/overclaimed end boundary certificates")
    end_certificates = []
    side_refusals = native["side_boundary_proof_refusals"]
    require(set(side_refusals) == {"input_limit", "wrong_line", "nonunit_weight", "wrong_units", "changed_frame", "changed_radius", "changed_angle", "changed_height", "missing_control", "last_request_precision", "exhausted_work"} and all(v is True for v in side_refusals.values()), "missing live side boundary refusals")
    side_proofs = native["side_boundary_certificates"]
    require(type(side_proofs["format_version"]) is int and side_proofs["format_version"] == 1 and len(side_proofs["certificates"]) == 8 and all(side_proofs[k] is False for k in ("native_floating_evaluation_certified", "end_contact_boundaries_certified", "whole_brep_certified")), "missing/overclaimed side boundary certificates")
    side_certificates = []
    seam_refusals = native["seam_boundary_proof_refusals"]
    require(set(seam_refusals) == set(SEAM_REFUSALS) and all(v is True for v in seam_refusals.values()), "missing live seam boundary refusals")
    seam_proofs = native["seam_boundary_certificates"]
    require(type(seam_proofs["format_version"]) is int and seam_proofs["format_version"] == 1 and len(seam_proofs["certificates"]) == 8 and all(seam_proofs[k] is False for k in ("native_floating_evaluation_certified", "paired_normals_certified", "whole_brep_certified")), "missing/overclaimed seam boundary certificates")
    seam_certificates = []
    certificates = []
    maxima = {key: 0. for key in ("point_m", "d1_m", "normal", "join_gap_m", "join_normal_gap")}
    phase_mismatches = []
    for law, (ref, actual) in enumerate(zip(reference["cases"], native["cases"])):
        require(ref["law"] == ("constant" if law == 0 else "variable") and abs(number(ref["minimum_radius_m"])-.0005) <= 1e-15 and abs(number(ref["maximum_radius_m"])-(.0005 if law == 0 else .0015)) <= 1e-15, "wrong reference law/phase")
        require(len(ref["patches"]) == len(ref["side_pcurves"]) == 4 and len(ref["contacts"]) == 8, "incomplete reference charts/contacts")
        for key in ("valid", "closed", "connected", "edge_incidence_two"): require(actual[key] is True, "invalid/open/disconnected native reference solid")
        for key, value in (("faces", 10), ("edges", 20), ("vertices", 12), ("shells", 1), ("solids", 1), ("euler", 2), ("free_edges", 0), ("multiple_edges", 0)):
            require(type(actual[key]) is int and actual[key] == value, "wrong sewn topology")
        lo, hi, volume = number(ref["volume_lower_m3"]), number(ref["volume_upper_m3"]), number(actual["volume_m3"])
        require(0 < lo <= hi < math.pi*.01**2*.02 and hi-lo <= 1e-12 and lo-VOLUME_M3 <= volume <= hi+VOLUME_M3, "native volume outside independent ideal enclosure")
        require(0 <= number(actual["volume_refinement_delta_m3"]) <= VOLUME_M3 and abs(number(actual["rotated_volume_m3"])-volume) <= VOLUME_M3, "unstable or noninvariant native volume")
        if law == 0:
            r = .0005
            constant = math.pi*.01**2*.02-2*math.pi*((.01-r)*(1-math.pi/4)*r*r+r**3/6)
            require(abs(number(ref["constant_limit_volume_m3"])-constant) <= 1e-20 and abs(volume-constant) <= VOLUME_M3, "constant-limit analytic volume failed")
        else: require(ref["constant_limit_volume_m3"] is None, "variable law replaced by constant")
        center = vector(actual["center_of_mass_m"])
        require(abs(center[0]) < .01 and abs(center[1]) < .01 and 0 < center[2] < .02, "nonphysical/nonfinite measured COM")
        require(len(actual["vertex_tolerances_m"]) == 12 and all(0 < number(x) <= JOIN_M for x in actual["vertex_tolerances_m"]), "inflated/missing sewn vertex tolerances")
        require(len(actual["surface_faces"]) == 10, "missing current native faces")
        faces, blends = Counter(), {}
        for face in actual["surface_faces"]:
            kind, chart = face["role"]["kind"], face["role"]["chart"]
            require(type(kind) is int and type(chart) is int and 0 <= kind <= 3 and (0 <= chart < 4 if kind == 0 else chart == -1), "unknown surface role")
            faces[kind] += 1
            require(face["location_identity"] is True, "native surface Location is not identity")
            require(face["forward"] is (kind != 3), "inverted blend/support orientation")
            if kind != 0:
                require(distance(face["support_origin_m"], [0, 0, .02 if kind == 2 else 0]) <= POINT_M and distance(face["support_axis"], [0, 0, 1]) <= 1e-14 and distance(face["support_x"], [1, 0, 0]) <= 1e-14 and abs(number(face["support_radius_m"])-(.01 if kind == 1 else 0.)) <= POINT_M, "unchanged analytic support lost")
                continue
            require(chart not in blends, "duplicate native rational chart")
            blends[chart] = face
            patch = ref["patches"][chart]
            require(type(patch["chart"]) is int and patch["chart"] == chart, "wrong reference chart order")
            for key, value in (("u_degree", 8), ("v_degree", 2)):
                require(type(face[key]) is int and face[key] == value and type(patch[key]) is int and patch[key] == value, "wrong rational surface degree")
            bound = number(patch["position_error_upper_m"])
            require(0 < bound <= 1e-12, "unresolved whole-patch actual-pole error")
            certificate = patch["normal_bounds"]
            jlo, jhi, alignment, error, eu, ev = normal_certificate(certificate, reference["normal_error_request"], reference["normal_max_leaves"])
            certificates.append(certificate)
            poles, weights = patch["control_points_m"], patch["weights"]
            require(len(poles) == len(weights) == len(face["poles_m"]) == len(face["weights"]) == 9 and all(len(row) == 3 for net in (poles, weights, face["poles_m"], face["weights"]) for row in net), "missing current control net")
            require(all(number(w) > 0 for net in (weights, face["weights"]) for row in net for w in row), "nonpositive patch weight")
            for i in range(9):
                for j in range(3):
                    require(distance(face["poles_m"][i][j], poles[i][j]) <= POINT_M and abs(face["weights"][i][j]/face["weights"][0][0]-weights[i][j]/weights[0][0]) <= 1e-13, "sewing changed rational surface controls")
            require(face["u_bounds"] == [0, 1] and face["v_bounds"] == [0, 1] and all(type(x) is not bool and number(x) in (0., 1.) for x in face["u_bounds"]+face["v_bounds"]) and face["u_periodic"] is False and face["v_periodic"] is False, "current native definition parameterization changed")
            raw = face["poles_mm"]
            require(len(raw) == 9 and all(len(row) == 3 for row in raw), "incomplete original native millimetre controls")
            for row, si_row in zip(raw, face["poles_m"]):
                for mm, si in zip(row, si_row): require(distance([x/1000 for x in vector(mm)], si) <= POINT_M, "raw native and rounded SI export disagree")
            snapshot = captured["certificates"][law*4+chart]
            expected = snapshot_request(reference, ref, face)
            # Exact binary64 DATA identity binds the proof transport, not a
            # geometric closeness test or a surrogate source-control comparison.
            require(isinstance(snapshot["request"], dict) and set(snapshot["request"]) == set(expected) and type(snapshot["request"]["chart"]) is int and type(snapshot["request"]["max_leaves"]) is int and all(same_numeric_data(snapshot["request"][key], value) for key, value in expected.items()), "native definition certificate capture/reference/request mismatch")
            native_position = number(snapshot["position_error_upper_m"])
            require(0 < native_position <= expected["maximum_position_error_m"], "unresolved native definition position certificate")
            njlo, njhi, nalignment, nerror, neu, nev = normal_certificate(snapshot["normal_bounds"], expected["maximum_normal_error"], expected["max_leaves"])
            native_certificates.append(snapshot)
            require(len(face["samples"]) == 81, "missing native D1/normal/surface witnesses")
            for index, sample in enumerate(face["samples"]):
                u, v = index//9/8, index%9/8
                require(number(sample["u"]) == u and number(sample["v"]) == v, "wrong rational surface parameter")
                point, du, dv = tensor(poles, weights, u, v)
                normal, _ = unit_cross(du, dv)
                ep, ed = distance(point, sample["point_m"]), max(distance(du, sample["du_m"]), distance(dv, sample["dv_m"]))
                en = distance(normal, sample["normal"])
                require(ep <= POINT_M and ed <= D1_M and en <= NORMAL, "current native tensor value/D1/normal differs from supplied controls")
                theta = chart*math.pi/2+2*math.atan(u)
                r = .0005 if law == 0 else .001+.0005*math.cos(theta)
                k = 0. if law == 0 else -.0005*math.sin(theta)/(.01-r)
                alpha = math.atan2(2*v+2*k*k*v*v, 1-v*v)
                ideal, ideal_normal = envelope(law, theta, alpha)
                native_normal, native_jacobian = unit_cross(vector(sample["du_m"]), vector(sample["dv_m"]))
                require(jlo-JACOBIAN_M2 <= native_jacobian <= jhi+JACOBIAN_M2 and distance(native_normal, ideal_normal) <= error+NORMAL and sum(x*y for x, y in zip(native_normal, ideal_normal)) >= alignment-NORMAL, "native D1/normal witness outside rounded rational certificate")
                require(njlo-JACOBIAN_M2 <= native_jacobian <= njhi+JACOBIAN_M2 and distance(sample["point_m"], ideal) <= native_position+POINT_M and distance(native_normal, ideal_normal) <= nerror+NORMAL and sum(x*y for x, y in zip(native_normal, ideal_normal)) >= nalignment-NORMAL, "native witness outside its captured surface-definition certificate")
                if law == 0:
                    # Independent exact toroidal-chart derivatives in original u,v.
                    rho, tu, av = .01-r+r*math.cos(alpha), 2/(1+u*u), 2/(1+v*v)
                    exact_du = [-rho*math.sin(theta)*tu, rho*math.cos(theta)*tu, 0.]
                    exact_dv = [-r*math.sin(alpha)*math.cos(theta)*av, -r*math.sin(alpha)*math.sin(theta)*av, r*math.cos(alpha)*av]
                    require(distance(sample["du_m"], exact_du) <= eu+D1_M and distance(sample["dv_m"], exact_dv) <= ev+D1_M, "native D1 outside independent constant-chart derivative bounds")
                    require(distance(sample["du_m"], exact_du) <= neu+D1_M and distance(sample["dv_m"], exact_dv) <= nev+D1_M, "native D1 outside captured constant-chart certificate")
                require(distance(sample["point_m"], ideal) <= bound+POINT_M and distance(sample["normal"], ideal_normal) <= NORMAL, "native rational surface differs from independent sphere envelope")
                maxima["point_m"], maxima["d1_m"], maxima["normal"] = max(maxima["point_m"], ep), max(maxima["d1_m"], ed), max(maxima["normal"], en)
        require(faces == {0: 4, 1: 4, 2: 1, 3: 1} and set(blends) == set(range(4)), "wrong analytic/blend face inventory")
        require(len(actual["shared_edges"]) == 20, "missing native adjacency/pcurves")
        pairs, seam_pairs, side_contacts, end_contacts = Counter(), [], [], []
        for edge in actual["shared_edges"]:
            require(edge["same_parameter"] is True and edge["same_range"] is True and 0 < number(edge["tolerance_m"]) <= JOIN_M, "missing flags/inflated native edge tolerance")
            require(len(edge["roles"]) == 2 and len(edge["range"]) == 2, "missing edge adjacency/range")
            roles = [(r["kind"], r["chart"]) for r in edge["roles"]]
            require(all(type(k) is int and type(c) is int and 0 <= k <= 3 and (0 <= c < 4 if k == 0 else c == -1) for k, c in roles), "invalid current edge owner")
            pair = tuple(sorted(k for k, _ in roles));pairs[pair] += 1
            first, last = map(number, edge["range"])
            require(first < last and len(edge["samples"]) == 9, "missing/invalid current edge samples")
            contact = None
            if pair == (0, 0):
                seam_pairs.append(tuple(sorted(c for _, c in roles)))
                left, right = seam_layout(edge)
                seam_chart = roles[left][1]
                seam_whole = grade_seam_boundary(actual, edge, seam_proofs["certificates"][law*4+seam_chart])
                seam_certificates.append(seam_whole)
            if pair in ((0, 1), (0, 2)):
                chart = next(c for k, c in roles if k == 0)
                (side_contacts if pair == (0, 1) else end_contacts).append(chart)
                contact = ref["contacts"][(0 if pair == (0, 1) else 4)+chart]
                require(contact["kind"] == ("side" if pair == (0, 1) else "end") and type(contact["chart"]) is int and contact["chart"] == chart and 0 < number(contact["position_error_upper_m"]) <= 1e-12, "wrong contact reference")
                controls(edge["contact_curve"], contact, 2 if pair == (0, 1) else 4)
                require(abs(first) <= 1e-14 and abs(last-1) <= 1e-14, "contact range changed")
                # Fixed representation domain is a DATA gate, not geometry
                # proximity: the proof covers exactly the canonical unit span.
                require(all(type(x) is not bool for x in edge["range"]) and edge["range"] == [0, 1], "contact definition domain unsupported")
                raw = edge["contact_curve"]["poles_mm"]
                require(len(raw) == len(edge["contact_curve"]["poles_m"]), "missing raw contact controls")
                for mm, si in zip(raw, edge["contact_curve"]["poles_m"]):
                    require(distance([x/1000 for x in vector(mm)], si) <= POINT_M, "raw contact and rounded SI export disagree")
                proof = contact_proofs["certificates"][law*8+(0 if pair == (0, 1) else 4)+chart]
                expected_request = contact_request(reference, ref, edge)
                echo = proof["request"]
                require(isinstance(echo, dict) and set(echo) == set(expected_request) and echo["kind"] == expected_request["kind"] and type(echo["chart"]) is int and all(same_numeric_data(echo[key], value) for key, value in expected_request.items() if key != "kind"), "contact certificate capture/reference/request mismatch")
                bound = number(proof["position_error_upper_m"])
                require(0 < bound <= expected_request["maximum_position_error_m"], "unresolved whole-curve native contact position certificate")
                contact_certificates.append(proof)
                if pair == (0, 1):
                    pcurve(edge["cylindrical_pcurve"], ref["side_pcurves"][chart], chart, law)
                    side_certificates.append(grade_side_boundary(reference, ref, actual, edge, side_proofs["certificates"][law*4+chart]))
                else: end_certificates.append(grade_end_boundary(reference, ref, actual, edge, end_proofs["certificates"][law*4+chart]))
            for j, sample in enumerate(edge["samples"]):
                t = first+(last-first)*edge_fraction(j)
                require(abs(number(sample["t"])-t) <= 1e-12 and len(sample["sides"]) == 2, "wrong native edge/pcurve parameter")
                point = vector(sample["point_m"])
                if pair == (0, 0):
                    expected_point, _ = rational(edge["seam_curve"]["poles_m"], edge["seam_curve"]["weights"], t)
                    require(distance(point, expected_point) <= POINT_M, "native seam witness differs from captured edge")
                    lp, rp = [sample["sides"][i]["point_m"] for i in (left, right)]
                    require(all(distance(a, b) <= seam_whole[k]+2*POINT_M for k, a, b in zip(SEAM_GAPS, (point, point, lp), (lp, rp, rp))), "native paired witness outside whole seam boundary certificate")
                    for i, end in ((left, 1), (right, 0)):
                        uv = sample["sides"][i]["uv"]
                        require(len(uv) == 2 and abs(number(uv[0])-end) <= 1e-14 and abs(number(uv[1])-t) <= 1e-14, "seam witness differs from canonical attached line")
                if contact is not None:
                    expected, _ = rational(contact["control_points_m"], contact["weights"], t)
                    require(distance(point, expected) <= POINT_M, "current sewn contact geometry changed")
                    theta = contact["chart"]*math.pi/2+2*math.atan(t)
                    radius = .0005 if law == 0 else .001+.0005*math.cos(theta)
                    rho = .01 if pair == (0, 1) else .01-radius
                    ideal_contact = [rho*math.cos(theta), rho*math.sin(theta), .02-radius if pair == (0, 1) else .02]
                    require(distance(point, ideal_contact) <= bound+POINT_M, "native edge witness outside captured contact certificate")
                for (kind, chart), side in zip(roles, sample["sides"]):
                    require(len(side["uv"]) == 2, "missing native pcurve UV")
                    u, v = map(number, side["uv"])
                    if pair == (0, 1) and kind == 1:
                        expected_uv = cubic_pcurve(edge["cylindrical_pcurve"]["poles_rad_m"], t)
                        require(abs(u-expected_uv[0]) <= 1e-14 and abs(v-expected_uv[1]) <= POINT_M, "native pcurve witnesses differ from current bounded controls")
                    if kind == 0:
                        require(-1e-7 <= u <= 1+1e-7 and -1e-7 <= v <= 1+1e-7, "pcurve left rational face domain")
                        p, du, dv = tensor(blends[chart]["poles_m"], blends[chart]["weights"], u, v);n, _ = unit_cross(du, dv)
                    elif kind == 1: p, n = [.01*math.cos(u), .01*math.sin(u), v], [math.cos(u), math.sin(u), 0.]
                    else: p, n = [u, v, .02 if kind == 2 else 0.], [0., 0., 1. if kind == 2 else -1.]
                    require(distance(p, side["point_m"]) <= POINT_M and distance(n, side["normal"]) <= NORMAL, "current attached pcurve/surface relation changed")
                    gap = distance(point, side["point_m"])
                    require(gap <= JOIN_M, "native join left its support")
                    maxima["join_gap_m"] = max(maxima["join_gap_m"], gap)
                a, b = [vector(side["normal"]) for side in sample["sides"]]
                if pair == (0, 2):
                    whole = end_proofs["certificates"][law*4+contact["chart"]]
                    bp = next(side["point_m"] for role, side in zip(roles, sample["sides"]) if role[0] == 0)
                    pp = next(side["point_m"] for role, side in zip(roles, sample["sides"]) if role[0] == 2)
                    require(distance(point, pp) <= whole["edge_to_plane_gap_upper_m"]+2*POINT_M and distance(point, bp) <= whole["edge_to_blend_gap_upper_m"]+2*POINT_M and distance(pp, bp) <= whole["plane_to_blend_gap_upper_m"]+2*POINT_M, "native paired witness outside whole end boundary certificate")
                if pair == (0, 1):
                    whole = side_proofs["certificates"][law*4+contact["chart"]]
                    bp = next(side["point_m"] for role, side in zip(roles, sample["sides"]) if role[0] == 0)
                    cp = next(side["point_m"] for role, side in zip(roles, sample["sides"]) if role[0] == 1)
                    require(distance(point, cp) <= whole["edge_to_cylinder_gap_upper_m"]+2*POINT_M and distance(point, bp) <= whole["edge_to_blend_gap_upper_m"]+2*POINT_M and distance(cp, bp) <= whole["cylinder_to_blend_gap_upper_m"]+2*POINT_M, "native paired witness outside whole side boundary certificate")
                if pair == (1, 3): require(abs(sum(x*y for x, y in zip(a, b))) <= NORMAL, "bottom edge lost expected right angle")
                else:
                    gap = distance(a, b);require(gap <= NORMAL, "native support/seam tangent-plane witness failed")
                    # Whole mathematical support/seam implications plus a separate
                    # native witness allowance; this does not certify native G1.
                    certified_gap = sum(ref["patches"][c]["normal_bounds"]["normal_error_upper"] for k, c in roles if k == 0)
                    if pair == (0, 1): certified_gap += ref["side_pcurves"][next(c for k, c in roles if k == 0)]["angle_error_upper_rad"]
                    if pair in ((0, 0), (0, 1), (0, 2)):
                        require(gap <= certified_gap+NORMAL, "native join normal witness outside mathematical boundary implication")
                    maxima["join_normal_gap"] = max(maxima["join_normal_gap"], gap)
        require(pairs == {(0, 0): 4, (0, 1): 4, (0, 2): 4, (1, 1): 4, (1, 3): 4} and sorted(seam_pairs) == [(0, 1), (0, 3), (1, 2), (2, 3)] and sorted(side_contacts) == sorted(end_contacts) == [0, 1, 2, 3], "wrong sewn support/seam adjacency")
        require(len(actual["classifications"]) == 8, "missing actual solid/phase classifiers")
        mismatch = 0
        for j, measured in enumerate(actual["classifications"]):
            theta, offset = j//2*math.pi/2, -.000025 if j%2 == 0 else .000025
            point, normal = envelope(law, theta, math.pi/4)
            expected = [p+offset*n for p, n in zip(point, normal)]
            state = 1 if j%2 == 0 else 0
            require(abs(number(measured["theta_rad"])-theta) <= 1e-14 and abs(number(measured["offset_m"])-offset) <= 1e-18 and distance(measured["point_m"], expected) <= POINT_M and type(measured["state"]) is int and measured["state"] == state, "native body fails inside/outside reference controls")
            require(type(measured["rotated_state"]) is int and measured["rotated_state"] in (0, 1), "missing actual transformed body classifier")
            mismatch += measured["rotated_state"] != state
        phase_mismatches.append(mismatch)
    require(phase_mismatches[0] == 0 and phase_mismatches[1] >= 2, "constant/variable actual solid phase decoys not distinguished")
    return {"diagnostic_passed": True, "candidate_admitted": False, "product_body_created": False,
            "native_diagnostic_solids_created": 2, "native_rational_blend_faces_created": 8,
            "native_seam_boundary_certificates": {"count": len(seam_certificates), "maximum_gap_m": max(c[k] for c in seam_certificates for k in SEAM_GAPS), "native_coordinate_conversion_included": True, "native_floating_evaluation_certified": False, "paired_normals_certified": False, "whole_brep_certified": False},
            "native_side_boundary_certificates": {"count": len(side_certificates), "maximum_gap_m": max(c[k] for c in side_certificates for k in ("edge_to_cylinder_gap_upper_m", "edge_to_blend_gap_upper_m", "cylinder_to_blend_gap_upper_m")), "native_coordinate_conversion_included": True, "native_floating_evaluation_certified": False, "end_contact_boundaries_certified": False, "whole_brep_certified": False},
            "native_end_boundary_certificates": {"count": len(end_certificates), "maximum_gap_m": max(c[k] for c in end_certificates for k in ("edge_to_plane_gap_upper_m", "edge_to_blend_gap_upper_m", "plane_to_blend_gap_upper_m")), "native_coordinate_conversion_included": True, "native_floating_evaluation_certified": False, "side_contact_boundaries_certified": False, "whole_brep_certified": False},
            "native_contact_definition_certificates": {"count": len(contact_certificates), "maximum_position_error_m": max(c["position_error_upper_m"] for c in contact_certificates), "includes_native_coordinate_conversion": True, "native_floating_evaluation_certified": False, "attached_pcurve_lifts_certified": False, "whole_brep_certified": False},
            "rounded_reference_certificates": {"count": len(certificates), "maximum_normal_error": max(c["normal_error_upper"] for c in certificates), "minimum_jacobian_m2": min(c["jacobian_area_lower_m2"] for c in certificates), "total_cover_leaves": sum(c["leaves"] for c in certificates), "global_native_certificate": False},
            "native_surface_definition_certificates": {"count": len(native_certificates), "maximum_position_error_m": max(c["position_error_upper_m"] for c in native_certificates), "maximum_normal_error": max(c["normal_bounds"]["normal_error_upper"] for c in native_certificates), "minimum_jacobian_m2": min(c["normal_bounds"]["jacobian_area_lower_m2"] for c in native_certificates), "total_cover_leaves": sum(c["normal_bounds"]["leaves"] for c in native_certificates), "includes_native_coordinate_conversion": True, "native_floating_evaluation_certified": False, "whole_brep_certified": False},
            "maxima": maxima, "rotated_classifier_mismatches": phase_mismatches,
            "native_allowances": {"point_m": POINT_M, "d1_m": D1_M, "normal": NORMAL, "join_m": JOIN_M, "volume_m3": VOLUME_M3, "jacobian_m2": JACOBIAN_M2}}


def run(occt_root, output):
    require(sys.platform.startswith("linux"), "pinned Linux SDK required")
    compiler = shutil.which("c++");require(compiler is not None, "C++17 compiler required")
    include, lib = occt_root/"include/opencascade", occt_root/"lib"
    libraries = ["TKTopAlgo", "TKShHealing", "TKGeomAlgo", "TKBRep", "TKGeomBase", "TKG3d", "TKG2d", "TKMath", "TKernel"]
    require((include/"Geom_BezierSurface.hxx").is_file() and all((lib/f"lib{x}.a").is_file() for x in libraries), "prepared native SDK missing")
    ref = json.loads(subprocess.run(["cargo", "run", "--quiet", "-p", "opencad-geometry", "--example", "periodic_fillet_rational"], check=True, cwd=ROOT, capture_output=True, text=True, timeout=180).stdout)
    values = ["2"]
    for case in ref["cases"]:
        for curve in case["contacts"]:
            values.append(str(curve["degree"]))
            for p, w in zip(curve["control_points_m"], curve["weights"]): values.extend(format(x, ".17g") for x in [*p, w])
        for pc in case["side_pcurves"]:
            values.append(str(pc["divisions"]))
            for p in pc["control_points_rad_m"]: values.extend(format(x, ".17g") for x in p)
        for patch in case["patches"]:
            for row, weights in zip(patch["control_points_m"], patch["weights"]):
                for p, w in zip(row, weights): values.extend(format(x, ".17g") for x in [*p, w])
    with tempfile.TemporaryDirectory(prefix="musubicad-rational-reference-") as directory:
        binary = Path(directory)/"probe"
        command = [compiler, "-O2", "-std=c++17", "-I", str(include), str(ROOT/"modules/kernel-occt/diagnostics/rational_reference.cpp"), "-L", str(lib), "-Wl,--start-group"]
        command += [f"-l{x}" for x in libraries]+["-Wl,--end-group", "-lpthread", "-ldl", "-o", str(binary)]
        subprocess.run(command, check=True, cwd=ROOT, timeout=180)
        result = subprocess.run([str(binary)], input="\n".join(values)+"\n", capture_output=True, text=True, timeout=120)
        require(result.returncode == 0, "native construction failed: "+result.stderr[:1000])
        native = json.loads(result.stdout)
    requests = [snapshot_request(ref, case, face) for case, actual in zip(ref["cases"], native["cases"])
                for face in sorted((f for f in actual["surface_faces"] if f["role"]["kind"] == 0), key=lambda f: f["role"]["chart"])]
    command = ["cargo", "run", "--quiet", "-p", "opencad-geometry", "--example", "periodic_fillet_native_snapshot"]
    payload = json.dumps({"format_version": 1, "requests": requests}, allow_nan=False)
    proof = subprocess.run(command, input=payload, check=True, cwd=ROOT, capture_output=True, text=True, timeout=180)
    native["surface_definition_certificates"] = json.loads(proof.stdout)
    wrong_chart = copy.deepcopy(requests);wrong_chart[0]["chart"] = 4
    tight_last = copy.deepcopy(requests);tight_last[-1]["maximum_normal_error"] = 1e-30
    adversarial = {"input_limit": payload+" "*max(1, 262145-len(payload.encode("utf-8"))),
                   "wrong_chart": json.dumps({"format_version": 1, "requests": wrong_chart}, allow_nan=False),
                   "last_request_precision": json.dumps({"format_version": 1, "requests": tight_last}, allow_nan=False)}
    native["surface_definition_proof_refusals"] = {}
    for name, bad_payload in adversarial.items():
        refused = subprocess.run(command, input=bad_payload, cwd=ROOT, capture_output=True, text=True, timeout=180)
        require(refused.returncode != 0 and not refused.stdout.strip(), "snapshot probe accepted invalid input or emitted partial certificates: "+name)
        native["surface_definition_proof_refusals"][name] = True
    requests = [contact_request(ref, case, edge) for case, actual in zip(ref["cases"], native["cases"]) for edge in contact_edges(actual)]
    command = ["cargo", "run", "--quiet", "-p", "opencad-geometry", "--example", "periodic_fillet_native_contacts"]
    payload = json.dumps({"format_version": 1, "requests": requests}, allow_nan=False)
    proof = subprocess.run(command, input=payload, check=True, cwd=ROOT, capture_output=True, text=True, timeout=180)
    native["contact_definition_certificates"] = json.loads(proof.stdout)
    wrong_chart = copy.deepcopy(requests);wrong_chart[0]["chart"] = 4
    wrong_units = copy.deepcopy(requests);wrong_units[0]["poles_mm"] = [[x/1000 for x in p] for p in wrong_units[0]["poles_mm"]]
    changed_weight = copy.deepcopy(requests);changed_weight[0]["weights"][1] *= 1.0001
    missing = copy.deepcopy(requests);missing[0]["poles_mm"].pop()
    tight_last = copy.deepcopy(requests);tight_last[-1]["maximum_position_error_m"] = 1e-30
    adversarial = {"input_limit": payload+" "*max(1, 65537-len(payload.encode("utf-8")))}
    for name, data in (("wrong_chart", wrong_chart), ("wrong_units", wrong_units), ("nonuniform_weight", changed_weight), ("missing_control", missing), ("last_request_precision", tight_last)):
        adversarial[name] = json.dumps({"format_version": 1, "requests": data}, allow_nan=False)
    native["contact_definition_proof_refusals"] = {}
    for name, bad_payload in adversarial.items():
        refused = subprocess.run(command, input=bad_payload, cwd=ROOT, capture_output=True, text=True, timeout=180)
        require(refused.returncode != 0 and not refused.stdout.strip(), "contact proof accepted invalid input or emitted partial certificates: "+name)
        native["contact_definition_proof_refusals"][name] = True
    requests = [end_boundary_request(ref, case, actual, edge) for case, actual in zip(ref["cases"], native["cases"]) for edge in contact_edges(actual) if any(r["kind"] == 2 for r in edge["roles"])]
    command = ["cargo", "run", "--quiet", "-p", "opencad-geometry", "--example", "periodic_fillet_end_boundary"]
    payload = json.dumps({"format_version": 1, "requests": requests}, allow_nan=False)
    proof = subprocess.run(command, input=payload, check=True, cwd=ROOT, capture_output=True, text=True, timeout=180)
    native["end_boundary_certificates"] = json.loads(proof.stdout)
    changes = {}
    for name in ("wrong_line", "wrong_units", "changed_frame", "changed_pcurve", "missing_control", "last_request_precision", "exhausted_work"):
        data = copy.deepcopy(requests)
        if name == "wrong_line": data[0]["blend_line"]["origin"][1] = 1.0000000000000002
        elif name == "wrong_units": data[0]["pcurve_poles_mm"] = [[x/1000 for x in p] for p in data[0]["pcurve_poles_mm"]]
        elif name == "changed_frame": data[0]["plane_origin_mm"][2] += .001
        elif name == "changed_pcurve": data[0]["pcurve_poles_mm"][2][0] += .001
        elif name == "missing_control": data[0]["pcurve_poles_mm"].pop()
        elif name == "last_request_precision": data[-1]["maximum_gap_m"] = 1e-30
        else: data[0]["max_leaves"] = 1
        changes[name] = json.dumps({"format_version": 1, "requests": data}, allow_nan=False)
    changes["input_limit"] = payload+" "*max(1, 262145-len(payload.encode("utf-8")))
    native["end_boundary_proof_refusals"] = {}
    for name, bad_payload in changes.items():
        refused = subprocess.run(command, input=bad_payload, cwd=ROOT, capture_output=True, text=True, timeout=180)
        require(refused.returncode != 0 and not refused.stdout.strip(), "end boundary accepted invalid input or emitted partial certificates: "+name)
        native["end_boundary_proof_refusals"][name] = True
    requests = [side_boundary_request(ref, case, actual, edge) for case, actual in zip(ref["cases"], native["cases"]) for edge in contact_edges(actual) if any(r["kind"] == 1 for r in edge["roles"])]
    command = ["cargo", "run", "--quiet", "-p", "opencad-geometry", "--example", "periodic_fillet_side_boundary"]
    payload = json.dumps({"format_version": 1, "requests": requests}, allow_nan=False)
    proof = subprocess.run(command, input=payload, check=True, cwd=ROOT, capture_output=True, text=True, timeout=180)
    native["side_boundary_certificates"] = json.loads(proof.stdout)
    changes = {}
    for name in ("wrong_line", "nonunit_weight", "wrong_units", "changed_frame", "changed_radius", "changed_angle", "changed_height", "missing_control", "last_request_precision", "exhausted_work"):
        data = copy.deepcopy(requests)
        if name == "wrong_line": data[0]["blend_line"]["origin"][1] = 1e-16
        elif name == "nonunit_weight": data[0]["pcurve_weights"][384] = 1.0000000000000002
        elif name == "wrong_units": data[0]["pcurve_poles_rad_mm"] = [[p[0], p[1]/1000] for p in data[0]["pcurve_poles_rad_mm"]]
        elif name == "changed_frame": data[0]["cylinder_origin_mm"][2] += .001
        elif name == "changed_radius": data[0]["cylinder_radius_mm"] += .001
        elif name == "changed_angle": data[0]["pcurve_poles_rad_mm"][384][0] += 1e-5
        elif name == "changed_height": data[0]["pcurve_poles_rad_mm"][384][1] += .001
        elif name == "missing_control": data[0]["pcurve_poles_rad_mm"].pop()
        elif name == "last_request_precision": data[-1]["maximum_gap_m"] = 1e-30
        else: data[0]["max_leaves"] = 1
        changes[name] = json.dumps({"format_version": 1, "requests": data}, allow_nan=False)
    changes["input_limit"] = payload+" "*max(1, 1048577-len(payload.encode("utf-8")))
    native["side_boundary_proof_refusals"] = {}
    for name, bad_payload in changes.items():
        refused = subprocess.run(command, input=bad_payload, cwd=ROOT, capture_output=True, text=True, timeout=180)
        require(refused.returncode != 0 and not refused.stdout.strip(), "side boundary accepted invalid input or emitted partial certificates: "+name)
        native["side_boundary_proof_refusals"][name] = True
    requests = [seam_boundary_request(actual, edge) for actual in native["cases"] for edge in seam_edges(actual)]
    command = ["cargo", "run", "--quiet", "-p", "opencad-geometry", "--example", "periodic_fillet_seam_boundary"]
    payload = json.dumps({"format_version": 1, "requests": requests}, allow_nan=False)
    proof = subprocess.run(command, input=payload, check=True, cwd=ROOT, capture_output=True, text=True, timeout=180)
    native["seam_boundary_certificates"] = json.loads(proof.stdout)
    changes = {}
    for name in SEAM_REFUSALS:
        if name == "input_limit":
            changes[name] = payload+" "*max(1, 262145-len(payload.encode("utf-8")))
            continue
        data = copy.deepcopy(requests)
        if name == "wrong_chart": data[0]["right_chart"] = data[0]["left_chart"]
        elif name == "wrong_line": data[0]["left_line"]["origin"][0] = 1.0000000000000002
        elif name == "wrong_units": data[0]["edge_poles_mm"] = [[x/1000 for x in p] for p in data[0]["edge_poles_mm"]]
        elif name == "changed_edge": data[0]["edge_poles_mm"][1][0] += .001
        elif name == "changed_weight": data[0]["edge_weights"][1] *= 1.001
        elif name == "changed_left": data[0]["left_poles_mm"][8][1][0] += .001
        elif name == "changed_right": data[0]["right_poles_mm"][0][1][0] += .001
        elif name == "missing_control": data[0]["edge_poles_mm"].pop()
        else: data[-1]["maximum_gap_m"] = 1e-30
        changes[name] = json.dumps({"format_version": 1, "requests": data}, allow_nan=False)
    native["seam_boundary_proof_refusals"] = {}
    for name, bad_payload in changes.items():
        refused = subprocess.run(command, input=bad_payload, cwd=ROOT, capture_output=True, text=True, timeout=180)
        require(refused.returncode != 0 and not refused.stdout.strip(), "seam boundary accepted invalid input or emitted partial certificates: "+name)
        native["seam_boundary_proof_refusals"][name] = True
    report = {"format_version": 1, "diagnostic_passed": False, "candidate_admitted": False, "reference": ref, "native": native}
    try: report.update(grade(ref, native))
    except (KeyError, TypeError, ValueError) as error:
        report["error"] = str(error);raise
    finally:
        output.parent.mkdir(parents=True, exist_ok=True);output.write_text(json.dumps(report, indent=2, allow_nan=False)+"\n", encoding="utf-8")
    print(f"rational native diagnostic solids measured; product unsupported; report={output}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--occt-root", type=Path, required=True);parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try: run(args.occt_root.resolve(), args.output.resolve())
    except (KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError) as error: parser.exit(1, f"rational reference diagnostic failed: {error}\n")


if __name__ == "__main__": main()
