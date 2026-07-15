#!/usr/bin/env bash
# Open MusubiCAD's real, generated design review without building the project.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
REPORT="$ROOT/docs/assets/review-demo/review.html"
ASSETS=(
  "$REPORT"
  "$ROOT/docs/assets/review-demo/review.json"
  "$ROOT/docs/assets/review-demo/github-summary.md"
  "$ROOT/docs/assets/review-demo/before.png"
  "$ROOT/docs/assets/review-demo/after.png"
  "$ROOT/docs/assets/review-demo/comparison.gif"
)

for asset in "${ASSETS[@]}"; do
  if [[ ! -s "$asset" ]]; then
    echo "Missing quick-start asset: $asset" >&2
    exit 1
  fi
done
grep -q '<title>MusubiCAD Review</title>' "$REPORT"

if [[ "${1:-}" == "--check" ]]; then
  echo "Quick-start review is complete: $REPORT"
  exit 0
fi

echo "Opening a real 80 mm → 100 mm DesignPatch review. No build or model mutation required."
if command -v open >/dev/null 2>&1; then
  open "$REPORT"
elif command -v xdg-open >/dev/null 2>&1; then
  xdg-open "$REPORT"
elif command -v cmd.exe >/dev/null 2>&1 && command -v cygpath >/dev/null 2>&1; then
  cmd.exe /c start "" "$(cygpath -w "$REPORT")"
else
  echo "No browser opener found. Open this file manually: $REPORT"
fi
