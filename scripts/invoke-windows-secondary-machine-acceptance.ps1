[CmdletBinding()]
param(
  [ValidateSet('Both', 'Upgrade', 'Rollback')]
  [string]$Mode = 'Both',

  [ValidatePattern('^\d{4}-\d{2}-\d{2}$')]
  [string]$FactDate = '2026-07-29',

  [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._-]{0,47}$')]
  [string]$RunIdPrefix = 'secondary-machine',

  [ValidateRange(1, 1000)]
  [int]$ColdStartCount = 20,
  [ValidateRange(1, 300)]
  [int]$StartupObservationSeconds = 8,
  [ValidateRange(1, 300)]
  [int]$ProcessExitTimeoutSeconds = 20,
  [ValidateRange(1, 3600)]
  [int]$InstallerTimeoutSeconds = 300,

  [bool]$RequireDifferentMachine = $true,
  [bool]$RequireDifferentUser = $true,

  [switch]$AllowExistingProductRegistration,
  [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$BundleRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..')).TrimEnd([IO.Path]::DirectorySeparatorChar)
$ManifestPath = Join-Path $BundleRoot 'bundle-manifest.json'
$ChecksumsPath = Join-Path $BundleRoot 'SHA256SUMS.txt'
$EnginePath = Join-Path $BundleRoot 'scripts\invoke-nsis-release-acceptance.ps1'

function Get-Sha256([string]$Path) {
  return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-TextSha256([string]$Text) {
  $sha = [Security.Cryptography.SHA256]::Create()
  try {
    $bytes = $Utf8NoBom.GetBytes($Text)
    return ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
  } finally {
    $sha.Dispose()
  }
}

function Test-IsContainedPath([string]$Candidate, [string]$Parent) {
  $fullCandidate = [IO.Path]::GetFullPath($Candidate).TrimEnd([IO.Path]::DirectorySeparatorChar)
  $fullParent = [IO.Path]::GetFullPath($Parent).TrimEnd([IO.Path]::DirectorySeparatorChar)
  if ($fullCandidate.Equals($fullParent, [StringComparison]::OrdinalIgnoreCase)) { return $true }
  return $fullCandidate.StartsWith($fullParent + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)
}

function Resolve-BundleFile([string]$RelativePath, [string]$Label) {
  if ([string]::IsNullOrWhiteSpace($RelativePath)) { throw ($Label + ' path is empty.') }
  if ([IO.Path]::IsPathRooted($RelativePath)) { throw ($Label + ' path must be relative: ' + $RelativePath) }
  $target = [IO.Path]::GetFullPath((Join-Path $BundleRoot $RelativePath.Replace('/', '\')))
  if (-not (Test-IsContainedPath $target $BundleRoot)) { throw ($Label + ' path escapes the bundle: ' + $RelativePath) }
  return $target
}

function Assert-NoReparsePointInPath([string]$Candidate, [string]$Label) {
  if (-not (Test-IsContainedPath $Candidate $BundleRoot)) { throw ($Label + ' must stay inside the bundle: ' + $Candidate) }
  $current = [IO.Path]::GetFullPath($Candidate).TrimEnd([IO.Path]::DirectorySeparatorChar)
  $stop = [IO.Path]::GetFullPath($BundleRoot).TrimEnd([IO.Path]::DirectorySeparatorChar)
  while ($current.Length -ge $stop.Length) {
    if (Test-Path -LiteralPath $current) {
      $item = Get-Item -LiteralPath $current -Force
      if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw ('Reparse point rejected for ' + $Label + ': ' + $current)
      }
    }
    if ($current.Equals($stop, [StringComparison]::OrdinalIgnoreCase)) { break }
    $parentDirectory = [IO.Directory]::GetParent($current)
    if ($null -eq $parentDirectory) { break }
    $current = $parentDirectory.FullName.TrimEnd([IO.Path]::DirectorySeparatorChar)
  }
}

function Assert-Sha256([string]$Value, [string]$Label) {
  if ($Value -cnotmatch '^[0-9a-f]{64}$') { throw ($Label + ' must be a lowercase SHA-256 hash.') }
}

function Assert-ValidWindowsPathSegment([string]$Value, [string]$Label) {
  if ($Value -match '^(?i:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\..*)?$') {
    throw ($Label + ' uses a reserved Windows device name: ' + $Value)
  }
}

function Assert-StringSet([object[]]$Actual, [string[]]$Expected, [string]$Label) {
  $actualValues = @($Actual | ForEach-Object { [string]$_ } | Sort-Object -Unique)
  $expectedValues = @($Expected | Sort-Object -Unique)
  if (($actualValues -join [Environment]::NewLine) -cne ($expectedValues -join [Environment]::NewLine)) {
    throw ($Label + ' set mismatch.')
  }
}

function Assert-File([string]$Path, [string]$Label) {
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "$Label not found: $Path" }
}

function Assert-Step([object]$Summary, [string]$Name, [string]$Status) {
  $matches = @($Summary.steps | Where-Object { [string]$_.name -ceq $Name -and [string]$_.status -ceq $Status })
  if ($matches.Count -ne 1) { throw "Expected exactly one summary step: $Name=$Status" }
}

Assert-File $ManifestPath 'Bundle manifest'
Assert-File $ChecksumsPath 'Bundle checksum list'
Assert-File $EnginePath 'Acceptance engine'
Assert-ValidWindowsPathSegment -Value $RunIdPrefix -Label 'RunIdPrefix'

$checksumRecords = 0
$checksumPaths = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
foreach ($line in Get-Content -LiteralPath $ChecksumsPath -Encoding UTF8) {
  if ($line -notmatch '^([0-9a-f]{64}) \*(.+)$') { throw "Invalid bundle checksum line: $line" }
  $checksumRecords++
  $relativePath = $matches[2].Replace('\', '/')
  if (-not $checksumPaths.Add($relativePath)) { throw "Duplicate bundle checksum path: $relativePath" }
  $target = Resolve-BundleFile -RelativePath $relativePath -Label 'Bundle file'
  Assert-NoReparsePointInPath -Candidate $target -Label 'bundle file'
  Assert-File $target 'Bundle file'
  if ((Get-Sha256 $target) -cne $matches[1]) { throw "Bundle checksum mismatch: $($matches[2])" }
}
if ($checksumRecords -lt 1) { throw 'Bundle checksum list is empty.' }

$manifest = Get-Content -LiteralPath $ManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ([int]$manifest.schemaVersion -ne 1) { throw 'Unsupported bundle schema version.' }
if ([string]$manifest.artifactKind -cne 'windows-secondary-machine-acceptance-bundle') { throw 'Unexpected bundle type.' }
if ([string]$manifest.factDate -cne $FactDate) { throw 'Bundle fact date mismatch.' }

$Version = [string]$manifest.version
$PreviousVersion = [string]$manifest.previousVersion
if ($Version -cnotmatch '^\d+\.\d+\.\d+$') { throw 'Bundle version is invalid.' }
if ($PreviousVersion -cnotmatch '^\d+\.\d+\.\d+$') { throw 'Bundle previousVersion is invalid.' }
if ([version]$PreviousVersion -ge [version]$Version) { throw 'Bundle previousVersion must be lower than version.' }
Assert-Sha256 -Value ([string]$manifest.currentInstaller.sha256) -Label 'Current installer manifest hash'
Assert-Sha256 -Value ([string]$manifest.previousInstaller.sha256) -Label 'Previous installer manifest hash'
Assert-Sha256 -Value ([string]$manifest.origin.machineIdSha256) -Label 'Origin machine hash'
Assert-Sha256 -Value ([string]$manifest.origin.userSidSha256) -Label 'Origin user SID hash'
if ([bool]$manifest.origin.rawIdentifiersIncluded) { throw 'Bundle manifest must not include raw origin identifiers.' }
if (-not [bool]$manifest.requirements.differentWindowsMachine) { throw 'Bundle must require a different Windows machine.' }
if (-not [bool]$manifest.requirements.differentWindowsUserSid) { throw 'Bundle must require a different Windows user SID.' }
if ([bool]$manifest.requirements.embeddedPreviewCredential) { throw 'Bundle must not require an embedded preview credential.' }
$requiredColdStartCount = [int]$manifest.requirements.coldStartCount
if ($requiredColdStartCount -lt 20) { throw 'Bundle cold-start requirement cannot be lower than 20.' }
if ($ColdStartCount -lt $requiredColdStartCount) {
  throw "ColdStartCount must be at least the bundle requirement of $requiredColdStartCount."
}
if (-not $DryRun -and (-not $RequireDifferentMachine -or -not $RequireDifferentUser)) {
  throw 'Machine and user identity overrides are allowed only during DryRun.'
}

$currentInstallerRelativePath = ([string]$manifest.currentInstaller.path).Replace('\', '/')
$previousInstallerRelativePath = ([string]$manifest.previousInstaller.path).Replace('\', '/')
$currentInstallerPath = Resolve-BundleFile -RelativePath $currentInstallerRelativePath -Label 'Current installer'
$previousInstallerPath = Resolve-BundleFile -RelativePath $previousInstallerRelativePath -Label 'Previous installer'
Assert-NoReparsePointInPath -Candidate $currentInstallerPath -Label 'current installer'
Assert-NoReparsePointInPath -Candidate $previousInstallerPath -Label 'previous installer'
Assert-File $currentInstallerPath 'Current installer'
Assert-File $previousInstallerPath 'Previous installer'
if ((Get-Item -LiteralPath $currentInstallerPath).Length -ne [int64]$manifest.currentInstaller.sizeBytes) { throw 'Current installer size mismatch.' }
if ((Get-Item -LiteralPath $previousInstallerPath).Length -ne [int64]$manifest.previousInstaller.sizeBytes) { throw 'Previous installer size mismatch.' }
if ((Get-Sha256 $currentInstallerPath) -cne [string]$manifest.currentInstaller.sha256) { throw 'Current installer manifest mismatch.' }
if ((Get-Sha256 $previousInstallerPath) -cne [string]$manifest.previousInstaller.sha256) { throw 'Previous installer manifest mismatch.' }

$manifestFilePaths = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
foreach ($fileRecord in @($manifest.files)) {
  $relativePath = ([string]$fileRecord.path).Replace('\', '/')
  if (-not $manifestFilePaths.Add($relativePath)) { throw "Duplicate bundle manifest path: $relativePath" }
  if (-not $checksumPaths.Contains($relativePath)) { throw "Bundle checksum list does not bind manifest file: $relativePath" }
  $target = Resolve-BundleFile -RelativePath $relativePath -Label 'Manifest file'
  Assert-NoReparsePointInPath -Candidate $target -Label 'manifest file'
  Assert-File $target 'Manifest file'
  Assert-Sha256 -Value ([string]$fileRecord.sha256) -Label ('Manifest file hash for ' + $relativePath)
  if ((Get-Item -LiteralPath $target).Length -ne [int64]$fileRecord.sizeBytes) { throw "Manifest file size mismatch: $relativePath" }
  if ((Get-Sha256 $target) -cne [string]$fileRecord.sha256) { throw "Manifest file hash mismatch: $relativePath" }
}
foreach ($requiredPath in @(
  'bundle-manifest.json',
  'scripts/invoke-nsis-release-acceptance.ps1',
  'scripts/invoke-windows-secondary-machine-acceptance.ps1',
  $currentInstallerRelativePath,
  $previousInstallerRelativePath
)) {
  if (-not $checksumPaths.Contains($requiredPath)) { throw "Bundle checksum list does not bind required file: $requiredPath" }
  if ($requiredPath -ne 'bundle-manifest.json' -and -not $manifestFilePaths.Contains($requiredPath)) {
    throw "Bundle manifest does not bind required payload: $requiredPath"
  }
}

$signature = Get-AuthenticodeSignature -LiteralPath $currentInstallerPath
if ($signature.Status -ne [Management.Automation.SignatureStatus]::NotSigned) { throw "Expected NotSigned installer, found $($signature.Status)." }
if ([string]$manifest.currentInstaller.authenticode -cne 'NotSigned') { throw 'Bundle manifest Authenticode expectation mismatch.' }

$machineGuid = [string](Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Cryptography' -Name MachineGuid -ErrorAction Stop).MachineGuid
$userSid = [string][Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$machineSha256 = Get-TextSha256 $machineGuid
$userSidSha256 = Get-TextSha256 $userSid
$differentMachine = $machineSha256 -cne [string]$manifest.origin.machineIdSha256
$differentUser = $userSidSha256 -cne [string]$manifest.origin.userSidSha256
if ($RequireDifferentMachine -and -not $differentMachine) { throw 'This bundle must be executed on a different Windows machine.' }
if ($RequireDifferentUser -and -not $differentUser) { throw 'This bundle must be executed by a different Windows user SID.' }

$dateToken = $FactDate.Replace('-', '')
$upgradeRunId = "$RunIdPrefix-$Version-$dateToken-upgrade"
$rollbackRunId = "$RunIdPrefix-$Version-$dateToken-rollback"
$runtimeRoot = Join-Path $BundleRoot '.runtime\nsis-acceptance'
$evidenceRoot = Join-Path $BundleRoot "evidence\$RunIdPrefix-$Version-$dateToken"
if (-not $DryRun) {
  if (Test-Path -LiteralPath $evidenceRoot) { throw "Evidence directory already exists: $evidenceRoot" }
  Assert-NoReparsePointInPath -Candidate $evidenceRoot -Label 'evidence directory'
}
$common = @{
  InstallerPath = $currentInstallerPath
  PreviousInstallerPath = $previousInstallerPath
  Version = $Version
  ColdStartCount = $ColdStartCount
  StartupObservationSeconds = $StartupObservationSeconds
  ProcessExitTimeoutSeconds = $ProcessExitTimeoutSeconds
  InstallerTimeoutSeconds = $InstallerTimeoutSeconds
}
if ($DryRun) { $common.DryRun = $true }
if ($AllowExistingProductRegistration) { $common.AllowExistingProductRegistration = $true }

$upgradeSummaryPath = $null
$rollbackSummaryPath = $null
$upgradeBackupManifestPath = $null
$rollbackBackupManifestPath = $null
$upgradeBackupManifestSha256 = $null
$rollbackBackupManifestSha256 = $null

if ($Mode -in @('Both', 'Upgrade')) {
  $upgradeArgs = @{} + $common
  $upgradeArgs.RunId = $upgradeRunId
  & $EnginePath @upgradeArgs
  if (-not $DryRun) {
    $upgradeSummaryPath = Join-Path $runtimeRoot "$upgradeRunId\acceptance-summary.json"
    Assert-File $upgradeSummaryPath 'Upgrade acceptance summary'
    $upgradeSummary = Get-Content -LiteralPath $upgradeSummaryPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ([string]$upgradeSummary.status -cne 'passed') { throw 'Upgrade summary did not pass.' }
    if ([string]$upgradeSummary.upgradeKind -cne 'cross-version-upgrade') { throw 'Upgrade summary is not cross-version.' }
    if ([string]$upgradeSummary.initialProductVersion -cne $PreviousVersion) { throw 'Initial product version mismatch.' }
    if ([string]$upgradeSummary.finalProductVersion -cne $Version) { throw 'Final product version mismatch.' }
    if ([int]$upgradeSummary.coldStartCount -ne $ColdStartCount) { throw 'Cold start count mismatch.' }
    if (-not [bool]$upgradeSummary.dataPreservation.backupCreated) { throw 'Upgrade backup was not created.' }
    if ([bool]$upgradeSummary.dataPreservation.rollbackAttempted) { throw 'Unexpected rollback during successful upgrade.' }
    $upgradeBackupManifestPath = [IO.Path]::GetFullPath([string]$upgradeSummary.dataPreservation.manifestPath)
    $upgradeRunRoot = Join-Path $runtimeRoot $upgradeRunId
    if (-not (Test-IsContainedPath $upgradeBackupManifestPath $upgradeRunRoot)) { throw 'Upgrade backup manifest escaped its acceptance run root.' }
    Assert-NoReparsePointInPath -Candidate $upgradeBackupManifestPath -Label 'upgrade backup manifest'
    Assert-File $upgradeBackupManifestPath 'Upgrade backup manifest'
    $upgradeBackupManifestSha256 = Get-Sha256 $upgradeBackupManifestPath
    foreach ($stepName in @('preflight', 'initial-install', 'data-backup', 'first-start', 'upgrade', 'restart', 'uninstall', 'registry-restore')) {
      Assert-Step $upgradeSummary $stepName 'passed'
    }
  }
}

if ($Mode -in @('Both', 'Rollback')) {
  $rollbackArgs = @{} + $common
  $rollbackArgs.RunId = $rollbackRunId
  $rollbackArgs.InjectFailureAfterUpgrade = $true
  $expectedFailureObserved = $false
  try {
    & $EnginePath @rollbackArgs
  } catch {
    if ($DryRun) { throw }
    $expectedFailureObserved = $true
  }
  if (-not $DryRun) {
    if (-not $expectedFailureObserved) { throw 'Rollback injection unexpectedly returned success.' }
    $rollbackSummaryPath = Join-Path $runtimeRoot "$rollbackRunId\acceptance-summary.json"
    Assert-File $rollbackSummaryPath 'Rollback acceptance summary'
    $rollbackSummary = Get-Content -LiteralPath $rollbackSummaryPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ([string]$rollbackSummary.status -cne 'failed') { throw 'Rollback summary must record the injected failure.' }
    if ([string]$rollbackSummary.error -cne 'Injected failure after overwrite upgrade to verify isolated data rollback.') { throw 'Rollback summary error mismatch.' }
    if (-not [bool]$rollbackSummary.dataPreservation.injectFailureAfterUpgrade) { throw 'Rollback injection flag is missing.' }
    if (-not [bool]$rollbackSummary.dataPreservation.backupCreated) { throw 'Rollback backup was not created.' }
    if (-not [bool]$rollbackSummary.dataPreservation.rollbackAttempted) { throw 'Rollback was not attempted.' }
    if (-not [bool]$rollbackSummary.dataPreservation.rollbackCompleted) { throw 'Rollback did not complete.' }
    if ($null -ne $rollbackSummary.dataPreservation.rollbackError) { throw 'Rollback summary contains rollbackError.' }
    if (-not [bool]$rollbackSummary.uninstallCompleted) { throw 'Rollback uninstall did not complete.' }
    if (-not [bool]$rollbackSummary.registryRestored) { throw 'Rollback registry restore did not complete.' }
    $rollbackBackupManifestPath = [IO.Path]::GetFullPath([string]$rollbackSummary.dataPreservation.manifestPath)
    $rollbackRunRoot = Join-Path $runtimeRoot $rollbackRunId
    if (-not (Test-IsContainedPath $rollbackBackupManifestPath $rollbackRunRoot)) { throw 'Rollback backup manifest escaped its acceptance run root.' }
    Assert-NoReparsePointInPath -Candidate $rollbackBackupManifestPath -Label 'rollback backup manifest'
    Assert-File $rollbackBackupManifestPath 'Rollback backup manifest'
    $rollbackBackupManifestSha256 = Get-Sha256 $rollbackBackupManifestPath
    foreach ($stepName in @('preflight', 'initial-install', 'data-backup', 'first-start', 'upgrade', 'data-rollback')) {
      Assert-Step $rollbackSummary $stepName 'passed'
    }
    Assert-Step $rollbackSummary 'acceptance' 'failed'
  }
}

if ($DryRun) {
  [ordered]@{
    factDate = $FactDate
    mode = $Mode
    version = $Version
    previousVersion = $PreviousVersion
    differentMachine = $differentMachine
    differentUser = $differentUser
    bundleChecksumsVerified = $checksumRecords
    authenticode = [string]$signature.Status
    releaseGateEligible = $false
    dryRun = $true
  } | ConvertTo-Json -Depth 5
  return
}

$evidenceRoot = [IO.Path]::GetFullPath($evidenceRoot)
New-Item -ItemType Directory -Path $evidenceRoot -Force | Out-Null
if ($null -ne $upgradeSummaryPath) { Copy-Item -LiteralPath $upgradeSummaryPath -Destination (Join-Path $evidenceRoot 'upgrade-acceptance-summary.json') }
if ($null -ne $rollbackSummaryPath) { Copy-Item -LiteralPath $rollbackSummaryPath -Destination (Join-Path $evidenceRoot 'rollback-acceptance-summary.json') }
if ($null -ne $upgradeBackupManifestPath) {
  $copiedUpgradeManifestPath = Join-Path $evidenceRoot 'upgrade-backup-manifest.json'
  Copy-Item -LiteralPath $upgradeBackupManifestPath -Destination $copiedUpgradeManifestPath
  if ((Get-Sha256 $copiedUpgradeManifestPath) -cne $upgradeBackupManifestSha256) { throw 'Copied upgrade backup manifest SHA-256 mismatch.' }
}
if ($null -ne $rollbackBackupManifestPath) {
  $copiedRollbackManifestPath = Join-Path $evidenceRoot 'rollback-backup-manifest.json'
  Copy-Item -LiteralPath $rollbackBackupManifestPath -Destination $copiedRollbackManifestPath
  if ((Get-Sha256 $copiedRollbackManifestPath) -cne $rollbackBackupManifestSha256) { throw 'Copied rollback backup manifest SHA-256 mismatch.' }
}

$releaseGateEligible = $Mode -ceq 'Both' -and $differentMachine -and $differentUser -and $ColdStartCount -ge 20 -and
  $null -ne $upgradeSummaryPath -and $null -ne $rollbackSummaryPath -and
  $null -ne $upgradeBackupManifestSha256 -and $null -ne $rollbackBackupManifestSha256

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
$evidence = [ordered]@{
  schemaVersion = 1
  artifactKind = 'windows-secondary-machine-acceptance-evidence'
  factDate = $FactDate
  mode = $Mode
  version = $Version
  previousVersion = $PreviousVersion
  machineIdSha256 = $machineSha256
  userSidSha256 = $userSidSha256
  rawIdentifiersIncluded = $false
  differentMachine = $differentMachine
  differentUser = $differentUser
  isElevated = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  osVersion = [Environment]::OSVersion.VersionString
  osArchitecture = [Environment]::GetEnvironmentVariable('PROCESSOR_ARCHITECTURE')
  powershellVersion = [string]$PSVersionTable.PSVersion
  bundleManifestSha256 = Get-Sha256 $ManifestPath
  currentInstallerSha256 = Get-Sha256 $currentInstallerPath
  previousInstallerSha256 = Get-Sha256 $previousInstallerPath
  upgradeSummarySha256 = if ($null -eq $upgradeSummaryPath) { $null } else { Get-Sha256 $upgradeSummaryPath }
  rollbackSummarySha256 = if ($null -eq $rollbackSummaryPath) { $null } else { Get-Sha256 $rollbackSummaryPath }
  upgradeBackupManifestSha256 = $upgradeBackupManifestSha256
  rollbackBackupManifestSha256 = $rollbackBackupManifestSha256
  releaseGateEligible = $releaseGateEligible
  completedAt = (Get-Date).ToString('o')
}
$evidencePath = Join-Path $evidenceRoot 'secondary-machine-evidence.json'
[IO.File]::WriteAllText($evidencePath, (($evidence | ConvertTo-Json -Depth 8) + "`n"), $Utf8NoBom)

$evidenceChecksums = @(
  Get-ChildItem -LiteralPath $evidenceRoot -File |
    Sort-Object Name |
    ForEach-Object { "$(Get-Sha256 $_.FullName) *$($_.Name)" }
)
[IO.File]::WriteAllText((Join-Path $evidenceRoot 'SHA256SUMS.txt'), (($evidenceChecksums -join "`n") + "`n"), $Utf8NoBom)

[ordered]@{
  status = 'passed'
  factDate = $FactDate
  mode = $Mode
  differentMachine = $differentMachine
  differentUser = $differentUser
  releaseGateEligible = $releaseGateEligible
  evidenceRoot = $evidenceRoot
  evidenceFiles = @($evidenceChecksums).Count
} | ConvertTo-Json -Depth 5
