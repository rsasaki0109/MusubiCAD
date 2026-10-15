"""ADR-079: closed native diagnostic approximations, not product fillet admission."""
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

ALLOWANCE_M = 1e-12
MASS_ALLOWANCE_M3 = 1e-15  # Comparison/stability allowance, not certified native error.
MOMENT_ALLOWANCE_M4 = 1e-17
COM_ALLOWANCE_M = 1e-10
AREA_ALLOWANCE_M2 = 1e-12  # Native comparison, not a certified native area error.
NAMES = ("constant-coarse", "constant-fine", "variable-coarse", "variable-fine")


def grade(reference: dict, native: dict) -> dict:
    for report in (reference, native):
        require(type(report.get("format_version")) is int and report["format_version"] == 1 and len(report["cases"]) == 4, "missing or invalid closed fixtures")
    require(native["occt_version"] == "8.0.0", "wrong native SDK")
    require(abs(number(reference["radius_m"])-0.01) < 1e-15 and abs(number(reference["height_m"])-0.02) < 1e-15, "wrong reference stock")
    findings = []
    for name, ref, actual in zip(NAMES, reference["cases"], native["cases"]):
        require(ref["name"] == name and ref["law"] == name.split("-")[0], "wrong law or fixture order")
        n = ref["divisions"]
        require(type(n) is int and 1 <= n <= 32 and n & (n-1) == 0, "invalid complete band grid")
        count, skirts = 4*n*n, 4*n
        require(len(ref["cells"]) == count, "incomplete reference band")
        requested = 1e-4 if name.endswith("coarse") else 2e-5
        require(abs(number(ref["requested_position_error_m"])-requested) < 1e-20 and 0 < number(ref["position_error_upper_m"]) <= requested and 0 < number(ref["normal_error_upper"]) <= 0.8, "uncertified band quality")
        for index, cell in enumerate(ref["cells"]):
            chart, k = divmod(index, n*n)
            i, j = divmod(k, n)
            require(cell["chart"] == chart and cell["chart_domain"] == [i/n, (i+1)/n, j/n, (j+1)/n], "wrong band coverage")
        ideal = ref["ideal"]
        require(abs(number(ideal["minimum_radius_m"])-0.0005) < 1e-15 and abs(number(ideal["maximum_radius_m"])-(0.0005 if ref["law"] == "constant" else 0.0015)) < 1e-15, "wrong ideal radius law")
        lo, hi = number(ideal["body_lower_m3"]), number(ideal["body_upper_m3"])
        require(0 < lo <= hi and hi-lo <= number(ideal["absolute_width_m3"]) <= 1e-12, "invalid ideal volume enclosure")
        require(type(ideal["cells"]) is int and (ideal["cells"] == 0 if ref["law"] == "constant" else 4 <= ideal["cells"] <= 262144), "wrong ideal integration path")
        if ref["law"] == "constant":
            r = 0.0005
            analytic = math.pi*0.01**2*0.02-2*math.pi*((0.01-r)*(1-math.pi/4)*r*r+r**3/6)
            require(lo-1e-20 <= analytic <= hi+1e-20, "independent constant-volume control failed")
        for field, expected in (("faces", count+skirts+2), ("vertices", 4*n*(n+2)), ("edges", 4*n*(2*n+3)), ("shells", 1), ("solids", 1), ("free_edges", 0), ("euler", 2), ("planar_caps", 2)):
            require(type(actual[field]) is int and actual[field] == expected, f"wrong closed {field}")
        require(all(actual[f] is True for f in ("brep_valid", "shell_closed", "face_connected", "edge_incidence_two", "cap_normals_outward")), "unclosed, misoriented or invalid native solid")
        volume, delta = number(actual["volume_m3"]), number(actual["volume_refinement_delta_m3"])
        require(0 < volume < math.pi*0.01**2*0.02 and 0 <= delta <= MASS_ALLOWANCE_M3, "invalid or unstable native volume")
        integrals = ref["boundary_integrals"]
        def enclosure(value, max_width):
            require(isinstance(value, list) and len(value) == 2, "missing boundary integral interval")
            lower, upper = map(number, value)
            require(lower <= upper and upper-lower <= max_width, "invalid or unresolved boundary integral")
            return lower, upper
        vlo, vhi = enclosure(integrals["volume_m3"], MASS_ALLOWANCE_M3)
        require(vlo > 0 and vlo-MASS_ALLOWANCE_M3 <= volume <= vhi+MASS_ALLOWANCE_M3, "native volume outside independent boundary comparison")
        moments, center = vector(actual["first_moments_m4"]), vector(actual["center_of_mass_m"])
        require(len(integrals["first_moments_m4"]) == 3, "missing moment enclosures")
        center_bounds = []
        for axis, interval in enumerate(integrals["first_moments_m4"]):
            lower, upper = enclosure(interval, MOMENT_ALLOWANCE_M4)
            require(lower-MOMENT_ALLOWANCE_M4 <= moments[axis] <= upper+MOMENT_ALLOWANCE_M4, "native moment outside independent boundary comparison")
            require(abs(moments[axis]-volume*center[axis]) <= 1e-22, "inconsistent native mass/center/moments")
            quotients = [m/v for m in (lower, upper) for v in (vlo, vhi)]
            clo, chi = math.nextafter(min(quotients), -math.inf), math.nextafter(max(quotients), math.inf)
            require(clo-COM_ALLOWANCE_M <= center[axis] <= chi+COM_ALLOWANCE_M, "native center outside independent boundary comparison")
            center_bounds.append([clo, chi])
        support_error = number(actual["max_sampled_skirt_cylinder_error_m"])
        require(ALLOWANCE_M < support_error <= ref["position_error_upper_m"]+ALLOWANCE_M, "skirt approximation distinction missing or changed")
        surfaces = actual["surface_cells"]
        expected_ids = {(i, -1) for i in range(count)} | {(-1, i) for i in range(skirts)}
        require(len(surfaces) == count+skirts and {(s["band_cell"], s["skirt_index"]) for s in surfaces} == expected_ids, "missing or duplicate sewn surfaces")
        area_bounds = ref["curved_area_bounds"]
        alo, ahi = number(area_bounds["lower_m2"]), number(area_bounds["upper_m2"])
        require(number(area_bounds["absolute_width_m2"]) == 1e-7 and type(area_bounds["max_leaves"]) is int and area_bounds["max_leaves"] == 32768, "wrong area precision/work request")
        require(0 < alo <= ahi and ahi-alo <= 1e-7 and type(area_bounds["leaves"]) is int and count+skirts <= area_bounds["leaves"] <= 32768, "invalid or unresolved curved area enclosure")
        native_area = number(actual["curved_surface_area_m2"])
        require(alo-AREA_ALLOWANCE_M2 <= native_area <= ahi+AREA_ALLOWANCE_M2, "native curved area outside independent enclosure comparison")
        face_areas = [number(s["area_m2"]) for s in surfaces]
        require(all(a > 0 for a in face_areas) and abs(sum(face_areas)-native_area) <= AREA_ALLOWANCE_M2, "inconsistent native curved face areas")
        cap_areas = actual["planar_cap_areas_m2"]
        require(isinstance(cap_areas, list) and len(cap_areas) == 2 and all(number(a) > 0 for a in cap_areas), "missing native cap area inventory")
        require(abs(number(actual["total_surface_area_m2"])-native_area-sum(cap_areas)) <= AREA_ALLOWANCE_M2, "inconsistent native total/curved/cap area inventory")
        max_error = 0.
        for surface in surfaces:
            cell, skirt = surface["band_cell"], surface["skirt_index"]
            require(type(cell) is int and type(skirt) is int, "invalid surface indices")
            if cell >= 0:
                poles = ref["cells"][cell]["control_points_m"]
            else:
                band_poles = ref["cells"][skirt*n]["control_points_m"]
                poles = [[[band_poles[i][0][0], band_poles[i][0][1], band_poles[i][0][2]*j/3] for j in range(4)] for i in range(4)]
            require(len(surface["samples"]) == 9, "missing native surface witnesses")
            for index, sample in enumerate(surface["samples"]):
                u, v = index//3/2, index%3/2
                require(number(sample["u"]) == u and number(sample["v"]) == v, "wrong native parameters")
                error = distance(sample["point_m"], bernstein(poles, u, v)[0])
                require(error <= ALLOWANCE_M, "native closed surface evaluation mismatch")
                max_error = max(max_error, error)
        findings.append({"name": name, "divisions": n, "native_volume_m3": volume, "polynomial_boundary_volume_m3": [vlo, vhi],
                         "polynomial_boundary_first_moments_m4": integrals["first_moments_m4"], "polynomial_boundary_center_m": center_bounds,
                         "curved_area_enclosure_m2": [alo, ahi], "native_curved_area_m2": native_area, "area_leaves": area_bounds["leaves"],
                         "native_first_moments_m4": moments, "native_center_of_mass_m": center,
                         "native_minus_ideal_interval_m3": [math.nextafter(volume-hi, -math.inf), math.nextafter(volume-lo, math.inf)],
                         "distance_to_ideal_interval_m3": max(lo-volume, volume-hi, 0.),
                         "max_sampled_surface_error_m": max_error, "sampled_skirt_cylinder_error_m": support_error})
    for coarse_index in (0, 2):
        coarse, fine = findings[coarse_index:coarse_index+2]
        require(reference["cases"][coarse_index]["ideal"] == reference["cases"][coarse_index+1]["ideal"], "inconsistent ideal reference between levels")
        require(fine["divisions"] > coarse["divisions"], "missing actual refinement")
        coarse_gap, fine_gap = coarse["distance_to_ideal_interval_m3"], fine["distance_to_ideal_interval_m3"]
        improves = fine_gap < coarse_gap-MASS_ALLOWANCE_M3 if coarse_gap > MASS_ALLOWANCE_M3 else fine_gap <= coarse_gap+MASS_ALLOWANCE_M3
        require(improves and fine["sampled_skirt_cylinder_error_m"] < coarse["sampled_skirt_cylinder_error_m"], "native approximation refinement changed")
    return {"diagnostic_passed": True, "candidate_admitted": False, "product_body_created": False,
            "native_diagnostic_solids_created": 4, "evaluation_allowance_m": ALLOWANCE_M, "mass_comparison_allowance_m3": MASS_ALLOWANCE_M3, "moment_comparison_allowance_m4": MOMENT_ALLOWANCE_M4, "center_comparison_allowance_m": COM_ALLOWANCE_M, "area_comparison_allowance_m2": AREA_ALLOWANCE_M2, "findings": findings}


