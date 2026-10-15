"""ADR-082: actual rational support edges/wires, never fillet admission."""
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

POSITION_ALLOWANCE_M = 1e-14  # Native conversion/evaluation, not whole-curve error.
TANGENT_ALLOWANCE_M = 1e-13  # Metres per dimensionless local parameter.


def rational(poles: list, weights: list, t: float) -> tuple[list, list]:
    n = len(weights)-1
    def basis(degree, i):
        return math.comb(degree, i)*t**i*(1-t)**(degree-i)
    denominator = sum(basis(n, i)*weights[i] for i in range(n+1))
    numerator = [sum(basis(n, i)*weights[i]*poles[i][k] for i in range(n+1)) for k in range(3)]
    dd = n*sum(basis(n-1, i)*(weights[i+1]-weights[i]) for i in range(n))
    dn = [n*sum(basis(n-1, i)*(weights[i+1]*poles[i+1][k]-weights[i]*poles[i][k]) for i in range(n)) for k in range(3)]
    return [x/denominator for x in numerator], [(dn[k]*denominator-numerator[k]*dd)/denominator**2 for k in range(3)]


def grade(reference: dict, native: dict) -> dict:
    require(type(reference.get("format_version")) is int and reference["format_version"] == 1 and type(native.get("format_version")) is int and native["format_version"] == 1, "wrong contact report format")
    require(native["occt_version"] == "8.0.0", "wrong native SDK")
    require(abs(number(reference["radius_m"])-0.01) < 1e-15 and abs(number(reference["height_m"])-0.02) < 1e-15, "wrong contact stock")
    require(len(reference["curves"]) == len(native["curves"]) == 16 and len(native["wires"]) == 4, "incomplete contact inventory")
    max_point, max_tangent, max_contact_law = 0., 0., 0.
    wrong_phase = [0., 0.]
    for index, (ref, actual) in enumerate(zip(reference["curves"], native["curves"])):
        law, kind, chart = ("constant" if index < 8 else "variable"), ("side" if index%8 < 4 else "end"), index%4
        degree = 2 if kind == "side" else 4
        require(ref["law"] == law and ref["kind"] == kind and type(ref["chart"]) is int and ref["chart"] == chart, "wrong contact order/phase")
        require(type(ref["degree"]) is int and ref["degree"] == degree and type(actual["degree"]) is int and actual["degree"] == degree and actual["rational"] is True and actual["edge_valid"] is True, "wrong actual rational edge")
        require(abs(number(ref["minimum_radius_m"])-0.0005) < 1e-15 and abs(number(ref["maximum_radius_m"])-(0.0005 if law == "constant" else 0.0015)) < 1e-15, "wrong contact law")
        bound = number(ref["position_error_upper_m"])
        require(0 < bound <= 1e-12, "unresolved contact positional bound")
        poles, weights = ref["control_points_m"], ref["weights"]
        require(len(poles) == len(weights) == len(actual["poles_m"]) == len(actual["weights"]) == degree+1, "wrong contact pole inventory")
        require(all(number(w) > 0 for w in weights+actual["weights"]), "nonpositive contact weight")
        require(actual["knots"] == [0, 1] and actual["multiplicities"] == [degree+1, degree+1], "wrong native clamped parameterization")
        for p, q, w, z in zip(poles, actual["poles_m"], weights, actual["weights"]):
            require(distance(p, q) <= POSITION_ALLOWANCE_M and abs(z/actual["weights"][0]-w/weights[0]) <= 1e-14, "native contact controls changed")
        following = reference["curves"][index-chart+(chart+1)%4]
        require(vector(poles[-1]) == vector(following["control_points_m"][0]), "nonidentical closed contact endpoints")
        require(len(actual["samples"]) == len(actual["rotated_samples"]) == 9, "missing actual native contact witnesses")
        for j, (sample, rotated) in enumerate(zip(actual["samples"], actual["rotated_samples"])):
            t = j/8
            require(number(sample["u"]) == t and number(rotated["u"]) == t, "wrong native contact parameter")
            p, d = rational(poles, weights, t)
            e, f = distance(sample["point_m"], p), distance(sample["tangent_m"], d)
            require(e <= POSITION_ALLOWANCE_M and f <= TANGENT_ALLOWANCE_M, "native rational value or D1 mismatch")
            max_point, max_tangent = max(max_point, e), max(max_tangent, f)
            theta = chart*math.pi/2+2*math.atan(t)
            radius = 0.0005+(0. if law == "constant" else 0.0005*(1+math.cos(theta)))
            rho = 0.01 if kind == "side" else 0.01-radius
            ideal = [rho*math.cos(theta), rho*math.sin(theta), 0.02-radius if kind == "side" else 0.02]
            require(distance(sample["point_m"], ideal) <= bound+POSITION_ALLOWANCE_M, "native contact differs from bounded ideal position")
            measured = vector(sample["point_m"])
            require(abs(math.hypot(*measured[:2])-0.01) <= POSITION_ALLOWANCE_M if kind == "side" else abs(measured[2]-0.02) <= POSITION_ALLOWANCE_M, "native contact left its support")
            azimuth = math.atan2(measured[1], measured[0])
            law_radius = 0.0005+(0. if law == "constant" else 0.0005*(1+math.cos(azimuth)))
            contact_radius = 0.02-measured[2] if kind == "side" else 0.01-math.hypot(*measured[:2])
            require(abs(contact_radius-law_radius) <= bound+POSITION_ALLOWANCE_M, "native physical azimuth contact law mismatch")
            max_contact_law = max(max_contact_law, abs(contact_radius-law_radius))
            tangent = vector(sample["tangent_m"])
            require(distance(rotated["tangent_m"], [-tangent[1], tangent[0], tangent[2]]) <= TANGENT_ALLOWANCE_M, "wrong actual rotated derivative")
            q = vector(rotated["point_m"])
            require(distance(q, [-measured[1], measured[0], measured[2]]) <= POSITION_ALLOWANCE_M, "missing actual quarter-turn decoy")
            actual_radius = 0.02-q[2] if kind == "side" else 0.01-math.hypot(*q[:2])
            azimuth = math.atan2(q[1], q[0])
            target = 0.0005+(0. if law == "constant" else 0.0005*(1+math.cos(azimuth)))
            wrong_phase[index//8] = max(wrong_phase[index//8], abs(actual_radius-target))
    for wire in native["wires"]:
        require(wire["valid"] is True and wire["closed"] is True and type(wire["edges"]) is int and wire["edges"] == 4 and type(wire["vertices"]) is int and wire["vertices"] == 4, "native contact wire not valid closed four-edge cycle")
        require(number(wire["length_m"]) > 0 and abs(number(wire["rotated_length_m"])-wire["length_m"]) <= 1e-12, "native rigid-turn length control failed")
    require(wrong_phase[0] <= POSITION_ALLOWANCE_M and wrong_phase[1] > 1e-4, "constant/variable physical phase controls not distinguished")
    return {"diagnostic_passed": True, "candidate_admitted": False, "product_body_created": False,
            "native_contact_edges_created": 16, "native_closed_contact_wires_created": 4,
            "position_evaluation_allowance_m": POSITION_ALLOWANCE_M, "tangent_evaluation_allowance_m": TANGENT_ALLOWANCE_M,
            "max_native_point_error_m": max_point, "max_native_contact_law_error_m": max_contact_law, "max_native_tangent_error_m": max_tangent,
            "rotated_constant_law_error_m": wrong_phase[0], "rotated_variable_law_error_m": wrong_phase[1]}


def run(occt_root: Path, output: Path) -> None:
    require(sys.platform.startswith("linux"), "pinned Linux SDK required")
    compiler = shutil.which("c++")
    require(compiler is not None, "C++17 compiler required")
    include, lib = occt_root/"include/opencascade", occt_root/"lib"
    libraries = ["TKTopAlgo", "TKShHealing", "TKGeomAlgo", "TKBRep", "TKGeomBase", "TKG3d", "TKG2d", "TKMath", "TKernel"]
    require((include/"Geom_BSplineCurve.hxx").is_file() and all((lib/f"lib{x}.a").is_file() for x in libraries), "prepared native SDK missing")
    ref = json.loads(subprocess.run(["cargo", "run", "--quiet", "-p", "opencad-geometry", "--example", "periodic_fillet_contacts"], check=True, cwd=ROOT, capture_output=True, text=True, timeout=180).stdout)
    values = ["16"]
    for curve in ref["curves"]:
        values.append(str(curve["degree"]))
        for pole, weight in zip(curve["control_points_m"], curve["weights"]):
            values.extend(format(x, ".17g") for x in [*pole, weight])
    with tempfile.TemporaryDirectory(prefix="musubicad-rational-contacts-") as directory:
        binary = Path(directory)/"probe"
        command = [compiler, "-O2", "-std=c++17", "-I", str(include), str(ROOT/"modules/kernel-occt/diagnostics/rational_contacts.cpp"), "-L", str(lib), "-Wl,--start-group"]
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
    print(f"rational contact edges/wires measured; product unsupported; report={output}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--occt-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        run(args.occt_root.resolve(), args.output.resolve())
    except (KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f"rational contact diagnostic failed: {error}\n")


if __name__ == "__main__": main()
