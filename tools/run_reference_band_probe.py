"""ADR-078: actual native sewing of complete bounded reference bands, not solids."""
from __future__ import annotations
import argparse
import json
import math
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from run_reference_bezier_probe import ROOT, bernstein, distance, number, require, vector

ALLOWANCE_M = 1e-12  # Native conversion/evaluation test allowance, not model quality.


def grade(reference: dict, native: dict) -> dict:
    for report in (reference, native):
        require(type(report.get("format_version")) is int and report["format_version"] == 1, "wrong report format")
        require(len(report["cases"]) == 2, "missing constant or variable band")
    require(native["occt_version"] == "8.0.0", "wrong native SDK")
    position, normal = number(reference["requested_position_error_m"]), number(reference["requested_normal_error"])
    budget = reference["max_patches"]
    require(position > 0 and 0 < normal <= 2 and type(budget) is int and 4 <= budget <= 16384, "invalid quality request")
    findings = []
    for name, ref, actual in zip(("constant", "variable"), reference["cases"], native["cases"]):
        require(ref["name"] == name, "wrong band fixture order")
        n = ref["divisions"]
        require(type(n) is int and n > 0 and n & (n-1) == 0, "non-dyadic cover")
        count = 4*n*n
        require(count <= budget and len(ref["cells"]) == count, "incomplete or excessive coverage")
        require(0 <= number(ref["position_error_upper_m"]) <= position and 0 <= number(ref["normal_error_upper"]) <= normal, "unmet band quality")
        for index, cell in enumerate(ref["cells"]):
            chart, remainder = divmod(index, n*n)
            i, j = divmod(remainder, n)
            require(cell["chart"] == chart and cell["chart_domain"] == [i/n, (i+1)/n, j/n, (j+1)/n], "wrong cell coverage/order")
            require(0 <= number(cell["position_error_upper_m"]) <= ref["position_error_upper_m"] and 0 <= number(cell["normal_error_upper"]) <= ref["normal_error_upper"], "uncertified cell")
        for field, expected in (("faces", count), ("vertices", 4*n*(n+1)), ("edges", 4*n*(2*n+1)), ("shells", 1), ("solids", 0), ("boundary_edges", 8*n), ("euler", 0)):
            require(type(actual[field]) is int and actual[field] == expected, f"wrong native {field}")
        require(actual["brep_valid"] is True and actual["boundary_degree_two"] is True and actual["edge_incidence_valid"] is True and actual["face_connected"] is True, "invalid native band topology")
        require(actual["boundary_cycles"] == [4*n, 4*n], "wrong native boundary cycles")
        require(abs(number(actual["sewing_tolerance_m"])-1e-9) <= 1e-24, "wrong native sewing tolerance")
        require(0 <= number(actual["max_boundary_support_error_m"]) <= position+ALLOWANCE_M, "native support mismatch")
        surfaces = actual["surface_cells"]
        require(len(surfaces) == count and {s["cell"] for s in surfaces} == set(range(count)), "missing or duplicate native face witnesses")
        max_error = 0.
        for surface in surfaces:
            require(type(surface["cell"]) is int, "invalid native cell index")
            poles = ref["cells"][surface["cell"]]["control_points_m"]
            require(len(poles) == 4 and len(surface["poles_m"]) == 4, "wrong native pole net")
            for row, measured in zip(poles, surface["poles_m"]):
                require(len(row) == 4 and len(measured) == 4, "wrong native pole row")
                for p, q in zip(row, measured):
                    require(distance(p, q) <= ALLOWANCE_M, "native poles changed")
            require(len(surface["samples"]) == 9, "missing native surface witnesses")
            for index, sample in enumerate(surface["samples"]):
                u, v = index//3/2, index%3/2
                require(number(sample["u"]) == u and number(sample["v"]) == v, "wrong native witness parameters")
                p = bernstein(poles, u, v)[0]
                error = distance(sample["point_m"], p)
                require(error <= ALLOWANCE_M, "native surface evaluation mismatch")
                max_error = max(max_error, error)
        loops = actual["boundary_loops_m"]
        require(len(loops) == 2, "missing boundary geometry")
        classes = []
        for loop in loops:
            require(len(loop) == 4*n, "incomplete boundary vertex cycle")
            labels = set()
            for p in loop:
                x, y, z = vector(p)
                rho = math.hypot(x, y)
                require(rho > 0, "invalid contact radius")
                radius = 0.0005 if name == "constant" else 0.001+0.0005*math.cos(math.atan2(y, x))
                side = abs(rho-0.01) < abs(z-0.02)
                expected = [0.01*x/rho, 0.01*y/rho, 0.02-radius] if side else [(0.01-radius)*x/rho, (0.01-radius)*y/rho, 0.02]
                require(distance(p, expected) <= ref["position_error_upper_m"]+ALLOWANCE_M, "native contact/phase mismatch")
                labels.add("side" if side else "top")
            require(len(labels) == 1, "mixed support boundary")
            classes.extend(labels)
        require(set(classes) == {"side", "top"}, "missing distinct support boundaries")
        findings.append({"name": name, "faces": count, "divisions": n, "max_sampled_native_evaluation_error_m": max_error})
    return {"diagnostic_passed": True, "candidate_admitted": False, "solid_created": False, "evaluation_allowance_m": ALLOWANCE_M, "findings": findings}


