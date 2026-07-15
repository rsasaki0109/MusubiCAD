#!/usr/bin/env bash
# Verify exact reports and perceptually stable visuals for the README review demo.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUTPUT="$ROOT/docs/assets/review-demo"
BASELINE="$(mktemp -d)"
cleanup() {
  if [[ -n "$BASELINE" && "$BASELINE" == "${TMPDIR:-/tmp}/"* ]]; then
    rm -rf -- "$BASELINE"
  fi
}
trap cleanup EXIT

cp -R "$OUTPUT/." "$BASELINE/"
"$ROOT/docs/assets/generate-review-demo.sh"

for report in review.json review.html github-summary.md; do
  cmp "$BASELINE/$report" "$OUTPUT/$report"
done

compare_visual() {
  local name="$1"
  local result normalized
  result="$(compare -metric MAE "$BASELINE/$name" "$OUTPUT/$name" null: 2>&1 || true)"
  normalized="$(printf '%s\n' "$result" | sed -nE 's/.*\(([0-9.eE+-]+)\).*/\1/p' | tail -n 1)"
  if [[ -z "$normalized" ]]; then
    echo "Could not parse ImageMagick MAE for $name: $result" >&2
    return 1
  fi
  echo "$name normalized MAE: $normalized"
  awk -v value="$normalized" 'BEGIN { exit !(value <= 0.01) }'
}

compare_visual before.png
compare_visual after.png
compare_visual comparison.gif

echo "README review reports match exactly and visuals are within 1% normalized MAE."
