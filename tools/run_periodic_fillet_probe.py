"""ADR-074 Linux SDK diagnostic; passing reproduces rejection, never admission."""
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
NAMES = {"constant", "baseline", "dense", "tight", "c2", "wrong_phase"}
ERRORS = (
    "max_sampled_top_error_m", "max_sampled_side_error_m",
    "max_sampled_support_error_m", "max_sampled_radius_error_m",
)
CONTACT_THRESHOLD_M = 1e-8  # Counterexample threshold, not production tolerance.
VOLUME_ALLOWANCE_M3 = 1e-15  # Comparison allowance, not certified native error.


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def number(value: object, name: str, positive: bool = False) -> float:
    require(type(value) in (int, float), f"{name}: expected a numeric SI value")
    require(math.isfinite(value), f"{name}: nonfinite measurement")
    require(value > 0 if positive else value >= 0, f"{name}: invalid sign")
    return float(value)


def grade(native: dict, reference: dict) -> dict:
    """Validate controls and measured counterexamples; synthetic tests are parser tests."""
    for name, report in (("native", native), ("reference", reference)):
        require(isinstance(report, dict), f"{name}: expected object")
        require(type(report.get("format_version")) is int and report["format_version"] == 1, f"{name}: unsupported format")
        for field, expected in (("radius_m", 0.01), ("height_m", 0.02)):
            value = number(report[field], f"{name}.{field}", True)
            require(abs(value - expected) <= 1e-15, f"{name}: wrong fixture {field}")
    require(native["occt_version"] == "8.0.0", "wrong pinned OCCT version")
    width = number(reference["absolute_width_m3"], "absolute_width_m3", True)
    require(abs(width - 1e-12) <= 1e-27, "wrong requested reference width")
    require(set(reference["references"]) == {"constant", "variable"}, "missing reference cases")
    for name, radii in (("constant", (0.001, 0.001)), ("variable", (0.0005, 0.0015))):
        ref = reference["references"][name]
        for field, expected in zip(("minimum_radius_m", "maximum_radius_m"), radii):
            require(abs(number(ref[field], field, True) - expected) <= 1e-15, "wrong reference radii")
        lo = number(ref["body_lower_m3"], "body_lower_m3", True)
        hi = number(ref["body_upper_m3"], "body_upper_m3", True)
        require(lo <= hi and hi - lo <= width, "invalid or insufficient reference enclosure")
        cells = ref["cells"]
        require(type(cells) is int and (cells == 0 if name == "constant" else 4 <= cells <= 262144), "wrong reference integration path")
    profiles = native["profiles"]
    require(isinstance(profiles, list) and len(profiles) == len(NAMES), "incomplete profiles")
    require({p["name"] for p in profiles} == NAMES, "duplicate, missing or unknown profile")
    rows = {p["name"]: p for p in profiles}
    findings = {}
    for name, row in rows.items():
        require(row["brep_valid"] is True, f"{name}: invalid native BREP")
        require(type(row["solids"]) is int and row["solids"] == 1, f"{name}: not one actual solid")
        for field, minimum in (("top_points", 257), ("side_points", 257), ("interior_points", 100)):
            require(type(row[field]) is int and row[field] >= minimum, f"{name}: missing physical {field}")
        volume = number(row["native_volume_m3"], f"{name}.native_volume_m3", True)
        require(volume <= math.pi * 0.01**2 * 0.02 + VOLUME_ALLOWANCE_M3, f"{name}: volume exceeds cylinder stock; check units")
        delta = number(row["volume_refinement_delta_m3"], f"{name}.refinement_delta")
        require(delta <= VOLUME_ALLOWANCE_M3, f"{name}: unstable volume estimate")
        errors = {key: number(row[key], f"{name}.{key}") for key in ERRORS}
        law_error = number(row["max_sampled_law_error_m"], f"{name}.law_error")
        require(type(row["law_points"]) is int, f"{name}: invalid law count")
        if name == "constant":
            require(row["law_points"] == 0, "constant law API must not be queried")
            require(all(e <= CONTACT_THRESHOLD_M for e in errors.values()), "constant control geometry failed")
            ref = reference["references"]["constant"]
            require(ref["body_lower_m3"] - VOLUME_ALLOWANCE_M3 <= volume <= ref["body_upper_m3"] + VOLUME_ALLOWANCE_M3, "constant volume control failed")
            findings[name] = {"measured_control_matches": True}
        else:
            require(row["law_points"] >= 513 and law_error <= CONTACT_THRESHOLD_M, f"{name}: stored-law control failed")
            violations = [key for key, error in errors.items() if error > CONTACT_THRESHOLD_M]
            require(len(violations) == len(ERRORS), f"{name}: geometric counterexample changed; investigate admission separately")
            ref = reference["references"]["variable"]
            outside = volume < ref["body_lower_m3"] - VOLUME_ALLOWANCE_M3 or volume > ref["body_upper_m3"] + VOLUME_ALLOWANCE_M3
            require(outside, f"{name}: volume discrepancy changed; review pinned recipe")
            findings[name] = {"sampled_counterexamples": violations, "volume_estimate_outside_reference": True}
    wrong, baseline = rows["wrong_phase"], rows["baseline"]
    require(abs(wrong["native_volume_m3"] - baseline["native_volume_m3"]) <= VOLUME_ALLOWANCE_M3, "rotated volume control failed")
    require(wrong["max_sampled_top_error_m"] > 1e-4 and wrong["max_sampled_side_error_m"] > 1e-4, "wrong-phase physical control missing")
    return {"diagnostic_passed": True, "candidate_admitted": False, "findings": findings}


