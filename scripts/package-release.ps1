[CmdletBinding()]
param(
  [string]$InstallerPath = "",
  [string]$Version = "",
  [string]$BuildManifestPath = "",
  [switch]$ReleaseCandidate
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Get-Sha256([string]$Path) { return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }
function Assert-Hash([string]$Path, [string]$Expected, [string]$Label) {
  $actual = Get-Sha256 $Path
  if ($actual -cne $Expected.ToLowerInvariant()) { throw "$Label SHA-256 mismatch." }
  return $actual
}

function Get-ReleaseIdentity([string]$Root) {
  $package = Get-Content -LiteralPath (Join-Path $Root "package.json") -Raw -Encoding UTF8 | ConvertFrom-Json
  $tauriText = Get-Content -LiteralPath (Join-Path $Root "src-tauri\tauri.conf.json") -Raw -Encoding UTF8
  $tauriVersionMatch = [regex]::Match($tauriText, '"version"\s*:\s*"(?<version>[^"]+)"')
  $productMatch = [regex]::Match($tauriText, '"productName"\s*:\s*"(?<name>[^"]+)"')
  $cargoText = Get-Content -LiteralPath (Join-Path $Root "src-tauri\Cargo.toml") -Raw -Encoding UTF8
  $cargoMatch = [regex]::Match($cargoText, '(?m)^version\s*=\s*"(?<version>[^"]+)"\s*$')
  if (-not $tauriVersionMatch.Success -or -not $productMatch.Success -or -not $cargoMatch.Success) {
    throw "One or more release identity fields are missing."
  }
  $versions = @([string]$package.version, $tauriVersionMatch.Groups["version"].Value, $cargoMatch.Groups["version"].Value)
  if (@($versions | Select-Object -Unique).Count -ne 1) {
    throw "Release version mismatch: package.json=$($versions[0]), tauri.conf.json=$($versions[1]), Cargo.toml=$($versions[2])."
  }
  return [pscustomobject][ordered]@{ version = $versions[0]; productName = $productMatch.Groups["name"].Value }
}

function Assert-NoTrackedSecretFiles([string]$Root) {
  $tracked = @(& git -C $Root ls-files)
  if ($LASTEXITCODE -ne 0) { throw "Unable to inspect tracked files." }
  $blocked = @($tracked | Where-Object {
    $_ -match '(^|/)\.env(?:\.|$)' -or
    $_ -match '(^|/)(?:r2\.config\.json|internal-preview-build\.json|credentials\.json|secrets\.json)$' -or
    $_ -match '\.(?:pfx|p12|pem|key)$'
  })
  if ($blocked.Count -gt 0) { throw "Tracked secret-bearing path gate failed: $($blocked -join ', ')" }
}

function Assert-PublicOnlyR2Config([string]$Root) {
  $path = Join-Path $Root "src-tauri\resources\r2.config.json"
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Public-only r2.config.json is missing." }
  $config = Get-Content -LiteralPath $path -Raw -Encoding UTF8 | ConvertFrom-Json
  if (-not [string]::IsNullOrWhiteSpace([string]$config.accessKeyId) -or
      -not [string]::IsNullOrWhiteSpace([string]$config.secretAccessKey)) {
    throw "Packaging blocked: r2.config.json contains credentials. Values were not printed."
  }
}

function Assert-TextHasNoCredentialAssignments([string]$Path) {
  $text = [System.IO.File]::ReadAllText($Path, [System.Text.Encoding]::UTF8)
  if ($text -match '(?im)(?:api[_-]?key|secret[_-]?access[_-]?key|access[_-]?key[_-]?id)\s*[:=]\s*["''][^"'']{8,}["'']') {
    throw "Credential-like assignment detected in $([System.IO.Path]::GetFileName($Path)). Values were not printed."
  }
}

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$identity = Get-ReleaseIdentity $repoRoot
if ([string]::IsNullOrWhiteSpace($Version)) { $Version = $identity.version }
if ($Version -cne $identity.version) { throw "Requested release version $Version does not match repository version $($identity.version)." }
Assert-NoTrackedSecretFiles $repoRoot
Assert-PublicOnlyR2Config $repoRoot

$expectedInstallerName = "$($identity.productName)_$Version`_x64-setup.exe"
if ([string]::IsNullOrWhiteSpace($InstallerPath)) { $InstallerPath = Join-Path $repoRoot "src-tauri\target\release\bundle\nsis\$expectedInstallerName" }
$resolvedInstaller = [System.IO.Path]::GetFullPath($InstallerPath)
if (-not (Test-Path -LiteralPath $resolvedInstaller -PathType Leaf)) { throw "Installer not found: $resolvedInstaller" }
if ((Split-Path -Leaf $resolvedInstaller) -cne $expectedInstallerName) { throw "Unexpected installer file name. Expected $expectedInstallerName." }

if ([string]::IsNullOrWhiteSpace($BuildManifestPath)) { $BuildManifestPath = "$resolvedInstaller.build-manifest.json" }
$resolvedBuildManifest = [System.IO.Path]::GetFullPath($BuildManifestPath)
if (-not (Test-Path -LiteralPath $resolvedBuildManifest -PathType Leaf)) { throw "Build manifest not found: $resolvedBuildManifest" }
$buildManifest = Get-Content -LiteralPath $resolvedBuildManifest -Raw -Encoding UTF8 | ConvertFrom-Json
if ([int]$buildManifest.schemaVersion -ne 1 -or [string]$buildManifest.artifactKind -cne "windows-release-candidate") { throw "Unsupported Windows build manifest." }
if ([string]$buildManifest.version -cne $Version) { throw "Build manifest version mismatch." }
if ([string]$buildManifest.installer.fileName -cne $expectedInstallerName) { throw "Build manifest installer name mismatch." }
if ([bool]$buildManifest.installer.unsigned -ne $true -or [string]$buildManifest.installer.authenticode -cne "NotSigned") { throw "Build manifest does not prove an unsigned installer." }
Assert-Hash $resolvedInstaller ([string]$buildManifest.installer.sha256) "Installer"
$buildLogPath = [System.IO.Path]::GetFullPath((Join-Path $repoRoot ([string]$buildManifest.buildLog.relativePath)))
if (-not (Test-Path -LiteralPath $buildLogPath -PathType Leaf)) { throw "Build log not found: $buildLogPath" }
Assert-Hash $buildLogPath ([string]$buildManifest.buildLog.sha256) "Build log"
Assert-TextHasNoCredentialAssignments $buildLogPath
if ([bool]$buildManifest.security.embeddedInternalApiKey -or [bool]$buildManifest.security.bundledR2Credentials) { throw "Build manifest reports embedded credentials." }

$installer = Get-Item -LiteralPath $resolvedInstaller
$installerSha256 = Get-Sha256 $resolvedInstaller
$signature = Get-AuthenticodeSignature -LiteralPath $resolvedInstaller
if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::NotSigned) { throw "Expected unsigned NSIS installer, got Authenticode status $($signature.Status)." }
$unsignedNoticePath = "$resolvedInstaller.unsigned.txt"
$checksumPath = "$resolvedInstaller.sha256"
if (-not (Test-Path -LiteralPath $unsignedNoticePath -PathType Leaf)) { throw "Unsigned notice is missing." }
if (-not ([System.IO.File]::ReadAllText($unsignedNoticePath, [System.Text.Encoding]::UTF8)).Contains("Authenticode: NotSigned")) { throw "Unsigned notice is invalid." }
if (-not (Test-Path -LiteralPath $checksumPath -PathType Leaf)) { throw "Installer checksum file is missing." }
if (-not ([System.IO.File]::ReadAllText($checksumPath, [System.Text.Encoding]::UTF8)).Contains($installerSha256)) { throw "Installer checksum file does not contain the measured SHA-256." }

$releaseRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "release\$Version"))
if (Test-Path -LiteralPath $releaseRoot) { throw "Release output already exists; refusing to overwrite: $releaseRoot" }
$snapshotOutput = Join-Path $repoRoot ".runtime\release-packaging\$Version-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $snapshotOutput -Force | Out-Null
& (Join-Path $repoRoot "scripts\create-source-snapshot.ps1") -Version $Version -OutputDirectory $snapshotOutput
if ($LASTEXITCODE -ne 0) { throw "Source snapshot failed with exit code $LASTEXITCODE." }
$snapshotDirs = @(Get-ChildItem -LiteralPath $snapshotOutput -Directory | Where-Object { $_.Name -notlike '.source-snapshot-staging-*' })
if ($snapshotDirs.Count -ne 1) { throw "Expected exactly one source snapshot directory, found $($snapshotDirs.Count)." }
$snapshotDir = $snapshotDirs[0].FullName
$snapshotManifestPath = Join-Path $snapshotDir "source-manifest.json"
$snapshotZip = Get-ChildItem -LiteralPath $snapshotDir -Filter "*.zip" -File
if (-not (Test-Path -LiteralPath $snapshotManifestPath -PathType Leaf) -or $null -eq $snapshotZip) { throw "Source snapshot is incomplete." }
$snapshotManifest = Get-Content -LiteralPath $snapshotManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ([int]$snapshotManifest.security.blockedFindings -ne 0 -or [string]$snapshotManifest.security.result -cne "passed") { throw "Source snapshot security gate did not pass." }

