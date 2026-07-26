[CmdletBinding()]
param(
  [string]$InstallerPath = "",
  [string]$PreviousInstallerPath = "",
  [string]$Version = "",
  [string]$RunId = "",
  [switch]$DryRun,
  [switch]$AllowExistingProductRegistration,
  [switch]$ExpectEmbeddedPreviewCredential,
  [int]$StartupObservationSeconds = 8,
  [int]$ProcessExitTimeoutSeconds = 20,
  [int]$InstallerTimeoutSeconds = 300
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$ProductName = "$([char]0x534A)$([char]0x5C71)$([char]0x5546)$([char]0x52A1)$([char]0x5DE5)$([char]0x4F5C)$([char]0x53F0)"
$AppIdentifier = "com.banshan.aigc.desktop"
$AppExeName = "bsaigc_desktop.exe"
$UninstallerName = "uninstall.exe"
$UninstallRegistryBase = "Software\Microsoft\Windows\CurrentVersion\Uninstall"
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$RuntimeRoot = [System.IO.Path]::GetFullPath((Join-Path $RepoRoot ".runtime"))

if ([string]::IsNullOrWhiteSpace($Version)) {
  $Version = [string](Get-Content -LiteralPath (Join-Path $RepoRoot "package.json") -Raw -Encoding UTF8 | ConvertFrom-Json).version
}

if ([string]::IsNullOrWhiteSpace($RunId)) {
  $RunId = "release-$Version-" + (Get-Date -Format "yyyyMMdd-HHmmss")
}
if ($RunId -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$') {
  throw "RunId $([char]0x53EA)$([char]0x80FD)$([char]0x5305)$([char]0x542B)$([char]0x5B57)$([char]0x6BCD)$([char]0x3001)$([char]0x6570)$([char]0x5B57)$([char]0x3001)$([char]0x70B9)$([char]0x3001)$([char]0x4E0B)$([char]0x5212)$([char]0x7EBF)$([char]0x548C)$([char]0x8FDE)$([char]0x5B57)$([char]0x7B26)$([char]0xFF0C)$([char]0x957F)$([char]0x5EA6)$([char]0x4E0D)$([char]0x5F97)$([char]0x8D85)$([char]0x8FC7) 64$([char]0x3002)"
}
if ($StartupObservationSeconds -lt 3) {
  throw "StartupObservationSeconds $([char]0x4E0D)$([char]0x5F97)$([char]0x5C0F)$([char]0x4E8E) 3$([char]0x3002)"
}
if ($ProcessExitTimeoutSeconds -lt 5) {
  throw "ProcessExitTimeoutSeconds $([char]0x4E0D)$([char]0x5F97)$([char]0x5C0F)$([char]0x4E8E) 5$([char]0x3002)"
}
if ($InstallerTimeoutSeconds -lt 30) {
  throw "InstallerTimeoutSeconds $([char]0x4E0D)$([char]0x5F97)$([char]0x5C0F)$([char]0x4E8E) 30$([char]0x3002)"
}

$RunRoot = [System.IO.Path]::GetFullPath((Join-Path $RuntimeRoot "nsis-acceptance\$RunId"))
$InstallRoot = [System.IO.Path]::GetFullPath((Join-Path $RunRoot "install"))
$ProfileRoot = [System.IO.Path]::GetFullPath((Join-Path $RunRoot "profile"))
$RoamingRoot = [System.IO.Path]::GetFullPath((Join-Path $ProfileRoot "AppData\Roaming"))
$LocalRoot = [System.IO.Path]::GetFullPath((Join-Path $ProfileRoot "AppData\Local"))
$TempRoot = [System.IO.Path]::GetFullPath((Join-Path $ProfileRoot "Temp"))
$DataRoot = [System.IO.Path]::GetFullPath((Join-Path $RoamingRoot $AppIdentifier))
$CredentialProbeProfileRoot = [System.IO.Path]::GetFullPath((Join-Path $RunRoot "credential-probe-profile"))
$CredentialProbeRoamingRoot = [System.IO.Path]::GetFullPath((Join-Path $CredentialProbeProfileRoot "AppData\Roaming"))
$CredentialProbeLocalRoot = [System.IO.Path]::GetFullPath((Join-Path $CredentialProbeProfileRoot "AppData\Local"))
$CredentialProbeTempRoot = [System.IO.Path]::GetFullPath((Join-Path $CredentialProbeProfileRoot "Temp"))
$CredentialProbeDataRoot = [System.IO.Path]::GetFullPath((Join-Path $CredentialProbeRoamingRoot $AppIdentifier))
$CredentialProbeStatePath = [System.IO.Path]::GetFullPath((Join-Path $CredentialProbeDataRoot "credentials\provider-key.dpapi"))
$CredentialVerifierPath = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "verify-embedded-preview-credential.ps1"))
$ExpectedPreviewProviderId = "bsaigc"
$ExpectedPreviewBaseUrl = "https://bsaigc.dpdns.org/v1"
$ExpectedPreviewModel = "gpt-5.6-sol"
$ExpectedCredentialSchemaVersion = 2
$LogRoot = [System.IO.Path]::GetFullPath((Join-Path $RunRoot "logs"))
$LogPath = [System.IO.Path]::GetFullPath((Join-Path $LogRoot "acceptance.log"))
$SummaryPath = [System.IO.Path]::GetFullPath((Join-Path $RunRoot "acceptance-summary.json"))
$RunMarkerPath = [System.IO.Path]::GetFullPath((Join-Path $RunRoot "run-marker.json"))
$RegistryBackupRoot = [System.IO.Path]::GetFullPath((Join-Path $RunRoot "registry-backup"))
$ExpectedInstallerName = "${ProductName}_${Version}_x64-setup.exe"
$AppPath = [System.IO.Path]::GetFullPath((Join-Path $InstallRoot $AppExeName))
$UninstallerPath = [System.IO.Path]::GetFullPath((Join-Path $InstallRoot $UninstallerName))

$script:LogFileEnabled = $false
$script:StartedProcesses = New-Object System.Collections.Generic.List[int]
$script:BaselineRegistryExports = New-Object System.Collections.Generic.List[object]
$script:InstalledByThisRun = $false
$script:UninstallCompleted = $false
$script:RegistryRestored = $false
$script:AcceptanceError = $null
$script:FinalInstallerSha256 = $null
$script:PreviousInstallerSha256 = $null
$script:InitialProductVersion = $null
$script:FinalProductVersion = $null
$script:UpgradeKind = "unknown"
$script:Steps = New-Object System.Collections.Generic.List[object]
$script:CredentialProbeResult = $null

