#!/usr/bin/env bash
# Regenerate the README web viewer: a self-contained HTML page of the robot arm
# with one slider per joint, and a screenshot of it with the joints moved.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ASSETS="$ROOT/docs/assets"
cargo run -q -p opencad-cli -- export \
  "$ROOT/examples/robot_arm_assembly.ocad.d" "$ASSETS/robot-arm-viewer.html"
# Shoulder 30°, elbow 80°, wrist -45°, in radians as the sliders take them.
node "$ASSETS/screenshot-viewer.js" "$ASSETS/robot-arm-viewer.html" \
  "$ASSETS/robot-arm-viewer.png" shoulder=0.5236 elbow=1.3963 wrist=-0.7854
echo "wrote robot-arm-viewer.html and robot-arm-viewer.png"
