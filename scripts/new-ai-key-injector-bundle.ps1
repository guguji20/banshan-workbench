[CmdletBinding()]
param(
  [string]$ConfigPath = "",
  [string]$OutputDirectory = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$runtimeRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot ".runtime"))
if ([string]::IsNullOrWhiteSpace($ConfigPath)) {
  $ConfigPath = Join-Path $runtimeRoot "internal-preview-build.json"
}
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
  $OutputDirectory = Join-Path $runtimeRoot "colleague-test\$(Get-Date -Format yyyyMMdd)\key-injector"
}

$resolvedConfig = [System.IO.Path]::GetFullPath($ConfigPath)
$resolvedOutput = [System.IO.Path]::GetFullPath($OutputDirectory)
$runtimePrefix = $runtimeRoot.TrimEnd('\') + '\'
if (-not $resolvedConfig.StartsWith($runtimePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
  throw "KEY source config must stay under .runtime."
}
if (-not $resolvedOutput.StartsWith($runtimePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
  throw "KEY injector output must stay under .runtime."
}
if (-not (Test-Path -LiteralPath $resolvedConfig -PathType Leaf)) {
  throw "KEY source config is missing."
}

$config = Get-Content -LiteralPath $resolvedConfig -Raw -Encoding UTF8 | ConvertFrom-Json
$apiKey = [string]$config.apiKey
$baseUrl = [string]$config.baseUrl
$model = [string]$config.model
if ([string]::IsNullOrWhiteSpace($apiKey) -or $apiKey.Trim().Length -lt 8) {
  throw "KEY source config does not contain a usable apiKey."
}
if ([string]::IsNullOrWhiteSpace($baseUrl)) { $baseUrl = "https://bsaigc.dpdns.org/v1" }
if ([string]::IsNullOrWhiteSpace($model)) { $model = "gpt-5.6-sol" }

New-Item -ItemType Directory -Path $resolvedOutput -Force | Out-Null
$injectorPath = Join-Path $resolvedOutput "install-key.ps1"
$launcherPath = Join-Path $resolvedOutput "双击注入KEY.cmd"
$readmePath = Join-Path $resolvedOutput "使用说明.txt"
$checksumPath = Join-Path $resolvedOutput "SHA256SUMS.txt"

function ConvertTo-Base64Utf8([string]$Value) {
  return [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($Value))
}

$apiKeyBase64 = ConvertTo-Base64Utf8 $apiKey.Trim()
$baseUrlBase64 = ConvertTo-Base64Utf8 $baseUrl.Trim().TrimEnd('/')
$modelBase64 = ConvertTo-Base64Utf8 $model.Trim()

$injectorTemplate = @'
[CmdletBinding()]
param(
  [string]$DataRoot = "",
  [switch]$NoDialog,
  [switch]$SkipRunningCheck
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Decode-Value([string]$Value) {
  return [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Value))
}

function Show-Result([string]$Message, [bool]$Success) {
  if ($NoDialog) {
    Write-Host $Message
    return
  }
  try {
    Add-Type -AssemblyName PresentationFramework -ErrorAction Stop
    $icon = if ($Success) { "Information" } else { "Error" }
    [System.Windows.MessageBox]::Show($Message, "华邦互娱商务系统 KEY 配置", "OK", $icon) | Out-Null
  } catch {
    Write-Host $Message
  }
}

try {
  $isWindows = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
  )
  if (-not $isWindows) { throw "此注入脚本仅支持 Windows。" }
  if (-not $SkipRunningCheck) {
    $running = @(Get-Process -Name "bsaigc_desktop" -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) { throw "请先退出华邦互娱商务系统，再双击运行本脚本。" }
  }

  if ([string]::IsNullOrWhiteSpace($DataRoot)) {
    $DataRoot = Join-Path $env:APPDATA "com.banshan.aigc.desktop"
  }
  $DataRoot = [System.IO.Path]::GetFullPath($DataRoot)
  $credentialRoot = Join-Path $DataRoot "credentials"
  $statePath = Join-Path $credentialRoot "provider-key.dpapi"
  New-Item -ItemType Directory -Path $credentialRoot -Force | Out-Null

  Add-Type -AssemblyName System.Security
  $apiKey = Decode-Value "__API_KEY_BASE64__"
  $baseUrl = Decode-Value "__BASE_URL_BASE64__"
  $model = Decode-Value "__MODEL_BASE64__"
  $now = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()

  $state = $null
  if (Test-Path -LiteralPath $statePath -PathType Leaf) {
    $encrypted = [IO.File]::ReadAllBytes($statePath)
    $plaintext = [Security.Cryptography.ProtectedData]::Unprotect(
      $encrypted,
      $null,
      [Security.Cryptography.DataProtectionScope]::CurrentUser
    )
    $state = [Text.Encoding]::UTF8.GetString($plaintext) | ConvertFrom-Json
    if ([int]$state.schemaVersion -ne 2) {
      throw "检测到不兼容的旧版 KEY 数据，请先启动新版应用完成迁移后再运行。"
    }
  } else {
    $state = [pscustomobject][ordered]@{
      schemaVersion = 2
      defaultProviderId = "bsaigc"
      defaultModel = $model
      providers = @()
      revision = 0
      updatedAt = $null
      receipts = @()
    }
  }

  $providers = @($state.providers)
  $provider = $providers | Where-Object { [string]$_.id -eq "bsaigc" } | Select-Object -First 1
  if ($null -eq $provider) {
    $provider = [pscustomobject][ordered]@{
      id = "bsaigc"
      name = "华邦互娱 AI"
      kind = "openAiCompatible"
      baseUrl = $baseUrl
      apiKey = $apiKey
      models = @($model)
      defaultModel = $model
      enabled = $true
      connection = [pscustomobject][ordered]@{
        state = "untested"
        message = "KEY 已通过独立配置包写入，请在设置中测试连接"
        latencyMs = $null
        testedAt = $null
        discoveredModels = @()
      }
      createdAt = $now
      updatedAt = $now
    }
    $providers += $provider
    $state.providers = $providers
  } else {
    $provider.name = "华邦互娱 AI"
    $provider.kind = "openAiCompatible"
    $provider.baseUrl = $baseUrl
    $provider.apiKey = $apiKey
    $provider.models = @($model)
    $provider.defaultModel = $model
    $provider.enabled = $true
    $provider.connection = [pscustomobject][ordered]@{
      state = "untested"
      message = "KEY 已通过独立配置包更新，请在设置中测试连接"
      latencyMs = $null
      testedAt = $null
      discoveredModels = @()
    }
    $provider.updatedAt = $now
  }

  $state.defaultProviderId = "bsaigc"
  $state.defaultModel = $model
  $state.revision = [int64]$state.revision + 1
  $state.updatedAt = $now
  $json = $state | ConvertTo-Json -Depth 30 -Compress
  $bytes = [Text.Encoding]::UTF8.GetBytes($json)
  $protected = [Security.Cryptography.ProtectedData]::Protect(
    $bytes,
    $null,
    [Security.Cryptography.DataProtectionScope]::CurrentUser
  )

  $tempPath = Join-Path $credentialRoot ("provider-key.{0}.tmp" -f [guid]::NewGuid().ToString("N"))
  [IO.File]::WriteAllBytes($tempPath, $protected)
  if (Test-Path -LiteralPath $statePath -PathType Leaf) {
    $backupPath = Join-Path $credentialRoot ("provider-key.backup-{0}.dpapi" -f (Get-Date -Format "yyyyMMdd-HHmmss"))
    Copy-Item -LiteralPath $statePath -Destination $backupPath -Force
  }
  Move-Item -LiteralPath $tempPath -Destination $statePath -Force

  $verificationEncrypted = [IO.File]::ReadAllBytes($statePath)
  $verificationPlaintext = [Security.Cryptography.ProtectedData]::Unprotect(
    $verificationEncrypted,
    $null,
    [Security.Cryptography.DataProtectionScope]::CurrentUser
  )
  $verification = [Text.Encoding]::UTF8.GetString($verificationPlaintext) | ConvertFrom-Json
  $savedProvider = @($verification.providers) | Where-Object { [string]$_.id -eq "bsaigc" } | Select-Object -First 1
  if ($null -eq $savedProvider -or [string]$savedProvider.apiKey -cne $apiKey) {
    throw "KEY 写入后的自检失败。"
  }

  Show-Result "KEY 已安全写入当前 Windows 用户。现在可以启动华邦互娱商务系统。" $true
  exit 0
} catch {
  Show-Result ("KEY 注入失败：`n" + $_.Exception.Message) $false
  exit 1
}
'@

$injectorContent = $injectorTemplate.Replace("__API_KEY_BASE64__", $apiKeyBase64).Replace("__BASE_URL_BASE64__", $baseUrlBase64).Replace("__MODEL_BASE64__", $modelBase64)
$utf8Bom = New-Object System.Text.UTF8Encoding($true)
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText($injectorPath, $injectorContent, $utf8Bom)

$launcherContent = "@echo off`r`nchcp 65001 >nul`r`npowershell.exe -NoProfile -ExecutionPolicy Bypass -File `"%~dp0install-key.ps1`"`r`nif errorlevel 1 pause`r`n"
[IO.File]::WriteAllText($launcherPath, $launcherContent, $utf8NoBom)

$readme = @"
华邦互娱商务系统 - Windows KEY 独立注入包

使用方法：
1. 先安装并完全退出“华邦互娱商务系统”。
2. 双击“$([IO.Path]::GetFileName($launcherPath))”。
3. 看到成功提示后重新启动应用，在设置中测试 AI 连接。

安全说明：
- 安装包本身不包含 KEY；KEY 只在本文件夹的 install-key.ps1 中携带。
- 注入后使用 Windows 当前用户 DPAPI 加密保存，其他 Windows 用户无法直接解密。
- 本文件夹属于机密资料，不要上传 GitHub、网盘公开链接或群聊。
- 每位同事应在自己的 Windows 登录账户下运行一次。
- 更新 KEY 时重新运行即可；旧加密文件会保留一份时间戳备份。
"@
[IO.File]::WriteAllText($readmePath, $readme, $utf8Bom)

$checksumLines = @($injectorPath, $launcherPath, $readmePath) | ForEach-Object {
  $hash = (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()
  "$hash *$([IO.Path]::GetFileName($_))"
}
[IO.File]::WriteAllText($checksumPath, (($checksumLines -join "`n") + "`n"), $utf8NoBom)

Write-Host "KEY injector bundle created: $resolvedOutput"
Write-Host "The secret was not printed. Do not commit or upload this directory."