function Test-IsDescendantPath {
  param(
    [Parameter(Mandatory = $true)][string]$Candidate,
    [Parameter(Mandatory = $true)][string]$Parent
  )

  $candidateFull = [System.IO.Path]::GetFullPath($Candidate).TrimEnd('\')
  $parentFull = [System.IO.Path]::GetFullPath($Parent).TrimEnd('\')
  $prefix = $parentFull + [System.IO.Path]::DirectorySeparatorChar
  return $candidateFull.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)
}

function Assert-NoExistingReparsePoint {
  param(
    [Parameter(Mandatory = $true)][string]$Candidate,
    [Parameter(Mandatory = $true)][string]$StopAt
  )

  $current = [System.IO.Path]::GetFullPath($Candidate).TrimEnd('\')
  $stop = [System.IO.Path]::GetFullPath($StopAt).TrimEnd('\')
  while ($current.Length -ge $stop.Length) {
    if (Test-Path -LiteralPath $current) {
      $item = Get-Item -LiteralPath $current -Force
      if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "$([char]0x62D2)$([char]0x7EDD)$([char]0x4F7F)$([char]0x7528)$([char]0x91CD)$([char]0x89E3)$([char]0x6790)$([char]0x70B9)$([char]0x8DEF)$([char]0x5F84)$([char]0xFF1A)$current"
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

function Assert-RuntimeTargetPath {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Label
  )

  $full = [System.IO.Path]::GetFullPath($Path)
  if (-not (Test-IsDescendantPath -Candidate $full -Parent $RuntimeRoot)) {
    throw "$Label $([char]0x5FC5)$([char]0x987B)$([char]0x4E25)$([char]0x683C)$([char]0x4F4D)$([char]0x4E8E) $RuntimeRoot $([char]0x4E0B)$([char]0xFF0C)$([char]0x5B9E)$([char]0x9645)$([char]0x4E3A)$([char]0xFF1A)$full"
  }
  Assert-NoExistingReparsePoint -Candidate $full -StopAt $RuntimeRoot
}

foreach ($target in @(
  @{ Path = $RunRoot; Label = "$([char]0x8FD0)$([char]0x884C)$([char]0x76EE)$([char]0x5F55)" },
  @{ Path = $InstallRoot; Label = "$([char]0x6D4B)$([char]0x8BD5)$([char]0x5B89)$([char]0x88C5)$([char]0x76EE)$([char]0x5F55)" },
  @{ Path = $ProfileRoot; Label = "$([char]0x9694)$([char]0x79BB)$([char]0x7528)$([char]0x6237)$([char]0x76EE)$([char]0x5F55)" },
  @{ Path = $DataRoot; Label = "$([char]0x9694)$([char]0x79BB)$([char]0x4E1A)$([char]0x52A1)$([char]0x6570)$([char]0x636E)$([char]0x76EE)$([char]0x5F55)" },
  @{ Path = $CredentialProbeProfileRoot; Label = "credential probe profile" },
  @{ Path = $CredentialProbeDataRoot; Label = "credential probe data root" },
  @{ Path = $CredentialProbeStatePath; Label = "credential probe state" },
  @{ Path = $LogRoot; Label = "$([char]0x65E5)$([char]0x5FD7)$([char]0x76EE)$([char]0x5F55)" },
  @{ Path = $RegistryBackupRoot; Label = "$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x5907)$([char]0x4EFD)$([char]0x76EE)$([char]0x5F55)" }
)) {
  Assert-RuntimeTargetPath -Path $target.Path -Label $target.Label
}

function Write-Log {
  param(
    [Parameter(Mandatory = $true)][ValidateSet("INFO", "WARN", "ERROR", "PASS", "DRYRUN")][string]$Level,
    [Parameter(Mandatory = $true)][string]$Message
  )

  $line = "{0} [{1}] {2}" -f (Get-Date -Format "yyyy-MM-dd HH:mm:ss.fff"), $Level, $Message
  Write-Host $line
  if ($script:LogFileEnabled) {
    [System.IO.File]::AppendAllText($LogPath, $line + [Environment]::NewLine, $Utf8NoBom)
  }
}

function Add-StepResult {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][ValidateSet("passed", "failed", "planned", "warning")][string]$Status,
    [Parameter(Mandatory = $true)][string]$Detail
  )

  $script:Steps.Add([ordered]@{
    name = $Name
    status = $Status
    detail = $Detail
    recordedAt = (Get-Date).ToString("o")
  })
}

function Write-JsonNoBom {
  param(
    [Parameter(Mandatory = $true)]$Value,
    [Parameter(Mandatory = $true)][string]$Path,
    [int]$Depth = 12
  )

  Assert-RuntimeTargetPath -Path $Path -Label "JSON $([char]0x8F93)$([char]0x51FA)"
  $json = $Value | ConvertTo-Json -Depth $Depth
  [System.IO.File]::WriteAllText($Path, $json + [Environment]::NewLine, $Utf8NoBom)
}

function Resolve-SourceFile {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Label
  )

  if ([string]::IsNullOrWhiteSpace($Path)) {
    throw "$Label $([char]0x8DEF)$([char]0x5F84)$([char]0x4E3A)$([char]0x7A7A)$([char]0x3002)"
  }
  $candidate = $Path
  if (-not [System.IO.Path]::IsPathRooted($candidate)) {
    $candidate = Join-Path $RepoRoot $candidate
  }
  $full = [System.IO.Path]::GetFullPath($candidate)
  if (-not (Test-Path -LiteralPath $full -PathType Leaf)) {
    throw "$Label $([char]0x4E0D)$([char]0x5B58)$([char]0x5728)$([char]0xFF1A)$full"
  }
  return $full
}

function Get-FileSha256Lower {
  param([Parameter(Mandatory = $true)][string]$Path)
  return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Invoke-HiddenProcessAndWait {
  param(
    [Parameter(Mandatory = $true)][string]$FilePath,
    [string]$Arguments = "",
    [Parameter(Mandatory = $true)][int]$TimeoutSeconds,
    [Parameter(Mandatory = $true)][string]$Label
  )

  Write-Log -Level INFO -Message "$Label$([char]0xFF1A)$FilePath $Arguments"
  $startInfo = New-Object System.Diagnostics.ProcessStartInfo
  $startInfo.FileName = $FilePath
  $startInfo.Arguments = $Arguments
  $startInfo.UseShellExecute = $false
  $startInfo.CreateNoWindow = $true
  $startInfo.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden
  $process = New-Object System.Diagnostics.Process
  $process.StartInfo = $startInfo
  if (-not $process.Start()) {
    throw "$Label $([char]0x542F)$([char]0x52A8)$([char]0x5931)$([char]0x8D25)$([char]0x3002)"
  }
  if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
    try { $process.Kill() } catch { }
    throw "$Label $([char]0x8D85)$([char]0x65F6)$([char]0xFF08)$TimeoutSeconds $([char]0x79D2)$([char]0xFF09)$([char]0x3002)"
  }
  if ($process.ExitCode -ne 0) {
    throw "$Label $([char]0x9000)$([char]0x51FA)$([char]0x7801)$([char]0x4E3A) $($process.ExitCode)$([char]0x3002)"
  }
  Write-Log -Level PASS -Message "$Label $([char]0x5B8C)$([char]0x6210)$([char]0xFF0C)$([char]0x9000)$([char]0x51FA)$([char]0x7801) 0$([char]0x3002)"
}

function Normalize-RegistryInstallLocation {
  param([AllowNull()][string]$Value)
  if ([string]::IsNullOrWhiteSpace($Value)) {
    return ""
  }
  $trimmed = $Value.Trim().Trim('"').TrimEnd('\')
  try {
    return [System.IO.Path]::GetFullPath($trimmed).TrimEnd('\')
  } catch {
    return $trimmed
  }
}

function Get-UninstallRegistryEntries {
  $entries = New-Object System.Collections.Generic.List[object]
  $baseKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($UninstallRegistryBase)
  if ($null -eq $baseKey) {
    return @()
  }
  try {
    foreach ($name in $baseKey.GetSubKeyNames()) {
      $subKey = $baseKey.OpenSubKey($name)
      if ($null -eq $subKey) {
        continue
      }
      try {
        $entries.Add([pscustomobject]@{
          subKeyName = $name
          registryPath = "HKCU\$UninstallRegistryBase\$name"
          displayName = [string]$subKey.GetValue("DisplayName", "")
          displayVersion = [string]$subKey.GetValue("DisplayVersion", "")
          installLocation = [string]$subKey.GetValue("InstallLocation", "")
          uninstallString = [string]$subKey.GetValue("UninstallString", "")
        })
      } finally {
        $subKey.Dispose()
      }
    }
  } finally {
    $baseKey.Dispose()
  }
  return $entries.ToArray()
}

function Test-RegistryEntryTargetsInstallRoot {
  param([Parameter(Mandatory = $true)]$Entry)

  $normalizedTarget = $InstallRoot.TrimEnd('\')
  $normalizedLocation = Normalize-RegistryInstallLocation -Value $Entry.installLocation
  if ($normalizedLocation.Equals($normalizedTarget, [System.StringComparison]::OrdinalIgnoreCase)) {
    return $true
  }
  if (-not [string]::IsNullOrWhiteSpace($Entry.uninstallString)) {
    $uninstall = $Entry.uninstallString.Trim().Trim('"')
    if ($uninstall.StartsWith($normalizedTarget + "\", [System.StringComparison]::OrdinalIgnoreCase)) {
      return $true
    }
  }
  return $false
}

function Get-ProductRegistryEntries {
  return @(Get-UninstallRegistryEntries | Where-Object {
    $_.displayName -eq $ProductName -or $_.subKeyName -eq $ProductName
  })
}

function Export-BaselineRegistryEntries {
  param([object[]]$Entries)

  if ($Entries.Count -eq 0) {
    return
  }
  New-Item -ItemType Directory -Path $RegistryBackupRoot -Force | Out-Null
  $index = 0
  foreach ($entry in $Entries) {
    $index++
    $backupPath = Join-Path $RegistryBackupRoot ("baseline-{0:D2}.reg" -f $index)
    Assert-RuntimeTargetPath -Path $backupPath -Label "$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x5907)$([char]0x4EFD)$([char]0x6587)$([char]0x4EF6)"
    Write-Log -Level INFO -Message "$([char]0x5907)$([char]0x4EFD)$([char]0x65E2)$([char]0x6709)$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x9879)$([char]0xFF1A)$($entry.registryPath)"
    & reg.exe export $entry.registryPath $backupPath /y | Out-Null
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $backupPath -PathType Leaf)) {
      throw "$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x5907)$([char]0x4EFD)$([char]0x5931)$([char]0x8D25)$([char]0xFF1A)$($entry.registryPath)"
    }
    $script:BaselineRegistryExports.Add([pscustomobject]@{
      registryPath = $entry.registryPath
      backupPath = $backupPath
      installLocation = $entry.installLocation
      displayVersion = $entry.displayVersion
    })
  }
}

function Remove-TestRegistryEntries {
  $testEntries = @(Get-UninstallRegistryEntries | Where-Object { Test-RegistryEntryTargetsInstallRoot -Entry $_ })
  foreach ($entry in $testEntries) {
    $registryProviderPath = "Registry::HKEY_CURRENT_USER\$UninstallRegistryBase\$($entry.subKeyName)"
    if (-not (Test-RegistryEntryTargetsInstallRoot -Entry $entry)) {
      throw "$([char]0x62D2)$([char]0x7EDD)$([char]0x6E05)$([char]0x7406)$([char]0x672A)$([char]0x660E)$([char]0x786E)$([char]0x6307)$([char]0x5411)$([char]0x6D4B)$([char]0x8BD5)$([char]0x5B89)$([char]0x88C5)$([char]0x76EE)$([char]0x5F55)$([char]0x7684)$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x9879)$([char]0xFF1A)$($entry.registryPath)"
    }
    Write-Log -Level WARN -Message "$([char]0x6E05)$([char]0x7406)$([char]0x6D4B)$([char]0x8BD5)$([char]0x6B8B)$([char]0x7559)$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x9879)$([char]0xFF1A)$($entry.registryPath)"
    Remove-Item -LiteralPath $registryProviderPath -Recurse -Force
  }
  $remaining = @(Get-UninstallRegistryEntries | Where-Object { Test-RegistryEntryTargetsInstallRoot -Entry $_ })
  if ($remaining.Count -gt 0) {
    throw "$([char]0x6D4B)$([char]0x8BD5)$([char]0x5B89)$([char]0x88C5)$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x9879)$([char]0x6E05)$([char]0x7406)$([char]0x5931)$([char]0x8D25)$([char]0x3002)"
  }
}

function Restore-BaselineRegistryEntries {
  if ($script:RegistryRestored) {
    return
  }
  foreach ($export in $script:BaselineRegistryExports) {
    Write-Log -Level INFO -Message "$([char]0x6062)$([char]0x590D)$([char]0x65E2)$([char]0x6709)$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x9879)$([char]0xFF1A)$($export.registryPath)"
    & reg.exe import $export.backupPath | Out-Null
    if ($LASTEXITCODE -ne 0) {
      throw "$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x6062)$([char]0x590D)$([char]0x5931)$([char]0x8D25)$([char]0xFF1A)$($export.registryPath)"
    }
  }
  $script:RegistryRestored = $true
}

function Wait-ForFile {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][int]$TimeoutSeconds,
    [Parameter(Mandatory = $true)][string]$Label
  )

  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  while ((Get-Date) -lt $deadline) {
    if (Test-Path -LiteralPath $Path -PathType Leaf) {
      return
    }
    Start-Sleep -Milliseconds 250
  }
  throw "$Label $([char]0x672A)$([char]0x5728) $TimeoutSeconds $([char]0x79D2)$([char]0x5185)$([char]0x51FA)$([char]0x73B0)$([char]0xFF1A)$Path"
}

function Wait-ForPathRemoval {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][int]$TimeoutSeconds,
    [Parameter(Mandatory = $true)][string]$Label
  )

  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  while ((Get-Date) -lt $deadline) {
    if (-not (Test-Path -LiteralPath $Path)) {
      return
    }
    Start-Sleep -Milliseconds 500
  }
  throw "$Label $([char]0x672A)$([char]0x5728) $TimeoutSeconds $([char]0x79D2)$([char]0x5185)$([char]0x5220)$([char]0x9664)$([char]0xFF1A)$Path"
}

