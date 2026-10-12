"""Evaluate an MCP agent host authoring MusubiCAD parts (MCAD-P7-012).

Each task in ``tools/mcp_eval_tasks.json`` runs Claude Code headless against
``musubicad mcp`` with only the MusubiCAD tools allowed.  The result is then
checked independently by regenerating the document with ``musubicad regen``:

- the solid volume must match the analytic value;
- the named parameters must exist;
- for assembly tasks, the instance and mate counts must match and the mates
  must be satisfied;
- for drawing tasks, the view projections and dimension lengths must match,
  and the drawing must export to SVG.
- for robot tasks, declared joints must match, URDF export must contain the
  expected joints, and a fresh reach query must meet the target residual.

The harness records turns, cost, and duration for each task.  It calls a
paid model, so it is never run in CI.  Run it by hand after changes to the
MCP surface or the authoring guide:

    python tools/mcp_eval.py --musubicad target/debug/musubicad.exe
    python tools/mcp_eval.py --only enclosure --out eval.json
    python tools/mcp_eval.py --self-test   # checker only, no model calls
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
TASKS = ROOT / "tools" / "mcp_eval_tasks.json"

# Agreement with the analytic volume, in cubic millimetres.  Expected values
# are given to 0.1 mm^3, and OCCT volumes are exact to far better than that.
VOLUME_TOLERANCE_MM3 = 0.1

VOLUME_LINE = re.compile(r"^volume_m3:\s*([-+0-9.eE]+)\s*$", re.MULTILINE)
MATE_ERROR_LINE = re.compile(r"^mate_max_error:\s*([-+0-9.eE]+)\s*$", re.MULTILINE)
ANGLE_LINE = re.compile(r"^dimension: (\S+) measured_angle_rad: ([-+0-9.eE]+)\s*$", re.MULTILINE)
ANGLE_TOLERANCE_RAD = 1e-9
SECTION_LINE = re.compile(r"^section: (\S+) area_m2: ([-+0-9.eE]+) loops: (\d+) edges: (\d+)\s*$", re.MULTILINE)
RADIAL_LINE = re.compile(r"^dimension: (\S+) measured_radial_length_m: ([-+0-9.eE]+) radius_m: ([-+0-9.eE]+)\s*$", re.MULTILINE)

DIMENSION_LINE = re.compile(r"^dimension: (\S+) measured_length_m: ([-+0-9.eE]+)\s*$", re.MULTILINE)

# Largest mate residual, in metres, an assembly task accepts as satisfied.
MATE_TOLERANCE_M = 1e-6

# Agreement of a drawing dimension's measured length with the task, in metres.
DIMENSION_TOLERANCE_M = 1e-6

INSTRUCTIONS = (
    "Use only the musubicad MCP tools. Read the server instructions and the "
    "authoring guide resource first. Always dry-run before applying and fix "
    "every reported error and sketch_under_constrained warning. Finish by "
    "regenerating the document."
)


def regenerated_volume_mm3(regen_output: str) -> float | None:
    """Parse the solid volume, in mm^3, from `musubicad regen` output."""
    match = VOLUME_LINE.search(regen_output)
    return float(match.group(1)) * 1e9 if match else None


def parameter_names(document: Path) -> set[str]:
    path = document / "graph" / "parameters.json"
    if not path.exists():
        return set()
    parameters = json.loads(path.read_text(encoding="utf-8")).get("parameters", {})
    return {entry.get("name", "") for entry in parameters.values()}


def assembly_counts(document: Path) -> tuple[int, int]:
    """(instances, active mates) in an assembly document."""
    path = document / "graph" / "assemblies.json"
    if not path.exists():
        return 0, 0
    assembly = json.loads(path.read_text(encoding="utf-8")).get("assembly") or {}
    mates = [mate for mate in assembly.get("mates", []) if not mate.get("suppressed")]
    return len(assembly.get("instances", [])), len(mates)


def assembly_joint_ids(document: Path) -> list[str]:
    """Sorted joint IDs declared by an assembly document."""
    path = document / "graph" / "assemblies.json"
    if not path.exists():
        return []
    assembly = json.loads(path.read_text(encoding="utf-8")).get("assembly") or {}
    return sorted(joint.get("id", "") for joint in assembly.get("joints", []))


def drawing_contents(document: Path, regen_output: str = "") -> tuple[list[str], list[float]]:
    """(sorted view projections, sorted dimension lengths in metres)."""
    path = document / "graph" / "drawings.json"
    if not path.exists():
        return [], []
    drawing = json.loads(path.read_text(encoding="utf-8")).get("drawing") or {}
    projections, lengths = [], []
    reported = {dim_id: float(value) for dim_id, value in DIMENSION_LINE.findall(regen_output)}
    for sheet in drawing.get("sheets", []):
        projections += [view.get("projection", "") for view in sheet.get("views", [])]
        for dimension in sheet.get("dimensions", []):
            if "edge_ref" in dimension:
                # Grade regenerated geometry, never cached/authored coordinates.
                # Missing evidence leaves a count mismatch and fails the task.
                if dimension["id"] in reported:
                    lengths.append(reported[dimension["id"]])
                continue
            start, end = dimension["start_model_m"], dimension["end_model_m"]
            lengths.append(sum((b - a) ** 2 for a, b in zip(start, end)) ** 0.5)
    return sorted(projections), sorted(lengths)


def check_reach_summary(task: dict, reach_output: str) -> list[str]:
    """Verify a `musubicad reach` JSON summary against task expectations."""
    problems = []
    try:
        summary = json.loads(reach_output)
    except json.JSONDecodeError as err:
        return [f"reach output was not JSON: {err}"]
    expected = task.get("expected_reach") or {}
    target = summary.get("target")
    if not isinstance(target, dict):
        return ["reach reported no target"]
    if target.get("reachable") is not expected.get("reachable", True):
        problems.append(
            f"reach reachable={target.get('reachable')}, "
            f"expected {expected.get('reachable', True)}"
        )
    if "max_gap_mm" in expected:
        gap = target.get("gap_mm")
        if not isinstance(gap, (int, float)):
            problems.append("reach reported no numeric gap_mm")
        elif gap > expected["max_gap_mm"]:
            problems.append(f"reach gap {gap} mm, expected <= {expected['max_gap_mm']} mm")
    if "min_joint_count" in expected:
        joints = target.get("joints", [])
        if len(joints) < expected["min_joint_count"]:
            problems.append(
                f"reach returned {len(joints)} joints, "
                f"expected at least {expected['min_joint_count']}"
            )
    return problems


def check_robot_artifacts(
    task: dict, document: Path, musubicad: Path, workdir: Path
) -> list[str]:
    """Re-export robot artifacts and run reach independently of the agent."""
    problems = []
    if "expected_urdf_joints" in task:
        output = workdir / "check.urdf"
        export = subprocess.run(
            [str(musubicad), "export", str(document), str(output)],
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        if export.returncode != 0:
            problems.append(f"URDF export failed: {export.stderr.strip()[-300:]}")
        elif not output.exists():
            problems.append("URDF export did not write check.urdf")
        else:
            try:
                root = ET.parse(output).getroot()
            except ET.ParseError as err:
                problems.append(f"URDF was not XML: {err}")
            else:
                names = sorted(joint.attrib.get("name", "") for joint in root.findall("joint"))
                expected = sorted(task["expected_urdf_joints"])
                if names != expected:
                    problems.append(f"URDF joints {names}, expected {expected}")
    if "expected_reach" in task:
        expected = task["expected_reach"]
        flags = [
            "reach",
            str(document),
            "--tool",
            expected["tool"],
            "--target",
            expected["target"],
        ]
        if "point" in expected:
            flags.extend(["--point", expected["point"]])
        if "tolerance" in expected:
            flags.extend(["--tolerance", expected["tolerance"]])
        reach = subprocess.run(
            [str(musubicad), *flags],
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        if reach.returncode != 0:
            problems.append(f"reach failed: {reach.stderr.strip()[-300:]}")
        else:
            problems.extend(check_reach_summary(task, reach.stdout))
    return problems


def check(
    task: dict,
    document: Path,
    regen_output: str,
    musubicad: Path | None = None,
    workdir: Path | None = None,
) -> tuple[bool, list[str]]:
    """Independently verify a finished task; returns (passed, problems)."""
    problems = []
    if "expected_volume_mm3" in task:
        volume = regenerated_volume_mm3(regen_output)
        expected = task["expected_volume_mm3"]
        if volume is None:
            problems.append("regeneration reported no volume")
        elif abs(volume - expected) > VOLUME_TOLERANCE_MM3:
            problems.append(f"volume {volume:.3f} mm^3, expected {expected} mm^3")
    missing = set(task.get("required_parameters", [])) - parameter_names(document)
    if missing:
        problems.append(f"missing parameters: {sorted(missing)}")
    if "expected_instances" in task:
        instances, mates = assembly_counts(document)
        if instances != task["expected_instances"]:
            problems.append(f"{instances} instances, expected {task['expected_instances']}")
        if mates != task["expected_mates"]:
            problems.append(f"{mates} active mates, expected {task['expected_mates']}")
        match = MATE_ERROR_LINE.search(regen_output)
        if match is None:
            problems.append("regeneration reported no mate error")
        elif float(match.group(1)) > MATE_TOLERANCE_M:
            problems.append(f"mates unsatisfied: max error {match.group(1)} m")
    if "expected_joint_ids" in task:
        joints = assembly_joint_ids(document)
        expected = sorted(task["expected_joint_ids"])
        if joints != expected:
            problems.append(f"joints {joints}, expected {expected}")
    if "expected_views" in task:
        projections, lengths = drawing_contents(document, regen_output)
        if projections != sorted(task["expected_views"]):
            problems.append(f"views {projections}, expected {sorted(task['expected_views'])}")
        expected_lengths = sorted(task.get("expected_dimension_lengths_m", []))
        if len(lengths) != len(expected_lengths) or any(
            abs(a - b) > DIMENSION_TOLERANCE_M for a, b in zip(lengths, expected_lengths)
        ):
            problems.append(f"dimension lengths {lengths} m, expected {expected_lengths} m")
        if "expected_dimension_edge_refs" in task:
            drawing = json.loads((document / "graph" / "drawings.json").read_text(encoding="utf-8")).get("drawing") or {}
            refs = sorted(dim.get("edge_ref", "") for sheet in drawing.get("sheets", []) for dim in sheet.get("dimensions", []))
            if refs != sorted(task["expected_dimension_edge_refs"]):
                problems.append(f"dimension edge refs {refs}, expected {task['expected_dimension_edge_refs']}")
    if "expected_sections" in task:
        drawing = json.loads((document / "graph" / "drawings.json").read_text(encoding="utf-8")).get("drawing") or {}
        authored = {v["id"]: v["section"] for sheet in drawing.get("sheets", []) for v in sheet.get("views", []) if v.get("section")}
        reported = {id_: (float(area), int(loops), int(edges)) for id_, area, loops, edges in SECTION_LINE.findall(regen_output)}
        desired = task["expected_sections"]
        if sorted(authored) != sorted(s["view_id"] for s in desired):
            problems.append("section view IDs differ")
        for section in desired:
            id_ = section["view_id"]
            measured = reported.get(id_)
            if measured is None or not math.isfinite(measured[0]) or abs(measured[0]-section["area_m2"]) > 1e-12 or measured[1:] != (section["loop_count"],section["edge_count"]):
                problems.append(f"section {id_} lacks matching fresh SI area/counts")
            if authored.get(id_, {}).get("plane") != section["plane"] or abs(authored.get(id_, {}).get("hatch_spacing_m",-1)-section["hatch_spacing_m"]) > 1e-9:
                problems.append(f"section {id_} plane/hatch authoring differs")
    if "expected_radial_dimensions" in task:
        drawing = json.loads((document / "graph" / "drawings.json").read_text(encoding="utf-8")).get("drawing") or {}
        dimensions = {dim["id"]: dim for sheet in drawing.get("sheets", []) for dim in sheet.get("radial_dimensions", [])}
        reported = {id_: (float(length), float(radius)) for id_, length, radius in RADIAL_LINE.findall(regen_output)}
        expected = task["expected_radial_dimensions"]
        if sorted(dimensions) != sorted(d["id"] for d in expected):
            problems.append("radial dimension IDs differ")
        for desired in expected:
            id_ = desired["id"]
            authored = dimensions.get(id_, {})
            measured = reported.get(id_)
            if measured is None or any(not math.isfinite(v) for v in measured) or abs(measured[0]-desired["measured_length_m"]) > 1e-9 or abs(measured[1]-desired["radius_m"]) > 1e-9:
                problems.append(f"radial dimension {id_} lacks matching fresh SI measurement")
            if authored.get("edge_ref") != desired["edge_ref"] or authored.get("kind") != desired["kind"] or abs(authored.get("leader_extension_m", -1)-desired["leader_extension_m"]) > 1e-9:
                problems.append(f"radial dimension {id_} authoring differs")
    if "expected_dimension_angles_rad" in task:
        drawing = json.loads((document / "graph" / "drawings.json").read_text(encoding="utf-8")).get("drawing") or {}
        dimensions = [dim for sheet in drawing.get("sheets", []) for dim in sheet.get("angular_dimensions", [])]
        reported = {id_: float(angle) for id_, angle in ANGLE_LINE.findall(regen_output)}
        angles = sorted(reported[dim["id"]] for dim in dimensions if dim["id"] in reported)
        expected = sorted(task["expected_dimension_angles_rad"])
        if len(angles) != len(expected) or any(abs(a-b) > ANGLE_TOLERANCE_RAD for a,b in zip(angles,expected)):
            problems.append(f"angular dimensions {angles} rad, expected {expected} rad")
        if "expected_angular_edge_refs" in task:
            refs = sorted(ref for dim in dimensions for ref in dim.get("edge_refs", []))
            if refs != sorted(task["expected_angular_edge_refs"]):
                problems.append("angular dimension semantic bindings differ")
        if "expected_arc_radii_m" in task:
            radii = sorted(dim["arc_radius_m"] for dim in dimensions)
            expected_radii = sorted(task["expected_arc_radii_m"])
            if len(radii) != len(expected_radii) or any(abs(a-b) > 1e-9 for a,b in zip(radii,expected_radii)):
                problems.append("angular sheet arc radii differ")
        if "expected_angular_ids" in task and sorted(dim["id"] for dim in dimensions) != sorted(task["expected_angular_ids"]):
            problems.append("angular dimension IDs differ")
    if (
        ("expected_urdf_joints" in task or "expected_reach" in task)
        and musubicad is not None
        and workdir is not None
    ):
        problems.extend(check_robot_artifacts(task, document, musubicad, workdir))
    return not problems, problems


def run_task(task: dict, musubicad: Path, claude: str, timeout_s: int) -> dict:
    workdir = Path(tempfile.mkdtemp(prefix=f"mcp-eval-{task['id']}-"))
    document = workdir / task["document"]
    if "source" in task:
        shutil.copytree(ROOT / task["source"], document)
    placeholders = {
        "document": document.as_posix(),
        "workdir": workdir.as_posix(),
        "root": ROOT.as_posix(),
    }
    # Setup steps are `musubicad` argument lists, e.g. exporting a STEP file
    # the agent must import.  They run before the agent starts.
    for step in task.get("setup", []):
        subprocess.run(
            [str(musubicad), *[arg.format(**placeholders) for arg in step]],
            check=True,
            capture_output=True,
        )
    config = workdir / "mcp.json"
    config.write_text(
        json.dumps(
            {"mcpServers": {"musubicad": {"command": str(musubicad), "args": ["mcp"]}}}
        ),
        encoding="utf-8",
    )
    prompt = f"{INSTRUCTIONS}\n\n{task['prompt'].format(**placeholders)}"
    started = time.monotonic()
    completed = subprocess.run(
        [
            claude,
            "-p",
            prompt,
            "--mcp-config",
            str(config),
            "--strict-mcp-config",
            "--allowedTools",
            "mcp__musubicad__*",
            "--output-format",
            "json",
        ],
        cwd=workdir,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=timeout_s,
    )
    elapsed = time.monotonic() - started
    try:
        host = json.loads(completed.stdout)
    except json.JSONDecodeError:
        host = {"is_error": True, "result": completed.stdout[-2000:] + completed.stderr[-2000:]}

    regen = subprocess.run(
        [str(musubicad), "regen", str(document)],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    passed, problems = check(task, document, regen.stdout, musubicad, workdir)
    if "expected_views" in task:
        # The views must resolve their model and project to an SVG sheet.
        export = subprocess.run(
            [str(musubicad), "export", str(document), str(workdir / "check.svg")],
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        if export.returncode != 0:
            passed = False
            problems.append(f"SVG export failed: {export.stderr.strip()[-300:]}")
    return {
        "id": task["id"],
        "passed": passed,
        "problems": problems,
        "volume_mm3": regenerated_volume_mm3(regen.stdout),
        "expected_volume_mm3": task.get("expected_volume_mm3"),
        "turns": host.get("num_turns"),
        "cost_usd": host.get("total_cost_usd"),
        "duration_s": round(elapsed, 1),
        "host_error": bool(host.get("is_error")),
        "workdir": str(workdir),
    }


def self_test() -> None:
    """Exercise the checker without calling a model."""
    output = "regenerated: 3 features\nvolume_m3: 0.0000053684999999\nmass_kg: 1\n"
    assert abs(regenerated_volume_mm3(output) - 5368.5) < 1e-6
    assert regenerated_volume_mm3("no volume here") is None
    task = {"expected_volume_mm3": 5368.5, "required_parameters": ["width"]}
    bracket = ROOT / "examples" / "bracket.ocad.d"
    assert check(task, bracket, output) == (True, [])
    passed, problems = check(task, bracket, "volume_m3: 0.000006\n")
    assert not passed and "volume" in problems[0]
    passed, problems = check(
        {**task, "required_parameters": ["wall"]}, bracket, output
    )
    assert not passed and "wall" in problems[0]
    pair = ROOT / "examples" / "assembly_two_brackets.ocad.d"
    assembly = {"expected_volume_mm3": 5368.5, "expected_instances": 2, "expected_mates": 2}
    assert check(assembly, pair, output + "mate_max_error: 1e-9\n") == (True, [])
    passed, problems = check(
        {**assembly, "expected_instances": 3}, pair, output + "mate_max_error: 0.01\n"
    )
    assert not passed and "instances" in problems[0] and "unsatisfied" in problems[1]
    sheet = ROOT / "examples" / "robot_arm_assembly_drawing.ocad.d"
    drawing = {"expected_views": ["front"], "expected_dimension_lengths_m": [0.16]}
    assert check(drawing, sheet, "") == (True, [])
    passed, problems = check(
        {"expected_views": ["front", "top"], "expected_dimension_lengths_m": [0.08]}, sheet, ""
    )
    assert not passed and "views" in problems[0] and "dimension" in problems[1]
    semantic_sheet = ROOT / "examples" / "semantic_bracket_drawing.ocad.d"
    semantic = {
        "expected_views": ["front"],
        "expected_dimension_lengths_m": [0.08],
        "expected_dimension_edge_refs": ["ref:edge:bracket_top_front"],
    }
    evidence = "dimension: dim:semantic_width measured_length_m: 0.08\n"
    assert check(semantic, semantic_sheet, evidence) == (True, [])
    assert not check(semantic, semantic_sheet, "")[0]
    assert not check(semantic, semantic_sheet, evidence.replace("0.08", "0.1"))[0]
    assert not check({**semantic, "expected_dimension_edge_refs": ["ref:edge:wrong"]}, semantic_sheet, evidence)[0]
    angular_sheet = ROOT / "examples" / "angular_plate_drawing.ocad.d"
    angular = {"expected_dimension_angles_rad": [1.5707963267948966], "expected_angular_edge_refs": ["ref:edge:plate_front", "ref:edge:plate_left"], "expected_angular_ids": ["dim:plate_corner"], "expected_arc_radii_m": [0.01]}
    angle_evidence = "dimension: dim:plate_corner measured_angle_rad: 1.5707963267948966\n"
    assert check(angular, angular_sheet, angle_evidence) == (True, [])
    assert not check(angular, angular_sheet, "")[0]
    assert not check({**angular, "expected_arc_radii_m": [0.015]}, angular_sheet, angle_evidence)[0]
    assert not check(angular, angular_sheet, angle_evidence.replace("1.5707963267948966", "0.5"))[0]
    assert not check({**angular, "expected_angular_edge_refs": ["ref:edge:wrong"]}, angular_sheet, angle_evidence)[0]
    radial_sheet = ROOT / "examples" / "radial_plate_drawing.ocad.d"
    radial = {"expected_radial_dimensions": [
        {"id": "dim:radius", "kind": "radius", "edge_ref": "ref:edge:circle", "measured_length_m": 0.01, "radius_m": 0.01, "leader_extension_m": 0.01},
        {"id": "dim:diameter", "kind": "diameter", "edge_ref": "ref:edge:circle", "measured_length_m": 0.02, "radius_m": 0.01, "leader_extension_m": 0.01}]}
    radial_evidence = "dimension: dim:radius measured_radial_length_m: 0.01 radius_m: 0.01\ndimension: dim:diameter measured_radial_length_m: 0.02 radius_m: 0.01\n"
    assert check(radial, radial_sheet, radial_evidence) == (True, [])
    assert not check(radial, radial_sheet, "")[0]
    assert not check(radial, radial_sheet, radial_evidence.replace("0.02", "0.03"))[0]
    assert not check(radial, radial_sheet, radial_evidence.replace("radius_m: 0.01", "radius_m: nan"))[0]
    section_sheet = ROOT / "examples" / "section_plate_drawing.ocad.d"
    sections = {"expected_sections": [{"view_id": "view:front", "area_m2": 0.0044, "loop_count": 2, "edge_count": 8, "plane": {"origin_m": [0,0,0.003], "normal": [0,0,1]}, "hatch_spacing_m": 0.003}]}
    section_evidence = "section: view:front area_m2: 0.004399999999999999 loops: 2 edges: 8\n"
    assert check(sections, section_sheet, section_evidence) == (True, [])
    assert not check(sections, section_sheet, "")[0]
    assert not check(sections, section_sheet, section_evidence.replace("loops: 2", "loops: 1"))[0]
    assert not check(sections, section_sheet, section_evidence.replace("0.004399999999999999", "0.0048"))[0]
    wrong_section = json.loads(json.dumps(sections))
    wrong_section["expected_sections"][0]["plane"]["origin_m"][2] = 0.004
    assert not check(wrong_section, section_sheet, section_evidence)[0]
    robot = ROOT / "examples" / "robot_arm_assembly.ocad.d"
    assert check(
        {"expected_joint_ids": ["joint:shoulder", "joint:elbow", "joint:wrist"]},
        robot,
        "",
    ) == (True, [])
    assert not check({"expected_joint_ids": ["joint:shoulder"]}, robot, "")[0]
    reach_task = {
        "expected_reach": {
            "tool": "gripper",
            "target": "120,170,63mm",
            "reachable": True,
            "max_gap_mm": 0.01,
            "min_joint_count": 3,
        }
    }
    assert check_reach_summary(
        reach_task,
        '{"target":{"reachable":true,"gap_mm":0.0,"joints":[{}, {}, {}]}}',
    ) == []
    assert check_reach_summary(
        reach_task,
        '{"target":{"reachable":false,"gap_mm":20.0,"joints":[{}]}}',
    )
    tasks = json.loads(TASKS.read_text(encoding="utf-8"))["tasks"]
    assert len({task["id"] for task in tasks}) == len(tasks)
    placeholders = {"document": "d", "workdir": "w", "root": "r"}
    for task in tasks:
        assert "{document}" in task["prompt"], task["id"]
        task["prompt"].format(**placeholders)
        for step in task.get("setup", []):
            assert all(isinstance(arg, str) for arg in step), task["id"]
            [arg.format(**placeholders) for arg in step]
    print("self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--musubicad", type=Path, default=ROOT / "target" / "debug" / "musubicad")
    parser.add_argument("--claude", default="claude")
    parser.add_argument("--only", action="append", help="task id to run (repeatable)")
    parser.add_argument("--timeout", type=int, default=900, help="seconds per task")
    parser.add_argument("--out", type=Path, help="write the JSON report here")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return 0

    musubicad = args.musubicad
    if not musubicad.exists() and musubicad.with_suffix(".exe").exists():
        musubicad = musubicad.with_suffix(".exe")
    tasks = json.loads(TASKS.read_text(encoding="utf-8"))["tasks"]
    if args.only:
        tasks = [task for task in tasks if task["id"] in args.only]
    results = [run_task(task, musubicad.resolve(), args.claude, args.timeout) for task in tasks]
    report = {
        "passed": sum(result["passed"] for result in results),
        "total": len(results),
        "cost_usd": round(sum(result["cost_usd"] or 0 for result in results), 4),
        "results": results,
    }
    text = json.dumps(report, indent=2)
    if args.out:
        args.out.write_text(text + "\n", encoding="utf-8")
    print(text)
    return 0 if report["passed"] == report["total"] else 1


if __name__ == "__main__":
    sys.exit(main())
