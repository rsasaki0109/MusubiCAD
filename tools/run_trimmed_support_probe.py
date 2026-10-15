"""ADR-083: trim actual analytic native supports; no blend or body admission."""
from __future__ import annotations
import argparse
import json
import math
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from run_reference_bezier_probe import ROOT, distance, number, require, vector
from run_rational_contact_probe import rational

POSITION_ALLOWANCE_M = 1e-14
PROJECTION_ALLOWANCE_M = 1e-9  # Numerical native gate, not a certified global bound.
AREA_ALLOWANCE_M2 = 1e-10  # Numerical native integration gate.


def radius(law, theta):
    return 0.0005 if law == 0 else 0.001+0.0005*math.cos(theta)


def expected_queries(law, chart, rotated=False):
    """Independent physical witnesses, in metres; classifier states IN=1 OUT=0."""
    if chart >= 0:
        angles = [(chart+1.5)*math.pi/2] if rotated else [(chart+j/4)*math.pi/2 for j in (1, 2, 3)]
        return [([0.01*math.cos(t), 0.01*math.sin(t), 0.02-radius(law, t)+sign*0.0001], 1 if sign < 0 else 0)
                for t in angles for sign in (-1, 1)]
    return [([0., 0., 0.02], 1)]+[
        ([(0.01-radius(law, t)+sign*0.0001)*math.cos(t),
          (0.01-radius(law, t)+sign*0.0001)*math.sin(t), 0.02], 1 if sign < 0 else 0)
        for t in [(j+.5)*math.pi/2 for j in range(4)] for sign in (-1, 1)]


