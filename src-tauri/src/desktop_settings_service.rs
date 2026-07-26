use crate::protocol::{
    CacheClearResult, ChannelAdapterState, ChannelAdapterStatus, CloudBackupMode,
    CloudBackupStatus, CommandReceipt, DesktopBuildChannel, DesktopSettingsCommandEnvelope,
    DesktopSettingsCommandResponse, DesktopSettingsSnapshot, DesktopUpdateStatus, HostError,
    OperationContext, StorageLocationStatus, StorageLocationTarget, StorageSettingsStatus,
    DESKTOP_SETTINGS_PROTOCOL_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const SETTINGS_AGGREGATE_ID: &str = "desktop-settings";
const RECEIPT_LIMIT: i64 = 256;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 256;
const MAX_CONTEXT_VALUE_BYTES: usize = 512;
const LOGICAL_DATA_ROOT: &str = "bsaigc-storage://data-root";

const R2_ENDPOINT_ENV: &str = "BSAIGC_R2_ENDPOINT";
const R2_ACCOUNT_ID_ENV: &str = "BSAIGC_R2_ACCOUNT_ID";
const R2_BUCKET_ENV: &str = "BSAIGC_R2_BUCKET";
const R2_ACCESS_KEY_ENV: &str = "BSAIGC_R2_ACCESS_KEY_ID";
const R2_SECRET_KEY_ENV: &str = "BSAIGC_R2_SECRET_ACCESS_KEY";
const FEISHU_BIN_ENV: &str = "BSAIGC_FEISHU_CLI_BIN";
const FEISHU_APP_ID_ENV: &str = "BSAIGC_FEISHU_APP_ID";
const FEISHU_APP_SECRET_ENV: &str = "BSAIGC_FEISHU_APP_SECRET";
const UPDATE_SOURCE_ENV: &str = "BSAIGC_UPDATE_MANIFEST_URL";

type LocationOpener = dyn Fn(&Path) -> Result<(), HostError> + Send + Sync + 'static;

#[derive(Clone)]
pub(crate) struct DesktopSettingsService {
    data_root: PathBuf,
    database_path: PathBuf,
    runtime: RuntimeStatus,
    shells: ShellStatus,
    operation_lock: Arc<Mutex<()>>,
    location_opener: Arc<LocationOpener>,
    update_outcome: Arc<Mutex<Option<UpdateCheckOutcome>>>,
}

/// Result of the most recent online update check (in-memory only).
#[derive(Clone)]
struct UpdateCheckOutcome {
    state: String,
    message: String,
    latest_version: Option<String>,
    download_url: Option<String>,
}

#[derive(serde::Deserialize)]
struct UpdateManifest {
    version: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

/// Numeric dotted-version comparison: returns true when `candidate` is newer.
fn version_newer(candidate: &str, current: &str) -> bool {
    let parse = |value: &str| -> Vec<u64> {
        value
            .trim()
            .trim_start_matches('v')
            .split('.')
            .map(|part| {
                part.chars()
                    .take_while(|character| character.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u64>()
                    .unwrap_or(0)
            })
            .collect()
    };
    let candidate = parse(candidate);
    let current = parse(current);
    let length = candidate.len().max(current.len());
    for index in 0..length {
        let left = candidate.get(index).copied().unwrap_or(0);
        let right = current.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

#[derive(Clone)]
struct RuntimeStatus {
    app_version: String,
    build_version: String,
    codex_runtime_version: String,
    build_channel: DesktopBuildChannel,
}

#[derive(Clone)]
struct ShellStatus {
    r2: R2ShellStatus,
    feishu_settings_detected: bool,
    update_source_configured: bool,
}

#[derive(Clone)]
enum R2ShellStatus {
    NotConfigured,
    Incomplete,
    Invalid,
    Configured,
}

#[derive(Debug)]
struct CommandMeta {
    command_id: String,
    command_type: &'static str,
    context: OperationContext,
    idempotency_key: String,
    expected_revision: Option<i64>,
    deadline_at: Option<i64>,
}

#[derive(Debug)]
struct StoredReceipt {
    command_id: String,
    command_type: String,
    request_fingerprint: String,
    state: ReceiptState,
    revision: i64,
    prepared_result_json: Option<String>,
    response_json: Option<String>,
    completed_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReceiptState {
    Prepared,
    Completed,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreparedCacheClear {
    freed_bytes: i64,
    cleared_locations: Vec<String>,
}

#[derive(Debug)]
struct RemovalEntry {
    path: PathBuf,
    kind: RemovalKind,
    size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemovalKind {
    File,
    Directory,
}

impl DesktopSettingsService {
    /// Opens the desktop settings slice without exposing the resolved data path to the UI.
    /// The caller passes the verified/pinned Codex sidecar version from the Codex host.
    pub(crate) fn open(
        data_root: &Path,
        codex_runtime_version: impl Into<String>,
    ) -> Result<Self, HostError> {
        Self::open_with_dependencies(
            data_root,
            codex_runtime_version.into(),
            ShellStatus::from_environment(),
            Arc::new(open_native_location),
        )
    }

    fn open_with_dependencies(
        data_root: &Path,
        codex_runtime_version: String,
        shells: ShellStatus,
        location_opener: Arc<LocationOpener>,
    ) -> Result<Self, HostError> {
        let data_root = prepare_data_root(data_root)?;
        let ledger_root = prepare_managed_directory(&data_root, "ledger")?;
        let database_path = ledger_root.join("bsaigc.sqlite3");
        validate_managed_file_candidate(&data_root, &database_path)?;

        let connection = open_connection(&database_path)?;
        migrate(&connection)?;

        let app_version = env!("CARGO_PKG_VERSION").to_string();
        let build_version = option_env!("BSAIGC_BUILD_VERSION")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&app_version)
            .to_string();
        let runtime = RuntimeStatus {
            app_version,
            build_version,
            codex_runtime_version: normalize_runtime_version(codex_runtime_version)?,
            build_channel: compiled_build_channel(),
        };

        Ok(Self {
            data_root,
            update_outcome: Arc::new(Mutex::new(None)),
            database_path,
            runtime,
            shells,
            operation_lock: Arc::new(Mutex::new(())),
            location_opener,
        })
    }

    pub(crate) fn execute(
        &self,
        command: DesktopSettingsCommandEnvelope,
    ) -> Result<DesktopSettingsCommandResponse, HostError> {
        validate_command(&command)?;
        let meta = command_meta(&command);
        let fingerprint = request_fingerprint(&command, &meta);
        let _guard = self
            .operation_lock
            .lock()
            .map_err(|_| HostError::internal("desktop settings operation lock is poisoned"))?;
        let mut connection = open_connection(&self.database_path)?;
        migrate(&connection)?;

        if !matches!(command, DesktopSettingsCommandEnvelope::Status { .. }) {
            if let Some(stored) = find_existing_receipt(&connection, &meta, &fingerprint)? {
                return self.replay_receipt(&mut connection, stored);
            }
        }

        validate_deadline(meta.deadline_at)?;
        let revision = load_state(&connection)?.0;
        validate_expected_revision(meta.expected_revision, revision)?;

        match command {
            DesktopSettingsCommandEnvelope::Status { .. } => {
                self.status_response(&connection, &meta, revision)
            }
            DesktopSettingsCommandEnvelope::OpenStorageLocation { payload, .. } => {
                self.open_storage_location(&mut connection, &meta, &fingerprint, payload.target)
            }
            DesktopSettingsCommandEnvelope::ClearCache { .. } => {
                self.prepare_and_clear_cache(&mut connection, &meta, &fingerprint, revision)
            }
            DesktopSettingsCommandEnvelope::CheckForUpdates { .. } => {
                self.record_update_check(&mut connection, &meta, &fingerprint, revision)
            }
        }
    }

    fn replay_receipt(
        &self,
        connection: &mut Connection,
        stored: StoredReceipt,
    ) -> Result<DesktopSettingsCommandResponse, HostError> {
        if stored.state == ReceiptState::Prepared {
            if stored.command_type != "settings.clearCache" {
                return Err(HostError::new(
                    "DESKTOP_SETTINGS_RECEIPT_CORRUPT",
                    "only cache cleanup may have a prepared desktop settings receipt",
                    false,
                ));
            }
            return self.finish_prepared_cache_clear(connection, &stored, true);
        }

        let response_json = stored.response_json.ok_or_else(|| {
            HostError::new(
                "DESKTOP_SETTINGS_RECEIPT_CORRUPT",
                "completed desktop settings receipt has no response",
                false,
            )
        })?;
        let mut response: DesktopSettingsCommandResponse = serde_json::from_str(&response_json)
            .map_err(|_| {
                HostError::new(
                    "DESKTOP_SETTINGS_RECEIPT_CORRUPT",
                    "desktop settings receipt response is invalid",
                    false,
                )
            })?;
        response.replayed = true;
        Ok(response)
    }

    fn status_response(
        &self,
        connection: &Connection,
        meta: &CommandMeta,
        revision: i64,
    ) -> Result<DesktopSettingsCommandResponse, HostError> {
        let snapshot = self.snapshot(connection, revision)?;
        Ok(DesktopSettingsCommandResponse {
            receipt: command_receipt(meta, revision, now_millis()),
            snapshot,
            cache_clear: None,
            replayed: false,
        })
    }

    fn open_storage_location(
        &self,
        connection: &mut Connection,
        meta: &CommandMeta,
        fingerprint: &str,
        target: StorageLocationTarget,
    ) -> Result<DesktopSettingsCommandResponse, HostError> {
        let path = self.ensure_storage_location(&target)?;
        (self.location_opener)(&path)?;

        let revision = load_state(connection)?.0;
        let response = DesktopSettingsCommandResponse {
            receipt: command_receipt(meta, revision, now_millis()),
            snapshot: self.snapshot(connection, revision)?,
            cache_clear: None,
            replayed: false,
        };
        persist_completed_receipt(connection, meta, fingerprint, &response)?;
        prune_receipts(connection)?;
        Ok(response)
    }

    fn prepare_and_clear_cache(
        &self,
        connection: &mut Connection,
        meta: &CommandMeta,
        fingerprint: &str,
        revision: i64,
    ) -> Result<DesktopSettingsCommandResponse, HostError> {
        let cache_root = self.ensure_storage_location(&StorageLocationTarget::Cache)?;
        let freed_bytes = strict_directory_size(&self.data_root, &cache_root)?;
        let next_revision = revision.checked_add(1).ok_or_else(|| {
            HostError::new(
                "DESKTOP_SETTINGS_REVISION_EXHAUSTED",
                "desktop settings revision is exhausted",
                false,
            )
        })?;
        let prepared = PreparedCacheClear {
            freed_bytes,
            cleared_locations: vec!["cache".to_string()],
        };

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        if let Some(stored) = find_existing_receipt_tx(&transaction, meta, fingerprint)? {
            transaction.commit().map_err(sql_error)?;
            return self.replay_receipt(connection, stored);
        }
        let actual_revision = load_state_tx(&transaction)?.0;
        validate_expected_revision(Some(revision), actual_revision)?;
        transaction
            .execute(
                "UPDATE desktop_settings_state SET revision = ?1 WHERE singleton = 1",
                [next_revision],
            )
            .map_err(sql_error)?;
        insert_prepared_receipt(&transaction, meta, fingerprint, next_revision, &prepared)?;
        transaction.commit().map_err(sql_error)?;

        let stored = require_receipt_by_key(connection, &meta.idempotency_key)?;
        self.finish_prepared_cache_clear(connection, &stored, false)
    }

    fn finish_prepared_cache_clear(
        &self,
        connection: &mut Connection,
        stored: &StoredReceipt,
        replayed: bool,
    ) -> Result<DesktopSettingsCommandResponse, HostError> {
        let prepared_json = stored.prepared_result_json.as_deref().ok_or_else(|| {
            HostError::new(
                "DESKTOP_SETTINGS_RECEIPT_CORRUPT",
                "prepared cache cleanup receipt has no result summary",
                false,
            )
        })?;
        let prepared: PreparedCacheClear = serde_json::from_str(prepared_json).map_err(|_| {
            HostError::new(
                "DESKTOP_SETTINGS_RECEIPT_CORRUPT",
                "prepared cache cleanup result is invalid",
                false,
            )
        })?;
        let cache_root = self.ensure_storage_location(&StorageLocationTarget::Cache)?;
        clear_directory_contents(&self.data_root, &cache_root)?;

        let meta = CommandMeta {
            command_id: stored.command_id.clone(),
            command_type: "settings.clearCache",
            context: OperationContext {
                actor_id: "receipt-replay".to_string(),
                account_id: None,
                project_id: None,
                window_id: "desktop-settings".to_string(),
                trace_id: "receipt-replay".to_string(),
            },
            idempotency_key: String::new(),
            expected_revision: None,
            deadline_at: None,
        };
        let response = DesktopSettingsCommandResponse {
            receipt: CommandReceipt {
                command_id: stored.command_id.clone(),
                idempotency_key: receipt_key(connection, &stored.command_id)?,
                command_type: stored.command_type.clone(),
                aggregate_id: SETTINGS_AGGREGATE_ID.to_string(),
                revision: stored.revision,
                last_event_sequence: 0,
                completed_at: stored.completed_at,
            },
            snapshot: self.snapshot(connection, stored.revision)?,
            cache_clear: Some(CacheClearResult {
                freed_bytes: prepared.freed_bytes,
                cleared_locations: prepared.cleared_locations,
            }),
            replayed,
        };
        complete_prepared_receipt(connection, &meta.command_id, &response)?;
        prune_receipts(connection)?;
        Ok(response)
    }

    fn record_update_check(
        &self,
        connection: &mut Connection,
        meta: &CommandMeta,
        fingerprint: &str,
        revision: i64,
    ) -> Result<DesktopSettingsCommandResponse, HostError> {
        self.perform_update_check();
        let now = now_millis();
        let next_revision = revision.checked_add(1).ok_or_else(|| {
            HostError::new(
                "DESKTOP_SETTINGS_REVISION_EXHAUSTED",
                "desktop settings revision is exhausted",
                false,
            )
        })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        if let Some(stored) = find_existing_receipt_tx(&transaction, meta, fingerprint)? {
            transaction.commit().map_err(sql_error)?;
            return self.replay_receipt(connection, stored);
        }
        let actual_revision = load_state_tx(&transaction)?.0;
        validate_expected_revision(Some(revision), actual_revision)?;
        transaction
            .execute(
                "UPDATE desktop_settings_state
                 SET revision = ?1, last_update_checked_at = ?2
                 WHERE singleton = 1",
                params![next_revision, now],
            )
            .map_err(sql_error)?;
        let response = DesktopSettingsCommandResponse {
            receipt: command_receipt(meta, next_revision, now),
            snapshot: self.snapshot(&transaction, next_revision)?,
            cache_clear: None,
            replayed: false,
        };
        insert_completed_receipt(&transaction, meta, fingerprint, &response)?;
        transaction.commit().map_err(sql_error)?;
        prune_receipts(connection)?;
        Ok(response)
    }

    fn snapshot(
        &self,
        connection: &Connection,
        revision: i64,
    ) -> Result<DesktopSettingsSnapshot, HostError> {
        let (_, last_update_checked_at) = load_state(connection)?;
        let storage = self.storage_status()?;
        let pending_items = pending_backup_items(connection)?;
        Ok(DesktopSettingsSnapshot {
            storage,
            channel_adapters: vec![self.feishu_status()],
            cloud_backup: self.r2_status(pending_items),
            update: self.update_status(last_update_checked_at),
            revision,
        })
    }

    fn storage_status(&self) -> Result<StorageSettingsStatus, HostError> {
        let specs = [
            (
                StorageLocationTarget::Ledger,
                "Local ledger",
                "ledger",
                true,
                false,
            ),
            (
                StorageLocationTarget::Vault,
                "Local Vault",
                "vault",
                true,
                false,
            ),
            (
                StorageLocationTarget::Cache,
                "Regenerable cache",
                "cache",
                false,
                true,
            ),
            (
                StorageLocationTarget::Staging,
                "Task staging",
                "staging",
                false,
                false,
            ),
            (
                StorageLocationTarget::Credentials,
                "Protected credentials",
                "credentials",
                true,
                false,
            ),
        ];
        let mut locations = Vec::with_capacity(specs.len() + 1);
        let mut total_bytes = 0_i64;
        let mut cache_bytes = 0_i64;
        for (target, label, relative, authoritative, clearable) in specs {
            let path = self.data_root.join(relative);
            let (exists, size_bytes) = inspect_location(&self.data_root, &path)?;
            total_bytes = total_bytes.saturating_add(size_bytes);
            if target == StorageLocationTarget::Cache {
                cache_bytes = size_bytes;
            }
            locations.push(StorageLocationStatus {
                target,
                label: label.to_string(),
                path: format!("bsaigc-storage://{relative}"),
                size_bytes,
                exists,
                authoritative,
                clearable,
            });
        }
        locations.insert(
            0,
            StorageLocationStatus {
                target: StorageLocationTarget::DataRoot,
                label: "Application data".to_string(),
                path: LOGICAL_DATA_ROOT.to_string(),
                size_bytes: total_bytes,
                exists: true,
                authoritative: true,
                clearable: false,
            },
        );
        Ok(StorageSettingsStatus {
            data_root: LOGICAL_DATA_ROOT.to_string(),
            total_bytes,
            cache_bytes,
            locations,
        })
    }

    fn ensure_storage_location(
        &self,
        target: &StorageLocationTarget,
    ) -> Result<PathBuf, HostError> {
        let relative = match target {
            StorageLocationTarget::DataRoot => return Ok(self.data_root.clone()),
            StorageLocationTarget::Ledger => "ledger",
            StorageLocationTarget::Vault => "vault",
            StorageLocationTarget::Cache => "cache",
            StorageLocationTarget::Staging => "staging",
            StorageLocationTarget::Credentials => "credentials",
        };
        prepare_managed_directory(&self.data_root, relative)
    }

    fn r2_status(&self, pending_items: i64) -> CloudBackupStatus {
        let (configured, state, message) = match self.shells.r2 {
            R2ShellStatus::NotConfigured => (
                false,
                "notConfigured",
                "R2 backup is not configured; Local Vault remains authoritative.",
            ),
            R2ShellStatus::Incomplete => (
                false,
                "degraded",
                "R2 settings are incomplete; no remote operation was attempted.",
            ),
            R2ShellStatus::Invalid => (
                false,
                "degraded",
                "R2 settings are invalid; no remote operation was attempted.",
            ),
            R2ShellStatus::Configured => (
                true,
                "adapterPending",
                "R2 settings are present; this settings slice does not claim transport readiness.",
            ),
        };
        CloudBackupStatus {
            provider: "cloudflare-r2".to_string(),
            mode: CloudBackupMode::AsyncBackupOnly,
            configured,
            ready: false,
            state: state.to_string(),
            message: message.to_string(),
            pending_items,
        }
    }

    fn feishu_status(&self) -> ChannelAdapterStatus {
        let (state, message) = if self.shells.feishu_settings_detected {
            (
                ChannelAdapterState::Degraded,
                "Feishu CLI settings were detected, but the channel adapter is not connected.",
            )
        } else {
            (
                ChannelAdapterState::Planned,
                "Feishu CLI is a reserved channel adapter for a later release.",
            )
        };
        ChannelAdapterStatus {
            id: "feishu-cli".to_string(),
            name: "Feishu CLI".to_string(),
            state,
            configured: false,
            capabilities: vec![
                "message.receive".to_string(),
                "message.send".to_string(),
                "attachment.reference".to_string(),
            ],
            message: message.to_string(),
        }
    }

    fn resolve_update_manifest_url(&self) -> Option<String> {
        if let Ok(value) = std::env::var(UPDATE_SOURCE_ENV) {
            let value = value.trim().to_string();
            if value.starts_with("https://") {
                return Some(value);
            }
        }
        crate::r2_backup::load_update_manifest_url()
    }

    /// Fetches the release manifest and remembers the outcome. Only invoked
    /// from the explicit "check for updates" command, never from passive
    /// status reads, so the UI stays snappy.
    fn perform_update_check(&self) {
        let Some(url) = self.resolve_update_manifest_url() else {
            if let Ok(mut guard) = self.update_outcome.lock() {
                *guard = None;
            }
            return;
        };
        let fetched = (|| -> Result<UpdateManifest, String> {
            let client = reqwest::blocking::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|error| format!("初始化网络客户端失败:{error}"))?;
            let response = client
                .get(&url)
                .send()
                .map_err(|error| format!("联网失败:{error}"))?;
            if !response.status().is_success() {
                return Err(format!("更新源返回 {}", response.status()));
            }
            response
                .json::<UpdateManifest>()
                .map_err(|error| format!("更新信息格式有误:{error}"))
        })();
        let outcome = match fetched {
            Ok(manifest) => {
                let latest = manifest.version.trim().to_string();
                if version_newer(&latest, &self.runtime.app_version) {
                    let notes = manifest
                        .notes
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty());
                    UpdateCheckOutcome {
                        state: "available".to_string(),
                        message: match notes {
                            Some(notes) => format!("发现新版本 {latest}:{notes}"),
                            None => format!("发现新版本 {latest},点下载按钮获取安装包。"),
                        },
                        latest_version: Some(latest),
                        download_url: manifest
                            .url
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| value.starts_with("https://"))
                            .map(str::to_string),
                    }
                } else {
                    UpdateCheckOutcome {
                        state: "upToDate".to_string(),
                        message: format!("当前已是最新版本({})。", self.runtime.app_version),
                        latest_version: Some(latest),
                        download_url: None,
                    }
                }
            }
            Err(message) => UpdateCheckOutcome {
                state: "failed".to_string(),
                message: format!("检查更新未完成:{message}"),
                latest_version: None,
                download_url: None,
            },
        };
        if let Ok(mut guard) = self.update_outcome.lock() {
            *guard = Some(outcome);
        }
    }

    fn update_status(&self, last_checked_at: Option<i64>) -> DesktopUpdateStatus {
        let source_configured =
            self.shells.update_source_configured || self.resolve_update_manifest_url().is_some();
        let outcome = self
            .update_outcome
            .lock()
            .ok()
            .and_then(|guard| guard.clone());
        let (state, message, latest_version, download_url) = match outcome {
            Some(outcome) => (
                outcome.state,
                outcome.message,
                outcome.latest_version,
                outcome.download_url,
            ),
            None if source_configured => (
                "idle".to_string(),
                "在线更新已接通,点「检查更新」联网比对最新版本。".to_string(),
                None,
                None,
            ),
            None => (
                "notConfigured".to_string(),
                "未配置在线更新源。".to_string(),
                None,
                None,
            ),
        };
        DesktopUpdateStatus {
            current_version: self.runtime.app_version.clone(),
            build_channel: self.runtime.build_channel.clone(),
            build_version: self.runtime.build_version.clone(),
            codex_runtime_version: self.runtime.codex_runtime_version.clone(),
            update_source_configured: source_configured,
            automatic_install_allowed: false,
            state,
            message,
            latest_version,
            download_url,
            last_checked_at,
        }
    }
}

impl ShellStatus {
    fn from_environment() -> Self {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup<F>(lookup: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let endpoint = non_empty(lookup(R2_ENDPOINT_ENV));
        let account_id = non_empty(lookup(R2_ACCOUNT_ID_ENV));
        let bucket = non_empty(lookup(R2_BUCKET_ENV));
        let access_key = non_empty(lookup(R2_ACCESS_KEY_ENV));
        let secret_key = non_empty(lookup(R2_SECRET_KEY_ENV));
        let r2_any = endpoint.is_some()
            || account_id.is_some()
            || bucket.is_some()
            || access_key.is_some()
            || secret_key.is_some();
        let r2_complete = (endpoint.is_some() || account_id.is_some())
            && bucket.is_some()
            && access_key.is_some()
            && secret_key.is_some();
        let r2_valid = endpoint
            .as_deref()
            .map(|value| value.starts_with("https://"))
            .unwrap_or(true)
            && bucket.as_deref().map(valid_bucket_name).unwrap_or(true)
            && account_id
                .as_deref()
                .map(|value| {
                    value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                })
                .unwrap_or(true);
        let r2 = if !r2_any {
            R2ShellStatus::NotConfigured
        } else if !r2_complete {
            R2ShellStatus::Incomplete
        } else if !r2_valid {
            R2ShellStatus::Invalid
        } else {
            R2ShellStatus::Configured
        };
        let feishu_settings_detected = [
            non_empty(lookup(FEISHU_BIN_ENV)),
            non_empty(lookup(FEISHU_APP_ID_ENV)),
            non_empty(lookup(FEISHU_APP_SECRET_ENV)),
        ]
        .into_iter()
        .any(|value| value.is_some());
        let update_source_configured =
            non_empty(lookup(UPDATE_SOURCE_ENV)).is_some_and(|value| value.starts_with("https://"));
        Self {
            r2,
            feishu_settings_detected,
            update_source_configured,
        }
    }
}

fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS desktop_settings_state (
                singleton INTEGER PRIMARY KEY NOT NULL CHECK(singleton = 1),
                revision INTEGER NOT NULL CHECK(revision >= 0),
                last_update_checked_at INTEGER
            );
            INSERT OR IGNORE INTO desktop_settings_state
                (singleton, revision, last_update_checked_at)
                VALUES (1, 0, NULL);

            CREATE TABLE IF NOT EXISTS desktop_settings_command_receipts (
                idempotency_key TEXT PRIMARY KEY NOT NULL,
                command_id TEXT NOT NULL UNIQUE,
                command_type TEXT NOT NULL,
                request_fingerprint TEXT NOT NULL CHECK(length(request_fingerprint) = 64),
                state TEXT NOT NULL CHECK(state IN ('prepared', 'completed')),
                revision INTEGER NOT NULL CHECK(revision >= 0),
                prepared_result_json TEXT,
                response_json TEXT,
                completed_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_desktop_settings_receipts_completed
                ON desktop_settings_command_receipts(completed_at);
            "#,
        )
        .map_err(sql_error)
}

fn open_connection(path: &Path) -> Result<Connection, HostError> {
    let connection = Connection::open(path).map_err(sql_error)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(sql_error)?;
    Ok(connection)
}

fn load_state(connection: &Connection) -> Result<(i64, Option<i64>), HostError> {
    connection
        .query_row(
            "SELECT revision, last_update_checked_at
             FROM desktop_settings_state WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(sql_error)
}

fn load_state_tx(transaction: &Transaction<'_>) -> Result<(i64, Option<i64>), HostError> {
    transaction
        .query_row(
            "SELECT revision, last_update_checked_at
             FROM desktop_settings_state WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(sql_error)
}

fn persist_completed_receipt(
    connection: &mut Connection,
    meta: &CommandMeta,
    fingerprint: &str,
    response: &DesktopSettingsCommandResponse,
) -> Result<(), HostError> {
    let response_json = serde_json::to_string(response).map_err(json_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    insert_completed_receipt_json(&transaction, meta, fingerprint, response, &response_json)?;
    transaction.commit().map_err(sql_error)
}

fn insert_completed_receipt(
    transaction: &Transaction<'_>,
    meta: &CommandMeta,
    fingerprint: &str,
    response: &DesktopSettingsCommandResponse,
) -> Result<(), HostError> {
    let response_json = serde_json::to_string(response).map_err(json_error)?;
    insert_completed_receipt_json(transaction, meta, fingerprint, response, &response_json)
}

fn insert_completed_receipt_json(
    transaction: &Transaction<'_>,
    meta: &CommandMeta,
    fingerprint: &str,
    response: &DesktopSettingsCommandResponse,
    response_json: &str,
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO desktop_settings_command_receipts
             (idempotency_key, command_id, command_type, request_fingerprint, state,
              revision, prepared_result_json, response_json, completed_at)
             VALUES (?1, ?2, ?3, ?4, 'completed', ?5, NULL, ?6, ?7)",
            params![
                meta.idempotency_key,
                meta.command_id,
                meta.command_type,
                fingerprint,
                response.snapshot.revision,
                response_json,
                response.receipt.completed_at,
            ],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn insert_prepared_receipt(
    transaction: &Transaction<'_>,
    meta: &CommandMeta,
    fingerprint: &str,
    revision: i64,
    prepared: &PreparedCacheClear,
) -> Result<(), HostError> {
    let prepared_json = serde_json::to_string(prepared).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO desktop_settings_command_receipts
             (idempotency_key, command_id, command_type, request_fingerprint, state,
              revision, prepared_result_json, response_json, completed_at)
             VALUES (?1, ?2, ?3, ?4, 'prepared', ?5, ?6, NULL, ?7)",
            params![
                meta.idempotency_key,
                meta.command_id,
                meta.command_type,
                fingerprint,
                revision,
                prepared_json,
                now_millis(),
            ],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn complete_prepared_receipt(
    connection: &mut Connection,
    command_id: &str,
    response: &DesktopSettingsCommandResponse,
) -> Result<(), HostError> {
    let response_json = serde_json::to_string(response).map_err(json_error)?;
    let changed = connection
        .execute(
            "UPDATE desktop_settings_command_receipts
             SET state = 'completed', response_json = ?1
             WHERE command_id = ?2 AND state = 'prepared'",
            params![response_json, command_id],
        )
        .map_err(sql_error)?;
    if changed != 1 {
        return Err(HostError::new(
            "DESKTOP_SETTINGS_RECEIPT_CONFLICT",
            "prepared cache cleanup receipt changed before completion",
            true,
        ));
    }
    Ok(())
}

fn find_existing_receipt(
    connection: &Connection,
    meta: &CommandMeta,
    fingerprint: &str,
) -> Result<Option<StoredReceipt>, HostError> {
    let by_key = query_receipt_by_key(connection, &meta.idempotency_key)?;
    let by_command = query_receipt_by_command(connection, &meta.command_id)?;
    validate_receipt_identity(by_key, by_command, meta, fingerprint)
}

fn find_existing_receipt_tx(
    transaction: &Transaction<'_>,
    meta: &CommandMeta,
    fingerprint: &str,
) -> Result<Option<StoredReceipt>, HostError> {
    let by_key = query_receipt_by_key(transaction, &meta.idempotency_key)?;
    let by_command = query_receipt_by_command(transaction, &meta.command_id)?;
    validate_receipt_identity(by_key, by_command, meta, fingerprint)
}

fn validate_receipt_identity(
    by_key: Option<StoredReceipt>,
    by_command: Option<StoredReceipt>,
    meta: &CommandMeta,
    fingerprint: &str,
) -> Result<Option<StoredReceipt>, HostError> {
    if let Some(receipt) = by_key {
        if receipt.command_id != meta.command_id
            || receipt.command_type != meta.command_type
            || receipt.request_fingerprint != fingerprint
        {
            return Err(HostError::new(
                "DESKTOP_SETTINGS_IDEMPOTENCY_CONFLICT",
                "idempotencyKey was already used by a different desktop settings command",
                false,
            ));
        }
        if let Some(command_receipt) = by_command {
            if command_receipt.command_id != receipt.command_id {
                return Err(HostError::new(
                    "DESKTOP_SETTINGS_COMMAND_CONFLICT",
                    "commandId and idempotencyKey refer to different settings commands",
                    false,
                ));
            }
        }
        return Ok(Some(receipt));
    }
    if by_command.is_some() {
        return Err(HostError::new(
            "DESKTOP_SETTINGS_COMMAND_CONFLICT",
            "commandId was already used with a different idempotencyKey",
            false,
        ));
    }
    Ok(None)
}

fn query_receipt_by_key(
    connection: &Connection,
    idempotency_key: &str,
) -> Result<Option<StoredReceipt>, HostError> {
    query_receipt(connection, "idempotency_key", idempotency_key)
}

fn query_receipt_by_command(
    connection: &Connection,
    command_id: &str,
) -> Result<Option<StoredReceipt>, HostError> {
    query_receipt(connection, "command_id", command_id)
}

fn query_receipt(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<StoredReceipt>, HostError> {
    let sql = format!(
        "SELECT command_id, command_type, request_fingerprint, state, revision,
                prepared_result_json, response_json, completed_at
         FROM desktop_settings_command_receipts WHERE {column} = ?1"
    );
    connection
        .query_row(&sql, [value], |row| {
            let state: String = row.get(3)?;
            Ok(StoredReceipt {
                command_id: row.get(0)?,
                command_type: row.get(1)?,
                request_fingerprint: row.get(2)?,
                state: if state == "prepared" {
                    ReceiptState::Prepared
                } else {
                    ReceiptState::Completed
                },
                revision: row.get(4)?,
                prepared_result_json: row.get(5)?,
                response_json: row.get(6)?,
                completed_at: row.get(7)?,
            })
        })
        .optional()
        .map_err(sql_error)
}

fn require_receipt_by_key(
    connection: &Connection,
    idempotency_key: &str,
) -> Result<StoredReceipt, HostError> {
    query_receipt(connection, "idempotency_key", idempotency_key)?.ok_or_else(|| {
        HostError::internal("prepared desktop settings receipt could not be recovered")
    })
}

fn receipt_key(connection: &Connection, command_id: &str) -> Result<String, HostError> {
    connection
        .query_row(
            "SELECT idempotency_key FROM desktop_settings_command_receipts
             WHERE command_id = ?1",
            [command_id],
            |row| row.get(0),
        )
        .map_err(sql_error)
}

fn prune_receipts(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute(
            "DELETE FROM desktop_settings_command_receipts
             WHERE state = 'completed' AND command_id IN (
                 SELECT command_id FROM desktop_settings_command_receipts
                 WHERE state = 'completed'
                 ORDER BY completed_at DESC, command_id DESC
                 LIMIT -1 OFFSET ?1
             )",
            [RECEIPT_LIMIT],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn pending_backup_items(connection: &Connection) -> Result<i64, HostError> {
    let table_exists = connection
        .query_row(
            "SELECT 1 FROM sqlite_master
             WHERE type = 'table' AND name = 'asset_backups'",
            [],
            |_| Ok(()),
        )
        .optional()
        .map_err(sql_error)?
        .is_some();
    if !table_exists {
        return Ok(0);
    }
    connection
        .query_row(
            "SELECT COUNT(*) FROM asset_backups
             WHERE state IN ('queued', 'uploading', 'failed')",
            [],
            |row| row.get(0),
        )
        .map_err(sql_error)
}

fn prepare_data_root(path: &Path) -> Result<PathBuf, HostError> {
    if path.as_os_str().is_empty() {
        return Err(HostError::validation("data root cannot be empty"));
    }
    fs::create_dir_all(path).map_err(|error| path_io_error("create data root", error))?;
    let metadata =
        fs::symlink_metadata(path).map_err(|error| path_io_error("inspect data root", error))?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(unsafe_path_error("data root must be a regular directory"));
    }
    let resolved =
        fs::canonicalize(path).map_err(|error| path_io_error("resolve data root", error))?;
    if resolved.parent().is_none() {
        return Err(unsafe_path_error(
            "filesystem root cannot be used as application data root",
        ));
    }
    Ok(resolved)
}

fn prepare_managed_directory(data_root: &Path, relative: &str) -> Result<PathBuf, HostError> {
    let path = data_root.join(relative);
    if !path.starts_with(data_root) || path.parent() != Some(data_root) {
        return Err(unsafe_path_error(
            "managed storage path escaped the data root",
        ));
    }
    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if !metadata.is_dir() || is_link_or_reparse(&metadata) {
                return Err(unsafe_path_error(
                    "managed storage path is not a regular directory",
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&path)
                .map_err(|error| path_io_error("create managed storage directory", error))?;
        }
        Err(error) => return Err(path_io_error("inspect managed storage directory", error)),
    }
    let resolved = fs::canonicalize(&path)
        .map_err(|error| path_io_error("resolve managed storage directory", error))?;
    if !resolved.starts_with(data_root) || resolved.parent() != Some(data_root) {
        return Err(unsafe_path_error(
            "managed storage directory escaped the data root",
        ));
    }
    Ok(resolved)
}

fn validate_managed_file_candidate(data_root: &Path, path: &Path) -> Result<(), HostError> {
    if !path.starts_with(data_root) {
        return Err(unsafe_path_error("managed file escaped the data root"));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !is_link_or_reparse(&metadata) => Ok(()),
        Ok(_) => Err(unsafe_path_error("managed database must be a regular file")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(path_io_error("inspect managed database", error)),
    }
}

fn inspect_location(data_root: &Path, path: &Path) -> Result<(bool, i64), HostError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok((false, 0)),
        Err(error) => Err(path_io_error("inspect storage location", error)),
        Ok(metadata) => {
            if !metadata.is_dir() || is_link_or_reparse(&metadata) {
                return Err(unsafe_path_error(
                    "storage location is not a regular directory",
                ));
            }
            let resolved = fs::canonicalize(path)
                .map_err(|error| path_io_error("resolve storage location", error))?;
            if !resolved.starts_with(data_root) {
                return Err(unsafe_path_error("storage location escaped the data root"));
            }
            Ok((true, directory_size_no_follow(data_root, &resolved)?))
        }
    }
}

fn directory_size_no_follow(data_root: &Path, root: &Path) -> Result<i64, HostError> {
    let mut total = 0_u64;
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        ensure_existing_directory_inside(data_root, &directory)?;
        for entry in fs::read_dir(&directory)
            .map_err(|error| path_io_error("read storage directory", error))?
        {
            let entry = entry.map_err(|error| path_io_error("read storage entry", error))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| path_io_error("inspect storage entry", error))?;
            if is_link_or_reparse(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    Ok(u64_to_i64(total))
}

fn strict_directory_size(data_root: &Path, root: &Path) -> Result<i64, HostError> {
    let entries = build_removal_plan(data_root, root)?;
    let total = entries
        .iter()
        .fold(0_u64, |total, entry| total.saturating_add(entry.size_bytes));
    Ok(u64_to_i64(total))
}

fn clear_directory_contents(data_root: &Path, root: &Path) -> Result<(), HostError> {
    let mut entries = build_removal_plan(data_root, root)?;
    entries.sort_by(|left, right| {
        right
            .path
            .components()
            .count()
            .cmp(&left.path.components().count())
            .then_with(|| right.path.cmp(&left.path))
    });
    for entry in entries {
        let metadata = match fs::symlink_metadata(&entry.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(path_io_error("inspect cache cleanup entry", error)),
        };
        if is_link_or_reparse(&metadata) {
            return Err(unsafe_path_error(
                "cache changed to a link or reparse point during cleanup",
            ));
        }
        match entry.kind {
            RemovalKind::File if metadata.is_file() => fs::remove_file(&entry.path)
                .map_err(|error| path_io_error("remove cache file", error))?,
            RemovalKind::Directory if metadata.is_dir() => fs::remove_dir(&entry.path)
                .map_err(|error| path_io_error("remove cache directory", error))?,
            _ => {
                return Err(unsafe_path_error("cache entry type changed during cleanup"));
            }
        }
    }
    Ok(())
}

fn build_removal_plan(data_root: &Path, root: &Path) -> Result<Vec<RemovalEntry>, HostError> {
    ensure_existing_directory_inside(data_root, root)?;
    let mut plan = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        ensure_existing_directory_inside(root, &directory)?;
        for entry in fs::read_dir(&directory)
            .map_err(|error| path_io_error("read cache directory", error))?
        {
            let entry = entry.map_err(|error| path_io_error("read cache entry", error))?;
            let path = entry.path();
            if !path.starts_with(root) {
                return Err(unsafe_path_error("cache entry escaped the cache root"));
            }
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| path_io_error("inspect cache entry", error))?;
            if is_link_or_reparse(&metadata) {
                return Err(unsafe_path_error(
                    "cache cleanup refuses links and reparse points",
                ));
            }
            if metadata.is_dir() {
                let resolved = fs::canonicalize(&path)
                    .map_err(|error| path_io_error("resolve cache directory", error))?;
                if !resolved.starts_with(root) || !resolved.starts_with(data_root) {
                    return Err(unsafe_path_error("cache directory escaped the data root"));
                }
                pending.push(resolved.clone());
                plan.push(RemovalEntry {
                    path: resolved,
                    kind: RemovalKind::Directory,
                    size_bytes: 0,
                });
            } else if metadata.is_file() {
                plan.push(RemovalEntry {
                    path,
                    kind: RemovalKind::File,
                    size_bytes: metadata.len(),
                });
            } else {
                return Err(unsafe_path_error(
                    "cache cleanup only accepts regular files and directories",
                ));
            }
        }
    }
    Ok(plan)
}

fn ensure_existing_directory_inside(root: &Path, path: &Path) -> Result<(), HostError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| path_io_error("inspect managed directory", error))?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(unsafe_path_error("managed directory is unsafe"));
    }
    let resolved = fs::canonicalize(path)
        .map_err(|error| path_io_error("resolve managed directory", error))?;
    if !resolved.starts_with(root) {
        return Err(unsafe_path_error("managed directory escaped its root"));
    }
    Ok(())
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn open_native_location(path: &Path) -> Result<(), HostError> {
    ensure_openable_directory(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        Command::new("explorer.exe")
            .arg(path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|error| path_io_error("open storage location", error))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|error| path_io_error("open storage location", error))?;
        Ok(())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|error| path_io_error("open storage location", error))?;
        Ok(())
    }
}

fn ensure_openable_directory(path: &Path) -> Result<(), HostError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| path_io_error("inspect storage location before opening", error))?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(unsafe_path_error("storage location is unsafe to open"));
    }
    Ok(())
}

fn validate_command(command: &DesktopSettingsCommandEnvelope) -> Result<(), HostError> {
    let meta = command_meta(command);
    if command_protocol_version(command) != DESKTOP_SETTINGS_PROTOCOL_VERSION {
        return Err(HostError::new(
            "UNSUPPORTED_PROTOCOL_VERSION",
            format!(
                "desktop settings requires protocol {}",
                DESKTOP_SETTINGS_PROTOCOL_VERSION
            ),
            false,
        ));
    }
    Uuid::parse_str(&meta.command_id)
        .map_err(|_| HostError::validation("commandId must be a UUID"))?;
    validate_text(
        "idempotencyKey",
        &meta.idempotency_key,
        MAX_IDEMPOTENCY_KEY_BYTES,
    )?;
    validate_text("actorId", &meta.context.actor_id, MAX_CONTEXT_VALUE_BYTES)?;
    validate_text("windowId", &meta.context.window_id, MAX_CONTEXT_VALUE_BYTES)?;
    validate_text("traceId", &meta.context.trace_id, MAX_CONTEXT_VALUE_BYTES)?;
    if meta.expected_revision.is_some_and(|value| value < 0) {
        return Err(HostError::validation("expectedRevision cannot be negative"));
    }
    Ok(())
}

fn command_protocol_version(command: &DesktopSettingsCommandEnvelope) -> &str {
    match command {
        DesktopSettingsCommandEnvelope::Status {
            protocol_version, ..
        }
        | DesktopSettingsCommandEnvelope::OpenStorageLocation {
            protocol_version, ..
        }
        | DesktopSettingsCommandEnvelope::ClearCache {
            protocol_version, ..
        }
        | DesktopSettingsCommandEnvelope::CheckForUpdates {
            protocol_version, ..
        } => protocol_version,
    }
}

fn command_meta(command: &DesktopSettingsCommandEnvelope) -> CommandMeta {
    match command {
        DesktopSettingsCommandEnvelope::Status {
            command_id,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => CommandMeta {
            command_id: command_id.clone(),
            command_type: "settings.status",
            context: context.clone(),
            idempotency_key: idempotency_key.clone(),
            expected_revision: *expected_revision,
            deadline_at: *deadline_at,
        },
        DesktopSettingsCommandEnvelope::OpenStorageLocation {
            command_id,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => CommandMeta {
            command_id: command_id.clone(),
            command_type: "settings.openStorageLocation",
            context: context.clone(),
            idempotency_key: idempotency_key.clone(),
            expected_revision: *expected_revision,
            deadline_at: *deadline_at,
        },
        DesktopSettingsCommandEnvelope::ClearCache {
            command_id,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => CommandMeta {
            command_id: command_id.clone(),
            command_type: "settings.clearCache",
            context: context.clone(),
            idempotency_key: idempotency_key.clone(),
            expected_revision: *expected_revision,
            deadline_at: *deadline_at,
        },
        DesktopSettingsCommandEnvelope::CheckForUpdates {
            command_id,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => CommandMeta {
            command_id: command_id.clone(),
            command_type: "settings.checkForUpdates",
            context: context.clone(),
            idempotency_key: idempotency_key.clone(),
            expected_revision: *expected_revision,
            deadline_at: *deadline_at,
        },
    }
}

fn request_fingerprint(command: &DesktopSettingsCommandEnvelope, meta: &CommandMeta) -> String {
    let mut digest = Sha256::new();
    digest_text(&mut digest, meta.command_type);
    digest_text(&mut digest, &meta.context.actor_id);
    digest_optional(&mut digest, meta.context.account_id.as_deref());
    digest_optional(&mut digest, meta.context.project_id.as_deref());
    digest_text(&mut digest, &meta.context.window_id);
    digest_optional(
        &mut digest,
        meta.expected_revision
            .map(|value| value.to_string())
            .as_deref(),
    );
    if let DesktopSettingsCommandEnvelope::OpenStorageLocation { payload, .. } = command {
        digest_text(&mut digest, storage_target_id(&payload.target));
    }
    format!("{:x}", digest.finalize())
}

fn storage_target_id(target: &StorageLocationTarget) -> &'static str {
    match target {
        StorageLocationTarget::DataRoot => "dataRoot",
        StorageLocationTarget::Ledger => "ledger",
        StorageLocationTarget::Vault => "vault",
        StorageLocationTarget::Cache => "cache",
        StorageLocationTarget::Staging => "staging",
        StorageLocationTarget::Credentials => "credentials",
    }
}

fn command_receipt(meta: &CommandMeta, revision: i64, completed_at: i64) -> CommandReceipt {
    CommandReceipt {
        command_id: meta.command_id.clone(),
        idempotency_key: meta.idempotency_key.clone(),
        command_type: meta.command_type.to_string(),
        aggregate_id: SETTINGS_AGGREGATE_ID.to_string(),
        revision,
        last_event_sequence: 0,
        completed_at,
    }
}

fn validate_expected_revision(expected: Option<i64>, actual: i64) -> Result<(), HostError> {
    if let Some(expected) = expected {
        if expected != actual {
            return Err(HostError::new(
                "DESKTOP_SETTINGS_REVISION_CONFLICT",
                format!("desktop settings revision is {actual}, request expected {expected}"),
                false,
            ));
        }
    }
    Ok(())
}

fn validate_deadline(deadline_at: Option<i64>) -> Result<(), HostError> {
    if deadline_at.is_some_and(|deadline| deadline <= now_millis()) {
        return Err(HostError::new(
            "COMMAND_DEADLINE_EXCEEDED",
            "desktop settings command deadline was exceeded",
            true,
        ));
    }
    Ok(())
}

fn validate_text(label: &str, value: &str, max_bytes: usize) -> Result<(), HostError> {
    if value.trim().is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(HostError::validation(format!(
            "{label} must contain 1..{max_bytes} safe bytes"
        )));
    }
    Ok(())
}

fn normalize_runtime_version(value: String) -> Result<String, HostError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_whitespace) {
        return Err(HostError::validation("Codex runtime version is invalid"));
    }
    Ok(value.to_string())
}

fn compiled_build_channel() -> DesktopBuildChannel {
    match option_env!("BSAIGC_BUILD_CHANNEL") {
        Some("stable") => DesktopBuildChannel::Stable,
        Some("internal-preview") | Some("internalPreview") => DesktopBuildChannel::InternalPreview,
        Some("development") => DesktopBuildChannel::Development,
        _ if cfg!(debug_assertions) => DesktopBuildChannel::Development,
        _ => DesktopBuildChannel::Stable,
    }
}

fn digest_text(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value.as_bytes());
}

fn digest_optional(digest: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            digest.update([1]);
            digest_text(digest, value);
        }
        None => digest.update([0]),
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn valid_bucket_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-.".contains(&byte))
}

