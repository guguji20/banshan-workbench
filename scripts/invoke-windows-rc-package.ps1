[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [ValidateNotNullOrEmpty()]
  [string]$Version,
  [switch]$ReleaseCandidate
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$repoPrefix = $repoRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
$releasePackagingDirectory = [System.IO.Path]::GetFullPath((Join-Path $repoRoot ".runtime\release-packaging"))
$releasePackagingPrefix = $releasePackagingDirectory.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)

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

function Resolve-ReleasePackagingPath([string]$Path, [string]$Label) {
  $resolved = Resolve-RepoPath $Path $Label
  if (-not $resolved.StartsWith($releasePackagingPrefix, [System.StringComparison]::OrdinalIgnoreCase) -and
      $resolved -ne $releasePackagingDirectory) {
    throw "$Label path must remain inside .runtime/release-packaging."
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
$runtimeRoot = Resolve-RepoPath (Join-Path $repoRoot ".runtime") "Runtime root"
$runtimeDirectory = Resolve-ReleasePackagingPath $releasePackagingDirectory "Release packaging runtime directory"
$backupPath = Resolve-ReleasePackagingPath (Join-Path $runtimeDirectory "r2.config.json.backup") "R2 config backup"
$packageScriptPath = Resolve-RepoPath (Join-Path $repoRoot "scripts\package-release.ps1") "Release packaging script"

Assert-NotReparsePoint $repoRoot "Repository root"
Assert-NotReparsePoint $runtimeRoot "Runtime root"
Assert-NotReparsePoint $runtimeDirectory "Release packaging runtime directory"
Assert-NotReparsePoint $configPath "R2 config"
Assert-NotReparsePoint $packageScriptPath "Release packaging script"

if (-not (Test-Path -LiteralPath $configPath -PathType Leaf)) {
  throw "R2 config does not exist."
}
if (-not (Test-Path -LiteralPath $packageScriptPath -PathType Leaf)) {
  throw "Release packaging script does not exist."
}
if (Test-Path -LiteralPath $backupPath) {
  throw "Refusing to overwrite an existing single-file backup."
}

$previousCargoJobs = $env:CARGO_BUILD_JOBS
$previousCargoDebug = $env:CARGO_PROFILE_TEST_DEBUG
$previousCargoIncremental = $env:CARGO_INCREMENTAL
$backupCreated = $false
$originalConfigHash = $null
$restoreFailure = $null

try {
  New-Item -ItemType Directory -Path $runtimeDirectory -Force | Out-Null
  Assert-NotReparsePoint $runtimeDirectory "Release packaging runtime directory"

  $originalConfigHash = (Get-FileHash -LiteralPath $configPath -Algorithm SHA256).Hash
  Copy-Item -LiteralPath $configPath -Destination $backupPath
  $backupCreated = $true
  Assert-NotReparsePoint $backupPath "R2 config backup"

  $configText = [System.IO.File]::ReadAllText($configPath, $utf8NoBom)
  $credentialPattern = '(?m)(?<prefix>"(?<name>accessKeyId|secretAccessKey)"\s*:\s*)"(?:\\.|[^"\\])*"'
  $credentialMatches = [regex]::Matches($configText, $credentialPattern)
  $accessKeyMatches = @($credentialMatches | Where-Object { $_.Groups["name"].Value -ceq "accessKeyId" })
  $secretKeyMatches = @($credentialMatches | Where-Object { $_.Groups["name"].Value -ceq "secretAccessKey" })
  if ($credentialMatches.Count -ne 2 -or $accessKeyMatches.Count -ne 1 -or $secretKeyMatches.Count -ne 1) {
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

  Write-Host "Invoking Windows RC release packaging with a temporary public-only R2 config."
  $global:LASTEXITCODE = 0
  if ($ReleaseCandidate) {
    & $packageScriptPath -Version $Version -ReleaseCandidate
  } else {
    & $packageScriptPath -Version $Version
  }
  $packageSucceeded = $?
  if (-not $packageSucceeded) {
    throw "Release packaging failed."
  }
  if ($LASTEXITCODE -ne 0) {
    throw "Release packaging failed with exit code $LASTEXITCODE."
  }
}
finally {
  if ($backupCreated -and (Test-Path -LiteralPath $backupPath -PathType Leaf)) {
    try {
      Copy-Item -LiteralPath $backupPath -Destination $configPath -Force
      $restoredConfigHash = (Get-FileHash -LiteralPath $configPath -Algorithm SHA256).Hash
      if ($restoredConfigHash -cne $originalConfigHash) {
        throw "Restored R2 config SHA-256 mismatch."
      }
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
