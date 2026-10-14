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
import hashlib
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
DETAIL_LINE = re.compile(r"^detail: (\S+) parent: (\S+) magnification: ([-+0-9.eE]+) scale: ([-+0-9.eE]+) segments: (\d+)\s*$", re.MULTILINE)
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


def drawing_dxf_problems(path: Path, task: dict) -> list[str]:
    """Check required primitive DXF evidence without adding a runtime dependency.

    Independent full-format/consumer validation is tools/test_drawing_dxf.py.
    """
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
        if len(lines) % 2:
            raise ValueError("unpaired tags")
        pairs = [(int(lines[i]), lines[i+1]) for i in range(0, len(lines), 2)]
        if not pairs or pairs[0] != (0, "SECTION") or pairs[-1] != (0, "EOF"):
            raise ValueError("missing section/end")
        header = {value: pairs[i+1] for i, (code, value) in enumerate(pairs[:-1]) if code == 9}
        if header.get("$ACADVER") != (1, "AC1027"):
            raise ValueError("unexpected format")
        entries, current = [], []
        for tag in pairs:
            if tag[0] == 0 and current:
                entries.append(current)
                current = []
            current.append(tag)
        if current:
            entries.append(current)
        entities = []
        in_entities = False
        for entry in entries:
            if entry[:2] == [(0, "SECTION"), (2, "ENTITIES")]:
                in_entities = True
            elif entry[0] == (0, "ENDSEC"):
                in_entities = False
            elif in_entities:
                entities.append(entry)
        paper = [e for e in entities if e[0] == (0, "LWPOLYLINE") and (8, "PAPER") in e]
        if len(paper) != 1 or (70, "1") not in paper[0] or (90, "4") not in paper[0]:
            raise ValueError("missing closed paper frame")
        width, height = task["expected_dxf_paper_mm"]
        actual = [[float(v) for c, v in paper[0] if c == code] for code in (10, 20)]
        wanted = [[0, width, width, 0], [0, 0, height, height]]
        problems = []
        if header.get("$INSUNITS") != (70, str(task.get("expected_dxf_units", 4))):
            problems.append("DXF units differ")
        if any(len(a) != 4 or any(not math.isfinite(v) or abs(v-w) > 1e-6 for v, w in zip(a, b)) for a, b in zip(actual, wanted)):
            problems.append("DXF paper dimensions differ")
        texts = [v for e in entities if e[0] == (0, "TEXT") for c, v in e if c == 1]
        if any(text not in texts for text in task.get("expected_dxf_text", [])):
            problems.append("required DXF labels are absent")
        if "expected_dxf_model_bounds_mm" in task:
            coordinates = [[], []]
            for e in entities:
                if e[0] == (0, "LINE") and any((8, layer) in e for layer in ("VISIBLE", "HIDDEN")):
                    for c, v in e:
                        if c in (10, 11):
                            coordinates[0].append(float(v))
                        elif c in (20, 21):
                            coordinates[1].append(float(v))
            desired = task["expected_dxf_model_bounds_mm"]
            for axis, coords in enumerate(coordinates):
                if not coords or not all(math.isfinite(v) for v in coords) or any(abs(v-w) > 1e-6 for v, w in zip([min(coords), max(coords)], [desired[0][axis], desired[1][axis]])):
                    problems.append("DXF projected model bounds differ")
        return problems
    except (OSError, UnicodeError, ValueError, IndexError):
        return ["required DXF artifact is absent or malformed"]


def bom_artifact_problems(path: Path, task: dict) -> list[str]:
    try:
        bom = json.loads(path.read_text(encoding="utf-8"))
        problems = []
        for name, desired in task["expected_bom_summary"].items():
            actual = bom.get(name)
            if actual != desired or (isinstance(desired, int) and type(actual) is not int):
                problems.append(f"BOM {name} differs")
        rows = bom["rows"]
        actual_rows = sorted((r["source_doc"], r["source_path"], r["quantity"]) for r in rows)
        expected_rows = sorted((r["source_doc"], r["source_path"], r["quantity"]) for r in task["expected_bom_rows"])
        if actual_rows != expected_rows:
            problems.append("BOM leaf quantities/sources differ")
        for row in rows:
            quantity, paths = row["quantity"], row["instance_paths"]
            if type(quantity) is not int or quantity <= 0 or len(paths) != quantity or any(
                not isinstance(p, list) or not p or any(not isinstance(i, str) or not i.startswith("instance:") for i in p) for p in paths
            ):
                problems.append("BOM quantity/provenance differs")
            elif len(set(tuple(p) for p in paths)) != quantity:
                problems.append("BOM occurrence paths are duplicated")
        if bom["leaf_part_count"] != len(rows) or bom["leaf_instance_count"] != sum(r["quantity"] for r in rows):
            problems.append("BOM totals differ from rows")
        return problems
    except (OSError, UnicodeError, ValueError, KeyError, TypeError, AttributeError):
        return ["required BOM artifact is absent or malformed"]


def child_tree_digest(document: Path) -> str:
    """Byte-exact nested source evidence; exclude the editable parent."""
    digest = hashlib.sha256()
    root = document / "parts"
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        digest.update(path.relative_to(root).as_posix().encode("utf-8") + b"\0")
        digest.update(path.read_bytes())
    return digest.hexdigest()


def check_articulated_evidence(task: dict, document: Path, report: dict) -> list[str]:
    """Compare source ownership and SI frames with independent analytic fixtures."""
    problems = []
    try:
        assembly = json.loads((document / "graph/assemblies.json").read_text(encoding="utf-8"))["assembly"]
        instances = {i["id"]: i for i in assembly["instances"]}
        configured = {i["id"]: i["articulation"] for i in assembly["instances"] if i.get("articulation") is not None}
        if configured != task["expected_articulations"]:
            problems.append("articulation source owner/revision/coordinates differ")
        guards = [c["source_kind"] for c in assembly["components"] if c["id"] == "component:mechanism"]
        if guards != [task.get("expected_source_kind","articulated_assembly")] or set(instances) != set(task.get("expected_instance_ids",["instance:left","instance:right"])):
            problems.append("articulated instance identity or mandatory guard differs")
        if child_tree_digest(document) != task["expected_child_tree_sha256"]:
            problems.append("child source files were modified")
        if "expected_context_dof" in task and report.get("context_dof") != task["expected_context_dof"]:
            problems.append("fully bound context DOF evidence differs")
        leaves = report["assembly_leaves"]
        expected = task["expected_articulated_leaves"]
        paths = [l["instance_path"] for l in leaves]
        if paths != sorted(paths) or paths != [l["instance_path"] for l in expected]:
            problems.append("posed leaf identities/order/count differ")
        else:
            for got, want in zip(leaves, expected):
                if (got["source_doc"], got["source_revision"]) != (want["source_doc"], want["source_revision"]):
                    problems.append("posed leaf source ownership differs")
                for key in ("translation_m", "rotation"):
                    actual = got["placement"][key]
                    target = want["placement"][key]
                    if key == "rotation":
                        actual = [v for row in actual for v in row]
                        target = [v for row in target for v in row]
                    if len(actual) != len(target) or any(not math.isfinite(a) or abs(a-b) > 1e-9 for a,b in zip(actual,target)):
                        problems.append("posed leaf analytic frame differs")
                bounds = got["bounds_size_m"]
                if len(bounds) != 3 or any(not math.isfinite(a) or abs(a-b) > 3e-7 for a,b in zip(bounds,want["bounds_size_m"])):
                    problems.append("posed leaf analytic bounds differ")
                if not math.isfinite(got["volume_m3"]) or abs(got["volume_m3"]-6e-6) > 1e-11:
                    problems.append("posed leaf analytic volume differs")
        if not math.isfinite(report["volume_m3"]) or abs(report["volume_m3"]-task.get("expected_volume_mm3",24000)*1e-9) > 1e-11:
            problems.append("articulated total volume differs")
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        problems.append("articulation evidence is absent or malformed")
    return problems


