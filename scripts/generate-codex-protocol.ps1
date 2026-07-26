[CmdletBinding()]
param(
  [string]$CodexBin = $env:BSAIGC_CODEX_BIN,
  [string]$Version = "0.144.5"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$PinnedVersion = "0.144.5"
$projectRoot = Split-Path -Parent $PSScriptRoot

if ($Version -ne $PinnedVersion) {
  throw "Protocol generation is pinned to Codex CLI $PinnedVersion; requested $Version."
}

function Test-ForbiddenCodexPath {
  param([Parameter(Mandatory = $true)][string]$Path)

  $normalized = $Path.Replace('/', '\').ToLowerInvariant()
  return $normalized.Contains('\windowsapps\') -or
    $normalized.EndsWith('\windowsapps') -or
    $normalized.EndsWith('.cmd') -or
    $normalized.EndsWith('.bat')
}

function Get-DefaultCodexCandidates {
  param([Parameter(Mandatory = $true)][string]$Root)

  $candidates = [System.Collections.Generic.List[string]]::new()
  $candidates.Add((Join-Path $Root 'codex-runtime\codex.exe'))
  $candidates.Add((Join-Path $Root 'src-tauri\resources\codex-runtime\codex.exe'))

  if ($env:APPDATA) {
    $architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToUpperInvariant()

    $package = $null
    $target = $null
    switch ($architecture) {
      'X64' {
        $package = 'codex-win32-x64'
        $target = 'x86_64-pc-windows-msvc'
      }
      'ARM64' {
        $package = 'codex-win32-arm64'
        $target = 'aarch64-pc-windows-msvc'
      }
    }

    if ($package) {
      $candidates.Add((Join-Path $env:APPDATA "npm\node_modules\@openai\codex\node_modules\@openai\$package\vendor\$target\bin\codex.exe"))
    }
  }

  return $candidates
}

function Resolve-PinnedCodexExecutable {
  param(
    [string]$RequestedPath,
    [Parameter(Mandatory = $true)][string]$Root,
    [Parameter(Mandatory = $true)][string]$RequiredVersion
  )

  $usingOverride = -not [string]::IsNullOrWhiteSpace($RequestedPath)
  if ($usingOverride) {
    if (-not [System.IO.Path]::IsPathRooted($RequestedPath)) {
      throw 'BSAIGC_CODEX_BIN/-CodexBin must be an absolute path to the native codex.exe.'
    }
    $candidates = @($RequestedPath)
  } else {
    $candidates = @(Get-DefaultCodexCandidates -Root $Root)
  }

  $errors = [System.Collections.Generic.List[string]]::new()
  foreach ($candidate in $candidates) {
    if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
      if ($usingOverride) { $errors.Add("not found: $candidate") }
      continue
    }

    $resolved = (Resolve-Path -LiteralPath $candidate).ProviderPath
    if ([System.IO.Path]::GetExtension($resolved) -ine '.exe') {
      $errors.Add("not a native .exe: $resolved")
      continue
    }
    if (Test-ForbiddenCodexPath -Path $resolved) {
      $errors.Add("forbidden wrapper/WindowsApps path: $resolved")
      continue
    }

    try {
      $versionOutput = (& $resolved --version 2>&1 | Out-String).Trim()
      $exitCode = $LASTEXITCODE
    } catch {
      $errors.Add("version probe failed: $resolved ($($_.Exception.Message))")
      continue
    }
    if ($exitCode -ne 0) {
      $errors.Add("version probe exited ${exitCode}: $resolved")
      continue
    }
    if ($versionOutput -notmatch "(?m)^codex-cli\s+$([regex]::Escape($RequiredVersion))\s*$") {
      $errors.Add("expected codex-cli $RequiredVersion, got '$versionOutput': $resolved")
      continue
    }

    return $resolved
  }

  $detail = if ($errors.Count -gt 0) { ' ' + ($errors -join '; ') } else { '' }
  throw "Native Codex CLI $RequiredVersion was not found.$detail Set BSAIGC_CODEX_BIN to its absolute codex.exe path."
}

$CodexBin = Resolve-PinnedCodexExecutable -RequestedPath $CodexBin -Root $projectRoot -RequiredVersion $PinnedVersion
$target = Join-Path $projectRoot "vendor\codex-app-server\v$PinnedVersion"
$typescript = Join-Path $target 'typescript'
$jsonSchema = Join-Path $target 'json-schema'
$protocolHome = Join-Path $projectRoot '.runtime\codex-protocol-home'

foreach ($path in @($typescript, $jsonSchema, $protocolHome)) {
  if (Test-Path -LiteralPath $path) {
    Remove-Item -Recurse -Force -LiteralPath $path
  }
  New-Item -ItemType Directory -Force -Path $path | Out-Null
}

$previousCodexHome = $env:CODEX_HOME
try {
  $env:CODEX_HOME = $protocolHome

  & $CodexBin app-server generate-ts --out $typescript
  if ($LASTEXITCODE -ne 0) { throw 'Codex TypeScript protocol generation failed.' }

  & $CodexBin app-server generate-json-schema --out $jsonSchema
  if ($LASTEXITCODE -ne 0) { throw 'Codex JSON Schema generation failed.' }
} finally {
  if ($null -eq $previousCodexHome) {
    Remove-Item Env:CODEX_HOME -ErrorAction SilentlyContinue
  } else {
    $env:CODEX_HOME = $previousCodexHome
  }
}

$tsCount = (Get-ChildItem -LiteralPath $typescript -Recurse -File | Measure-Object).Count
$schemaCount = (Get-ChildItem -LiteralPath $jsonSchema -Recurse -File | Measure-Object).Count
Write-Host "Generated Codex app-server v$PinnedVersion protocol with $CodexBin`: $tsCount TypeScript files, $schemaCount schema files."
