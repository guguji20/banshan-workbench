[CmdletBinding()]
param(
  [ValidatePattern('^\d+\.\d+\.\d+$')]
  [string]$Version = '1.3.4',

  [ValidatePattern('^\d+\.\d+\.\d+$')]
  [string]$PreviousVersion = '1.3.3',

  [ValidatePattern('^\d{4}-\d{2}-\d{2}$')]
  [string]$FactDate = '2026-07-29',

  [string]$OutputRoot = '',

  [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$RepoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..')).TrimEnd([IO.Path]::DirectorySeparatorChar)
$PortableRoot = [IO.Path]::GetFullPath((Join-Path $RepoRoot '.runtime\windows-secondary-machine')).TrimEnd([IO.Path]::DirectorySeparatorChar)

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

function Resolve-ContainedFile([string]$Root, [string]$RelativePath, [string]$Label) {
  if ([string]::IsNullOrWhiteSpace($RelativePath)) { throw "$Label path is empty." }
  if ([IO.Path]::IsPathRooted($RelativePath)) { throw "$Label path must be relative: $RelativePath" }
  $target = [IO.Path]::GetFullPath((Join-Path $Root $RelativePath.Replace('/', '\')))
  if (-not (Test-IsContainedPath $target $Root)) { throw "$Label path escapes $Root`: $RelativePath" }
  return $target
}

function Assert-NoReparsePointInPath([string]$Candidate, [string]$Parent, [string]$Label) {
  if (-not (Test-IsContainedPath $Candidate $Parent)) { throw "$Label must stay under $Parent`: $Candidate" }
  $current = [IO.Path]::GetFullPath($Candidate).TrimEnd([IO.Path]::DirectorySeparatorChar)
  $stop = [IO.Path]::GetFullPath($Parent).TrimEnd([IO.Path]::DirectorySeparatorChar)
  while ($current.Length -ge $stop.Length) {
    if (Test-Path -LiteralPath $current) {
      $item = Get-Item -LiteralPath $current -Force
      if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Reparse point rejected for $Label`: $current"
      }
    }
    if ($current.Equals($stop, [StringComparison]::OrdinalIgnoreCase)) { break }
    $parentDirectory = [IO.Directory]::GetParent($current)
    if ($null -eq $parentDirectory) { break }
    $current = $parentDirectory.FullName.TrimEnd([IO.Path]::DirectorySeparatorChar)
  }
}

function Assert-File([string]$Path, [string]$Label) {
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "$Label not found: $Path"
  }
}

function Assert-ReleaseChecksums([string]$ReleaseRoot, [string[]]$RequiredRelativePaths) {
  $checksumsPath = Join-Path $ReleaseRoot 'SHA256SUMS.txt'
  Assert-File $checksumsPath 'Release checksum list'
  $records = 0
  $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
  foreach ($line in Get-Content -LiteralPath $checksumsPath -Encoding UTF8) {
    if ($line -notmatch '^([0-9a-f]{64}) \*(.+)$') {
      throw "Invalid release checksum line: $line"
    }
    $records++
    $relativePath = $matches[2].Replace('\', '/')
    if (-not $seen.Add($relativePath)) { throw "Duplicate release checksum path: $relativePath" }
    $target = Resolve-ContainedFile -Root $ReleaseRoot -RelativePath $relativePath -Label 'Release artifact'
    Assert-File $target 'Release artifact'
    $actual = Get-Sha256 $target
    if ($actual -cne $matches[1]) {
      throw "Release checksum mismatch: $($matches[2])"
    }
  }
  if ($records -lt 1) { throw 'Release checksum list is empty.' }
  foreach ($requiredRelativePath in $RequiredRelativePaths) {
    if (-not $seen.Contains($requiredRelativePath.Replace('\', '/'))) {
      throw "Release checksum list does not bind required artifact: $requiredRelativePath"
    }
  }
}

function New-Utf8Zip([string]$SourceRoot, [string]$ZipPath) {
  Add-Type -AssemblyName System.IO.Compression
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  $sourcePrefix = $SourceRoot.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
  $files = @(Get-ChildItem -LiteralPath $SourceRoot -File -Recurse | Sort-Object FullName)
  $stream = [IO.File]::Open($ZipPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
  try {
    $archive = New-Object IO.Compression.ZipArchive($stream, [IO.Compression.ZipArchiveMode]::Create, $false, $Utf8NoBom)
    try {
      foreach ($file in $files) {
        $relativePath = $file.FullName.Substring($sourcePrefix.Length).Replace('\', '/')
        $entry = $archive.CreateEntry($relativePath, [IO.Compression.CompressionLevel]::Optimal)
        $entryStream = $entry.Open()
        $inputStream = [IO.File]::OpenRead($file.FullName)
        try {
          $inputStream.CopyTo($entryStream)
        } finally {
          $inputStream.Dispose()
          $entryStream.Dispose()
        }
      }
    } finally {
      $archive.Dispose()
    }
  } finally {
    $stream.Dispose()
  }
}

$ProductName = "$([char]0x534E)$([char]0x90A6)$([char]0x4E92)$([char]0x5A31)$([char]0x5546)$([char]0x52A1)$([char]0x7CFB)$([char]0x7EDF)"
$currentReleaseRoot = Join-Path $RepoRoot "release\$Version"
$previousReleaseRoot = Join-Path $RepoRoot "release\$PreviousVersion"
$releaseManifestPath = Join-Path $currentReleaseRoot 'release-manifest.json'
$currentSourceName = "huabang-business-system-v$Version-windows-x64-setup-unsigned.exe"
$currentSourcePath = Join-Path $currentReleaseRoot $currentSourceName
$currentPortableName = "${ProductName}_${Version}_x64-setup.exe"
$previousPortableName = "${ProductName}_${PreviousVersion}_x64-setup.exe"
$previousSourcePath = Join-Path $previousReleaseRoot $previousPortableName
$engineSourcePath = Join-Path $PSScriptRoot 'invoke-nsis-release-acceptance.ps1'
$runnerSourcePath = Join-Path $PSScriptRoot 'invoke-windows-secondary-machine-acceptance.ps1'
$guideSourcePath = Join-Path $RepoRoot 'docs\WINDOWS_SECONDARY_MACHINE_ACCEPTANCE_20260729.md'

if ([version]$PreviousVersion -ge [version]$Version) {
  throw "PreviousVersion must be lower than Version: $PreviousVersion -> $Version"
}

Assert-NoReparsePointInPath -Candidate $currentReleaseRoot -Parent $RepoRoot -Label 'current release root'
Assert-NoReparsePointInPath -Candidate $previousReleaseRoot -Parent $RepoRoot -Label 'previous release root'

Assert-File $releaseManifestPath 'Release manifest'
Assert-File $currentSourcePath 'Current installer'
Assert-File $previousSourcePath 'Previous installer'
Assert-File $engineSourcePath 'Acceptance engine'
Assert-File $runnerSourcePath 'Portable acceptance runner'
Assert-File $guideSourcePath 'Portable acceptance guide'
Assert-ReleaseChecksums -ReleaseRoot $currentReleaseRoot -RequiredRelativePaths @($currentSourceName, 'release-manifest.json')
Assert-ReleaseChecksums -ReleaseRoot $previousReleaseRoot -RequiredRelativePaths @($previousPortableName)

$releaseManifest = Get-Content -LiteralPath $releaseManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ([string]$releaseManifest.version -cne $Version) { throw 'Release manifest version mismatch.' }
if ([string]$releaseManifest.installer.name -cne $currentSourceName) { throw 'Release manifest installer name mismatch.' }

$currentSha256 = Get-Sha256 $currentSourcePath
$previousSha256 = Get-Sha256 $previousSourcePath
if ($currentSha256 -cne [string]$releaseManifest.installer.sha256) { throw 'Current installer SHA-256 mismatch.' }
if ((Get-Item -LiteralPath $currentSourcePath).Length -ne [int64]$releaseManifest.installer.sizeBytes) { throw 'Current installer size mismatch.' }
$signature = Get-AuthenticodeSignature -LiteralPath $currentSourcePath
if ($signature.Status -ne [Management.Automation.SignatureStatus]::NotSigned) { throw "Expected NotSigned installer, found $($signature.Status)." }
if ($currentSha256 -ceq $previousSha256) { throw 'Current and previous installer hashes must differ.' }

$machineGuid = [string](Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Cryptography' -Name MachineGuid -ErrorAction Stop).MachineGuid
$userSid = [string][Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$originMachineSha256 = Get-TextSha256 $machineGuid
$originUserSidSha256 = Get-TextSha256 $userSid

if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
  $OutputRoot = $PortableRoot
} elseif (-not [IO.Path]::IsPathRooted($OutputRoot)) {
  $OutputRoot = Join-Path $RepoRoot $OutputRoot
}
$OutputRoot = [IO.Path]::GetFullPath($OutputRoot).TrimEnd([IO.Path]::DirectorySeparatorChar)
if (-not (Test-IsContainedPath $OutputRoot $PortableRoot)) {
  throw "OutputRoot must stay under $PortableRoot"
}
Assert-NoReparsePointInPath -Candidate $OutputRoot -Parent $RepoRoot -Label 'bundle output root'

$dateToken = $FactDate.Replace('-', '')
$bundleName = "windows-secondary-machine-acceptance-$Version-$dateToken"
$bundleRoot = Join-Path $OutputRoot $bundleName
$zipPath = Join-Path $OutputRoot ($bundleName + '.zip')
$plan = [ordered]@{
  factDate = $FactDate
  version = $Version
  previousVersion = $PreviousVersion
  bundleRoot = $bundleRoot
  zipPath = $zipPath
  currentInstallerSha256 = $currentSha256
  previousInstallerSha256 = $previousSha256
  authenticode = [string]$signature.Status
  originMachineFingerprintRecorded = $true
  originUserFingerprintRecorded = $true
  dryRun = [bool]$DryRun
}

if ($DryRun) {
  $plan | ConvertTo-Json -Depth 5
  return
}

if (Test-Path -LiteralPath $bundleRoot) { throw "Bundle directory already exists: $bundleRoot" }
if (Test-Path -LiteralPath $zipPath) { throw "Bundle ZIP already exists: $zipPath" }

New-Item -ItemType Directory -Path (Join-Path $bundleRoot 'artifacts') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $bundleRoot 'scripts') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $bundleRoot 'docs') -Force | Out-Null

Copy-Item -LiteralPath $currentSourcePath -Destination (Join-Path $bundleRoot "artifacts\$currentPortableName")
Copy-Item -LiteralPath $previousSourcePath -Destination (Join-Path $bundleRoot "artifacts\$previousPortableName")
Copy-Item -LiteralPath $engineSourcePath -Destination (Join-Path $bundleRoot 'scripts\invoke-nsis-release-acceptance.ps1')
Copy-Item -LiteralPath $runnerSourcePath -Destination (Join-Path $bundleRoot 'scripts\invoke-windows-secondary-machine-acceptance.ps1')
Copy-Item -LiteralPath $guideSourcePath -Destination (Join-Path $bundleRoot 'docs\WINDOWS_SECONDARY_MACHINE_ACCEPTANCE_20260729.md')

$fileRecords = @(
  Get-ChildItem -LiteralPath $bundleRoot -File -Recurse |
    Sort-Object FullName |
    ForEach-Object {
      $relativePath = $_.FullName.Substring($bundleRoot.TrimEnd('\').Length + 1).Replace('\', '/')
      [ordered]@{
        path = $relativePath
        sizeBytes = [int64]$_.Length
        sha256 = Get-Sha256 $_.FullName
      }
    }
)

$bundleManifest = [ordered]@{
  schemaVersion = 1
  artifactKind = 'windows-secondary-machine-acceptance-bundle'
  factDate = $FactDate
  version = $Version
  previousVersion = $PreviousVersion
  currentInstaller = [ordered]@{ path = "artifacts/$currentPortableName"; sizeBytes = [int64](Get-Item -LiteralPath $currentSourcePath).Length; sha256 = $currentSha256; authenticode = 'NotSigned' }
  previousInstaller = [ordered]@{ path = "artifacts/$previousPortableName"; sizeBytes = [int64](Get-Item -LiteralPath $previousSourcePath).Length; sha256 = $previousSha256 }
  origin = [ordered]@{ machineIdSha256 = $originMachineSha256; userSidSha256 = $originUserSidSha256; rawIdentifiersIncluded = $false }
  requirements = [ordered]@{ differentWindowsMachine = $true; differentWindowsUserSid = $true; coldStartCount = 20; embeddedPreviewCredential = $false }
  files = @($fileRecords)
}
$bundleManifestPath = Join-Path $bundleRoot 'bundle-manifest.json'
[IO.File]::WriteAllText($bundleManifestPath, (($bundleManifest | ConvertTo-Json -Depth 10) + "`n"), $Utf8NoBom)

$checksumLines = @(
  Get-ChildItem -LiteralPath $bundleRoot -File -Recurse |
    Where-Object { $_.Name -cne 'SHA256SUMS.txt' } |
    Sort-Object FullName |
    ForEach-Object {
      $relativePath = $_.FullName.Substring($bundleRoot.TrimEnd('\').Length + 1).Replace('\', '/')
      "$(Get-Sha256 $_.FullName) *$relativePath"
    }
)
$checksumsPath = Join-Path $bundleRoot 'SHA256SUMS.txt'
[IO.File]::WriteAllText($checksumsPath, (($checksumLines -join "`n") + "`n"), $Utf8NoBom)

foreach ($line in Get-Content -LiteralPath $checksumsPath -Encoding UTF8) {
  if ($line -notmatch '^([0-9a-f]{64}) \*(.+)$') { throw "Invalid bundle checksum line: $line" }
  $target = Join-Path $bundleRoot ($matches[2].Replace('/', '\'))
  if ((Get-Sha256 $target) -cne $matches[1]) { throw "Bundle checksum mismatch: $($matches[2])" }
}

New-Utf8Zip -SourceRoot $bundleRoot -ZipPath $zipPath
$plan.bundleSha256 = Get-Sha256 $zipPath
$plan.bundleFiles = @($checksumLines).Count
$plan | ConvertTo-Json -Depth 5