function Invoke-NsisInstall {
  param(
    [Parameter(Mandatory = $true)][string]$SourceInstaller,
    [Parameter(Mandatory = $true)][string]$Label
  )

  Assert-RuntimeTargetPath -Path $InstallRoot -Label "NSIS $([char]0x5B89)$([char]0x88C5)$([char]0x76EE)$([char]0x6807)"
  $arguments = "/S /D=$InstallRoot"
  Invoke-HiddenProcessAndWait -FilePath $SourceInstaller -Arguments $arguments -TimeoutSeconds $InstallerTimeoutSeconds -Label $Label
  $script:InstalledByThisRun = $true
  Wait-ForFile -Path $AppPath -TimeoutSeconds 30 -Label "$([char]0x4E3B)$([char]0x7A0B)$([char]0x5E8F)"
  Wait-ForFile -Path $UninstallerPath -TimeoutSeconds 30 -Label "$([char]0x5378)$([char]0x8F7D)$([char]0x7A0B)$([char]0x5E8F)"
}

function Assert-InstalledAppIdentity {
  param([Parameter(Mandatory = $true)][string]$ExpectedVersion)

  if (-not (Test-Path -LiteralPath $AppPath -PathType Leaf)) {
    throw "$([char]0x4E3B)$([char]0x7A0B)$([char]0x5E8F)$([char]0x4E0D)$([char]0x5B58)$([char]0x5728)$([char]0xFF1A)$AppPath"
  }
  $versionInfo = (Get-Item -LiteralPath $AppPath).VersionInfo
  if ($versionInfo.ProductName -ne $ProductName) {
    throw "$([char]0x5B89)$([char]0x88C5)$([char]0x540E)$([char]0x7684) ProductName $([char]0x4E0D)$([char]0x5339)$([char]0x914D)$([char]0x3002)$([char]0x9884)$([char]0x671F) $ProductName$([char]0xFF0C)$([char]0x5B9E)$([char]0x9645) $($versionInfo.ProductName)$([char]0x3002)"
  }
  if ($versionInfo.ProductVersion -ne $ExpectedVersion) {
    throw "$([char]0x5B89)$([char]0x88C5)$([char]0x540E)$([char]0x7684) ProductVersion $([char]0x4E0D)$([char]0x5339)$([char]0x914D)$([char]0x3002)$([char]0x9884)$([char]0x671F) $ExpectedVersion$([char]0xFF0C)$([char]0x5B9E)$([char]0x9645) $($versionInfo.ProductVersion)$([char]0x3002)"
  }
  return $versionInfo.ProductVersion
}

function Assert-CodexLegalFiles {
  $codexRoot = Join-Path $InstallRoot "resources\codex-runtime"
  $manifestPath = Join-Path $codexRoot "manifest.json"
  if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "$([char]0x5B89)$([char]0x88C5)$([char]0x76EE)$([char]0x5F55)$([char]0x7F3A)$([char]0x5C11) Codex manifest$([char]0xFF1A)$manifestPath"
  }
  $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
  if ($manifest.license.licenseFile -ne "LICENSE" -or $manifest.license.noticeFile -ne "NOTICE") {
    throw "Codex manifest $([char]0x7684) LICENSE/NOTICE $([char]0x58F0)$([char]0x660E)$([char]0x4E0D)$([char]0x6B63)$([char]0x786E)$([char]0x3002)"
  }
  foreach ($required in @("LICENSE", "NOTICE")) {
    $filePath = Join-Path $codexRoot $required
    if (-not (Test-Path -LiteralPath $filePath -PathType Leaf)) {
      throw "$([char]0x5B89)$([char]0x88C5)$([char]0x76EE)$([char]0x5F55)$([char]0x7F3A)$([char]0x5C11) Codex $required$([char]0xFF1A)$filePath"
    }
    $record = @($manifest.files | Where-Object { $_.path -eq $required })
    if ($record.Count -ne 1) {
      throw "Codex manifest $([char]0x5FC5)$([char]0x987B)$([char]0x4E14)$([char]0x53EA)$([char]0x80FD)$([char]0x5305)$([char]0x542B)$([char]0x4E00)$([char]0x6761) $required $([char]0x8BB0)$([char]0x5F55)$([char]0x3002)"
    }
    $actualLength = (Get-Item -LiteralPath $filePath).Length
    $actualSha = Get-FileSha256Lower -Path $filePath
    if ([int64]$record[0].sizeBytes -ne $actualLength) {
      throw "Codex $required $([char]0x5927)$([char]0x5C0F)$([char]0x6821)$([char]0x9A8C)$([char]0x5931)$([char]0x8D25)$([char]0x3002)"
    }
    if ([string]$record[0].sha256 -ne $actualSha) {
      throw "Codex $required SHA-256 $([char]0x6821)$([char]0x9A8C)$([char]0x5931)$([char]0x8D25)$([char]0x3002)"
    }
    Write-Log -Level PASS -Message "Codex $required $([char]0x5DF2)$([char]0x5B89)$([char]0x88C5)$([char]0x4E14) Manifest $([char]0x5927)$([char]0x5C0F)/SHA-256 $([char]0x6821)$([char]0x9A8C)$([char]0x901A)$([char]0x8FC7)$([char]0x3002)"
  }
}

function Initialize-IsolatedProfileLayout {
  param(
    [Parameter(Mandatory = $true)][string]$ProfileRootPath,
    [Parameter(Mandatory = $true)][string]$RoamingRootPath,
    [Parameter(Mandatory = $true)][string]$LocalRootPath,
    [Parameter(Mandatory = $true)][string]$TempRootPath,
    [Parameter(Mandatory = $true)][string]$Label
  )

  foreach ($path in @($ProfileRootPath, $RoamingRootPath, $LocalRootPath, $TempRootPath)) {
    Assert-RuntimeTargetPath -Path $path -Label $Label
    New-Item -ItemType Directory -Path $path -Force | Out-Null
  }
}

