param(
  [string]$Tag = "rust-v0.144.5",
  [string]$ExpectedSha256 = "d4398b3652ca7974428c4de46d0e1ebb8793ccb7c65f52b05a7a55078ec49fb5"
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$upstreamRoot = Join-Path $projectRoot "upstream"
$archive = Join-Path $upstreamRoot "openai-codex-$Tag.zip"
$extractTemp = Join-Path $upstreamRoot ".extract-$Tag"
$destination = Join-Path $upstreamRoot "openai-codex"
$url = "https://codeload.github.com/openai/codex/zip/refs/tags/$Tag"

New-Item -ItemType Directory -Force -Path $upstreamRoot | Out-Null
Invoke-WebRequest -Uri $url -OutFile $archive -UseBasicParsing
$actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant()
if ($ExpectedSha256 -and $actual -ne $ExpectedSha256.ToLowerInvariant()) {
  throw "Codex archive hash mismatch. Expected $ExpectedSha256, got $actual"
}

if (Test-Path $extractTemp) { Remove-Item -Recurse -Force -LiteralPath $extractTemp }
New-Item -ItemType Directory -Force -Path $extractTemp | Out-Null
Expand-Archive -LiteralPath $archive -DestinationPath $extractTemp -Force
$source = Get-ChildItem -LiteralPath $extractTemp -Directory | Select-Object -First 1
if (-not $source) { throw "The Codex archive did not contain a source directory." }

if (Test-Path $destination) { Remove-Item -Recurse -Force -LiteralPath $destination }
Move-Item -LiteralPath $source.FullName -Destination $destination
Remove-Item -Recurse -Force -LiteralPath $extractTemp
Write-Host "Pinned OpenAI Codex $Tag at $destination"
Write-Host "SHA-256: $actual"
