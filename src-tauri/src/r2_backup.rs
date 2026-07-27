use crate::backup_outbox::{BackupCommandOutcome, BackupOutbox};
use crate::protocol::{
    AssetBackupRecord, BackupCommandEnvelope, BackupDomainEvent, BackupState, HostError,
};
use futures_util::StreamExt;
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::multipart::MultipartStore;
use object_store::path::Path as ObjectPath;
use object_store::{
    Attribute, Attributes, ClientOptions, GetOptions, ObjectStore, ObjectStoreExt,
    PutMultipartOptions, PutOptions, PutResult,
};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const SINGLE_PUT_LIMIT: u64 = 8 * 1024 * 1024;
const MIN_MULTIPART_PART_SIZE: u64 = 8 * 1024 * 1024;
const MAX_MULTIPART_PARTS: u64 = 9_999;
const ONE_MIB: u64 = 1024 * 1024;
const HASH_BUFFER_SIZE: usize = 1024 * 1024;
const MAX_PERSISTED_ERROR_CHARS: usize = 3_500;

const ENV_ENDPOINT: &str = "BSAIGC_R2_ENDPOINT";
const ENV_ACCOUNT_ID: &str = "BSAIGC_R2_ACCOUNT_ID";
const ENV_BUCKET: &str = "BSAIGC_R2_BUCKET";
const ENV_ACCESS_KEY_ID: &str = "BSAIGC_R2_ACCESS_KEY_ID";
const ENV_SECRET_ACCESS_KEY: &str = "BSAIGC_R2_SECRET_ACCESS_KEY";
const ENV_REGION: &str = "BSAIGC_R2_REGION";
const ENV_PREFIX: &str = "BSAIGC_R2_PREFIX";
const ENV_POLL_MILLIS: &str = "BSAIGC_R2_POLL_INTERVAL_MS";
const ENV_CONNECT_TIMEOUT_SECS: &str = "BSAIGC_R2_CONNECT_TIMEOUT_SECONDS";
const ENV_REQUEST_TIMEOUT_SECS: &str = "BSAIGC_R2_REQUEST_TIMEOUT_SECONDS";

type BackupEventSink = Arc<dyn Fn(&[BackupDomainEvent]) + Send + Sync + 'static>;

/// Background sidecar for the durable upload outbox.
///
/// A disabled/degraded worker intentionally leaves queued records untouched. Local Vault and
/// SQLite have already committed the business result before anything reaches this type.
pub struct R2BackupWorker {
    control: Arc<WorkerControl>,
    thread: Mutex<Option<JoinHandle<()>>>,
    restore: Option<RestoreCoordinator>,
    ready: bool,
    degraded_reason: Option<String>,
}

impl R2BackupWorker {
    pub fn start_from_env(
        outbox: Arc<BackupOutbox>,
        database_path: PathBuf,
        vault_root: PathBuf,
        event_sink: BackupEventSink,
    ) -> Self {
        let config = match R2Config::load() {
            R2ConfigLoad::Configured(config) => config,
            R2ConfigLoad::Degraded(reason) => return Self::degraded(reason),
        };
        let poll_interval = config.poll_interval;
        let transport = match CloudflareR2Transport::new(config) {
            Ok(transport) => Arc::new(transport) as Arc<dyn BackupTransport>,
            Err(error) => {
                return Self::degraded(format!(
                    "R2 transport configuration is invalid: {}",
                    bounded_error(&error)
                ));
            }
        };
        match Self::start_with_transport(
            outbox,
            database_path,
            vault_root,
            transport,
            event_sink,
            poll_interval,
        ) {
            Ok(worker) => worker,
            Err(error) => Self::degraded(format!(
                "R2 backup worker could not start: {}",
                bounded_error(&error)
            )),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn degraded_reason(&self) -> Option<&str> {
        self.degraded_reason.as_deref()
    }

    /// Wakes the sidecar after a durable queue/retry command. A disabled worker safely ignores it.
    pub fn wake(&self) {
        self.control.wake();
    }

    /// Interrupts an active transport after the outbox cancellation has committed.
    pub fn cancel(&self, asset_id: &str) {
        self.control.cancel_asset(asset_id);
    }

    pub fn restore(
        &self,
        command: BackupCommandEnvelope,
    ) -> Result<BackupCommandOutcome, HostError> {
        let Some(restore) = &self.restore else {
            return Err(HostError::new(
                "BACKUP_RESTORE_DEGRADED",
                self.degraded_reason
                    .clone()
                    .unwrap_or_else(|| "R2 restore is unavailable".to_string()),
                true,
            ));
        };
        restore.execute(command)
    }

    pub fn shutdown(&self) {
        self.control.shutdown();
        let handle = self.thread.lock().ok().and_then(|mut slot| slot.take());
        if let Some(handle) = handle {
            if handle.thread().id() != std::thread::current().id() {
                let _ = handle.join();
            }
        }
    }

    fn degraded(reason: String) -> Self {
        Self {
            control: Arc::new(WorkerControl::default()),
            thread: Mutex::new(None),
            restore: None,
            ready: false,
            degraded_reason: Some(reason),
        }
    }

    fn start_with_transport(
        outbox: Arc<BackupOutbox>,
        database_path: PathBuf,
        vault_root: PathBuf,
        transport: Arc<dyn BackupTransport>,
        event_sink: BackupEventSink,
        poll_interval: Duration,
    ) -> Result<Self, String> {
        let control = Arc::new(WorkerControl::default());
        let restore = RestoreCoordinator {
            outbox: Arc::clone(&outbox),
            database_path: database_path.clone(),
            vault_root: vault_root.clone(),
            transport: Arc::clone(&transport),
        };
        let core = BackupWorkerCore {
            outbox,
            database_path,
            vault_root,
            transport,
            event_sink,
            control: Arc::clone(&control),
        };
        let loop_control = Arc::clone(&control);
        let thread = std::thread::Builder::new()
            .name("bsaigc-r2-backup".to_string())
            .spawn(move || run_worker_loop(core, loop_control, poll_interval))
            .map_err(|error| error.to_string())?;
        Ok(Self {
            control,
            thread: Mutex::new(Some(thread)),
            restore: Some(restore),
            ready: true,
            degraded_reason: None,
        })
    }
}

impl Drop for R2BackupWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Default)]
struct WorkerControl {
    signal: Mutex<WorkerSignal>,
    wake_condition: Condvar,
    active_uploads: Mutex<HashMap<String, CancellationToken>>,
}

#[derive(Default)]
struct WorkerSignal {
    shutdown: bool,
    wake_requested: bool,
}

impl WorkerControl {
    fn is_shutdown(&self) -> bool {
        self.signal
            .lock()
            .map(|signal| signal.shutdown)
            .unwrap_or(true)
    }

    fn wake(&self) {
        if let Ok(mut signal) = self.signal.lock() {
            signal.wake_requested = true;
            self.wake_condition.notify_all();
        }
    }

    fn wait(&self, timeout: Duration) {
        let Ok(mut signal) = self.signal.lock() else {
            return;
        };
        if signal.shutdown {
            return;
        }
        if !signal.wake_requested {
            if let Ok((next, _)) = self.wake_condition.wait_timeout(signal, timeout) {
                signal = next;
            } else {
                return;
            }
        }
        signal.wake_requested = false;
    }

    fn shutdown(&self) {
        if let Ok(mut signal) = self.signal.lock() {
            signal.shutdown = true;
            signal.wake_requested = true;
        }
        if let Ok(active) = self.active_uploads.lock() {
            for cancellation in active.values() {
                cancellation.cancel();
            }
        }
        self.wake_condition.notify_all();
    }

    fn register_upload(&self, asset_id: &str) -> CancellationToken {
        let cancellation = CancellationToken::new();
        if self.is_shutdown() {
            cancellation.cancel();
        }
        if let Ok(mut active) = self.active_uploads.lock() {
            active.insert(asset_id.to_string(), cancellation.clone());
        }
        cancellation
    }

    fn finish_upload(&self, asset_id: &str) {
        if let Ok(mut active) = self.active_uploads.lock() {
            active.remove(asset_id);
        }
    }

    fn cancel_asset(&self, asset_id: &str) {
        if let Ok(active) = self.active_uploads.lock() {
            if let Some(cancellation) = active.get(asset_id) {
                cancellation.cancel();
            }
        }
        self.wake();
    }
}

struct BackupWorkerCore {
    outbox: Arc<BackupOutbox>,
    database_path: PathBuf,
    vault_root: PathBuf,
    transport: Arc<dyn BackupTransport>,
    event_sink: BackupEventSink,
    control: Arc<WorkerControl>,
}

impl BackupWorkerCore {
    /// Returns true when an outbox record was claimed, including a record that failed and was
    /// durably rescheduled. Outbox/SQLite infrastructure failures are returned to the loop.
    fn process_once(&self) -> Result<bool, HostError> {
        if self.control.is_shutdown() {
            return Ok(false);
        }
        let trace_id = format!("r2-backup-worker:{}", Uuid::new_v4());
        let claim = self.outbox.claim_next(&trace_id)?;
        (self.event_sink)(&claim.emitted_events);
        let Some(backup) = claim.backup else {
            return Ok(false);
        };

        let cancellation = self.control.register_upload(&backup.asset_id);
        let result = self.process_claimed(&backup, &cancellation);
        self.control.finish_upload(&backup.asset_id);

        match result {
            Ok(uploaded) => {
                if !self.still_owns_claim(&backup)? {
                    return Ok(true);
                }
                let outcome = self.outbox.mark_backed_up(
                    &backup.asset_id,
                    &backup.content_sha256,
                    backup.revision,
                    &uploaded.remote_object_key,
                    uploaded.remote_etag.as_deref(),
                    &trace_id,
                )?;
                (self.event_sink)(&outcome.emitted_events);
            }
            Err(BackupWorkError::Cancelled) => {
                // User cancellation is already durable. Shutdown cancellation intentionally leaves
                // Uploading behind so BackupOutbox startup recovery can requeue it after restart.
            }
            Err(BackupWorkError::Failed(error)) => {
                if !self.still_owns_claim(&backup)? {
                    return Ok(true);
                }
                let outcome = self.outbox.mark_failed(
                    &backup.asset_id,
                    &backup.content_sha256,
                    backup.revision,
                    &bounded_error(&error),
                    &trace_id,
                )?;
                (self.event_sink)(&outcome.emitted_events);
            }
        }
        Ok(true)
    }

    fn still_owns_claim(&self, claimed: &AssetBackupRecord) -> Result<bool, HostError> {
        Ok(self
            .outbox
            .get_exact(&claimed.asset_id, &claimed.content_sha256)?
            .is_some_and(|current| {
                current.state == BackupState::Uploading && current.revision == claimed.revision
            }))
    }

    fn process_claimed(
        &self,
        backup: &AssetBackupRecord,
        cancellation: &CancellationToken,
    ) -> Result<BackupUploadResult, BackupWorkError> {
        if cancellation.is_cancelled() {
            return Err(BackupWorkError::Cancelled);
        }
        let asset =
            resolve_vault_asset(&self.database_path, &self.vault_root, backup, cancellation)?;
        let actual_sha256 = sha256_file(&asset.local_path, cancellation)?;
        if !actual_sha256.eq_ignore_ascii_case(&backup.content_sha256) {
            return Err(BackupWorkError::Failed(format!(
                "Local Vault SHA-256 mismatch for asset {}",
                backup.asset_id
            )));
        }
        self.transport.upload(
            BackupUploadRequest {
                asset_id: asset.asset_id,
                local_path: asset.local_path,
                mime_type: asset.mime_type,
                size_bytes: asset.size_bytes,
                content_sha256: actual_sha256,
            },
            cancellation.clone(),
        )
    }
}