def check_counterbore_source(task: dict, document: Path) -> list[str]:
    """A mass match alone cannot substitute a plain bore or changed stock."""
    try:
        baseline = ROOT / task["source"]
        old = json.loads((baseline / "graph/features.json").read_text(encoding="utf-8"))
        new = json.loads((document / "graph/features.json").read_text(encoding="utf-8"))
        before = {n["id"]: n for n in old["feature_nodes"]}
        after = {n["id"]: n for n in new["feature_nodes"]}
        if before != after or old["feature_graph"]["order"] != new["feature_graph"]["order"]:
            return ["counterbore source/standard/seat/target/order was altered"]
        for filename in ["sketches.json", "semantic_refs.json"]:
            if json.loads((baseline / "graph" / filename).read_text(encoding="utf-8")) != json.loads((document / "graph" / filename).read_text(encoding="utf-8")):
                return ["counterbore sketch or owned entry reference was altered"]
        definition = after[task["expected_counterbore"]]["definition"]
        if definition["type"] != "seated_standard_hole" or definition["seat"]["type"] != "counterbore":
            return ["explicit counterbore source is missing"]
        return []
    except (OSError, ValueError, KeyError, TypeError):
        return ["counterbore source evidence absent or malformed"]


def check_countersink_source(task: dict, document: Path) -> list[str]:
    """A mass match alone cannot substitute a plain bore or changed stock."""
    try:
        baseline = ROOT / task["source"]
        old = json.loads((baseline / "graph/features.json").read_text(encoding="utf-8"))
        new = json.loads((document / "graph/features.json").read_text(encoding="utf-8"))
        before = {n["id"]: n for n in old["feature_nodes"]}
        after = {n["id"]: n for n in new["feature_nodes"]}
        if before != after or old["feature_graph"]["order"] != new["feature_graph"]["order"]:
            return ["countersink source/standard/seat/target/order was altered"]
        for filename in ["sketches.json", "semantic_refs.json"]:
            if json.loads((baseline / "graph" / filename).read_text(encoding="utf-8")) != json.loads((document / "graph" / filename).read_text(encoding="utf-8")):
                return ["countersink sketch or owned entry reference was altered"]
        definition = after[task["expected_countersink"]]["definition"]
        if definition["type"] != "seated_standard_hole" or definition["seat"]["type"] != "countersink":
            return ["explicit countersink source is missing"]
        return []
    except (OSError, ValueError, KeyError, TypeError):
        return ["countersink source evidence absent or malformed"]



def check_nominal_thread_source(task: dict, document: Path) -> list[str]:
    """Source intent/ownership guard; physical phase is graded by kernel tests."""
    try:
        baseline = ROOT / task["source"]
        for filename in ["sketches.json", "parameters.json", "semantic_refs.json"]:
            if json.loads((baseline / "graph" / filename).read_text()) != json.loads((document / "graph" / filename).read_text()):
                return ["nominal thread authored stock, parameters or side ownership changed"]
        before = json.loads((baseline / "graph/features.json").read_text())
        expected = next(n for n in before["feature_nodes"] if n["id"] == task["expected_nominal_thread"])["definition"]
        if expected["type"] != "nominal_metric_thread" or expected["representation"] != "nominal_metric_basic_flat_root":
            return ["invalid nominal thread baseline"]
        expected["handedness"] = task["expected_thread_hand"]
        after = json.loads((document / "graph/features.json").read_text())
        if after != before:
            return ["nominal thread representation, hand, turns, source or order differs"]
        return []
    except (OSError, ValueError, KeyError, TypeError, StopIteration):
        return ["nominal thread source evidence absent or malformed"]


def check_cylindrical_draft_source(task: dict, document: Path) -> list[str]:
    """Sign cannot be graded by volume alone for a centred neutral plane."""
    try:
        baseline = ROOT / task["source"]
        for filename in ["features.json", "sketches.json", "semantic_refs.json"]:
            before = json.loads((baseline / "graph" / filename).read_text(encoding="utf-8"))
            after = json.loads((document / "graph" / filename).read_text(encoding="utf-8"))
            if before != after:
                return ["cylindrical draft source, neutral plane, ownership or order changed"]
        features = json.loads((document / "graph/features.json").read_text(encoding="utf-8"))["feature_nodes"]
        definition = next(n for n in features if n["id"] == task["expected_cylindrical_draft"])["definition"]
        if definition["type"] != "cylindrical_draft":
            return ["explicit cylindrical draft source is missing"]
        old = json.loads((baseline / "graph/parameters.json").read_text(encoding="utf-8"))
        new = json.loads((document / "graph/parameters.json").read_text(encoding="utf-8"))
        expected = old["parameters"]["param:draft_angle"]
        actual = new["parameters"]["param:draft_angle"]
        if actual["expr"] != task["expected_draft_angle_expr"]:
            return ["signed draft angle expression differs"]
        for field in ["id", "name"]:
            if actual[field] != expected[field]:
                return ["draft parameter identity changed"]
        # Only the requested expression and its disposable dirty flag may differ.
        normalized = json.loads(json.dumps(new))
        normalized["parameters"]["param:draft_angle"] = expected
        if normalized != old:
            return ["another draft parameter source was changed"]
        return []
    except (OSError, ValueError, KeyError, TypeError, StopIteration):
        return ["cylindrical draft source evidence absent or malformed"]


def check_tapered_rib_source(task: dict, document: Path) -> list[str]:
    """A volume match cannot substitute a plain rib or altered end/source."""
    try:
        baseline = ROOT / task["source"]
        for filename in ["features.json", "sketches.json", "semantic_refs.json"]:
            before = json.loads((baseline / "graph" / filename).read_text(encoding="utf-8"))
            after = json.loads((document / "graph" / filename).read_text(encoding="utf-8"))
            if before != after:
                return ["tapered rib source, neutral plane, ownership or order changed"]
        features = json.loads((document / "graph/features.json").read_text(encoding="utf-8"))["feature_nodes"]
        definition = next(n for n in features if n["id"] == task["expected_tapered_rib"])["definition"]
        if definition["type"] != "tapered_rib":
            return ["explicit tapered rib source is missing"]
        old = json.loads((baseline / "graph/parameters.json").read_text(encoding="utf-8"))
        new = json.loads((document / "graph/parameters.json").read_text(encoding="utf-8"))
        expected = old["parameters"]["param:rib_height"]
        actual = new["parameters"]["param:rib_height"]
        if actual["expr"] != task["expected_rib_height_expr"]:
            return ["rib height expression differs"]
        for field in ["id", "name"]:
            if actual[field] != expected[field]:
                return ["rib parameter identity changed"]
        # Only the requested expression and its disposable dirty flag may differ.
        normalized = json.loads(json.dumps(new))
        normalized["parameters"]["param:rib_height"] = expected
        if normalized != old:
            return ["another rib parameter source was changed"]
        return []
    except (OSError, ValueError, KeyError, TypeError, StopIteration):
        return ["tapered rib source evidence absent or malformed"]


