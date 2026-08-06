[CmdletBinding()]
param(
  [string]$RunId = 'data-migration-rollback-safety',
  [switch]$DryRun,
  [switch]$KeepArtifacts
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$scriptRoot = [System.IO.Path]::GetFullPath($PSScriptRoot)
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $scriptRoot '..'))
$runtimeRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot '.runtime'))
$acceptanceRoot = [System.IO.Path]::GetFullPath((Join-Path $runtimeRoot 'data-migration-rollback'))
$runRoot = [System.IO.Path]::GetFullPath((Join-Path $acceptanceRoot $RunId))
$profileRoot = [System.IO.Path]::GetFullPath((Join-Path $runRoot 'profile'))
$dataRoot = [System.IO.Path]::GetFullPath((Join-Path $profileRoot 'AppData\Roaming\com.banshan.aigc.desktop'))
$installRoot = [System.IO.Path]::GetFullPath((Join-Path $runRoot 'install'))
$backupRoot = [System.IO.Path]::GetFullPath((Join-Path $runRoot 'rollback\backup'))
$backupManifestPath = [System.IO.Path]::GetFullPath((Join-Path $runRoot 'rollback\backup-manifest.json'))
$quarantineRoot = [System.IO.Path]::GetFullPath((Join-Path $runRoot 'rollback\failed-state'))
$summaryPath = [System.IO.Path]::GetFullPath((Join-Path $runRoot 'migration-rollback-summary.json'))
$markerPath = [System.IO.Path]::GetFullPath((Join-Path $runRoot 'run-marker.json'))
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$startedAt = (Get-Date).ToString('o')
$steps = New-Object System.Collections.Generic.List[object]
$errorMessage = $null

if ($RunId -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$') {
  throw 'RunId must contain only ASCII letters, numbers, dot, underscore, or hyphen.'
}

