#!/usr/bin/env bash
# Refresh all six-axis evidence from the committed Design Graph.
# Screenshot requires Node, Playwright and an installed Chromium browser.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
MODEL="$ROOT/examples/six_axis_arm.ocad.d"
ASSETS="$ROOT/docs/assets"
cargo run -q -p opencad-cli -- regen "$MODEL"
cargo run -q -p opencad-cli -- export "$MODEL" "$ASSETS/six-axis-arm.urdf"
cargo run -q -p opencad-cli -- export "$MODEL" "$ASSETS/six-axis-viewer.html"
cargo run -q -p opencad-cli -- animate-joints "$MODEL" "$ASSETS/six-axis-joints.gif" \
  --width 640 --aspect 4:3 --frames-per-move 16 --fps 16 \
  --pose base_yaw=70deg,shoulder=20deg,elbow=-25deg,wrist_pitch=-30deg \
  --pose base_yaw=-60deg,shoulder=-15deg,elbow=25deg,wrist_roll=90deg,wrist_pitch=40deg,flange_roll=90deg \
  --pose base_yaw=10deg,shoulder=30deg,elbow=-60deg,wrist_roll=-60deg,wrist_pitch=-60deg,flange_roll=-120deg
node "$ASSETS/screenshot-viewer.js" "$ASSETS/six-axis-viewer.html" \
  "$ASSETS/six-axis-viewer.png" base_yaw=0.6109 shoulder=0.3491 elbow=-0.5236 \
  wrist_roll=1.0472 wrist_pitch=-0.6981