function Initialize-IsolatedProfile {
  Initialize-IsolatedProfileLayout `
    -ProfileRootPath $ProfileRoot `
    -RoamingRootPath $RoamingRoot `
    -LocalRootPath $LocalRoot `
    -TempRootPath $TempRoot `
    -Label "$([char]0x9694)$([char]0x79BB) Profile $([char]0x8DEF)$([char]0x5F84)"
}

function Initialize-CredentialProbeProfile {
  Initialize-IsolatedProfileLayout `
    -ProfileRootPath $CredentialProbeProfileRoot `
    -RoamingRootPath $CredentialProbeRoamingRoot `
    -LocalRootPath $CredentialProbeLocalRoot `
    -TempRootPath $CredentialProbeTempRoot `
    -Label "credential probe profile path"
}

function Start-IsolatedApplicationForProfile {
  param(
    [Parameter(Mandatory = $true)][string]$Label,
    [Parameter(Mandatory = $true)][string]$ProfileRootPath,
    [Parameter(Mandatory = $true)][string]$RoamingRootPath,
    [Parameter(Mandatory = $true)][string]$LocalRootPath,
    [Parameter(Mandatory = $true)][string]$TempRootPath,
    [Parameter(Mandatory = $true)][string]$DataRootPath
  )

  if (-not (Test-Path -LiteralPath $AppPath -PathType Leaf)) {
    throw "$Label $([char]0x65E0)$([char]0x6CD5)$([char]0x542F)$([char]0x52A8)$([char]0xFF0C)$([char]0x4E3B)$([char]0x7A0B)$([char]0x5E8F)$([char]0x4E0D)$([char]0x5B58)$([char]0x5728)$([char]0xFF1A)$AppPath"
  }
  foreach ($path in @($ProfileRootPath, $RoamingRootPath, $LocalRootPath, $TempRootPath, $DataRootPath)) {
    Assert-RuntimeTargetPath -Path $path -Label "isolated application profile path"
  }

  $startInfo = New-Object System.Diagnostics.ProcessStartInfo
  $startInfo.FileName = $AppPath
  $startInfo.WorkingDirectory = $InstallRoot
  $startInfo.UseShellExecute = $false
  $startInfo.CreateNoWindow = $true
  $startInfo.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden
  $startInfo.EnvironmentVariables["USERPROFILE"] = $ProfileRootPath
  $startInfo.EnvironmentVariables["HOME"] = $ProfileRootPath
  $startInfo.EnvironmentVariables["APPDATA"] = $RoamingRootPath
  $startInfo.EnvironmentVariables["LOCALAPPDATA"] = $LocalRootPath
  $startInfo.EnvironmentVariables["TEMP"] = $TempRootPath
  $startInfo.EnvironmentVariables["TMP"] = $TempRootPath
  $startInfo.EnvironmentVariables["CODEX_HOME"] = (Join-Path $DataRootPath "codex-home")

  $process = New-Object System.Diagnostics.Process
  $process.StartInfo = $startInfo
  Write-Log -Level INFO -Message "$Label$([char]0xFF1A)$([char]0x4EE5)$([char]0x9694)$([char]0x79BB) Profile $([char]0x542F)$([char]0x52A8) $AppPath"
  if (-not $process.Start()) {
    throw "$Label $([char]0x542F)$([char]0x52A8)$([char]0x5931)$([char]0x8D25)$([char]0x3002)"
  }
  $script:StartedProcesses.Add($process.Id)
  Start-Sleep -Seconds $StartupObservationSeconds
  if ($process.HasExited) {
    throw "$Label $([char]0x5728)$([char]0x89C2)$([char]0x5BDF)$([char]0x671F)$([char]0x5185)$([char]0x9000)$([char]0x51FA)$([char]0xFF0C)$([char]0x9000)$([char]0x51FA)$([char]0x7801) $($process.ExitCode)$([char]0x3002)"
  }
  Write-Log -Level PASS -Message "$Label $([char]0x4FDD)$([char]0x6301)$([char]0x8FD0)$([char]0x884C) $StartupObservationSeconds $([char]0x79D2)$([char]0xFF0C)PID=$($process.Id)$([char]0x3002)"
  return $process
}

function Start-IsolatedApplication {
  param([Parameter(Mandatory = $true)][string]$Label)

  return Start-IsolatedApplicationForProfile `
    -Label $Label `
    -ProfileRootPath $ProfileRoot `
    -RoamingRootPath $RoamingRoot `
    -LocalRootPath $LocalRoot `
    -TempRootPath $TempRoot `
    -DataRootPath $DataRoot
}
function Stop-TrackedApplication {
  param(
    [Parameter(Mandatory = $true)][System.Diagnostics.Process]$Process,
    [Parameter(Mandatory = $true)][string]$Label
  )

  if ($Process.HasExited) {
    Write-Log -Level INFO -Message "$Label $([char]0x5DF2)$([char]0x81EA)$([char]0x884C)$([char]0x9000)$([char]0x51FA)$([char]0x3002)"
    return
  }
  $closed = $false
  try { $closed = $Process.CloseMainWindow() } catch { $closed = $false }
  if ($closed -and $Process.WaitForExit($ProcessExitTimeoutSeconds * 1000)) {
    Write-Log -Level PASS -Message "$Label $([char]0x5DF2)$([char]0x6B63)$([char]0x5E38)$([char]0x9000)$([char]0x51FA)$([char]0x3002)"
    return
  }
  Write-Log -Level WARN -Message "$Label $([char]0x672A)$([char]0x5728)$([char]0x65F6)$([char]0x9650)$([char]0x5185)$([char]0x6B63)$([char]0x5E38)$([char]0x9000)$([char]0x51FA)$([char]0xFF0C)$([char]0x53EA)$([char]0x7EC8)$([char]0x6B62)$([char]0x672C)$([char]0x6B21)$([char]0x9A8C)$([char]0x6536)$([char]0x542F)$([char]0x52A8)$([char]0x7684) PID=$($Process.Id)$([char]0x3002)"
  try { $Process.Kill() } catch { }
  if (-not $Process.WaitForExit($ProcessExitTimeoutSeconds * 1000)) {
    throw "$Label $([char]0x65E0)$([char]0x6CD5)$([char]0x7EC8)$([char]0x6B62) PID=$($Process.Id)$([char]0x3002)"
  }
}

function Stop-TestInstallProcesses {
  $normalizedInstallRoot = $InstallRoot.TrimEnd('\') + "\"
  $processes = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object {
    -not [string]::IsNullOrWhiteSpace($_.ExecutablePath) -and
    $_.ExecutablePath.StartsWith($normalizedInstallRoot, [System.StringComparison]::OrdinalIgnoreCase)
  })
  foreach ($process in $processes) {
    $processPath = [System.IO.Path]::GetFullPath($process.ExecutablePath)
    if (-not (Test-IsDescendantPath -Candidate $processPath -Parent $InstallRoot)) {
      throw "$([char]0x62D2)$([char]0x7EDD)$([char]0x7EC8)$([char]0x6B62)$([char]0x6D4B)$([char]0x8BD5)$([char]0x5B89)$([char]0x88C5)$([char]0x76EE)$([char]0x5F55)$([char]0x5916)$([char]0x7684)$([char]0x8FDB)$([char]0x7A0B)$([char]0xFF1A)$processPath"
    }
    Write-Log -Level WARN -Message "$([char]0x7EC8)$([char]0x6B62)$([char]0x6D4B)$([char]0x8BD5)$([char]0x5B89)$([char]0x88C5)$([char]0x76EE)$([char]0x5F55)$([char]0x6B8B)$([char]0x7559)$([char]0x8FDB)$([char]0x7A0B) PID=$($process.ProcessId)$([char]0xFF1A)$processPath"
    Stop-Process -Id $process.ProcessId -Force -ErrorAction Stop
  }
}

