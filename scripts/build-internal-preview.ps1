[CmdletBinding()]
param(
  [string]$ConfigPath = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$runtimeRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot ".runtime"))
if ([string]::IsNullOrWhiteSpace($ConfigPath)) {
  $ConfigPath = Join-Path $runtimeRoot "internal-preview-build.json"
}
$resolvedConfig = [System.IO.Path]::GetFullPath($ConfigPath)
$runtimePrefix = $runtimeRoot.TrimEnd('\') + [System.IO.Path]::DirectorySeparatorChar
if (-not $resolvedConfig.StartsWith($runtimePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
  throw "Internal preview build config must stay under .runtime."
}
if (-not (Test-Path -LiteralPath $resolvedConfig -PathType Leaf)) {
  throw "Internal preview build config is missing: $resolvedConfig"
}

$config = Get-Content -LiteralPath $resolvedConfig -Raw -Encoding UTF8 | ConvertFrom-Json
$apiKey = [string]$config.apiKey
$baseUrl = ([string]$config.baseUrl).Trim().TrimEnd('/')
$model = ([string]$config.model).Trim()
if ($apiKey.Trim().Length -lt 8) {
  throw "Internal preview API key is missing or invalid."
}
if ($baseUrl -cne "https://bsaigc.dpdns.org/v1") {
  throw "Internal preview base URL must match the release contract."
}
if ($model -cne "gpt-5.6-sol") {
  throw "Internal preview model must be gpt-5.6-sol."
}

$previousKey = $env:BSAIGC_INTERNAL_API_KEY
$previousBaseUrl = $env:BSAIGC_INTERNAL_BASE_URL
$previousModel = $env:BSAIGC_INTERNAL_MODEL
try {
  Push-Location $repoRoot
  try {
    $env:BSAIGC_INTERNAL_API_KEY = $null
    $env:BSAIGC_INTERNAL_BASE_URL = $null
    $env:BSAIGC_INTERNAL_MODEL = $null
    & pnpm release:verify
    if ($LASTEXITCODE -ne 0) {
      throw "Internal preview quality gate failed with exit code $LASTEXITCODE."
    }

    $env:BSAIGC_INTERNAL_API_KEY = $apiKey
    $env:BSAIGC_INTERNAL_BASE_URL = $baseUrl
    $env:BSAIGC_INTERNAL_MODEL = $model
    Write-Host "Building internal preview with provider=bsaigc model=$model (credential redacted)."
    & pnpm tauri build --bundles nsis
    if ($LASTEXITCODE -ne 0) {
      throw "Internal preview release build failed with exit code $LASTEXITCODE."
    }
  } finally {
    Pop-Location
  }
} finally {
  $env:BSAIGC_INTERNAL_API_KEY = $previousKey
  $env:BSAIGC_INTERNAL_BASE_URL = $previousBaseUrl
  $env:BSAIGC_INTERNAL_MODEL = $previousModel
  $apiKey = $null
  $config = $null
}
