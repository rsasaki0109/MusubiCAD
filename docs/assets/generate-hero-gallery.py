"""Render the README gallery from real checked-in MusubiCAD examples.

Build the current native CLI, install Pillow for decoding/verification, then run:

    cargo build -p opencad-cli
    python docs/assets/generate-hero-gallery.py --musubicad target/debug/musubicad

The eight CPU clips use the existing parameter/joint animation commands. The
two presentation clips need a Vulkan adapter (Mesa's software adapter works).
No source documents are written. GIF pixels come directly from the native CLI;
Pillow only reads them to check dimensions, motion, timing, and looping.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile

from PIL import Image, ImageSequence


ROOT = Path(__file__).resolve().parents[2]
OUTPUT = Path("docs/assets/hero-gallery")
SIZE = ["--width", "600", "--height", "400"]


def sweep(parameter: str, start: str, end: str, view: str = "iso") -> list[str]:
    return [
        "--param", parameter, "--from", start, "--to", end,
        "--steps", "14", "--view", view, "--style", "studio", "--fps", "10", *SIZE,
    ]


# Keep the order in sync with the two-column opening README gallery.
CLIPS = [
    ("six-axis-arm", "Six axes, one Design Graph", "six_axis_arm", "animate-joints", [
        "--pose", "base_yaw=65deg,shoulder=20deg,elbow=-35deg,wrist_roll=40deg,wrist_pitch=-25deg,flange_roll=60deg",
        "--pose", "base_yaw=-55deg,shoulder=-15deg,elbow=25deg,wrist_roll=-50deg,wrist_pitch=35deg,flange_roll=-60deg",
        "--frames-per-move", "16", "--style", "studio", "--fps", "12", *SIZE,
    ]),
    ("actuator-hub", "Change a dimension. Rebuild the solid.", "robot_joint_actuator", "animate-sweep",
     sweep("upper_hub_height", "24mm", "60mm")),
    ("bearing-bore", "Resize a real bearing bore", "bearing_carrier", "animate-sweep",
     sweep("bore_diameter", "8mm", "26mm")),
    ("revolve-angle", "A profile becomes a revolved solid", "physical_rectangular_revolve", "animate-sweep",
     sweep("turn_angle", "60deg", "330deg")),
    ("cylindrical-draft", "Draft about a neutral plane", "cylindrical_draft", "animate-sweep",
     sweep("draft_angle_rad", "2deg", "14deg")),
    ("tapered-rib", "Grow a tapered reinforcement rib", "tapered_rib_plate", "animate-sweep",
     sweep("rib_height", "8mm", "26mm")),
    ("ellipse-profile", "An analytic ellipse drives the solid", "ellipse_plate", "animate-sweep",
     sweep("minor_radius", "8mm", "24mm")),
    ("variable-fillet", "Vary the radius along an edge", "variable_fillet_plate", "animate-sweep",
     sweep("start_radius", "0.5mm", "2.5mm")),
    ("circular-pattern", "One feature, a circular pattern", "bracket_pin_ring", "animate", [
        "--frames", "48", "--orbit-deg", "360", "--pitch-deg", "30", "--fps", "12", *SIZE,
    ]),
    ("feature-build", "Watch the Feature Graph build a part", "robot_joint_actuator", "animate-features", [
        "--frames", "54", "--orbit-deg", "80", "--pitch-deg", "28", "--fps", "12", *SIZE,
    ]),
]


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources(document: Path) -> dict[str, str]:
    return {
        path.relative_to(ROOT).as_posix(): sha256(path)
        for path in sorted(document.rglob("*")) if path.is_file()
    }


def inspect_gif(path: Path) -> dict:
    with Image.open(path) as gif:
        if gif.format != "GIF" or gif.size != (600, 400) or gif.info.get("loop") != 0:
            raise ValueError(f"{path.name}: expected a 600 x 400 infinitely looping GIF")
        durations = []
        pixels = set()
        for frame in ImageSequence.Iterator(gif):
            durations.append(frame.info.get("duration", 0))
            pixels.add(hashlib.sha256(frame.convert("RGB").tobytes()).hexdigest())
        if len(pixels) < 2 or not durations or min(durations) <= 0:
            raise ValueError(f"{path.name}: missing motion or positive frame timing")
    size = path.stat().st_size
    if size > 1_500_000:
        raise ValueError(f"{path.name}: {size} bytes exceeds the 1.5 MB clip budget")
    return {
        "width_px": 600, "height_px": 400, "frames": len(durations),
        "distinct_frames": len(pixels), "duration_ms": sum(durations),
        "loop": "infinite", "bytes": size, "sha256": sha256(path),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--musubicad", default="musubicad", help="Current native CLI executable")
    parser.add_argument("--only", choices=[clip[0] for clip in CLIPS], help="Preview one clip; leave the full-gallery manifest untouched")
    args = parser.parse_args()
    executable = shutil.which(args.musubicad)
    if not executable:
        parser.error(f"executable not found: {args.musubicad}")
    executable = str(Path(executable).resolve())
    destination = ROOT / OUTPUT
    destination.mkdir(parents=True, exist_ok=True)
    records = []
    for slug, title, example, command, flags in CLIPS:
        if args.only and args.only != slug:
            continue
        source = Path("examples") / f"{example}.ocad.d"
        before = sources(ROOT / source)
        if not before:
            raise ValueError(f"missing example: {source}")
        target = OUTPUT / f"{slug}.gif"
        print(f"Rendering {target} from {source}", flush=True)
        with tempfile.TemporaryDirectory(prefix="musubicad-hero-") as temporary:
            rendered = Path(temporary) / target.name
            result = subprocess.run(
                [executable, command, str(source), str(rendered), *flags],
                cwd=ROOT, text=True, capture_output=True,
            )
            if result.returncode:
                raise RuntimeError(f"{slug}: {result.stderr.strip()}")
            metadata = inspect_gif(rendered)
            if sources(ROOT / source) != before:
                raise ValueError(f"{slug}: rendering modified the source example")
            shutil.copyfile(rendered, ROOT / target)
        record = {
            "id": slug, "title": title, "source": source.as_posix(),
            "output": target.as_posix(),
            "command": shlex.join(["musubicad", command, source.as_posix(), target.as_posix(), *flags]),
            "source_sha256": before, "gif": metadata,
        }
        records.append(record)
        print(f"  {metadata['frames']} frames, {metadata['bytes']:,} bytes; source unchanged", flush=True)
    if not args.only:
        total = sum(record["gif"]["bytes"] for record in records)
        if total > 6_000_000:
            raise ValueError(f"gallery exceeds its 6 MB download budget: {total} bytes")
        manifest = {"generator": "docs/assets/generate-hero-gallery.py", "clips": records, "total_bytes": total}
        (destination / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print(f"Gallery complete: {len(records)} clips, {total:,} bytes", flush=True)


if __name__ == "__main__":
    main()