fn u64_to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn unsafe_path_error(message: impl Into<String>) -> HostError {
    HostError::new("DESKTOP_SETTINGS_PATH_UNSAFE", message, false)
}

fn path_io_error(action: &str, error: std::io::Error) -> HostError {
    HostError::new(
        "DESKTOP_SETTINGS_STORAGE_IO",
        format!("{action} failed: {error}"),
        true,
    )
}

fn sql_error(error: rusqlite::Error) -> HostError {
    HostError::new(
        "DESKTOP_SETTINGS_SQLITE",
        format!("desktop settings SQLite operation failed: {error}"),
        true,
    )
}

fn json_error(error: serde_json::Error) -> HostError {
    HostError::internal(format!("desktop settings JSON operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    struct Fixture {
        _temp: TempDir,
        service: DesktopSettingsService,
        opened: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl Fixture {
        fn new(shells: ShellStatus) -> Self {
            let temp = tempfile::tempdir().unwrap();
            let opened = Arc::new(Mutex::new(Vec::new()));
            let opened_sink = Arc::clone(&opened);
            let opener: Arc<LocationOpener> = Arc::new(move |path| {
                opened_sink.lock().unwrap().push(path.to_path_buf());
                Ok(())
            });
            let service = DesktopSettingsService::open_with_dependencies(
                temp.path(),
                "0.144.5".to_string(),
                shells,
                opener,
            )
            .unwrap();
            Self {
                _temp: temp,
                service,
                opened,
            }
        }
    }

    fn empty_shells() -> ShellStatus {
        ShellStatus::from_lookup(|_| None)
    }

    fn context() -> OperationContext {
        OperationContext {
            actor_id: "operator-1".to_string(),
            account_id: Some("account-1".to_string()),
            project_id: None,
            window_id: "settings-window".to_string(),
            trace_id: Uuid::new_v4().to_string(),
        }
    }

    fn status_command() -> DesktopSettingsCommandEnvelope {
        DesktopSettingsCommandEnvelope::Status {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: DESKTOP_SETTINGS_PROTOCOL_VERSION.to_string(),
            context: context(),
            idempotency_key: format!("status-{}", Uuid::new_v4()),
            expected_revision: None,
            deadline_at: None,
        }
    }

    fn clear_command(
        command_id: &str,
        idempotency_key: &str,
        expected_revision: i64,
    ) -> DesktopSettingsCommandEnvelope {
        DesktopSettingsCommandEnvelope::ClearCache {
            command_id: command_id.to_string(),
            protocol_version: DESKTOP_SETTINGS_PROTOCOL_VERSION.to_string(),
            context: context(),
            idempotency_key: idempotency_key.to_string(),
            expected_revision: Some(expected_revision),
            deadline_at: None,
        }
    }

    fn update_command(
        command_id: &str,
        idempotency_key: &str,
        expected_revision: i64,
    ) -> DesktopSettingsCommandEnvelope {
        DesktopSettingsCommandEnvelope::CheckForUpdates {
            command_id: command_id.to_string(),
            protocol_version: DESKTOP_SETTINGS_PROTOCOL_VERSION.to_string(),
            context: context(),
            idempotency_key: idempotency_key.to_string(),
            expected_revision: Some(expected_revision),
            deadline_at: None,
        }
    }

    #[test]
    fn snapshot_uses_logical_paths_and_reports_authority() {
        let fixture = Fixture::new(empty_shells());
        fs::create_dir_all(fixture.service.data_root.join("vault/project-1")).unwrap();
        fs::write(
            fixture
                .service
                .data_root
                .join("vault/project-1/contract.pdf"),
            b"contract",
        )
        .unwrap();
        fs::create_dir_all(fixture.service.data_root.join("cache/previews")).unwrap();
        fs::write(
            fixture.service.data_root.join("cache/previews/page-1.webp"),
            b"preview",
        )
        .unwrap();

        let response = fixture.service.execute(status_command()).unwrap();
        assert_eq!(response.snapshot.update.codex_runtime_version, "0.144.5");
        assert_eq!(response.snapshot.storage.data_root, LOGICAL_DATA_ROOT);
        assert_eq!(response.snapshot.storage.cache_bytes, 7);
        assert!(response
            .snapshot
            .storage
            .locations
            .iter()
            .all(|location| !location
                .path
                .contains(fixture.service.data_root.to_str().unwrap())));
        assert!(
            response
                .snapshot
                .storage
                .locations
                .iter()
                .find(|location| location.target == StorageLocationTarget::Vault)
                .unwrap()
                .authoritative
        );
        let clearable: Vec<_> = response
            .snapshot
            .storage
            .locations
            .iter()
            .filter(|location| location.clearable)
            .map(|location| &location.target)
            .collect();
        assert_eq!(clearable, vec![&StorageLocationTarget::Cache]);
        let wire = serde_json::to_string(&response).unwrap();
        assert!(!wire.contains(fixture.service.data_root.to_str().unwrap()));
    }

    #[test]
    fn cache_clear_is_durable_counted_and_replayed_without_touching_new_cache() {
        let fixture = Fixture::new(empty_shells());
        let cache = fixture.service.data_root.join("cache");
        fs::create_dir_all(cache.join("nested")).unwrap();
        fs::write(cache.join("a.bin"), vec![1_u8; 11]).unwrap();
        fs::write(cache.join("nested/b.bin"), vec![2_u8; 17]).unwrap();
        fs::create_dir_all(fixture.service.data_root.join("staging")).unwrap();
        fs::write(fixture.service.data_root.join("staging/keep.bin"), b"keep").unwrap();
        fs::create_dir_all(fixture.service.data_root.join("vault")).unwrap();
        fs::write(fixture.service.data_root.join("vault/keep.bin"), b"vault").unwrap();

        let command_id = Uuid::new_v4().to_string();
        let first = fixture
            .service
            .execute(clear_command(&command_id, "clear-cache-1", 0))
            .unwrap();
        assert_eq!(first.cache_clear.as_ref().unwrap().freed_bytes, 28);
        assert_eq!(
            first.cache_clear.as_ref().unwrap().cleared_locations,
            vec!["cache"]
        );
        assert!(!first.replayed);
        assert!(fs::read_dir(&cache).unwrap().next().is_none());
        assert!(fixture.service.data_root.join("staging/keep.bin").exists());
        assert!(fixture.service.data_root.join("vault/keep.bin").exists());

        fs::write(cache.join("new-preview.bin"), vec![3_u8; 5]).unwrap();
        let replay = fixture
            .service
            .execute(clear_command(&command_id, "clear-cache-1", 0))
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.cache_clear.as_ref().unwrap().freed_bytes, 28);
        assert!(cache.join("new-preview.bin").exists());
        assert_eq!(replay.snapshot.revision, 1);
    }

    #[test]
    fn prepared_cache_receipt_resumes_after_interruption() {
        let fixture = Fixture::new(empty_shells());
        let cache = fixture.service.data_root.join("cache");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("pending.bin"), vec![9_u8; 23]).unwrap();
        let command_id = Uuid::new_v4().to_string();
        let command = clear_command(&command_id, "clear-cache-interrupted", 0);
        let meta = command_meta(&command);
        let fingerprint = request_fingerprint(&command, &meta);
        let mut connection = open_connection(&fixture.service.database_path).unwrap();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        transaction
            .execute(
                "UPDATE desktop_settings_state SET revision = 1 WHERE singleton = 1",
                [],
            )
            .unwrap();
        insert_prepared_receipt(
            &transaction,
            &meta,
            &fingerprint,
            1,
            &PreparedCacheClear {
                freed_bytes: 23,
                cleared_locations: vec!["cache".to_string()],
            },
        )
        .unwrap();
        transaction.commit().unwrap();

        let resumed = fixture.service.execute(command).unwrap();
        assert!(resumed.replayed);
        assert_eq!(resumed.cache_clear.unwrap().freed_bytes, 23);
        assert!(fs::read_dir(cache).unwrap().next().is_none());
        let stored = require_receipt_by_key(&connection, "clear-cache-interrupted").unwrap();
        assert_eq!(stored.state, ReceiptState::Completed);
    }

    #[test]
    fn idempotency_and_revision_conflicts_are_rejected() {
        let fixture = Fixture::new(empty_shells());
        fs::create_dir_all(fixture.service.data_root.join("cache")).unwrap();
        let command_id = Uuid::new_v4().to_string();
        fixture
            .service
            .execute(clear_command(&command_id, "same-key", 0))
            .unwrap();

        let key_conflict = fixture
            .service
            .execute(clear_command(&Uuid::new_v4().to_string(), "same-key", 1))
            .unwrap_err();
        assert_eq!(key_conflict.code, "DESKTOP_SETTINGS_IDEMPOTENCY_CONFLICT");

        let stale = fixture
            .service
            .execute(update_command(
                &Uuid::new_v4().to_string(),
                "stale-update",
                0,
            ))
            .unwrap_err();
        assert_eq!(stale.code, "DESKTOP_SETTINGS_REVISION_CONFLICT");
    }

    #[test]
    fn open_location_uses_whitelisted_local_path_and_replays_once() {
        let fixture = Fixture::new(empty_shells());
        let command_id = Uuid::new_v4().to_string();
        let command = || DesktopSettingsCommandEnvelope::OpenStorageLocation {
            command_id: command_id.clone(),
            protocol_version: DESKTOP_SETTINGS_PROTOCOL_VERSION.to_string(),
            context: context(),
            payload: crate::protocol::OpenStorageLocationPayload {
                target: StorageLocationTarget::Vault,
            },
            idempotency_key: "open-vault-1".to_string(),
            expected_revision: Some(0),
            deadline_at: None,
        };
        fixture.service.execute(command()).unwrap();
        fixture.service.execute(command()).unwrap();
        let opened = fixture.opened.lock().unwrap();
        assert_eq!(opened.len(), 1);
        assert_eq!(opened[0], fixture.service.data_root.join("vault"));
    }

    #[test]
    fn provider_shells_never_claim_transport_readiness() {
        let fixture = Fixture::new(ShellStatus::from_lookup(|name| {
            match name {
                R2_ENDPOINT_ENV => Some("https://example.r2.cloudflarestorage.com"),
                R2_BUCKET_ENV => Some("business-backup"),
                R2_ACCESS_KEY_ENV => Some("access"),
                R2_SECRET_KEY_ENV => Some("secret"),
                FEISHU_BIN_ENV => Some("feishu-cli.exe"),
                UPDATE_SOURCE_ENV => Some("https://updates.example.com/manifest.json"),
                _ => None,
            }
            .map(str::to_string)
        }));
        let status = fixture.service.execute(status_command()).unwrap().snapshot;
        assert!(status.cloud_backup.configured);
        assert!(!status.cloud_backup.ready);
        assert_eq!(status.cloud_backup.state, "adapterPending");
        assert_eq!(
            status.channel_adapters[0].state,
            ChannelAdapterState::Degraded
        );
        assert!(!status.channel_adapters[0].configured);
        assert!(!status.update.automatic_install_allowed);
        assert_eq!(status.update.state, "adapterPending");
        assert!(status.update.message.contains("no network"));
    }

    #[test]
    fn update_shell_records_attempt_but_makes_no_release_claim() {
        let fixture = Fixture::new(empty_shells());
        let command_id = Uuid::new_v4().to_string();
        let first = fixture
            .service
            .execute(update_command(&command_id, "update-check-1", 0))
            .unwrap();
        assert_eq!(first.snapshot.revision, 1);
        assert_eq!(first.snapshot.update.state, "notConfigured");
        assert!(!first.snapshot.update.update_source_configured);
        assert!(!first.snapshot.update.automatic_install_allowed);
        assert!(first.snapshot.update.last_checked_at.is_some());
        assert!(first.snapshot.update.message.contains("reserved"));

        let replay = fixture
            .service
            .execute(update_command(&command_id, "update-check-1", 0))
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(
            replay.snapshot.update.last_checked_at,
            first.snapshot.update.last_checked_at
        );
    }

    #[test]
    fn expired_uncommitted_command_is_rejected() {
        let fixture = Fixture::new(empty_shells());
        let command = DesktopSettingsCommandEnvelope::ClearCache {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: DESKTOP_SETTINGS_PROTOCOL_VERSION.to_string(),
            context: context(),
            idempotency_key: "expired-clear".to_string(),
            expected_revision: Some(0),
            deadline_at: Some(now_millis() - 1),
        };
        let error = fixture.service.execute(command).unwrap_err();
        assert_eq!(error.code, "COMMAND_DEADLINE_EXCEEDED");
    }

    #[test]
    fn opener_failure_is_not_persisted_as_success() {
        let temp = tempfile::tempdir().unwrap();
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempt_sink = Arc::clone(&attempts);
        let opener: Arc<LocationOpener> = Arc::new(move |_| {
            attempt_sink.fetch_add(1, Ordering::SeqCst);
            Err(HostError::new("OPEN_FAILED", "test opener failed", true))
        });
        let service = DesktopSettingsService::open_with_dependencies(
            temp.path(),
            "0.144.5".to_string(),
            empty_shells(),
            opener,
        )
        .unwrap();
        let command_id = Uuid::new_v4().to_string();
        for _ in 0..2 {
            let error = service
                .execute(DesktopSettingsCommandEnvelope::OpenStorageLocation {
                    command_id: command_id.clone(),
                    protocol_version: DESKTOP_SETTINGS_PROTOCOL_VERSION.to_string(),
                    context: context(),
                    payload: crate::protocol::OpenStorageLocationPayload {
                        target: StorageLocationTarget::DataRoot,
                    },
                    idempotency_key: "open-failure".to_string(),
                    expected_revision: Some(0),
                    deadline_at: None,
                })
                .unwrap_err();
            assert_eq!(error.code, "OPEN_FAILED");
        }
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[cfg(unix)]
    #[test]
    fn cache_symlink_is_rejected_without_touching_external_content() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new(empty_shells());
        let external = tempfile::tempdir().unwrap();
        fs::write(external.path().join("keep.txt"), b"keep").unwrap();
        let cache = fixture.service.data_root.join("cache");
        fs::create_dir_all(&cache).unwrap();
        symlink(external.path(), cache.join("escape")).unwrap();
        let error = fixture
            .service
            .execute(clear_command(
                &Uuid::new_v4().to_string(),
                "unsafe-cache",
                0,
            ))
            .unwrap_err();
        assert_eq!(error.code, "DESKTOP_SETTINGS_PATH_UNSAFE");
        assert!(external.path().join("keep.txt").exists());
    }
}
