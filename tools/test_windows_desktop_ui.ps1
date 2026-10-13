[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$BinaryPath
)
$ErrorActionPreference = 'Stop'
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$binary = (Resolve-Path -LiteralPath $BinaryPath).Path
$fixture = Join-Path $repositoryRoot 'examples/cubic_plate.ocad.d'
function Get-SourceHashes([string]$Root) {
    return @((Get-ChildItem -LiteralPath $Root -Recurse -File | Sort-Object FullName | ForEach-Object {
        $relative = [System.IO.Path]::GetRelativePath($Root, $_.FullName)
        "$relative $((Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash)"
    })) -join "`n"
}
$original = Get-SourceHashes $fixture
$work = Join-Path ([System.IO.Path]::GetTempPath()) ('musubicad-windows-ui-' + [guid]::NewGuid())
$part = Join-Path $work 'examples/bracket.ocad.d'
$previousArguments = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$previousProfile = $env:WEBVIEW2_USER_DATA_FOLDER
$process = $null
try {
    New-Item -ItemType Directory -Path (Join-Path $work 'examples') | Out-Null
    Copy-Item -LiteralPath $fixture -Destination $part -Recurse
    # The debugger belongs only to this disposable test process, on loopback.
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-address=127.0.0.1 --remote-debugging-port=9223'
    $env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $work 'webview-profile'
    $process = Start-Process -FilePath $binary -WorkingDirectory $work -PassThru
    $endpoint = 'http://127.0.0.1:9223'
    $ready = $false
    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($process.HasExited) { throw 'Native Tauri app exited before its WebView2 loaded' }
        try {
            $pages = @(Invoke-RestMethod -Uri "$endpoint/json/list" -TimeoutSec 2)
            if (@($pages | Where-Object { $_.type -eq 'page' -and $_.url -match '^https?://tauri\.localhost(/|$)' }).Count -eq 1) {
                $ready = $true
                break
            }
        } catch { }
        Start-Sleep -Milliseconds 250
    }
    if (-not $ready) { throw 'No real Tauri WebView2 inspector page appeared' }
    $runtime = Invoke-RestMethod -Uri "$endpoint/json/version" -TimeoutSec 5
    Write-Output "WebView2 runtime: $($runtime.Browser); protocol=$($runtime.'Protocol-Version')"
    & node (Join-Path $PSScriptRoot 'test_windows_tauri_editor.mjs') $endpoint $part
    if ($LASTEXITCODE -ne 0) { throw 'Actual Windows UI editing checks failed' }
    if ((Get-SourceHashes $fixture) -cne $original) { throw 'Original fixture changed' }
    Write-Output "Windows native Tauri forms and source preservation: passed; OS=$([System.Environment]::OSVersion.VersionString)"
}
finally {
    if ($null -ne $process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $previousArguments
    $env:WEBVIEW2_USER_DATA_FOLDER = $previousProfile
    # WebView2 child processes can take a moment to release their own profile.
    for ($attempt = 0; $attempt -lt 20 -and (Test-Path -LiteralPath $work); $attempt++) {
        try { Remove-Item -LiteralPath $work -Recurse -Force }
        catch {
            if ($attempt -eq 19) { throw }
            Start-Sleep -Milliseconds 250
        }
    }
}
