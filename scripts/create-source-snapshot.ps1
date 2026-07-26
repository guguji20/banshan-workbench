[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [ValidatePattern('^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$')]
  [string]$Version,

  [Parameter(Mandatory = $true)]
  [string]$OutputDirectory,

  [ValidateRange(1, 1024)]
  [int]$LargeBinaryThresholdMiB = 20,

  [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$RepoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..')).TrimEnd([IO.Path]::DirectorySeparatorChar)
$OutputRoot = [IO.Path]::GetFullPath($OutputDirectory).TrimEnd([IO.Path]::DirectorySeparatorChar)
$RepoPrefix = $RepoRoot + [IO.Path]::DirectorySeparatorChar
$OutputPrefix = $OutputRoot + [IO.Path]::DirectorySeparatorChar
$StagingRoot = $null
$MarkerName = '.bsaigc-source-snapshot-staging'

function Write-Utf8([string]$Path, [string]$Text) {
  [IO.File]::WriteAllText($Path, ($Text -replace "`r?`n", "`n"), $Utf8NoBom)
}

function Get-Sha256([string]$Path) {
  return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-TextSha256([string]$Text) {
  $sha = [Security.Cryptography.SHA256]::Create()
  try {
    $bytes = $Utf8NoBom.GetBytes($Text)
    return ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
  } finally { $sha.Dispose() }
}

function Get-RelativePath([string]$FullPath) {
  $full = [IO.Path]::GetFullPath($FullPath)
  if (-not $full.StartsWith($RepoPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Path escaped repository root: $full"
  }
  $relative = $full.Substring($RepoPrefix.Length).Replace('\', '/')
  if ([string]::IsNullOrWhiteSpace($relative) -or [IO.Path]::IsPathRooted($relative) -or $relative -match '(^|/)\.\.(/|$)') {
    throw "Unsafe repository-relative path: $relative"
  }
  return $relative
}

function Test-UnderPath([string]$Candidate, [string]$Parent) {
  $candidateFull = [IO.Path]::GetFullPath($Candidate).TrimEnd([IO.Path]::DirectorySeparatorChar)
  $parentFull = [IO.Path]::GetFullPath($Parent).TrimEnd([IO.Path]::DirectorySeparatorChar)
  return $candidateFull.Equals($parentFull, [StringComparison]::OrdinalIgnoreCase) -or
    $candidateFull.StartsWith($parentFull + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)
}

function Quote-NativeArgument([string]$Value) {
  if ($Value.Length -gt 0 -and $Value -notmatch '[\s"]') { return $Value }
  return '"' + $Value.Replace('\', '\').Replace('"', '\"') + '"'
}

function Invoke-Git([string[]]$Arguments) {
  $psi = New-Object Diagnostics.ProcessStartInfo
  $psi.FileName = 'git.exe'
  $psi.WorkingDirectory = $RepoRoot
  $all = @('-c', ('safe.directory=' + $RepoRoot.Replace('\', '/')), '-c', 'core.quotepath=false') + $Arguments
  $psi.Arguments = (($all | ForEach-Object { Quote-NativeArgument ([string]$_) }) -join ' ')
  $psi.UseShellExecute = $false
  $psi.CreateNoWindow = $true
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  try {
    $psi.StandardOutputEncoding = $Utf8NoBom
    $psi.StandardErrorEncoding = $Utf8NoBom
  } catch { }
  $process = New-Object Diagnostics.Process
  $process.StartInfo = $psi
  if (-not $process.Start()) { throw 'Unable to start git.exe.' }
  $stdoutTask = $process.StandardOutput.ReadToEndAsync()
  $stderrTask = $process.StandardError.ReadToEndAsync()
  $process.WaitForExit()
  $stdout = $stdoutTask.Result
  $stderr = $stderrTask.Result
  if ($process.ExitCode -ne 0) {
    throw "git $($Arguments -join ' ') failed ($($process.ExitCode)): $stderr"
  }
  return $stdout
}

function Test-SyntheticValue([string]$Value) {
  $lower = $Value.ToLowerInvariant()
  return $lower -match '(test|fake|dummy|example|sample|placeholder|redacted|changeme|replace-me|not-a-real|should-not|provider-secret|legacy-secret)' -or $lower -match '^sk-(?:[a-z]+-){1,6}(?:secret|survive|only)$'
}

function Test-HighEntropyValue([string]$Value) {
  $value = $Value.Trim()
  if ($value.Length -lt 16 -or (Test-SyntheticValue $value)) { return $false }
  if ($value -match '^\$|^%[A-Za-z_][A-Za-z0-9_]*%$|^\{\{|^<[^>]+>$|process\.env|env::|getenv') { return $false }
  if ($value -match '^[0-9a-fA-F]{32,}$') { return $true }
  $classes = 0
  if ($value -cmatch '[a-z]') { $classes++ }
  if ($value -cmatch '[A-Z]') { $classes++ }
  if ($value -match '[0-9]') { $classes++ }
  if ($value -match '[-_+/=]') { $classes++ }
  $unique = ($value.ToCharArray() | Select-Object -Unique).Count
  return $value.Length -ge 24 -and $classes -ge 2 -and $unique -ge 10
}

function Get-LineNumber([string]$Text, [int]$Index) {
  if ($Index -le 0) { return 1 }
  return ([regex]::Matches($Text.Substring(0, $Index), "`n").Count + 1)
}

function Test-ProbablyBinary([byte[]]$Bytes) {
  $limit = [Math]::Min($Bytes.Length, 8192)
  for ($i = 0; $i -lt $limit; $i++) {
    if ($Bytes[$i] -eq 0) { return $true }
  }
  return $false
}

function Find-Secrets([string]$RelativePath, [byte[]]$Bytes) {
  $findings = New-Object Collections.Generic.List[object]
  if (Test-ProbablyBinary $Bytes) { return $findings.ToArray() }
  $text = $Utf8NoBom.GetString($Bytes)
  $rules = @(
    @{ Name = 'private-key'; Pattern = '-----BEGIN\s+(?:RSA\s+|EC\s+|OPENSSH\s+|DSA\s+)?PRIVATE\s+KEY-----' },
    @{ Name = 'openai-style-key'; Pattern = '(?<![A-Za-z0-9_])sk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{20,}' },
    @{ Name = 'github-token'; Pattern = '(?<![A-Za-z0-9_])gh[pousr]_[A-Za-z0-9]{20,}' },
    @{ Name = 'aws-access-key'; Pattern = '(?<![A-Z0-9])(?:AKIA|ASIA)[0-9A-Z]{16}(?![A-Z0-9])' },
    @{ Name = 'google-api-key'; Pattern = '(?<![A-Za-z0-9_])AIza[0-9A-Za-z_-]{30,}' },
    @{ Name = 'slack-token'; Pattern = '(?<![A-Za-z0-9_])xox[baprs]-[A-Za-z0-9-]{10,}' }
  )
  foreach ($rule in $rules) {
    foreach ($match in [regex]::Matches($text, $rule.Pattern)) {
      if (-not (Test-SyntheticValue $match.Value)) {
        $findings.Add([pscustomobject][ordered]@{ path = $RelativePath; line = Get-LineNumber $text $match.Index; rule = $rule.Name })
      }
    }
  }
  $assignment = '(?im)(?:api[_-]?key|access[_-]?token|auth[_-]?token|client[_-]?secret|secret[_-]?key|password)\s*["'']?\s*[:=]\s*(?:"([^"\r\n]{12,})"|''([^''\r\n]{12,})'')'
  foreach ($match in [regex]::Matches($text, $assignment)) {
    $value = if ($match.Groups[1].Success) { $match.Groups[1].Value } else { $match.Groups[2].Value }
    if (Test-HighEntropyValue $value) {
      $findings.Add([pscustomobject][ordered]@{ path = $RelativePath; line = Get-LineNumber $text $match.Index; rule = 'literal-secret-assignment' })
    }
  }
  return $findings.ToArray()
}

function Get-SourceInventory {
  $excludedRoots = @('.git', 'node_modules', 'dist', 'src-tauri/target', 'release', '.runtime', 'upstream')
  $compiledExtensions = @('.exe', '.dll', '.pdb', '.lib', '.so', '.dylib', '.node', '.wasm', '.msi')
  $archiveExtensions = @('.zip', '.7z', '.rar', '.tar', '.gz', '.bz2', '.xz')
  $threshold = [int64]$LargeBinaryThresholdMiB * 1MB
  $included = New-Object Collections.Generic.List[object]
  $excluded = New-Object Collections.Generic.List[object]
  $findings = New-Object Collections.Generic.List[object]
  $stack = New-Object Collections.Generic.Stack[IO.DirectoryInfo]
  $stack.Push((Get-Item -LiteralPath $RepoRoot))

  while ($stack.Count -gt 0) {
    $directory = $stack.Pop()
    foreach ($childDirectory in @(Get-ChildItem -LiteralPath $directory.FullName -Force -Directory | Sort-Object FullName -Descending)) {
      $relative = Get-RelativePath $childDirectory.FullName
      $reason = $null
      foreach ($root in $excludedRoots) {
        if ($relative.Equals($root, [StringComparison]::OrdinalIgnoreCase) -or $relative.StartsWith($root + '/', [StringComparison]::OrdinalIgnoreCase)) {
          $reason = 'excluded-root:' + $root
          break
        }
      }
      if (-not $reason -and $OutputRootInsideRepo -and (Test-UnderPath $childDirectory.FullName $OutputRoot)) { $reason = 'output-directory' }
      if (-not $reason -and (($childDirectory.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) { $reason = 'reparse-point-directory' }
      if (-not $reason -and (Test-Path -LiteralPath (Join-Path $childDirectory.FullName '.git'))) { $reason = 'nested-git-repository' }
      if ($reason) {
        $excluded.Add([pscustomobject][ordered]@{ relativePath = $relative; reason = $reason; kind = 'directory' })
      } else {
        $stack.Push($childDirectory)
      }
    }

    foreach ($file in @(Get-ChildItem -LiteralPath $directory.FullName -Force -File | Sort-Object FullName)) {
      $relative = Get-RelativePath $file.FullName
      if (($OutputRootInsideRepo -and (Test-UnderPath $file.FullName $OutputRoot)) -or (($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) {
        $excluded.Add([pscustomobject][ordered]@{ relativePath = $relative; reason = 'output-or-reparse-point'; kind = 'file'; size = $file.Length })
        continue
      }
      $leaf = $file.Name.ToLowerInvariant()
      if (($leaf -like '.env*' -and $leaf -notmatch '^\.env\.(example|sample|template)$') -or $leaf -match '^(id_rsa|id_ed25519|credentials\.json|secrets\.json)$' -or $file.Extension.ToLowerInvariant() -in @('.key', '.p12', '.pfx')) {
        $findings.Add([pscustomobject][ordered]@{ path = $relative; line = 0; rule = 'sensitive-filename' })
        continue
      }
      $extension = $file.Extension.ToLowerInvariant()
      $bytes = [IO.File]::ReadAllBytes($file.FullName)
      $binary = Test-ProbablyBinary $bytes
      $excludeReason = $null
      if ($extension -in $compiledExtensions) { $excludeReason = 'compiled-binary' }
      elseif ($extension -in $archiveExtensions) { $excludeReason = 'archive-binary' }
      elseif ($file.Length -gt $threshold -and $binary) { $excludeReason = 'large-binary' }
      if ($excludeReason) {
        $excluded.Add([pscustomobject][ordered]@{ relativePath = $relative; reason = $excludeReason; kind = 'file'; size = $file.Length; sha256 = Get-Sha256 $file.FullName })
        continue
      }
      foreach ($finding in @(Find-Secrets $relative $bytes)) { $findings.Add($finding) }
      $included.Add([pscustomobject][ordered]@{ relativePath = $relative; fullPath = $file.FullName; size = $file.Length; sha256 = Get-Sha256 $file.FullName })
    }
  }
  return [pscustomobject]@{ included = @($included.ToArray() | Sort-Object relativePath); excluded = @($excluded.ToArray() | Sort-Object relativePath); findings = @($findings.ToArray() | Sort-Object path, line, rule) }
}

function New-DeterministicZip([object[]]$Files, [string]$ZipPath) {
  Add-Type -AssemblyName System.IO.Compression
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  $stream = [IO.File]::Open($ZipPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
  try {
    $archive = New-Object IO.Compression.ZipArchive($stream, [IO.Compression.ZipArchiveMode]::Create, $false, $Utf8NoBom)
    try {
      foreach ($file in $Files) {
        $entry = $archive.CreateEntry($file.relativePath, [IO.Compression.CompressionLevel]::Optimal)
        $entry.LastWriteTime = New-Object DateTimeOffset(2000, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
        $input = [IO.File]::OpenRead($file.fullPath)
        $output = $entry.Open()
        try { $input.CopyTo($output) } finally { $output.Dispose(); $input.Dispose() }
      }
    } finally { $archive.Dispose() }
  } finally { $stream.Dispose() }
}

function Test-Zip([object[]]$Files, [string]$ZipPath) {
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  $expected = @{}
  foreach ($file in $Files) { $expected[$file.relativePath] = $file }
  $archive = [IO.Compression.ZipFile]::OpenRead($ZipPath)
  try {
    if ($archive.Entries.Count -ne $Files.Count) { throw 'ZIP entry count mismatch.' }
    foreach ($entry in $archive.Entries) {
      if (-not $expected.ContainsKey($entry.FullName)) { throw "Unexpected ZIP entry: $($entry.FullName)" }
      $file = $expected[$entry.FullName]
      if ($entry.Length -ne $file.size) { throw "ZIP size mismatch: $($entry.FullName)" }
      $sha = [Security.Cryptography.SHA256]::Create()
      $entryStream = $entry.Open()
      try { $actual = ([BitConverter]::ToString($sha.ComputeHash($entryStream))).Replace('-', '').ToLowerInvariant() } finally { $entryStream.Dispose(); $sha.Dispose() }
      if ($actual -ne $file.sha256) { throw "ZIP hash mismatch: $($entry.FullName)" }
    }
  } finally { $archive.Dispose() }
}

function Remove-StagingSafely {
  if (-not $StagingRoot -or -not (Test-Path -LiteralPath $StagingRoot)) { return }
  if (-not (Test-UnderPath $StagingRoot $OutputRoot)) { throw "Refusing to clean staging outside output root: $StagingRoot" }
  if ((Split-Path -Leaf $StagingRoot) -notlike '.source-snapshot-staging-*') { throw "Refusing to clean unrecognized staging directory: $StagingRoot" }
  if (-not (Test-Path -LiteralPath (Join-Path $StagingRoot $MarkerName) -PathType Leaf)) { throw "Refusing to clean staging without marker: $StagingRoot" }
  Remove-Item -LiteralPath $StagingRoot -Recurse -Force
}

$OutputRootInsideRepo = Test-UnderPath $OutputRoot $RepoRoot
if ($OutputRoot.Equals($RepoRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'OutputDirectory cannot be the repository root.' }

try {
  if (-not (Test-Path -LiteralPath (Join-Path $RepoRoot '.git'))) { throw "Not a Git repository: $RepoRoot" }
  if (-not (Test-Path -LiteralPath $OutputRoot)) { New-Item -ItemType Directory -Path $OutputRoot | Out-Null }
  $OutputRoot = (Resolve-Path -LiteralPath $OutputRoot).Path.TrimEnd([IO.Path]::DirectorySeparatorChar)
  $StagingRoot = Join-Path $OutputRoot ('.source-snapshot-staging-' + [Guid]::NewGuid().ToString('N'))
  if (-not (Test-UnderPath $StagingRoot $OutputRoot)) { throw 'Unsafe staging path.' }
  New-Item -ItemType Directory -Path $StagingRoot | Out-Null
  Write-Utf8 (Join-Path $StagingRoot $MarkerName) 'created-by=create-source-snapshot.ps1'

  $head = (Invoke-Git @('rev-parse', 'HEAD')).Trim()
  $branch = (Invoke-Git @('branch', '--show-current')).Trim()
  if ([string]::IsNullOrWhiteSpace($branch)) { $branch = '(detached)' }
  $commitTime = (Invoke-Git @('show', '-s', '--format=%cI', 'HEAD')).Trim()
  $status = Invoke-Git @('status', '--porcelain=v1', '--branch', '--untracked-files=all')
  $trackedRaw = Invoke-Git @('ls-files', '-z')
  $untrackedRaw = Invoke-Git @('ls-files', '--others', '--exclude-standard', '-z')
  $tracked = @{}
  foreach ($path in @($trackedRaw -split "`0")) { if ($path) { $tracked[$path.Replace('\', '/')] = $true } }
  $untrackedSet = @{}
  foreach ($path in @($untrackedRaw -split "`0")) { if ($path) { $untrackedSet[$path.Replace('\', '/')] = $true } }

  $inventory = Get-SourceInventory
  if ($inventory.findings.Count -gt 0) {
    Write-Host "Sensitive material detected. Snapshot blocked. Values are intentionally not printed." -ForegroundColor Red
    foreach ($finding in $inventory.findings) { Write-Host ("BLOCKED {0}:{1} [{2}]" -f $finding.path, $finding.line, $finding.rule) }
    throw "Sensitive information gate failed with $($inventory.findings.Count) finding(s)."
  }

  $includedPaths = @{}
  foreach ($file in $inventory.included) { $includedPaths[$file.relativePath] = $true }
  $untrackedIncluded = @($untrackedSet.Keys | Where-Object { $includedPaths.ContainsKey($_) } | Sort-Object)
  $untrackedText = if ($untrackedIncluded.Count -gt 0) { ($untrackedIncluded -join "`n") + "`n" } else { '' }

  $pathspecs = @('.', ':(exclude).git/**', ':(exclude)node_modules/**', ':(exclude)dist/**', ':(exclude)src-tauri/target/**', ':(exclude)release/**', ':(exclude).runtime/**', ':(exclude)upstream/**')
  foreach ($excluded in $inventory.excluded) {
    if ($excluded.kind -eq 'file') { $pathspecs += (':(exclude)' + $excluded.relativePath) }
  }
  $patch = Invoke-Git (@('diff', '--binary', '--full-index', '--no-ext-diff', 'HEAD', '--') + $pathspecs)
  $patchFindings = @(Find-Secrets 'git-diff.binary.patch' $Utf8NoBom.GetBytes($patch))
  if ($patchFindings.Count -gt 0) {
    Write-Host 'Sensitive material detected in Git diff. Snapshot blocked. Values are intentionally not printed.' -ForegroundColor Red
    foreach ($finding in $patchFindings) { Write-Host ("BLOCKED {0}:{1} [{2}]" -f $finding.path, $finding.line, $finding.rule) }
    throw "Sensitive information gate failed for Git diff with $($patchFindings.Count) finding(s)."
  }

  $treeMaterial = ($inventory.included | ForEach-Object { "$($_.relativePath)`t$($_.size)`t$($_.sha256)" }) -join "`n"
  $sourceTreeSha = Get-TextSha256 ($treeMaterial + "`n")
  $patchSha = Get-TextSha256 $patch
  $untrackedSha = Get-TextSha256 $untrackedText
  $statusSha = Get-TextSha256 $status
  $finalSha = Get-TextSha256 ("BSAIGC_SOURCE_SNAPSHOT_V1`nversion=$Version`nhead=$head`ntree=$sourceTreeSha`npatch=$patchSha`nuntracked=$untrackedSha`n")

  Write-Host "Repository: $RepoRoot"
  Write-Host "Git HEAD: $head"
  Write-Host "Branch: $branch"
  Write-Host "Included files: $($inventory.included.Count)"
  Write-Host "Excluded entries: $($inventory.excluded.Count)"
  Write-Host 'Sensitive findings: 0'
  Write-Host "FINAL_SNAPSHOT_SHA256: $finalSha"

  if ($DryRun) {
    Write-Host 'DRY_RUN_OK: no final snapshot was written.'
    Remove-StagingSafely
    $StagingRoot = $null
    exit 0
  }

  $timestamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
  $shortHead = $head.Substring(0, [Math]::Min(12, $head.Length))
  $baseName = "bsaigc-desktop-source-$Version-$shortHead-$timestamp"
  $zipName = "$baseName.zip"
  $zipPath = Join-Path $StagingRoot $zipName
  New-DeterministicZip $inventory.included $zipPath
  Test-Zip $inventory.included $zipPath
  $zipSha = Get-Sha256 $zipPath

  Write-Utf8 (Join-Path $StagingRoot 'git-status.txt') $status
  Write-Utf8 (Join-Path $StagingRoot 'git-diff.binary.patch') $patch
  Write-Utf8 (Join-Path $StagingRoot 'untracked-files.txt') $untrackedText

  $manifestFiles = foreach ($file in $inventory.included) {
    [pscustomobject][ordered]@{
      relativePath = $file.relativePath
      size = [int64]$file.size
      sha256 = $file.sha256
      gitState = if ($tracked.ContainsKey($file.relativePath)) { 'tracked' } elseif ($untrackedSet.ContainsKey($file.relativePath)) { 'untracked' } else { 'ignored-or-other' }
    }
  }
  $manifest = [ordered]@{
    schemaVersion = 1
    product = 'BSAIGC Desktop'
    version = $Version
    createdAtUtc = (Get-Date).ToUniversalTime().ToString('o')
    repository = [ordered]@{ branch = $branch; head = $head; commitTime = $commitTime; dirty = -not [string]::IsNullOrWhiteSpace($status); statusSha256 = $statusSha }
    snapshot = [ordered]@{
      finalSha256 = $finalSha
      sourceTreeSha256 = $sourceTreeSha
      sourceZip = $zipName
      sourceZipSha256 = $zipSha
      gitDiffSha256 = $patchSha
      untrackedListSha256 = $untrackedSha
      includedFileCount = $inventory.included.Count
      untrackedIncludedCount = $untrackedIncluded.Count
      largeBinaryThresholdMiB = $LargeBinaryThresholdMiB
    }
    security = [ordered]@{ blockedFindings = 0; result = 'passed'; valuesPrinted = $false }
    excluded = $inventory.excluded
    files = @($manifestFiles)
  }
  $manifestPath = Join-Path $StagingRoot 'source-manifest.json'
  Write-Utf8 $manifestPath (($manifest | ConvertTo-Json -Depth 8) + "`n")
  $manifestSha = Get-Sha256 $manifestPath

  $sumLines = @(
    "$zipSha  $zipName",
    "$manifestSha  source-manifest.json",
    "$(Get-Sha256 (Join-Path $StagingRoot 'git-diff.binary.patch'))  git-diff.binary.patch",
    "$(Get-Sha256 (Join-Path $StagingRoot 'git-status.txt'))  git-status.txt",
    "$(Get-Sha256 (Join-Path $StagingRoot 'untracked-files.txt'))  untracked-files.txt",
    "FINAL_SNAPSHOT_SHA256  $finalSha"
  )
  Write-Utf8 (Join-Path $StagingRoot 'SHA256SUMS.txt') (($sumLines -join "`n") + "`n")

  $finalDirectory = Join-Path $OutputRoot $baseName
  if (Test-Path -LiteralPath $finalDirectory) { throw "Final snapshot directory already exists: $finalDirectory" }
  if (-not (Test-UnderPath $finalDirectory $OutputRoot)) { throw 'Unsafe final snapshot path.' }
  Move-Item -LiteralPath $StagingRoot -Destination $finalDirectory
  $StagingRoot = $null

  Write-Host "SNAPSHOT_DIRECTORY: $finalDirectory"
  Write-Host "SOURCE_ZIP_SHA256: $zipSha"
  Write-Host "MANIFEST_SHA256: $manifestSha"
  Write-Host 'SNAPSHOT_OK'
} catch {
  try { Remove-StagingSafely } catch { Write-Warning $_.Exception.Message }
  throw
}