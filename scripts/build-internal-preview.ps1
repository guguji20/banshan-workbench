[CmdletBinding()]
param(
  [string]$ConfigPath = "",
  [switch]$UseExistingInstaller
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Get-Sha256([string]$Path) {
  return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-ReleaseIdentity([string]$Root) {
  $package = Get-Content -LiteralPath (Join-Path $Root "package.json") -Raw -Encoding UTF8 | ConvertFrom-Json
  $tauri = Get-Content -LiteralPath (Join-Path $Root "src-tauri\tauri.conf.json") -Raw -Encoding UTF8 | ConvertFrom-Json
  $cargoText = Get-Content -LiteralPath (Join-Path $Root "src-tauri\Cargo.toml") -Raw -Encoding UTF8
  $cargoMatch = [regex]::Match($cargoText, '(?m)^version\s*=\s*"(?<version>[^"]+)"\s*$')
  if (-not $cargoMatch.Success) { throw "Cargo package version is missing." }
  $versions = @([string]$package.version, [string]$tauri.version, $cargoMatch.Groups["version"].Value)
  if (@($versions | Select-Object -Unique).Count -ne 1) {
    throw "Release version mismatch: package.json=$($versions[0]), tauri.conf.json=$($versions[1]), Cargo.toml=$($versions[2])."
  }
  if ($versions[0] -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') {
    throw "Release version is not a supported semantic version: $($versions[0])"
  }
  if ([string]::IsNullOrWhiteSpace([string]$tauri.productName)) { throw "Tauri productName is missing." }
  return [pscustomobject][ordered]@{ version = $versions[0]; productName = [string]$tauri.productName }
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
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
    throw "Public-only r2.config.json is missing. Create it with blank accessKeyId and secretAccessKey values before building."
  }
  $config = Get-Content -LiteralPath $path -Raw -Encoding UTF8 | ConvertFrom-Json
  if (-not [string]::IsNullOrWhiteSpace([string]$config.accessKeyId) -or
      -not [string]::IsNullOrWhiteSpace([string]$config.secretAccessKey)) {
    throw "Release candidate build blocked: r2.config.json contains credentials. Values were not printed."
  }
}

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$runtimeRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot ".runtime"))
$identity = Get-ReleaseIdentity $repoRoot