$sourceArchiveName = "huabang-business-system-v$Version-source.zip"
$buildLogName = "build-windows-$Version.log"
$releaseInstallerName = "huabang-business-system-v$Version-windows-x64-setup-unsigned.exe"
$releaseRoot = New-Item -ItemType Directory -Path $releaseRoot -Force
$releaseInstallerPath = Join-Path $releaseRoot $releaseInstallerName
$sourceArchivePath = Join-Path $releaseRoot $sourceArchiveName
$releaseBuildLogPath = Join-Path $releaseRoot $buildLogName
$releaseBuildManifestPath = Join-Path $releaseRoot "build-manifest.json"
$releaseUnsignedNoticePath = Join-Path $releaseRoot "UNSIGNED.txt"
Copy-Item -LiteralPath $resolvedInstaller -Destination $releaseInstallerPath
Copy-Item -LiteralPath $snapshotZip.FullName -Destination $sourceArchivePath
Copy-Item -LiteralPath $buildLogPath -Destination $releaseBuildLogPath
Copy-Item -LiteralPath $resolvedBuildManifest -Destination $releaseBuildManifestPath
Copy-Item -LiteralPath $unsignedNoticePath -Destination $releaseUnsignedNoticePath
Copy-Item -LiteralPath $snapshotManifestPath -Destination (Join-Path $releaseRoot "source-manifest.json")

if (-not $ReleaseCandidate) {
  $releaseNotesPath = Join-Path $repoRoot "docs\RELEASE_$Version.md"
  if (-not (Test-Path -LiteralPath $releaseNotesPath -PathType Leaf)) { throw "Release notes not found: $releaseNotesPath" }
  $releaseNotes = [System.IO.File]::ReadAllText($releaseNotesPath, [System.Text.Encoding]::UTF8)
  if (-not $releaseNotes.Contains("releaseStatus: final-internal-accepted")) { throw "Release notes lack final internal acceptance." }
  $unsignedChineseLabel = [string]::Concat([char]0x672A, [char]0x7B7E, [char]0x540D)
  if (-not $releaseNotes.Contains($unsignedChineseLabel) -and -not $releaseNotes.Contains("unsigned")) { throw "Release notes must disclose unsigned status." }
  if ($releaseNotes.Contains("__PENDING_")) { throw "Release notes still contain pending placeholders." }
  Copy-Item -LiteralPath $releaseNotesPath -Destination (Join-Path $releaseRoot "RELEASE_$Version.md")
}

$fileRecords = @()
foreach ($file in Get-ChildItem -LiteralPath $releaseRoot -File | Sort-Object Name) {
  $fileRecords += [ordered]@{ name = $file.Name; sizeBytes = [int64]$file.Length; sha256 = Get-Sha256 $file.FullName }
}
$releaseManifest = [ordered]@{
  schemaVersion = 1
  artifactKind = if ($ReleaseCandidate) { "windows-release-candidate" } else { "windows-release" }
  product = $identity.productName
  version = $Version
  createdAtUtc = (Get-Date).ToUniversalTime().ToString("o")
  installer = [ordered]@{ name = $releaseInstallerName; sha256 = $installerSha256; sizeBytes = [int64]$installer.Length; authenticode = [string]$signature.Status; unsigned = $true }
  sourceSnapshot = [ordered]@{ archive = $sourceArchiveName; sha256 = Get-Sha256 $sourceArchivePath; manifest = "source-manifest.json"; finalSha256 = [string]$snapshotManifest.snapshot.finalSha256 }
  build = [ordered]@{ log = $buildLogName; logSha256 = Get-Sha256 $releaseBuildLogPath; manifest = "build-manifest.json" }
  security = [ordered]@{ sourceContainsSecrets = $false; embeddedInternalApiKey = $false; bundledR2Credentials = $false; authenticode = [string]$signature.Status; unsigned = $true }
  files = @($fileRecords)
}
$releaseManifestPath = Join-Path $releaseRoot "release-manifest.json"
[System.IO.File]::WriteAllText($releaseManifestPath, (($releaseManifest | ConvertTo-Json -Depth 8) + "`n"), $Utf8NoBom)

$checksumLines = @()
foreach ($file in Get-ChildItem -LiteralPath $releaseRoot -File | Sort-Object Name) {
  if ($file.Name -cne "SHA256SUMS.txt") { $checksumLines += "$(Get-Sha256 $file.FullName) *$($file.Name)" }
}
$checksumsPath = Join-Path $releaseRoot "SHA256SUMS.txt"
[System.IO.File]::WriteAllText($checksumsPath, (($checksumLines -join "`n") + "`n"), $Utf8NoBom)
foreach ($line in Get-Content -LiteralPath $checksumsPath -Encoding UTF8) {
  $parts = $line -split '\s+\*', 2
  if ($parts.Count -ne 2) { throw "Invalid checksum line." }
  Assert-Hash (Join-Path $releaseRoot $parts[1]) $parts[0] "Release artifact $($parts[1])" | Out-Null
}

Write-Host "Release package prepared: $releaseRoot"
Write-Host "Artifact: $releaseInstallerName"
Write-Host "Installer SHA-256: $installerSha256"
Write-Host "Authenticode: $($signature.Status)"
Write-Host "Source snapshot: $sourceArchiveName"
Write-Host "Release manifest: $releaseManifestPath"
Write-Host "SHA256SUMS: $checksumsPath"