def run(occt_root: Path, output: Path) -> None:
    require(sys.platform.startswith("linux"), "pinned Linux SDK required")
    compiler = shutil.which("c++")
    require(compiler is not None, "C++17 compiler required")
    include, lib = occt_root/"include/opencascade", occt_root/"lib"
    libraries = ["TKTopAlgo", "TKShHealing", "TKGeomAlgo", "TKBRep", "TKGeomBase", "TKG3d", "TKG2d", "TKMath", "TKernel"]
    require((include/"BRepBuilderAPI_Sewing.hxx").is_file() and all((lib/f"lib{x}.a").is_file() for x in libraries), "prepared SDK missing")
    reference = json.loads(subprocess.run(["cargo", "run", "--quiet", "-p", "opencad-geometry", "--example", "periodic_fillet_bezier_bands"], check=True, cwd=ROOT, capture_output=True, text=True, timeout=180).stdout)
    values = ["2"]
    for case in reference["cases"]:
        values.append(str(len(case["cells"])))
        values.extend(format(x, ".17g") for c in case["cells"] for row in c["control_points_m"] for p in row for x in p)
    with tempfile.TemporaryDirectory(prefix="musubicad-reference-band-") as directory:
        binary = Path(directory)/"probe"
        command = [compiler, "-O2", "-std=c++17", "-I", str(include), str(ROOT/"modules/kernel-occt/diagnostics/reference_band.cpp"), "-L", str(lib), "-Wl,--start-group"]
        command += [f"-l{x}" for x in libraries]+["-Wl,--end-group", "-lpthread", "-ldl", "-o", str(binary)]
        subprocess.run(command, check=True, cwd=ROOT, timeout=180)
        native = json.loads(subprocess.run([str(binary)], input="\n".join(values)+"\n", check=True, capture_output=True, text=True, timeout=180).stdout)
    report = {"format_version": 1, "diagnostic_passed": False, "candidate_admitted": False, "solid_created": False, "reference": reference, "native": native}
    try:
        report.update(grade(reference, native))
    except (KeyError, TypeError, ValueError) as error:
        report["error"] = str(error)
        raise
    finally:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, indent=2, allow_nan=False)+"\n", encoding="utf-8")
    print(f"native periodic reference bands sewn and measured; no solid admitted; report={output}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--occt-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        run(args.occt_root.resolve(), args.output.resolve())
    except (KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f"native band diagnostic failed: {error}\n")


if __name__ == "__main__":
    main()
