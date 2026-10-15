"""ADR-076: measure actual bounded-reference Bezier surfaces, not CAD admission."""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
EVALUATION_ALLOWANCE_M = 1e-14
NORMAL_ALLOWANCE = 1e-11  # Dimensionless native evaluation test allowance.
AREA_ALLOWANCE_M2 = 1e-24
NAMES = {f"{law}-{chart}-{region}" for law in ("constant", "variable")
         for chart in range(4) for region in ("side", "interior", "top", "seam-start", "seam-end")}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def number(x: object) -> float:
    require(type(x) in (int, float) and math.isfinite(x), "nonfinite/non-numeric measurement")
    return float(x)


def vector(x: object) -> list[float]:
    require(isinstance(x, list) and len(x) == 3, "expected xyz vector")
    return [number(y) for y in x]


def distance(a: list, b: list) -> float:
    return math.sqrt(sum((x-y)**2 for x, y in zip(vector(a), vector(b))))


def unit_cross(du: list, dv: list) -> tuple[list, float]:
    w = [du[(i+1)%3]*dv[(i+2)%3]-du[(i+2)%3]*dv[(i+1)%3] for i in range(3)]
    area = math.sqrt(sum(x*x for x in w))
    require(math.isfinite(area) and area > 0, "degenerate Bernstein normal")
    return [x/area for x in w], area


def bernstein(poles: list, u: float, v: float) -> tuple[list, list, list]:
    """Independent polynomial evaluator and analytic local derivatives."""
    def basis(x):
        return [(1-x)**3, 3*x*(1-x)**2, 3*x*x*(1-x), x**3]
    def quadratic(x):
        return [(1-x)**2, 2*x*(1-x), x*x]
    a, b, da, db = basis(u), basis(v), quadratic(u), quadratic(v)
    p = [sum(a[i]*b[j]*poles[i][j][k] for i in range(4) for j in range(4)) for k in range(3)]
    du = [sum(3*da[i]*b[j]*(poles[i+1][j][k]-poles[i][j][k]) for i in range(3) for j in range(4)) for k in range(3)]
    dv = [sum(3*a[i]*db[j]*(poles[i][j+1][k]-poles[i][j][k]) for i in range(4) for j in range(3)) for k in range(3)]
    return p, du, dv


def grade(reference: dict, native: dict) -> dict:
    for report in (reference, native):
        require(isinstance(report, dict) and type(report.get("format_version")) is int and report["format_version"] == 1, "wrong report format")
        require(isinstance(report["cases"], list) and len(report["cases"]) == len(NAMES), "missing fixture cases")
    require(native["occt_version"] == "8.0.0", "wrong OCCT SDK")
    require({row["name"] for row in reference["cases"]} == NAMES, "missing or duplicate reference case")
    findings = []
    for ref, actual in zip(reference["cases"], native["cases"]):
        bound = number(ref["position_error_upper_m"])
        base = number(ref["reference_to_bilinear_error_upper_m"])
        delta = number(ref["control_net_deviation_upper_m"])
        require(0 <= base and 0 <= delta and base+delta <= bound < 1e-6, "invalid positional bound")
        area_lo, area_hi = number(ref["jacobian_area_lower_m2"]), number(ref["jacobian_area_upper_m2"])
        alignment, normal_error = number(ref["ideal_normal_alignment_lower"]), number(ref["normal_error_upper"])
        nlo, nhi = vector(ref["normal_lower"]), vector(ref["normal_upper"])
        require(0 < area_lo <= area_hi and 0 < alignment <= 1 and 0 <= normal_error < 0.1 and all(a <= b for a, b in zip(nlo, nhi)), "invalid normal certificate")
        require(type(actual["u_degree"]) is int and actual["u_degree"] == 3 and type(actual["v_degree"]) is int and actual["v_degree"] == 3, "wrong native degrees")
        poles = ref["control_points_m"]
        require(isinstance(poles, list) and len(poles) == 4 and all(isinstance(row, list) and len(row) == 4 for row in poles), "wrong control net size")
        for row in poles:
            for pole in row:
                vector(pole)
        # Exact data transfer check, not a geometric equality comparison.
        require(actual["poles_m"] == poles, "native control coordinates changed")
        require(len(actual["samples"]) == 25 and len(ref["samples"]) == 25, "missing surface witnesses")
        max_error, max_normal_error = 0., 0.
        for index, (expected, measured) in enumerate(zip(ref["samples"], actual["samples"])):
            u, v = index//5/4, index%5/4
            require(number(measured["u"]) == u and number(measured["v"]) == v and number(expected["u"]) == u and number(expected["v"]) == v, "wrong witness parameters")
            p, du, dv = bernstein(poles, u, v)
            for field, value in (("point_m", p), ("du_m", du), ("dv_m", dv)):
                require(distance(measured[field], value) <= EVALUATION_ALLOWANCE_M, "native Bernstein evaluation mismatch")
            error = distance(measured["point_m"], expected["reference_point_m"])
            require(error <= bound + EVALUATION_ALLOWANCE_M, "native surface outside reference bound")
            max_error = max(max_error, error)
            expected_normal, expected_area = unit_cross(du, dv)
            n, ideal_n = vector(measured["normal"]), vector(expected["reference_normal"])
            area = number(measured["jacobian_area_m2"])
            require(abs(sum(x*x for x in n)-1) <= NORMAL_ALLOWANCE and abs(sum(x*x for x in ideal_n)-1) <= NORMAL_ALLOWANCE, "nonunit normal")
            require(distance(n, expected_normal) <= NORMAL_ALLOWANCE, "native Bernstein normal mismatch")
            require(abs(area-expected_area) <= expected_area*NORMAL_ALLOWANCE+AREA_ALLOWANCE_M2, "native area mismatch")
            require(area_lo-AREA_ALLOWANCE_M2 <= area <= area_hi+AREA_ALLOWANCE_M2 and area > 0, "native area outside certificate")
            require(all(lo-NORMAL_ALLOWANCE <= x <= hi+NORMAL_ALLOWANCE for x, lo, hi in zip(n, nlo, nhi)), "native normal outside component bounds")
            require(sum(a*b for a, b in zip(n, ideal_n))+NORMAL_ALLOWANCE >= alignment, "native orientation mismatch")
            normal_delta = distance(n, ideal_n)
            require(normal_delta <= normal_error+NORMAL_ALLOWANCE, "native normal outside reference bound")
            max_normal_error = max(max_normal_error, normal_delta)
        findings.append({"name": ref["name"], "position_error_upper_m": bound,
                         "max_sampled_native_reference_error_m": max_error,
                         "normal_error_upper": normal_error, "max_sampled_normal_error": max_normal_error})
    rows = {ref["name"]: actual for ref, actual in zip(reference["cases"], native["cases"])}
    certificates = {ref["name"]: ref for ref in reference["cases"]}
    seam_findings = []
    for law in ("constant", "variable"):
        for chart in range(4):
            end = rows[f"{law}-{chart}-seam-end"]
            start = rows[f"{law}-{(chart+1)%4}-seam-start"]
            require(end["poles_m"][3] == start["poles_m"][0], "shared native seam poles changed")
            end_ref = certificates[f"{law}-{chart}-seam-end"]
            start_ref = certificates[f"{law}-{(chart+1)%4}-seam-start"]
            gap_bound = math.nextafter(end_ref["normal_error_upper"]+start_ref["normal_error_upper"], math.inf)
            max_gap = 0.
            for last, first in zip(end["samples"][20:25], start["samples"][0:5]):
                require(distance(last["point_m"], first["point_m"]) <= EVALUATION_ALLOWANCE_M, "native seam positions disagree")
                gap = distance(last["normal"], first["normal"])
                require(gap <= gap_bound+2*NORMAL_ALLOWANCE, "native seam normal gap outside bound")
                max_gap = max(max_gap, gap)
            seam_findings.append({"name": f"{law}-{chart}", "normal_gap_upper": gap_bound, "max_sampled_normal_gap": max_gap})
    return {"diagnostic_passed": True, "candidate_admitted": False,
            "evaluation_allowance_m": EVALUATION_ALLOWANCE_M, "normal_evaluation_allowance": NORMAL_ALLOWANCE,
            "area_evaluation_allowance_m2": AREA_ALLOWANCE_M2, "findings": findings, "seams": seam_findings}


