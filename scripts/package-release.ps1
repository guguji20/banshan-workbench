[CmdletBinding()]
param(
  [string]$InstallerPath = "",
  [string]$Version = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$ProductName = -join ([char[]](0x534A, 0x5C71, 0x5546, 0x52A1, 0x5DE5, 0x4F5C, 0x53F0))
$ReleaseDateLabel = -join ([char[]](0x53D1, 0x5E03, 0x65E5, 0x671F))
$FullWidthColon = [char]0xFF1A
$UnsignedLabel = -join ([char[]](0x672A, 0x7B7E, 0x540D))

$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
if ([string]::IsNullOrWhiteSpace($Version)) {
  $Version = [string](Get-Content -LiteralPath (Join-Path $RepoRoot "package.json") -Raw -Encoding UTF8 | ConvertFrom-Json).version
}
$ExpectedInstallerName = "${ProductName}_${Version}_x64-setup.exe"
if ([string]::IsNullOrWhiteSpace($InstallerPath)) {
  $InstallerPath = Join-Path $RepoRoot "src-tauri\target\release\bundle\nsis\$ExpectedInstallerName"
}
$ResolvedInstaller = [System.IO.Path]::GetFullPath($InstallerPath)
$ReleaseRoot = [System.IO.Path]::GetFullPath((Join-Path $RepoRoot "release\$Version"))
$ReleaseNotesPath = Join-Path $RepoRoot "docs\RELEASE_$Version.md"
$ManifestPath = Join-Path $RepoRoot "src-tauri\resources\codex-runtime\manifest.json"

if (-not (Test-Path -LiteralPath $ResolvedInstaller -PathType Leaf)) {
  throw "Installer not found: $ResolvedInstaller"
}
if ((Split-Path -Leaf $ResolvedInstaller) -ne $ExpectedInstallerName) {
  throw "Unexpected installer file name. Expected $ExpectedInstallerName."
}
if (-not (Test-Path -LiteralPath $ReleaseNotesPath -PathType Leaf)) {
  throw "Release notes not found: $ReleaseNotesPath"
}
if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
  throw "Codex sidecar manifest not found: $ManifestPath"
}

$installer = Get-Item -LiteralPath $ResolvedInstaller
$sourceFiles = @(
  Get-ChildItem -LiteralPath (Join-Path $RepoRoot "src") -Recurse -File
  Get-ChildItem -LiteralPath (Join-Path $RepoRoot "src-tauri\src") -Recurse -File
  Get-ChildItem -LiteralPath (Join-Path $RepoRoot "src-tauri\resources") -Recurse -File
  Get-ChildItem -LiteralPath (Join-Path $RepoRoot "public") -Recurse -File
  Get-Item -LiteralPath (Join-Path $RepoRoot "package.json")
  Get-Item -LiteralPath (Join-Path $RepoRoot "pnpm-lock.yaml")
  Get-Item -LiteralPath (Join-Path $RepoRoot "src-tauri\Cargo.toml")
  Get-Item -LiteralPath (Join-Path $RepoRoot "src-tauri\Cargo.lock")
  Get-Item -LiteralPath (Join-Path $RepoRoot "src-tauri\tauri.conf.json")
)
$newestSource = $sourceFiles | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
if ($newestSource.LastWriteTimeUtc -gt $installer.LastWriteTimeUtc) {
  throw "Installer is older than product source: $($newestSource.FullName). Rebuild before packaging."
}
$installerSha256 = (Get-FileHash -LiteralPath $ResolvedInstaller -Algorithm SHA256).Hash.ToLowerInvariant()
$signature = Get-AuthenticodeSignature -LiteralPath $ResolvedInstaller
if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::NotSigned) {
  throw "Expected unsigned NSIS installer, got Authenticode status $($signature.Status)."
}

$releaseNotes = [System.IO.File]::ReadAllText($ReleaseNotesPath, [System.Text.Encoding]::UTF8)
$releaseDatePattern = [regex]::Escape("$ReleaseDateLabel$FullWidthColon") + '`(?<date>\d{4}-\d{2}-\d{2})`'
$releaseDateMatch = [regex]::Match($releaseNotes, $releaseDatePattern)
if (-not $releaseDateMatch.Success) {
  throw "Release notes must contain a YYYY-MM-DD release date."
}
$parsedReleaseDate = [datetime]::MinValue
if (-not [datetime]::TryParseExact(
    $releaseDateMatch.Groups["date"].Value,
    "yyyy-MM-dd",
    [System.Globalization.CultureInfo]::InvariantCulture,
    [System.Globalization.DateTimeStyles]::None,
    [ref]$parsedReleaseDate
  )) {
  throw "Release notes contain an invalid release date."
}
if ($parsedReleaseDate.Date -gt (Get-Date).Date) {
  throw "Release notes cannot use a future release date."
}
$FinalInternalStatusMarker = "releaseStatus: final-internal-accepted"
if (-not $releaseNotes.Contains($FinalInternalStatusMarker)) {
  throw "Release notes must declare the final internal-release acceptance status."
}
if ($releaseNotes.Contains("尚未完成最终安装包验收")) {
  throw "Release notes still declare that final installer acceptance is incomplete."
}
if (-not $releaseNotes.Contains($installerSha256)) {
  throw "Release notes do not contain the final installer SHA-256: $installerSha256"
}
if ($releaseNotes.Contains("__PENDING_")) {
  throw "Release notes still contain pending placeholders."
}
if (-not $releaseNotes.Contains($UnsignedLabel)) {
  throw "Release notes must explicitly disclose that the NSIS installer is unsigned."
}

$manifest = Get-Content -LiteralPath $ManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
$manifestPaths = @($manifest.files | ForEach-Object { $_.path })
foreach ($requiredPath in @("LICENSE", "NOTICE")) {
  if ($manifestPaths -notcontains $requiredPath) {
    throw "Codex sidecar manifest does not include $requiredPath."
  }
}

New-Item -ItemType Directory -Path $ReleaseRoot -Force | Out-Null
$releaseInstaller = Join-Path $ReleaseRoot $ExpectedInstallerName
$releaseNotesCopy = Join-Path $ReleaseRoot "RELEASE_$Version.md"
$checksumsPath = Join-Path $ReleaseRoot "SHA256SUMS.txt"

Copy-Item -LiteralPath $ResolvedInstaller -Destination $releaseInstaller -Force
Copy-Item -LiteralPath $ReleaseNotesPath -Destination $releaseNotesCopy -Force
[System.IO.File]::WriteAllText(
  $checksumsPath,
  "$installerSha256 *$ExpectedInstallerName`n",
  $Utf8NoBom
)

$copiedSha256 = (Get-FileHash -LiteralPath $releaseInstaller -Algorithm SHA256).Hash.ToLowerInvariant()
if ($copiedSha256 -ne $installerSha256) {
  throw "Release copy checksum mismatch."
}

Write-Host "Release package prepared: $ReleaseRoot"
Write-Host "Installer: $ExpectedInstallerName"
Write-Host "Size: $($installer.Length) bytes"
Write-Host "SHA-256: $installerSha256"
Write-Host "Authenticode: $($signature.Status)"
