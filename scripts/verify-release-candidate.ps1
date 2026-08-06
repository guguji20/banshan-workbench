[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [ValidatePattern('^\d+\.\d+\.\d+$')]
  [string]$Version,
  [string]$AcceptanceSummaryPath = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$Utf8 = New-Object System.Text.UTF8Encoding($false, $true)

function Assert-True([bool]$Condition, [string]$Message) {
  if (-not $Condition) { throw $Message }
}

function Get-Sha256([string]$Path) {
  return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Read-Json([string]$Path, [string]$Label) {
  Assert-True (Test-Path -LiteralPath $Path -PathType Leaf) "$Label is missing: $Path"
  try { return ([System.IO.File]::ReadAllText($Path, $Utf8) | ConvertFrom-Json) }
  catch { throw "$Label is not valid strict UTF-8 JSON: $Path" }
}

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$releaseRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "release\$Version"))
Assert-True (Test-Path -LiteralPath $releaseRoot -PathType Container) "Release directory is missing."

$releaseManifest = Read-Json (Join-Path $releaseRoot "release-manifest.json") "Release manifest"
$buildManifest = Read-Json (Join-Path $releaseRoot "build-manifest.json") "Build manifest"
$sourceManifest = Read-Json (Join-Path $releaseRoot "source-manifest.json") "Source manifest"
$installerName = "huabang-business-system-v$Version-windows-x64-setup-unsigned.exe"
$installerPath = Join-Path $releaseRoot $installerName

Assert-True ([string]$releaseManifest.artifactKind -ceq "windows-release-candidate") "Release artifact kind mismatch."
Assert-True ([string]$releaseManifest.version -ceq $Version) "Release manifest version mismatch."
Assert-True ([string]$buildManifest.version -ceq $Version) "Build manifest version mismatch."
Assert-True ([string]$sourceManifest.version -ceq $Version) "Source manifest version mismatch."
Assert-True (-not [bool]$releaseManifest.security.embeddedInternalApiKey) "Release manifest reports an embedded API key."
Assert-True (-not [bool]$releaseManifest.security.bundledR2Credentials) "Release manifest reports bundled R2 credentials."
Assert-True (-not [bool]$buildManifest.security.embeddedInternalApiKey) "Build manifest reports an embedded API key."
Assert-True (-not [bool]$buildManifest.security.bundledR2Credentials) "Build manifest reports bundled R2 credentials."
Assert-True ([bool]$buildManifest.security.publicOnlyR2Config) "Build manifest lacks public-only R2 evidence."
Assert-True ([int]$sourceManifest.security.blockedFindings -eq 0) "Source manifest contains blocked findings."
Assert-True ([string]$sourceManifest.security.result -ceq "passed") "Source security gate did not pass."

Assert-True (Test-Path -LiteralPath $installerPath -PathType Leaf) "Installer is missing."
$installer = Get-Item -LiteralPath $installerPath
$installerSha256 = Get-Sha256 $installerPath
$signature = Get-AuthenticodeSignature -LiteralPath $installerPath
Assert-True ($signature.Status -eq [System.Management.Automation.SignatureStatus]::NotSigned) "Installer Authenticode must be NotSigned."
Assert-True ([string]$releaseManifest.installer.name -ceq $installerName) "Installer name mismatch."
Assert-True ([string]$releaseManifest.installer.sha256 -ceq $installerSha256) "Release installer SHA-256 mismatch."
Assert-True ([int64]$releaseManifest.installer.sizeBytes -eq [int64]$installer.Length) "Release installer size mismatch."
Assert-True ([string]$buildManifest.installer.sha256 -ceq $installerSha256) "Build installer SHA-256 mismatch."

$notice = [System.IO.File]::ReadAllText((Join-Path $releaseRoot "UNSIGNED.txt"), $Utf8)
Assert-True ($notice.Contains("Authenticode: NotSigned")) "UNSIGNED.txt lacks NotSigned disclosure."
Assert-True ($notice.Contains("Version: $Version")) "UNSIGNED.txt version mismatch."
Assert-True ($notice.Contains("SHA-256: $installerSha256")) "UNSIGNED.txt SHA-256 mismatch."

$manifestRecords = @($releaseManifest.files)
$manifestNames = @($manifestRecords | ForEach-Object { [string]$_.name })
Assert-True ($manifestNames.Count -eq @($manifestNames | Select-Object -Unique).Count) "Release manifest contains duplicate names."
foreach ($record in $manifestRecords) {
  $name = [string]$record.name
  Assert-True ([System.IO.Path]::GetFileName($name) -ceq $name) "Release manifest paths must be flat."
  $path = Join-Path $releaseRoot $name
  Assert-True (Test-Path -LiteralPath $path -PathType Leaf) "Manifest file is missing: $name"
  Assert-True ([int64]$record.sizeBytes -eq [int64](Get-Item -LiteralPath $path).Length) "Manifest size mismatch: $name"
  Assert-True ([string]$record.sha256 -ceq (Get-Sha256 $path)) "Manifest SHA-256 mismatch: $name"
}

