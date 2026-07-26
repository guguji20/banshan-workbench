[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$PinnedVersion = "0.144.5"
$PinnedPlatformVersion = "0.144.5-win32-x64"
$TargetTriple = "x86_64-pc-windows-msvc"
$TarballUrl = "https://registry.npmjs.org/@openai/codex/-/codex-$PinnedPlatformVersion.tgz"
$TarballSha1 = "b9d63532a8cb0e113625c3c9ed0f14b669f50e87"
$TarballIntegrity = "sha512-DnsSTlnnzleTxvLwIGnBitKInscxn2I7qASqosS8Fv+qysBygd+ZiBn/SQsRCgQ28PAlsNzmd3Gf3ZTecolAmg=="
$SignerSubject = 'CN="OpenAI OpCo, LLC", O="OpenAI OpCo, LLC", L=San Francisco, S=California, C=US'
$SignerThumbprint = "838CD705CC1344F84DAF4A7479BD532445B3ABED"

$ExpectedFiles = [ordered]@{
  "codex.exe" = @{ Size = 341195568L; Sha256 = "efdb3540ef74b9909408c8d38da79483454797b36f471e3e004fc2bf2b70e22a" }
  "codex-resources/codex-command-runner.exe" = @{ Size = 1271088L; Sha256 = "61578d088b9ea335c7a66bf4b1b0abe615dd8c2b37dde28b8618084f353989d7" }
  "codex-resources/codex-windows-sandbox-setup.exe" = @{ Size = 8817456L; Sha256 = "26d484975fca809537bf279de0330bb756047b0b3645c65b5b46930970ae1dff" }
  "codex-path/rg.exe" = @{ Size = 4266496L; Sha256 = "decdd4992f3f1b9a5ef9898f1b40ab16886d579d6516b4efd3d5eaa19364e408" }
  "rg.exe" = @{ Size = 4266496L; Sha256 = "decdd4992f3f1b9a5ef9898f1b40ab16886d579d6516b4efd3d5eaa19364e408" }
  "codex-package.json" = @{ Size = 215L; Sha256 = "b4bce9027f4cf61aa020d5080a33a7d381cee081580728676b02ed367b2bed3b" }
  "LICENSE" = @{ Size = 10926L; Sha256 = "d17f227e4df5da1600391338865ce0f3055211760a36688f816941d58232d8dc" }
  "NOTICE" = @{ Size = 242L; Sha256 = "9d71575ecfd9a843fc1677b0efb08053c6ba9fd686a0de1a6f5382fd3c220915" }
}

$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$RuntimeRoot = Join-Path $RepoRoot "src-tauri\resources\codex-runtime"
$ManifestPath = Join-Path $RuntimeRoot "manifest.json"
$TauriConfigPath = Join-Path $RepoRoot "src-tauri\tauri.conf.json"

function Get-LowerSha256 {
  param([Parameter(Mandatory = $true)][string]$Path)
  return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
  throw "Codex sidecar manifest is missing: $ManifestPath"
}

$manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
if ($manifest.schemaVersion -ne 1) { throw "Unsupported sidecar manifest schema: $($manifest.schemaVersion)" }
if ($manifest.version -ne $PinnedVersion) { throw "Manifest version mismatch: $($manifest.version)" }
if ($manifest.cliVersionOutput -ne "codex-cli $PinnedVersion") { throw "Manifest CLI output mismatch: $($manifest.cliVersionOutput)" }
if ($manifest.target -ne $TargetTriple) { throw "Manifest target mismatch: $($manifest.target)" }
if ($manifest.entrypoint -ne "codex.exe") { throw "Manifest entrypoint mismatch: $($manifest.entrypoint)" }
if ($manifest.source.releaseTag -ne "rust-v$PinnedVersion") { throw "Manifest release tag mismatch: $($manifest.source.releaseTag)" }
if ($manifest.source.npmPlatformVersion -ne $PinnedPlatformVersion) { throw "Manifest npm version mismatch: $($manifest.source.npmPlatformVersion)" }
if ($manifest.source.npmTarball -ne $TarballUrl) { throw "Manifest npm tarball mismatch: $($manifest.source.npmTarball)" }
if ($manifest.source.npmTarballSha1 -ne $TarballSha1) { throw "Manifest npm tarball SHA-1 mismatch." }
if ($manifest.source.npmTarballIntegrity -ne $TarballIntegrity) { throw "Manifest npm tarball integrity mismatch." }
if ($manifest.authenticode.signerSubject -ne $SignerSubject) { throw "Manifest signer subject mismatch." }
if ($manifest.authenticode.signerThumbprint -ne $SignerThumbprint) { throw "Manifest signer thumbprint mismatch." }
if ($manifest.license.expression -ne "Apache-2.0") { throw "Manifest license expression mismatch." }
if ($manifest.license.licenseFile -ne "LICENSE") { throw "Manifest license path mismatch." }
if ($manifest.license.noticeFile -ne "NOTICE") { throw "Manifest notice path mismatch." }

$manifestByPath = @{}
foreach ($record in $manifest.files) {
  $manifestByPath[$record.path] = $record
}

foreach ($relativePath in $ExpectedFiles.Keys) {
  $windowsRelativePath = $relativePath.Replace('/', '\')
  $path = Join-Path $RuntimeRoot $windowsRelativePath
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
    throw "Bundled Codex file is missing: $relativePath"
  }

  $item = Get-Item -LiteralPath $path
  $expected = $ExpectedFiles[$relativePath]
  if ($item.Length -ne $expected.Size) {
    throw "Unexpected size for $relativePath. Expected $($expected.Size), got $($item.Length)."
  }

  $sha256 = Get-LowerSha256 -Path $path
  if ($sha256 -ne $expected.Sha256) {
    throw "Unexpected SHA-256 for $relativePath. Expected $($expected.Sha256), got $sha256."
  }

  if (-not $manifestByPath.ContainsKey($relativePath)) {
    throw "Manifest does not contain $relativePath."
  }
  $manifestRecord = $manifestByPath[$relativePath]
  if ([long]$manifestRecord.sizeBytes -ne $expected.Size -or $manifestRecord.sha256 -ne $expected.Sha256) {
    throw "Manifest file record does not match pinned values for $relativePath."
  }
}

$actualFiles = Get-ChildItem -LiteralPath $RuntimeRoot -File -Recurse | ForEach-Object {
  $_.FullName.Substring($RuntimeRoot.Length).TrimStart('\').Replace('\', '/')
} | Where-Object { $_ -ne "manifest.json" } | Sort-Object
$expectedPaths = @($ExpectedFiles.Keys) | Sort-Object
if (($actualFiles -join "`n") -ne ($expectedPaths -join "`n")) {
  throw "Unexpected files exist in codex-runtime. Expected: $($expectedPaths -join ', '). Actual: $($actualFiles -join ', ')."
}

$codexBin = Join-Path $RuntimeRoot "codex.exe"
$versionOutput = (& $codexBin --version 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $versionOutput -ne "codex-cli $PinnedVersion") {
  throw "Bundled Codex version check failed: '$versionOutput'."
}

$signature = Get-AuthenticodeSignature -LiteralPath $codexBin
if ($null -eq $signature.SignerCertificate) { throw "Bundled codex.exe is not Authenticode signed." }
if ($signature.SignerCertificate.Subject -ne $SignerSubject) { throw "Bundled codex.exe signer mismatch." }
if ($signature.SignerCertificate.Thumbprint -ne $SignerThumbprint) { throw "Bundled codex.exe signer thumbprint mismatch." }
if ($signature.Status -eq [System.Management.Automation.SignatureStatus]::HashMismatch -or $signature.Status -eq [System.Management.Automation.SignatureStatus]::NotSigned) {
  throw "Bundled codex.exe Authenticode status is $($signature.Status)."
}

$tauriConfig = Get-Content -LiteralPath $TauriConfigPath -Raw -Encoding UTF8 | ConvertFrom-Json
$resourcePaths = @($tauriConfig.bundle.resources)
if ($resourcePaths -notcontains "resources/codex-runtime/") {
  throw "Tauri bundle resources do not include resources/codex-runtime/."
}

$manifestSha256 = Get-LowerSha256 -Path $ManifestPath
Write-Host "Codex sidecar verification passed."
Write-Host "Version: $versionOutput"
Write-Host "Entrypoint SHA-256: $($ExpectedFiles['codex.exe'].Sha256)"
Write-Host "Entrypoint size: $($ExpectedFiles['codex.exe'].Size) bytes"
Write-Host "Manifest SHA-256: $manifestSha256"
Write-Host "Authenticode: $($signature.Status); $($signature.SignerCertificate.Subject)"
Write-Host "Tauri resource: resources/codex-runtime/"