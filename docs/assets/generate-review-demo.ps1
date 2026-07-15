# Regenerate the README's flagship CAD review from committed inputs.
$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$Output = Join-Path $Root "docs\assets\review-demo"
$GeneratedFiles = @(
    "review.json",
    "review.html",
    "github-summary.md",
    "before.png",
    "after.png",
    "comparison.gif",
    "before-drawing.svg",
    "after-drawing.svg"
)

New-Item -ItemType Directory -Force -Path $Output | Out-Null
foreach ($Name in $GeneratedFiles) {
    Remove-Item -LiteralPath (Join-Path $Output $Name) -Force -ErrorAction SilentlyContinue
}

Push-Location $Root
try {
    cargo run --locked -p opencad-cli -- review `
        examples/bracket.ocad.d `
        examples/agent/review_width_patch.json `
        --output docs/assets/review-demo
    if ($LASTEXITCODE -ne 0) {
        throw "README review generation failed with exit code $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

Write-Host "Regenerated docs/assets/review-demo from the flagship DesignPatch."
