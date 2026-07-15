# Open MusubiCAD's real, generated design review without building the project.
param([switch]$Check)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot
$Review = Join-Path $Root "docs\assets\review-demo"
$Report = Join-Path $Review "review.html"
$Assets = @(
    $Report,
    (Join-Path $Review "review.json"),
    (Join-Path $Review "github-summary.md"),
    (Join-Path $Review "before.png"),
    (Join-Path $Review "after.png"),
    (Join-Path $Review "comparison.gif")
)

foreach ($Asset in $Assets) {
    if (-not (Test-Path -LiteralPath $Asset -PathType Leaf) -or
        (Get-Item -LiteralPath $Asset).Length -eq 0) {
        throw "Missing quick-start asset: $Asset"
    }
}
if (-not (Select-String -LiteralPath $Report -SimpleMatch "<title>MusubiCAD Review</title>" -Quiet)) {
    throw "Quick-start report is not a MusubiCAD design review: $Report"
}

if ($Check) {
    Write-Host "Quick-start review is complete: $Report"
    exit 0
}

Write-Host "Opening a real 80 mm -> 100 mm DesignPatch review. No build or model mutation required."
Start-Process -FilePath $Report