def run(occt_root: Path, output: Path) -> None:
    require(sys.platform.startswith("linux"), "pinned Linux SDK required")
    compiler = shutil.which("c++")
    require(compiler is not None, "C++17 compiler required")
    include, lib = occt_root/"include/opencascade", occt_root/"lib"
    libraries = ["TKTopAlgo", "TKShHealing", "TKGeomAlgo", "TKBRep", "TKGeomBase", "TKG3d", "TKG2d", "TKMath", "TKernel"]
    require((include/"BRepBuilderAPI_MakeSolid.hxx").is_file() and all((lib/f"lib{x}.a").is_file() for x in libraries), "prepared native SDK missing")
    reference = json.loads(subprocess.run(["cargo", "run", "--quiet", "-p", "opencad-geometry", "--example", "closed_periodic_reference"], check=True, cwd=ROOT, capture_output=True, text=True, timeout=300).stdout)
    values = ["4"]
    for case in reference["cases"]:
        values.append(str(case["divisions"]))
        values.extend(format(x, ".17g") for c in case["cells"] for row in c["control_points_m"] for p in row for x in p)
    with tempfile.TemporaryDirectory(prefix="musubicad-closed-reference-") as directory:
        binary = Path(directory)/"probe"
        command = [compiler, "-O2", "-std=c++17", "-I", str(include), str(ROOT/"modules/kernel-occt/diagnostics/closed_reference.cpp"), "-L", str(lib), "-Wl,--start-group"]
        command += [f"-l{x}" for x in libraries]+["-Wl,--end-group", "-lpthread", "-ldl", "-o", str(binary)]
        subprocess.run(command, check=True, cwd=ROOT, timeout=180)
        native = json.loads(subprocess.run([str(binary)], input="\n".join(values)+"\n", check=True, capture_output=True, text=True, timeout=240).stdout)
    report = {"format_version": 1, "diagnostic_passed": False, "candidate_admitted": False, "reference": reference, "native": native}
    try:
        report.update(grade(reference, native))
    except (KeyError, TypeError, ValueError) as error:
        report["error"] = str(error)
        raise
    finally:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, indent=2, allow_nan=False)+"\n", encoding="utf-8")
    print(f"closed native diagnostic approximations measured; product unsupported; report={output}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--occt-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        run(args.occt_root.resolve(), args.output.resolve())
    except (KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f"closed native diagnostic failed: {error}\n")


if __name__ == "__main__": main()
