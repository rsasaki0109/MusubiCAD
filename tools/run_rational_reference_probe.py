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
            if pair == (0, 0): seam_pairs.append(tuple(sorted(c for _, c in roles)))
            if pair in ((0, 1), (0, 2)):
                chart = next(c for k, c in roles if k == 0)
                (side_contacts if pair == (0, 1) else end_contacts).append(chart)
                contact = ref["contacts"][(0 if pair == (0, 1) else 4)+chart]
                require(contact["kind"] == ("side" if pair == (0, 1) else "end") and type(contact["chart"]) is int and contact["chart"] == chart and 0 < number(contact["position_error_upper_m"]) <= 1e-12, "wrong contact reference")
                controls(edge["contact_curve"], contact, 2 if pair == (0, 1) else 4)
                require(abs(first) <= 1e-14 and abs(last-1) <= 1e-14, "contact range changed")
                if pair == (0, 1): pcurve(edge["cylindrical_pcurve"], ref["side_pcurves"][chart], chart, law)
            for j, sample in enumerate(edge["samples"]):
                t = first+(last-first)*edge_fraction(j)
                require(abs(number(sample["t"])-t) <= 1e-12 and len(sample["sides"]) == 2, "wrong native edge/pcurve parameter")
                point = vector(sample["point_m"])
                if contact is not None:
                    expected, _ = rational(contact["control_points_m"], contact["weights"], t)
                    require(distance(point, expected) <= POINT_M, "current sewn contact geometry changed")
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
