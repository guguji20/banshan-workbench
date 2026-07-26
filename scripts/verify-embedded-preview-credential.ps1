[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][ValidateNotNullOrEmpty()][string]$StatePath,
  [string]$ExpectedProviderId = "bsaigc",
  [string]$ExpectedBaseUrl = "https://bsaigc.dpdns.org/v1",
  [string]$ExpectedModel = "gpt-5.6-sol",
  [int]$ExpectedSchemaVersion = 2
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-RequiredPropertyValue {
  param(
    [Parameter(Mandatory = $true)]$Object,
    [Parameter(Mandatory = $true)][string]$Name
  )

  $property = $Object.PSObject.Properties[$Name]
  if ($null -eq $property) {
    throw "Credential probe failed: required state field is missing."
  }
  return $property.Value
}

function Clear-ParsedApiKeys {
  param([AllowNull()]$Document)

  if ($null -eq $Document) {
    return
  }
  $providersProperty = $Document.PSObject.Properties["providers"]
  if ($null -eq $providersProperty) {
    return
  }
  foreach ($provider in @($providersProperty.Value)) {
    if ($null -eq $provider) {
      continue
    }
    $apiKeyProperty = $provider.PSObject.Properties["apiKey"]
    if ($null -ne $apiKeyProperty) {
      $apiKeyProperty.Value = $null
    }
  }
}

if ($ExpectedSchemaVersion -lt 1) {
  throw "Credential probe failed: expected schema version is invalid."
}
if ([string]::IsNullOrWhiteSpace($ExpectedProviderId)) {
  throw "Credential probe failed: expected provider id is missing."
}
if ([string]::IsNullOrWhiteSpace($ExpectedBaseUrl) -or [string]::IsNullOrWhiteSpace($ExpectedModel)) {
  throw "Credential probe failed: expected endpoint or model is missing."
}

$resolvedStatePath = [System.IO.Path]::GetFullPath($StatePath)
if (-not (Test-Path -LiteralPath $resolvedStatePath -PathType Leaf)) {
  throw "Credential probe failed: protected state file was not created."
}

