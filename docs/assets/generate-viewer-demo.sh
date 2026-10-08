#!/usr/bin/env bash
# Regenerate the README web viewers: self-contained HTML pages of the planar and
# six-axis arms with one slider per joint, and a screenshot of the six-axis arm
# with its joints moved.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ASSETS="$ROOT/docs/assets"
cargo run -q -p opencad-cli -- export \
  "$ROOT/examples/robot_arm_assembly.ocad.d" "$ASSETS/robot-arm-viewer.html"
cargo run -q -p opencad-cli -- export \
  "$ROOT/examples/six_axis_arm.ocad.d" "$ASSETS/six-axis-viewer.html"
# Base 35°, shoulder 20°, elbow -30°, wrist roll 60°, wrist pitch -40°, in
# radians as the sliders take them.
node "$ASSETS/screenshot-viewer.js" "$ASSETS/six-axis-viewer.html" \
  "$ASSETS/six-axis-viewer.png" base_yaw=0.6109 shoulder=0.3491 elbow=-0.5236 \
  wrist_roll=1.0472 wrist_pitch=-0.6981
echo "wrote robot-arm-viewer.html, six-axis-viewer.html, and six-axis-viewer.png"