def run(occt_root: Path, output: Path) -> None:
    require(sys.platform.startswith("linux"), "this diagnostic requires the pinned Linux SDK")
    compiler = shutil.which("c++")
    require(compiler is not None, "C++17 compiler not found")
    libraries = ["TKFillet", "TKOffset", "TKBool", "TKBO", "TKPrim", "TKTopAlgo", "TKShHealing", "TKGeomAlgo", "TKBRep", "TKGeomBase", "TKG3d", "TKG2d", "TKMath", "TKernel"]
    include, lib = occt_root / "include/opencascade", occt_root / "lib"
    require((include / "BRepFilletAPI_MakeFillet.hxx").is_file(), "prepared OCCT headers missing")
    require(all((lib / f"lib{name}.a").is_file() for name in libraries), "prepared OCCT static libraries missing")
    with tempfile.TemporaryDirectory(prefix="musubicad-periodic-probe-") as directory:
        binary = Path(directory) / "probe"
        command = [compiler, "-O2", "-std=c++17", "-I", str(include), str(ROOT / "modules/kernel-occt/diagnostics/periodic_fillet.cpp"), "-L", str(lib), "-Wl,--start-group"]
        command += [f"-l{name}" for name in libraries] + ["-Wl,--end-group", "-lpthread", "-ldl", "-o", str(binary)]
        subprocess.run(command, check=True, cwd=ROOT, timeout=180)
        native = json.loads(subprocess.run([str(binary)], check=True, capture_output=True, text=True, timeout=120).stdout)
    reference = json.loads(subprocess.run(["cargo", "run", "--quiet", "-p", "opencad-geometry", "--example", "periodic_fillet_volume_bounds"], check=True, cwd=ROOT, capture_output=True, text=True, timeout=180).stdout)
    report = {"format_version": 1, "diagnostic_passed": False, "candidate_admitted": False, "native": native, "reference": reference}
    try:
        report.update(grade(native, reference))
    except (KeyError, TypeError, ValueError) as error:
        report["error"] = str(error)
        raise
    finally:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(f"periodic native diagnostic: controls/counterexamples reproduced; candidate remains unsupported; report={output}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--occt-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        run(args.occt_root.resolve(), args.output.resolve())
    except (KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f"periodic native diagnostic failed: {error}\n")


if __name__ == "__main__":
    main()