$checksumRecords = @()
$checksumPath = Join-Path $releaseRoot "SHA256SUMS.txt"
foreach ($line in [System.IO.File]::ReadAllLines($checksumPath, $Utf8)) {
  if ([string]::IsNullOrWhiteSpace($line)) { continue }
  $match = [regex]::Match($line, '^(?<hash>[0-9a-fA-F]{64})\s+\*(?<name>[^\\/]+)$')
  Assert-True $match.Success "Invalid SHA256SUMS line."
  $checksumRecords += [pscustomobject]@{ name = $match.Groups["name"].Value; sha256 = $match.Groups["hash"].Value.ToLowerInvariant() }
}
$checksumNames = @($checksumRecords | ForEach-Object { $_.name })
Assert-True ($checksumNames.Count -eq @($checksumNames | Select-Object -Unique).Count) "SHA256SUMS contains duplicate names."
$actualNames = @(Get-ChildItem -LiteralPath $releaseRoot -File | Sort-Object Name | ForEach-Object { $_.Name })
$expectedNames = @($manifestNames + "release-manifest.json" + "SHA256SUMS.txt" | Sort-Object)
Assert-True (($actualNames -join "`n") -ceq ($expectedNames -join "`n")) "Release directory contains missing or untracked files."
$coveredNames = @($checksumNames | Sort-Object)
$requiredCoveredNames = @($actualNames | Where-Object { $_ -cne "SHA256SUMS.txt" } | Sort-Object)
Assert-True (($coveredNames -join "`n") -ceq ($requiredCoveredNames -join "`n")) "SHA256SUMS coverage mismatch."
foreach ($record in $checksumRecords) {
  Assert-True ($record.sha256 -ceq (Get-Sha256 (Join-Path $releaseRoot $record.name))) "SHA256SUMS mismatch: $($record.name)"
}

if ([string]::IsNullOrWhiteSpace($AcceptanceSummaryPath)) {
  $AcceptanceSummaryPath = Join-Path $releaseRoot "nsis-acceptance-summary.json"
}
$acceptance = Read-Json ([System.IO.Path]::GetFullPath($AcceptanceSummaryPath)) "Acceptance summary"
Assert-True ([string]$acceptance.status -ceq "passed") "Acceptance did not pass."
Assert-True ([string]$acceptance.version -ceq $Version) "Acceptance version mismatch."
Assert-True ([string]$acceptance.upgradeKind -ceq "same-package-reinstall") "Acceptance kind mismatch."
Assert-True ([string]$acceptance.installer.sha256 -ceq $installerSha256) "Acceptance installer SHA-256 mismatch."
Assert-True ([int]$acceptance.coldStartCount -ge 20) "Acceptance requires 20 cold starts."
Assert-True ([bool]$acceptance.uninstallCompleted -and [bool]$acceptance.registryRestored) "Acceptance cleanup evidence mismatch."

$upgradePath = Join-Path $releaseRoot "upgrade-acceptance-summary.json"
$rollbackPath = Join-Path $releaseRoot "rollback-acceptance-summary.json"
if ((Test-Path -LiteralPath $upgradePath) -or (Test-Path -LiteralPath $rollbackPath)) {
  $upgrade = Read-Json $upgradePath "Cross-version acceptance summary"
  $rollback = Read-Json $rollbackPath "Rollback acceptance summary"
  Assert-True ([string]$upgrade.status -ceq "passed" -and [string]$upgrade.upgradeKind -ceq "cross-version-upgrade") "Cross-version acceptance did not pass."
  Assert-True ([string]$upgrade.finalProductVersion -ceq $Version -and [string]$upgrade.installer.sha256 -ceq $installerSha256) "Cross-version evidence mismatch."
  Assert-True ([string]$rollback.status -ceq "failed" -and [bool]$rollback.dataPreservation.injectFailureAfterUpgrade) "Rollback lacks the expected injected failure."
  Assert-True ([bool]$rollback.dataPreservation.rollbackAttempted -and [bool]$rollback.dataPreservation.rollbackCompleted) "Rollback did not complete."
  Assert-True ($null -eq $rollback.dataPreservation.rollbackError -and [bool]$rollback.uninstallCompleted -and [bool]$rollback.registryRestored) "Rollback cleanup evidence mismatch."
}

if ([bool]$buildManifest.repository.dirty) { Write-Warning "repository.dirty=true; source snapshot is the reproducibility boundary." }
Write-Host "Windows release candidate upload gate passed: $Version"
Write-Host "Installer SHA-256: $installerSha256"
Write-Host "Installer bytes: $($installer.Length)"
Write-Host "Authenticode: $($signature.Status)"