def check(
    task: dict,
    document: Path,
    regen_output: str,
    musubicad: Path | None = None,
    workdir: Path | None = None,
) -> tuple[bool, list[str]]:
    """Independently verify a finished task; returns (passed, problems)."""
    problems = []
    if "expected_nominal_thread" in task:
        problems.extend(check_nominal_thread_source(task, document))
    if "expected_tapered_rib" in task:
        problems.extend(check_tapered_rib_source(task, document))
    if "expected_cylindrical_draft" in task:
        problems.extend(check_cylindrical_draft_source(task, document))
    if "expected_counterbore" in task:
        problems.extend(check_counterbore_source(task, document))
    if "expected_countersink" in task:
        problems.extend(check_countersink_source(task, document))
    if "expected_articulated_leaves" in task:
        if musubicad is None:
            problems.append("articulated checker requires fresh native regeneration")
        else:
            try:
                result = subprocess.run([str(musubicad), "regen", str(document), "--json"], capture_output=True, text=True, timeout=60)
                if result.returncode:
                    problems.append("articulated regeneration refused")
                else:
                    problems.extend(check_articulated_evidence(task, document, json.loads(result.stdout)))
            except (OSError, ValueError, subprocess.TimeoutExpired):
                problems.append("articulated regeneration evidence unavailable")
    if "expected_joint_couplings" in task:
        try:
            assembly = json.loads((document / "graph" / "assemblies.json").read_text(encoding="utf-8"))["assembly"]
            couplings = assembly.get("couplings", [])
            if sorted(couplings, key=lambda c: c["id"]) != sorted(task["expected_joint_couplings"], key=lambda c: c["id"]):
                problems.append("joint coupling identities or coefficients differ from expected source")
            placements = {i["id"]: i["placement"] for i in assembly.get("instances", [])}
            if placements != task.get("expected_mechanical_placements", placements):
                problems.append("coupling edit changed authored mechanical placements")
        except (OSError, ValueError, KeyError, TypeError, AttributeError):
            problems.append("joint coupling source is absent or malformed")
    if "expected_joint_pose" in task and musubicad is not None:
        expected = task["expected_joint_pose"]
        command = [str(musubicad), "inspect-joint-pose", str(document)]
        command.extend(f"{joint}={value}" for joint, value in sorted(expected["positions"].items()))
        try:
            result = subprocess.run(command, capture_output=True, text=True, timeout=30)
            if result.returncode:
                problems.append("joint pose backend refused expected input")
            else:
                positions = {p["joint_id"]: p for p in json.loads(result.stdout)["positions"]}
                for desired in expected["derived"]:
                    actual = positions.get(desired["joint_id"], {})
                    key = "position_m" if "position_m" in desired else "position_rad"
                    value = actual.get(key)
                    if actual.get("derived") is not True or type(value) not in (int, float) or not math.isfinite(value) or abs(value - desired[key]) > 1e-9:
                        problems.append(f"derived joint {desired['joint_id']} differs from analytic SI position")
        except (OSError, ValueError, KeyError, TypeError, AttributeError, subprocess.TimeoutExpired):
            problems.append("joint pose response is absent or malformed")
    if "expected_exploded_views" in task:
        try:
            assembly = json.loads((document / "graph" / "assemblies.json").read_text(encoding="utf-8"))["assembly"]
            views = {v["id"]: v for v in assembly.get("exploded_views", [])}
            if len(views) != len(assembly.get("exploded_views", [])):
                problems.append("duplicate exploded view IDs")
            for expected in task["expected_exploded_views"]:
                actual = views.get(expected["id"], {})
                actual = dict(actual)
                actual["offsets"] = sorted(actual.get("offsets", []), key=lambda o: o["instance_id"])
                expected = dict(expected)
                expected["offsets"] = sorted(expected.get("offsets", []), key=lambda o: o["instance_id"])
                if actual != expected:
                    problems.append(f"exploded view {expected['id']} lacks expected SI offsets")
            placements = {i["id"]: i["placement"] for i in assembly.get("instances", [])}
            if placements != task.get("expected_mechanical_placements", placements):
                problems.append("exploded presentation changed mechanical placements")
        except (OSError, ValueError, KeyError, TypeError, AttributeError):
            problems.append("exploded assembly source is absent or malformed")
    if "expected_projected_sketches" in task:
        try:
            definitions = json.loads((document / "graph" / "features.json").read_text(encoding="utf-8"))["feature_nodes"]
            definitions = {node["id"]: node["definition"] for node in definitions}
            authored = json.loads((document / "graph" / "sketches.json").read_text(encoding="utf-8"))["sketches"]
            authored_ids = {sketch["id"] for sketch in authored}
            for desired in task["expected_projected_sketches"]:
                definition = definitions.get(desired["id"], {})
                if definition.get("type") != "projected_sketch" or definition.get("sketch_id") != desired["sketch_id"] or definition.get("projections") != desired["projections"] or desired["sketch_id"] in authored_ids:
                    problems.append(f"projection {desired['id']} lacks graph-owned source bindings or saves derived coordinates")
        except (OSError, ValueError, KeyError, TypeError, AttributeError):
            problems.append("projection source is absent or malformed")
    if "expected_cubic_sources" in task:
        try:
            sketches = json.loads((document / "graph" / "sketches.json").read_text(encoding="utf-8"))["sketches"]
            by_id = {sketch["id"]: sketch for sketch in sketches}
            for desired in task["expected_cubic_sources"]:
                entities = {entity["id"]: entity for entity in by_id.get(desired["sketch_id"], {}).get("entities", [])}
                curve = entities.get(desired["entity_id"], {})
                if curve.get("type") != "cubic_bezier" or curve.get("control_points") != desired["control_points"] or curve.get("construction", False):
                    problems.append(f"cubic {desired['entity_id']} lacks expected stable closed control identity")
                if set(curve) - {"type", "id", "construction", "control_points"}:
                    problems.append("cubic source contains unsupported or derived fields")
                params = json.loads((document / "graph" / "parameters.json").read_text(encoding="utf-8"))["parameters"]
                expressions = {p["name"]: p["expr"] for p in params.values()}
                if any(expressions.get(name) != expr for name, expr in desired.get("parameter_expressions", {}).items()):
                    problems.append("cubic parameters differ from the requested control dimensions")
                if "extrude_id" in desired:
                    nodes = json.loads((document / "graph" / "features.json").read_text(encoding="utf-8"))["feature_nodes"]
                    node = next((n for n in nodes if n["id"] == desired["extrude_id"]), {})
                    if node.get("suppressed", False) or node.get("definition") != desired["extrude_definition"]:
                        problems.append("cubic exact profile/normal extrusion contract was replaced")
                for point_id, coordinates in desired["point_coordinates"].items():
                    point = entities.get(point_id, {})
                    if point.get("type") != "point" or [point.get("x"), point.get("y")] != coordinates:
                        problems.append(f"cubic control {point_id} lost its authored coordinates/expressions")
        except (OSError, ValueError, KeyError, TypeError, AttributeError):
            problems.append("cubic source is absent or malformed")
    if "expected_ellipse_sources" in task:
        try:
            sketches = json.loads((document / "graph" / "sketches.json").read_text(encoding="utf-8"))["sketches"]
            by_id = {sketch["id"]: sketch for sketch in sketches}
            for desired in task["expected_ellipse_sources"]:
                entities = {entity["id"]: entity for entity in by_id.get(desired["sketch_id"], {}).get("entities", [])}
                entity = entities.get(desired["entity_id"], {})
                if entity.get("type") != "ellipse" or any(entity.get(field) != desired[field] for field in ("center", "major_radius", "minor_radius", "axis_angle")):
                    problems.append(f"ellipse {desired['entity_id']} lacks expected parametric source identity")
        except (OSError, ValueError, KeyError, TypeError, AttributeError):
            problems.append("ellipse source is absent or malformed")
    if "expected_sketch_points" in task:
        try:
            sketches = json.loads((document / "graph" / "sketches.json").read_text(encoding="utf-8"))["sketches"]
            by_id = {sketch["id"]: sketch for sketch in sketches}
            for desired in task["expected_sketch_points"]:
                entities = {entity["id"]: entity for entity in by_id.get(desired["sketch_id"], {}).get("entities", [])}
                point = entities.get(desired["point_id"], {})
                actual = [point.get("x"), point.get("y")]
                if point.get("type") != "point" or any(type(v) not in (int, float) or not math.isfinite(v) for v in actual) or math.dist(actual, desired["position_m"]) > 1e-9:
                    problems.append(f"sketch point {desired['point_id']} lacks expected source SI coordinates")
        except (OSError, ValueError, KeyError, TypeError, AttributeError):
            problems.append("sketch point source is absent or malformed")
    if "expected_construction_entities" in task:
        try:
            sketches = json.loads((document / "graph" / "sketches.json").read_text(encoding="utf-8"))["sketches"]
            by_id = {sketch["id"]: sketch for sketch in sketches}
            for desired in task["expected_construction_entities"]:
                entities = {entity["id"]: entity for entity in by_id.get(desired["sketch_id"], {}).get("entities", [])}
                entity = entities.get(desired["entity_id"])
                if entity is None or entity.get("construction", False) is not desired["construction"] or entity.get("type") != desired["type"]:
                    problems.append(f"construction entity {desired['entity_id']} lacks expected stable type/flag")
        except (OSError, ValueError, KeyError, TypeError, AttributeError):
            problems.append("construction sketch source is absent or malformed")
    if "expected_imported_meshes" in task:
        try:
            nodes = json.loads((document / "graph" / "features.json").read_text(encoding="utf-8"))["feature_nodes"]
            by_id = {node["id"]: node["definition"] for node in nodes}
            for desired in task["expected_imported_meshes"]:
                definition = by_id.get(desired["id"], {})
                source = definition.get("source", "")
                safe = re.fullmatch(r"imports/[a-z0-9_]+(?:[.-][a-z0-9_]+)*\.stl", source)
                asset = (document / source).resolve()
                if not safe or not asset.is_relative_to(document.resolve()) or definition.get("type") != "imported_mesh" or definition.get("source_unit") != desired["source_unit"] or definition.get("sha256") != desired["sha256"] or hashlib.sha256(asset.read_bytes()).hexdigest() != desired["sha256"]:
                    problems.append(f"imported mesh {desired['id']} lacks expected explicit-unit source")
        except (OSError, ValueError, KeyError, TypeError):
            problems.append("imported mesh source is absent or malformed")
    if "expected_selection_ref" in task:
        try:
            refs = json.loads((document / "graph" / "semantic_refs.json").read_text(encoding="utf-8"))["semantic_refs"]
            desired = task["expected_selection_ref"]
            matches = [r for r in refs if r.get("ref_id") == desired["ref_id"] and r.get("kind") == "face" and r.get("semantic", {}).get("created_by") == desired["created_by"] and r.get("semantic", {}).get("role") == desired["role"]]
            if len(matches) != 1:
                problems.append("selection semantic reference identity/owner is missing or ambiguous")
        except (OSError, ValueError, KeyError, TypeError):
            problems.append("selection semantic reference source is absent or malformed")
    if "expected_parameter_expressions" in task:
        try:
            params = json.loads((document / "graph" / "parameters.json").read_text(encoding="utf-8"))["parameters"]
            expressions = {p["name"]: p["expr"] for p in params.values()}
            if any(expressions.get(name) != value for name, value in task["expected_parameter_expressions"].items()):
                problems.append("selection edit changed unrelated named parameter expressions")
        except (OSError, ValueError, KeyError, TypeError):
            problems.append("selection parameter source is absent or malformed")
    if "expected_feature_editor" in task:
        try:
            features = json.loads((document / "graph" / "features.json").read_text(encoding="utf-8"))
            desired = task["expected_feature_editor"]
            if features["feature_graph"]["order"] != desired["order"]:
                problems.append("feature editor order differs from the required source order")
            by_id = {node["id"]: node for node in features["feature_nodes"]}
            for node_id, expected in desired["nodes"].items():
                node = by_id.get(node_id, {})
                if node.get("suppressed") != expected["suppressed"] or any(node.get("definition", {}).get(k) != v for k, v in expected.get("definition", {}).items()):
                    problems.append(f"feature editor source {node_id} differs from required identity/expression/suppression")
        except (OSError, ValueError, KeyError, TypeError):
            problems.append("feature editor source is absent or malformed")
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
        if match is None and task["expected_mates"] > 0:
            problems.append("regeneration reported no mate error")
        elif match is not None and float(match.group(1)) > MATE_TOLERANCE_M:
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
    if "expected_sketch_geometry" in task:
        sketches = json.loads((document / "graph" / "sketches.json").read_text(encoding="utf-8"))["sketches"]
        by_id = {s["id"]: s for s in sketches}
        for desired in task["expected_sketch_geometry"]:
            sketch = by_id.get(desired["id"])
            if sketch is None:
                problems.append("required imported sketch is absent")
                continue
            entities = sketch["entities"]
            counts = {kind: sum(e["type"] == kind for e in entities) for kind in ("point", "line", "circle", "arc")}
            if counts != desired["counts"]:
                problems.append("imported sketch primitive counts differ")
            try:
                points = [[float(e["x"]), float(e["y"])] for e in entities if e["type"] == "point"]
                if not points or not all(math.isfinite(v) for p in points for v in p):
                    raise ValueError("invalid point coordinates")
                bounds = [[min(p[a] for p in points) for a in (0,1)], [max(p[a] for p in points) for a in (0,1)]]
                if any(abs(a-b) > 1e-9 for actual, expected in zip(bounds, desired["bounds_m"]) for a,b in zip(actual,expected)):
                    problems.append("imported sketch SI bounds differ")
            except (ValueError, TypeError, KeyError):
                problems.append("imported sketch coordinates are not finite SI literals")
    if "expected_bom_name" in task:
        problems.extend(bom_artifact_problems(document.parent / task["expected_bom_name"], task))
    if "expected_dxf_name" in task:
        problems.extend(drawing_dxf_problems(document.parent / task["expected_dxf_name"], task))
    if "expected_pdf_name" in task:
        pdf_path = document.parent / task["expected_pdf_name"]
        try:
            pdf = pdf_path.read_bytes()
        except OSError:
            pdf = b""
        if not pdf.startswith(b"%PDF-1.7") or not pdf.rstrip().endswith(b"%%EOF"):
            problems.append("required PDF artifact is absent or malformed")
        boxes = re.findall(rb"/MediaBox\s*\[([^\]]+)\]", pdf)
        expected_papers = task["expected_pdf_paper_mm"]
        if len(boxes) != task["expected_pdf_pages"] or len(boxes) != len(expected_papers):
            problems.append("PDF page count differs")
        for box, desired in zip(boxes, expected_papers):
            try:
                coordinates = [float(v) for v in box.split()]
            except ValueError:
                coordinates = []
            if len(coordinates) != 4 or not all(math.isfinite(v) for v in coordinates) or any(abs(a-b*72.0/25.4) > 1e-3 for a,b in zip(coordinates[2:],desired)):
                problems.append("PDF paper dimensions differ from requested millimetres")
    if "expected_title_blocks" in task:
        drawing = json.loads((document / "graph" / "drawings.json").read_text(encoding="utf-8")).get("drawing") or {}
        blocks = {sheet["id"]: sheet["title_block"] for sheet in drawing.get("sheets", []) if sheet.get("title_block") is not None}
        if blocks != task["expected_title_blocks"]:
            problems.append("authored title-block metadata differs")
    if "expected_details" in task:
        drawing = json.loads((document / "graph" / "drawings.json").read_text(encoding="utf-8")).get("drawing") or {}
        views = {v["id"]: v for sheet in drawing.get("sheets", []) for v in sheet.get("views", []) if v.get("detail")}
        reported = {id_: (parent, float(mag), float(scale), int(count)) for id_, parent, mag, scale, count in DETAIL_LINE.findall(regen_output)}
        desired = task["expected_details"]
        if sorted(views) != sorted(v["view_id"] for v in desired):
            problems.append("detail view IDs differ")
        for detail in desired:
            id_ = detail["view_id"]
            measured = reported.get(id_)
            if measured is None or measured[0] != detail["parent_view_id"] or not all(math.isfinite(v) for v in measured[1:3]) or abs(measured[1]-detail["magnification"]) > 1e-9 or abs(measured[2]-detail["scale"]) > 1e-9 or measured[3] != detail["segment_count"]:
                problems.append(f"detail {id_} lacks matching fresh scale/clipping evidence")
            authored = views.get(id_, {}).get("detail", {})
            if any(authored.get(key) != detail[key] for key in ("parent_view_id", "region_min_m", "region_max_m", "magnification")) or abs(views.get(id_,{}).get("scale",-1)-detail["scale"]) > 1e-9:
                problems.append(f"detail {id_} parent/region/scale authoring differs")
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
    counterbore_task = next(t for t in json.loads(TASKS.read_text(encoding="utf-8"))["tasks"] if t["id"] == "counterbore_seat_depth")
    seed = ROOT / counterbore_task["source"]
    assert check_counterbore_source(counterbore_task, seed) == []
    with tempfile.TemporaryDirectory() as folder:
        candidate = Path(folder) / "plate.ocad.d"
        shutil.copytree(seed, candidate)
        path = candidate / "graph/features.json"
        data = json.loads(path.read_text(encoding="utf-8"))
        next(n for n in data["feature_nodes"] if n["id"] == "feature:seat")["definition"]["type"] = "hole"
        path.write_text(json.dumps(data), encoding="utf-8")
        assert check_counterbore_source(counterbore_task, candidate)
    countersink_task = next(t for t in json.loads(TASKS.read_text(encoding="utf-8"))["tasks"] if t["id"] == "countersink_angle")
    sink_seed = ROOT / countersink_task["source"]
    assert check_countersink_source(countersink_task, sink_seed) == []
    with tempfile.TemporaryDirectory() as folder:
        candidate = Path(folder) / "plate.ocad.d"
        shutil.copytree(sink_seed, candidate)
        path = candidate / "graph/features.json"
        data = json.loads(path.read_text(encoding="utf-8"))
        next(n for n in data["feature_nodes"] if n["id"] == "feature:seat")["definition"]["seat"]["type"] = "counterbore"
        path.write_text(json.dumps(data), encoding="utf-8")
        passed, problems = check(countersink_task, candidate, f"volume_m3: {countersink_task['expected_volume_mm3']/1e9}\n")
        assert not passed and problems  # A claimed mass cannot substitute a different seat.
    thread_task = next(t for t in json.loads(TASKS.read_text())["tasks"] if t["id"] == "nominal_thread_handedness")
    with tempfile.TemporaryDirectory() as folder:
        candidate = Path(folder) / "thread.ocad.d"
        shutil.copytree(ROOT / thread_task["source"], candidate)
        path = candidate / "graph/features.json"
        data = json.loads(path.read_text())
        definition = next(n for n in data["feature_nodes"] if n["id"] == "feature:thread")["definition"]
        definition["handedness"] = "left"
        path.write_text(json.dumps(data))
        assert check_nominal_thread_source(thread_task, candidate) == []
        for field, wrong in [("handedness", "right"), ("type", "helix_sweep"), ("type", "extrude"), ("representation", "cosmetic"), ("turns", 4), ("face_ref", "ref:face:foreign")]:
            old = definition[field]
            definition[field] = wrong
            path.write_text(json.dumps(data))
            assert not check(thread_task, candidate, f"volume_m3: {thread_task['expected_volume_mm3']/1e9}\n")[0]
            definition[field] = old
    draft_task = next(t for t in json.loads(TASKS.read_text(encoding="utf-8"))["tasks"] if t["id"] == "cylindrical_draft_angle")
    with tempfile.TemporaryDirectory() as folder:
        candidate = Path(folder) / "draft.ocad.d"
        shutil.copytree(ROOT / draft_task["source"], candidate)
        path = candidate / "graph/parameters.json"
        data = json.loads(path.read_text(encoding="utf-8"))
        data["parameters"]["param:draft_angle"]["expr"] = "-5 deg"
        path.write_text(json.dumps(data), encoding="utf-8")
        assert check_cylindrical_draft_source(draft_task, candidate) == []
        data["parameters"]["param:draft_angle"]["expr"] = "5 deg"
        path.write_text(json.dumps(data), encoding="utf-8")
        assert not check(draft_task, candidate, f"volume_m3: {draft_task['expected_volume_mm3']/1e9}\n")[0]
        data["parameters"]["param:draft_angle"]["expr"] = "-5 deg"
        path.write_text(json.dumps(data), encoding="utf-8")
        path = candidate / "graph/features.json"
        data = json.loads(path.read_text(encoding="utf-8"))
        next(n for n in data["feature_nodes"] if n["id"] == "feature:draft")["definition"]["type"] = "draft"
        path.write_text(json.dumps(data), encoding="utf-8")
        assert not check(draft_task, candidate, f"volume_m3: {draft_task['expected_volume_mm3']/1e9}\n")[0]
    rib_task = next(t for t in json.loads(TASKS.read_text(encoding="utf-8"))["tasks"] if t["id"] == "tapered_rib_height")
    with tempfile.TemporaryDirectory() as folder:
        candidate = Path(folder) / "rib.ocad.d"
        shutil.copytree(ROOT / rib_task["source"], candidate)
        path = candidate / "graph/parameters.json"
        data = json.loads(path.read_text(encoding="utf-8"))
        data["parameters"]["param:rib_height"]["expr"] = "12 mm"
        path.write_text(json.dumps(data), encoding="utf-8")
        assert check_tapered_rib_source(rib_task, candidate) == []
        path = candidate / "graph/features.json"
        data = json.loads(path.read_text(encoding="utf-8"))
        next(n for n in data["feature_nodes"] if n["id"] == "feature:rib")["definition"]["type"] = "rib"
        path.write_text(json.dumps(data), encoding="utf-8")
        assert not check(rib_task, candidate, "volume_m3: 0.000013224\n")[0]
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
    detail_sheet = ROOT / "examples" / "detail_plate_drawing.ocad.d"
    details = {"expected_details": [{"view_id": "view:detail", "parent_view_id": "view:front", "region_min_m": [0.06,0.04], "region_max_m": [0.09,0.07], "magnification": 2.0, "scale": 1.0, "segment_count": 2}]}
    detail_evidence = "detail: view:detail parent: view:front magnification: 2 scale: 1 segments: 2\n"
    assert check(details, detail_sheet, detail_evidence) == (True, [])
    assert not check(details, detail_sheet, "")[0]
    assert not check(details, detail_sheet, detail_evidence.replace("segments: 2", "segments: 4"))[0]
    assert not check(details, detail_sheet, detail_evidence.replace("scale: 1", "scale: 2"))[0]
    wrong_details = json.loads(json.dumps(details))
    wrong_details["expected_details"][0]["region_min_m"] = [0.05,0.04]
    assert not check(wrong_details, detail_sheet, detail_evidence)[0]
    title_sheet = ROOT / "examples" / "title_plate_drawing.ocad.d"
    titles = {"expected_title_blocks": {"sheet:a4": {"title": "Inspection plate", "drawing_number": "MUSUBI-P12-005", "revision": "A", "date": "2026-10-12", "author": "MusubiCAD example", "material": "Steel", "scale_note": "FRONT 1:2"}}}
    assert check(titles, title_sheet, "") == (True, [])
    wrong_title = json.loads(json.dumps(titles))
    wrong_title["expected_title_blocks"]["sheet:a4"]["revision"] = "B"
    assert not check(wrong_title, title_sheet, "")[0]
    assert not check({"expected_title_blocks": {}}, title_sheet, "")[0]
    dxf_task = {"expected_dxf_name": "title_plate_drawing.dxf", "expected_dxf_units": 4, "expected_dxf_paper_mm": [210,297], "expected_dxf_text": ["Inspection plate", "A", "FRONT 1:2"], "expected_dxf_model_bounds_mm": [[50,100],[90,130]]}
    assert check(dxf_task, title_sheet, "") == (True, [])
    for key, bad in [("expected_dxf_name", "missing.dxf"), ("expected_dxf_units", 1), ("expected_dxf_paper_mm", [297,210]), ("expected_dxf_text", ["missing label"]), ("expected_dxf_model_bounds_mm", [[50,100],[91,130]])]:
        assert not check({**dxf_task, key: bad}, title_sheet, "")[0]
    dxf_imported = ROOT / "examples" / "dxf_import_plate.ocad.d"
    sketch_task = {"expected_sketch_geometry": [{"id": "sketch:dxf", "counts": {"point":4,"line":4,"circle":0,"arc":0}, "bounds_m": [[0,0],[0.08,0.06]]}]}
    assert check(sketch_task, dxf_imported, "") == (True, [])
    wrong_sketch = json.loads(json.dumps(sketch_task))
    wrong_sketch["expected_sketch_geometry"][0]["bounds_m"][1][0] = 80
    assert not check(wrong_sketch, dxf_imported, "")[0]
    wrong_sketch["expected_sketch_geometry"][0]["id"] = "sketch:missing"
    assert not check(wrong_sketch, dxf_imported, "")[0]
    bom_sheet = ROOT / "examples" / "bom_nested_assembly.ocad.d"
    bom_task = {"expected_bom_name": "bom_nested_assembly.bom.json", "expected_bom_summary": {"assembly_doc": "doc:bom_nested", "declared_component_count": 2, "top_level_instance_count": 3, "leaf_part_count": 1, "leaf_instance_count": 7}, "expected_bom_rows": [{"source_doc": "doc:title_plate", "source_path": "parts/plate.ocad.d", "quantity": 7}]}
    assert check(bom_task, bom_sheet, "") == (True, [])
    assert check({"expected_instances": 3, "expected_mates": 0}, bom_sheet, "") == (True, [])
    assert not check({**bom_task, "expected_bom_name": "missing.bom.json"}, bom_sheet, "")[0]
    assert not check({**bom_task, "expected_bom_summary": {"leaf_instance_count": 8}}, bom_sheet, "")[0]
    assert not check({**bom_task, "expected_bom_rows": []}, bom_sheet, "")[0]
    with tempfile.TemporaryDirectory() as directory:
        artifact = Path(directory) / "bom.json"
        bad = json.loads((ROOT / "examples" / "bom_nested_assembly.bom.json").read_text(encoding="utf-8"))
        bad["rows"][0]["instance_paths"] = [["instance:duplicate"]] * 7
        artifact.write_text(json.dumps(bad), encoding="utf-8")
        assert bom_artifact_problems(artifact, bom_task)
        artifact.write_text("{bad JSON", encoding="utf-8")
        assert bom_artifact_problems(artifact, bom_task)
    printable_sheet = ROOT / "examples" / "printable_plate_drawing.ocad.d"
    pdf_task = {"expected_pdf_name": "printable_plate_drawing.pdf", "expected_pdf_pages": 2, "expected_pdf_paper_mm": [[210,297],[297,210]]}
    assert check(pdf_task, printable_sheet, "") == (True, [])
    assert not check({**pdf_task, "expected_pdf_name": "missing.pdf"}, printable_sheet, "")[0]
    assert not check({**pdf_task, "expected_pdf_pages": 1}, printable_sheet, "")[0]
    assert not check({**pdf_task, "expected_pdf_paper_mm": [[297,210],[297,210]]}, printable_sheet, "")[0]
    projection_doc = ROOT / "examples" / "projected_circle_plate.ocad.d"
    projection_task = {"expected_projected_sketches": [{"id": "feature:projection", "sketch_id": "sketch:projection", "projections": [{"source_feature": "feature:source_sketch", "source_entity": "ent:source_circle", "curve_id": "ent:projected_curve", "point_ids": ["ent:projected_center"], "construction": False}]}]}
    assert check(projection_task, projection_doc, "") == (True, [])
    for field, value in [("id", "feature:missing"), ("sketch_id", "sketch:wrong"), ("projections", [])]:
        wrong_projection = json.loads(json.dumps(projection_task))
        wrong_projection["expected_projected_sketches"][0][field] = value
        assert not check(wrong_projection, projection_doc, "")[0]
    cubic_doc = ROOT / "examples" / "cubic_plate.ocad.d"
    cubic_task = next(t for t in json.loads(TASKS.read_text(encoding="utf-8"))["tasks"] if t["id"] == "cubic_edit_control")
    source_task = {"expected_cubic_sources": json.loads(json.dumps(cubic_task["expected_cubic_sources"]))}
    source_task["expected_cubic_sources"][0]["parameter_expressions"]["a"] = "40 mm"
    assert check(source_task, cubic_doc, "") == (True, [])
    for field, value in [("entity_id", "ent:missing"), ("control_points", ["ent:p0"]), ("point_coordinates", {"ent:p1": [0.04, 0.]})]:
        wrong = json.loads(json.dumps(source_task))
        wrong["expected_cubic_sources"][0][field] = value
        assert not check(wrong, cubic_doc, "")[0]
    assert not check(cubic_task, cubic_doc, "volume_m3: 0.00000144\n")[0]
    ellipse_doc = ROOT / "examples" / "ellipse_plate.ocad.d"
    ellipse_task = {"expected_ellipse_sources": [{"sketch_id": "sketch:ellipse", "entity_id": "ent:ellipse", "center": "ent:center", "major_radius": "major_radius", "minor_radius": "minor_radius", "axis_angle": "axis_angle_rad"}]}
    assert check(ellipse_task, ellipse_doc, "") == (True, [])
    for field, value in [("entity_id", "ent:missing"), ("major_radius", 0.03), ("axis_angle", 0.5235987755982988)]:
        wrong_ellipse = json.loads(json.dumps(ellipse_task))
        wrong_ellipse["expected_ellipse_sources"][0][field] = value
        assert not check(wrong_ellipse, ellipse_doc, "")[0]
    edited_doc = ROOT / "examples" / "trim_extend_plate.ocad.d"
    edited_task = {"expected_sketch_points": [{"sketch_id": "sketch:base", "point_id": "ent:edited_end", "position_m": [0.04, 0.03]}]}
    assert check(edited_task, edited_doc, "") == (True, [])
    for field, value in [("point_id", "ent:missing"), ("position_m", [40,30])]:
        wrong_edit = json.loads(json.dumps(edited_task))
        wrong_edit["expected_sketch_points"][0][field] = value
        assert not check(wrong_edit, edited_doc, "")[0]
    construction_doc = ROOT / "examples" / "construction_plate.ocad.d"
    construction_task = {"expected_construction_entities": [{"sketch_id": "sketch:base", "entity_id": "ent:guide_line", "type": "line", "construction": True}]}
    assert check(construction_task, construction_doc, "") == (True, [])
    for field, value in [("entity_id", "ent:missing"), ("construction", False), ("type", "circle")]:
        wrong_construction = json.loads(json.dumps(construction_task))
        wrong_construction["expected_construction_entities"][0][field] = value
        assert not check(wrong_construction, construction_doc, "")[0]
    mesh_doc = ROOT / "examples" / "stl_import_plate.ocad.d"
    mesh_task = {"expected_imported_meshes": [{"id": "feature:stl_plate", "source_unit": "mm", "sha256": "860dfa927f74c7b1ef3b6abbdda3ddfa3ecdb077127fd7f2f8b2ebccce7510b0"}]}
    assert check(mesh_task, mesh_doc, "") == (True, [])
    for field, value in [("id", "feature:missing"), ("source_unit", "m"), ("sha256", "0" * 64)]:
        wrong_mesh = json.loads(json.dumps(mesh_task))
        wrong_mesh["expected_imported_meshes"][0][field] = value
        assert not check(wrong_mesh, mesh_doc, "")[0]
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
    arm = ROOT / "examples" / "robot_arm_assembly.ocad.d"
    assembly = json.loads((arm / "graph" / "assemblies.json").read_text())["assembly"]
    exploded_task = {"expected_exploded_views": assembly["exploded_views"], "expected_mechanical_placements": {i["id"]: i["placement"] for i in assembly["instances"]}}
    assert check(exploded_task, arm, "") == (True, [])
    assert not check({**exploded_task, "expected_exploded_views": [{"id": "view:exploded", "name": "Wrong", "offsets": []}]}, arm, "")[0]
    assert not check({**exploded_task, "expected_mechanical_placements": {}}, arm, "")[0]
    coupled = ROOT / "examples" / "coupled_arm.ocad.d"
    source = json.loads((coupled / "graph" / "assemblies.json").read_text())["assembly"]
    coupling_task = {"expected_joint_couplings": source["couplings"], "expected_mechanical_placements": {i["id"]: i["placement"] for i in source["instances"]}}
    assert check(coupling_task, coupled, "") == (True, [])
    assert not check({**coupling_task, "expected_joint_couplings": []}, coupled, "")[0]
    assert not check({**coupling_task, "expected_mechanical_placements": {}}, coupled, "")[0]
    editor_task = next(t for t in json.loads(TASKS.read_text(encoding="utf-8"))["tasks"] if t["id"] == "feature_tree_edit")
    with tempfile.TemporaryDirectory() as editor_tmp:
        editor_doc = Path(editor_tmp) / "editor.ocad.d"
        shutil.copytree(ROOT / "examples/cubic_plate.ocad.d", editor_doc)
        features_file = editor_doc / "graph" / "features.json"
        features = json.loads(features_file.read_text(encoding="utf-8"))
        base = next(n for n in features["feature_nodes"] if n["id"] == "feature:cubic_body")
        base["suppressed"] = True
        second = json.loads(json.dumps(base))
        second.update(id="feature:second", name="Second cubic prism", suppressed=False)
        second["definition"]["length_expr"] = "10 mm"
        second["definition"]["extent"]["length"]["value_si"] = 0.012
        features["feature_nodes"].append(second)
        features["feature_graph"]["order"] = list(editor_task["expected_feature_editor"]["order"])
        features_file.write_text(json.dumps(features), encoding="utf-8")
        assert check(editor_task, editor_doc, "volume_m3: 0.0000024\n")[0], check(editor_task, editor_doc, "volume_m3: 0.0000024\n")[1]
        second["definition"]["profile_ref"] = "wrong:profile"
        features_file.write_text(json.dumps(features), encoding="utf-8")
        assert not check(editor_task, editor_doc, "volume_m3: 0.0000024\n")[0]
        second["definition"]["profile_ref"] = "profile:bezier:ent:cubic"
        features["feature_graph"]["order"].reverse()
        features_file.write_text(json.dumps(features), encoding="utf-8")
        assert not check(editor_task, editor_doc, "volume_m3: 0.0000024\n")[0]
    selection_task = next(t for t in json.loads(TASKS.read_text(encoding="utf-8"))["tasks"] if t["id"] == "semantic_selection_edit")
    with tempfile.TemporaryDirectory() as selection_tmp:
        selection_doc = Path(selection_tmp) / "selection.ocad.d"
        shutil.copytree(ROOT / "examples/ellipse_plate.ocad.d", selection_doc)
        features_file = selection_doc / "graph" / "features.json"
        features = json.loads(features_file.read_text(encoding="utf-8"))
        next(n for n in features["feature_nodes"] if n["id"] == "feature:ellipse_body")["definition"]["length_expr"] = "10 mm"
        features_file.write_text(json.dumps(features), encoding="utf-8")
        ref_file = selection_doc / "graph" / "semantic_refs.json"
        refs = {"semantic_refs":[{"ref_id":"ref:face:ellipse_top","kind":"face","semantic":{"created_by":"feature:ellipse_body","role":"top"}}]}
        ref_file.write_text(json.dumps(refs), encoding="utf-8")
        output = f"volume_m3: {selection_task['expected_volume_mm3'] * 1e-9}\n"
        assert check(selection_task, selection_doc, output)[0], check(selection_task, selection_doc, output)[1]
        refs["semantic_refs"][0]["semantic"]["created_by"] = "feature:wrong_name_hint"
        ref_file.write_text(json.dumps(refs), encoding="utf-8")
        assert not check(selection_task, selection_doc, output)[0]
        refs["semantic_refs"][0]["semantic"]["created_by"] = "feature:ellipse_body"
        ref_file.write_text(json.dumps(refs), encoding="utf-8")
        params_file = selection_doc / "graph" / "parameters.json"
        params = json.loads(params_file.read_text(encoding="utf-8"))
        params["parameters"]["param:thickness"]["expr"] = "10 mm"
        params_file.write_text(json.dumps(params), encoding="utf-8")
        assert not check(selection_task, selection_doc, output)[0]
    sketch_task = next(t for t in json.loads(TASKS.read_text(encoding="utf-8"))["tasks"] if t["id"] == "interactive_sketch_edit")
    with tempfile.TemporaryDirectory() as sketch_tmp:
        sketch_doc = Path(sketch_tmp) / "sketch.ocad.d"
        shutil.copytree(ROOT / "examples/ellipse_plate.ocad.d", sketch_doc)
        sketches_file = sketch_doc / "graph" / "sketches.json"
        sketches = json.loads(sketches_file.read_text(encoding="utf-8"))
        ellipse = next(e for e in sketches["sketches"][0]["entities"] if e["id"] == "ent:ellipse")
        ellipse.update(major_radius="40 mm", minor_radius="15 mm", axis_angle="0 deg")
        sketches_file.write_text(json.dumps(sketches), encoding="utf-8")
        output = f"volume_m3: {sketch_task['expected_volume_mm3'] * 1e-9}\n"
        assert check(sketch_task, sketch_doc, output)[0], check(sketch_task, sketch_doc, output)[1]
        ellipse["center"] = "ent:anchor"
        sketches_file.write_text(json.dumps(sketches), encoding="utf-8")
        assert not check(sketch_task, sketch_doc, output)[0]
        ellipse["center"] = "ent:center"
        sketches_file.write_text(json.dumps(sketches), encoding="utf-8")
        assert not check(sketch_task, sketch_doc, "volume_m3: 0.00001\n")[0]
    import copy
    task = next(t for t in json.loads(TASKS.read_text())["tasks"] if t["id"] == "articulated_copy_zero")
    task = copy.deepcopy(task)
    task["expected_articulations"]["instance:left"]["positions"][0]["position"] = "90 deg"
    task["expected_articulated_leaves"][1]["placement"]["rotation"] = [[0,-1,0],[1,0,0],[0,0,1]]
    task["expected_articulated_leaves"][1]["bounds_size_m"] = [.02,.01,.03]
    report = {"volume_m3":24e-6,"assembly_leaves":copy.deepcopy(task["expected_articulated_leaves"])}
    for leaf in report["assembly_leaves"]:
        leaf["volume_m3"] = 6e-6
    document = ROOT / "examples/articulated_copies.ocad.d"
    assert not check_articulated_evidence(task,document,report)
    for field, value in [("volume_m3",float("nan")),("source_doc","doc:wrong"),("bounds_size_m",[.01,.01,.03]),("source_revision","stale")]:
        wrong = copy.deepcopy(report)
        wrong["assembly_leaves"][1][field] = value
        assert check_articulated_evidence(task,document,wrong)
    wrong = copy.deepcopy(report)
    wrong["assembly_leaves"][1]["placement"]["rotation"] = [[1,0,0],[0,1,0],[0,0,1]]
    assert check_articulated_evidence(task,document,wrong)
    wrong = copy.deepcopy(report)
    wrong["assembly_leaves"].pop()
    assert check_articulated_evidence(task,document,wrong)
    wrong_task = copy.deepcopy(task)
    wrong_task["expected_child_tree_sha256"] = "wrong"
    assert check_articulated_evidence(wrong_task,document,report)
    for task_id, angular in [("parent_context_slider",False),("parent_context_hinge",True)]:
        task = copy.deepcopy(next(t for t in json.loads(TASKS.read_text())["tasks"] if t["id"] == task_id))
        document = ROOT / task["source"]
        source = json.loads((document/"graph/assemblies.json").read_text())["assembly"]
        task["expected_articulations"] = {i["id"]:i["articulation"] for i in source["instances"] if i.get("articulation")}
        if angular:
            task["expected_articulated_leaves"][1]["placement"]["rotation"] = [[-1,0,0],[0,-1,0],[0,0,1]]
            task["expected_articulated_leaves"][1]["bounds_size_m"] = [.01,.02,.03]
        else:
            task["expected_articulated_leaves"][1]["placement"]["translation_m"][1] = -.025
        report = {"context_dof":0,"volume_m3":30e-6,"assembly_leaves":copy.deepcopy(task["expected_articulated_leaves"])}
        for leaf in report["assembly_leaves"]: leaf["volume_m3"] = 6e-6
        assert not check_articulated_evidence(task,document,report)
        wrong = copy.deepcopy(report); wrong["context_dof"] = 1
        assert check_articulated_evidence(task,document,wrong)
        wrong = copy.deepcopy(report); wrong.pop("context_dof")
        assert check_articulated_evidence(task,document,wrong)
        wrong = copy.deepcopy(report); wrong["assembly_leaves"][3]["placement"]["translation_m"][0] += .025
        assert check_articulated_evidence(task,document,wrong)
        wrong_task = copy.deepcopy(task)
        relation = wrong_task["expected_articulations"]["instance:left"]["context_constraints"][0]["relation"]
        relation["angle" if angular else "distance"] = "0 deg" if angular else "0 mm"
        assert check_articulated_evidence(wrong_task,document,report)
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
