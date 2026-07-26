[CmdletBinding()]
param(
  [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$')]
  [string]$RunId = "latest"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$RuntimeRoot = [System.IO.Path]::GetFullPath((Join-Path $RepoRoot ".runtime"))
$EvidenceRoot = [System.IO.Path]::GetFullPath((Join-Path $RuntimeRoot "contract-review-e2e\$RunId"))
$LogRoot = Join-Path $EvidenceRoot "logs"
$SummaryPath = Join-Path $EvidenceRoot "summary.json"
$PreflightPath = Join-Path $EvidenceRoot "preflight.json"
$StartedAt = (Get-Date).ToUniversalTime()
$Results = New-Object System.Collections.Generic.List[object]
$Preflight = $null

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

function Assert-SafeEvidencePath {
  param([Parameter(Mandatory = $true)][string]$Path)

  if (-not (Test-IsDescendantPath -Candidate $Path -Parent $RuntimeRoot)) {
    throw "Evidence path must stay inside .runtime: $Path"
  }
}

function Write-JsonNoBom {
  param(
    [Parameter(Mandatory = $true)]$Value,
    [Parameter(Mandatory = $true)][string]$Path,
    [int]$Depth = 16
  )

  Assert-SafeEvidencePath -Path $Path
  $json = $Value | ConvertTo-Json -Depth $Depth
  [System.IO.File]::WriteAllText($Path, $json + [Environment]::NewLine, $Utf8NoBom)
}

function Add-Result {
  param(
    [Parameter(Mandatory = $true)][string]$Id,
    [Parameter(Mandatory = $true)][ValidateSet("passed", "failed")][string]$Status,
    [Parameter(Mandatory = $true)][string]$Coverage,
    [Parameter(Mandatory = $true)][string]$Detail,
    [long]$DurationMs = 0,
    [AllowNull()][string]$StdoutLog = $null,
    [AllowNull()][string]$StderrLog = $null
  )

  $Results.Add([ordered]@{
    id = $Id
    status = $Status
    coverage = $Coverage
    detail = $Detail
    durationMs = $DurationMs
    stdoutLog = $StdoutLog
    stderrLog = $StderrLog
  })
}

function Assert-FileContains {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Pattern,
    [Parameter(Mandatory = $true)][string]$Description
  )

  $content = Get-Content -LiteralPath $Path -Raw -Encoding UTF8
  if ($content -notmatch $Pattern) {
    throw "Static interface check failed: $Description ($Path)"
  }
}

function Get-RelativeEvidencePath {
  param([Parameter(Mandatory = $true)][string]$Path)

  $prefix = $RepoRoot.TrimEnd('\') + [System.IO.Path]::DirectorySeparatorChar
  return $Path.Substring($prefix.Length).Replace('\', '/')
}

function Invoke-CheckedProcess {
  param(
    [Parameter(Mandatory = $true)][string]$Id,
    [Parameter(Mandatory = $true)][string]$Coverage,
    [Parameter(Mandatory = $true)][string]$FilePath,
    [Parameter(Mandatory = $true)][string[]]$Arguments
  )

  $stdoutPath = Join-Path $LogRoot "$Id.stdout.log"
  $stderrPath = Join-Path $LogRoot "$Id.stderr.log"
  [System.IO.File]::WriteAllText($stdoutPath, "", $Utf8NoBom)
  [System.IO.File]::WriteAllText($stderrPath, "", $Utf8NoBom)
  $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()

  try {
    $escapedArguments = @($Arguments | ForEach-Object {
      if ($_ -match '[\s"]') {
        '"' + ($_ -replace '(\\*)"', '$1$1\"' -replace '(\\+)$', '$1$1') + '"'
      } else {
        $_
      }
    })
    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = $FilePath
    $startInfo.Arguments = $escapedArguments -join " "
    $startInfo.WorkingDirectory = $RepoRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
      throw "$Id could not start."
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $stdout = $stdoutTask.GetAwaiter().GetResult()
    $stderr = $stderrTask.GetAwaiter().GetResult()
    [System.IO.File]::WriteAllText($stdoutPath, $stdout, $Utf8NoBom)
    [System.IO.File]::WriteAllText($stderrPath, $stderr, $Utf8NoBom)
    $stopwatch.Stop()
    if ($process.ExitCode -ne 0) {
      Add-Result `
        -Id $Id `
        -Status failed `
        -Coverage $Coverage `
        -Detail "Process exited with code $($process.ExitCode)." `
        -DurationMs $stopwatch.ElapsedMilliseconds `
        -StdoutLog (Get-RelativeEvidencePath $stdoutPath) `
        -StderrLog (Get-RelativeEvidencePath $stderrPath)
      throw "$Id failed with exit code $($process.ExitCode)"
    }

    Add-Result `
      -Id $Id `
      -Status passed `
      -Coverage $Coverage `
      -Detail "Process exited with code 0." `
      -DurationMs $stopwatch.ElapsedMilliseconds `
      -StdoutLog (Get-RelativeEvidencePath $stdoutPath) `
      -StderrLog (Get-RelativeEvidencePath $stderrPath)
  } catch {
    if ($stopwatch.IsRunning) {
      $stopwatch.Stop()
    }
    if (-not ($Results | Where-Object { $_.id -eq $Id })) {
      Add-Result `
        -Id $Id `
        -Status failed `
        -Coverage $Coverage `
        -Detail $_.Exception.Message `
        -DurationMs $stopwatch.ElapsedMilliseconds `
        -StdoutLog (Get-RelativeEvidencePath $stdoutPath) `
        -StderrLog (Get-RelativeEvidencePath $stderrPath)
    }
    throw
  }
}

function Write-Summary {
  param(
    [Parameter(Mandatory = $true)][ValidateSet("passed", "failed")][string]$Status,
    [AllowNull()][string]$Failure
  )

  $head = $null
  try {
    $head = (& git -C $RepoRoot rev-parse HEAD 2>$null).Trim()
  } catch {
    $head = $null
  }

  $summary = [ordered]@{
    schemaVersion = 1
    suite = "contract-review-minimum-closed-loop"
    runId = $RunId
    status = $Status
    startedAt = $StartedAt.ToString("o")
    completedAt = (Get-Date).ToUniversalTime().ToString("o")
    gitHead = $head
    providerNetworkRequired = $false
    r2NetworkRequired = $false
    productionSourceModified = $false
    failure = $Failure
    preflight = $Preflight
    results = @($Results | ForEach-Object { $_ })
  }
  Write-JsonNoBom -Value $summary -Path $SummaryPath
}

Assert-SafeEvidencePath -Path $EvidenceRoot
New-Item -ItemType Directory -Force -Path $LogRoot | Out-Null

try {
  $requiredFiles = @(
    "src-tauri/tauri.conf.json",
    "src-tauri/src/lib.rs",
    "src-tauri/src/asset_service.rs",
    "src-tauri/src/contract_review_runtime.rs",
    "src-tauri/src/contract_review_service.rs",
    "src-tauri/src/r2_backup.rs",
    "src/client-sdk/DesktopHostAdapter.ts",
    "scripts/create-business-qa-fixtures.py"
  )
  foreach ($relativePath in $requiredFiles) {
    $fullPath = Join-Path $RepoRoot $relativePath
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
      throw "Required file is missing: $relativePath"
    }
  }

  Assert-FileContains `
    -Path (Join-Path $RepoRoot "src-tauri/src/lib.rs") `
    -Pattern 'fn\s+execute_contract_review_command\s*\(' `
    -Description "Tauri contract review command entry"
  Assert-FileContains `
    -Path (Join-Path $RepoRoot "src-tauri/src/lib.rs") `
    -Pattern 'generate_handler!\[[\s\S]*execute_contract_review_command' `
    -Description "Tauri invoke handler registration"
  Assert-FileContains `
    -Path (Join-Path $RepoRoot "src/client-sdk/DesktopHostAdapter.ts") `
    -Pattern 'invoke<ContractReviewCommandResponse>\([\s\S]*"execute_contract_review_command"' `
    -Description "Client SDK to Tauri adapter mapping"

  $testMarkers = @(
    "real_standard_low_risk_docx_completes_html_and_docx_closure",
    "restart_preserves_complete_review_graph",
    "command_receipt_replays_without_duplicate_state_or_event",
    "backup_outbox_queue_failure_never_downgrades_completed_local_review",
    "transport_failure_only_fails_backup_and_preserves_local_asset"
  )
  $combinedRustSource = @(
    Get-Content -LiteralPath (Join-Path $RepoRoot "src-tauri/src/contract_review_runtime.rs") -Raw -Encoding UTF8
    Get-Content -LiteralPath (Join-Path $RepoRoot "src-tauri/src/contract_review_service.rs") -Raw -Encoding UTF8
    Get-Content -LiteralPath (Join-Path $RepoRoot "src-tauri/src/r2_backup.rs") -Raw -Encoding UTF8
  ) -join "`n"
  foreach ($marker in $testMarkers) {
    if ($combinedRustSource -notmatch [regex]::Escape($marker)) {
      throw "Required acceptance test is missing: $marker"
    }
  }

  $tauriConfig = Get-Content -LiteralPath (Join-Path $RepoRoot "src-tauri/tauri.conf.json") -Raw -Encoding UTF8 | ConvertFrom-Json
  if (@($tauriConfig.bundle.targets) -notcontains "nsis") {
    throw "The desktop package must expose an NSIS bundle target."
  }
  $installer = Get-ChildItem -LiteralPath (Join-Path $RepoRoot "release/$($tauriConfig.version)") -Filter "*setup.exe" -File -ErrorAction SilentlyContinue | Select-Object -First 1
  $installerRecord = if ($null -eq $installer) {
    [ordered]@{ present = $false; file = $null; sha256 = $null }
  } else {
    [ordered]@{
      present = $true
      file = "release/$($tauriConfig.version)/$($installer.Name)"
      sha256 = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    }
  }

  $Preflight = [ordered]@{
    productName = [string]$tauriConfig.productName
    version = [string]$tauriConfig.version
    identifier = [string]$tauriConfig.identifier
    bundleTarget = "nsis"
    tauriCommand = "execute_contract_review_command"
    clientTransport = "DesktopHostAdapter -> Tauri IPC"
    installedCliContractReviewEntry = $false
    executionMode = "production Rust service layer through deterministic tests"
    installer = $installerRecord
    selectedTests = $testMarkers
  }
  Write-JsonNoBom -Value $Preflight -Path $PreflightPath
  Add-Result `
    -Id "static-interface" `
    -Status passed `
    -Coverage "Tauri command, Client SDK adapter, Rust fixtures, NSIS package interface" `
    -Detail "Static command and package interfaces are present; the installed executable has no contract-review CLI entry."

  $python = Get-Command python -ErrorAction Stop
  Invoke-CheckedProcess `
    -Id "fixture-generation" `
    -Coverage "Deterministic DOCX contract input" `
    -FilePath $python.Source `
    -Arguments @("scripts/create-business-qa-fixtures.py")

  $manifestPath = Join-Path $RepoRoot ".runtime/qa-fixtures/manifest.json"
  $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
  $fixture = @($manifest.fixtures | Where-Object { $_.id -eq "contract-standard-low-risk" })
  if ($fixture.Count -ne 1) {
    throw "QA fixture manifest must contain exactly one contract-standard-low-risk record."
  }
  $fixturePath = Join-Path (Split-Path $manifestPath -Parent) $fixture[0].file
  $actualHash = (Get-FileHash -LiteralPath $fixturePath -Algorithm SHA256).Hash.ToLowerInvariant()
  $actualSize = (Get-Item -LiteralPath $fixturePath).Length
  if ($actualHash -ne [string]$fixture[0].sha256 -or $actualSize -ne [long]$fixture[0].byteSize) {
    throw "Generated contract fixture does not match its manifest."
  }
  Add-Result `
    -Id "fixture-integrity" `
    -Status passed `
    -Coverage "Input hash and byte-size authority" `
    -Detail "Standard contract DOCX matches manifest SHA-256 and byte size."

  $cargo = Get-Command cargo -ErrorAction Stop
  $manifestArguments = @("test", "--manifest-path", "src-tauri/Cargo.toml", "--lib")
  $tests = @(
    [ordered]@{
      id = "closed-loop"
      name = "contract_review_runtime::tests::real_standard_low_risk_docx_completes_html_and_docx_closure"
      coverage = "Import -> Vault -> extraction -> rules -> Agent unavailable degradation -> human decisions -> HTML/DOCX report Artifact -> Vault -> backup outbox"
    },
    [ordered]@{
      id = "restart-persistence"
      name = "contract_review_service::tests::restart_preserves_complete_review_graph"
      coverage = "SQLite local record and complete review graph survive reopen"
    },
    [ordered]@{
      id = "command-idempotency"
      name = "contract_review_service::tests::command_receipt_replays_without_duplicate_state_or_event"
      coverage = "Idempotency receipt replay without duplicate state or events"
    },
    [ordered]@{
      id = "backup-outbox-nonblocking"
      name = "contract_review_runtime::tests::backup_outbox_queue_failure_never_downgrades_completed_local_review"
      coverage = "Backup outbox failure cannot downgrade completed local review or remove Vault artifacts"
    },
    [ordered]@{
      id = "r2-network-nonblocking"
      name = "r2_backup::tests::transport_failure_only_fails_backup_and_preserves_local_asset"
      coverage = "R2 transport failure affects backup state only; Local Vault and asset record remain authoritative"
    }
  )

  foreach ($test in $tests) {
    Invoke-CheckedProcess `
      -Id $test.id `
      -Coverage $test.coverage `
      -FilePath $cargo.Source `
      -Arguments ($manifestArguments + @($test.name, "--", "--exact", "--nocapture"))
  }

  Write-Summary -Status passed -Failure $null
  Write-Host "Contract review E2E acceptance passed."
  Write-Host "Evidence: $SummaryPath"
  exit 0
} catch {
  $failure = $_.Exception.Message
  try {
    Write-Summary -Status failed -Failure $failure
  } catch {
    Write-Error "Acceptance failed and summary writing also failed: $($_.Exception.Message)"
  }
  Write-Error "Contract review E2E acceptance failed: $failure"
  exit 1
}
