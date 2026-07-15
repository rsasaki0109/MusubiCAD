#!/usr/bin/env bash
# Regenerate the README's flagship CAD review from committed inputs.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUTPUT="$ROOT/docs/assets/review-demo"

mkdir -p "$OUTPUT"
rm -f \
  "$OUTPUT/review.json" \
  "$OUTPUT/review.html" \
  "$OUTPUT/github-summary.md" \
  "$OUTPUT/before.png" \
  "$OUTPUT/after.png" \
  "$OUTPUT/comparison.gif" \
  "$OUTPUT/before-drawing.svg" \
  "$OUTPUT/after-drawing.svg"

cd "$ROOT"
cargo run --locked -p opencad-cli -- review \
  examples/bracket.ocad.d \
  examples/agent/review_width_patch.json \
  --output docs/assets/review-demo

echo "Regenerated docs/assets/review-demo from the flagship DesignPatch."
