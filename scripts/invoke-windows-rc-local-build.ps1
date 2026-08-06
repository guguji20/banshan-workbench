[CmdletBinding()]
param(
  [switch]$UseExistingInstaller
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$repoPrefix = $repoRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)

function Resolve-RepoPath([string]$Path, [string]$Label) {
  if ([string]::IsNullOrWhiteSpace($Path)) {
    throw "$Label path is empty."
  }
  $resolved = [System.IO.Path]::GetFullPath($Path)
  if (-not $resolved.StartsWith($repoPrefix, [System.StringComparison]::OrdinalIgnoreCase) -and
      $resolved -ne $repoRoot) {
    throw "$Label path must remain inside the repository."
  }
  return $resolved
}

function Assert-NotReparsePoint([string]$Path, [string]$Label) {
  if (Test-Path -LiteralPath $Path) {
    $item = Get-Item -LiteralPath $Path -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
      throw "$Label must not be a reparse point."
    }
  }
}

$configPath = Resolve-RepoPath (Join-Path $repoRoot "src-tauri\resources\r2.config.json") "R2 config"
$runtimeDirectory = Resolve-RepoPath (Join-Path $repoRoot ".runtime\windows-rc") "Windows RC runtime directory"
$backupPath = Resolve-RepoPath (Join-Path $runtimeDirectory "r2.config.json.backup") "R2 config backup"
$publicOnlyBuildPath = Resolve-RepoPath (Join-Path $runtimeDirectory "public-only-build.json") "Public-only build config"
$buildScriptPath = Resolve-RepoPath (Join-Path $repoRoot "scripts\build-internal-preview.ps1") "Internal preview build script"

Assert-NotReparsePoint $repoRoot "Repository root"
Assert-NotReparsePoint $configPath "R2 config"
Assert-NotReparsePoint $runtimeDirectory "Windows RC runtime directory"
Assert-NotReparsePoint $buildScriptPath "Internal preview build script"

if (-not (Test-Path -LiteralPath $configPath -PathType Leaf)) {
  throw "R2 config does not exist."
}
if (-not (Test-Path -LiteralPath $buildScriptPath -PathType Leaf)) {
  throw "Internal preview build script does not exist."
}
if (Test-Path -LiteralPath $backupPath) {
  throw "Refusing to overwrite an existing single-file backup."
}
if (Test-Path -LiteralPath $publicOnlyBuildPath) {
  throw "The public-only build config must not exist before invocation."
}

$previousCargoJobs = $env:CARGO_BUILD_JOBS
$previousCargoDebug = $env:CARGO_PROFILE_TEST_DEBUG
$previousCargoIncremental = $env:CARGO_INCREMENTAL
$backupCreated = $false
$restoreFailure = $null

try {
  New-Item -ItemType Directory -Path $runtimeDirectory -Force | Out-Null
  Assert-NotReparsePoint $runtimeDirectory "Windows RC runtime directory"

  Copy-Item -LiteralPath $configPath -Destination $backupPath -Force
  $backupCreated = $true
  Assert-NotReparsePoint $backupPath "R2 config backup"

  $configText = [System.IO.File]::ReadAllText($configPath, $utf8NoBom)
  $credentialPattern = '(?m)(?<prefix>"(?:accessKeyId|secretAccessKey)"\s*:\s*)"(?:\\.|[^"\\])*"'
  $credentialMatches = [regex]::Matches($configText, $credentialPattern)
  if ($credentialMatches.Count -ne 2) {
    throw "R2 config must contain exactly one accessKeyId and one secretAccessKey field."
  }

  $publicConfigText = [regex]::Replace(
    $configText,
    $credentialPattern,
    [System.Text.RegularExpressions.MatchEvaluator]{
      param($Match)
      return $Match.Groups["prefix"].Value + '""'
    }
  )

  $publicConfig = $publicConfigText | ConvertFrom-Json
  if (-not [string]::IsNullOrEmpty([string]$publicConfig.accessKeyId) -or
      -not [string]::IsNullOrEmpty([string]$publicConfig.secretAccessKey)) {
    throw "Sanitized R2 config did not clear both credential fields."
  }

  [System.IO.File]::WriteAllText($configPath, $publicConfigText, $utf8NoBom)
  $env:CARGO_BUILD_JOBS = "1"
  $env:CARGO_PROFILE_TEST_DEBUG = "0"
  $env:CARGO_INCREMENTAL = "0"

  Write-Host "Invoking the Windows RC build with a temporary public-only R2 config."
  if ($UseExistingInstaller) {
    & $buildScriptPath -ConfigPath $publicOnlyBuildPath -UseExistingInstaller
  } else {
    & $buildScriptPath -ConfigPath $publicOnlyBuildPath
  }
  if ($LASTEXITCODE -ne 0) {
    throw "Internal preview build failed with exit code $LASTEXITCODE."
  }
}
finally {
  if ($backupCreated -and (Test-Path -LiteralPath $backupPath -PathType Leaf)) {
    try {
      Copy-Item -LiteralPath $backupPath -Destination $configPath -Force
    }
    catch {
      $restoreFailure = $_
    }

    try {
      Remove-Item -LiteralPath $backupPath -Force
    }
    catch {
      if ($null -eq $restoreFailure) {
        $restoreFailure = $_
      }
    }
  }

  $env:CARGO_BUILD_JOBS = $previousCargoJobs
  $env:CARGO_PROFILE_TEST_DEBUG = $previousCargoDebug
  $env:CARGO_INCREMENTAL = $previousCargoIncremental

  if ($null -ne $restoreFailure) {
    throw "Unable to restore the original R2 config or remove its single-file backup."
  }
}