function Test-DescendantPath {
  param([Parameter(Mandatory = $true)][string]$Candidate, [Parameter(Mandatory = $true)][string]$Parent)
  $candidateFull = [System.IO.Path]::GetFullPath($Candidate).TrimEnd('\')
  $parentFull = [System.IO.Path]::GetFullPath($Parent).TrimEnd('\')
  return $candidateFull.StartsWith($parentFull + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)
}

function Assert-SafeRuntimePath {
  param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$Label)
  if (-not (Test-DescendantPath -Candidate $Path -Parent $acceptanceRoot)) {
    throw ($Label + ' must remain below ' + $acceptanceRoot + ': ' + $Path)
  }
  $current = [System.IO.Path]::GetFullPath($Path).TrimEnd('\')
  $stop = [System.IO.Path]::GetFullPath($acceptanceRoot).TrimEnd('\')
  while ($current.Length -ge $stop.Length) {
    if (Test-Path -LiteralPath $current) {
      $item = Get-Item -LiteralPath $current -Force
      if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw ('reparse point rejected for ' + $Label + ': ' + $current)
      }
    }
    if ($current.Equals($stop, [System.StringComparison]::OrdinalIgnoreCase)) {
      break
    }
    $parent = [System.IO.Directory]::GetParent($current)
    if ($null -eq $parent) {
      break
    }
    $current = $parent.FullName.TrimEnd('\')
  }
}

foreach ($pathSpec in @(
  @{ path = $runRoot; label = 'run root' },
  @{ path = $profileRoot; label = 'isolated profile root' },
  @{ path = $dataRoot; label = 'isolated AppData root' },
  @{ path = $installRoot; label = 'isolated install root' },
  @{ path = $backupRoot; label = 'rollback backup root' },
  @{ path = $backupManifestPath; label = 'rollback backup manifest' },
  @{ path = $quarantineRoot; label = 'rollback quarantine root' },
  @{ path = $summaryPath; label = 'summary path' },
  @{ path = $markerPath; label = 'marker path' }
)) {
  Assert-SafeRuntimePath -Path $pathSpec.path -Label $pathSpec.label
}

function Add-Step {
  param([Parameter(Mandatory = $true)][string]$Name, [Parameter(Mandatory = $true)][string]$Status, [Parameter(Mandatory = $true)][string]$Detail)
  $steps.Add([ordered]@{ name = $Name; status = $Status; detail = $Detail; recordedAt = (Get-Date).ToString('o') })
}

function Get-FileHashLower {
  param([Parameter(Mandatory = $true)][string]$Path)
  return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-Snapshot {
  param([Parameter(Mandatory = $true)][string]$Root)
  $records = New-Object System.Collections.Generic.List[object]
  foreach ($file in @(Get-ChildItem -LiteralPath $Root -Recurse -Force -File | Sort-Object FullName)) {
    $records.Add([ordered]@{
      relativePath = $file.FullName.Substring($Root.Length + 1).Replace('\', '/')
      length = $file.Length
      sha256 = Get-FileHashLower -Path $file.FullName
    })
  }
  return $records.ToArray()
}

function Assert-SnapshotEqual {
  param([Parameter(Mandatory = $true)][object[]]$Expected, [Parameter(Mandatory = $true)][object[]]$Actual, [Parameter(Mandatory = $true)][string]$Label)
  $expectedJson = $Expected | ConvertTo-Json -Depth 8 -Compress
  $actualJson = $Actual | ConvertTo-Json -Depth 8 -Compress
  if ($expectedJson -ne $actualJson) {
    throw ($Label + ' snapshot mismatch.')
  }
}

function Write-JsonFile {
  param([Parameter(Mandatory = $true)]$Value, [Parameter(Mandatory = $true)][string]$Path)
  $parent = Split-Path -Parent $Path
  New-Item -ItemType Directory -Path $parent -Force | Out-Null
  [System.IO.File]::WriteAllText($Path, (($Value | ConvertTo-Json -Depth 12) + [Environment]::NewLine), $utf8NoBom)
}

function New-IsolatedFixture {
  foreach ($relativePath in @(
    'ledger',
    'vault',
    'credentials',
    'codex-home\workspaces',
    'vault\.business-workspace-staging'
  )) {
    New-Item -ItemType Directory -Path (Join-Path $dataRoot $relativePath) -Force | Out-Null
  }
  $sentinels = @(
    @{ domain = 'sqlite'; path = 'ledger\.nsis-acceptance-sqlite-sentinel.json' },
    @{ domain = 'vault'; path = 'vault\.nsis-acceptance-vault-sentinel.json' },
    @{ domain = 'credentials'; path = 'credentials\.nsis-acceptance-credentials-sentinel.json' },
    @{ domain = 'brain-workspace'; path = 'codex-home\workspaces\.nsis-acceptance-brain-workspace-sentinel.json' },
    @{ domain = 'business-workspace'; path = 'vault\.business-workspace-staging\.nsis-acceptance-business-workspace-sentinel.json' }
  )
  foreach ($sentinel in $sentinels) {
    Write-JsonFile -Value ([ordered]@{
      schemaVersion = 1
      runId = $RunId
      domain = $sentinel.domain
      purpose = 'isolated migration, rollback, and uninstall preservation sentinel'
    }) -Path (Join-Path $dataRoot $sentinel.path)
  }
  [System.IO.File]::WriteAllText((Join-Path $dataRoot 'ledger\bsaigc.sqlite3'), 'SQLite fixture sentinel: authoritative ledger placeholder', $utf8NoBom)
  [System.IO.File]::WriteAllText((Join-Path $dataRoot 'vault\asset-preservation.bin'), 'Local Vault fixture sentinel', $utf8NoBom)
  [System.IO.File]::WriteAllText((Join-Path $dataRoot 'credentials\provider-key.dpapi'), 'isolated credential fixture sentinel', $utf8NoBom)
  [System.IO.File]::WriteAllText((Join-Path $dataRoot 'codex-home\workspaces\brain-project.json'), 'brain workspace fixture sentinel', $utf8NoBom)
  [System.IO.File]::WriteAllText((Join-Path $dataRoot 'vault\.business-workspace-staging\workspace-record.json'), 'business workspace fixture sentinel', $utf8NoBom)
  return @(Get-Snapshot -Root $dataRoot)
}

function Write-Summary {
  param([Parameter(Mandatory = $true)][string]$Status)
  $summary = [ordered]@{
    schemaVersion = 1
    purpose = 'isolated data migration, upgrade rollback, and uninstall acceptance'
    runId = $RunId
    status = $Status
    dryRun = [bool]$DryRun
    startedAt = $startedAt
    finishedAt = (Get-Date).ToString('o')
    runtimeRoot = $runtimeRoot
    runRoot = $runRoot
    profileRoot = $profileRoot
    dataRoot = $dataRoot
    installRoot = $installRoot
    rollbackBackupRoot = $backupRoot
    rollbackBackupManifestPath = $backupManifestPath
    rollbackBackupManifestSha256 = if (Test-Path -LiteralPath $backupManifestPath -PathType Leaf) { Get-FileHashLower -Path $backupManifestPath } else { $null }
    rollbackQuarantineRoot = $quarantineRoot
    defaultSafety = 'writes only below .runtime/data-migration-rollback; never uses real AppData'
    steps = $steps.ToArray()
    error = $errorMessage
  }
  if (-not $DryRun) {
    Write-JsonFile -Value $summary -Path $summaryPath
  }
  return $summary
}

try {
  if ($DryRun) {
    Add-Step -Name 'path-safety' -Status 'planned' -Detail 'All generated paths resolve below .runtime/data-migration-rollback and reparse points are rejected.'
    Add-Step -Name 'fixture' -Status 'planned' -Detail 'Create isolated AppData sentinels for SQLite, Vault, credentials, brain workspace, and business workspace.'
    Add-Step -Name 'upgrade' -Status 'planned' -Detail 'Overwrite only the isolated install root and leave the isolated AppData tree untouched.'
    Add-Step -Name 'backup' -Status 'planned' -Detail 'Create and verify an isolated SHA-256 backup manifest before failure injection.'
    Add-Step -Name 'rollback' -Status 'planned' -Detail 'Quarantine failed isolated data and restore the pre-upgrade SHA-256 snapshot.'
    Add-Step -Name 'uninstall' -Status 'planned' -Detail 'Remove only the isolated install root and verify every user-data hash remains.'
    Write-Output ((Write-Summary -Status 'planned') | ConvertTo-Json -Depth 12)
    exit 0
  }

  if (Test-Path -LiteralPath $runRoot) {
    Assert-SafeRuntimePath -Path $runRoot -Label 'idempotent run reset'
    Remove-Item -LiteralPath $runRoot -Recurse -Force
  }
  New-Item -ItemType Directory -Path $runRoot -Force | Out-Null
  Write-JsonFile -Value ([ordered]@{ schemaVersion = 1; runId = $RunId; purpose = 'isolated data migration rollback acceptance' }) -Path $markerPath
  $beforeSnapshot = @(New-IsolatedFixture)
  Add-Step -Name 'fixture' -Status 'passed' -Detail 'Five domain sentinels and representative authoritative files created below isolated AppData.'

  New-Item -ItemType Directory -Path $installRoot -Force | Out-Null
  [System.IO.File]::WriteAllText((Join-Path $installRoot 'version.txt'), 'previous', $utf8NoBom)
  [System.IO.File]::WriteAllText((Join-Path $installRoot 'user-data-must-not-be-here.txt'), 'install payload', $utf8NoBom)
  [System.IO.File]::WriteAllText((Join-Path $installRoot 'version.txt'), 'candidate', $utf8NoBom)
  Assert-SnapshotEqual -Expected $beforeSnapshot -Actual @(Get-Snapshot -Root $dataRoot) -Label 'overwrite upgrade'
  Add-Step -Name 'upgrade' -Status 'passed' -Detail 'Candidate payload overwrote the isolated install root while all five data domains remained byte-for-byte unchanged.'

  New-Item -ItemType Directory -Path (Split-Path -Parent $backupRoot) -Force | Out-Null
  Copy-Item -LiteralPath $dataRoot -Destination $backupRoot -Recurse -Force
  $backupSnapshot = @(Get-Snapshot -Root $backupRoot)
  Assert-SnapshotEqual -Expected $beforeSnapshot -Actual $backupSnapshot -Label 'backup'
  Write-JsonFile -Value $backupSnapshot -Path $backupManifestPath
  $verifiedBackupSnapshot = [object[]](Get-Content -LiteralPath $backupManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json)
  Assert-SnapshotEqual -Expected $beforeSnapshot -Actual $verifiedBackupSnapshot -Label 'backup manifest'
  Add-Step -Name 'backup' -Status 'passed' -Detail 'Pre-upgrade isolated AppData backup verified by SHA-256 manifest.'

  Remove-Item -LiteralPath (Join-Path $dataRoot 'credentials\.nsis-acceptance-credentials-sentinel.json') -Force
  [System.IO.File]::WriteAllText((Join-Path $dataRoot 'ledger\.nsis-acceptance-sqlite-sentinel.json'), 'corrupted by simulated migration failure', $utf8NoBom)
  [System.IO.File]::WriteAllText((Join-Path $installRoot 'migration-failure.marker'), 'failure injected', $utf8NoBom)
  if (Test-Path -LiteralPath $quarantineRoot) {
    throw ('rollback quarantine already exists: ' + $quarantineRoot)
  }
  Move-Item -LiteralPath $dataRoot -Destination $quarantineRoot
  Copy-Item -LiteralPath $backupRoot -Destination $dataRoot -Recurse -Force
  $rollbackExpectedSnapshot = [object[]](Get-Content -LiteralPath $backupManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json)
  Assert-SnapshotEqual -Expected $rollbackExpectedSnapshot -Actual @(Get-Snapshot -Root $dataRoot) -Label 'rollback restore'
  if (-not (Test-Path -LiteralPath (Join-Path $quarantineRoot 'ledger\.nsis-acceptance-sqlite-sentinel.json'))) {
    throw 'failed-state quarantine did not retain the mutated data tree.'
  }
  Add-Step -Name 'rollback' -Status 'passed' -Detail 'Simulated migration failure was quarantined and the exact pre-upgrade data snapshot was restored.'

  Remove-Item -LiteralPath $installRoot -Recurse -Force
  Assert-SnapshotEqual -Expected $beforeSnapshot -Actual @(Get-Snapshot -Root $dataRoot) -Label 'uninstall preservation'
  Add-Step -Name 'uninstall' -Status 'passed' -Detail 'Uninstall simulation removed only the isolated install root; SQLite, Vault, credentials, brain workspace, and business workspace survived.'

  Write-Output ((Write-Summary -Status 'passed') | ConvertTo-Json -Depth 12)
} catch {
  $errorMessage = $_.Exception.Message
  Add-Step -Name 'acceptance' -Status 'failed' -Detail $errorMessage
  Write-Output ((Write-Summary -Status 'failed') | ConvertTo-Json -Depth 12)
  exit 1
}

if (-not $KeepArtifacts -and (Test-Path -LiteralPath $runRoot)) {
  Assert-SafeRuntimePath -Path $runRoot -Label 'cleanup run root'
  Remove-Item -LiteralPath $runRoot -Recurse -Force
}
