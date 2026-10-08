#!/usr/bin/env bash
# Regenerate README preview assets from committed example documents.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ASSETS="$ROOT/docs/assets"
TMP_WIDE="$(mktemp -d)/bracket_wide.ocad.d"

mkdir -p "$ASSETS"
cp -r "$ROOT/examples/bracket.ocad.d" "$TMP_WIDE"

cargo run -p opencad-cli -- screenshot "$ROOT/examples/bracket.ocad.d" "$ASSETS/frame_base.png"
cargo run -p opencad-cli -- patch "$TMP_WIDE" "$ROOT/examples/agent/width_patch.json"
cargo run -p opencad-cli -- screenshot "$TMP_WIDE" "$ASSETS/frame_wide.png"
ffmpeg -y -i "$ASSETS/frame_base.png" -vf "scale=1280:-1:flags=lanczos" "$ASSETS/preview.png"
WGPU_BACKEND="${WGPU_BACKEND:-vulkan}" cargo run -p opencad-cli -- screenshot "$ROOT/examples/bracket_pin_row.ocad.d" "$ASSETS/frame_pin_row.png"
ffmpeg -y -i "$ASSETS/frame_pin_row.png" -vf "scale=1280:-1:flags=lanczos" "$ASSETS/preview_pin_row.png"
WGPU_BACKEND="${WGPU_BACKEND:-vulkan}" cargo run -p opencad-cli -- screenshot "$ROOT/examples/bracket_pin_ring.ocad.d" "$ASSETS/frame_pin_ring.png"
ffmpeg -y -i "$ASSETS/frame_pin_ring.png" -vf "scale=1280:-1:flags=lanczos" "$ASSETS/preview_pin_ring.png"
WGPU_BACKEND="${WGPU_BACKEND:-vulkan}" cargo run -p opencad-cli -- screenshot "$ROOT/examples/bracket_pin_mirror.ocad.d" "$ASSETS/frame_pin_mirror.png"
ffmpeg -y -i "$ASSETS/frame_pin_mirror.png" -vf "scale=1280:-1:flags=lanczos" "$ASSETS/preview_pin_mirror.png"
WGPU_BACKEND="${WGPU_BACKEND:-vulkan}" cargo run -p opencad-cli -- animate-features \
  "$ROOT/examples/robot_joint_actuator.ocad.d" "$ASSETS/robot-joint-feature-build.gif" \
  --frames 54 --fps 9 --orbit-deg 35 --pitch-deg 30
WGPU_BACKEND="${WGPU_BACKEND:-vulkan}" cargo run -p opencad-cli -- animate \
  "$ROOT/examples/robot_joint_actuator.ocad.d" "$ASSETS/robot-joint-orbit.gif" \
  --frames 60 --fps 12 --orbit-deg 360 --pitch-deg 28
cargo run -p opencad-cli -- animate-joints \
  "$ROOT/examples/robot_arm_assembly.ocad.d" "$ASSETS/robot-arm-joints.gif" \
  --width 640 --height 400 --frames-per-move 16 --fps 16 \
  --pose shoulder=70deg,elbow=-80deg,wrist=45deg \
  --pose shoulder=-60deg,elbow=110deg,wrist=-60deg \
  --pose shoulder=20deg,elbow=40deg,wrist=80deg
ffmpeg -y \
  -loop 1 -t 1.5 -framerate 4 -i "$ASSETS/frame_base.png" \
  -loop 1 -t 1.5 -framerate 4 -i "$ASSETS/frame_wide.png" \
  -filter_complex "[0:v]scale=960:540:force_original_aspect_ratio=decrease,pad=960:540:(ow-iw)/2:(oh-ih)/2:color=0x1f2430,format=rgb24[v0];[1:v]scale=960:540:force_original_aspect_ratio=decrease,pad=960:540:(ow-iw)/2:(oh-ih)/2:color=0x1f2430,format=rgb24[v1];[v0][v1]xfade=transition=fade:duration=0.5:offset=1.0,format=rgb24" \
  -r 4 "$ASSETS/preview.gif"
rm -f "$ASSETS/frame_base.png" "$ASSETS/frame_wide.png" "$ASSETS/frame_pin_row.png" "$ASSETS/frame_pin_ring.png" "$ASSETS/frame_pin_mirror.png"
rm -rf "$(dirname "$TMP_WIDE")"
echo "wrote README preview images, robot-joint feature/orbit GIFs, and the arm joint GIF"
