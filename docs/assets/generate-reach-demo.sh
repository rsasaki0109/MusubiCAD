#!/usr/bin/env bash
# Regenerate the README reach demo: the robot arm falls 20 mm short of a
# target, two verified patches lengthen the upper arm by 40 mm, and the same
# target is reached.  Runs on a temporary copy; the example is not changed.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ASSETS="$ROOT/docs/assets"
WORK="$(mktemp -d)"
trap 'rm -rf -- "$WORK"' EXIT
cp -R "$ROOT/examples/robot_arm_assembly.ocad.d" "$WORK/arm.ocad.d"

musubicad() { cargo run -q -p opencad-cli -- "$@"; }
REACH=(--tool gripper --point 0,40,7mm --target 0,330,63mm)
GIF=(--width 560 --aspect 4:3 --frames-per-move 18 --fps 15)

musubicad reach "$WORK/arm.ocad.d" "${REACH[@]}" --gif "$ASSETS/reach-before.gif" "${GIF[@]}"
musubicad patch "$WORK/arm.ocad.d/parts/upper_arm.ocad.d" \
  "$ROOT/examples/agent/reach_upper_arm_length_patch.json"
musubicad patch "$WORK/arm.ocad.d" "$ROOT/examples/agent/reach_elbow_connector_patch.json"
musubicad reach "$WORK/arm.ocad.d" "${REACH[@]}" --gif "$ASSETS/reach-after.gif" "${GIF[@]}"
echo "wrote reach-before.gif and reach-after.gif"
