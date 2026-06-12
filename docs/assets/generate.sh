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
ffmpeg -y \
  -loop 1 -t 1.5 -framerate 4 -i "$ASSETS/frame_base.png" \
  -loop 1 -t 1.5 -framerate 4 -i "$ASSETS/frame_wide.png" \
  -filter_complex "[0:v]scale=960:540:force_original_aspect_ratio=decrease,pad=960:540:(ow-iw)/2:(oh-ih)/2:color=0x1f2430,format=rgb24[v0];[1:v]scale=960:540:force_original_aspect_ratio=decrease,pad=960:540:(ow-iw)/2:(oh-ih)/2:color=0x1f2430,format=rgb24[v1];[v0][v1]xfade=transition=fade:duration=0.5:offset=1.0,format=rgb24" \
  -r 4 "$ASSETS/preview.gif"
rm -f "$ASSETS/frame_base.png" "$ASSETS/frame_wide.png"
rm -rf "$(dirname "$TMP_WIDE")"
echo "wrote $ASSETS/preview.png and $ASSETS/preview.gif"
