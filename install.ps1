# Install the MusubiCAD CLI (`musubicad`) from GitHub Releases on Windows.
#
#   irm https://raw.githubusercontent.com/rsasaki0109/MusubiCAD/main/install.ps1 | iex
#
# Downloads the release archive, verifies it against the release's SHA256SUMS,
# installs one self-contained executable (OpenCASCADE is linked in), and adds
# its directory to the user PATH.  No build is needed.
#
# Environment:
#   MUSUBICAD_VERSION      release to install, e.g. 0.2.0 (default: latest)
#   MUSUBICAD_INSTALL_DIR  destination (default: %LOCALAPPDATA%\Programs\musubicad)
#   MUSUBICAD_REPO         GitHub owner/repo (default: rsasaki0109/MusubiCAD)
#   MUSUBICAD_BASE_URL     releases URL (default: https://github.com/$MUSUBICAD_REPO/releases)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

function Get-Setting([string]$Name, [string]$Default) {
    $value = [Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrEmpty($value)) { return $Default }
    return $value
}

$Repo = Get-Setting "MUSUBICAD_REPO" "rsasaki0109/MusubiCAD"
$Version = Get-Setting "MUSUBICAD_VERSION" "latest"
$InstallDir = Get-Setting "MUSUBICAD_INSTALL_DIR" ""
if ([string]::IsNullOrEmpty($InstallDir)) { $InstallDir = Join-Path $env:LOCALAPPDATA "Programs\musubicad" }
$BaseUrl = Get-Setting "MUSUBICAD_BASE_URL" "https://github.com/$Repo/releases"

if (-not [Environment]::Is64BitOperatingSystem -or
    $env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
    throw "musubicad-install: no prebuilt binary for this Windows architecture; build from source: https://github.com/$Repo/blob/main/docs/developer-guide/index.md"
}
$Platform = "windows-x86_64"

if ($Version -eq "latest") {
    $release = Invoke-RestMethod -UseBasicParsing "https://api.github.com/repos/$Repo/releases/latest"
    $Tag = $release.tag_name
    if ([string]::IsNullOrEmpty($Tag)) { throw "musubicad-install: could not determine the latest release of $Repo" }
} else {
    $Tag = "v" + $Version.TrimStart("v")
}

$Package = "musubicad-cli-$Tag-$Platform"
$Archive = "$Package.zip"
$Temp = Join-Path ([IO.Path]::GetTempPath()) ("musubicad-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $Temp | Out-Null
try {
    Write-Host "musubicad-install: downloading $Archive"
    Invoke-WebRequest -UseBasicParsing "$BaseUrl/download/$Tag/$Archive" -OutFile (Join-Path $Temp $Archive)
    Invoke-WebRequest -UseBasicParsing "$BaseUrl/download/$Tag/SHA256SUMS" -OutFile (Join-Path $Temp "SHA256SUMS")

    $expected = $null
    foreach ($line in Get-Content (Join-Path $Temp "SHA256SUMS")) {
        $fields = $line -split "\s+", 2
        if ($fields.Count -eq 2 -and $fields[1].TrimStart("*") -eq $Archive) { $expected = $fields[0].ToLowerInvariant() }
    }
    if (-not $expected) { throw "musubicad-install: SHA256SUMS of $Tag has no entry for $Archive" }
    $actual = (Get-FileHash -Algorithm SHA256 (Join-Path $Temp $Archive)).Hash.ToLowerInvariant()
    if ($expected -ne $actual) { throw "musubicad-install: checksum mismatch for $Archive (expected $expected, got $actual)" }

    Expand-Archive -LiteralPath (Join-Path $Temp $Archive) -DestinationPath $Temp
    $Binary = Join-Path $Temp "$Package\musubicad.exe"
    if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
        throw "musubicad-install: $Tag predates the musubicad command; install v0.2.0 or newer"
    }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -LiteralPath $Binary -Destination (Join-Path $InstallDir "musubicad.exe") -Force
} finally {
    Remove-Item -Recurse -Force -LiteralPath $Temp -ErrorAction SilentlyContinue
}

$Installed = (& (Join-Path $InstallDir "musubicad.exe") version | Select-Object -First 1)
Write-Host "musubicad-install: installed $Installed to $InstallDir\musubicad.exe"

$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not (($UserPath -split ";") -contains $InstallDir)) {
    $NewPath = if ([string]::IsNullOrEmpty($UserPath)) { $InstallDir } else { "$UserPath;$InstallDir" }
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    $env:Path = "$env:Path;$InstallDir"
    Write-Host "musubicad-install: added $InstallDir to your user PATH (open a new terminal to pick it up)"
}

Write-Host "musubicad-install: next, connect your agent:"
Write-Host "  Claude Code:  claude plugin marketplace add $Repo; claude plugin install musubicad@musubicad"
Write-Host "  Any MCP host: command `"musubicad`", args [`"mcp`"]"