struct RestoreCoordinator {
    outbox: Arc<BackupOutbox>,
    database_path: PathBuf,
    vault_root: PathBuf,
    transport: Arc<dyn BackupTransport>,
}

impl RestoreCoordinator {
    fn execute(&self, command: BackupCommandEnvelope) -> Result<BackupCommandOutcome, HostError> {
        let prepared = self.outbox.prepare_restore(&command)?;
        if let Some(response) = prepared.replayed_response {
            return Ok(BackupCommandOutcome {
                response,
                emitted_events: Vec::new(),
            });
        }

        let backup = prepared.backup;
        let remote_object_key = backup
            .remote_object_key
            .as_deref()
            .expect("restore preparation requires a remote object key");
        let expected_object_key = self
            .transport
            .expected_object_key(&backup.asset_id, &backup.content_sha256);
        if remote_object_key != expected_object_key {
            return Err(HostError::new(
                "BACKUP_RESTORE_OBJECT_KEY_MISMATCH",
                format!(
                    "remote object key does not match the configured backup namespace for asset {}",
                    backup.asset_id
                ),
                false,
            ));
        }

        let vault_asset = resolve_restore_vault_asset(
            &self.database_path,
            &self.vault_root,
            &backup.asset_id,
            &backup.content_sha256,
        )?;
        match inspect_restore_destination(&vault_asset)? {
            RestoreDestinationState::Correct => return self.outbox.complete_restore(command),
            RestoreDestinationState::Missing => {}
        }

        let mut staging = RestoreStagingFile::create(&vault_asset.vault_root)?;
        let staging_file = staging.take_file()?;
        let remote = self
            .transport
            .download(
                BackupRestoreRequest {
                    asset_id: backup.asset_id.clone(),
                    remote_object_key: remote_object_key.to_string(),
                    expected_sha256: backup.content_sha256.clone(),
                    expected_size_bytes: vault_asset.size_bytes,
                    expected_etag: backup.remote_etag.clone(),
                    staging_file,
                },
                CancellationToken::new(),
            )
            .map_err(restore_work_error)?;
        validate_remote_restore_result(&backup, &vault_asset, &remote)?;

        let staged_metadata = fs::symlink_metadata(staging.path()).map_err(|error| {
            HostError::new(
                "BACKUP_RESTORE_STAGING_FAILED",
                format!("inspect restored staging file failed: {error}"),
                true,
            )
        })?;
        if is_link_or_reparse(&staged_metadata)
            || !staged_metadata.is_file()
            || staged_metadata.len() != vault_asset.size_bytes
        {
            return Err(HostError::new(
                "BACKUP_RESTORE_INTEGRITY_FAILED",
                "restored staging file size or type does not match Local Vault metadata",
                false,
            ));
        }
        let staged_sha256 = sha256_path(staging.path()).map_err(|error| {
            HostError::new(
                "BACKUP_RESTORE_STAGING_FAILED",
                format!("hash restored staging file failed: {error}"),
                true,
            )
        })?;
        if !staged_sha256.eq_ignore_ascii_case(&backup.content_sha256) {
            return Err(HostError::new(
                "BACKUP_RESTORE_INTEGRITY_FAILED",
                format!(
                    "downloaded R2 content SHA-256 does not match asset {}",
                    backup.asset_id
                ),
                false,
            ));
        }

        commit_restore_without_overwrite(&mut staging, &vault_asset)?;
        self.outbox.complete_restore(command)
    }
}

struct RestoreVaultAsset {
    asset_id: String,
    vault_root: PathBuf,
    final_path: PathBuf,
    content_sha256: String,
    size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestoreDestinationState {
    Missing,
    Correct,
}

fn resolve_restore_vault_asset(
    database_path: &Path,
    vault_root: &Path,
    asset_id: &str,
    expected_sha256: &str,
) -> Result<RestoreVaultAsset, HostError> {
    let connection = Connection::open(database_path).map_err(|error| {
        HostError::internal(format!(
            "open Local Vault metadata database for restore failed: {error}"
        ))
    })?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| {
            HostError::internal(format!(
                "configure Local Vault database for restore failed: {error}"
            ))
        })?;
    let stored = connection
        .query_row(
            "SELECT id, size_bytes, sha256, storage_rel_path, status
             FROM assets WHERE id = ?1",
            params![asset_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|error| {
            HostError::internal(format!("read Local Vault restore metadata failed: {error}"))
        })?
        .ok_or_else(|| {
            HostError::new(
                "ASSET_NOT_FOUND",
                format!("Local Vault asset {asset_id} does not exist"),
                false,
            )
        })?;
    let (stored_asset_id, size_bytes, stored_sha256, storage_relative_path, status) = stored;
    if status != "ready" || size_bytes < 0 {
        return Err(HostError::new(
            "BACKUP_RESTORE_ASSET_INVALID",
            format!("Local Vault asset {asset_id} is not a valid ready asset"),
            false,
        ));
    }
    if !stored_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(HostError::new(
            "BACKUP_RESTORE_HASH_MISMATCH",
            format!("Local Vault metadata hash changed for asset {asset_id}"),
            false,
        ));
    }

    let canonical_root = validate_vault_root(vault_root)?;
    let final_path = prepare_restore_path(&canonical_root, &storage_relative_path)?;
    Ok(RestoreVaultAsset {
        asset_id: stored_asset_id,
        vault_root: canonical_root,
        final_path,
        content_sha256: stored_sha256,
        size_bytes: size_bytes as u64,
    })
}

fn validate_vault_root(vault_root: &Path) -> Result<PathBuf, HostError> {
    let metadata = fs::symlink_metadata(vault_root).map_err(|error| {
        HostError::new(
            "BACKUP_RESTORE_VAULT_PATH_INVALID",
            format!("Local Vault root is unavailable: {error}"),
            false,
        )
    })?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(HostError::new(
            "BACKUP_RESTORE_VAULT_PATH_INVALID",
            "Local Vault root must be a regular directory",
            false,
        ));
    }
    fs::canonicalize(vault_root).map_err(|error| {
        HostError::new(
            "BACKUP_RESTORE_VAULT_PATH_INVALID",
            format!("resolve Local Vault root failed: {error}"),
            false,
        )
    })
}

fn prepare_restore_path(
    canonical_root: &Path,
    storage_relative_path: &str,
) -> Result<PathBuf, HostError> {
    let relative = Path::new(storage_relative_path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(HostError::new(
            "BACKUP_RESTORE_VAULT_PATH_INVALID",
            "Local Vault asset has an invalid storage locator",
            false,
        ));
    }
    let parent_relative = relative.parent().ok_or_else(|| {
        HostError::new(
            "BACKUP_RESTORE_VAULT_PATH_INVALID",
            "Local Vault asset storage locator has no parent",
            false,
        )
    })?;
    let mut current = canonical_root.to_path_buf();
    for component in parent_relative.components() {
        let Component::Normal(segment) = component else {
            unreachable!("relative path components were validated")
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if is_link_or_reparse(&metadata) || !metadata.is_dir() {
                    return Err(HostError::new(
                        "BACKUP_RESTORE_VAULT_PATH_INVALID",
                        "Local Vault restore path contains a link, reparse point, or file",
                        false,
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        return Err(HostError::new(
                            "BACKUP_RESTORE_VAULT_PATH_INVALID",
                            format!("create Local Vault restore directory failed: {error}"),
                            false,
                        ));
                    }
                }
                let metadata = fs::symlink_metadata(&current).map_err(|error| {
                    HostError::new(
                        "BACKUP_RESTORE_VAULT_PATH_INVALID",
                        format!("inspect Local Vault restore directory failed: {error}"),
                        false,
                    )
                })?;
                if is_link_or_reparse(&metadata) || !metadata.is_dir() {
                    return Err(HostError::new(
                        "BACKUP_RESTORE_VAULT_PATH_INVALID",
                        "created Local Vault restore directory is unsafe",
                        false,
                    ));
                }
            }
            Err(error) => {
                return Err(HostError::new(
                    "BACKUP_RESTORE_VAULT_PATH_INVALID",
                    format!("inspect Local Vault restore path failed: {error}"),
                    false,
                ));
            }
        }
        let resolved = fs::canonicalize(&current).map_err(|error| {
            HostError::new(
                "BACKUP_RESTORE_VAULT_PATH_INVALID",
                format!("resolve Local Vault restore directory failed: {error}"),
                false,
            )
        })?;
        if !resolved.starts_with(canonical_root) {
            return Err(HostError::new(
                "BACKUP_RESTORE_VAULT_PATH_INVALID",
                "Local Vault restore path escaped the Vault root",
                false,
            ));
        }
    }
    Ok(canonical_root.join(relative))
}

fn inspect_restore_destination(
    asset: &RestoreVaultAsset,
) -> Result<RestoreDestinationState, HostError> {
    let parent = asset.final_path.parent().ok_or_else(|| {
        HostError::new(
            "BACKUP_RESTORE_VAULT_PATH_INVALID",
            "Local Vault restore destination has no parent",
            false,
        )
    })?;
    validate_existing_restore_parent(&asset.vault_root, parent)?;
    match fs::symlink_metadata(&asset.final_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(RestoreDestinationState::Missing)
        }
        Err(error) => Err(HostError::new(
            "BACKUP_RESTORE_DESTINATION_INVALID",
            format!("inspect Local Vault restore destination failed: {error}"),
            false,
        )),
        Ok(metadata) => {
            if is_link_or_reparse(&metadata) || !metadata.is_file() {
                return Err(restore_destination_conflict(&asset.asset_id));
            }
            let resolved = fs::canonicalize(&asset.final_path).map_err(|error| {
                HostError::new(
                    "BACKUP_RESTORE_DESTINATION_INVALID",
                    format!("resolve Local Vault restore destination failed: {error}"),
                    false,
                )
            })?;
            if !resolved.starts_with(&asset.vault_root) {
                return Err(HostError::new(
                    "BACKUP_RESTORE_VAULT_PATH_INVALID",
                    "Local Vault restore destination escaped the Vault root",
                    false,
                ));
            }
            if metadata.len() != asset.size_bytes {
                return Err(restore_destination_conflict(&asset.asset_id));
            }
            let sha256 = sha256_path(&resolved).map_err(|error| {
                HostError::new(
                    "BACKUP_RESTORE_DESTINATION_INVALID",
                    format!("hash Local Vault restore destination failed: {error}"),
                    true,
                )
            })?;
            if sha256.eq_ignore_ascii_case(&asset.content_sha256) {
                Ok(RestoreDestinationState::Correct)
            } else {
                Err(restore_destination_conflict(&asset.asset_id))
            }
        }
    }
}

fn restore_destination_conflict(asset_id: &str) -> HostError {
    HostError::new(
        "BACKUP_RESTORE_DESTINATION_CONFLICT",
        format!(
            "Local Vault destination for asset {asset_id} already contains different data; it was not overwritten"
        ),
        false,
    )
}

