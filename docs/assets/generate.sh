#!/usr/bin/env bash
# Regenerate README preview assets from committed example documents.
#
# Produces:
#   preview.gif        — 360° turntable orbit of the bracket (seamless loop)
#   preview_param.gif  — parametric width patch morph (80 mm → 100 mm)
#   preview.png        — hero still with the parametric sketch overlay
#   preview_pin_row.png / preview_pin_ring.png / preview_pin_mirror.png
#
# Frames are rendered at high resolution and downscaled with lanczos so the
# edges are supersampled. GIFs use a dedicated palette (palettegen/paletteuse)
# so the gradient backdrop stays clean instead of dithering into noise.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ASSETS="$ROOT/docs/assets"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$ASSETS"

CLI=(cargo run --quiet -p opencad-cli --)
export WGPU_BACKEND="${WGPU_BACKEND:-vulkan}"

# Render width (high-res 16:9; downscaled below).
RW=1600
RH=900
# Output width in the README.
OUT_W=1280
GIF_W=900
FPS=20
FRAMES=40

# --- Hero still: clean studio solid (matches the turntable GIF) -------------
"${CLI[@]}" turntable "$ROOT/examples/bracket.ocad.d" "$WORK/hero" \
  --frames 1 --width "$RW" --height "$RH"
ffmpeg -y -loglevel error -i "$WORK/hero/frame_0000.png" \
  -vf "scale=${OUT_W}:-1:flags=lanczos" "$ASSETS/preview.png"

# --- Pattern stills --------------------------------------------------------
# Pin bosses protrude toward the viewer, so a grazing 3/4 angle (per pattern)
# lets the bosses break the silhouette and read against the plate.
render_still() {
  local doc="$1" out="$2" pitch="$3" yaw="$4"
  "${CLI[@]}" turntable "$doc" "$WORK/still" \
    --frames 1 --width "$RW" --height "$RH" --pitch "$pitch" --yaw "$yaw"
  ffmpeg -y -loglevel error -i "$WORK/still/frame_0000.png" \
    -vf "scale=${OUT_W}:-1:flags=lanczos" "$out"
  rm -rf "$WORK/still"
}
render_still "$ROOT/examples/bracket_pin_row.ocad.d"    "$ASSETS/preview_pin_row.png"    24 26
render_still "$ROOT/examples/bracket_pin_ring.ocad.d"   "$ASSETS/preview_pin_ring.png"   26 32
render_still "$ROOT/examples/bracket_pin_mirror.ocad.d" "$ASSETS/preview_pin_mirror.png" 24 30

# --- Hero GIF: 360° turntable orbit ----------------------------------------
"${CLI[@]}" turntable "$ROOT/examples/bracket.ocad.d" "$WORK/orbit" \
  --frames "$FRAMES" --width "$RW" --height "$RH"
ffmpeg -y -loglevel error -framerate "$FPS" -i "$WORK/orbit/frame_%04d.png" \
  -vf "scale=${GIF_W}:-1:flags=lanczos,palettegen=max_colors=192:stats_mode=full" "$WORK/palette.png"
ffmpeg -y -loglevel error -framerate "$FPS" -i "$WORK/orbit/frame_%04d.png" -i "$WORK/palette.png" \
  -lavfi "scale=${GIF_W}:-1:flags=lanczos[s];[s][1:v]paletteuse=dither=bayer:bayer_scale=4" \
  -loop 0 "$ASSETS/preview.gif"

# --- Parametric morph GIF: 80 mm -> 100 mm width patch ---------------------
WIDE="$WORK/bracket_wide.ocad.d"
cp -r "$ROOT/examples/bracket.ocad.d" "$WIDE"
"${CLI[@]}" patch "$WIDE" "$ROOT/examples/agent/width_patch.json"
"${CLI[@]}" turntable "$ROOT/examples/bracket.ocad.d" "$WORK/base1" \
  --frames 1 --width "$RW" --height "$RH"
"${CLI[@]}" turntable "$WIDE" "$WORK/wide1" \
  --frames 1 --width "$RW" --height "$RH"
ffmpeg -y -loglevel error \
  -loop 1 -t 1.6 -framerate "$FPS" -i "$WORK/base1/frame_0000.png" \
  -loop 1 -t 1.6 -framerate "$FPS" -i "$WORK/wide1/frame_0000.png" \
  -filter_complex "[0:v]scale=${GIF_W}:-1:flags=lanczos,format=rgb24[v0];\
[1:v]scale=${GIF_W}:-1:flags=lanczos,format=rgb24[v1];\
[v0][v1]xfade=transition=fade:duration=0.6:offset=1.0,split[a][b];\
[a]palettegen=stats_mode=full[p];[b][p]paletteuse=dither=bayer:bayer_scale=4" \
  -loop 0 "$ASSETS/preview_param.gif"

echo "wrote:"
echo "  $ASSETS/preview.gif (360 turntable)"
echo "  $ASSETS/preview_param.gif (80mm -> 100mm patch)"
echo "  $ASSETS/preview.png"
echo "  $ASSETS/preview_pin_row.png, preview_pin_ring.png, preview_pin_mirror.png"
