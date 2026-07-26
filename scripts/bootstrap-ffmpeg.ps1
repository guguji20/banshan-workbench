[CmdletBinding()]
param(
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$runtimeRoot = Join-Path $repoRoot "src-tauri\resources\media-runtime"
$cacheRoot = Join-Path $repoRoot ".runtime\downloads"
$archivePath = Join-Path $cacheRoot "ffmpeg-master-latest-win64-lgpl-shared.zip"
$downloadUrl = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-lgpl-shared.zip"
$expectedSha256 = "b3d3fb6928e5c146aa1194195742c9e9b708be2ad4f9db53a6bca79413c17bb6"
$markerPath = Join-Path $runtimeRoot "BSAIGC_FFMPEG_SHA256.txt"

function Assert-WorkspacePath([string]$Path) {
    $resolvedRoot = [IO.Path]::GetFullPath($repoRoot).TrimEnd('\') + '\'
    $resolvedPath = [IO.Path]::GetFullPath($Path)
    if (-not $resolvedPath.StartsWith($resolvedRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to modify a path outside the workspace: $resolvedPath"
    }
}

Assert-WorkspacePath $runtimeRoot
Assert-WorkspacePath $cacheRoot

if (-not $Force -and
    (Test-Path -LiteralPath (Join-Path $runtimeRoot "ffmpeg.exe") -PathType Leaf) -and
    (Test-Path -LiteralPath (Join-Path $runtimeRoot "ffprobe.exe") -PathType Leaf) -and
    (Test-Path -LiteralPath $markerPath -PathType Leaf) -and
    (Get-Content -Raw -LiteralPath $markerPath).Trim() -eq $expectedSha256) {
    Write-Host "Pinned FFmpeg runtime is already installed."
    exit 0
}

New-Item -ItemType Directory -Force -Path $cacheRoot | Out-Null
if ($Force -or -not (Test-Path -LiteralPath $archivePath -PathType Leaf)) {
    Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath -UseBasicParsing
}

$actualSha256 = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualSha256 -ne $expectedSha256) {
    throw "FFmpeg archive checksum mismatch. Expected $expectedSha256, received $actualSha256."
}

$extractRoot = Join-Path $cacheRoot "ffmpeg-extracted"
if (Test-Path -LiteralPath $extractRoot) {
    Remove-Item -LiteralPath $extractRoot -Recurse -Force
}
Expand-Archive -LiteralPath $archivePath -DestinationPath $extractRoot -Force

$ffmpeg = Get-ChildItem -LiteralPath $extractRoot -Recurse -Filter "ffmpeg.exe" -File |
    Select-Object -First 1
$ffprobe = Get-ChildItem -LiteralPath $extractRoot -Recurse -Filter "ffprobe.exe" -File |
    Select-Object -First 1
if (-not $ffmpeg -or -not $ffprobe -or $ffmpeg.DirectoryName -ne $ffprobe.DirectoryName) {
    throw "Downloaded FFmpeg runtime does not contain the expected shared Windows layout."
}

if (Test-Path -LiteralPath $runtimeRoot) {
    Remove-Item -LiteralPath $runtimeRoot -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $runtimeRoot | Out-Null

Get-ChildItem -LiteralPath $ffmpeg.DirectoryName -File |
    Where-Object { $_.Extension -in @(".exe", ".dll") -and $_.Name -ne "ffplay.exe" } |
    Copy-Item -Destination $runtimeRoot
Set-Content -LiteralPath $markerPath -Value $expectedSha256 -Encoding ascii

$license = Get-ChildItem -LiteralPath $extractRoot -Recurse -File |
    Where-Object { $_.Name -match "^(LICENSE|COPYING)(\..*)?$" } |
    Select-Object -First 1
if ($license) {
    Copy-Item -LiteralPath $license.FullName -Destination (Join-Path $runtimeRoot $license.Name)
}

Write-Host "Pinned FFmpeg LGPL shared runtime installed at $runtimeRoot."