struct RestoreStagingFile {
    path: PathBuf,
    file: Option<File>,
    armed: bool,
}

impl RestoreStagingFile {
    fn create(vault_root: &Path) -> Result<Self, HostError> {
        let staging_root = prepare_restore_staging_root(vault_root)?;
        for _ in 0..8 {
            let path = staging_root.join(format!("{}.part", Uuid::new_v4()));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    let metadata = file.metadata().map_err(|error| {
                        HostError::new(
                            "BACKUP_RESTORE_STAGING_FAILED",
                            format!("inspect opened restore staging file failed: {error}"),
                            true,
                        )
                    })?;
                    if is_link_or_reparse(&metadata) || !metadata.is_file() {
                        return Err(HostError::new(
                            "BACKUP_RESTORE_STAGING_FAILED",
                            "restore staging file is not a regular file",
                            false,
                        ));
                    }
                    return Ok(Self {
                        path,
                        file: Some(file),
                        armed: true,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(HostError::new(
                        "BACKUP_RESTORE_STAGING_FAILED",
                        format!("create restore staging file failed: {error}"),
                        true,
                    ));
                }
            }
        }
        Err(HostError::new(
            "BACKUP_RESTORE_STAGING_FAILED",
            "could not allocate a unique restore staging file",
            true,
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn take_file(&mut self) -> Result<File, HostError> {
        self.file
            .take()
            .ok_or_else(|| HostError::internal("restore staging file handle was already consumed"))
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RestoreStagingFile {
    fn drop(&mut self) {
        drop(self.file.take());
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn prepare_restore_staging_root(vault_root: &Path) -> Result<PathBuf, HostError> {
    let staging_root = vault_root.join(".restore-staging");
    match fs::symlink_metadata(&staging_root) {
        Ok(metadata) => {
            if is_link_or_reparse(&metadata) || !metadata.is_dir() {
                return Err(HostError::new(
                    "BACKUP_RESTORE_STAGING_FAILED",
                    "restore staging root must be a regular directory",
                    false,
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match fs::create_dir(&staging_root) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(HostError::new(
                        "BACKUP_RESTORE_STAGING_FAILED",
                        format!("create restore staging root failed: {error}"),
                        true,
                    ));
                }
            }
        }
        Err(error) => {
            return Err(HostError::new(
                "BACKUP_RESTORE_STAGING_FAILED",
                format!("inspect restore staging root failed: {error}"),
                true,
            ));
        }
    }
    let metadata = fs::symlink_metadata(&staging_root).map_err(|error| {
        HostError::new(
            "BACKUP_RESTORE_STAGING_FAILED",
            format!("inspect restore staging root failed: {error}"),
            true,
        )
    })?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(HostError::new(
            "BACKUP_RESTORE_STAGING_FAILED",
            "restore staging root is unsafe",
            false,
        ));
    }
    let resolved = fs::canonicalize(&staging_root).map_err(|error| {
        HostError::new(
            "BACKUP_RESTORE_STAGING_FAILED",
            format!("resolve restore staging root failed: {error}"),
            true,
        )
    })?;
    if resolved.parent() != Some(vault_root) || !resolved.starts_with(vault_root) {
        return Err(HostError::new(
            "BACKUP_RESTORE_STAGING_FAILED",
            "restore staging root escaped the Vault",
            false,
        ));
    }
    Ok(resolved)
}

fn validate_remote_restore_result(
    backup: &AssetBackupRecord,
    asset: &RestoreVaultAsset,
    remote: &BackupRestoreResult,
) -> Result<(), HostError> {
    if remote.size_bytes != asset.size_bytes
        || remote.asset_id.as_deref() != Some(backup.asset_id.as_str())
        || !remote
            .sha256
            .as_deref()
            .is_some_and(|actual| actual.eq_ignore_ascii_case(&backup.content_sha256))
        || backup
            .remote_etag
            .as_ref()
            .is_some_and(|expected| remote.etag.as_ref() != Some(expected))
    {
        return Err(HostError::new(
            "BACKUP_RESTORE_REMOTE_METADATA_MISMATCH",
            format!(
                "R2 metadata, size, or ETag does not match the backup manifest for asset {}",
                backup.asset_id
            ),
            false,
        ));
    }
    Ok(())
}

fn commit_restore_without_overwrite(
    staging: &mut RestoreStagingFile,
    asset: &RestoreVaultAsset,
) -> Result<(), HostError> {
    match inspect_restore_destination(asset)? {
        RestoreDestinationState::Correct => return Ok(()),
        RestoreDestinationState::Missing => {}
    }
    let parent = asset.final_path.parent().ok_or_else(|| {
        HostError::new(
            "BACKUP_RESTORE_VAULT_PATH_INVALID",
            "Local Vault restore destination has no parent",
            false,
        )
    })?;
    validate_existing_restore_parent(&asset.vault_root, parent)?;
    match fs::hard_link(staging.path(), &asset.final_path) {
        Ok(()) => {
            if fs::remove_file(staging.path()).is_ok() {
                staging.disarm();
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            match inspect_restore_destination(asset)? {
                RestoreDestinationState::Correct => Ok(()),
                RestoreDestinationState::Missing => Err(HostError::new(
                    "BACKUP_RESTORE_COMMIT_FAILED",
                    "restore destination disappeared during atomic commit",
                    true,
                )),
            }
        }
        Err(error) => Err(HostError::new(
            "BACKUP_RESTORE_COMMIT_FAILED",
            format!("atomically commit restored Vault file failed: {error}"),
            true,
        )),
    }
}

fn validate_existing_restore_parent(vault_root: &Path, parent: &Path) -> Result<(), HostError> {
    let relative = parent.strip_prefix(vault_root).map_err(|_| {
        HostError::new(
            "BACKUP_RESTORE_VAULT_PATH_INVALID",
            "Local Vault restore parent escaped the Vault",
            false,
        )
    })?;
    let mut current = vault_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(HostError::new(
                "BACKUP_RESTORE_VAULT_PATH_INVALID",
                "Local Vault restore parent is not a safe path",
                false,
            ));
        };
        current.push(segment);
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            HostError::new(
                "BACKUP_RESTORE_VAULT_PATH_INVALID",
                format!("inspect Local Vault restore parent failed: {error}"),
                false,
            )
        })?;
        if is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(HostError::new(
                "BACKUP_RESTORE_VAULT_PATH_INVALID",
                "Local Vault restore parent contains a link or reparse point",
                false,
            ));
        }
        let resolved = fs::canonicalize(&current).map_err(|error| {
            HostError::new(
                "BACKUP_RESTORE_VAULT_PATH_INVALID",
                format!("resolve Local Vault restore parent failed: {error}"),
                false,
            )
        })?;
        if !resolved.starts_with(vault_root) {
            return Err(HostError::new(
                "BACKUP_RESTORE_VAULT_PATH_INVALID",
                "Local Vault restore parent escaped the Vault",
                false,
            ));
        }
    }
    Ok(())
}

fn sha256_path(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_SIZE];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn restore_work_error(error: BackupWorkError) -> HostError {
    match error {
        BackupWorkError::Cancelled => HostError::new(
            "BACKUP_RESTORE_CANCELLED",
            "R2 restore was cancelled before Local Vault commit",
            true,
        ),
        BackupWorkError::Failed(message) => {
            HostError::new("BACKUP_RESTORE_FAILED", bounded_error(&message), true)
        }
    }
}

fn run_worker_loop(core: BackupWorkerCore, control: Arc<WorkerControl>, poll_interval: Duration) {
    while !control.is_shutdown() {
        match core.process_once() {
            Ok(true) => continue,
            Ok(false) => control.wait(poll_interval),
            Err(error) => {
                eprintln!(
                    "R2 backup worker deferred after durable outbox error: code={} retryable={}",
                    error.code, error.retryable
                );
                control.wait(poll_interval);
            }
        }
    }
}

struct VaultAsset {
    asset_id: String,
    local_path: PathBuf,
    mime_type: String,
    size_bytes: u64,
}

fn resolve_vault_asset(
    database_path: &Path,
    vault_root: &Path,
    backup: &AssetBackupRecord,
    cancellation: &CancellationToken,
) -> Result<VaultAsset, BackupWorkError> {
    if cancellation.is_cancelled() {
        return Err(BackupWorkError::Cancelled);
    }
    let connection = Connection::open(database_path).map_err(|_| {
        BackupWorkError::Failed("open Local Vault metadata database failed".to_string())
    })?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|_| {
            BackupWorkError::Failed("configure Local Vault database failed".to_string())
        })?;
    let stored = connection
        .query_row(
            "SELECT id, mime_type, size_bytes, sha256, storage_rel_path, status
             FROM assets WHERE id = ?1",
            params![backup.asset_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(|_| BackupWorkError::Failed("read Local Vault asset metadata failed".to_string()))?
        .ok_or_else(|| {
            BackupWorkError::Failed(format!(
                "Local Vault asset {} does not exist",
                backup.asset_id
            ))
        })?;
    let (asset_id, mime_type, size_bytes, sha256, storage_relative_path, status) = stored;
    if status != "ready" {
        return Err(BackupWorkError::Failed(format!(
            "Local Vault asset {} is not ready",
            backup.asset_id
        )));
    }
    if size_bytes < 0 {
        return Err(BackupWorkError::Failed(format!(
            "Local Vault asset {} has invalid size metadata",
            backup.asset_id
        )));
    }
    if !sha256.eq_ignore_ascii_case(&backup.content_sha256) {
        return Err(BackupWorkError::Failed(format!(
            "Local Vault metadata hash does not match queued backup {}",
            backup.asset_id
        )));
    }

    let local_path = resolve_authoritative_vault_path(vault_root, &storage_relative_path)?;
    let metadata = std::fs::metadata(&local_path).map_err(|_| {
        BackupWorkError::Failed(format!(
            "Local Vault file for asset {} is unavailable",
            backup.asset_id
        ))
    })?;
    if !metadata.is_file() || metadata.len() != size_bytes as u64 {
        return Err(BackupWorkError::Failed(format!(
            "Local Vault file size does not match asset {} metadata",
            backup.asset_id
        )));
    }
    Ok(VaultAsset {
        asset_id,
        local_path,
        mime_type,
        size_bytes: size_bytes as u64,
    })
}

fn resolve_authoritative_vault_path(
    vault_root: &Path,
    storage_relative_path: &str,
) -> Result<PathBuf, BackupWorkError> {
    let relative = Path::new(storage_relative_path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(BackupWorkError::Failed(
            "Local Vault asset has an invalid storage locator".to_string(),
        ));
    }
    let canonical_root = std::fs::canonicalize(vault_root)
        .map_err(|_| BackupWorkError::Failed("Local Vault root is unavailable".to_string()))?;
    let mut candidate = canonical_root.clone();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            unreachable!("relative path components were validated")
        };
        candidate.push(segment);
        let metadata = std::fs::symlink_metadata(&candidate).map_err(|_| {
            BackupWorkError::Failed("Local Vault asset path is unavailable".to_string())
        })?;
        if is_link_or_reparse(&metadata) {
            return Err(BackupWorkError::Failed(
                "Local Vault asset path contains a link or reparse point".to_string(),
            ));
        }
    }
    let canonical_file = std::fs::canonicalize(&candidate).map_err(|_| {
        BackupWorkError::Failed("Local Vault asset file is unavailable".to_string())
    })?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err(BackupWorkError::Failed(
            "Local Vault asset path escaped the Vault root".to_string(),
        ));
    }
    Ok(canonical_file)
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn sha256_file(
    local_path: &Path,
    cancellation: &CancellationToken,
) -> Result<String, BackupWorkError> {
    let mut file = File::open(local_path)
        .map_err(|_| BackupWorkError::Failed("open Local Vault file failed".to_string()))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| BackupWorkError::Failed("seek Local Vault file failed".to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_SIZE];
    loop {
        if cancellation.is_cancelled() {
            return Err(BackupWorkError::Cancelled);
        }
        let read = file
            .read(&mut buffer)
            .map_err(|_| BackupWorkError::Failed("read Local Vault file failed".to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

trait BackupTransport: Send + Sync {
    fn expected_object_key(&self, asset_id: &str, content_sha256: &str) -> String;

    fn upload(
        &self,
        request: BackupUploadRequest,
        cancellation: CancellationToken,
    ) -> Result<BackupUploadResult, BackupWorkError>;

    fn download(
        &self,
        request: BackupRestoreRequest,
        cancellation: CancellationToken,
    ) -> Result<BackupRestoreResult, BackupWorkError>;
}

#[derive(Clone)]
struct BackupUploadRequest {
    asset_id: String,
    local_path: PathBuf,
    mime_type: String,
    size_bytes: u64,
    content_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BackupUploadResult {
    remote_object_key: String,
    remote_etag: Option<String>,
}

struct BackupRestoreRequest {
    asset_id: String,
    remote_object_key: String,
    expected_sha256: String,
    expected_size_bytes: u64,
    expected_etag: Option<String>,
    staging_file: File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BackupRestoreResult {
    size_bytes: u64,
    asset_id: Option<String>,
    sha256: Option<String>,
    etag: Option<String>,
}

enum BackupWorkError {
    Cancelled,
    Failed(String),
}

struct CloudflareR2Transport {
    runtime: Mutex<Runtime>,
    store: AmazonS3,
    object_prefix: String,
}

impl CloudflareR2Transport {
    fn new(config: R2Config) -> Result<Self, String> {
        let options = ClientOptions::new()
            .with_connect_timeout(config.connect_timeout)
            .with_timeout(config.request_timeout);
        let store = AmazonS3Builder::new()
            .with_bucket_name(config.bucket)
            .with_region(config.region)
            .with_endpoint(config.endpoint)
            .with_access_key_id(config.access_key_id)
            .with_secret_access_key(config.secret_access_key)
            .with_virtual_hosted_style_request(false)
            .with_client_options(options)
            .build()
            .map_err(|error| error.to_string())?;
        let runtime = RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            runtime: Mutex::new(runtime),
            store,
            object_prefix: config.object_prefix,
        })
    }

    async fn upload_async(
        store: &AmazonS3,
        object_prefix: &str,
        request: &BackupUploadRequest,
        cancellation: &CancellationToken,
    ) -> Result<BackupUploadResult, BackupWorkError> {
        if cancellation.is_cancelled() {
            return Err(BackupWorkError::Cancelled);
        }
        let remote_object_key =
            remote_object_key(object_prefix, &request.asset_id, &request.content_sha256);
        let location = ObjectPath::parse(&remote_object_key)
            .map_err(|_| BackupWorkError::Failed("construct R2 object key failed".to_string()))?;
        let attributes = upload_attributes(request);
        let put_result = if request.size_bytes <= SINGLE_PUT_LIMIT {
            upload_single(store, &location, request, attributes, cancellation).await?
        } else {
            upload_multipart(store, &location, request, attributes, cancellation).await?
        };
        let verified_etag = verify_remote_object(store, &location, request, cancellation).await?;
        Ok(BackupUploadResult {
            remote_object_key,
            remote_etag: verified_etag.or(put_result.e_tag),
        })
    }

    async fn download_async(
        store: &AmazonS3,
        object_prefix: &str,
        request: BackupRestoreRequest,
        cancellation: &CancellationToken,
    ) -> Result<BackupRestoreResult, BackupWorkError> {
        if cancellation.is_cancelled() {
            return Err(BackupWorkError::Cancelled);
        }
        let expected_key =
            remote_object_key(object_prefix, &request.asset_id, &request.expected_sha256);
        if request.remote_object_key != expected_key {
            return Err(BackupWorkError::Failed(
                "R2 restore object key is outside the configured backup namespace".to_string(),
            ));
        }
        let location = ObjectPath::parse(&request.remote_object_key).map_err(|_| {
            BackupWorkError::Failed("parse R2 restore object key failed".to_string())
        })?;
        let mut options = GetOptions::default();
        options.if_match.clone_from(&request.expected_etag);
        let result = tokio::select! {
            _ = cancellation.cancelled() => return Err(BackupWorkError::Cancelled),
            result = store.get_opts(&location, options) => {
                result.map_err(|error| BackupWorkError::Failed(format!("download R2 object failed: {error}")))?
            }
        };
        let size_bytes = result.meta.size;
        let etag = result.meta.e_tag.clone();
        let sha_key = Attribute::Metadata(Cow::Borrowed("bsaigc-sha256"));
        let asset_key = Attribute::Metadata(Cow::Borrowed("bsaigc-asset-id"));
        let sha256 = result
            .attributes
            .get(&sha_key)
            .map(|value| value.as_ref().to_string());
        let asset_id = result
            .attributes
            .get(&asset_key)
            .map(|value| value.as_ref().to_string());
        if size_bytes != request.expected_size_bytes
            || !sha256
                .as_deref()
                .is_some_and(|actual| actual.eq_ignore_ascii_case(&request.expected_sha256))
            || asset_id.as_deref() != Some(request.asset_id.as_str())
            || request
                .expected_etag
                .as_ref()
                .is_some_and(|expected| etag.as_ref() != Some(expected))
        {
            return Err(BackupWorkError::Failed(
                "R2 restore object metadata, size, or ETag verification failed".to_string(),
            ));
        }

        let mut staging_file = tokio::fs::File::from_std(request.staging_file);
        let mut stream = result.into_stream();
        let mut downloaded = 0_u64;
        while let Some(chunk) = tokio::select! {
            _ = cancellation.cancelled() => return Err(BackupWorkError::Cancelled),
            next = stream.next() => next,
        } {
            let chunk = chunk.map_err(|error| {
                BackupWorkError::Failed(format!("stream R2 restore object failed: {error}"))
            })?;
            downloaded = downloaded.checked_add(chunk.len() as u64).ok_or_else(|| {
                BackupWorkError::Failed("R2 restore byte count overflowed".to_string())
            })?;
            if downloaded > request.expected_size_bytes {
                return Err(BackupWorkError::Failed(
                    "R2 restore object exceeded the expected size".to_string(),
                ));
            }
            tokio::select! {
                _ = cancellation.cancelled() => return Err(BackupWorkError::Cancelled),
                result = staging_file.write_all(&chunk) => {
                    result.map_err(|error| BackupWorkError::Failed(format!("write restore staging file failed: {error}")))?;
                }
            }
        }
        if downloaded != request.expected_size_bytes {
            return Err(BackupWorkError::Failed(
                "R2 restore byte count does not match the backup manifest".to_string(),
            ));
        }
        staging_file.flush().await.map_err(|error| {
            BackupWorkError::Failed(format!("flush restore staging file failed: {error}"))
        })?;
        staging_file.sync_all().await.map_err(|error| {
            BackupWorkError::Failed(format!("sync restore staging file failed: {error}"))
        })?;
        drop(staging_file);
        Ok(BackupRestoreResult {
            size_bytes,
            asset_id,
            sha256,
            etag,
        })
    }
}

impl BackupTransport for CloudflareR2Transport {
    fn expected_object_key(&self, asset_id: &str, content_sha256: &str) -> String {
        remote_object_key(&self.object_prefix, asset_id, content_sha256)
    }

    fn upload(
        &self,
        request: BackupUploadRequest,
        cancellation: CancellationToken,
    ) -> Result<BackupUploadResult, BackupWorkError> {
        let runtime = self.runtime.lock().map_err(|_| {
            BackupWorkError::Failed("R2 transport runtime lock is poisoned".to_string())
        })?;
        runtime.block_on(Self::upload_async(
            &self.store,
            &self.object_prefix,
            &request,
            &cancellation,
        ))
    }

    fn download(
        &self,
        request: BackupRestoreRequest,
        cancellation: CancellationToken,
    ) -> Result<BackupRestoreResult, BackupWorkError> {
        let runtime = self.runtime.lock().map_err(|_| {
            BackupWorkError::Failed("R2 transport runtime lock is poisoned".to_string())
        })?;
        runtime.block_on(Self::download_async(
            &self.store,
            &self.object_prefix,
            request,
            &cancellation,
        ))
    }
}

async fn upload_single(
    store: &AmazonS3,
    location: &ObjectPath,
    request: &BackupUploadRequest,
    attributes: Attributes,
    cancellation: &CancellationToken,
) -> Result<PutResult, BackupWorkError> {
    let bytes = tokio::select! {
        _ = cancellation.cancelled() => return Err(BackupWorkError::Cancelled),
        result = tokio::fs::read(&request.local_path) => {
            result.map_err(|_| BackupWorkError::Failed("read Local Vault file for R2 upload failed".to_string()))?
        }
    };
    if bytes.len() as u64 != request.size_bytes {
        return Err(BackupWorkError::Failed(
            "Local Vault file changed before R2 upload".to_string(),
        ));
    }
    let options = PutOptions {
        attributes,
        ..Default::default()
    };
    tokio::select! {
        _ = cancellation.cancelled() => Err(BackupWorkError::Cancelled),
        result = store.put_opts(location, bytes.into(), options) => {
            result.map_err(|error| BackupWorkError::Failed(format!("R2 single upload failed: {error}")))
        }
    }
}

async fn upload_multipart(
    store: &AmazonS3,
    location: &ObjectPath,
    request: &BackupUploadRequest,
    attributes: Attributes,
    cancellation: &CancellationToken,
) -> Result<PutResult, BackupWorkError> {
    let options = PutMultipartOptions {
        attributes,
        ..Default::default()
    };
    let upload_id = tokio::select! {
        _ = cancellation.cancelled() => return Err(BackupWorkError::Cancelled),
        result = store.create_multipart_opts(location, options) => {
            result.map_err(|error| BackupWorkError::Failed(format!("start R2 multipart upload failed: {error}")))?
        }
    };
    let result = upload_multipart_parts(store, location, &upload_id, request, cancellation).await;
    match result {
        Ok(parts) => {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    let _ = store.abort_multipart(location, &upload_id).await;
                    Err(BackupWorkError::Cancelled)
                }
                completed = store.complete_multipart(location, &upload_id, parts) => {
                    completed.map_err(|error| BackupWorkError::Failed(format!("complete R2 multipart upload failed: {error}")))
                }
            }
        }
        Err(error) => {
            let _ = store.abort_multipart(location, &upload_id).await;
            Err(error)
        }
    }
}

async fn upload_multipart_parts(
    store: &AmazonS3,
    location: &ObjectPath,
    upload_id: &object_store::MultipartId,
    request: &BackupUploadRequest,
    cancellation: &CancellationToken,
) -> Result<Vec<object_store::multipart::PartId>, BackupWorkError> {
    let mut file = tokio::select! {
        _ = cancellation.cancelled() => return Err(BackupWorkError::Cancelled),
        result = tokio::fs::File::open(&request.local_path) => {
            result.map_err(|_| BackupWorkError::Failed("open Local Vault file for R2 upload failed".to_string()))?
        }
    };
    let part_size = multipart_part_size(request.size_bytes);
    let mut parts = Vec::new();
    let mut total_uploaded = 0_u64;
    loop {
        let mut chunk = vec![0_u8; part_size];
        let mut filled = 0_usize;
        while filled < part_size {
            let read = tokio::select! {
                _ = cancellation.cancelled() => return Err(BackupWorkError::Cancelled),
                result = file.read(&mut chunk[filled..]) => {
                    result.map_err(|_| BackupWorkError::Failed("read Local Vault file during R2 upload failed".to_string()))?
                }
            };
            if read == 0 {
                break;
            }
            filled += read;
        }
        if filled == 0 {
            break;
        }
        chunk.truncate(filled);
        let part_index = parts.len();
        let part = tokio::select! {
            _ = cancellation.cancelled() => return Err(BackupWorkError::Cancelled),
            result = store.put_part(location, upload_id, part_index, chunk.into()) => {
                result.map_err(|error| BackupWorkError::Failed(format!("upload R2 multipart part failed: {error}")))?
            }
        };
        parts.push(part);
        total_uploaded = total_uploaded.saturating_add(filled as u64);
        if filled < part_size {
            break;
        }
    }
    if total_uploaded != request.size_bytes {
        return Err(BackupWorkError::Failed(
            "Local Vault file changed during R2 upload".to_string(),
        ));
    }
    Ok(parts)
}

async fn verify_remote_object(
    store: &AmazonS3,
    location: &ObjectPath,
    request: &BackupUploadRequest,
    cancellation: &CancellationToken,
) -> Result<Option<String>, BackupWorkError> {
    let result = tokio::select! {
        _ = cancellation.cancelled() => return Err(BackupWorkError::Cancelled),
        result = store.get_opts(location, GetOptions::new().with_head(true)) => {
            result.map_err(|error| BackupWorkError::Failed(format!("verify R2 object failed: {error}")))?
        }
    };
    if result.meta.size != request.size_bytes {
        return Err(BackupWorkError::Failed(
            "R2 object size verification failed".to_string(),
        ));
    }
    let sha_key = Attribute::Metadata(Cow::Borrowed("bsaigc-sha256"));
    let asset_key = Attribute::Metadata(Cow::Borrowed("bsaigc-asset-id"));
    if result.attributes.get(&sha_key).map(|value| value.as_ref())
        != Some(request.content_sha256.as_str())
        || result
            .attributes
            .get(&asset_key)
            .map(|value| value.as_ref())
            != Some(request.asset_id.as_str())
    {
        return Err(BackupWorkError::Failed(
            "R2 object metadata verification failed".to_string(),
        ));
    }
    Ok(result.meta.e_tag)
}

fn upload_attributes(request: &BackupUploadRequest) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert(Attribute::ContentType, request.mime_type.clone().into());
    attributes.insert(
        Attribute::Metadata(Cow::Borrowed("bsaigc-asset-id")),
        request.asset_id.clone().into(),
    );
    attributes.insert(
        Attribute::Metadata(Cow::Borrowed("bsaigc-sha256")),
        request.content_sha256.clone().into(),
    );
    attributes
}

fn multipart_part_size(size_bytes: u64) -> usize {
    let minimum_for_part_limit = size_bytes.div_ceil(MAX_MULTIPART_PARTS);
    let rounded = minimum_for_part_limit.div_ceil(ONE_MIB) * ONE_MIB;
    rounded.max(MIN_MULTIPART_PART_SIZE) as usize
}

fn remote_object_key(prefix: &str, asset_id: &str, content_sha256: &str) -> String {
    format!(
        "{prefix}/assets/{}/{asset_id}/{content_sha256}",
        &content_sha256[..2]
    )
}

struct R2Config {
    endpoint: String,
    bucket: String,
    access_key_id: String,
    secret_access_key: String,
    region: String,
    object_prefix: String,
    poll_interval: Duration,
    connect_timeout: Duration,
    request_timeout: Duration,
}

enum R2ConfigLoad {
    Configured(R2Config),
    Degraded(String),
}

impl R2Config {
    /// Loads the R2 configuration from (in priority order) environment
    /// variables, then a `r2.config.json` file next to the executable /
    /// bundled resources / project root. The file keeps end users away from
    /// environment variables entirely: ship the JSON and the app is wired.
    fn load() -> R2ConfigLoad {
        let file_values = load_r2_config_file();
        Self::from_lookup(move |name| {
            if let Ok(value) = std::env::var(name) {
                if !value.trim().is_empty() {
                    return Some(value);
                }
            }
            file_values
                .as_ref()
                .and_then(|values| values.get(name).cloned())
        })
    }

    fn from_lookup<F>(lookup: F) -> R2ConfigLoad
    where
        F: Fn(&str) -> Option<String>,
    {
        let endpoint = non_empty(lookup(ENV_ENDPOINT));
        let account_id = non_empty(lookup(ENV_ACCOUNT_ID));
        let bucket = non_empty(lookup(ENV_BUCKET));
        let access_key_id = non_empty(lookup(ENV_ACCESS_KEY_ID));
        let secret_access_key = non_empty(lookup(ENV_SECRET_ACCESS_KEY));
        let any_configured = endpoint.is_some()
            || account_id.is_some()
            || bucket.is_some()
            || access_key_id.is_some()
            || secret_access_key.is_some();
        if !any_configured {
            return R2ConfigLoad::Degraded(
                "R2 backup is not configured; Local Vault remains authoritative".to_string(),
            );
        }

        let mut missing = Vec::new();
        if endpoint.is_none() && account_id.is_none() {
            missing.push(format!("{ENV_ENDPOINT} or {ENV_ACCOUNT_ID}"));
        }
        if bucket.is_none() {
            missing.push(ENV_BUCKET.to_string());
        }
        if access_key_id.is_none() {
            missing.push(ENV_ACCESS_KEY_ID.to_string());
        }
        if secret_access_key.is_none() {
            missing.push(ENV_SECRET_ACCESS_KEY.to_string());
        }
        if !missing.is_empty() {
            return R2ConfigLoad::Degraded(format!(
                "R2 backup configuration is incomplete; missing {}",
                missing.join(", ")
            ));
        }

        let endpoint = match endpoint {
            Some(value) => value.trim_end_matches('/').to_string(),
            None => {
                let account = account_id.expect("validated R2 account id");
                if !account
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                {
                    return R2ConfigLoad::Degraded(
                        "R2 account ID contains unsupported characters".to_string(),
                    );
                }
                format!("https://{account}.r2.cloudflarestorage.com")
            }
        };
        if !endpoint.starts_with("https://") {
            return R2ConfigLoad::Degraded("R2 endpoint must use HTTPS".to_string());
        }
        let bucket = bucket.expect("validated R2 bucket");
        if bucket.len() > 63
            || !bucket.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-.".contains(&byte)
            })
        {
            return R2ConfigLoad::Degraded(
                "R2 bucket name contains unsupported characters".to_string(),
            );
        }
        let object_prefix = match normalize_prefix(
            non_empty(lookup(ENV_PREFIX)).unwrap_or_else(|| "bsaigc-backup/v1".to_string()),
        ) {
            Ok(prefix) => prefix,
            Err(error) => return R2ConfigLoad::Degraded(error),
        };
        let poll_interval = match parse_duration(
            non_empty(lookup(ENV_POLL_MILLIS)),
            DEFAULT_POLL_INTERVAL,
            100,
            60_000,
            ENV_POLL_MILLIS,
            true,
        ) {
            Ok(value) => value,
            Err(error) => return R2ConfigLoad::Degraded(error),
        };
        let connect_timeout = match parse_duration(
            non_empty(lookup(ENV_CONNECT_TIMEOUT_SECS)),
            DEFAULT_CONNECT_TIMEOUT,
            1,
            300,
            ENV_CONNECT_TIMEOUT_SECS,
            false,
        ) {
            Ok(value) => value,
            Err(error) => return R2ConfigLoad::Degraded(error),
        };
        let request_timeout = match parse_duration(
            non_empty(lookup(ENV_REQUEST_TIMEOUT_SECS)),
            DEFAULT_REQUEST_TIMEOUT,
            30,
            86_400,
            ENV_REQUEST_TIMEOUT_SECS,
            false,
        ) {
            Ok(value) => value,
            Err(error) => return R2ConfigLoad::Degraded(error),
        };

        R2ConfigLoad::Configured(R2Config {
            endpoint,
            bucket,
            access_key_id: access_key_id.expect("validated R2 access key"),
            secret_access_key: secret_access_key.expect("validated R2 secret key"),
            region: non_empty(lookup(ENV_REGION)).unwrap_or_else(|| "auto".to_string()),
            object_prefix,
            poll_interval,
            connect_timeout,
            request_timeout,
        })
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn normalize_prefix(value: String) -> Result<String, String> {
    let value = value.trim_matches('/');
    if value.is_empty() || value.len() > 512 {
        return Err("R2 object prefix length must be 1..512".to_string());
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err("R2 object prefix contains an invalid path segment".to_string());
    }
    Ok(value.to_string())
}

fn parse_duration(
    raw: Option<String>,
    default: Duration,
    minimum: u64,
    maximum: u64,
    variable: &str,
    milliseconds: bool,
) -> Result<Duration, String> {
    let Some(raw) = raw else {
        return Ok(default);
    };
    let value = raw
        .parse::<u64>()
        .map_err(|_| format!("{variable} must be an integer"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!(
            "{variable} must be between {minimum} and {maximum}"
        ));
    }
    Ok(if milliseconds {
        Duration::from_millis(value)
    } else {
        Duration::from_secs(value)
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct R2FileConfig {
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    account_id: Option<String>,
    #[serde(default)]
    bucket: Option<String>,
    #[serde(default)]
    access_key_id: Option<String>,
    #[serde(default)]
    secret_access_key: Option<String>,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    prefix: Option<String>,
}

fn r2_config_file_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("r2.config.json"));
            candidates.push(dir.join("resources").join("r2.config.json"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("r2.config.json"));
        candidates.push(
            cwd.join("src-tauri")
                .join("resources")
                .join("r2.config.json"),
        );
        candidates.push(cwd.join("resources").join("r2.config.json"));
    }
    candidates
}

/// Reads the first parseable `r2.config.json` and maps its fields onto the
/// same variable names the environment uses. Returns `None` when no file
/// exists or none carries a non-empty value, so an empty placeholder shipped
/// in the installer behaves exactly like "not configured".
fn load_r2_config_file() -> Option<HashMap<String, String>> {
    for candidate in r2_config_file_candidates() {
        let Ok(raw) = fs::read_to_string(&candidate) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<R2FileConfig>(&raw) else {
            eprintln!(
                "r2.config.json at {} is not valid JSON; ignoring",
                candidate.display()
            );
            continue;
        };
        let mut values = HashMap::new();
        let mut insert = |key: &str, value: &Option<String>| {
            if let Some(value) = value {
                if !value.trim().is_empty() {
                    values.insert(key.to_string(), value.trim().to_string());
                }
            }
        };
        insert(ENV_ENDPOINT, &parsed.endpoint);
        insert(ENV_ACCOUNT_ID, &parsed.account_id);
        insert(ENV_BUCKET, &parsed.bucket);
        insert(ENV_ACCESS_KEY_ID, &parsed.access_key_id);
        insert(ENV_SECRET_ACCESS_KEY, &parsed.secret_access_key);
        insert(ENV_REGION, &parsed.region);
        insert(ENV_PREFIX, &parsed.prefix);
        if !values.is_empty() {
            return Some(values);
        }
    }
    None
}

#[cfg(not(test))]
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateConfigProbe {
    #[serde(default)]
    update_manifest_url: Option<String>,
}

/// Reads the online-update manifest URL from the same `r2.config.json`.
/// Only HTTPS URLs are accepted.
#[cfg(not(test))]
pub(crate) fn load_update_manifest_url() -> Option<String> {
    for candidate in r2_config_file_candidates() {
        let Ok(raw) = fs::read_to_string(&candidate) else {
            continue;
        };
        if let Ok(parsed) = serde_json::from_str::<UpdateConfigProbe>(&raw) {
            if let Some(url) = parsed.update_manifest_url {
                let url = url.trim().to_string();
                if url.starts_with("https://") {
                    return Some(url);
                }
            }
        }
    }
    None
}

fn bounded_error(error: &str) -> String {
    error.chars().take(MAX_PERSISTED_ERROR_CHARS).collect()
}

/// Synchronous small-object JSON store for shared registries (for example the
/// login account registry). Reuses the same `BSAIGC_R2_*` environment
/// configuration as the backup worker, but with short interactive timeouts.
/// Registry objects live under `<prefix>/registry/`.
pub struct RegistryStore {
    runtime: Mutex<Runtime>,
    store: AmazonS3,
    object_prefix: String,
}

pub enum RegistryStoreLoad {
    Configured(RegistryStore),
    Unconfigured,
    Invalid(String),
}

impl RegistryStore {
    pub fn from_env() -> RegistryStoreLoad {
        let mut config = match R2Config::load() {
            R2ConfigLoad::Configured(config) => config,
            R2ConfigLoad::Degraded(reason) => {
                return if reason.contains("not configured") {
                    RegistryStoreLoad::Unconfigured
                } else {
                    RegistryStoreLoad::Invalid(reason)
                };
            }
        };
        config.connect_timeout = Duration::from_secs(5);
        config.request_timeout = Duration::from_secs(20);
        let options = ClientOptions::new()
            .with_connect_timeout(config.connect_timeout)
            .with_timeout(config.request_timeout);
        let store = match AmazonS3Builder::new()
            .with_bucket_name(config.bucket)
            .with_region(config.region)
            .with_endpoint(config.endpoint)
            .with_access_key_id(config.access_key_id)
            .with_secret_access_key(config.secret_access_key)
            .with_virtual_hosted_style_request(false)
            .with_client_options(options)
            .build()
        {
            Ok(store) => store,
            Err(error) => return RegistryStoreLoad::Invalid(bounded_error(&error.to_string())),
        };
        let runtime = match RuntimeBuilder::new_current_thread().enable_all().build() {
            Ok(runtime) => runtime,
            Err(error) => return RegistryStoreLoad::Invalid(bounded_error(&error.to_string())),
        };
        RegistryStoreLoad::Configured(Self {
            runtime: Mutex::new(runtime),
            store,
            object_prefix: config.object_prefix,
        })
    }

    fn registry_location(&self, name: &str) -> Result<ObjectPath, String> {
        let key = format!("{}/registry/{name}", self.object_prefix.trim_matches('/'));
        ObjectPath::parse(&key).map_err(|_| format!("construct registry object key failed: {key}"))
    }

    pub fn get_object(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let location = self.registry_location(name)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "registry runtime lock is poisoned".to_string())?;
        runtime.block_on(async {
            match self.store.get(&location).await {
                Ok(result) => match result.bytes().await {
                    Ok(bytes) => Ok(Some(bytes.to_vec())),
                    Err(error) => Err(bounded_error(&format!(
                        "read registry object failed: {error}"
                    ))),
                },
                Err(object_store::Error::NotFound { .. }) => Ok(None),
                Err(error) => Err(bounded_error(&format!(
                    "download registry object failed: {error}"
                ))),
            }
        })
    }

    pub fn put_object(&self, name: &str, bytes: Vec<u8>) -> Result<(), String> {
        let location = self.registry_location(name)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "registry runtime lock is poisoned".to_string())?;
        runtime.block_on(async {
            self.store
                .put(&location, bytes.into())
                .await
                .map(|_| ())
                .map_err(|error| bounded_error(&format!("upload registry object failed: {error}")))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_service;
    use crate::protocol::{
        BackupCommandEnvelope, BackupEventType, CancelAssetBackupPayload, OperationContext,
        QueueAssetBackupPayload, RestoreAssetBackupPayload, BACKUP_PROTOCOL_VERSION,
    };
    use std::collections::VecDeque;
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tempfile::TempDir;

    struct Fixture {
        _temp: TempDir,
        database_path: PathBuf,
        vault_root: PathBuf,
        outbox: Arc<BackupOutbox>,
        asset_id: String,
        asset_sha256: String,
    }

    impl Fixture {
        fn new(content: &[u8]) -> Self {
            let temp = tempfile::tempdir().unwrap();
            let database_path = temp.path().join("ledger").join("test.sqlite3");
            let vault_root = temp.path().join("vault");
            std::fs::create_dir_all(database_path.parent().unwrap()).unwrap();
            let source = temp.path().join("contract.pdf");
            std::fs::write(&source, content).unwrap();
            let mut connection = Connection::open(&database_path).unwrap();
            asset_service::migrate(&connection).unwrap();
            let asset =
                asset_service::import_file(&mut connection, &vault_root, None, &source).unwrap();
            drop(connection);
            let outbox = Arc::new(BackupOutbox::open(&database_path).unwrap());
            outbox
                .queue(queue_command(&asset.id), &asset.sha256)
                .unwrap();
            Self {
                _temp: temp,
                database_path,
                vault_root,
                outbox,
                asset_id: asset.id,
                asset_sha256: asset.sha256,
            }
        }

        fn core(&self, transport: Arc<dyn BackupTransport>) -> BackupWorkerCore {
            BackupWorkerCore {
                outbox: Arc::clone(&self.outbox),
                database_path: self.database_path.clone(),
                vault_root: self.vault_root.clone(),
                transport,
                event_sink: Arc::new(|_| {}),
                control: Arc::new(WorkerControl::default()),
            }
        }

        fn local_path(&self) -> PathBuf {
            let connection = Connection::open(&self.database_path).unwrap();
            let relative: String = connection
                .query_row(
                    "SELECT storage_rel_path FROM assets WHERE id = ?1",
                    params![self.asset_id],
                    |row| row.get(0),
                )
                .unwrap();
            self.vault_root.join(relative)
        }

        fn mark_backed_up(&self, object_key: &str, etag: Option<&str>) -> AssetBackupRecord {
            let claimed = self
                .outbox
                .claim_next("test:restore:claim")
                .unwrap()
                .backup
                .unwrap();
            self.outbox
                .mark_backed_up(
                    &self.asset_id,
                    &self.asset_sha256,
                    claimed.revision,
                    object_key,
                    etag,
                    "test:restore:backed-up",
                )
                .unwrap()
                .backup
        }

        fn restore_coordinator(&self, transport: Arc<dyn BackupTransport>) -> RestoreCoordinator {
            RestoreCoordinator {
                outbox: Arc::clone(&self.outbox),
                database_path: self.database_path.clone(),
                vault_root: self.vault_root.clone(),
                transport,
            }
        }
        fn restore_staging_paths(&self) -> Vec<PathBuf> {
            let staging_root = self.vault_root.join(".restore-staging");
            if !staging_root.exists() {
                return Vec::new();
            }
            std::fs::read_dir(staging_root)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect()
        }
    }

    struct FakeTransport {
        outcomes: Mutex<VecDeque<Result<BackupUploadResult, String>>>,
        requests: Mutex<Vec<BackupUploadRequest>>,
    }

    impl FakeTransport {
        fn succeeding() -> Self {
            Self {
                outcomes: Mutex::new(VecDeque::from([Ok(BackupUploadResult {
                    remote_object_key: "fake/vault/object".to_string(),
                    remote_etag: Some("fake-etag".to_string()),
                })])),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn failing(message: &str) -> Self {
            Self {
                outcomes: Mutex::new(VecDeque::from([Err(message.to_string())])),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    impl BackupTransport for FakeTransport {
        fn expected_object_key(&self, _asset_id: &str, _content_sha256: &str) -> String {
            "fake/vault/object".to_string()
        }

        fn upload(
            &self,
            request: BackupUploadRequest,
            cancellation: CancellationToken,
        ) -> Result<BackupUploadResult, BackupWorkError> {
            if cancellation.is_cancelled() {
                return Err(BackupWorkError::Cancelled);
            }
            self.requests
                .lock()
                .map_err(|_| BackupWorkError::Failed("fake request lock poisoned".to_string()))?
                .push(request);
            match self
                .outcomes
                .lock()
                .map_err(|_| BackupWorkError::Failed("fake outcomes lock poisoned".to_string()))?
                .pop_front()
            {
                Some(Ok(result)) => Ok(result),
                Some(Err(error)) => Err(BackupWorkError::Failed(error)),
                None => Err(BackupWorkError::Failed(
                    "fake transport has no outcome".to_string(),
                )),
            }
        }

        fn download(
            &self,
            _request: BackupRestoreRequest,
            _cancellation: CancellationToken,
        ) -> Result<BackupRestoreResult, BackupWorkError> {
            Err(BackupWorkError::Failed(
                "fake upload transport does not provide restore objects".to_string(),
            ))
        }
    }

    struct BlockingFakeTransport {
        started: Arc<AtomicBool>,
    }

    #[derive(Clone)]
    struct FakeRemoteObject {
        content: Vec<u8>,
        size_bytes: u64,
        asset_id: Option<String>,
        sha256: Option<String>,
        etag: Option<String>,
    }

    struct FakeRestoreTransport {
        object_prefix: String,
        objects: Mutex<HashMap<String, FakeRemoteObject>>,
        requests: Mutex<Vec<String>>,
    }

    impl FakeRestoreTransport {
        fn empty() -> Self {
            Self {
                object_prefix: "fake-backup/v1".to_string(),
                objects: Mutex::new(HashMap::new()),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn with_object(
            asset_id: &str,
            sha256: &str,
            content: Vec<u8>,
            metadata_sha256: Option<String>,
        ) -> Self {
            let transport = Self::empty();
            let key = transport.expected_object_key(asset_id, sha256);
            transport.objects.lock().unwrap().insert(
                key,
                FakeRemoteObject {
                    size_bytes: content.len() as u64,
                    content,
                    asset_id: Some(asset_id.to_string()),
                    sha256: metadata_sha256.or_else(|| Some(sha256.to_string())),
                    etag: Some("restore-etag".to_string()),
                },
            );
            transport
        }
    }

    impl BackupTransport for FakeRestoreTransport {
        fn expected_object_key(&self, asset_id: &str, content_sha256: &str) -> String {
            remote_object_key(&self.object_prefix, asset_id, content_sha256)
        }

        fn upload(
            &self,
            request: BackupUploadRequest,
            _cancellation: CancellationToken,
        ) -> Result<BackupUploadResult, BackupWorkError> {
            Ok(BackupUploadResult {
                remote_object_key: self
                    .expected_object_key(&request.asset_id, &request.content_sha256),
                remote_etag: Some("restore-etag".to_string()),
            })
        }

        fn download(
            &self,
            mut request: BackupRestoreRequest,
            cancellation: CancellationToken,
        ) -> Result<BackupRestoreResult, BackupWorkError> {
            if cancellation.is_cancelled() {
                return Err(BackupWorkError::Cancelled);
            }
            self.requests
                .lock()
                .map_err(|_| BackupWorkError::Failed("fake request lock poisoned".to_string()))?
                .push(request.remote_object_key.clone());
            let object = self
                .objects
                .lock()
                .map_err(|_| BackupWorkError::Failed("fake object lock poisoned".to_string()))?
                .get(&request.remote_object_key)
                .cloned()
                .ok_or_else(|| {
                    BackupWorkError::Failed(format!(
                        "unknown R2 object key {}",
                        request.remote_object_key
                    ))
                })?;
            for chunk in object.content.chunks(3) {
                if cancellation.is_cancelled() {
                    return Err(BackupWorkError::Cancelled);
                }
                request.staging_file.write_all(chunk).map_err(|error| {
                    BackupWorkError::Failed(format!("fake staging write failed: {error}"))
                })?;
            }
            request.staging_file.flush().map_err(|error| {
                BackupWorkError::Failed(format!("fake staging flush failed: {error}"))
            })?;
            request.staging_file.sync_all().map_err(|error| {
                BackupWorkError::Failed(format!("fake staging sync failed: {error}"))
            })?;
            Ok(BackupRestoreResult {
                size_bytes: object.size_bytes,
                asset_id: object.asset_id,
                sha256: object.sha256,
                etag: object.etag,
            })
        }
    }

    impl BackupTransport for BlockingFakeTransport {
        fn expected_object_key(&self, _asset_id: &str, _content_sha256: &str) -> String {
            "fake/blocking/object".to_string()
        }

        fn upload(
            &self,
            _request: BackupUploadRequest,
            cancellation: CancellationToken,
        ) -> Result<BackupUploadResult, BackupWorkError> {
            self.started.store(true, Ordering::SeqCst);
            while !cancellation.is_cancelled() {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(BackupWorkError::Cancelled)
        }

        fn download(
            &self,
            _request: BackupRestoreRequest,
            _cancellation: CancellationToken,
        ) -> Result<BackupRestoreResult, BackupWorkError> {
            Err(BackupWorkError::Failed(
                "blocking upload transport does not provide restore objects".to_string(),
            ))
        }
    }

    #[test]
    fn fake_transport_completes_claim_and_persists_remote_identity() {
        let fixture = Fixture::new(b"signed contract");
        let transport = Arc::new(FakeTransport::succeeding());
        assert!(fixture.core(transport.clone()).process_once().unwrap());

        let backup = fixture.outbox.get(&fixture.asset_id).unwrap().unwrap();
        assert_eq!(backup.state, BackupState::BackedUp);
        assert_eq!(
            backup.remote_object_key.as_deref(),
            Some("fake/vault/object")
        );
        assert_eq!(backup.remote_etag.as_deref(), Some("fake-etag"));
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].asset_id, fixture.asset_id);
        assert_eq!(requests[0].content_sha256, fixture.asset_sha256);
    }

    #[test]
    fn hash_mismatch_fails_durably_without_calling_transport() {
        let fixture = Fixture::new(b"original contract");
        std::fs::write(fixture.local_path(), b"tampered contract").unwrap();
        let transport = Arc::new(FakeTransport::succeeding());
        assert!(fixture.core(transport.clone()).process_once().unwrap());

        let backup = fixture.outbox.get(&fixture.asset_id).unwrap().unwrap();
        assert_eq!(backup.state, BackupState::Failed);
        assert!(backup.last_error.unwrap().contains("SHA-256 mismatch"));
        assert!(transport.requests.lock().unwrap().is_empty());
    }

    #[test]
    fn transport_failure_only_fails_backup_and_preserves_local_asset() {
        let fixture = Fixture::new(b"local authority survives offline R2");
        let transport = Arc::new(FakeTransport::failing("network unavailable"));
        assert!(fixture.core(transport).process_once().unwrap());

        let backup = fixture.outbox.get(&fixture.asset_id).unwrap().unwrap();
        assert_eq!(backup.state, BackupState::Failed);
        assert!(backup.last_error.unwrap().contains("network unavailable"));
        assert_eq!(
            std::fs::read(fixture.local_path()).unwrap(),
            b"local authority survives offline R2"
        );
        let connection = Connection::open(&fixture.database_path).unwrap();
        let asset = asset_service::get_asset(&connection, &fixture.asset_id).unwrap();
        assert_eq!(asset.sha256, fixture.asset_sha256);
    }

    #[test]
    fn durable_cancel_interrupts_active_transport_without_overwriting_cancelled_state() {
        let fixture = Fixture::new(b"cancel this upload");
        let started = Arc::new(AtomicBool::new(false));
        let worker = R2BackupWorker::start_with_transport(
            Arc::clone(&fixture.outbox),
            fixture.database_path.clone(),
            fixture.vault_root.clone(),
            Arc::new(BlockingFakeTransport {
                started: Arc::clone(&started),
            }),
            Arc::new(|_| {}),
            Duration::from_millis(10),
        )
        .unwrap();

        wait_until(Duration::from_secs(2), || started.load(Ordering::SeqCst));
        let uploading = fixture.outbox.get(&fixture.asset_id).unwrap().unwrap();
        assert_eq!(uploading.state, BackupState::Uploading);
        fixture
            .outbox
            .cancel(cancel_command(&fixture.asset_id, uploading.revision))
            .unwrap();
        worker.cancel(&fixture.asset_id);
        wait_until(Duration::from_secs(2), || {
            fixture
                .outbox
                .get(&fixture.asset_id)
                .unwrap()
                .is_some_and(|backup| backup.state == BackupState::Cancelled)
        });
        worker.shutdown();
        assert_eq!(
            fixture
                .outbox
                .get(&fixture.asset_id)
                .unwrap()
                .unwrap()
                .state,
            BackupState::Cancelled
        );
    }

    #[test]
    fn restart_recovers_interrupted_claim_and_fake_transport_finishes_it() {
        let fixture = Fixture::new(b"restart-safe backup");
        let claim = fixture.outbox.claim_next("test:before-restart").unwrap();
        assert_eq!(claim.backup.unwrap().state, BackupState::Uploading);

        let recovered = Arc::new(BackupOutbox::open(&fixture.database_path).unwrap());
        assert_eq!(
            recovered.get(&fixture.asset_id).unwrap().unwrap().state,
            BackupState::Queued
        );
        let core = BackupWorkerCore {
            outbox: Arc::clone(&recovered),
            database_path: fixture.database_path.clone(),
            vault_root: fixture.vault_root.clone(),
            transport: Arc::new(FakeTransport::succeeding()),
            event_sink: Arc::new(|_| {}),
            control: Arc::new(WorkerControl::default()),
        };
        assert!(core.process_once().unwrap());
        assert_eq!(
            recovered.get(&fixture.asset_id).unwrap().unwrap().state,
            BackupState::BackedUp
        );
    }

    #[test]
    fn restore_streams_to_staging_commits_atomically_and_replays_without_redownload() {
        let content = b"contract restore payload".to_vec();
        let fixture = Fixture::new(&content);
        let transport = Arc::new(FakeRestoreTransport::with_object(
            &fixture.asset_id,
            &fixture.asset_sha256,
            content.clone(),
            None,
        ));
        let object_key = transport.expected_object_key(&fixture.asset_id, &fixture.asset_sha256);
        let backup = fixture.mark_backed_up(&object_key, Some("restore-etag"));
        std::fs::remove_file(fixture.local_path()).unwrap();
        let command = restore_command(
            &fixture.asset_id,
            &fixture.asset_sha256,
            backup.revision,
            "restore-success-0001",
        );
        let coordinator = fixture.restore_coordinator(transport.clone());

        let restored = coordinator.execute(command.clone()).unwrap();
        assert!(!restored.response.replayed);
        assert_eq!(restored.response.backup.state, BackupState::BackedUp);
        assert_eq!(restored.emitted_events.len(), 1);
        assert_eq!(
            restored.emitted_events[0].event_type,
            BackupEventType::Restored
        );
        assert_eq!(std::fs::read(fixture.local_path()).unwrap(), content);
        assert!(fixture.restore_staging_paths().is_empty());
        assert_eq!(transport.requests.lock().unwrap().len(), 1);

        let replayed = coordinator.execute(command).unwrap();
        assert!(replayed.response.replayed);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(transport.requests.lock().unwrap().len(), 1);
        assert_eq!(
            fixture
                .outbox
                .replay_events(0, 100)
                .unwrap()
                .iter()
                .filter(|event| event.event_type == BackupEventType::Restored)
                .count(),
            1
        );
    }

    #[test]
    fn correct_existing_destination_finishes_receipt_without_network_download() {
        let content = b"already durable local vault bytes".to_vec();
        let fixture = Fixture::new(&content);
        let transport = Arc::new(FakeRestoreTransport::empty());
        let object_key = transport.expected_object_key(&fixture.asset_id, &fixture.asset_sha256);
        let backup = fixture.mark_backed_up(&object_key, Some("restore-etag"));
        let command = restore_command(
            &fixture.asset_id,
            &fixture.asset_sha256,
            backup.revision,
            "restore-existing-0001",
        );

        let restored = fixture
            .restore_coordinator(transport.clone())
            .execute(command)
            .unwrap();

        assert_eq!(std::fs::read(fixture.local_path()).unwrap(), content);
        assert_eq!(restored.emitted_events.len(), 1);
        assert_eq!(
            restored.emitted_events[0].event_type,
            BackupEventType::Restored
        );
        assert!(transport.requests.lock().unwrap().is_empty());
        assert!(fixture.restore_staging_paths().is_empty());
    }

    #[test]
    fn restore_rejects_remote_metadata_sha_without_committing_or_leaking_staging() {
        let content = b"metadata verification payload".to_vec();
        let fixture = Fixture::new(&content);
        let transport = Arc::new(FakeRestoreTransport::with_object(
            &fixture.asset_id,
            &fixture.asset_sha256,
            content,
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
        ));
        let object_key = transport.expected_object_key(&fixture.asset_id, &fixture.asset_sha256);
        let backup = fixture.mark_backed_up(&object_key, Some("restore-etag"));
        let local_path = fixture.local_path();
        std::fs::remove_file(&local_path).unwrap();
        let command = restore_command(
            &fixture.asset_id,
            &fixture.asset_sha256,
            backup.revision,
            "restore-bad-metadata-0001",
        );

        let error = fixture
            .restore_coordinator(transport)
            .execute(command)
            .unwrap_err();

        assert_eq!(error.code, "BACKUP_RESTORE_REMOTE_METADATA_MISMATCH");
        assert!(!local_path.exists());
        assert!(fixture.restore_staging_paths().is_empty());
        assert!(!fixture
            .outbox
            .replay_events(0, 100)
            .unwrap()
            .iter()
            .any(|event| event.event_type == BackupEventType::Restored));
    }

    #[test]
    fn restore_rejects_etag_mismatch_without_committing() {
        let content = b"etag verification payload".to_vec();
        let fixture = Fixture::new(&content);
        let transport = Arc::new(FakeRestoreTransport::with_object(
            &fixture.asset_id,
            &fixture.asset_sha256,
            content,
            None,
        ));
        let object_key = transport.expected_object_key(&fixture.asset_id, &fixture.asset_sha256);
        let backup = fixture.mark_backed_up(&object_key, Some("manifest-etag"));
        let local_path = fixture.local_path();
        std::fs::remove_file(&local_path).unwrap();
        let command = restore_command(
            &fixture.asset_id,
            &fixture.asset_sha256,
            backup.revision,
            "restore-bad-etag-0001",
        );

        let error = fixture
            .restore_coordinator(transport)
            .execute(command)
            .unwrap_err();

        assert_eq!(error.code, "BACKUP_RESTORE_REMOTE_METADATA_MISMATCH");
        assert!(!local_path.exists());
        assert!(fixture.restore_staging_paths().is_empty());
    }

    #[test]
    fn restore_rejects_size_mismatch_without_committing() {
        let expected = b"expected size payload".to_vec();
        let mut oversized = expected.clone();
        oversized.push(b'!');
        let fixture = Fixture::new(&expected);
        let transport = Arc::new(FakeRestoreTransport::with_object(
            &fixture.asset_id,
            &fixture.asset_sha256,
            oversized,
            None,
        ));
        let object_key = transport.expected_object_key(&fixture.asset_id, &fixture.asset_sha256);
        let backup = fixture.mark_backed_up(&object_key, Some("restore-etag"));
        let local_path = fixture.local_path();
        std::fs::remove_file(&local_path).unwrap();
        let command = restore_command(
            &fixture.asset_id,
            &fixture.asset_sha256,
            backup.revision,
            "restore-bad-size-0001",
        );

        let error = fixture
            .restore_coordinator(transport)
            .execute(command)
            .unwrap_err();

        assert_eq!(error.code, "BACKUP_RESTORE_REMOTE_METADATA_MISMATCH");
        assert!(!local_path.exists());
        assert!(fixture.restore_staging_paths().is_empty());
    }

    #[test]
    fn restore_hashes_downloaded_bytes_and_rejects_content_tampering() {
        let expected = b"content integrity payload".to_vec();
        let mut tampered = expected.clone();
        tampered[0] ^= 0x20;
        let fixture = Fixture::new(&expected);
        let transport = Arc::new(FakeRestoreTransport::with_object(
            &fixture.asset_id,
            &fixture.asset_sha256,
            tampered,
            None,
        ));
        let object_key = transport.expected_object_key(&fixture.asset_id, &fixture.asset_sha256);
        let backup = fixture.mark_backed_up(&object_key, Some("restore-etag"));
        let local_path = fixture.local_path();
        std::fs::remove_file(&local_path).unwrap();
        let command = restore_command(
            &fixture.asset_id,
            &fixture.asset_sha256,
            backup.revision,
            "restore-bad-content-0001",
        );

        let error = fixture
            .restore_coordinator(transport)
            .execute(command)
            .unwrap_err();

        assert_eq!(error.code, "BACKUP_RESTORE_INTEGRITY_FAILED");
        assert!(!local_path.exists());
        assert!(fixture.restore_staging_paths().is_empty());
    }

    #[test]
    fn restore_refuses_to_overwrite_conflicting_local_vault_content() {
        let expected = b"authoritative vault content".to_vec();
        let fixture = Fixture::new(&expected);
        let transport = Arc::new(FakeRestoreTransport::with_object(
            &fixture.asset_id,
            &fixture.asset_sha256,
            expected,
            None,
        ));
        let object_key = transport.expected_object_key(&fixture.asset_id, &fixture.asset_sha256);
        let backup = fixture.mark_backed_up(&object_key, Some("restore-etag"));
        let local_path = fixture.local_path();
        let conflicting = vec![b'x'; std::fs::metadata(&local_path).unwrap().len() as usize];
        std::fs::write(&local_path, &conflicting).unwrap();
        let command = restore_command(
            &fixture.asset_id,
            &fixture.asset_sha256,
            backup.revision,
            "restore-conflict-0001",
        );

        let error = fixture
            .restore_coordinator(transport.clone())
            .execute(command)
            .unwrap_err();

        assert_eq!(error.code, "BACKUP_RESTORE_DESTINATION_CONFLICT");
        assert_eq!(std::fs::read(local_path).unwrap(), conflicting);
        assert!(transport.requests.lock().unwrap().is_empty());
        assert!(fixture.restore_staging_paths().is_empty());
    }

    #[test]
    fn restore_rejects_vault_path_traversal_before_network_or_filesystem_escape() {
        let content = b"path traversal payload".to_vec();
        let fixture = Fixture::new(&content);
        let transport = Arc::new(FakeRestoreTransport::with_object(
            &fixture.asset_id,
            &fixture.asset_sha256,
            content,
            None,
        ));
        let object_key = transport.expected_object_key(&fixture.asset_id, &fixture.asset_sha256);
        let backup = fixture.mark_backed_up(&object_key, Some("restore-etag"));
        std::fs::remove_file(fixture.local_path()).unwrap();
        Connection::open(&fixture.database_path)
            .unwrap()
            .execute(
                "UPDATE assets SET storage_rel_path = '../escaped.bin' WHERE id = ?1",
                params![&fixture.asset_id],
            )
            .unwrap();
        let escaped = fixture.vault_root.parent().unwrap().join("escaped.bin");
        let command = restore_command(
            &fixture.asset_id,
            &fixture.asset_sha256,
            backup.revision,
            "restore-path-escape-0001",
        );

        let error = fixture
            .restore_coordinator(transport.clone())
            .execute(command)
            .unwrap_err();

        assert_eq!(error.code, "BACKUP_RESTORE_VAULT_PATH_INVALID");
        assert!(!escaped.exists());
        assert!(transport.requests.lock().unwrap().is_empty());
        assert!(fixture.restore_staging_paths().is_empty());
    }

    #[test]
    fn transient_r2_failure_preserves_local_authority_and_same_command_can_retry() {
        let content = b"retryable restore payload".to_vec();
        let fixture = Fixture::new(&content);
        let transport = Arc::new(FakeRestoreTransport::empty());
        let object_key = transport.expected_object_key(&fixture.asset_id, &fixture.asset_sha256);
        let backup = fixture.mark_backed_up(&object_key, Some("restore-etag"));
        let local_path = fixture.local_path();
        std::fs::remove_file(&local_path).unwrap();
        let command = restore_command(
            &fixture.asset_id,
            &fixture.asset_sha256,
            backup.revision,
            "restore-retryable-0001",
        );
        let coordinator = fixture.restore_coordinator(transport.clone());

        let first_error = coordinator.execute(command.clone()).unwrap_err();
        assert_eq!(first_error.code, "BACKUP_RESTORE_FAILED");
        let unchanged = fixture.outbox.get(&fixture.asset_id).unwrap().unwrap();
        assert_eq!(unchanged.state, BackupState::BackedUp);
        assert_eq!(unchanged.revision, backup.revision);
        assert!(!local_path.exists());
        assert!(fixture.restore_staging_paths().is_empty());

        transport.objects.lock().unwrap().insert(
            object_key,
            FakeRemoteObject {
                size_bytes: content.len() as u64,
                content: content.clone(),
                asset_id: Some(fixture.asset_id.clone()),
                sha256: Some(fixture.asset_sha256.clone()),
                etag: Some("restore-etag".to_string()),
            },
        );
        let restored = coordinator.execute(command.clone()).unwrap();
        assert!(!restored.response.replayed);
        assert_eq!(std::fs::read(&local_path).unwrap(), content);
        let replayed = coordinator.execute(command).unwrap();
        assert!(replayed.response.replayed);
        assert_eq!(transport.requests.lock().unwrap().len(), 2);
    }
    #[test]
    fn incomplete_or_missing_environment_is_degraded_without_exposing_secrets() {
        let empty = R2Config::from_lookup(|_| None);
        match empty {
            R2ConfigLoad::Degraded(reason) => {
                assert!(reason.contains("not configured"));
            }
            R2ConfigLoad::Configured(_) => panic!("missing environment must be degraded"),
        }

        let mut values = HashMap::new();
        values.insert(ENV_ACCOUNT_ID, "account-id".to_string());
        values.insert(ENV_BUCKET, "contracts".to_string());
        values.insert(ENV_ACCESS_KEY_ID, "access".to_string());
        values.insert(ENV_SECRET_ACCESS_KEY, "super-secret-value".to_string());
        values.insert(ENV_ENDPOINT, "http://unsafe.example".to_string());
        match R2Config::from_lookup(|name| values.get(name).cloned()) {
            R2ConfigLoad::Degraded(reason) => {
                assert!(reason.contains("HTTPS"));
                assert!(!reason.contains("super-secret-value"));
            }
            R2ConfigLoad::Configured(_) => panic!("HTTP endpoint must be rejected"),
        }
    }

    #[test]
    fn multipart_size_stays_within_r2_part_count_for_one_terabyte() {
        let one_terabyte = 1024_u64 * 1024 * 1024 * 1024;
        let part_size = multipart_part_size(one_terabyte) as u64;
        assert!(part_size >= MIN_MULTIPART_PART_SIZE);
        assert!(one_terabyte.div_ceil(part_size) <= MAX_MULTIPART_PARTS);
    }

    fn queue_command(asset_id: &str) -> BackupCommandEnvelope {
        BackupCommandEnvelope::Queue {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
            context: test_context("queue"),
            payload: QueueAssetBackupPayload {
                asset_id: asset_id.to_string(),
            },
            idempotency_key: format!("test:queue:{asset_id}"),
            expected_revision: None,
            deadline_at: None,
        }
    }

    fn restore_command(
        asset_id: &str,
        expected_sha256: &str,
        revision: i64,
        idempotency_key: &str,
    ) -> BackupCommandEnvelope {
        BackupCommandEnvelope::Restore {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
            context: test_context("restore"),
            payload: RestoreAssetBackupPayload {
                asset_id: asset_id.to_string(),
                expected_sha256: expected_sha256.to_string(),
            },
            idempotency_key: idempotency_key.to_string(),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }
    fn cancel_command(asset_id: &str, revision: i64) -> BackupCommandEnvelope {
        BackupCommandEnvelope::Cancel {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
            context: test_context("cancel"),
            payload: CancelAssetBackupPayload {
                asset_id: asset_id.to_string(),
            },
            idempotency_key: format!("test:cancel:{asset_id}:{revision}"),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }

    fn test_context(action: &str) -> OperationContext {
        OperationContext {
            actor_id: "test-operator".to_string(),
            account_id: None,
            project_id: None,
            window_id: "test-window".to_string(),
            trace_id: format!("test:r2:{action}:{}", Uuid::new_v4()),
        }
    }

    fn wait_until(timeout: Duration, predicate: impl Fn() -> bool) {
        let started = std::time::Instant::now();
        while !predicate() {
            assert!(
                started.elapsed() < timeout,
                "timed out waiting for condition"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