$encryptedBytes = $null
$plaintextBytes = $null
$jsonText = $null
$document = $null
try {
  $encryptedBytes = [System.IO.File]::ReadAllBytes($resolvedStatePath)
  if ($null -eq $encryptedBytes -or $encryptedBytes.Length -eq 0) {
    throw "Credential probe failed: protected state file is empty."
  }

  Add-Type -AssemblyName System.Security -ErrorAction Stop
  try {
    $plaintextBytes = [System.Security.Cryptography.ProtectedData]::Unprotect(
      $encryptedBytes,
      $null,
      [System.Security.Cryptography.DataProtectionScope]::CurrentUser
    )
  } catch {
    throw "Credential probe failed: current-user DPAPI decryption failed."
  }
  if ($null -eq $plaintextBytes -or $plaintextBytes.Length -eq 0) {
    throw "Credential probe failed: decrypted state is empty."
  }

  try {
    $jsonText = [System.Text.Encoding]::UTF8.GetString($plaintextBytes)
    $document = ConvertFrom-Json -InputObject $jsonText -ErrorAction Stop
  } catch {
    throw "Credential probe failed: decrypted state is not valid JSON."
  } finally {
    $jsonText = $null
  }

  $schemaVersion = [int](Get-RequiredPropertyValue -Object $document -Name "schemaVersion")
  $schemaValid = $schemaVersion -eq $ExpectedSchemaVersion
  if (-not $schemaValid) {
    throw "Credential probe failed: state schema version is not supported."
  }

  $defaultProviderId = [string](Get-RequiredPropertyValue -Object $document -Name "defaultProviderId")
  $defaultProviderValid = $defaultProviderId -ceq $ExpectedProviderId
  if (-not $defaultProviderValid) {
    throw "Credential probe failed: default provider is not the expected internal-preview provider."
  }

  $providers = @((Get-RequiredPropertyValue -Object $document -Name "providers"))
  $matchingProviders = @($providers | Where-Object {
    $idProperty = $_.PSObject.Properties["id"]
    $null -ne $idProperty -and [string]$idProperty.Value -ceq $ExpectedProviderId
  })
  if ($matchingProviders.Count -ne 1) {
    throw "Credential probe failed: expected provider record is missing or duplicated."
  }
  $provider = $matchingProviders[0]

  $providerEnabled = [bool](Get-RequiredPropertyValue -Object $provider -Name "enabled")
  if (-not $providerEnabled) {
    throw "Credential probe failed: expected provider is disabled."
  }

  $apiKeyProperty = $provider.PSObject.Properties["apiKey"]
  $apiKeyConfigured = $null -ne $apiKeyProperty -and
    $null -ne $apiKeyProperty.Value -and
    -not [string]::IsNullOrWhiteSpace([string]$apiKeyProperty.Value)
  if ($null -ne $apiKeyProperty) {
    $apiKeyProperty.Value = $null
  }
  if (-not $apiKeyConfigured) {
    throw "Credential probe failed: expected provider has no embedded preview credential."
  }

  $baseUrl = [string](Get-RequiredPropertyValue -Object $provider -Name "baseUrl")
  [System.Uri]$baseUri = $null
  $httpsBaseUrl = [System.Uri]::TryCreate($baseUrl, [System.UriKind]::Absolute, [ref]$baseUri) -and
    $baseUri.Scheme -ceq [System.Uri]::UriSchemeHttps
  $baseUrlMatches = $httpsBaseUrl -and $baseUrl.TrimEnd('/') -ceq $ExpectedBaseUrl.TrimEnd('/')
  $baseUrl = $null
  $baseUri = $null
  if (-not $httpsBaseUrl) {
    throw "Credential probe failed: expected provider base URL is not absolute HTTPS."
  }
  if (-not $baseUrlMatches) {
    throw "Credential probe failed: internal-preview provider endpoint does not match the release contract."
  }

  $stateDefaultModel = [string](Get-RequiredPropertyValue -Object $document -Name "defaultModel")
  $providerDefaultModel = [string](Get-RequiredPropertyValue -Object $provider -Name "defaultModel")
  $defaultModelConfigured = -not [string]::IsNullOrWhiteSpace($stateDefaultModel) -and
    -not [string]::IsNullOrWhiteSpace($providerDefaultModel) -and
    $stateDefaultModel -ceq $providerDefaultModel
  $defaultModelMatches = $defaultModelConfigured -and $stateDefaultModel -ceq $ExpectedModel
  if (-not $defaultModelConfigured) {
    throw "Credential probe failed: default model is missing or inconsistent."
  }
  if (-not $defaultModelMatches) {
    throw "Credential probe failed: internal-preview default model does not match the release contract."
  }

  $models = @((Get-RequiredPropertyValue -Object $provider -Name "models"))
  $defaultModelListed = @($models | Where-Object { [string]$_ -ceq $stateDefaultModel }).Count -gt 0
  $stateDefaultModel = $null
  $providerDefaultModel = $null
  $models = $null
  if (-not $defaultModelListed) {
    throw "Credential probe failed: default model is not present in the provider model list."
  }

  return [pscustomobject][ordered]@{
    schemaVersion = $schemaVersion
    providerId = $ExpectedProviderId
    decrypted = $true
    schemaValid = $schemaValid
    defaultProviderValid = $defaultProviderValid
    providerEnabled = $providerEnabled
    apiKeyConfigured = $apiKeyConfigured
    httpsBaseUrl = $httpsBaseUrl
    baseUrlMatches = $baseUrlMatches
    defaultModelConfigured = $defaultModelConfigured
    defaultModelMatches = $defaultModelMatches
    defaultModelListed = $defaultModelListed
  }
} finally {
  Clear-ParsedApiKeys -Document $document
  $document = $null
  $jsonText = $null
  if ($null -ne $plaintextBytes) {
    [System.Array]::Clear($plaintextBytes, 0, $plaintextBytes.Length)
    $plaintextBytes = $null
  }
  if ($null -ne $encryptedBytes) {
    [System.Array]::Clear($encryptedBytes, 0, $encryptedBytes.Length)
    $encryptedBytes = $null
  }
}
