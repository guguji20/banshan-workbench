[CmdletBinding()]
param(
  [string]$SourcePackageRoot = "",
  [switch]$ForceDownload
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$PinnedVersion = "0.144.5"
$PinnedPlatformVersion = "0.144.5-win32-x64"
$TargetTriple = "x86_64-pc-windows-msvc"
$Registry = "https://registry.npmjs.org/"
$PackageSpec = "@openai/codex@$PinnedPlatformVersion"
$TarballUrl = "https://registry.npmjs.org/@openai/codex/-/codex-$PinnedPlatformVersion.tgz"
$TarballSha1 = "b9d63532a8cb0e113625c3c9ed0f14b669f50e87"
$TarballIntegrity = "sha512-DnsSTlnnzleTxvLwIGnBitKInscxn2I7qASqosS8Fv+qysBygd+ZiBn/SQsRCgQ28PAlsNzmd3Gf3ZTecolAmg=="
$SignerSubject = 'CN="OpenAI OpCo, LLC", O="OpenAI OpCo, LLC", L=San Francisco, S=California, C=US'
$SignerThumbprint = "838CD705CC1344F84DAF4A7479BD532445B3ABED"

$PinnedSourceFiles = [ordered]@{
  "bin\codex.exe" = @{ Root = "package"; Path = "bin\codex.exe"; Size = 341195568L; Sha256 = "efdb3540ef74b9909408c8d38da79483454797b36f471e3e004fc2bf2b70e22a" }
  "codex-resources\codex-command-runner.exe" = @{ Root = "package"; Path = "codex-resources\codex-command-runner.exe"; Size = 1271088L; Sha256 = "61578d088b9ea335c7a66bf4b1b0abe615dd8c2b37dde28b8618084f353989d7" }
  "codex-resources\codex-windows-sandbox-setup.exe" = @{ Root = "package"; Path = "codex-resources\codex-windows-sandbox-setup.exe"; Size = 8817456L; Sha256 = "26d484975fca809537bf279de0330bb756047b0b3645c65b5b46930970ae1dff" }
  "codex-path\rg.exe" = @{ Root = "package"; Path = "codex-path\rg.exe"; Size = 4266496L; Sha256 = "decdd4992f3f1b9a5ef9898f1b40ab16886d579d6516b4efd3d5eaa19364e408" }
  "codex-package.json" = @{ Root = "package"; Path = "codex-package.json"; Size = 215L; Sha256 = "b4bce9027f4cf61aa020d5080a33a7d381cee081580728676b02ed367b2bed3b" }
  "vendor\openai-codex\rust-v0.144.5\LICENSE" = @{ Root = "repository"; Path = "vendor\openai-codex\rust-v0.144.5\LICENSE"; Size = 10926L; Sha256 = "d17f227e4df5da1600391338865ce0f3055211760a36688f816941d58232d8dc" }
  "vendor\openai-codex\rust-v0.144.5\NOTICE" = @{ Root = "repository"; Path = "vendor\openai-codex\rust-v0.144.5\NOTICE"; Size = 242L; Sha256 = "9d71575ecfd9a843fc1677b0efb08053c6ba9fd686a0de1a6f5382fd3c220915" }
}

$DestinationMappings = @(
  @{ Source = "bin\codex.exe"; Destination = "codex.exe"; Role = "app-server entrypoint" },
  @{ Source = "codex-resources\codex-command-runner.exe"; Destination = "codex-resources\codex-command-runner.exe"; Role = "Windows sandbox command runner" },
  @{ Source = "codex-resources\codex-windows-sandbox-setup.exe"; Destination = "codex-resources\codex-windows-sandbox-setup.exe"; Role = "Windows sandbox setup helper" },
  @{ Source = "codex-path\rg.exe"; Destination = "codex-path\rg.exe"; Role = "official package search helper" },
  @{ Source = "codex-path\rg.exe"; Destination = "rg.exe"; Role = "sidecar-adjacent search helper" },
  @{ Source = "codex-package.json"; Destination = "codex-package.json"; Role = "official package layout metadata" },
  @{ Source = "vendor\openai-codex\rust-v0.144.5\LICENSE"; Destination = "LICENSE"; Role = "Apache-2.0 license" },
  @{ Source = "vendor\openai-codex\rust-v0.144.5\NOTICE"; Destination = "NOTICE"; Role = "OpenAI Codex notice" }
)

$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$RuntimeRoot = Join-Path $RepoRoot "src-tauri\resources\codex-runtime"
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$DownloadRoot = $null

function Get-LowerSha256 {
  param([Parameter(Mandatory = $true)][string]$Path)
  return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-Sha512Integrity {
  param([Parameter(Mandatory = $true)][string]$Path)
  $algorithm = [System.Security.Cryptography.SHA512]::Create()
  try {
    $stream = [System.IO.File]::OpenRead($Path)
    try {
      $digest = $algorithm.ComputeHash($stream)
    } finally {
      $stream.Dispose()
    }
  } finally {
    $algorithm.Dispose()
  }
  return "sha512-$([Convert]::ToBase64String($digest))"
}

function Assert-PinnedSourcePackage {
  param([Parameter(Mandatory = $true)][string]$Root)

  $resolvedRoot = [System.IO.Path]::GetFullPath($Root)
  foreach ($sourceKey in $PinnedSourceFiles.Keys) {
    $expected = $PinnedSourceFiles[$sourceKey]
    if ($expected.Root -ne "package") {
      continue
    }
    $path = Join-Path $resolvedRoot $expected.Path
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
      throw "Pinned source file is missing: $path"
    }

    $item = Get-Item -LiteralPath $path
    if ($item.Length -ne $expected.Size) {
      throw "Unexpected size for $sourceKey. Expected $($expected.Size), got $($item.Length)."
    }

    $sha256 = Get-LowerSha256 -Path $path
    if ($sha256 -ne $expected.Sha256) {
      throw "Unexpected SHA-256 for $sourceKey. Expected $($expected.Sha256), got $sha256."
    }
  }

  $codexBin = Join-Path $resolvedRoot "bin\codex.exe"
  $versionOutput = (& $codexBin --version 2>&1 | Out-String).Trim()
  if ($LASTEXITCODE -ne 0 -or $versionOutput -ne "codex-cli $PinnedVersion") {
    throw "Unexpected Codex version output: '$versionOutput'."
  }

  $signature = Get-AuthenticodeSignature -LiteralPath $codexBin
  if ($null -eq $signature.SignerCertificate) {
    throw "The pinned codex.exe is not Authenticode signed."
  }
  if ($signature.SignerCertificate.Subject -ne $SignerSubject) {
    throw "Unexpected codex.exe signer: $($signature.SignerCertificate.Subject)"
  }
  if ($signature.SignerCertificate.Thumbprint -ne $SignerThumbprint) {
    throw "Unexpected codex.exe signer thumbprint: $($signature.SignerCertificate.Thumbprint)"
  }

  return $resolvedRoot
}

function Resolve-LocalSourcePackage {
  $candidates = New-Object System.Collections.Generic.List[string]
  if (-not [string]::IsNullOrWhiteSpace($SourcePackageRoot)) {
    $candidates.Add($SourcePackageRoot)
  }
  if (-not [string]::IsNullOrWhiteSpace($env:BSAIGC_CODEX_PACKAGE_ROOT)) {
    $candidates.Add($env:BSAIGC_CODEX_PACKAGE_ROOT)
  }
  if (-not [string]::IsNullOrWhiteSpace($env:APPDATA)) {
    $candidates.Add((Join-Path $env:APPDATA "npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\$TargetTriple"))
  }

  foreach ($candidate in $candidates) {
    if (-not (Test-Path -LiteralPath $candidate -PathType Container)) {
      continue
    }
    try {
      return Assert-PinnedSourcePackage -Root $candidate
    } catch {
      Write-Warning "Skipping Codex source candidate '$candidate': $($_.Exception.Message)"
    }
  }
  return $null
}

function Download-OfficialSourcePackage {
  $npm = Get-Command npm.cmd -ErrorAction SilentlyContinue
  if ($null -eq $npm) {
    $npm = Get-Command npm -ErrorAction Stop
  }
  $tar = Get-Command tar.exe -ErrorAction SilentlyContinue
  if ($null -eq $tar) {
    $tar = Get-Command tar -ErrorAction Stop
  }

  $tempBase = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
  $script:DownloadRoot = Join-Path $tempBase ("bsaigc-codex-sidecar-$PinnedVersion-" + [Guid]::NewGuid().ToString("N"))
  New-Item -ItemType Directory -Path $script:DownloadRoot | Out-Null

  Write-Host "Downloading $PackageSpec from $Registry"
  $packOutput = & $npm.Source pack $PackageSpec --silent --registry=$Registry --pack-destination $script:DownloadRoot 2>&1
  if ($LASTEXITCODE -ne 0) {
    throw "npm pack failed: $($packOutput | Out-String)"
  }

  $tarballName = ($packOutput | Select-Object -Last 1).ToString().Trim()
  $tarballPath = Join-Path $script:DownloadRoot $tarballName
  if (-not (Test-Path -LiteralPath $tarballPath -PathType Leaf)) {
    throw "npm pack did not produce the expected tarball: $tarballPath"
  }

  $actualSha1 = (Get-FileHash -LiteralPath $tarballPath -Algorithm SHA1).Hash.ToLowerInvariant()
  if ($actualSha1 -ne $TarballSha1) {
    throw "Official npm tarball SHA-1 mismatch. Expected $TarballSha1, got $actualSha1."
  }
  $actualIntegrity = Get-Sha512Integrity -Path $tarballPath
  if ($actualIntegrity -ne $TarballIntegrity) {
    throw "Official npm tarball integrity mismatch. Expected $TarballIntegrity, got $actualIntegrity."
  }

  $extractRoot = Join-Path $script:DownloadRoot "extract"
  New-Item -ItemType Directory -Path $extractRoot | Out-Null
  & $tar.Source -xzf $tarballPath -C $extractRoot
  if ($LASTEXITCODE -ne 0) {
    throw "Failed to extract official Codex npm tarball."
  }

  return Assert-PinnedSourcePackage -Root (Join-Path $extractRoot "package\vendor\$TargetTriple")
}

function Remove-DownloadRootSafely {
  if ([string]::IsNullOrWhiteSpace($script:DownloadRoot) -or -not (Test-Path -LiteralPath $script:DownloadRoot)) {
    return
  }

  $tempBase = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\') + '\'
  $resolved = [System.IO.Path]::GetFullPath($script:DownloadRoot)
  $leaf = Split-Path -Leaf $resolved
  if (-not $resolved.StartsWith($tempBase, [System.StringComparison]::OrdinalIgnoreCase) -or -not $leaf.StartsWith("bsaigc-codex-sidecar-", [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to remove unexpected temporary directory: $resolved"
  }

  Remove-Item -LiteralPath $resolved -Recurse -Force
}

try {
  $sourceRoot = $null
  if (-not $ForceDownload) {
    $sourceRoot = Resolve-LocalSourcePackage
  }
  if ($null -eq $sourceRoot) {
    $sourceRoot = Download-OfficialSourcePackage
  }

  New-Item -ItemType Directory -Path $RuntimeRoot -Force | Out-Null
  foreach ($mapping in $DestinationMappings) {
    $expected = $PinnedSourceFiles[$mapping.Source]
    $sourceBase = if ($expected.Root -eq "package") { $sourceRoot } else { $RepoRoot }
    $sourcePath = Join-Path $sourceBase $expected.Path
    if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
      throw "Pinned repository file is missing: $sourcePath"
    }
    $sourceItem = Get-Item -LiteralPath $sourcePath
    $sourceSha256 = Get-LowerSha256 -Path $sourcePath
    if ($sourceItem.Length -ne $expected.Size -or $sourceSha256 -ne $expected.Sha256) {
      throw "Pinned source integrity mismatch: $($mapping.Source)"
    }
    $destinationPath = Join-Path $RuntimeRoot $mapping.Destination
    $destinationParent = Split-Path -Parent $destinationPath
    New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
    Copy-Item -LiteralPath $sourcePath -Destination $destinationPath -Force
  }

  $manifestFiles = @()
  foreach ($mapping in $DestinationMappings) {
    $path = Join-Path $RuntimeRoot $mapping.Destination
    $item = Get-Item -LiteralPath $path
    $manifestFiles += [ordered]@{
      path = $mapping.Destination.Replace('\', '/')
      sourcePath = $mapping.Source.Replace('\', '/')
      role = $mapping.Role
      sizeBytes = [long]$item.Length
      sha256 = Get-LowerSha256 -Path $path
    }
  }

  $manifest = [ordered]@{
    schemaVersion = 1
    name = "OpenAI Codex CLI sidecar"
    version = $PinnedVersion
    cliVersionOutput = "codex-cli $PinnedVersion"
    target = $TargetTriple
    entrypoint = "codex.exe"
    layout = "bsaigc-tauri-resource-v1"
    source = [ordered]@{
      repository = "https://github.com/openai/codex"
      releaseTag = "rust-v$PinnedVersion"
      npmPackage = "@openai/codex"
      npmPlatformVersion = $PinnedPlatformVersion
      npmRegistry = $Registry
      npmTarball = $TarballUrl
      npmTarballSha1 = $TarballSha1
      npmTarballIntegrity = $TarballIntegrity
    }
    authenticode = [ordered]@{
      signerSubject = $SignerSubject
      signerThumbprint = $SignerThumbprint
    }
    license = [ordered]@{
      expression = "Apache-2.0"
      licenseFile = "LICENSE"
      noticeFile = "NOTICE"
    }
    files = $manifestFiles
  }

  $manifestPath = Join-Path $RuntimeRoot "manifest.json"
  $manifestJson = $manifest | ConvertTo-Json -Depth 8
  [System.IO.File]::WriteAllText($manifestPath, $manifestJson + "`n", $Utf8NoBom)

  Write-Host "Codex sidecar prepared: $RuntimeRoot"
  Write-Host "Version: codex-cli $PinnedVersion"
  Write-Host "Entrypoint SHA-256: $($PinnedSourceFiles['bin\codex.exe'].Sha256)"
  Write-Host "Entrypoint size: $($PinnedSourceFiles['bin\codex.exe'].Size) bytes"
} finally {
  Remove-DownloadRootSafely
}