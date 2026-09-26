"""Evaluate an MCP agent host authoring MusubiCAD parts (MCAD-P7-012).

Each task in ``tools/mcp_eval_tasks.json`` runs Claude Code headless against
``opencad mcp`` with only the MusubiCAD tools allowed.  The result is then
checked independently by regenerating the document with ``opencad regen``:

- the solid volume must match the analytic value;
- the named parameters must exist;
- for assembly tasks, the instance and mate counts must match and the mates
  must be satisfied.

The harness records turns, cost, and duration for each task.  It calls a
paid model, so it is never run in CI.  Run it by hand after changes to the
MCP surface or the authoring guide:

    python tools/mcp_eval.py --opencad target/debug/opencad.exe
    python tools/mcp_eval.py --only enclosure --out eval.json
    python tools/mcp_eval.py --self-test   # checker only, no model calls
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
TASKS = ROOT / "tools" / "mcp_eval_tasks.json"

# Agreement with the analytic volume, in cubic millimetres.  Expected values
# are given to 0.1 mm^3, and OCCT volumes are exact to far better than that.
VOLUME_TOLERANCE_MM3 = 0.1

VOLUME_LINE = re.compile(r"^volume_m3:\s*([-+0-9.eE]+)\s*$", re.MULTILINE)
MATE_ERROR_LINE = re.compile(r"^mate_max_error:\s*([-+0-9.eE]+)\s*$", re.MULTILINE)

# Largest mate residual, in metres, an assembly task accepts as satisfied.
MATE_TOLERANCE_M = 1e-6

INSTRUCTIONS = (
    "Use only the musubicad MCP tools. Read the server instructions and the "
    "authoring guide resource first. Always dry-run before applying and fix "
    "every reported error and sketch_under_constrained warning. Finish by "
    "regenerating the document."
)


def regenerated_volume_mm3(regen_output: str) -> float | None:
    """Parse the solid volume, in mm^3, from `opencad regen` output."""
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


def check(task: dict, document: Path, regen_output: str) -> tuple[bool, list[str]]:
    """Independently verify a finished task; returns (passed, problems)."""
    problems = []
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
    return not problems, problems


def run_task(task: dict, opencad: Path, claude: str, timeout_s: int) -> dict:
    workdir = Path(tempfile.mkdtemp(prefix=f"mcp-eval-{task['id']}-"))
    document = workdir / task["document"]
    if "source" in task:
        shutil.copytree(ROOT / task["source"], document)
    placeholders = {
        "document": document.as_posix(),
        "workdir": workdir.as_posix(),
        "root": ROOT.as_posix(),
    }
    # Setup steps are `opencad` argument lists, e.g. exporting a STEP file
    # the agent must import.  They run before the agent starts.
    for step in task.get("setup", []):
        subprocess.run(
            [str(opencad), *[arg.format(**placeholders) for arg in step]],
            check=True,
            capture_output=True,
        )
    config = workdir / "mcp.json"
    config.write_text(
        json.dumps(
            {"mcpServers": {"musubicad": {"command": str(opencad), "args": ["mcp"]}}}
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
        [str(opencad), "regen", str(document)],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    passed, problems = check(task, document, regen.stdout)
    return {
        "id": task["id"],
        "passed": passed,
        "problems": problems,
        "volume_mm3": regenerated_volume_mm3(regen.stdout),
        "expected_volume_mm3": task["expected_volume_mm3"],
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
    parser.add_argument("--opencad", type=Path, default=ROOT / "target" / "debug" / "opencad")
    parser.add_argument("--claude", default="claude")
    parser.add_argument("--only", action="append", help="task id to run (repeatable)")
    parser.add_argument("--timeout", type=int, default=900, help="seconds per task")
    parser.add_argument("--out", type=Path, help="write the JSON report here")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return 0

    opencad = args.opencad
    if not opencad.exists() and opencad.with_suffix(".exe").exists():
        opencad = opencad.with_suffix(".exe")
    tasks = json.loads(TASKS.read_text(encoding="utf-8"))["tasks"]
    if args.only:
        tasks = [task for task in tasks if task["id"] in args.only]
    results = [run_task(task, opencad.resolve(), args.claude, args.timeout) for task in tasks]
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