if ([string]::IsNullOrWhiteSpace($ConfigPath)) { $ConfigPath = Join-Path $runtimeRoot "internal-preview-build.json" }
$resolvedConfig = [System.IO.Path]::GetFullPath($ConfigPath)
$runtimePrefix = $runtimeRoot.TrimEnd('\') + [System.IO.Path]::DirectorySeparatorChar
if (-not $resolvedConfig.StartsWith($runtimePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
  throw "Internal preview build config must stay under .runtime."
}
if (Test-Path -LiteralPath $resolvedConfig -PathType Leaf) {
  $legacyConfig = Get-Content -LiteralPath $resolvedConfig -Raw -Encoding UTF8 | ConvertFrom-Json
  if (-not [string]::IsNullOrWhiteSpace([string]$legacyConfig.apiKey)) {
    throw "Release candidate build blocked: the legacy internal-preview config contains an API key. Remove it or use a public-only config."
  }
}

if (-not [string]::IsNullOrWhiteSpace($env:BSAIGC_INTERNAL_API_KEY)) {
  throw "Release candidate build blocked: BSAIGC_INTERNAL_API_KEY must not be present."
}
Assert-NoTrackedSecretFiles $repoRoot
Assert-PublicOnlyR2Config $repoRoot

$artifactRoot = Join-Path $runtimeRoot "windows-rc\$($identity.version)"
New-Item -ItemType Directory -Path $artifactRoot -Force | Out-Null
$buildLogPath = Join-Path $artifactRoot "build-windows-$($identity.version).log"
$transcriptStarted = $false

$previousKey = $env:BSAIGC_INTERNAL_API_KEY
$previousBaseUrl = $env:BSAIGC_INTERNAL_BASE_URL
$previousModel = $env:BSAIGC_INTERNAL_MODEL
$previousCargoJobs = $env:CARGO_BUILD_JOBS
$previousCargoDebug = $env:CARGO_PROFILE_TEST_DEBUG
$previousCargoIncremental = $env:CARGO_INCREMENTAL
try {
  $env:BSAIGC_INTERNAL_API_KEY = $null
  $env:BSAIGC_INTERNAL_BASE_URL = $null
  $env:BSAIGC_INTERNAL_MODEL = $null
  $env:CARGO_BUILD_JOBS = "1"
  $env:CARGO_PROFILE_TEST_DEBUG = "0"
  $env:CARGO_INCREMENTAL = "0"
  Start-Transcript -LiteralPath $buildLogPath -Force | Out-Null
  $transcriptStarted = $true
  if ($UseExistingInstaller) {
    Write-Host "Reusing the precisely named unsigned Windows release candidate installer for post-processing."
  } else {
    Push-Location $repoRoot
    try {
      Write-Host "Building unsigned Windows release candidate $($identity.version) without embedded credentials."
      & pnpm release:verify
      if ($LASTEXITCODE -ne 0) { throw "Release candidate quality gate failed with exit code $LASTEXITCODE." }
      & pnpm tauri build --bundles nsis
      if ($LASTEXITCODE -ne 0) { throw "Release candidate build failed with exit code $LASTEXITCODE." }
    } finally { Pop-Location }
  }
} finally {
  if ($transcriptStarted) { Stop-Transcript | Out-Null }
  $env:BSAIGC_INTERNAL_API_KEY = $previousKey
  $env:BSAIGC_INTERNAL_BASE_URL = $previousBaseUrl
  $env:BSAIGC_INTERNAL_MODEL = $previousModel
  $env:CARGO_BUILD_JOBS = $previousCargoJobs
  $env:CARGO_PROFILE_TEST_DEBUG = $previousCargoDebug
  $env:CARGO_INCREMENTAL = $previousCargoIncremental
}

$installerName = "$($identity.productName)_$($identity.version)_x64-setup.exe"
$installerDirectory = Join-Path $repoRoot "src-tauri\target\release\bundle\nsis"
$installerPath = Join-Path $installerDirectory $installerName
if (-not (Test-Path -LiteralPath $installerPath -PathType Leaf)) {
  throw "Expected NSIS installer was not produced: $installerPath"
}
$installerHash = Get-Sha256 $installerPath
$signature = Get-AuthenticodeSignature -LiteralPath $installerPath
if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::NotSigned) {
  throw "Unsigned RC chain expected Authenticode=NotSigned, got $($signature.Status)."
}

$checksumPath = "$installerPath.sha256"
$unsignedNoticePath = "$installerPath.unsigned.txt"
$buildManifestPath = "$installerPath.build-manifest.json"
[System.IO.File]::WriteAllText($checksumPath, "$installerHash *$installerName`n", $Utf8NoBom)
[System.IO.File]::WriteAllText($unsignedNoticePath, "UNSIGNED WINDOWS RELEASE CANDIDATE`nAuthenticode: NotSigned`nVersion: $($identity.version)`nSHA-256: $installerHash`nDo not present this artifact as code-signed.`n", $Utf8NoBom)

$gitHead = (& git -C $repoRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw "Unable to read Git HEAD for build manifest." }
$gitDirty = -not [string]::IsNullOrWhiteSpace((& git -C $repoRoot status --porcelain))
$resolvedBuildLogPath = [System.IO.Path]::GetFullPath($buildLogPath)
$repoRootPrefix = $repoRoot.TrimEnd('\') + '\'
if (-not $resolvedBuildLogPath.StartsWith($repoRootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
  throw "Build log path must remain inside the repository."
}
$relativeBuildLog = $resolvedBuildLogPath.Substring($repoRoot.TrimEnd('\').Length + 1).Replace('\', '/')
$manifest = [ordered]@{
  schemaVersion = 1
  artifactKind = "windows-release-candidate"
  product = $identity.productName
  version = $identity.version
  builtAtUtc = (Get-Date).ToUniversalTime().ToString("o")
  repository = [ordered]@{ head = $gitHead; dirty = $gitDirty }
  versionSources = [ordered]@{ packageJson = $identity.version; tauriConfig = $identity.version; cargoToml = $identity.version }
  installer = [ordered]@{ fileName = $installerName; sizeBytes = [int64](Get-Item -LiteralPath $installerPath).Length; sha256 = $installerHash; authenticode = [string]$signature.Status; unsigned = $true }
  buildLog = [ordered]@{ relativePath = $relativeBuildLog; sha256 = Get-Sha256 $buildLogPath }
  security = [ordered]@{ embeddedInternalApiKey = $false; bundledR2Credentials = $false; trackedSecretPaths = $false; publicOnlyR2Config = $true }
}
[System.IO.File]::WriteAllText($buildManifestPath, (($manifest | ConvertTo-Json -Depth 8) + "`n"), $Utf8NoBom)

Write-Host "Release candidate installer: $installerPath"
Write-Host "SHA-256: $installerHash"
Write-Host "Authenticode: $($signature.Status)"
Write-Host "Build log: $buildLogPath"
Write-Host "Build manifest: $buildManifestPath"