function Invoke-FinalCandidateCredentialProbe {
  if (-not (Test-Path -LiteralPath $CredentialVerifierPath -PathType Leaf)) {
    throw "Credential probe failed: verifier script is missing."
  }

  Initialize-CredentialProbeProfile
  if (Test-Path -LiteralPath $CredentialProbeStatePath) {
    throw "Credential probe failed: protected state existed before final-candidate first start."
  }

  $probeProcess = $null
  try {
    $probeProcess = Start-IsolatedApplicationForProfile `
      -Label "final candidate credential probe" `
      -ProfileRootPath $CredentialProbeProfileRoot `
      -RoamingRootPath $CredentialProbeRoamingRoot `
      -LocalRootPath $CredentialProbeLocalRoot `
      -TempRootPath $CredentialProbeTempRoot `
      -DataRootPath $CredentialProbeDataRoot
    Wait-ForFile -Path $CredentialProbeStatePath -TimeoutSeconds 30 -Label "final candidate protected credential state"
  } finally {
    if ($null -ne $probeProcess) {
      Stop-TrackedApplication -Process $probeProcess -Label "final candidate credential probe process"
    }
    Stop-TestInstallProcesses
  }

  $probeOutput = @(& $CredentialVerifierPath `
    -StatePath $CredentialProbeStatePath `
    -ExpectedProviderId $ExpectedPreviewProviderId `
    -ExpectedBaseUrl $ExpectedPreviewBaseUrl `
    -ExpectedModel $ExpectedPreviewModel `
    -ExpectedSchemaVersion $ExpectedCredentialSchemaVersion)
  if ($probeOutput.Count -ne 1) {
    throw "Credential probe failed: verifier returned an unexpected result."
  }
  $result = $probeOutput[0]
  foreach ($requiredFlag in @(
    "decrypted",
    "schemaValid",
    "defaultProviderValid",
    "providerEnabled",
    "apiKeyConfigured",
    "httpsBaseUrl",
    "baseUrlMatches",
    "defaultModelConfigured",
    "defaultModelMatches",
    "defaultModelListed"
  )) {
    $property = $result.PSObject.Properties[$requiredFlag]
    if ($null -eq $property -or -not [bool]$property.Value) {
      throw "Credential probe failed: one or more safe assertions did not pass."
    }
  }
  if ([int]$result.schemaVersion -ne $ExpectedCredentialSchemaVersion -or
      [string]$result.providerId -cne $ExpectedPreviewProviderId) {
    throw "Credential probe failed: safe identity assertions did not pass."
  }

  $script:CredentialProbeResult = [ordered]@{
    schemaVersion = [int]$result.schemaVersion
    providerId = [string]$result.providerId
    decrypted = [bool]$result.decrypted
    schemaValid = [bool]$result.schemaValid
    defaultProviderValid = [bool]$result.defaultProviderValid
    providerEnabled = [bool]$result.providerEnabled
    apiKeyConfigured = [bool]$result.apiKeyConfigured
    httpsBaseUrl = [bool]$result.httpsBaseUrl
    baseUrlMatches = [bool]$result.baseUrlMatches
    defaultModelConfigured = [bool]$result.defaultModelConfigured
    defaultModelMatches = [bool]$result.defaultModelMatches
    defaultModelListed = [bool]$result.defaultModelListed
  }
  Add-StepResult -Name "preview-credential-bootstrap" -Status "passed" -Detail "The final candidate created and decrypted a valid DPAPI-protected internal-preview provider state in a fresh probe profile."
}
function Assert-AuthoritativeDataCreated {
  $ledgerPath = Join-Path $DataRoot "ledger\bsaigc.sqlite3"
  $vaultPath = Join-Path $DataRoot "vault"
  if (-not (Test-Path -LiteralPath $ledgerPath -PathType Leaf)) {
    throw "$([char]0x9694)$([char]0x79BB) Profile $([char]0x672A)$([char]0x751F)$([char]0x6210) SQLite Ledger$([char]0xFF1A)$ledgerPath"
  }
  if (-not (Test-Path -LiteralPath $vaultPath -PathType Container)) {
    throw "$([char]0x9694)$([char]0x79BB) Profile $([char]0x672A)$([char]0x751F)$([char]0x6210) Local Vault$([char]0xFF1A)$vaultPath"
  }
  Write-Log -Level PASS -Message "SQLite Ledger $([char]0x4E0E) Local Vault $([char]0x5DF2)$([char]0x5728)$([char]0x9694)$([char]0x79BB) Profile $([char]0x4E2D)$([char]0x5EFA)$([char]0x7ACB)$([char]0x3002)"
}

function New-PreservationSentinels {
  $ledgerSentinel = Join-Path $DataRoot "ledger\.nsis-acceptance-ledger-sentinel.json"
  $vaultSentinel = Join-Path $DataRoot "vault\.nsis-acceptance-vault-sentinel.txt"
  foreach ($path in @($ledgerSentinel, $vaultSentinel)) {
    Assert-RuntimeTargetPath -Path $path -Label "$([char]0x6570)$([char]0x636E)$([char]0x4FDD)$([char]0x7559)$([char]0x54E8)$([char]0x5175)"
  }
  $ledgerPayload = [ordered]@{
    runId = $RunId
    purpose = "verify SQLite authority directory survives upgrade and uninstall"
    createdAt = (Get-Date).ToString("o")
  }
  Write-JsonNoBom -Value $ledgerPayload -Path $ledgerSentinel
  [System.IO.File]::WriteAllText($vaultSentinel, "runId=$RunId`npurpose=verify Local Vault survives upgrade and uninstall`n", $Utf8NoBom)
  return @($ledgerSentinel, $vaultSentinel)
}

function Get-AuthoritativeDataSnapshot {
  $records = New-Object System.Collections.Generic.List[object]
  foreach ($relativeRoot in @("ledger", "vault")) {
    $root = Join-Path $DataRoot $relativeRoot
    if (-not (Test-Path -LiteralPath $root -PathType Container)) {
      throw "$([char]0x6570)$([char]0x636E)$([char]0x5FEB)$([char]0x7167)$([char]0x76EE)$([char]0x5F55)$([char]0x4E0D)$([char]0x5B58)$([char]0x5728)$([char]0xFF1A)$root"
    }
    $files = @(Get-ChildItem -LiteralPath $root -Recurse -Force -File | Sort-Object FullName)
    foreach ($file in $files) {
      $records.Add([ordered]@{
        relativePath = $file.FullName.Substring($DataRoot.Length + 1).Replace('\', '/')
        length = $file.Length
        sha256 = Get-FileSha256Lower -Path $file.FullName
      })
    }
  }
  return $records.ToArray()
}

function Assert-SnapshotsEqual {
  param(
    [Parameter(Mandatory = $true)][object[]]$Before,
    [Parameter(Mandatory = $true)][object[]]$After,
    [Parameter(Mandatory = $true)][string]$Label
  )

  $beforeJson = $Before | ConvertTo-Json -Depth 5 -Compress
  $afterJson = $After | ConvertTo-Json -Depth 5 -Compress
  if ($beforeJson -ne $afterJson) {
    throw "$Label $([char]0x524D)$([char]0x540E) SQLite/Vault $([char]0x6587)$([char]0x4EF6)$([char]0x96C6)$([char]0x5408)$([char]0x3001)$([char]0x5927)$([char]0x5C0F)$([char]0x6216) SHA-256 $([char]0x4E0D)$([char]0x4E00)$([char]0x81F4)$([char]0x3002)"
  }
  Write-Log -Level PASS -Message "$Label $([char]0x672A)$([char]0x6539)$([char]0x5199)$([char]0x9694)$([char]0x79BB) SQLite/Vault $([char]0x6570)$([char]0x636E)$([char]0x3002)"
}

function Assert-SentinelsExist {
  param(
    [Parameter(Mandatory = $true)][string[]]$Paths,
    [Parameter(Mandatory = $true)][string]$Label
  )

  foreach ($path in $Paths) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
      throw "$Label $([char]0x540E)$([char]0x6570)$([char]0x636E)$([char]0x4FDD)$([char]0x7559)$([char]0x54E8)$([char]0x5175)$([char]0x4E22)$([char]0x5931)$([char]0xFF1A)$path"
    }
  }
  Write-Log -Level PASS -Message "$Label $([char]0x540E) SQLite/Vault $([char]0x6570)$([char]0x636E)$([char]0x4FDD)$([char]0x7559)$([char]0x54E8)$([char]0x5175)$([char]0x4ECD)$([char]0x5B58)$([char]0x5728)$([char]0x3002)"
}

function Invoke-TestUninstall {
  if ($script:UninstallCompleted) {
    return
  }
  Stop-TestInstallProcesses
  if (Test-Path -LiteralPath $UninstallerPath -PathType Leaf) {
    Invoke-HiddenProcessAndWait -FilePath $UninstallerPath -Arguments "/S" -TimeoutSeconds $InstallerTimeoutSeconds -Label "NSIS $([char]0x9759)$([char]0x9ED8)$([char]0x5378)$([char]0x8F7D)"
  } else {
    Write-Log -Level WARN -Message "$([char]0x5378)$([char]0x8F7D)$([char]0x7A0B)$([char]0x5E8F)$([char]0x4E0D)$([char]0x5B58)$([char]0x5728)$([char]0xFF0C)$([char]0x8DF3)$([char]0x8FC7)$([char]0x6267)$([char]0x884C)$([char]0xFF1A)$UninstallerPath"
  }
  Wait-ForPathRemoval -Path $AppPath -TimeoutSeconds 30 -Label "installed application"
  Wait-ForPathRemoval -Path $UninstallerPath -TimeoutSeconds 30 -Label "uninstaller"
  Remove-TestRegistryEntries
  $script:UninstallCompleted = $true
  Write-Log -Level PASS -Message "NSIS $([char]0x5378)$([char]0x8F7D)$([char]0x5B8C)$([char]0x6210)$([char]0xFF0C)$([char]0x6D4B)$([char]0x8BD5)$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x9879)$([char]0x5DF2)$([char]0x6E05)$([char]0x7406)$([char]0x3002)"
}

function Write-AcceptanceSummary {
  if ($DryRun -or -not (Test-Path -LiteralPath $RunRoot -PathType Container)) {
    return
  }
  $status = "passed"
  if ($null -ne $script:AcceptanceError) {
    $status = "failed"
  }
  $summary = [ordered]@{
    schemaVersion = 1
    productName = $ProductName
    version = $Version
    runId = $RunId
    status = $status
    dryRun = [bool]$DryRun
    startedAt = $script:StartedAt
    finishedAt = (Get-Date).ToString("o")
    installer = [ordered]@{
      path = $script:ResolvedInstallerPath
      sha256 = $script:FinalInstallerSha256
      authenticode = "NotSigned"
    }
    previousInstaller = if ([string]::IsNullOrWhiteSpace($script:ResolvedPreviousInstallerPath)) { $null } else { [ordered]@{ path = $script:ResolvedPreviousInstallerPath; sha256 = $script:PreviousInstallerSha256 } }
    upgradeKind = $script:UpgradeKind
    initialProductVersion = $script:InitialProductVersion
    finalProductVersion = $script:FinalProductVersion
    runtimeRoot = $RuntimeRoot
    runRoot = $RunRoot
    installRoot = $InstallRoot
    profileRoot = $ProfileRoot
    dataRoot = $DataRoot
    expectEmbeddedPreviewCredential = [bool]$ExpectEmbeddedPreviewCredential
    credentialProbe = $script:CredentialProbeResult
    uninstallCompleted = $script:UninstallCompleted
    registryRestored = $script:RegistryRestored
    error = if ($null -eq $script:AcceptanceError) { $null } else { $script:AcceptanceError.Exception.Message }
    steps = $script:Steps.ToArray()
  }
  Write-JsonNoBom -Value $summary -Path $SummaryPath -Depth 15
}

$script:StartedAt = (Get-Date).ToString("o")
$script:ResolvedInstallerPath = ""
$script:ResolvedPreviousInstallerPath = ""

try {
  if ([string]::IsNullOrWhiteSpace($InstallerPath)) {
    $InstallerPath = Join-Path $RepoRoot "src-tauri\target\release\bundle\nsis\$ExpectedInstallerName"
  }
  $script:ResolvedInstallerPath = Resolve-SourceFile -Path $InstallerPath -Label "$([char]0x6700)$([char]0x7EC8) NSIS $([char]0x5B89)$([char]0x88C5)$([char]0x5305)"
  if ((Split-Path -Leaf $script:ResolvedInstallerPath) -ne $ExpectedInstallerName) {
    throw "$([char]0x6700)$([char]0x7EC8)$([char]0x5B89)$([char]0x88C5)$([char]0x5305)$([char]0x6587)$([char]0x4EF6)$([char]0x540D)$([char]0x4E0D)$([char]0x6B63)$([char]0x786E)$([char]0x3002)$([char]0x9884)$([char]0x671F)$([char]0xFF1A)$ExpectedInstallerName"
  }
  $script:FinalInstallerSha256 = Get-FileSha256Lower -Path $script:ResolvedInstallerPath
  $signature = Get-AuthenticodeSignature -LiteralPath $script:ResolvedInstallerPath
  if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::NotSigned) {
    throw "1.0 $([char]0x9A8C)$([char]0x6536)$([char]0x9884)$([char]0x671F)$([char]0x672A)$([char]0x7B7E)$([char]0x540D) NSIS$([char]0xFF0C)$([char]0x5B9E)$([char]0x9645) Authenticode $([char]0x72B6)$([char]0x6001)$([char]0x4E3A) $($signature.Status)$([char]0x3002)"
  }
  if ([string]::IsNullOrWhiteSpace($PreviousInstallerPath)) {
    $script:ResolvedPreviousInstallerPath = $script:ResolvedInstallerPath
    $script:PreviousInstallerSha256 = $script:FinalInstallerSha256
    $script:UpgradeKind = "same-version-reinstall"
  } else {
    $script:ResolvedPreviousInstallerPath = Resolve-SourceFile -Path $PreviousInstallerPath -Label "$([char]0x524D)$([char]0x4E00)$([char]0x7248)$([char]0x672C) NSIS $([char]0x5B89)$([char]0x88C5)$([char]0x5305)"
    $script:PreviousInstallerSha256 = Get-FileSha256Lower -Path $script:ResolvedPreviousInstallerPath
    if ($script:PreviousInstallerSha256 -eq $script:FinalInstallerSha256) {
      $script:UpgradeKind = "same-package-reinstall"
    } else {
      $script:UpgradeKind = "candidate-upgrade"
    }
  }
  Add-StepResult -Name "preflight" -Status "passed" -Detail "$([char]0x5B89)$([char]0x88C5)$([char]0x5305)$([char]0x5B58)$([char]0x5728)$([char]0x3001)$([char]0x6587)$([char]0x4EF6)$([char]0x540D)$([char]0x6B63)$([char]0x786E)$([char]0x3001)$([char]0x6700)$([char]0x7EC8)$([char]0x5305) Authenticode=NotSigned$([char]0x3002)"

  $existingRegistrations = @(Get-ProductRegistryEntries)
  if ($existingRegistrations.Count -gt 0 -and -not $AllowExistingProductRegistration) {
    $locations = ($existingRegistrations | ForEach-Object { "[$($_.displayVersion)] $($_.installLocation)" }) -join "; "
    $message = "$([char]0x53D1)$([char]0x73B0)$([char]0x65E2)$([char]0x6709)$([char]0x4EA7)$([char]0x54C1)$([char]0x6CE8)$([char]0x518C)$([char]0xFF0C)$([char]0x5B9E)$([char]0x9645)$([char]0x6267)$([char]0x884C)$([char]0x5C06)$([char]0x62D2)$([char]0x7EDD)$([char]0x8986)$([char]0x76D6)$([char]0xFF1A)$locations$([char]0x3002)$([char]0x8BF7)$([char]0x4F7F)$([char]0x7528)$([char]0x5E72)$([char]0x51C0)$([char]0x6D4B)$([char]0x8BD5)$([char]0x8D26)$([char]0x53F7)/VM$([char]0xFF0C)$([char]0x6216)$([char]0x660E)$([char]0x786E)$([char]0x4F20)$([char]0x5165) -AllowExistingProductRegistration$([char]0x3002)"
    if ($DryRun) {
      Write-Log -Level WARN -Message $message
      Add-StepResult -Name "existing-registration" -Status "warning" -Detail $message
    } else {
      throw $message
    }
  }

  if ($DryRun) {
    Write-Log -Level DRYRUN -Message "$([char]0x8FD0)$([char]0x884C)$([char]0x76EE)$([char]0x5F55)$([char]0xFF1A)$RunRoot"
    Write-Log -Level DRYRUN -Message "$([char]0x5B89)$([char]0x88C5)$([char]0x76EE)$([char]0x5F55)$([char]0xFF1A)$InstallRoot"
    Write-Log -Level DRYRUN -Message "$([char]0x9694)$([char]0x79BB) Profile$([char]0xFF1A)$ProfileRoot"
    Write-Log -Level DRYRUN -Message "$([char]0x8BA1)$([char]0x5212)$([char]0xFF1A)$([char]0x521D)$([char]0x88C5) $([char]0x2192) $([char]0x542F)$([char]0x52A8)/$([char]0x9000)$([char]0x51FA) $([char]0x2192) SQLite/Vault $([char]0x54E8)$([char]0x5175)$([char]0x4E0E)$([char]0x5FEB)$([char]0x7167) $([char]0x2192) $([char]0x5347)$([char]0x7EA7)/$([char]0x91CD)$([char]0x88C5) $([char]0x2192) $([char]0x6570)$([char]0x636E)$([char]0x6BD4)$([char]0x5BF9) $([char]0x2192) $([char]0x4E24)$([char]0x6B21)$([char]0x5E94)$([char]0x7528)$([char]0x91CD)$([char]0x542F) $([char]0x2192) LICENSE/NOTICE $([char]0x6821)$([char]0x9A8C) $([char]0x2192) $([char]0x5378)$([char]0x8F7D) $([char]0x2192) $([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x6E05)$([char]0x7406) $([char]0x2192) $([char]0x6570)$([char]0x636E)$([char]0x4FDD)$([char]0x7559)$([char]0x6821)$([char]0x9A8C)$([char]0x3002)"
    if ($script:UpgradeKind -like "same-*") {
      Write-Log -Level WARN -Message "$([char]0x672A)$([char]0x63D0)$([char]0x4F9B)$([char]0x4E0D)$([char]0x540C)$([char]0x7684) PreviousInstallerPath$([char]0xFF0C)$([char]0x672C)$([char]0x6B21)$([char]0x53EA)$([char]0x80FD)$([char]0x8BA1)$([char]0x5212)$([char]0x540C)$([char]0x7248)$([char]0x672C)$([char]0x91CD)$([char]0x88C5)$([char]0xFF1B)$([char]0x4E0D)$([char]0x80FD)$([char]0x636E)$([char]0x6B64)$([char]0x5BA3)$([char]0x79F0)$([char]0x8DE8)$([char]0x7248)$([char]0x672C)$([char]0x5347)$([char]0x7EA7)$([char]0x5DF2)$([char]0x901A)$([char]0x8FC7)$([char]0x3002)"
      Add-StepResult -Name "upgrade-mode" -Status "warning" -Detail "$([char]0x672A)$([char]0x63D0)$([char]0x4F9B)$([char]0x4E0D)$([char]0x540C)$([char]0x524D)$([char]0x4E00)$([char]0x7248)$([char]0x672C)$([char]0xFF0C)$([char]0x53EA)$([char]0x9A8C)$([char]0x8BC1)$([char]0x540C)$([char]0x7248)$([char]0x672C)$([char]0x91CD)$([char]0x88C5)$([char]0x3002)"
    } else {
      Add-StepResult -Name "upgrade-mode" -Status "planned" -Detail "$([char]0x5C06)$([char]0x4F7F)$([char]0x7528)$([char]0x72EC)$([char]0x7ACB)$([char]0x524D)$([char]0x4E00)$([char]0x7248)$([char]0x672C)$([char]0x5B89)$([char]0x88C5)$([char]0x5305)$([char]0x9A8C)$([char]0x8BC1)$([char]0x5347)$([char]0x7EA7)$([char]0x3002)"
    }
    Add-StepResult -Name "lifecycle" -Status "planned" -Detail "Dry-run $([char]0x672A)$([char]0x6267)$([char]0x884C)$([char]0x4EFB)$([char]0x4F55)$([char]0x5B89)$([char]0x88C5)$([char]0x3001)$([char]0x8FDB)$([char]0x7A0B)$([char]0x3001)$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x6216)$([char]0x6587)$([char]0x4EF6)$([char]0x7CFB)$([char]0x7EDF)$([char]0x53D8)$([char]0x66F4)$([char]0x3002)"
    return
  }

  if (Test-Path -LiteralPath $RunRoot) {
    throw "$([char]0x8FD0)$([char]0x884C)$([char]0x76EE)$([char]0x5F55)$([char]0x5DF2)$([char]0x5B58)$([char]0x5728)$([char]0xFF0C)$([char]0x62D2)$([char]0x7EDD)$([char]0x8986)$([char]0x76D6)$([char]0x6216)$([char]0x5220)$([char]0x9664)$([char]0xFF1A)$RunRoot$([char]0x3002)$([char]0x8BF7)$([char]0x66F4)$([char]0x6362) RunId$([char]0x3002)"
  }
  New-Item -ItemType Directory -Path $LogRoot -Force | Out-Null
  New-Item -ItemType Directory -Path $RegistryBackupRoot -Force | Out-Null
  $script:LogFileEnabled = $true
  Write-JsonNoBom -Value ([ordered]@{
    schemaVersion = 1
    runId = $RunId
    productName = $ProductName
    purpose = "BSAIGC NSIS release acceptance"
    createdAt = (Get-Date).ToString("o")
  }) -Path $RunMarkerPath
  Write-Log -Level INFO -Message "$([char]0x9A8C)$([char]0x6536)$([char]0x5F00)$([char]0x59CB)$([char]0x3002)$([char]0x6240)$([char]0x6709)$([char]0x6D4B)$([char]0x8BD5)$([char]0x5199)$([char]0x5165)$([char]0x5747)$([char]0x9650)$([char]0x5236)$([char]0x5728)$([char]0xFF1A)$RunRoot"

  if ($existingRegistrations.Count -gt 0) {
    Write-Log -Level WARN -Message "$([char]0x5DF2)$([char]0x663E)$([char]0x5F0F)$([char]0x5141)$([char]0x8BB8)$([char]0x8986)$([char]0x76D6)$([char]0x65E2)$([char]0x6709)$([char]0x4EA7)$([char]0x54C1)$([char]0x6CE8)$([char]0x518C)$([char]0xFF1B)$([char]0x811A)$([char]0x672C)$([char]0x4F1A)$([char]0x5907)$([char]0x4EFD)$([char]0x5E76)$([char]0x5728)$([char]0x7ED3)$([char]0x675F)$([char]0x65F6)$([char]0x6062)$([char]0x590D)$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0xFF0C)$([char]0x4F46)$([char]0x4E0D)$([char]0x4F1A)$([char]0x627F)$([char]0x8BFA)$([char]0x6062)$([char]0x590D)$([char]0x88AB)$([char]0x5916)$([char]0x90E8)$([char]0x5B89)$([char]0x88C5)$([char]0x5668)$([char]0x6539)$([char]0x52A8)$([char]0x7684)$([char]0x65E2)$([char]0x6709)$([char]0x5B89)$([char]0x88C5)$([char]0x6587)$([char]0x4EF6)$([char]0x3002)"
    Export-BaselineRegistryEntries -Entries $existingRegistrations
  }

  Initialize-IsolatedProfile
  Invoke-NsisInstall -SourceInstaller $script:ResolvedPreviousInstallerPath -Label "NSIS $([char]0x521D)$([char]0x59CB)$([char]0x5B89)$([char]0x88C5)"
  $script:InitialProductVersion = (Get-Item -LiteralPath $AppPath).VersionInfo.ProductVersion
  if ((Get-Item -LiteralPath $AppPath).VersionInfo.ProductName -ne $ProductName) {
    throw "$([char]0x524D)$([char]0x4E00)$([char]0x7248)$([char]0x672C)$([char]0x5B89)$([char]0x88C5)$([char]0x5305)$([char]0x4E0D)$([char]0x662F)$([char]0x540C)$([char]0x4E00)$([char]0x4EA7)$([char]0x54C1)$([char]0xFF0C)$([char]0x4E0D)$([char]0x80FD)$([char]0x4F5C)$([char]0x4E3A)$([char]0x5347)$([char]0x7EA7)$([char]0x57FA)$([char]0x7EBF)$([char]0x3002)"
  }
  Add-StepResult -Name "initial-install" -Status "passed" -Detail "Initial installation and product identity validation passed."

  $firstProcess = Start-IsolatedApplication -Label "$([char]0x9996)$([char]0x6B21)$([char]0x542F)$([char]0x52A8)"
  Stop-TrackedApplication -Process $firstProcess -Label "$([char]0x9996)$([char]0x6B21)$([char]0x542F)$([char]0x52A8)$([char]0x8FDB)$([char]0x7A0B)"
  Stop-TestInstallProcesses
  Assert-AuthoritativeDataCreated
  $sentinels = @(New-PreservationSentinels)
  $beforeUpgradeSnapshot = @(Get-AuthoritativeDataSnapshot)
  Write-JsonNoBom -Value $beforeUpgradeSnapshot -Path (Join-Path $RunRoot "data-before-upgrade.json")
  Add-StepResult -Name "first-start" -Status "passed" -Detail "$([char]0x5E94)$([char]0x7528)$([char]0x542F)$([char]0x52A8)$([char]0x5E76)$([char]0x9000)$([char]0x51FA)$([char]0xFF0C)$([char]0x9694)$([char]0x79BB) SQLite/Vault $([char]0x5DF2)$([char]0x5EFA)$([char]0x7ACB)$([char]0x3002)"

  Invoke-NsisInstall -SourceInstaller $script:ResolvedInstallerPath -Label "NSIS $([char]0x5347)$([char]0x7EA7)$([char]0x5B89)$([char]0x88C5)"
  $script:FinalProductVersion = Assert-InstalledAppIdentity -ExpectedVersion $Version
  $initialVersionObject = [version]$script:InitialProductVersion
  $finalVersionObject = [version]$script:FinalProductVersion
  if ($script:PreviousInstallerSha256 -eq $script:FinalInstallerSha256) {
    $script:UpgradeKind = "same-package-reinstall"
  } elseif ($initialVersionObject -lt $finalVersionObject) {
    $script:UpgradeKind = "cross-version-upgrade"
  } elseif ($initialVersionObject -eq $finalVersionObject) {
    $script:UpgradeKind = "same-version-replacement"
  } else {
    throw "The previous installer product version is newer than the final candidate; downgrade acceptance is not allowed."
  }
  Assert-CodexLegalFiles
  if ($ExpectEmbeddedPreviewCredential) {
    Invoke-FinalCandidateCredentialProbe
  }
  $afterUpgradeBeforeLaunchSnapshot = @(Get-AuthoritativeDataSnapshot)
  Write-JsonNoBom -Value $afterUpgradeBeforeLaunchSnapshot -Path (Join-Path $RunRoot "data-after-upgrade-before-launch.json")
  Assert-SnapshotsEqual -Before $beforeUpgradeSnapshot -After $afterUpgradeBeforeLaunchSnapshot -Label "$([char]0x5347)$([char]0x7EA7)$([char]0x5B89)$([char]0x88C5)"
  Assert-SentinelsExist -Paths $sentinels -Label "$([char]0x5347)$([char]0x7EA7)$([char]0x5B89)$([char]0x88C5)"
  Add-StepResult -Name "upgrade" -Status "passed" -Detail "$([char]0x6700)$([char]0x7EC8)$([char]0x5B89)$([char]0x88C5)$([char]0x5305)$([char]0x8986)$([char]0x76D6)$([char]0x5B89)$([char]0x88C5)$([char]0x6210)$([char]0x529F)$([char]0xFF0C)$([char]0x542F)$([char]0x52A8)$([char]0x524D) SQLite/Vault $([char]0x5FEB)$([char]0x7167)$([char]0x5B8C)$([char]0x5168)$([char]0x4E00)$([char]0x81F4)$([char]0x3002)"

  $restartOne = Start-IsolatedApplication -Label "$([char]0x5347)$([char]0x7EA7)$([char]0x540E)$([char]0x7B2C) 1 $([char]0x6B21)$([char]0x542F)$([char]0x52A8)"
  $restartOnePid = $restartOne.Id
  Stop-TrackedApplication -Process $restartOne -Label "$([char]0x5347)$([char]0x7EA7)$([char]0x540E)$([char]0x7B2C) 1 $([char]0x6B21)$([char]0x542F)$([char]0x52A8)$([char]0x8FDB)$([char]0x7A0B)"
  Stop-TestInstallProcesses
  Assert-SentinelsExist -Paths $sentinels -Label "$([char]0x5347)$([char]0x7EA7)$([char]0x540E)$([char]0x7B2C) 1 $([char]0x6B21)$([char]0x542F)$([char]0x52A8)"

  $restartTwo = Start-IsolatedApplication -Label "$([char]0x5347)$([char]0x7EA7)$([char]0x540E)$([char]0x7B2C) 2 $([char]0x6B21)$([char]0x542F)$([char]0x52A8)"
  $restartTwoPid = $restartTwo.Id
  if ($restartOnePid -eq $restartTwoPid) {
    throw "$([char]0x4E24)$([char]0x6B21)$([char]0x542F)$([char]0x52A8)$([char]0x590D)$([char]0x7528)$([char]0x4E86)$([char]0x540C)$([char]0x4E00) PID$([char]0xFF0C)$([char]0x91CD)$([char]0x542F)$([char]0x9A8C)$([char]0x8BC1)$([char]0x65E0)$([char]0x6548)$([char]0x3002)"
  }
  Stop-TrackedApplication -Process $restartTwo -Label "$([char]0x5347)$([char]0x7EA7)$([char]0x540E)$([char]0x7B2C) 2 $([char]0x6B21)$([char]0x542F)$([char]0x52A8)$([char]0x8FDB)$([char]0x7A0B)"
  Stop-TestInstallProcesses
  Assert-SentinelsExist -Paths $sentinels -Label "$([char]0x5347)$([char]0x7EA7)$([char]0x540E)$([char]0x7B2C) 2 $([char]0x6B21)$([char]0x542F)$([char]0x52A8)"
  Add-StepResult -Name "restart" -Status "passed" -Detail "$([char]0x5347)$([char]0x7EA7)$([char]0x540E)$([char]0x5B8C)$([char]0x6210)$([char]0x4E24)$([char]0x6B21)$([char]0x72EC)$([char]0x7ACB) PID $([char]0x7684)$([char]0x542F)$([char]0x52A8)/$([char]0x9000)$([char]0x51FA)$([char]0xFF0C)$([char]0x6570)$([char]0x636E)$([char]0x54E8)$([char]0x5175)$([char]0x4FDD)$([char]0x7559)$([char]0x3002)"

  Invoke-TestUninstall
  Assert-SentinelsExist -Paths $sentinels -Label "$([char]0x5378)$([char]0x8F7D)"
  Assert-AuthoritativeDataCreated
  if (Test-Path -LiteralPath $InstallRoot -PathType Container) {
    $remainingFiles = @(Get-ChildItem -LiteralPath $InstallRoot -Recurse -Force -File -ErrorAction SilentlyContinue)
    if ($remainingFiles.Count -gt 0) {
      throw "$([char]0x5378)$([char]0x8F7D)$([char]0x540E)$([char]0x6D4B)$([char]0x8BD5)$([char]0x5B89)$([char]0x88C5)$([char]0x76EE)$([char]0x5F55)$([char]0x4ECD)$([char]0x542B)$([char]0x6587)$([char]0x4EF6)$([char]0xFF1B)$([char]0x811A)$([char]0x672C)$([char]0x4E0D)$([char]0x4F1A)$([char]0x9012)$([char]0x5F52)$([char]0x5220)$([char]0x9664)$([char]0xFF0C)$([char]0x9700)$([char]0x68C0)$([char]0x67E5)$([char]0xFF1A)$InstallRoot"
    }
    Write-Log -Level WARN -Message "$([char]0x5378)$([char]0x8F7D)$([char]0x540E)$([char]0x7559)$([char]0x4E0B)$([char]0x7A7A)$([char]0x5B89)$([char]0x88C5)$([char]0x76EE)$([char]0x5F55)$([char]0xFF1B)$([char]0x811A)$([char]0x672C)$([char]0x6309)$([char]0x5B89)$([char]0x5168)$([char]0x89C4)$([char]0x5219)$([char]0x4E0D)$([char]0x4E3B)$([char]0x52A8)$([char]0x5220)$([char]0x9664)$([char]0x76EE)$([char]0x5F55)$([char]0x3002)"
  }
  Add-StepResult -Name "uninstall" -Status "passed" -Detail "$([char]0x5378)$([char]0x8F7D)$([char]0x9000)$([char]0x51FA)$([char]0x7801)$([char]0x4E3A) 0$([char]0xFF0C)$([char]0x4E3B)$([char]0x7A0B)$([char]0x5E8F)/$([char]0x5378)$([char]0x8F7D)$([char]0x7A0B)$([char]0x5E8F)/$([char]0x6D4B)$([char]0x8BD5)$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x9879)$([char]0x5DF2)$([char]0x79FB)$([char]0x9664)$([char]0xFF0C)SQLite/Vault $([char]0x4FDD)$([char]0x7559)$([char]0x3002)"

  Restore-BaselineRegistryEntries
  Add-StepResult -Name "registry-restore" -Status "passed" -Detail "$([char]0x65E2)$([char]0x6709)$([char]0x4EA7)$([char]0x54C1)$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x5FEB)$([char]0x7167)$([char]0x5DF2)$([char]0x6062)$([char]0x590D)$([char]0xFF08)$([char]0x5982)$([char]0x6709)$([char]0xFF09)$([char]0x3002)"
  Write-Log -Level PASS -Message "NSIS $([char]0x53D1)$([char]0x5E03)$([char]0x9A8C)$([char]0x6536)$([char]0x5168)$([char]0x90E8)$([char]0x901A)$([char]0x8FC7)$([char]0x3002)$([char]0x7ED3)$([char]0x679C)$([char]0x76EE)$([char]0x5F55)$([char]0xFF1A)$RunRoot"
} catch {
  $script:AcceptanceError = $_
  Add-StepResult -Name "acceptance" -Status "failed" -Detail $_.Exception.Message
  Write-Log -Level ERROR -Message $_.Exception.Message
} finally {
  if (-not $DryRun) {
    try {
      Stop-TestInstallProcesses
    } catch {
      Write-Log -Level ERROR -Message "$([char]0x6E05)$([char]0x7406)$([char]0x6D4B)$([char]0x8BD5)$([char]0x8FDB)$([char]0x7A0B)$([char]0x5931)$([char]0x8D25)$([char]0xFF1A)$($_.Exception.Message)"
      if ($null -eq $script:AcceptanceError) { $script:AcceptanceError = $_ }
    }
    if ($script:InstalledByThisRun -and -not $script:UninstallCompleted) {
      try {
        Write-Log -Level WARN -Message "$([char]0x9A8C)$([char]0x6536)$([char]0x672A)$([char]0x6B63)$([char]0x5E38)$([char]0x8D70)$([char]0x5230)$([char]0x5378)$([char]0x8F7D)$([char]0xFF0C)$([char]0x6267)$([char]0x884C)$([char]0x53D7)$([char]0x9650)$([char]0x6E05)$([char]0x7406)$([char]0x3002)"
        Invoke-TestUninstall
      } catch {
        Write-Log -Level ERROR -Message "$([char]0x53D7)$([char]0x9650)$([char]0x5378)$([char]0x8F7D)$([char]0x6E05)$([char]0x7406)$([char]0x5931)$([char]0x8D25)$([char]0xFF1A)$($_.Exception.Message)"
        if ($null -eq $script:AcceptanceError) { $script:AcceptanceError = $_ }
      }
    }
    try {
      Restore-BaselineRegistryEntries
    } catch {
      Write-Log -Level ERROR -Message "$([char]0x6062)$([char]0x590D)$([char]0x65E2)$([char]0x6709)$([char]0x6CE8)$([char]0x518C)$([char]0x8868)$([char]0x5931)$([char]0x8D25)$([char]0xFF1A)$($_.Exception.Message)"
      if ($null -eq $script:AcceptanceError) { $script:AcceptanceError = $_ }
    }
    try {
      Write-AcceptanceSummary
    } catch {
      Write-Log -Level ERROR -Message "$([char]0x5199)$([char]0x5165)$([char]0x9A8C)$([char]0x6536)$([char]0x6458)$([char]0x8981)$([char]0x5931)$([char]0x8D25)$([char]0xFF1A)$($_.Exception.Message)"
      if ($null -eq $script:AcceptanceError) { $script:AcceptanceError = $_ }
    }
  }
}

if ($null -ne $script:AcceptanceError) {
  throw $script:AcceptanceError
}