def grade(reference: dict, native: dict) -> dict:
    for report in (reference, native):
        require(type(report.get("format_version")) is int and report["format_version"] == 1, "wrong support report format")
    require(native["occt_version"] == "8.0.0", "wrong native SDK")
    require(abs(number(reference["radius_m"])-0.01) <= 1e-15 and abs(number(reference["height_m"])-0.02) <= 1e-15, "wrong support stock")
    require(len(reference["curves"]) == 16 and len(native["faces"]) == 10, "incomplete support inventory")
    max_gap, max_area, max_control = 0., 0., 0.
    phase_mismatches = [0, 0]
    for index, face in enumerate(native["faces"]):
        law, slot = divmod(index, 5)
        side, chart = slot < 4, slot if slot < 4 else -1
        require(face["kind"] == ("side" if side else "end") and type(face["chart"]) is int and face["chart"] == chart, "wrong support order")
        require(face["valid"] is True and face["forward"] is True and face["closed_wire"] is True, "invalid or reversed support face")
        for key, value in (("wires", 1), ("edges", 4), ("vertices", 4), ("shells", 0), ("solids", 0)):
            require(type(face[key]) is int and face[key] == value, "unexpected support topology")
        require(face["support_type"] == ("cylinder" if side else "plane"), "analytic support replaced")
        require(distance(face["support_origin_m"], [0., 0., 0. if side else .02]) <= POSITION_ALLOWANCE_M and
                distance(face["support_axis"], [0., 0., 1.]) <= 1e-14 and distance(face["support_x"], [1., 0., 0.]) <= 1e-14 and
                abs(number(face["support_radius_m"])-(.01 if side else 0.)) <= POSITION_ALLOWANCE_M, "original support frame/dimensions changed")
        require(0 <= number(face["projection_achieved_m"]) <= PROJECTION_ALLOWANCE_M, "projection tolerance exceeded")
        require(len(face["vertex_tolerances_m"]) == 4 and all(0 < number(x) <= PROJECTION_ALLOWANCE_M for x in face["vertex_tolerances_m"]), "inflated/missing vertex tolerance")
        m, a = (.0005, 0.) if law == 0 else (.001, .0005)
        area = .01*((.02-m)*math.pi/2-a*(math.sin((chart+1)*math.pi/2)-math.sin(chart*math.pi/2))) if side else math.pi*((.01-m)**2+a*a/2)
        residual = abs(number(face["area_m2"])-area)
        require(number(face["area_m2"]) > 0 and residual <= AREA_ALLOWANCE_M2, "retained native support area mismatch")
        max_area = max(max_area, residual)
        require(len(face["boundaries"]) == 4, "missing support boundaries")
        contacts = []
        for edge in face["boundaries"]:
            require(edge["same_parameter"] is True and edge["same_range"] is True, "missing native parameter flags")
            require(0 < number(edge["tolerance_m"]) <= PROJECTION_ALLOWANCE_M, "inflated edge tolerance")
            require(len(edge["range"]) == len(edge["pcurve_range"]) == 2, "missing parameter ranges")
            first, last = map(number, edge["range"])
            pf, pl = map(number, edge["pcurve_range"])
            require(first < last and abs(first-pf) <= 1e-14 and abs(last-pl) <= 1e-14, "mismatched current pcurve range")
            contact = edge["contact_chart"]
            require(type(contact) is int and -1 <= contact < 4, "wrong contact index")
            ref = None
            if contact >= 0:
                contacts.append(contact)
                ref = reference["curves"][law*8+(0 if side else 4)+contact]
                degree = 2 if side else 4
                require(ref["law"] == ("constant" if law == 0 else "variable") and ref["kind"] == ("side" if side else "end") and type(ref["chart"]) is int and ref["chart"] == contact, "wrong contact reference")
                require(type(ref["degree"]) is int and ref["degree"] == degree and type(edge["degree"]) is int and edge["degree"] == degree, "wrong current contact degree")
                require(0 < number(ref["position_error_upper_m"]) <= 1e-12 and abs(number(ref["minimum_radius_m"])-.0005) <= 1e-15 and abs(number(ref["maximum_radius_m"])-(.0005 if law == 0 else .0015)) <= 1e-15, "wrong contact law/bound")
                require(abs(first) <= 1e-14 and abs(last-1) <= 1e-14 and edge["knots"] == [0, 1] and edge["multiplicities"] == [degree+1]*2, "contact parameterization changed")
                require(len(ref["control_points_m"]) == len(ref["weights"]) == len(edge["poles_m"]) == len(edge["weights"]) == degree+1, "missing contact poles/weights")
                require(all(number(x) > 0 for x in ref["weights"]+edge["weights"]), "nonpositive contact weight")
                for p, q, w, v in zip(ref["control_points_m"], edge["poles_m"], ref["weights"], edge["weights"]):
                    error = distance(p, q)
                    require(error <= POSITION_ALLOWANCE_M and abs(v/edge["weights"][0]-w/ref["weights"][0]) <= 1e-14, "contact curve changed during trimming")
                    max_control = max(max_control, error)
            require(len(edge["samples"]) == 17, "missing current boundary/pcurve witnesses")
            for j, sample in enumerate(edge["samples"]):
                t = first+(last-first)*j/16
                require(abs(number(sample["t"])-t) <= 1e-14, "wrong boundary witness parameter")
                point, lifted, normal = vector(sample["point_m"]), vector(sample["lifted_m"]), vector(sample["normal"])
                require(len(sample["uv"]) == 2, "missing current UV")
                u, v = map(number, sample["uv"])
                support_point = [.01*math.cos(u), .01*math.sin(u), v] if side else [u, v, .02]
                support_normal = [math.cos(u), math.sin(u), 0.] if side else [0., 0., 1.]
                require(distance(lifted, support_point) <= POSITION_ALLOWANCE_M and distance(normal, support_normal) <= 1e-12, "current pcurve lifted onto wrong support")
                if side:
                    require(chart*math.pi/2-1e-7 <= u <= (chart+1)*math.pi/2+1e-7 and -.000000001 <= v <= .02+1e-9, "pcurve left admitted quadrant")
                gap = distance(point, lifted)
                require(gap <= PROJECTION_ALLOWANCE_M, "current pcurve/3D boundary mismatch")
                max_gap = max(max_gap, gap)
                if ref is not None:
                    expected, _ = rational(ref["control_points_m"], ref["weights"], t)
                    theta = contact*math.pi/2+2*math.atan(t)
                    rho = .01 if side else .01-radius(law, theta)
                    ideal = [rho*math.cos(theta), rho*math.sin(theta), .02-radius(law, theta) if side else .02]
                    require(distance(point, expected) <= POSITION_ALLOWANCE_M and distance(point, ideal) <= ref["position_error_upper_m"]+POSITION_ALLOWANCE_M, "trimmed contact no longer follows physical radius law")
        require(sorted(contacts) == ([chart] if side else [0, 1, 2, 3]), "missing/duplicate trimmed contact edges")
        queries = expected_queries(law, chart)
        require(len(face["classifications"]) == len(queries), "missing retained-region witnesses")
        for measured, (point, state) in zip(face["classifications"], queries):
            require(distance(measured["point_m"], point) <= POSITION_ALLOWANCE_M and type(measured["state"]) is int and measured["state"] == state, "retained/removed native support region mismatch")
        rotated = expected_queries(law, chart, True) if side else []
        require(len(face["rotated_classifications"]) == len(rotated), "missing actual native phase decoy")
        for measured, (point, state) in zip(face["rotated_classifications"], rotated):
            require(distance(measured["point_m"], point) <= POSITION_ALLOWANCE_M and type(measured["state"]) is int and measured["state"] in (0, 1), "invalid phase-decoy classifier witness")
            phase_mismatches[law] += measured["state"] != state
    require(phase_mismatches[0] == 0 and phase_mismatches[1] >= 2, "constant/variable native phase controls not distinguished")
    return {"diagnostic_passed": True, "candidate_admitted": False, "product_body_created": False,
            "native_trimmed_cylinder_faces_created": 8, "native_trimmed_plane_faces_created": 2,
            "projection_allowance_m": PROJECTION_ALLOWANCE_M, "area_allowance_m2": AREA_ALLOWANCE_M2,
            "max_pcurve_witness_gap_m": max_gap, "max_native_area_residual_m2": max_area,
            "max_native_contact_control_error_m": max_control, "rotated_region_mismatches": phase_mismatches}


