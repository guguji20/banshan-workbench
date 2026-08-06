use crate::protocol::{
    AiCredentialCommandEnvelope, AiCredentialCommandResponse, AiCredentialProtection,
    AiCredentialStatus, AiProviderConnectionState, AiProviderConnectionStatus, AiProviderKind,
    AiProviderRecord, CommandReceipt, DiscoverAiProviderModelsPayload, HostError,
    UpsertAiProviderPayload, AI_CREDENTIAL_PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const PRODUCT_PROVIDER_ID: &str = "bsaigc";
const LEGACY_PRODUCT_PROVIDER_NAME: &str = "半山 AIGC";
const PRODUCT_PROVIDER_NAME: &str = "华邦互娱 AI";
const PRODUCT_PROVIDER_BASE_URL: &str = "https://bsaigc.dpdns.org/v1";
const PRODUCT_DEFAULT_MODEL: &str = "gpt-5.6-sol";
const AGGREGATE_ID: &str = "ai-provider-settings";
const LEGACY_STATE_SCHEMA_VERSION: u32 = 1;
const STATE_SCHEMA_VERSION: u32 = 2;
const MAX_STATE_BYTES: usize = 1_048_576;
const MAX_RECEIPTS: usize = 64;
const MAX_PROVIDERS: usize = 32;
const MAX_MODELS_PER_PROVIDER: usize = 128;
const MAX_NAME_BYTES: usize = 128;
const MAX_URL_BYTES: usize = 2048;
const MAX_MODEL_BYTES: usize = 256;
const MIN_API_KEY_BYTES: usize = 8;
const MAX_API_KEY_BYTES: usize = 4096;
const MAX_MODEL_RESPONSE_BYTES: usize = 1_048_576;
const PROVIDER_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const PROVIDER_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub(crate) struct AiCredentialService {
    state_path: PathBuf,
    lock: std::sync::Arc<Mutex<()>>,
}

pub(crate) struct AiRuntimeProviderConfig {
    pub(crate) provider_id: String,
    pub(crate) base_url: String,
    pub(crate) model: String,
    pub(crate) api_key: Zeroizing<String>,
}

impl std::fmt::Debug for AiRuntimeProviderConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AiRuntimeProviderConfig")
            .field("provider_id", &self.provider_id)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecureCredentialState {
    schema_version: u32,
    default_provider_id: Option<String>,
    default_model: Option<String>,
    providers: Vec<SecureProviderState>,
    revision: i64,
    updated_at: Option<i64>,
    receipts: Vec<SecureCommandReceipt>,
}

impl Default for SecureCredentialState {
    fn default() -> Self {
        let now = now_millis();
        let product = default_product_provider(now);
        Self {
            schema_version: STATE_SCHEMA_VERSION,
            default_provider_id: Some(product.id.clone()),
            default_model: Some(product.default_model.clone()),
            providers: vec![product],
            revision: 0,
            updated_at: None,
            receipts: Vec::new(),
        }
    }
}

impl Drop for SecureCredentialState {
    fn drop(&mut self) {
        for provider in &mut self.providers {
            if let Some(api_key) = provider.api_key.as_mut() {
                api_key.zeroize();
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SecureProviderState {
    id: String,
    name: String,
    kind: AiProviderKind,
    base_url: String,
    api_key: Option<String>,
    models: Vec<String>,
    default_model: String,
    enabled: bool,
    connection: AiProviderConnectionStatus,
    created_at: i64,
    updated_at: i64,
}

impl Drop for SecureProviderState {
    fn drop(&mut self) {
        if let Some(api_key) = self.api_key.as_mut() {
            api_key.zeroize();
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySecureCredentialState {
    schema_version: u32,
    provider: String,
    api_key: Option<String>,
    revision: i64,
    updated_at: Option<i64>,
    #[serde(default)]
    receipts: Vec<LegacySecureCommandReceipt>,
}

impl Drop for LegacySecureCredentialState {
    fn drop(&mut self) {
        if let Some(api_key) = self.api_key.as_mut() {
            api_key.zeroize();
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySecureCommandReceipt {
    command_id: String,
    idempotency_key: String,
    command_type: String,
    request_fingerprint: String,
    response: LegacyAiCredentialCommandResponse,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyAiCredentialCommandResponse {
    receipt: CommandReceipt,
    status: LegacyAiCredentialStatus,
    replayed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyAiCredentialStatus {
    provider: String,
    configured: bool,
    persisted: bool,
    protection: Option<AiCredentialProtection>,
    revision: i64,
    updated_at: Option<i64>,
    applies_on_next_runtime_start: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecureCommandReceipt {
    command_id: String,
    idempotency_key: String,
    command_type: String,
    request_fingerprint: String,
    response: AiCredentialCommandResponse,
}

struct CommandMeta<'a> {
    command_id: &'a str,
    protocol_version: &'a str,
    idempotency_key: &'a str,
    expected_revision: Option<i64>,
    deadline_at: Option<i64>,
    command_type: &'static str,
}

struct MutationOutcome {
    changed: bool,
    runtime_changed: bool,
    connection_test: Option<AiProviderConnectionStatus>,
}

#[derive(Deserialize)]
struct ProviderModelListResponse {
    data: Vec<ProviderModelRecord>,
}

#[derive(Deserialize)]
struct ProviderModelRecord {
    id: String,
}

#[derive(Debug)]
struct ProviderProbeSuccess {
    models: Vec<String>,
    truncated: bool,
}

#[derive(Debug)]
enum ProviderProbeFailure {
    Unauthorized,
    Forbidden,
    NotFound,
    RateLimited,
    Http(u16),
    Timeout,
    Network,
    ResponseTooLarge,
    InvalidResponse,
}

impl ProviderProbeFailure {
    fn message(&self) -> String {
        match self {
            Self::Unauthorized => "API Key 无效或未被服务接受（HTTP 401）".to_string(),
            Self::Forbidden => "当前 API Key 没有读取模型列表的权限（HTTP 403）".to_string(),
            Self::NotFound => {
                "未找到模型列表接口，请确认 Base URL 包含正确的 /v1 路径（HTTP 404）".to_string()
            }
            Self::RateLimited => "服务暂时限流，请稍后重试（HTTP 429）".to_string(),
            Self::Http(status) => format!("模型列表请求失败（HTTP {status}）"),
            Self::Timeout => "连接服务超时，请检查网络或 Base URL".to_string(),
            Self::Network => "无法连接到服务，请检查网络、证书或 Base URL".to_string(),
            Self::ResponseTooLarge => "服务返回的模型列表过大，已停止读取".to_string(),
            Self::InvalidResponse => "服务返回的模型列表格式无效，应为 data[].id".to_string(),
        }
    }
}

impl MutationOutcome {
    fn unchanged() -> Self {
        Self {
            changed: false,
            runtime_changed: false,
            connection_test: None,
        }
    }
}

impl AiCredentialService {
    pub(crate) fn open(data_root: &Path) -> Result<Self, HostError> {
        let credential_root = data_root.join("credentials");
        fs::create_dir_all(&credential_root).map_err(|_| {
            HostError::new(
                "AI_CREDENTIAL_STORAGE_UNAVAILABLE",
                "无法准备本地 AI 凭据存储。",
                true,
            )
        })?;
        let service = Self {
            state_path: credential_root.join("provider-key.dpapi"),
            lock: std::sync::Arc::new(Mutex::new(())),
        };

        // Internal preview builds can inject a first-run key at compile time without
        // writing it to source control. Once a state file exists, user choices win.
        if !service.state_path.exists() && embedded_provider_api_key().is_some() {
            let state = SecureCredentialState::default();
            service.write_state(&state)?;
        }
        Ok(service)
    }

    #[cfg(test)]
    pub(crate) fn status(&self) -> Result<AiCredentialStatus, HostError> {
        let _guard = self.lock.lock().map_err(|_| storage_lock_error())?;
        let state = self.read_state()?;
        Ok(status_from_state(&state, false))
    }

    #[cfg(test)]
    pub(crate) fn load_api_key(&self) -> Result<Option<Zeroizing<String>>, HostError> {
        Ok(self
            .load_runtime_provider()?
            .map(|provider| provider.api_key))
    }

    pub(crate) fn load_runtime_provider(
        &self,
    ) -> Result<Option<AiRuntimeProviderConfig>, HostError> {
        let _guard = self.lock.lock().map_err(|_| storage_lock_error())?;
        let state = self.read_state()?;
        let Some(provider) = selected_provider(&state).filter(|provider| provider.enabled) else {
            return Ok(None);
        };
        let Some(api_key) = provider
            .api_key
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(None);
        };
        let model = state
            .default_model
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(provider.default_model.as_str())
            .to_string();
        Ok(Some(AiRuntimeProviderConfig {
            provider_id: provider.id.clone(),
            base_url: provider.base_url.clone(),
            model,
            api_key: Zeroizing::new(api_key.clone()),
        }))
    }

    pub(crate) fn execute(
        &self,
        command: AiCredentialCommandEnvelope,
    ) -> Result<AiCredentialCommandResponse, HostError> {
        validate_command(&command)?;
        let meta = command_meta(&command);
        validate_deadline(meta.deadline_at)?;

        let _guard = self.lock.lock().map_err(|_| storage_lock_error())?;
        let mut state = self.read_state()?;

        if matches!(&command, AiCredentialCommandEnvelope::Status { .. }) {
            validate_expected_revision(meta.expected_revision, state.revision)?;
            let status = status_from_state(&state, false);
            return Ok(AiCredentialCommandResponse {
                receipt: command_receipt(&meta, status.revision, now_millis()),
                status,
                connection_test: None,
                replayed: false,
            });
        }

        let fingerprint = request_fingerprint(&command);
        if let Some(response) = replay_receipt(&state, &meta, &fingerprint)? {
            return Ok(response);
        }
        validate_expected_revision(meta.expected_revision, state.revision)?;

        let now = now_millis();
        let outcome = match &command {
            AiCredentialCommandEnvelope::UpsertProvider { payload, .. } => {
                upsert_provider(&mut state, payload, now)?
            }
            AiCredentialCommandEnvelope::RemoveProvider { payload, .. } => {
                remove_provider(&mut state, &payload.provider_id)?
            }
            AiCredentialCommandEnvelope::SelectProvider { payload, .. } => {
                select_provider(&mut state, &payload.provider_id, &payload.model, now)?
            }
            AiCredentialCommandEnvelope::TestProvider { payload, .. } => {
                test_provider(&mut state, &payload.provider_id, now)?
            }
            AiCredentialCommandEnvelope::DiscoverModels { payload, .. } => {
                discover_provider_models(&mut state, payload, now)?
            }
            AiCredentialCommandEnvelope::ClearProviderApiKey { payload, .. } => {
                clear_provider_api_key(&mut state, &payload.provider_id, now)?
            }
            AiCredentialCommandEnvelope::SaveBsaigcApiKey { payload, .. } => {
                save_legacy_api_key(&mut state, &payload.api_key, now)?
            }
            AiCredentialCommandEnvelope::ClearBsaigcApiKey { .. } => {
                clear_legacy_api_key(&mut state, now)?
            }
            AiCredentialCommandEnvelope::Status { .. } => unreachable!(),
        };

        if outcome.changed {
            state.revision = next_revision(state.revision)?;
            state.updated_at = Some(now);
        }
        let status = status_from_state(&state, outcome.runtime_changed);
        let response = AiCredentialCommandResponse {
            receipt: command_receipt(&meta, state.revision, now),
            status,
            connection_test: outcome.connection_test,
            replayed: false,
        };
        state.receipts.push(SecureCommandReceipt {
            command_id: meta.command_id.to_string(),
            idempotency_key: meta.idempotency_key.to_string(),
            command_type: meta.command_type.to_string(),
            request_fingerprint: fingerprint,
            response: response.clone(),
        });
        if state.receipts.len() > MAX_RECEIPTS {
            let excess = state.receipts.len() - MAX_RECEIPTS;
            state.receipts.drain(0..excess);
        }
        self.write_state(&state)?;
        Ok(response)
    }

    fn read_state(&self) -> Result<SecureCredentialState, HostError> {
        let encrypted = match fs::read(&self.state_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SecureCredentialState::default());
            }
            Err(_) => return Err(storage_read_error()),
        };
        if encrypted.is_empty() || encrypted.len() > MAX_STATE_BYTES {
            return Err(storage_corrupt_error());
        }
        let decrypted = Zeroizing::new(unprotect_current_user(&encrypted)?);
        let document: serde_json::Value =
            serde_json::from_slice(&decrypted).map_err(|_| storage_corrupt_error())?;
        let schema_version = document
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(storage_corrupt_error)?;

        match schema_version {
            STATE_SCHEMA_VERSION => {
                let mut state: SecureCredentialState =
                    serde_json::from_value(document).map_err(|_| storage_corrupt_error())?;
                if validate_stored_state(&mut state)? {
                    self.write_state(&state)?;
                }
                Ok(state)
            }
            LEGACY_STATE_SCHEMA_VERSION => {
                let legacy: LegacySecureCredentialState =
                    serde_json::from_value(document).map_err(|_| storage_corrupt_error())?;
                let mut state = migrate_legacy_state(legacy)?;
                validate_stored_state(&mut state)?;
                self.write_state(&state)?;
                Ok(state)
            }
            _ => Err(storage_corrupt_error()),
        }
    }

    fn write_state(&self, state: &SecureCredentialState) -> Result<(), HostError> {
        let plaintext =
            Zeroizing::new(serde_json::to_vec(state).map_err(|_| storage_write_error())?);
        let encrypted = protect_current_user(&plaintext)?;
        if encrypted.is_empty() || encrypted.len() > MAX_STATE_BYTES {
            return Err(storage_write_error());
        }
        let parent = self.state_path.parent().ok_or_else(storage_write_error)?;
        fs::create_dir_all(parent).map_err(|_| storage_write_error())?;
        let temp_path = parent.join(format!("provider-key.{}.tmp", Uuid::new_v4()));
        let write_result = (|| -> Result<(), HostError> {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp_path)
                .map_err(|_| storage_write_error())?;
            file.write_all(&encrypted)
                .and_then(|_| file.sync_all())
                .map_err(|_| storage_write_error())?;
            replace_file_atomically(&temp_path, &self.state_path)
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result
    }
}

fn embedded_provider_api_key() -> Option<String> {
    option_env!("BSAIGC_INTERNAL_API_KEY")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn embedded_provider_base_url() -> String {
    option_env!("BSAIGC_INTERNAL_BASE_URL")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(PRODUCT_PROVIDER_BASE_URL)
        .trim_end_matches('/')
        .to_string()
}

fn embedded_provider_model() -> String {
    option_env!("BSAIGC_INTERNAL_MODEL")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(PRODUCT_DEFAULT_MODEL)
        .to_string()
}

fn default_product_provider(now: i64) -> SecureProviderState {
    let model = embedded_provider_model();
    SecureProviderState {
        id: PRODUCT_PROVIDER_ID.to_string(),
        name: PRODUCT_PROVIDER_NAME.to_string(),
        kind: AiProviderKind::OpenAiCompatible,
        base_url: embedded_provider_base_url(),
        api_key: embedded_provider_api_key(),
        models: vec![model.clone()],
        default_model: model,
        enabled: true,
        connection: untested_connection("尚未测试连接"),
        created_at: now,
        updated_at: now,
    }
}

fn command_meta(command: &AiCredentialCommandEnvelope) -> CommandMeta<'_> {
    macro_rules! meta {
        ($command_id:expr, $protocol_version:expr, $idempotency_key:expr, $expected_revision:expr, $deadline_at:expr, $command_type:expr) => {
            CommandMeta {
                command_id: $command_id,
                protocol_version: $protocol_version,
                idempotency_key: $idempotency_key,
                expected_revision: *$expected_revision,
                deadline_at: *$deadline_at,
                command_type: $command_type,
            }
        };
    }
    match command {
        AiCredentialCommandEnvelope::Status {
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => meta!(
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            "aiCredentials.status"
        ),
        AiCredentialCommandEnvelope::UpsertProvider {
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => meta!(
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            "aiCredentials.upsertProvider"
        ),
        AiCredentialCommandEnvelope::RemoveProvider {
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => meta!(
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            "aiCredentials.removeProvider"
        ),
        AiCredentialCommandEnvelope::SelectProvider {
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => meta!(
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            "aiCredentials.selectProvider"
        ),
        AiCredentialCommandEnvelope::TestProvider {
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => meta!(
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            "aiCredentials.testProvider"
        ),
        AiCredentialCommandEnvelope::DiscoverModels {
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => meta!(
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            "aiCredentials.discoverModels"
        ),
        AiCredentialCommandEnvelope::ClearProviderApiKey {
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => meta!(
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            "aiCredentials.clearProviderApiKey"
        ),
        AiCredentialCommandEnvelope::SaveBsaigcApiKey {
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => meta!(
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            "aiCredentials.saveBsaigcApiKey"
        ),
        AiCredentialCommandEnvelope::ClearBsaigcApiKey {
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => meta!(
            command_id,
            protocol_version,
            idempotency_key,
            expected_revision,
            deadline_at,
            "aiCredentials.clearBsaigcApiKey"
        ),
    }
}

fn validate_command(command: &AiCredentialCommandEnvelope) -> Result<(), HostError> {
    let meta = command_meta(command);
    validate_identifier("commandId", meta.command_id)?;
    validate_identifier("idempotencyKey", meta.idempotency_key)?;
    if meta.protocol_version != AI_CREDENTIAL_PROTOCOL_VERSION {
        return Err(HostError::new(
            "AI_CREDENTIAL_PROTOCOL_UNSUPPORTED",
            "当前客户端的 AI 配置协议版本不受支持，请升级应用。",
            false,
        ));
    }
    match command {
        AiCredentialCommandEnvelope::UpsertProvider { payload, .. } => {
            validate_upsert_payload(payload)?;
        }
        AiCredentialCommandEnvelope::RemoveProvider { payload, .. }
        | AiCredentialCommandEnvelope::TestProvider { payload, .. }
        | AiCredentialCommandEnvelope::ClearProviderApiKey { payload, .. } => {
            validate_provider_id(&payload.provider_id)?;
        }
        AiCredentialCommandEnvelope::DiscoverModels { payload, .. } => {
            validate_discover_models_payload(payload)?;
        }
        AiCredentialCommandEnvelope::SelectProvider { payload, .. } => {
            validate_provider_id(&payload.provider_id)?;
            validate_model(&payload.model)?;
        }
        AiCredentialCommandEnvelope::SaveBsaigcApiKey { payload, .. } => {
            validate_api_key(payload.api_key.trim())?;
        }
        AiCredentialCommandEnvelope::ClearBsaigcApiKey { .. }
        | AiCredentialCommandEnvelope::Status { .. } => {}
    }
    Ok(())
}

fn validate_upsert_payload(payload: &UpsertAiProviderPayload) -> Result<(), HostError> {
    if let Some(provider_id) = payload.provider_id.as_deref() {
        validate_provider_id(provider_id)?;
    }
    validate_bounded_text("供应商名称", &payload.name, MAX_NAME_BYTES)?;
    validate_base_url(&payload.base_url)?;
    if let Some(api_key) = payload.api_key.as_deref() {
        validate_api_key(api_key.trim())?;
    }
    if payload.models.len() > MAX_MODELS_PER_PROVIDER {
        return Err(HostError::validation("单个供应商的模型数量过多"));
    }
    validate_model(&payload.default_model)?;
    for model in &payload.models {
        validate_model(model)?;
    }
    Ok(())
}

fn validate_discover_models_payload(
    payload: &DiscoverAiProviderModelsPayload,
) -> Result<(), HostError> {
    if let Some(provider_id) = payload.provider_id.as_deref() {
        validate_provider_id(provider_id)?;
    }
    validate_base_url(&payload.base_url)?;
    if let Some(api_key) = payload.api_key.as_deref() {
        validate_api_key(api_key.trim())?;
    }
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> Result<(), HostError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(HostError::validation(format!("{label} 无效")));
    }
    Ok(())
}

fn validate_provider_id(value: &str) -> Result<(), HostError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(HostError::validation("providerId 无效"));
    }
    Ok(())
}

fn validate_bounded_text(label: &str, value: &str, max_bytes: usize) -> Result<(), HostError> {
    let value = value.trim();
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(HostError::validation(format!("{label} 无效")));
    }
    Ok(())
}

fn validate_model(value: &str) -> Result<(), HostError> {
    validate_bounded_text("模型名称", value, MAX_MODEL_BYTES)
}

fn validate_api_key(value: &str) -> Result<(), HostError> {
    if value.len() < MIN_API_KEY_BYTES
        || value.len() > MAX_API_KEY_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(HostError::new(
            "AI_CREDENTIAL_INVALID",
            "API Key 格式无效，请检查后重试。",
            false,
        ));
    }
    Ok(())
}

fn validate_base_url(value: &str) -> Result<String, HostError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_URL_BYTES
        || value.chars().any(char::is_whitespace)
        || value.contains('?')
        || value.contains('#')
    {
        return Err(HostError::validation("供应商地址无效"));
    }
    let (scheme, remainder) = value
        .split_once("://")
        .ok_or_else(|| HostError::validation("供应商地址必须是完整 HTTP(S) URL"))?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "https" && scheme != "http" {
        return Err(HostError::validation("供应商地址必须使用 HTTP 或 HTTPS"));
    }
    let authority = remainder.split('/').next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return Err(HostError::validation(
            "供应商地址必须包含主机且不能携带凭据",
        ));
    }
    let host = provider_authority_host(authority)?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false);
    if scheme == "http" && !loopback {
        return Err(HostError::validation("远程供应商必须使用 HTTPS"));
    }
    Ok(value.trim_end_matches('/').to_string())
}

fn provider_authority_host(authority: &str) -> Result<&str, HostError> {
    if let Some(bracketed) = authority.strip_prefix('[') {
        let end = bracketed
            .find(']')
            .ok_or_else(|| HostError::validation("供应商地址中的 IPv6 主机无效"))?;
        let host = &bracketed[..end];
        if host.parse::<IpAddr>().is_err() {
            return Err(HostError::validation("供应商地址中的 IP 无效"));
        }
        validate_provider_port(&bracketed[end + 1..])?;
        return Ok(host);
    }
    if authority.matches(':').count() > 1 {
        return Err(HostError::validation("IPv6 地址必须使用方括号"));
    }
    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, ""), |(host, port)| (host, port));
    if host.is_empty() {
        return Err(HostError::validation("供应商地址缺少主机"));
    }
    if authority.contains(':') {
        validate_provider_port(&format!(":{port}"))?;
    }
    Ok(host)
}

fn validate_provider_port(suffix: &str) -> Result<(), HostError> {
    if suffix.is_empty() {
        return Ok(());
    }
    let port = suffix
        .strip_prefix(':')
        .ok_or_else(|| HostError::validation("供应商端口无效"))?;
    if port.is_empty() || port.parse::<u16>().is_err() {
        return Err(HostError::validation("供应商端口无效"));
    }
    Ok(())
}

fn validate_deadline(deadline_at: Option<i64>) -> Result<(), HostError> {
    if deadline_at.is_some_and(|deadline| deadline <= now_millis()) {
        return Err(HostError::new(
            "COMMAND_DEADLINE_EXCEEDED",
            "AI 配置操作已超时，请重试。",
            true,
        ));
    }
    Ok(())
}

fn validate_expected_revision(expected: Option<i64>, actual: i64) -> Result<(), HostError> {
    if let Some(expected) = expected {
        if expected < 0 {
            return Err(HostError::validation("expectedRevision 不能小于 0"));
        }
        if expected != actual {
            return Err(HostError::new(
                "AI_CREDENTIAL_REVISION_CONFLICT",
                "AI 配置已在其他窗口更新，请刷新后重试。",
                false,
            ));
        }
    }
    Ok(())
}

fn request_fingerprint(command: &AiCredentialCommandEnvelope) -> String {
    let mut digest = Sha256::new();
    let meta = command_meta(command);
    digest.update(meta.command_type.as_bytes());
    digest.update([0]);
    match command {
        AiCredentialCommandEnvelope::UpsertProvider { payload, .. } => {
            digest_optional(&mut digest, payload.provider_id.as_deref());
            digest_value(&mut digest, payload.name.trim());
            digest_value(
                &mut digest,
                match payload.kind {
                    AiProviderKind::OpenAiCompatible => "openAiCompatible",
                },
            );
            digest_value(&mut digest, payload.base_url.trim());
            digest_optional(&mut digest, payload.api_key.as_deref().map(str::trim));
            for model in &payload.models {
                digest_value(&mut digest, model.trim());
            }
            digest_value(&mut digest, payload.default_model.trim());
            digest_value(&mut digest, if payload.set_default { "1" } else { "0" });
            digest_value(&mut digest, if payload.enabled { "1" } else { "0" });
        }
        AiCredentialCommandEnvelope::RemoveProvider { payload, .. }
        | AiCredentialCommandEnvelope::TestProvider { payload, .. }
        | AiCredentialCommandEnvelope::ClearProviderApiKey { payload, .. } => {
            digest_value(&mut digest, payload.provider_id.trim());
        }
        AiCredentialCommandEnvelope::DiscoverModels { payload, .. } => {
            digest_optional(&mut digest, payload.provider_id.as_deref().map(str::trim));
            digest_value(
                &mut digest,
                match payload.kind {
                    AiProviderKind::OpenAiCompatible => "openAiCompatible",
                },
            );
            digest_value(&mut digest, payload.base_url.trim());
            digest_optional(&mut digest, payload.api_key.as_deref().map(str::trim));
        }
        AiCredentialCommandEnvelope::SelectProvider { payload, .. } => {
            digest_value(&mut digest, payload.provider_id.trim());
            digest_value(&mut digest, payload.model.trim());
        }
        AiCredentialCommandEnvelope::SaveBsaigcApiKey { payload, .. } => {
            digest_value(&mut digest, payload.api_key.trim());
        }
        AiCredentialCommandEnvelope::ClearBsaigcApiKey { .. }
        | AiCredentialCommandEnvelope::Status { .. } => {}
    }
    format!("{:x}", digest.finalize())
}

fn digest_value(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value.as_bytes());
}

fn digest_optional(digest: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            digest.update([1]);
            digest_value(digest, value);
        }
        None => digest.update([0]),
    }
}

fn replay_receipt(
    state: &SecureCredentialState,
    meta: &CommandMeta<'_>,
    fingerprint: &str,
) -> Result<Option<AiCredentialCommandResponse>, HostError> {
    if let Some(receipt) = state
        .receipts
        .iter()
        .find(|receipt| receipt.idempotency_key == meta.idempotency_key)
    {
        if receipt.command_id != meta.command_id
            || receipt.command_type != meta.command_type
            || receipt.request_fingerprint != fingerprint
        {
            return Err(HostError::new(
                "AI_CREDENTIAL_IDEMPOTENCY_CONFLICT",
                "该操作标识已被其他 AI 配置请求使用。",
                false,
            ));
        }
        let mut response = receipt.response.clone();
        response.replayed = true;
        return Ok(Some(response));
    }
    if state
        .receipts
        .iter()
        .any(|receipt| receipt.command_id == meta.command_id)
    {
        return Err(HostError::new(
            "AI_CREDENTIAL_COMMAND_CONFLICT",
            "该命令编号已被使用，请重试。",
            false,
        ));
    }
    Ok(None)
}

fn upsert_provider(
    state: &mut SecureCredentialState,
    payload: &UpsertAiProviderPayload,
    now: i64,
) -> Result<MutationOutcome, HostError> {
    let provider_id = payload
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("provider-{}", Uuid::new_v4().simple()));
    validate_provider_id(&provider_id)?;

    let name = payload.name.trim().to_string();
    let base_url = validate_base_url(&payload.base_url)?;
    let models = normalize_models(&payload.models, &payload.default_model)?;
    let default_model = payload.default_model.trim().to_string();
    let api_key = payload
        .api_key
        .as_deref()
        .map(str::trim)
        .map(ToOwned::to_owned);

    if state.providers.len() >= MAX_PROVIDERS
        && !state
            .providers
            .iter()
            .any(|provider| provider.id == provider_id)
    {
        return Err(HostError::new(
            "AI_PROVIDER_LIMIT_REACHED",
            "已达到 AI 供应商数量上限。",
            false,
        ));
    }

    let old_default = state.default_provider_id.clone();
    let mut runtime_changed = false;
    let mut changed = false;
    if let Some(provider) = state
        .providers
        .iter_mut()
        .find(|provider| provider.id == provider_id)
    {
        let was_selected = old_default.as_deref() == Some(provider_id.as_str());
        let connection_sensitive_change = provider.base_url != base_url
            || provider.default_model != default_model
            || provider.models != models
            || provider.enabled != payload.enabled
            || api_key
                .as_ref()
                .is_some_and(|value| provider.api_key.as_ref() != Some(value));
        if provider.name != name
            || provider.kind != payload.kind
            || provider.base_url != base_url
            || provider.models != models
            || provider.default_model != default_model
            || provider.enabled != payload.enabled
            || api_key
                .as_ref()
                .is_some_and(|value| provider.api_key.as_ref() != Some(value))
        {
            provider.name = name;
            provider.kind = payload.kind.clone();
            provider.base_url = base_url;
            provider.models = models;
            provider.default_model = default_model.clone();
            provider.enabled = payload.enabled;
            if let Some(api_key) = api_key {
                if let Some(old) = provider.api_key.replace(api_key) {
                    let mut old = old;
                    old.zeroize();
                }
            }
            if connection_sensitive_change {
                provider.connection = untested_connection("配置已变更，请重新测试连接");
            }
            provider.updated_at = now;
            changed = true;
            runtime_changed = was_selected;
        }
    } else {
        state.providers.push(SecureProviderState {
            id: provider_id.clone(),
            name,
            kind: payload.kind.clone(),
            base_url,
            api_key,
            models,
            default_model: default_model.clone(),
            enabled: payload.enabled,
            connection: untested_connection("尚未测试连接"),
            created_at: now,
            updated_at: now,
        });
        changed = true;
    }

    if payload.set_default || state.default_provider_id.is_none() {
        if !payload.enabled {
            return Err(HostError::validation("默认供应商必须处于启用状态"));
        }
        if state.default_provider_id.as_deref() != Some(provider_id.as_str())
            || state.default_model.as_deref() != Some(default_model.as_str())
        {
            state.default_provider_id = Some(provider_id.clone());
            state.default_model = Some(default_model);
            changed = true;
            runtime_changed = true;
        }
    } else if state.default_provider_id.as_deref() == Some(provider_id.as_str())
        && state.default_model.as_deref() != Some(default_model.as_str())
    {
        state.default_model = Some(default_model);
        changed = true;
        runtime_changed = true;
    }

    Ok(MutationOutcome {
        changed,
        runtime_changed,
        connection_test: None,
    })
}

fn remove_provider(
    state: &mut SecureCredentialState,
    provider_id: &str,
) -> Result<MutationOutcome, HostError> {
    if state.providers.len() <= 1 {
        return Err(HostError::new(
            "AI_PROVIDER_LAST_REQUIRED",
            "at least one AI provider must remain configured",
            false,
        ));
    }
    let index = state
        .providers
        .iter()
        .position(|provider| provider.id == provider_id)
        .ok_or_else(|| provider_not_found(provider_id))?;
    let was_default = state.default_provider_id.as_deref() == Some(provider_id);
    state.providers.remove(index);
    if was_default {
        let replacement = state
            .providers
            .iter()
            .find(|provider| {
                provider.enabled
                    && provider
                        .api_key
                        .as_ref()
                        .is_some_and(|api_key| !api_key.trim().is_empty())
            })
            .or_else(|| state.providers.iter().find(|provider| provider.enabled))
            .or_else(|| state.providers.first());
        state.default_provider_id = replacement.map(|provider| provider.id.clone());
        state.default_model = replacement.map(|provider| provider.default_model.clone());
    }
    Ok(MutationOutcome {
        changed: true,
        runtime_changed: was_default,
        connection_test: None,
    })
}

fn select_provider(
    state: &mut SecureCredentialState,
    provider_id: &str,
    model: &str,
    now: i64,
) -> Result<MutationOutcome, HostError> {
    let provider = state
        .providers
        .iter_mut()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| provider_not_found(provider_id))?;
    if !provider.enabled {
        return Err(HostError::new(
            "AI_PROVIDER_DISABLED",
            "该 AI 供应商已停用，不能设为默认供应商。",
            false,
        ));
    }
    let model = model.trim().to_string();
    validate_model(&model)?;
    let mut changed = false;
    if !provider.models.iter().any(|candidate| candidate == &model) {
        if provider.models.len() >= MAX_MODELS_PER_PROVIDER {
            return Err(HostError::validation("单个供应商的模型数量过多"));
        }
        provider.models.push(model.clone());
        changed = true;
    }
    if provider.default_model != model {
        provider.default_model = model.clone();
        provider.updated_at = now;
        changed = true;
    }
    if state.default_provider_id.as_deref() != Some(provider_id)
        || state.default_model.as_deref() != Some(model.as_str())
    {
        state.default_provider_id = Some(provider_id.to_string());
        state.default_model = Some(model);
        changed = true;
    }
    Ok(MutationOutcome {
        changed,
        runtime_changed: changed,
        connection_test: None,
    })
}

fn test_provider(
    state: &mut SecureCredentialState,
    provider_id: &str,
    now: i64,
) -> Result<MutationOutcome, HostError> {
    let provider_index = state
        .providers
        .iter()
        .position(|provider| provider.id == provider_id)
        .ok_or_else(|| provider_not_found(provider_id))?;
    let provider = &state.providers[provider_index];
    let api_key = provider
        .api_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| Zeroizing::new(value.to_string()));
    let connection = if !provider.enabled {
        failed_connection("AI 服务已停用", now)
    } else if let Some(api_key) = api_key.as_deref() {
        probe_provider_models(&provider.base_url, api_key, now)
    } else {
        failed_connection("尚未配置 API Key", now)
    };
    let provider = &mut state.providers[provider_index];
    provider.connection = connection.clone();
    provider.updated_at = now;
    Ok(MutationOutcome {
        changed: true,
        runtime_changed: false,
        connection_test: Some(connection),
    })
}

fn discover_provider_models(
    state: &mut SecureCredentialState,
    payload: &DiscoverAiProviderModelsPayload,
    now: i64,
) -> Result<MutationOutcome, HostError> {
    let provider_index = payload
        .provider_id
        .as_deref()
        .map(|provider_id| {
            state
                .providers
                .iter()
                .position(|provider| provider.id == provider_id)
                .ok_or_else(|| provider_not_found(provider_id))
        })
        .transpose()?;
    let base_url = validate_base_url(&payload.base_url)?;
    let provided_api_key = payload
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let api_key = provided_api_key
        .map(|value| Zeroizing::new(value.to_string()))
        .or_else(|| {
            provider_index
                .and_then(|index| state.providers[index].api_key.as_deref())
                .filter(|value| !value.trim().is_empty())
                .map(|value| Zeroizing::new(value.to_string()))
        });
    let enabled = provider_index
        .map(|index| state.providers[index].enabled)
        .unwrap_or(true);
    let connection = if !enabled {
        failed_connection("AI 服务已停用", now)
    } else if let Some(api_key) = api_key.as_deref() {
        probe_provider_models(&base_url, api_key, now)
    } else {
        failed_connection("请输入 API Key，或先保存已有服务的密钥", now)
    };

    let changed = if let Some(index) = provider_index {
        let provider = &mut state.providers[index];
        provider.connection = connection.clone();
        provider.updated_at = now;
        true
    } else {
        false
    };
    Ok(MutationOutcome {
        changed,
        runtime_changed: false,
        connection_test: Some(connection),
    })
}

fn probe_provider_models(
    base_url: &str,
    api_key: &str,
    tested_at: i64,
) -> AiProviderConnectionStatus {
    let started = Instant::now();
    let result = fetch_provider_models(
        base_url,
        api_key,
        PROVIDER_CONNECT_TIMEOUT,
        PROVIDER_REQUEST_TIMEOUT,
    );
    let latency_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);
    match result {
        Ok(result) if result.models.is_empty() => AiProviderConnectionStatus {
            state: AiProviderConnectionState::Warning,
            message: "连接成功，但服务没有返回可用模型".to_string(),
            latency_ms: Some(latency_ms),
            tested_at: Some(tested_at),
            discovered_models: Vec::new(),
        },
        Ok(result) => {
            let count = result.models.len();
            AiProviderConnectionStatus {
                state: AiProviderConnectionState::Ready,
                message: if result.truncated {
                    format!("连接成功，已读取前 {count} 个模型")
                } else {
                    format!("连接成功，已拉取 {count} 个模型")
                },
                latency_ms: Some(latency_ms),
                tested_at: Some(tested_at),
                discovered_models: result.models,
            }
        }
        Err(error) => AiProviderConnectionStatus {
            state: AiProviderConnectionState::Failed,
            message: error.message(),
            latency_ms: Some(latency_ms),
            tested_at: Some(tested_at),
            discovered_models: Vec::new(),
        },
    }
}

fn fetch_provider_models(
    base_url: &str,
    api_key: &str,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<ProviderProbeSuccess, ProviderProbeFailure> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("BSAIGC-Desktop/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| ProviderProbeFailure::Network)?;
    let models_url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut response = client
        .get(models_url)
        .bearer_auth(api_key)
        .send()
        .map_err(|error| {
            if error.is_timeout() {
                ProviderProbeFailure::Timeout
            } else {
                ProviderProbeFailure::Network
            }
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => ProviderProbeFailure::Unauthorized,
            403 => ProviderProbeFailure::Forbidden,
            404 => ProviderProbeFailure::NotFound,
            429 => ProviderProbeFailure::RateLimited,
            code => ProviderProbeFailure::Http(code),
        });
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_MODEL_RESPONSE_BYTES as u64)
    {
        return Err(ProviderProbeFailure::ResponseTooLarge);
    }

    let mut body = Vec::new();
    (&mut response)
        .take((MAX_MODEL_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|_| ProviderProbeFailure::Network)?;
    if body.len() > MAX_MODEL_RESPONSE_BYTES {
        return Err(ProviderProbeFailure::ResponseTooLarge);
    }
    let response: ProviderModelListResponse =
        serde_json::from_slice(&body).map_err(|_| ProviderProbeFailure::InvalidResponse)?;
    let source_count = response.data.len();
    let mut models = response
        .data
        .into_iter()
        .map(|model| model.id.trim().to_string())
        .filter(|model| !model.is_empty())
        .filter(|model| model.len() <= MAX_MODEL_BYTES && !model.chars().any(char::is_control))
        .collect::<Vec<_>>();
    models.sort_unstable();
    models.dedup();
    if models.is_empty() && source_count > 0 {
        return Err(ProviderProbeFailure::InvalidResponse);
    }
    let truncated = models.len() > MAX_MODELS_PER_PROVIDER;
    models.truncate(MAX_MODELS_PER_PROVIDER);
    Ok(ProviderProbeSuccess { models, truncated })
}

fn clear_provider_api_key(
    state: &mut SecureCredentialState,
    provider_id: &str,
    now: i64,
) -> Result<MutationOutcome, HostError> {
    let is_default = state.default_provider_id.as_deref() == Some(provider_id);
    let provider = state
        .providers
        .iter_mut()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| provider_not_found(provider_id))?;
    let Some(mut api_key) = provider.api_key.take() else {
        return Ok(MutationOutcome::unchanged());
    };
    api_key.zeroize();
    provider.connection = untested_connection("API Key 已移除");
    provider.updated_at = now;
    Ok(MutationOutcome {
        changed: true,
        runtime_changed: is_default,
        connection_test: None,
    })
}

fn save_legacy_api_key(
    state: &mut SecureCredentialState,
    api_key: &str,
    now: i64,
) -> Result<MutationOutcome, HostError> {
    let api_key = api_key.trim().to_string();
    validate_api_key(&api_key)?;
    let mut changed = false;
    let provider = if let Some(index) = state
        .providers
        .iter()
        .position(|provider| provider.id == PRODUCT_PROVIDER_ID)
    {
        &mut state.providers[index]
    } else {
        state.providers.push(default_product_provider(now));
        changed = true;
        state
            .providers
            .last_mut()
            .expect("provider was just inserted")
    };
    if provider.api_key.as_ref() != Some(&api_key) {
        if let Some(old) = provider.api_key.replace(api_key) {
            let mut old = old;
            old.zeroize();
        }
        provider.connection = untested_connection("API Key 已更新，请重新测试连接");
        provider.updated_at = now;
        changed = true;
    }
    if state.default_provider_id.as_deref() != Some(PRODUCT_PROVIDER_ID)
        || state.default_model.as_deref() != Some(provider.default_model.as_str())
    {
        state.default_provider_id = Some(PRODUCT_PROVIDER_ID.to_string());
        state.default_model = Some(provider.default_model.clone());
        changed = true;
    }
    Ok(MutationOutcome {
        changed,
        runtime_changed: changed,
        connection_test: None,
    })
}

fn clear_legacy_api_key(
    state: &mut SecureCredentialState,
    now: i64,
) -> Result<MutationOutcome, HostError> {
    clear_provider_api_key(state, PRODUCT_PROVIDER_ID, now)
}

fn normalize_models(models: &[String], default_model: &str) -> Result<Vec<String>, HostError> {
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for model in models
        .iter()
        .map(|model| model.trim())
        .chain(std::iter::once(default_model.trim()))
    {
        validate_model(model)?;
        if seen.insert(model.to_string()) {
            normalized.push(model.to_string());
        }
    }
    if normalized.len() > MAX_MODELS_PER_PROVIDER {
        return Err(HostError::validation("单个供应商的模型数量过多"));
    }
    Ok(normalized)
}

fn selected_provider(state: &SecureCredentialState) -> Option<&SecureProviderState> {
    state
        .default_provider_id
        .as_deref()
        .and_then(|provider_id| {
            state
                .providers
                .iter()
                .find(|provider| provider.id == provider_id)
        })
}

fn provider_not_found(provider_id: &str) -> HostError {
    HostError::new(
        "AI_PROVIDER_NOT_FOUND",
        format!("未找到 AI 供应商：{provider_id}"),
        false,
    )
}

fn status_from_state(
    state: &SecureCredentialState,
    applies_on_next_runtime_start: bool,
) -> AiCredentialStatus {
    let selected = selected_provider(state);
    let configured = selected
        .and_then(|provider| provider.api_key.as_ref())
        .is_some_and(|value| !value.trim().is_empty());
    AiCredentialStatus {
        provider: selected
            .map(|provider| provider.name.clone())
            .unwrap_or_default(),
        configured,
        persisted: configured,
        protection: protection(),
        revision: state.revision,
        updated_at: state.updated_at,
        applies_on_next_runtime_start,
        default_provider_id: state.default_provider_id.clone(),
        default_model: state.default_model.clone(),
        providers: state
            .providers
            .iter()
            .map(|provider| provider_record(provider, state.default_provider_id.as_deref()))
            .collect(),
    }
}

fn provider_record(
    provider: &SecureProviderState,
    default_provider_id: Option<&str>,
) -> AiProviderRecord {
    AiProviderRecord {
        id: provider.id.clone(),
        name: provider.name.clone(),
        kind: provider.kind.clone(),
        base_url: provider.base_url.clone(),
        api_key_configured: provider
            .api_key
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
        api_key_hint: provider.api_key.as_deref().map(api_key_hint),
        models: provider.models.clone(),
        default_model: provider.default_model.clone(),
        is_default: default_provider_id == Some(provider.id.as_str()),
        enabled: provider.enabled,
        connection: provider.connection.clone(),
        created_at: provider.created_at,
        updated_at: provider.updated_at,
    }
}

fn api_key_hint(api_key: &str) -> String {
    let tail: String = api_key
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("••••{tail}")
}

fn migrate_legacy_state(
    mut legacy: LegacySecureCredentialState,
) -> Result<SecureCredentialState, HostError> {
    if legacy.schema_version != LEGACY_STATE_SCHEMA_VERSION || legacy.revision < 0 {
        return Err(storage_corrupt_error());
    }
    let now = legacy.updated_at.unwrap_or_else(now_millis);
    let model = embedded_provider_model();
    let provider_name = if legacy.provider.trim().is_empty() {
        PRODUCT_PROVIDER_NAME.to_string()
    } else {
        legacy.provider.trim().to_string()
    };
    let provider = SecureProviderState {
        id: PRODUCT_PROVIDER_ID.to_string(),
        name: provider_name,
        kind: AiProviderKind::OpenAiCompatible,
        base_url: embedded_provider_base_url(),
        api_key: legacy.api_key.take(),
        models: vec![model.clone()],
        default_model: model.clone(),
        enabled: true,
        connection: untested_connection("已从历史配置迁移，请重新测试连接"),
        created_at: now,
        updated_at: now,
    };
    let mut state = SecureCredentialState {
        schema_version: STATE_SCHEMA_VERSION,
        default_provider_id: Some(PRODUCT_PROVIDER_ID.to_string()),
        default_model: Some(model),
        providers: vec![provider],
        revision: legacy.revision,
        updated_at: legacy.updated_at,
        receipts: Vec::new(),
    };
    let provider_records = status_from_state(&state, false).providers;
    let legacy_receipts = std::mem::take(&mut legacy.receipts);
    state.receipts = legacy_receipts
        .into_iter()
        .take(MAX_RECEIPTS)
        .map(|receipt| {
            let legacy_status = receipt.response.status;
            let status = AiCredentialStatus {
                provider: legacy_status.provider,
                configured: legacy_status.configured,
                persisted: legacy_status.persisted,
                protection: legacy_status.protection,
                revision: legacy_status.revision,
                updated_at: legacy_status.updated_at,
                applies_on_next_runtime_start: legacy_status.applies_on_next_runtime_start,
                default_provider_id: Some(PRODUCT_PROVIDER_ID.to_string()),
                default_model: state.default_model.clone(),
                providers: provider_records.clone(),
            };
            SecureCommandReceipt {
                command_id: receipt.command_id,
                idempotency_key: receipt.idempotency_key,
                command_type: receipt.command_type,
                request_fingerprint: receipt.request_fingerprint,
                response: AiCredentialCommandResponse {
                    receipt: receipt.response.receipt,
                    status,
                    connection_test: None,
                    replayed: receipt.response.replayed,
                },
            }
        })
        .collect();
    Ok(state)
}

fn validate_stored_state(state: &mut SecureCredentialState) -> Result<bool, HostError> {
    let mut changed = false;
    if state.schema_version != STATE_SCHEMA_VERSION
        || state.revision < 0
        || state.providers.len() > MAX_PROVIDERS
    {
        return Err(storage_corrupt_error());
    }
    if state.providers.is_empty() {
        let provider = default_product_provider(state.updated_at.unwrap_or_else(now_millis));
        state.default_provider_id = Some(provider.id.clone());
        state.default_model = Some(provider.default_model.clone());
        state.providers.push(provider);
        changed = true;
    }

    let mut ids = HashSet::new();
    for provider in &mut state.providers {
        if provider.id == PRODUCT_PROVIDER_ID && provider.name == LEGACY_PRODUCT_PROVIDER_NAME {
            provider.name = PRODUCT_PROVIDER_NAME.to_string();
            changed = true;
        }
        validate_provider_id(&provider.id).map_err(|_| storage_corrupt_error())?;
        if !ids.insert(provider.id.clone()) {
            return Err(storage_corrupt_error());
        }
        validate_bounded_text("供应商名称", &provider.name, MAX_NAME_BYTES)
            .map_err(|_| storage_corrupt_error())?;
        provider.base_url =
            validate_base_url(&provider.base_url).map_err(|_| storage_corrupt_error())?;
        if let Some(api_key) = provider.api_key.as_deref() {
            validate_api_key(api_key).map_err(|_| storage_corrupt_error())?;
        }
        provider.models = normalize_models(&provider.models, &provider.default_model)
            .map_err(|_| storage_corrupt_error())?;
        if provider.created_at < 0 || provider.updated_at < 0 {
            return Err(storage_corrupt_error());
        }
        if provider.connection.discovered_models.len() > MAX_MODELS_PER_PROVIDER {
            return Err(storage_corrupt_error());
        }
    }

    let selected_valid = state.default_provider_id.as_deref().is_some_and(|id| {
        state
            .providers
            .iter()
            .any(|provider| provider.id == id && provider.enabled)
    });
    if !selected_valid {
        let replacement = state
            .providers
            .iter()
            .find(|provider| provider.enabled)
            .or_else(|| state.providers.first())
            .expect("providers is non-empty");
        state.default_provider_id = Some(replacement.id.clone());
        state.default_model = Some(replacement.default_model.clone());
    } else if let Some(provider) = selected_provider(state) {
        let selected_model = state
            .default_model
            .as_deref()
            .filter(|model| provider.models.iter().any(|candidate| candidate == model))
            .unwrap_or(provider.default_model.as_str())
            .to_string();
        state.default_model = Some(selected_model);
    }

    if state.receipts.len() > MAX_RECEIPTS {
        let excess = state.receipts.len() - MAX_RECEIPTS;
        state.receipts.drain(0..excess);
    }
    Ok(changed)
}

fn untested_connection(message: &str) -> AiProviderConnectionStatus {
    AiProviderConnectionStatus {
        state: AiProviderConnectionState::Untested,
        message: message.to_string(),
        latency_ms: None,
        tested_at: None,
        discovered_models: Vec::new(),
    }
}

fn failed_connection(message: &str, tested_at: i64) -> AiProviderConnectionStatus {
    AiProviderConnectionStatus {
        state: AiProviderConnectionState::Failed,
        message: message.to_string(),
        latency_ms: None,
        tested_at: Some(tested_at),
        discovered_models: Vec::new(),
    }
}

fn command_receipt(meta: &CommandMeta<'_>, revision: i64, completed_at: i64) -> CommandReceipt {
    CommandReceipt {
        command_id: meta.command_id.to_string(),
        idempotency_key: meta.idempotency_key.to_string(),
        command_type: meta.command_type.to_string(),
        aggregate_id: AGGREGATE_ID.to_string(),
        revision,
        last_event_sequence: 0,
        completed_at,
    }
}

fn next_revision(current: i64) -> Result<i64, HostError> {
    current.checked_add(1).ok_or_else(|| {
        HostError::new(
            "AI_CREDENTIAL_REVISION_EXHAUSTED",
            "AI 配置版本已达到上限。",
            false,
        )
    })
}

fn protection() -> Option<AiCredentialProtection> {
    #[cfg(windows)]
    {
        Some(AiCredentialProtection::WindowsDpapiCurrentUser)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn storage_lock_error() -> HostError {
    HostError::new(
        "AI_CREDENTIAL_BUSY",
        "AI 配置正在被其他操作更新，请稍后重试。",
        true,
    )
}

fn storage_read_error() -> HostError {
    HostError::new(
        "AI_CREDENTIAL_READ_FAILED",
        "无法读取本地 AI 配置，请重试。",
        true,
    )
}

fn storage_write_error() -> HostError {
    HostError::new(
        "AI_CREDENTIAL_WRITE_FAILED",
        "无法安全保存本地 AI 配置，请重试。",
        true,
    )
}

fn storage_corrupt_error() -> HostError {
    HostError::new(
        "AI_CREDENTIAL_CORRUPT",
        "本地 AI 配置无法识别或已经损坏，请清除后重新配置。",
        false,
    )
}

#[cfg(windows)]
fn protect_current_user(plaintext: &[u8]) -> Result<Vec<u8>, HostError> {
    use std::slice;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let cb_data = u32::try_from(plaintext.len()).map_err(|_| storage_write_error())?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: cb_data,
        pbData: plaintext.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let protected = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if protected == 0 || output.pbData.is_null() || output.cbData == 0 {
        return Err(storage_write_error());
    }
    let bytes = unsafe { slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(bytes)
}

#[cfg(windows)]
fn unprotect_current_user(ciphertext: &[u8]) -> Result<Vec<u8>, HostError> {
    use std::slice;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let cb_data = u32::try_from(ciphertext.len()).map_err(|_| storage_corrupt_error())?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: cb_data,
        pbData: ciphertext.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let unprotected = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if unprotected == 0 || output.pbData.is_null() {
        return Err(storage_corrupt_error());
    }
    let bytes = unsafe { slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(bytes)
}

#[cfg(not(windows))]
fn protect_current_user(_plaintext: &[u8]) -> Result<Vec<u8>, HostError> {
    Err(HostError::new(
        "AI_CREDENTIAL_PROTECTION_UNAVAILABLE",
        "当前系统不支持本地 AI 凭据保护。",
        false,
    ))
}

#[cfg(not(windows))]
fn unprotect_current_user(_ciphertext: &[u8]) -> Result<Vec<u8>, HostError> {
    Err(HostError::new(
        "AI_CREDENTIAL_PROTECTION_UNAVAILABLE",
        "当前系统不支持本地 AI 凭据保护。",
        false,
    ))
}

#[cfg(windows)]
fn replace_file_atomically(source: &Path, destination: &Path) -> Result<(), HostError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let moved = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        return Err(storage_write_error());
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file_atomically(source: &Path, destination: &Path) -> Result<(), HostError> {
    if destination.exists() {
        fs::remove_file(destination).map_err(|_| storage_write_error())?;
    }
    fs::rename(source, destination).map_err(|_| storage_write_error())
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::protocol::{
        AiProviderIdPayload, DiscoverAiProviderModelsPayload, OperationContext,
        SaveBsaigcProviderApiKeyPayload, SelectAiProviderPayload, UpsertAiProviderPayload,
    };
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc::{self, Receiver};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    struct MockModelsServer {
        base_url: String,
        requests: Receiver<String>,
        worker: JoinHandle<()>,
    }

    impl MockModelsServer {
        fn finish(self) {
            self.worker.join().unwrap();
        }
    }

    fn mock_models_server(
        status: u16,
        body: String,
        delay: Duration,
        content_length: Option<usize>,
    ) -> MockModelsServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (request_sender, requests) = mpsc::channel();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let bytes = stream.read(&mut buffer).unwrap_or(0);
                if bytes == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..bytes]);
            }
            let _ = request_sender.send(String::from_utf8_lossy(&request).to_string());
            if !delay.is_zero() {
                thread::sleep(delay);
            }
            let reason = match status {
                200 => "OK",
                401 => "Unauthorized",
                403 => "Forbidden",
                404 => "Not Found",
                429 => "Too Many Requests",
                _ => "Server Error",
            };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                content_length.unwrap_or(body.len())
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(body.as_bytes());
            let _ = stream.flush();
        });
        MockModelsServer {
            base_url: format!("http://{address}/v1"),
            requests,
            worker,
        }
    }

    #[test]
    fn product_preview_defaults_match_the_release_contract() {
        assert_eq!(PRODUCT_PROVIDER_ID, "bsaigc");
        assert_eq!(PRODUCT_PROVIDER_BASE_URL, "https://bsaigc.dpdns.org/v1");
        assert_eq!(PRODUCT_DEFAULT_MODEL, "gpt-5.6-sol");
    }

    fn context() -> OperationContext {
        OperationContext {
            actor_id: "local-operator".to_string(),
            account_id: None,
            project_id: None,
            window_id: "test-window".to_string(),
            trace_id: Uuid::new_v4().to_string(),
        }
    }

    fn save_command_at(
        api_key: &str,
        command_id: &str,
        idempotency_key: &str,
        expected_revision: i64,
    ) -> AiCredentialCommandEnvelope {
        AiCredentialCommandEnvelope::SaveBsaigcApiKey {
            command_id: command_id.to_string(),
            protocol_version: AI_CREDENTIAL_PROTOCOL_VERSION.to_string(),
            context: context(),
            payload: SaveBsaigcProviderApiKeyPayload {
                api_key: api_key.to_string(),
            },
            idempotency_key: idempotency_key.to_string(),
            expected_revision: Some(expected_revision),
            deadline_at: Some(now_millis() + 60_000),
        }
    }

    fn save_command(
        api_key: &str,
        command_id: &str,
        idempotency_key: &str,
    ) -> AiCredentialCommandEnvelope {
        save_command_at(api_key, command_id, idempotency_key, 0)
    }

    fn upsert_command(
        provider_id: &str,
        api_key: Option<&str>,
        model: &str,
        set_default: bool,
        expected_revision: i64,
        suffix: &str,
    ) -> AiCredentialCommandEnvelope {
        AiCredentialCommandEnvelope::UpsertProvider {
            command_id: format!("upsert-{suffix}"),
            protocol_version: AI_CREDENTIAL_PROTOCOL_VERSION.to_string(),
            context: context(),
            payload: UpsertAiProviderPayload {
                provider_id: Some(provider_id.to_string()),
                name: format!("Provider {provider_id}"),
                kind: AiProviderKind::OpenAiCompatible,
                base_url: format!("https://providers.example.invalid/{provider_id}/v1"),
                api_key: api_key.map(str::to_string),
                models: vec![model.to_string()],
                default_model: model.to_string(),
                set_default,
                enabled: true,
            },
            idempotency_key: format!("idem-upsert-{suffix}"),
            expected_revision: Some(expected_revision),
            deadline_at: Some(now_millis() + 60_000),
        }
    }

    fn provider_id_command(
        provider_id: &str,
        expected_revision: i64,
        suffix: &str,
        command_type: &str,
    ) -> AiCredentialCommandEnvelope {
        let payload = AiProviderIdPayload {
            provider_id: provider_id.to_string(),
        };
        match command_type {
            "remove" => AiCredentialCommandEnvelope::RemoveProvider {
                command_id: format!("remove-{suffix}"),
                protocol_version: AI_CREDENTIAL_PROTOCOL_VERSION.to_string(),
                context: context(),
                payload,
                idempotency_key: format!("idem-remove-{suffix}"),
                expected_revision: Some(expected_revision),
                deadline_at: Some(now_millis() + 60_000),
            },
            "test" => AiCredentialCommandEnvelope::TestProvider {
                command_id: format!("test-{suffix}"),
                protocol_version: AI_CREDENTIAL_PROTOCOL_VERSION.to_string(),
                context: context(),
                payload,
                idempotency_key: format!("idem-test-{suffix}"),
                expected_revision: Some(expected_revision),
                deadline_at: Some(now_millis() + 60_000),
            },
            "clear-key" => AiCredentialCommandEnvelope::ClearProviderApiKey {
                command_id: format!("clear-key-{suffix}"),
                protocol_version: AI_CREDENTIAL_PROTOCOL_VERSION.to_string(),
                context: context(),
                payload,
                idempotency_key: format!("idem-clear-key-{suffix}"),
                expected_revision: Some(expected_revision),
                deadline_at: Some(now_millis() + 60_000),
            },
            _ => panic!("unsupported test command type"),
        }
    }

    #[test]
    fn dpapi_state_never_persists_plaintext_and_round_trips() {
        let temp = tempfile::tempdir().unwrap();
        let service = AiCredentialService::open(temp.path()).unwrap();
        let secret = "sk-test-business-workbench-secret";
        let command = save_command(secret, "save-1", "idem-1");
        assert!(!format!("{command:?}").contains(secret));
        assert!(!serde_json::to_string(&command).unwrap().contains(secret));

        let saved = service.execute(command).unwrap();
        assert!(saved.status.configured);
        assert_eq!(saved.status.revision, 1);
        assert_eq!(saved.status.default_provider_id.as_deref(), Some("bsaigc"));
        assert_eq!(saved.status.providers.len(), 1);
        assert_eq!(
            saved.status.providers[0].api_key_hint.as_deref(),
            Some("••••cret")
        );
        assert!(saved.connection_test.is_none());
        assert!(!serde_json::to_string(&saved).unwrap().contains(secret));
        assert_eq!(
            service
                .load_api_key()
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some(secret)
        );

        let disk = fs::read(temp.path().join("credentials/provider-key.dpapi")).unwrap();
        assert!(!disk
            .windows(secret.len())
            .any(|window| window == secret.as_bytes()));
        assert!(serde_json::from_slice::<serde_json::Value>(&disk).is_err());

        let replayed = service
            .execute(save_command(secret, "save-1", "idem-1"))
            .unwrap();
        assert!(replayed.replayed);
        assert_eq!(replayed.status.revision, 1);

        let no_op = service
            .execute(save_command_at(secret, "save-2", "idem-2", 1))
            .unwrap();
        assert_eq!(no_op.status.revision, 1);
        assert!(!no_op.status.applies_on_next_runtime_start);
    }

    #[test]
    fn clear_removes_key_and_preserves_monotonic_revision() {
        let temp = tempfile::tempdir().unwrap();
        let service = AiCredentialService::open(temp.path()).unwrap();
        service
            .execute(save_command("sk-test-clear-secret", "save-1", "idem-save"))
            .unwrap();
        let cleared = service
            .execute(AiCredentialCommandEnvelope::ClearBsaigcApiKey {
                command_id: "clear-1".to_string(),
                protocol_version: AI_CREDENTIAL_PROTOCOL_VERSION.to_string(),
                context: context(),
                idempotency_key: "idem-clear".to_string(),
                expected_revision: Some(1),
                deadline_at: Some(now_millis() + 60_000),
            })
            .unwrap();
        assert!(!cleared.status.configured);
        assert_eq!(cleared.status.revision, 2);
        assert!(cleared.status.providers[0].api_key_hint.is_none());
        assert!(service.load_api_key().unwrap().is_none());
        assert_eq!(service.status().unwrap().revision, 2);
    }

    #[test]
    fn idempotency_key_cannot_be_reused_for_a_different_secret() {
        let temp = tempfile::tempdir().unwrap();
        let service = AiCredentialService::open(temp.path()).unwrap();
        service
            .execute(save_command("sk-first-business-secret", "save-1", "idem-1"))
            .unwrap();
        let error = service
            .execute(save_command(
                "sk-second-business-secret",
                "save-1",
                "idem-1",
            ))
            .unwrap_err();
        assert_eq!(error.code, "AI_CREDENTIAL_IDEMPOTENCY_CONFLICT");
        assert_eq!(
            service
                .load_api_key()
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some("sk-first-business-secret")
        );
    }

    #[test]
    fn multi_provider_lifecycle_selects_clears_and_falls_back() {
        let temp = tempfile::tempdir().unwrap();
        let service = AiCredentialService::open(temp.path()).unwrap();
        let mut initial_state = SecureCredentialState::default();
        if let Some(product) = initial_state
            .providers
            .iter_mut()
            .find(|provider| provider.id == PRODUCT_PROVIDER_ID)
        {
            product.api_key = None;
        }
        service.write_state(&initial_state).unwrap();
        let first_secret = "sk-provider-first-secret";
        let second_secret = "sk-provider-second-secret";

        let first = service
            .execute(upsert_command(
                "provider-one",
                Some(first_secret),
                "model-one",
                true,
                0,
                "one",
            ))
            .unwrap();
        assert_eq!(first.status.revision, 1);
        assert_eq!(
            first.status.default_provider_id.as_deref(),
            Some("provider-one")
        );
        assert_eq!(first.status.default_model.as_deref(), Some("model-one"));

        let second = service
            .execute(upsert_command(
                "provider-two",
                Some(second_secret),
                "model-two",
                false,
                1,
                "two",
            ))
            .unwrap();
        assert_eq!(second.status.revision, 2);
        assert_eq!(second.status.providers.len(), 3);
        assert!(second
            .status
            .providers
            .iter()
            .any(|provider| provider.id == PRODUCT_PROVIDER_ID));
        assert_eq!(
            second.status.default_provider_id.as_deref(),
            Some("provider-one")
        );

        let selected = service
            .execute(AiCredentialCommandEnvelope::SelectProvider {
                command_id: "select-two".to_string(),
                protocol_version: AI_CREDENTIAL_PROTOCOL_VERSION.to_string(),
                context: context(),
                payload: SelectAiProviderPayload {
                    provider_id: "provider-two".to_string(),
                    model: "model-two".to_string(),
                },
                idempotency_key: "idem-select-two".to_string(),
                expected_revision: Some(2),
                deadline_at: Some(now_millis() + 60_000),
            })
            .unwrap();
        assert_eq!(selected.status.revision, 3);
        assert_eq!(
            selected.status.default_provider_id.as_deref(),
            Some("provider-two")
        );
        assert_eq!(
            service
                .load_api_key()
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some(second_secret)
        );

        let cleared = service
            .execute(provider_id_command("provider-two", 3, "two", "clear-key"))
            .unwrap();
        assert_eq!(cleared.status.revision, 4);
        assert!(!cleared.status.configured);
        assert!(service.load_api_key().unwrap().is_none());

        let removed = service
            .execute(provider_id_command("provider-two", 4, "two", "remove"))
            .unwrap();
        assert_eq!(removed.status.revision, 5);
        assert_eq!(
            removed.status.default_provider_id.as_deref(),
            Some("provider-one")
        );
        assert_eq!(removed.status.providers.len(), 2);
        assert!(removed
            .status
            .providers
            .iter()
            .any(|provider| provider.id == PRODUCT_PROVIDER_ID));
        assert_eq!(
            service
                .load_api_key()
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some(first_secret)
        );
    }

    #[test]
    fn test_provider_performs_a_real_models_request_and_replays_its_result() {
        let temp = tempfile::tempdir().unwrap();
        let service = AiCredentialService::open(temp.path()).unwrap();
        let server = mock_models_server(
            200,
            r#"{"data":[{"id":"gpt-5.6-sol"},{"id":"gpt-5.6-sol"},{"id":"gpt-5.6-mini"}]}"#
                .to_string(),
            Duration::ZERO,
            None,
        );
        let mut upsert = upsert_command(
            "provider-one",
            Some("sk-provider-live-secret"),
            "model-one",
            true,
            0,
            "one",
        );
        if let AiCredentialCommandEnvelope::UpsertProvider { payload, .. } = &mut upsert {
            payload.base_url = server.base_url.clone();
        }
        service.execute(upsert).unwrap();
        let command = provider_id_command("provider-one", 1, "one", "test");
        let tested = service.execute(command).unwrap();
        let connection = tested.connection_test.as_ref().unwrap();
        assert_eq!(connection.state, AiProviderConnectionState::Ready);
        assert!(connection.message.contains("已拉取 2 个模型"));
        assert!(connection.latency_ms.is_some());
        assert_eq!(
            connection.discovered_models,
            vec!["gpt-5.6-mini", "gpt-5.6-sol"]
        );
        assert!(connection.tested_at.is_some());
        assert_eq!(tested.status.revision, 2);
        assert!(!tested.status.applies_on_next_runtime_start);
        assert_eq!(
            tested
                .status
                .providers
                .iter()
                .find(|provider| provider.id == "provider-one")
                .unwrap()
                .connection
                .state,
            AiProviderConnectionState::Ready
        );
        let request = server
            .requests
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert!(request.starts_with("GET /v1/models HTTP/1.1\r\n"));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer sk-provider-live-secret\r\n"));
        server.finish();

        let replay = service
            .execute(provider_id_command("provider-one", 1, "one", "test"))
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(
            replay.connection_test.unwrap().state,
            AiProviderConnectionState::Ready
        );
    }

    #[test]
    fn provider_model_probe_reports_status_failures_without_leaking_the_key() {
        let secret = "sk-provider-status-secret";
        for (status, expected) in [(401, "HTTP 401"), (403, "HTTP 403"), (404, "HTTP 404")] {
            let server = mock_models_server(status, "{}".to_string(), Duration::ZERO, None);
            let error = fetch_provider_models(
                &server.base_url,
                secret,
                Duration::from_millis(100),
                Duration::from_millis(250),
            )
            .unwrap_err();
            let message = error.message();
            assert!(message.contains(expected));
            assert!(!message.contains(secret));
            let request = server
                .requests
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer sk-provider-status-secret\r\n"));
            server.finish();
        }
    }

    #[test]
    fn provider_model_probe_handles_timeout_invalid_empty_and_large_responses() {
        let timeout_server = mock_models_server(
            200,
            r#"{"data":[]}"#.to_string(),
            Duration::from_millis(200),
            None,
        );
        let timeout = fetch_provider_models(
            &timeout_server.base_url,
            "sk-provider-timeout-secret",
            Duration::from_millis(50),
            Duration::from_millis(50),
        )
        .unwrap_err();
        assert!(matches!(timeout, ProviderProbeFailure::Timeout));
        let _ = timeout_server
            .requests
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        timeout_server.finish();

        let invalid_server = mock_models_server(200, "not-json".to_string(), Duration::ZERO, None);
        let invalid = fetch_provider_models(
            &invalid_server.base_url,
            "sk-provider-invalid-secret",
            Duration::from_millis(100),
            Duration::from_millis(250),
        )
        .unwrap_err();
        assert!(matches!(invalid, ProviderProbeFailure::InvalidResponse));
        invalid_server.finish();

        let empty_server =
            mock_models_server(200, r#"{"data":[]}"#.to_string(), Duration::ZERO, None);
        let empty = fetch_provider_models(
            &empty_server.base_url,
            "sk-provider-empty-secret",
            Duration::from_millis(100),
            Duration::from_millis(250),
        )
        .unwrap();
        assert!(empty.models.is_empty());
        assert!(!empty.truncated);
        empty_server.finish();

        let oversized_server = mock_models_server(
            200,
            "{}".to_string(),
            Duration::ZERO,
            Some(MAX_MODEL_RESPONSE_BYTES + 1),
        );
        let oversized = fetch_provider_models(
            &oversized_server.base_url,
            "sk-provider-large-secret",
            Duration::from_millis(100),
            Duration::from_millis(250),
        )
        .unwrap_err();
        assert!(matches!(oversized, ProviderProbeFailure::ResponseTooLarge));
        oversized_server.finish();
    }

    #[test]
    fn provider_model_probe_sorts_deduplicates_and_limits_models() {
        let mut models = vec![
            serde_json::json!({ "id": "gpt-z" }),
            serde_json::json!({ "id": "gpt-a" }),
            serde_json::json!({ "id": "gpt-a" }),
            serde_json::json!({ "id": "x".repeat(MAX_MODEL_BYTES + 1) }),
        ];
        models.extend(
            (0..MAX_MODELS_PER_PROVIDER + 4)
                .map(|index| serde_json::json!({ "id": format!("model-{index:03}") })),
        );
        let server = mock_models_server(
            200,
            serde_json::json!({ "data": models }).to_string(),
            Duration::ZERO,
            None,
        );
        let result = fetch_provider_models(
            &server.base_url,
            "sk-provider-models-secret",
            Duration::from_millis(100),
            Duration::from_millis(250),
        )
        .unwrap();
        assert_eq!(result.models.len(), MAX_MODELS_PER_PROVIDER);
        assert_eq!(result.models.first().map(String::as_str), Some("gpt-a"));
        assert!(result.truncated);
        assert!(!result
            .models
            .iter()
            .any(|model| model.len() > MAX_MODEL_BYTES));
        server.finish();
    }

    #[test]
    fn draft_model_discovery_uses_the_submitted_key_without_persisting_it() {
        let temp = tempfile::tempdir().unwrap();
        let service = AiCredentialService::open(temp.path()).unwrap();
        let server = mock_models_server(
            200,
            r#"{"data":[{"id":"gpt-draft"}]}"#.to_string(),
            Duration::ZERO,
            None,
        );
        let secret = "sk-provider-draft-secret";
        let command = AiCredentialCommandEnvelope::DiscoverModels {
            command_id: "discover-draft".to_string(),
            protocol_version: AI_CREDENTIAL_PROTOCOL_VERSION.to_string(),
            context: context(),
            payload: DiscoverAiProviderModelsPayload {
                provider_id: None,
                kind: AiProviderKind::OpenAiCompatible,
                base_url: server.base_url.clone(),
                api_key: Some(secret.to_string()),
            },
            idempotency_key: "idem-discover-draft".to_string(),
            expected_revision: Some(0),
            deadline_at: Some(now_millis() + 60_000),
        };
        assert!(!format!("{command:?}").contains(secret));
        assert!(!serde_json::to_string(&command).unwrap().contains(secret));

        let response = service.execute(command).unwrap();
        assert_eq!(response.status.revision, 0);
        assert!(!response.status.configured);
        let connection = response.connection_test.unwrap();
        assert_eq!(connection.state, AiProviderConnectionState::Ready);
        assert_eq!(connection.discovered_models, vec!["gpt-draft"]);
        assert!(service.load_api_key().unwrap().is_none());
        let request = server
            .requests
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer sk-provider-draft-secret\r\n"));
        server.finish();
    }

    #[test]
    fn legacy_single_key_state_migrates_in_place_without_losing_revision() {
        let temp = tempfile::tempdir().unwrap();
        let service = AiCredentialService::open(temp.path()).unwrap();
        let secret = "sk-legacy-provider-secret";
        let legacy = serde_json::json!({
            "schemaVersion": 1,
            "provider": "OpenAI API",
            "apiKey": secret,
            "revision": 7,
            "updatedAt": 1_720_000_000_i64,
            "receipts": []
        });
        let plaintext = serde_json::to_vec(&legacy).unwrap();
        let encrypted = protect_current_user(&plaintext).unwrap();
        fs::write(&service.state_path, encrypted).unwrap();

        let status = service.status().unwrap();
        assert_eq!(status.revision, 7);
        assert_eq!(status.default_provider_id.as_deref(), Some("bsaigc"));
        assert_eq!(status.providers.len(), 1);
        assert!(status.configured);
        assert_eq!(
            service
                .load_api_key()
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some(secret)
        );

        let migrated_disk = fs::read(&service.state_path).unwrap();
        assert!(!migrated_disk
            .windows(secret.len())
            .any(|window| window == secret.as_bytes()));
        let migrated_plaintext = Zeroizing::new(unprotect_current_user(&migrated_disk).unwrap());
        let migrated: serde_json::Value = serde_json::from_slice(&migrated_plaintext).unwrap();
        assert_eq!(migrated["schemaVersion"], STATE_SCHEMA_VERSION);
        assert_eq!(migrated["defaultProviderId"], "bsaigc");
        assert_eq!(migrated["providers"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn unsafe_remote_http_and_stale_or_expired_commands_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let service = AiCredentialService::open(temp.path()).unwrap();
        let mut unsafe_command = upsert_command(
            "provider-one",
            Some("sk-provider-validation-secret"),
            "model-one",
            true,
            0,
            "unsafe",
        );
        if let AiCredentialCommandEnvelope::UpsertProvider { payload, .. } = &mut unsafe_command {
            payload.base_url = "http://remote.example.invalid/v1".to_string();
        }
        let error = service.execute(unsafe_command).unwrap_err();
        assert_eq!(error.code, "VALIDATION_FAILED");
        assert_eq!(service.status().unwrap().revision, 0);

        let expired = AiCredentialCommandEnvelope::Status {
            command_id: "status-expired".to_string(),
            protocol_version: AI_CREDENTIAL_PROTOCOL_VERSION.to_string(),
            context: context(),
            idempotency_key: "idem-status-expired".to_string(),
            expected_revision: Some(0),
            deadline_at: Some(now_millis() - 1),
        };
        assert_eq!(
            service.execute(expired).unwrap_err().code,
            "COMMAND_DEADLINE_EXCEEDED"
        );

        service
            .execute(upsert_command(
                "provider-one",
                None,
                "model-one",
                true,
                0,
                "valid",
            ))
            .unwrap();
        let stale = AiCredentialCommandEnvelope::Status {
            command_id: "status-stale".to_string(),
            protocol_version: AI_CREDENTIAL_PROTOCOL_VERSION.to_string(),
            context: context(),
            idempotency_key: "idem-status-stale".to_string(),
            expected_revision: Some(0),
            deadline_at: Some(now_millis() + 60_000),
        };
        assert_eq!(
            service.execute(stale).unwrap_err().code,
            "AI_CREDENTIAL_REVISION_CONFLICT"
        );
    }
}
