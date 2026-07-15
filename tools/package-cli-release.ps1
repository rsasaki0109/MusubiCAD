param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Platform,
    [Parameter(Mandatory = $true)][string]$BinaryPath,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)

$ErrorActionPreference = "Stop"

if ($Version -notmatch '^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$') {
    throw "Invalid release version: $Version"
}
if ($Platform -notmatch '^(linux|windows|macos)-(x86_64|aarch64)$') {
    throw "Invalid release platform: $Platform"
}

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Binary = (Resolve-Path -LiteralPath $BinaryPath).Path
$Output = [System.IO.Path]::GetFullPath($OutputDirectory)
$PackageName = "musubicad-cli-v$Version-$Platform"
$StagingRoot = Join-Path $Output ("staging-" + [guid]::NewGuid().ToString("N"))
$Package = Join-Path $StagingRoot $PackageName
$ExampleAgent = Join-Path $Package "examples\agent"

New-Item -ItemType Directory -Force -Path $ExampleAgent | Out-Null
$ExecutableName = if ($Platform.StartsWith("windows-")) { "opencad.exe" } else { "opencad" }
Copy-Item -LiteralPath $Binary -Destination (Join-Path $Package $ExecutableName)
Copy-Item -LiteralPath (Join-Path $Root "LICENSE") -Destination $Package
Copy-Item -LiteralPath (Join-Path $Root "README.md") -Destination $Package
Copy-Item -LiteralPath (Join-Path $Root "docs\release-quickstart.md") `
    -Destination (Join-Path $Package "QUICKSTART.md")
Copy-Item -Recurse -LiteralPath (Join-Path $Root "examples\bracket.ocad.d") `
    -Destination (Join-Path $Package "examples\bracket.ocad.d")
Copy-Item -LiteralPath (Join-Path $Root "examples\agent\review_width_patch.json") `
    -Destination $ExampleAgent

New-Item -ItemType Directory -Force -Path $Output | Out-Null
if ($Platform.StartsWith("windows-")) {
    $Archive = Join-Path $Output "$PackageName.zip"
    Compress-Archive -Path $Package -DestinationPath $Archive -Force
} else {
    & chmod +x (Join-Path $Package $ExecutableName)
    if ($LASTEXITCODE -ne 0) {
        throw "chmod failed with exit code $LASTEXITCODE"
    }
    $Archive = Join-Path $Output "$PackageName.tar.gz"
    & tar -czf $Archive -C $StagingRoot $PackageName
    if ($LASTEXITCODE -ne 0) {
        throw "tar failed with exit code $LASTEXITCODE"
    }
}

if (-not (Test-Path -LiteralPath $Archive -PathType Leaf) -or
    (Get-Item -LiteralPath $Archive).Length -eq 0) {
    throw "Release archive was not created: $Archive"
}
Write-Host "Created $Archive"