def run(occt_root: Path, output: Path) -> None:
    require(sys.platform.startswith("linux"), "pinned Linux SDK required")
    compiler = shutil.which("c++")
    require(compiler is not None, "C++17 compiler required")
    include, lib = occt_root/"include/opencascade", occt_root/"lib"
    require((include/"Geom_BezierSurface.hxx").is_file(), "prepared OCCT headers missing")
    libraries = ["TKG3d", "TKG2d", "TKMath", "TKernel"]
    require(all((lib/f"lib{name}.a").is_file() for name in libraries), "prepared OCCT libraries missing")
    reference = json.loads(subprocess.run(["cargo", "run", "--quiet", "-p", "opencad-geometry", "--example", "periodic_fillet_bezier_patches"], check=True, cwd=ROOT, capture_output=True, text=True, timeout=180).stdout)
    values = [str(len(NAMES))]
    for case in reference["cases"]:
        values.extend(format(x, ".17g") for row in case["control_points_m"] for p in row for x in p)
    with tempfile.TemporaryDirectory(prefix="musubicad-reference-bezier-") as directory:
        binary = Path(directory)/"probe"
        command = [compiler, "-O2", "-std=c++17", "-I", str(include), str(ROOT/"modules/kernel-occt/diagnostics/reference_bezier.cpp"), "-L", str(lib), "-Wl,--start-group"]
        command += [f"-l{name}" for name in libraries]+["-Wl,--end-group", "-lpthread", "-ldl", "-o", str(binary)]
        subprocess.run(command, check=True, cwd=ROOT, timeout=180)
        native = json.loads(subprocess.run([str(binary)], input="\n".join(values)+"\n", check=True, text=True, capture_output=True, timeout=60).stdout)
    report = {"format_version": 1, "diagnostic_passed": False, "candidate_admitted": False, "reference": reference, "native": native}
    try:
        report.update(grade(reference, native))
    except (KeyError, TypeError, ValueError) as error:
        report["error"] = str(error)
        raise
    finally:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, indent=2, allow_nan=False)+"\n", encoding="utf-8")
    print(f"native Bezier reference diagnostic passed; no solid admitted; report={output}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--occt-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        run(args.occt_root.resolve(), args.output.resolve())
    except (KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f"native Bezier diagnostic failed: {error}\n")


if __name__ == "__main__":
    main()