def run(occt_root: Path, output: Path) -> None:
    require(sys.platform.startswith("linux"), "pinned Linux SDK required")
    compiler = shutil.which("c++")
    require(compiler is not None, "C++17 compiler required")
    include, lib = occt_root/"include/opencascade", occt_root/"lib"
    libraries = ["TKTopAlgo", "TKShHealing", "TKGeomAlgo", "TKBRep", "TKGeomBase", "TKG3d", "TKG2d", "TKMath", "TKernel"]
    require((include/"GeomProjLib.hxx").is_file() and all((lib/f"lib{x}.a").is_file() for x in libraries), "prepared native SDK missing")
    ref = json.loads(subprocess.run(["cargo", "run", "--quiet", "-p", "opencad-geometry", "--example", "periodic_fillet_contacts"], check=True, cwd=ROOT, capture_output=True, text=True, timeout=180).stdout)
    values = ["16"]
    for curve in ref["curves"]:
        values.append(str(curve["degree"]))
        for pole, weight in zip(curve["control_points_m"], curve["weights"]):
            values.extend(format(x, ".17g") for x in [*pole, weight])
    with tempfile.TemporaryDirectory(prefix="musubicad-trimmed-supports-") as directory:
        binary = Path(directory)/"probe"
        command = [compiler, "-O2", "-std=c++17", "-I", str(include), str(ROOT/"modules/kernel-occt/diagnostics/trimmed_supports.cpp"), "-L", str(lib), "-Wl,--start-group"]
        command += [f"-l{x}" for x in libraries]+["-Wl,--end-group", "-lpthread", "-ldl", "-o", str(binary)]
        subprocess.run(command, check=True, cwd=ROOT, timeout=180)
        native = json.loads(subprocess.run([str(binary)], input="\n".join(values)+"\n", check=True, capture_output=True, text=True, timeout=60).stdout)
    report = {"format_version": 1, "diagnostic_passed": False, "candidate_admitted": False, "reference": ref, "native": native}
    try:
        report.update(grade(ref, native))
    except (KeyError, TypeError, ValueError) as error:
        report["error"] = str(error)
        raise
    finally:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, indent=2, allow_nan=False)+"\n", encoding="utf-8")
    print(f"native analytic supports trimmed; blend/body unsupported; report={output}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--occt-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        run(args.occt_root.resolve(), args.output.resolve())
    except (KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f"support trimming diagnostic failed: {error}\n")


if __name__ == "__main__": main()
