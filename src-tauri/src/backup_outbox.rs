use crate::protocol::{
    AssetBackupRecord, BackupCommandEnvelope, BackupCommandResponse, BackupDomainEvent,
    BackupEventType, BackupState, CommandReceipt, HostError, OperationContext,
    BACKUP_PROTOCOL_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const BACKUP_COLUMNS: &str = "asset_id, content_sha256, state, attempt_count, \
    next_attempt_at, last_error, remote_object_key, remote_etag, revision, created_at, \
    updated_at, backed_up_at";
const STARTUP_RECOVERY_TRACE_ID: &str = "backup-outbox:startup-recovery";
const DEFAULT_BASE_BACKOFF_MILLIS: i64 = 5_000;
const DEFAULT_MAX_BACKOFF_MILLIS: i64 = 6 * 60 * 60 * 1_000;
const MAX_PAGE_SIZE: usize = 1_000;

/// Retry timing belongs to the durable outbox, not to the R2 transport adapter.
/// The adapter receives an already-claimed record and reports only success or failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupRetryPolicy {
    pub base_backoff_millis: i64,
    pub max_backoff_millis: i64,
}

impl Default for BackupRetryPolicy {
    fn default() -> Self {
        Self {
            base_backoff_millis: DEFAULT_BASE_BACKOFF_MILLIS,
            max_backoff_millis: DEFAULT_MAX_BACKOFF_MILLIS,
        }
    }
}

impl BackupRetryPolicy {
    fn validate(self) -> Result<Self, HostError> {
        if self.base_backoff_millis <= 0 {
            return Err(HostError::validation(
                "backup baseBackoffMillis must be greater than zero",
            ));
        }
        if self.max_backoff_millis < self.base_backoff_millis {
            return Err(HostError::validation(
                "backup maxBackoffMillis must be greater than or equal to baseBackoffMillis",
            ));
        }
        Ok(self)
    }

    fn delay_for_attempt(self, attempt_count: i64) -> i64 {
        let exponent = attempt_count.saturating_sub(1).clamp(0, 62) as u32;
        self.base_backoff_millis
            .checked_mul(1_i64.checked_shl(exponent).unwrap_or(i64::MAX))
            .unwrap_or(i64::MAX)
            .min(self.max_backoff_millis)
    }
}

/// SQLite-backed R2 backup outbox.
///
/// This module owns only durable backup intent and lifecycle state. A Local Vault write is
/// already successful before an item is queued here; R2 failures never roll back or downgrade
/// that local success. No network calls, R2 credentials, or local file reads occur in this type.
pub struct BackupOutbox {
    connection: Mutex<Connection>,
    retry_policy: BackupRetryPolicy,
}

pub type DurableBackupOutbox = BackupOutbox;

#[derive(Debug)]
pub struct BackupCommandOutcome {
    pub response: BackupCommandResponse,
    pub emitted_events: Vec<BackupDomainEvent>,
}

#[derive(Debug)]
pub struct BackupLifecycleOutcome {
    pub backup: AssetBackupRecord,
    pub emitted_events: Vec<BackupDomainEvent>,
}

#[derive(Debug)]
pub struct BackupClaimOutcome {
    pub backup: Option<AssetBackupRecord>,
    pub emitted_events: Vec<BackupDomainEvent>,
}

#[derive(Debug)]
pub struct BackupRecoveryOutcome {
    pub backups: Vec<AssetBackupRecord>,
    pub emitted_events: Vec<BackupDomainEvent>,
}

#[derive(Debug)]
pub struct BackupRestorePreparation {
    pub backup: AssetBackupRecord,
    pub replayed_response: Option<BackupCommandResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReusableBackedUpObject {
    pub asset_id: String,
    pub remote_object_key: String,
    pub etag: Option<String>,
}

#[derive(Debug)]
struct BackupCommandMeta {
    command_id: String,
    protocol_version: String,
    context: OperationContext,
    idempotency_key: String,
    expected_revision: Option<i64>,
    deadline_at: Option<i64>,
}

#[derive(Debug)]
struct StoredReceipt {
    command_id: String,
    idempotency_key: String,
    request_fingerprint: String,
    response_json: String,
}

impl BackupOutbox {
    pub fn open(database_path: &Path) -> Result<Self, HostError> {
        Self::open_with_retry_policy(database_path, BackupRetryPolicy::default())
    }

    pub fn open_with_retry_policy(
        database_path: &Path,
        retry_policy: BackupRetryPolicy,
    ) -> Result<Self, HostError> {
        if let Some(parent) = database_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                HostError::internal(format!("create backup outbox directory failed: {error}"))
            })?;
        }
        let connection = Connection::open(database_path).map_err(sql_error)?;
        Self::from_connection_with_retry_policy(connection, retry_policy)
    }

    pub fn from_connection(connection: Connection) -> Result<Self, HostError> {
        Self::from_connection_with_retry_policy(connection, BackupRetryPolicy::default())
    }

    pub fn from_connection_with_retry_policy(
        connection: Connection,
        retry_policy: BackupRetryPolicy,
    ) -> Result<Self, HostError> {
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(sql_error)?;
        migrate(&connection)?;
        let outbox = Self {
            connection: Mutex::new(connection),
            retry_policy: retry_policy.validate()?,
        };
        outbox.recover_interrupted_uploads(STARTUP_RECOVERY_TRACE_ID)?;
        Ok(outbox)
    }

    pub fn migrate(connection: &Connection) -> Result<(), HostError> {
        migrate(connection)
    }

    /// Queues an immutable Local Vault asset for asynchronous backup.
    ///
    /// `content_sha256` is resolved and verified by the caller from the Local Vault authority.
    /// Re-queueing the same `(assetId, hash)` is a semantic no-op; reusing an assetId with a
    /// different hash is rejected because stable Vault asset IDs are immutable.
    pub fn queue(
        &self,
        command: BackupCommandEnvelope,
        content_sha256: &str,
    ) -> Result<BackupCommandOutcome, HostError> {
        if !matches!(command, BackupCommandEnvelope::Queue { .. }) {
            return Err(HostError::validation(
                "BackupOutbox::queue requires a backup.queue command",
            ));
        }
        self.execute_command(command, Some(content_sha256))
    }

    pub fn retry(&self, command: BackupCommandEnvelope) -> Result<BackupCommandOutcome, HostError> {
        if !matches!(command, BackupCommandEnvelope::Retry { .. }) {
            return Err(HostError::validation(
                "BackupOutbox::retry requires a backup.retry command",
            ));
        }
        self.execute_command(command, None)
    }

    pub fn cancel(
        &self,
        command: BackupCommandEnvelope,
    ) -> Result<BackupCommandOutcome, HostError> {
        if !matches!(command, BackupCommandEnvelope::Cancel { .. }) {
            return Err(HostError::validation(
                "BackupOutbox::cancel requires a backup.cancel command",
            ));
        }
        self.execute_command(command, None)
    }

    /// Executes upload-outbox lifecycle commands with durable command receipts. Restore is
    /// prepared and completed through the two-phase methods below because its remote I/O must
    /// never run while the SQLite write transaction is held.
    pub fn execute_command(
        &self,
        command: BackupCommandEnvelope,
        queue_content_sha256: Option<&str>,
    ) -> Result<BackupCommandOutcome, HostError> {
        validate_backup_command(&command, queue_content_sha256)?;
        let meta = backup_command_meta(&command);
        let command_type = backup_command_type(&command);
        let fingerprint = backup_command_fingerprint(&command, queue_content_sha256)?;

        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        if let Some(response) = find_existing_receipt(
            &transaction,
            &meta.command_id,
            &meta.idempotency_key,
            &fingerprint,
        )? {
            transaction.commit().map_err(sql_error)?;
            return Ok(BackupCommandOutcome {
                response,
                emitted_events: Vec::new(),
            });
        }

        validate_deadline(meta.deadline_at)?;
        let (backup, event_type) = match &command {
            BackupCommandEnvelope::Queue { payload, .. } => queue_asset_tx(
                &transaction,
                &payload.asset_id,
                queue_content_sha256.expect("validated queue hash"),
                meta.expected_revision,
            )?,
            BackupCommandEnvelope::Retry { payload, .. } => retry_asset_tx(
                &transaction,
                &payload.asset_id,
                meta.expected_revision.expect("validated retry revision"),
            )?,
            BackupCommandEnvelope::Cancel { payload, .. } => cancel_asset_tx(
                &transaction,
                &payload.asset_id,
                meta.expected_revision.expect("validated cancel revision"),
            )?,
            BackupCommandEnvelope::Restore { .. } => {
                return Err(HostError::new(
                    "BACKUP_RESTORE_NOT_OUTBOX_COMMAND",
                    "backup.restore requires the explicit restore workflow",
                    false,
                ));
            }
        };

        let event = event_type
            .map(|event_type| {
                append_backup_event(&transaction, event_type, &backup, &meta.context.trace_id)
            })
            .transpose()?;
        let last_event_sequence = match event.as_ref() {
            Some(event) => event.sequence,
            None => latest_event_sequence_tx(&transaction, &backup.asset_id)?,
        };
        let completed_at = now_millis();
        let response = BackupCommandResponse {
            receipt: CommandReceipt {
                command_id: meta.command_id.clone(),
                idempotency_key: meta.idempotency_key.clone(),
                command_type: command_type.to_string(),
                aggregate_id: backup.asset_id.clone(),
                revision: backup.revision,
                last_event_sequence,
                completed_at,
            },
            backup,
            replayed: false,
        };
        let response_json = serde_json::to_string(&response).map_err(json_error)?;
        transaction
            .execute(
                "INSERT INTO backup_command_receipts
                 (idempotency_key, command_id, command_type, request_fingerprint,
                  response_json, completed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    meta.idempotency_key,
                    meta.command_id,
                    command_type,
                    fingerprint,
                    response_json,
                    completed_at
                ],
            )
            .map_err(sql_error)?;
        transaction.commit().map_err(sql_error)?;

        Ok(BackupCommandOutcome {
            response,
            emitted_events: event.into_iter().collect(),
        })
    }

    /// Validates a restore command and snapshots the immutable backup manifest before network I/O.
    /// A committed receipt is returned immediately so retries never download the object twice.
    pub fn prepare_restore(
        &self,
        command: &BackupCommandEnvelope,
    ) -> Result<BackupRestorePreparation, HostError> {
        if !matches!(command, BackupCommandEnvelope::Restore { .. }) {
            return Err(HostError::validation(
                "BackupOutbox::prepare_restore requires a backup.restore command",
            ));
        }
        validate_backup_command(command, None)?;
        let meta = backup_command_meta(command);
        let fingerprint = backup_command_fingerprint(command, None)?;

        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        if let Some(response) = find_existing_receipt(
            &transaction,
            &meta.command_id,
            &meta.idempotency_key,
            &fingerprint,
        )? {
            let backup = response.backup.clone();
            transaction.commit().map_err(sql_error)?;
            return Ok(BackupRestorePreparation {
                backup,
                replayed_response: Some(response),
            });
        }

        validate_deadline(meta.deadline_at)?;
        let backup = validate_restore_target_tx(&transaction, command)?;
        transaction.commit().map_err(sql_error)?;
        Ok(BackupRestorePreparation {
            backup,
            replayed_response: None,
        })
    }

    /// Commits `backup.restored` and its command receipt after the Vault file is durable.
    /// If the process stopped after the file commit, rerunning the same command reaches this
    /// method again and safely finishes the missing SQLite commit.
    pub fn complete_restore(
        &self,
        command: BackupCommandEnvelope,
    ) -> Result<BackupCommandOutcome, HostError> {
        if !matches!(command, BackupCommandEnvelope::Restore { .. }) {
            return Err(HostError::validation(
                "BackupOutbox::complete_restore requires a backup.restore command",
            ));
        }
        validate_backup_command(&command, None)?;
        let meta = backup_command_meta(&command);
        let command_type = backup_command_type(&command);
        let fingerprint = backup_command_fingerprint(&command, None)?;

        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        if let Some(response) = find_existing_receipt(
            &transaction,
            &meta.command_id,
            &meta.idempotency_key,
            &fingerprint,
        )? {
            transaction.commit().map_err(sql_error)?;
            return Ok(BackupCommandOutcome {
                response,
                emitted_events: Vec::new(),
            });
        }

        validate_deadline(meta.deadline_at)?;
        let backup = validate_restore_target_tx(&transaction, &command)?;
        let event = append_backup_event(
            &transaction,
            BackupEventType::Restored,
            &backup,
            &meta.context.trace_id,
        )?;
        let completed_at = now_millis();
        let response = BackupCommandResponse {
            receipt: CommandReceipt {
                command_id: meta.command_id.clone(),
                idempotency_key: meta.idempotency_key.clone(),
                command_type: command_type.to_string(),
                aggregate_id: backup.asset_id.clone(),
                revision: backup.revision,
                last_event_sequence: event.sequence,
                completed_at,
            },
            backup,
            replayed: false,
        };
        let response_json = serde_json::to_string(&response).map_err(json_error)?;
        transaction
            .execute(
                "INSERT INTO backup_command_receipts
                 (idempotency_key, command_id, command_type, request_fingerprint,
                  response_json, completed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    meta.idempotency_key,
                    meta.command_id,
                    command_type,
                    fingerprint,
                    response_json,
                    completed_at
                ],
            )
            .map_err(sql_error)?;
        transaction.commit().map_err(sql_error)?;

        Ok(BackupCommandOutcome {
            response,
            emitted_events: vec![event],
        })
    }

    pub fn claim_next(&self, trace_id: &str) -> Result<BackupClaimOutcome, HostError> {
        self.claim_next_at(now_millis(), trace_id)
    }

    pub fn mark_backed_up(
        &self,
        asset_id: &str,
        content_sha256: &str,
        expected_revision: i64,
        remote_object_key: &str,
        remote_etag: Option<&str>,
        trace_id: &str,
    ) -> Result<BackupLifecycleOutcome, HostError> {
        self.mark_backed_up_at(
            asset_id,
            content_sha256,
            expected_revision,
            remote_object_key,
            remote_etag,
            trace_id,
            now_millis(),
        )
    }

    pub fn mark_failed(
        &self,
        asset_id: &str,
        content_sha256: &str,
        expected_revision: i64,
        error: &str,
        trace_id: &str,
    ) -> Result<BackupLifecycleOutcome, HostError> {
        self.mark_failed_at(
            asset_id,
            content_sha256,
            expected_revision,
            error,
            trace_id,
            now_millis(),
        )
    }

    pub fn get(&self, asset_id: &str) -> Result<Option<AssetBackupRecord>, HostError> {
        validate_asset_id(asset_id)?;
        let connection = self.lock()?;
        get_backup(&connection, asset_id)
    }

    pub fn get_exact(
        &self,
        asset_id: &str,
        content_sha256: &str,
    ) -> Result<Option<AssetBackupRecord>, HostError> {
        validate_asset_id(asset_id)?;
        validate_sha256(content_sha256)?;
        Ok(self
            .get(asset_id)?
            .filter(|backup| backup.content_sha256.eq_ignore_ascii_case(content_sha256)))
    }

    /// Finds the newest successful remote object for content that is already backed up.
    ///
    /// This is intentionally content-addressed and does not alter the existing asset-scoped
    /// backup lifecycle. Stable tie-breakers keep reuse deterministic when completion times match.
    pub fn find_latest_backed_up_by_sha256(
        &self,
        content_sha256: &str,
    ) -> Result<Option<ReusableBackedUpObject>, HostError> {
        validate_sha256(content_sha256)?;
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT asset_id, remote_object_key, remote_etag
                 FROM asset_backups
                 WHERE content_sha256 = ?1
                   AND state = 'backedUp'
                   AND backed_up_at IS NOT NULL
                   AND remote_object_key IS NOT NULL
                   AND length(trim(remote_object_key)) > 0
                 ORDER BY backed_up_at DESC, updated_at DESC, revision DESC,
                          created_at DESC, asset_id ASC
                 LIMIT 1",
                params![content_sha256.to_ascii_lowercase()],
                |row| {
                    Ok(ReusableBackedUpObject {
                        asset_id: row.get(0)?,
                        remote_object_key: row.get(1)?,
                        etag: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(sql_error)
    }

    pub fn list(&self, limit: usize) -> Result<Vec<AssetBackupRecord>, HostError> {
        validate_limit(limit)?;
        let connection = self.lock()?;
        let sql = format!(
            "SELECT {BACKUP_COLUMNS} FROM asset_backups
             ORDER BY updated_at DESC, asset_id ASC LIMIT ?1"
        );
        let mut statement = connection.prepare(&sql).map_err(sql_error)?;
        let rows = statement
            .query_map(params![limit as i64], backup_from_row)
            .map_err(sql_error)?;
        collect_rows(rows)
    }

    pub fn list_by_state(
        &self,
        state: BackupState,
        limit: usize,
    ) -> Result<Vec<AssetBackupRecord>, HostError> {
        validate_limit(limit)?;
        let connection = self.lock()?;
        let sql = format!(
            "SELECT {BACKUP_COLUMNS} FROM asset_backups
             WHERE state = ?1 ORDER BY updated_at DESC, asset_id ASC LIMIT ?2"
        );
        let mut statement = connection.prepare(&sql).map_err(sql_error)?;
        let rows = statement
            .query_map(params![state_to_db(state), limit as i64], backup_from_row)
            .map_err(sql_error)?;
        collect_rows(rows)
    }

    pub fn replay_events(
        &self,
        after_sequence: i64,
        limit: usize,
    ) -> Result<Vec<BackupDomainEvent>, HostError> {
        if after_sequence < 0 {
            return Err(HostError::validation(
                "backup event afterSequence must be non-negative",
            ));
        }
        validate_limit(limit)?;
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT sequence, event_id, event_type, asset_id, revision, occurred_at,
                        trace_id, payload_json
                 FROM backup_events WHERE sequence > ?1 ORDER BY sequence ASC LIMIT ?2",
            )
            .map_err(sql_error)?;
        let rows = statement
            .query_map(params![after_sequence, limit as i64], event_from_row)
            .map_err(sql_error)?;
        collect_rows(rows)
    }

    /// Startup recovery is safe to call repeatedly. Only Uploading rows are transitioned, so a
    /// second recovery pass is a no-op. Attempt count is preserved and the next claim fences stale
    /// worker completions through the incremented revision.
    pub fn recover_interrupted_uploads(
        &self,
        trace_id: &str,
    ) -> Result<BackupRecoveryOutcome, HostError> {
        validate_trace_id(trace_id)?;
        let now = now_millis();
        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        let sql = format!(
            "SELECT {BACKUP_COLUMNS} FROM asset_backups
             WHERE state = 'uploading' ORDER BY asset_id ASC"
        );
        let interrupted = {
            let mut statement = transaction.prepare(&sql).map_err(sql_error)?;
            let rows = statement
                .query_map([], backup_from_row)
                .map_err(sql_error)?;
            collect_rows(rows)?
        };
        let mut recovered = Vec::with_capacity(interrupted.len());
        let mut emitted_events = Vec::with_capacity(interrupted.len());
        for backup in interrupted {
            let changed = transaction
                .execute(
                    "UPDATE asset_backups
                     SET state = 'queued', next_attempt_at = ?1,
                         last_error = 'upload interrupted by application restart',
                         revision = revision + 1, updated_at = ?1
                     WHERE asset_id = ?2 AND revision = ?3 AND state = 'uploading'",
                    params![now, backup.asset_id, backup.revision],
                )
                .map_err(sql_error)?;
            ensure_changed(changed, &backup.asset_id)?;
            let recovered_backup = get_backup_tx(&transaction, &backup.asset_id)?
                .ok_or_else(|| backup_not_found(&backup.asset_id))?;
            emitted_events.push(append_backup_event(
                &transaction,
                BackupEventType::Queued,
                &recovered_backup,
                trace_id,
            )?);
            recovered.push(recovered_backup);
        }
        transaction.commit().map_err(sql_error)?;
        Ok(BackupRecoveryOutcome {
            backups: recovered,
            emitted_events,
        })
    }

    fn claim_next_at(&self, now: i64, trace_id: &str) -> Result<BackupClaimOutcome, HostError> {
        validate_trace_id(trace_id)?;
        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        let sql = format!(
            "SELECT {BACKUP_COLUMNS} FROM asset_backups
             WHERE (state = 'queued' OR state = 'failed')
               AND COALESCE(next_attempt_at, 0) <= ?1
             ORDER BY COALESCE(next_attempt_at, created_at) ASC, created_at ASC, asset_id ASC
             LIMIT 1"
        );
        let candidate = transaction
            .query_row(&sql, params![now], backup_from_row)
            .optional()
            .map_err(sql_error)?;
        let Some(candidate) = candidate else {
            transaction.commit().map_err(sql_error)?;
            return Ok(BackupClaimOutcome {
                backup: None,
                emitted_events: Vec::new(),
            });
        };
        let changed = transaction
            .execute(
                "UPDATE asset_backups
                 SET state = 'uploading', attempt_count = attempt_count + 1,
                     next_attempt_at = NULL, last_error = NULL,
                     revision = revision + 1, updated_at = ?1
                 WHERE asset_id = ?2 AND revision = ?3
                   AND (state = 'queued' OR state = 'failed')
                   AND COALESCE(next_attempt_at, 0) <= ?1",
                params![now, candidate.asset_id, candidate.revision],
            )
            .map_err(sql_error)?;
        ensure_changed(changed, &candidate.asset_id)?;
        let backup = get_backup_tx(&transaction, &candidate.asset_id)?
            .ok_or_else(|| backup_not_found(&candidate.asset_id))?;
        let event =
            append_backup_event(&transaction, BackupEventType::Uploading, &backup, trace_id)?;
        transaction.commit().map_err(sql_error)?;
        Ok(BackupClaimOutcome {
            backup: Some(backup),
            emitted_events: vec![event],
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn mark_backed_up_at(
        &self,
        asset_id: &str,
        content_sha256: &str,
        expected_revision: i64,
        remote_object_key: &str,
        remote_etag: Option<&str>,
        trace_id: &str,
        now: i64,
    ) -> Result<BackupLifecycleOutcome, HostError> {
        validate_asset_id(asset_id)?;
        validate_sha256(content_sha256)?;
        validate_expected_revision(expected_revision)?;
        validate_trace_id(trace_id)?;
        if remote_object_key.trim().is_empty() || remote_object_key.len() > 1_024 {
            return Err(HostError::validation(
                "remoteObjectKey length must be 1..1024",
            ));
        }
        if remote_etag.is_some_and(|etag| etag.trim().is_empty() || etag.len() > 512) {
            return Err(HostError::validation(
                "remoteEtag length must be 1..512 when provided",
            ));
        }

        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        let current = require_backup_tx(&transaction, asset_id)?;
        validate_hash_matches(&current, content_sha256)?;
        validate_revision(&current, expected_revision)?;
        if current.state != BackupState::Uploading {
            return Err(invalid_state(asset_id, current.state, "mark backed up"));
        }
        let changed = transaction
            .execute(
                "UPDATE asset_backups
                 SET state = 'backedUp', next_attempt_at = NULL, last_error = NULL,
                     remote_object_key = ?1, remote_etag = ?2, backed_up_at = ?3,
                     revision = revision + 1, updated_at = ?3
                 WHERE asset_id = ?4 AND revision = ?5 AND state = 'uploading'",
                params![
                    remote_object_key,
                    remote_etag,
                    now,
                    asset_id,
                    expected_revision
                ],
            )
            .map_err(sql_error)?;
        ensure_changed(changed, asset_id)?;
        let backup = require_backup_tx(&transaction, asset_id)?;
        let event =
            append_backup_event(&transaction, BackupEventType::BackedUp, &backup, trace_id)?;
        transaction.commit().map_err(sql_error)?;
        Ok(BackupLifecycleOutcome {
            backup,
            emitted_events: vec![event],
        })
    }

    fn mark_failed_at(
        &self,
        asset_id: &str,
        content_sha256: &str,
        expected_revision: i64,
        error: &str,
        trace_id: &str,
        now: i64,
    ) -> Result<BackupLifecycleOutcome, HostError> {
        validate_asset_id(asset_id)?;
        validate_sha256(content_sha256)?;
        validate_expected_revision(expected_revision)?;
        validate_trace_id(trace_id)?;
        let error = error.trim();
        if error.is_empty() || error.len() > 4_096 {
            return Err(HostError::validation(
                "backup failure error length must be 1..4096",
            ));
        }

        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        let current = require_backup_tx(&transaction, asset_id)?;
        validate_hash_matches(&current, content_sha256)?;
        validate_revision(&current, expected_revision)?;
        if current.state != BackupState::Uploading {
            return Err(invalid_state(asset_id, current.state, "mark failed"));
        }
        if current.attempt_count <= 0 {
            return Err(HostError::internal(format!(
                "uploading backup {asset_id} has invalid attemptCount {}",
                current.attempt_count
            )));
        }
        let delay = self.retry_policy.delay_for_attempt(current.attempt_count);
        let next_attempt_at = now.saturating_add(delay);
        let changed = transaction
            .execute(
                "UPDATE asset_backups
                 SET state = 'failed', next_attempt_at = ?1, last_error = ?2,
                     revision = revision + 1, updated_at = ?3
                 WHERE asset_id = ?4 AND revision = ?5 AND state = 'uploading'",
                params![next_attempt_at, error, now, asset_id, expected_revision],
            )
            .map_err(sql_error)?;
        ensure_changed(changed, asset_id)?;
        let backup = require_backup_tx(&transaction, asset_id)?;
        let event = append_backup_event(&transaction, BackupEventType::Failed, &backup, trace_id)?;
        transaction.commit().map_err(sql_error)?;
        Ok(BackupLifecycleOutcome {
            backup,
            emitted_events: vec![event],
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, HostError> {
        self.connection
            .lock()
            .map_err(|_| HostError::internal("backup outbox lock poisoned"))
    }
}

fn queue_asset_tx(
    transaction: &Transaction<'_>,
    asset_id: &str,
    content_sha256: &str,
    expected_revision: Option<i64>,
) -> Result<(AssetBackupRecord, Option<BackupEventType>), HostError> {
    if let Some(existing) = get_backup_tx(transaction, asset_id)? {
        validate_hash_matches(&existing, content_sha256)?;
        if let Some(expected_revision) = expected_revision {
            validate_revision(&existing, expected_revision)?;
        }
        return Ok((existing, None));
    }
    let now = now_millis();
    transaction
        .execute(
            "INSERT INTO asset_backups
             (asset_id, content_sha256, state, attempt_count, next_attempt_at,
              last_error, remote_object_key, remote_etag, revision, created_at,
              updated_at, backed_up_at)
             VALUES (?1, ?2, 'queued', 0, ?3, NULL, NULL, NULL, 1, ?3, ?3, NULL)",
            params![asset_id, content_sha256.to_ascii_lowercase(), now],
        )
        .map_err(sql_error)?;
    let backup = require_backup_tx(transaction, asset_id)?;
    Ok((backup, Some(BackupEventType::Queued)))
}

fn retry_asset_tx(
    transaction: &Transaction<'_>,
    asset_id: &str,
    expected_revision: i64,
) -> Result<(AssetBackupRecord, Option<BackupEventType>), HostError> {
    let current = require_backup_tx(transaction, asset_id)?;
    validate_revision(&current, expected_revision)?;
    match current.state {
        BackupState::Failed | BackupState::Cancelled | BackupState::NotScheduled => {
            let now = now_millis();
            let changed = transaction
                .execute(
                    "UPDATE asset_backups
                     SET state = 'queued', next_attempt_at = ?1, last_error = NULL,
                         revision = revision + 1, updated_at = ?1
                     WHERE asset_id = ?2 AND revision = ?3
                       AND state IN ('failed', 'cancelled', 'notScheduled')",
                    params![now, asset_id, expected_revision],
                )
                .map_err(sql_error)?;
            ensure_changed(changed, asset_id)?;
            Ok((
                require_backup_tx(transaction, asset_id)?,
                Some(BackupEventType::Queued),
            ))
        }
        BackupState::Queued => Ok((current, None)),
        BackupState::Uploading | BackupState::BackedUp => {
            Err(invalid_state(asset_id, current.state, "retry"))
        }
    }
}

fn cancel_asset_tx(
    transaction: &Transaction<'_>,
    asset_id: &str,
    expected_revision: i64,
) -> Result<(AssetBackupRecord, Option<BackupEventType>), HostError> {
    let current = require_backup_tx(transaction, asset_id)?;
    validate_revision(&current, expected_revision)?;
    match current.state {
        BackupState::Cancelled => Ok((current, None)),
        BackupState::BackedUp => Err(invalid_state(asset_id, current.state, "cancel")),
        BackupState::NotScheduled
        | BackupState::Queued
        | BackupState::Uploading
        | BackupState::Failed => {
            let now = now_millis();
            let changed = transaction
                .execute(
                    "UPDATE asset_backups
                     SET state = 'cancelled', next_attempt_at = NULL,
                         revision = revision + 1, updated_at = ?1
                     WHERE asset_id = ?2 AND revision = ?3 AND state != 'backedUp'",
                    params![now, asset_id, expected_revision],
                )
                .map_err(sql_error)?;
            ensure_changed(changed, asset_id)?;
            Ok((
                require_backup_tx(transaction, asset_id)?,
                Some(BackupEventType::Cancelled),
            ))
        }
    }
}

fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS asset_backups (
                asset_id TEXT PRIMARY KEY NOT NULL,
                content_sha256 TEXT NOT NULL,
                state TEXT NOT NULL CHECK (
                    state IN ('notScheduled', 'queued', 'uploading', 'backedUp', 'failed', 'cancelled')
                ),
                attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
                next_attempt_at INTEGER,
                last_error TEXT,
                remote_object_key TEXT,
                remote_etag TEXT,
                revision INTEGER NOT NULL CHECK (revision > 0),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                backed_up_at INTEGER,
                UNIQUE(asset_id, content_sha256)
            );

            CREATE INDEX IF NOT EXISTS idx_asset_backups_claim
                ON asset_backups(state, next_attempt_at, created_at, asset_id);
            CREATE INDEX IF NOT EXISTS idx_asset_backups_hash
                ON asset_backups(content_sha256);

            CREATE TABLE IF NOT EXISTS backup_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL,
                asset_id TEXT NOT NULL,
                revision INTEGER NOT NULL,
                occurred_at INTEGER NOT NULL,
                trace_id TEXT NOT NULL,
                payload_json TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_backup_events_asset_sequence
                ON backup_events(asset_id, sequence);

            CREATE TABLE IF NOT EXISTS backup_command_receipts (
                idempotency_key TEXT PRIMARY KEY NOT NULL,
                command_id TEXT NOT NULL UNIQUE,
                command_type TEXT NOT NULL,
                request_fingerprint TEXT NOT NULL,
                response_json TEXT NOT NULL,
                completed_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_backup_command_receipts_completed
                ON backup_command_receipts(completed_at);
            "#,
        )
        .map_err(sql_error)
}

fn find_existing_receipt(
    transaction: &Transaction<'_>,
    command_id: &str,
    idempotency_key: &str,
    request_fingerprint: &str,
) -> Result<Option<BackupCommandResponse>, HostError> {
    let by_key = load_receipt_by_key(transaction, idempotency_key)?;
    let by_command = load_receipt_by_command(transaction, command_id)?;

    if let Some(receipt) = &by_key {
        if receipt.command_id != command_id || receipt.request_fingerprint != request_fingerprint {
            return Err(HostError::new(
                "IDEMPOTENCY_KEY_REUSED",
                "backup idempotencyKey was already committed for a different command",
                false,
            ));
        }
    }
    if let Some(receipt) = &by_command {
        if receipt.idempotency_key != idempotency_key
            || receipt.request_fingerprint != request_fingerprint
        {
            return Err(HostError::new(
                "COMMAND_ID_REUSED",
                "backup commandId was already committed for a different command",
                false,
            ));
        }
    }
    if let (Some(by_key), Some(by_command)) = (&by_key, &by_command) {
        if by_key.command_id != by_command.command_id
            || by_key.idempotency_key != by_command.idempotency_key
        {
            return Err(HostError::new(
                "COMMAND_IDENTITY_COLLISION",
                "backup commandId and idempotencyKey identify different committed commands",
                false,
            ));
        }
    }
    by_key
        .or(by_command)
        .map(|receipt| {
            let mut response: BackupCommandResponse =
                serde_json::from_str(&receipt.response_json).map_err(json_error)?;
            response.replayed = true;
            Ok(response)
        })
        .transpose()
}

fn load_receipt_by_key(
    transaction: &Transaction<'_>,
    idempotency_key: &str,
) -> Result<Option<StoredReceipt>, HostError> {
    transaction
        .query_row(
            "SELECT command_id, idempotency_key, request_fingerprint, response_json
             FROM backup_command_receipts WHERE idempotency_key = ?1",
            params![idempotency_key],
            receipt_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn load_receipt_by_command(
    transaction: &Transaction<'_>,
    command_id: &str,
) -> Result<Option<StoredReceipt>, HostError> {
    transaction
        .query_row(
            "SELECT command_id, idempotency_key, request_fingerprint, response_json
             FROM backup_command_receipts WHERE command_id = ?1",
            params![command_id],
            receipt_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn receipt_from_row(row: &Row<'_>) -> rusqlite::Result<StoredReceipt> {
    Ok(StoredReceipt {
        command_id: row.get(0)?,
        idempotency_key: row.get(1)?,
        request_fingerprint: row.get(2)?,
        response_json: row.get(3)?,
    })
}

fn append_backup_event(
    transaction: &Transaction<'_>,
    event_type: BackupEventType,
    backup: &AssetBackupRecord,
    trace_id: &str,
) -> Result<BackupDomainEvent, HostError> {
    let event_id = Uuid::new_v4().to_string();
    let occurred_at = now_millis();
    let payload_json = serde_json::to_string(backup).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO backup_events
             (event_id, event_type, asset_id, revision, occurred_at, trace_id, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event_id,
                event_type_to_db(&event_type),
                backup.asset_id,
                backup.revision,
                occurred_at,
                trace_id,
                payload_json
            ],
        )
        .map_err(sql_error)?;
    Ok(BackupDomainEvent {
        sequence: transaction.last_insert_rowid(),
        event_id,
        event_type,
        asset_id: backup.asset_id.clone(),
        revision: backup.revision,
        occurred_at,
        trace_id: trace_id.to_string(),
        backup: backup.clone(),
    })
}

fn latest_event_sequence_tx(
    transaction: &Transaction<'_>,
    asset_id: &str,
) -> Result<i64, HostError> {
    transaction
        .query_row(
            "SELECT COALESCE(MAX(sequence), 0) FROM backup_events WHERE asset_id = ?1",
            params![asset_id],
            |row| row.get(0),
        )
        .map_err(sql_error)
}

fn get_backup(
    connection: &Connection,
    asset_id: &str,
) -> Result<Option<AssetBackupRecord>, HostError> {
    let sql = format!("SELECT {BACKUP_COLUMNS} FROM asset_backups WHERE asset_id = ?1");
    connection
        .query_row(&sql, params![asset_id], backup_from_row)
        .optional()
        .map_err(sql_error)
}

fn get_backup_tx(
    transaction: &Transaction<'_>,
    asset_id: &str,
) -> Result<Option<AssetBackupRecord>, HostError> {
    let sql = format!("SELECT {BACKUP_COLUMNS} FROM asset_backups WHERE asset_id = ?1");
    transaction
        .query_row(&sql, params![asset_id], backup_from_row)
        .optional()
        .map_err(sql_error)
}

fn require_backup_tx(
    transaction: &Transaction<'_>,
    asset_id: &str,
) -> Result<AssetBackupRecord, HostError> {
    get_backup_tx(transaction, asset_id)?.ok_or_else(|| backup_not_found(asset_id))
}

fn validate_restore_target_tx(
    transaction: &Transaction<'_>,
    command: &BackupCommandEnvelope,
) -> Result<AssetBackupRecord, HostError> {
    let BackupCommandEnvelope::Restore {
        payload,
        expected_revision,
        ..
    } = command
    else {
        return Err(HostError::validation(
            "restore validation requires a backup.restore command",
        ));
    };
    let backup = require_backup_tx(transaction, &payload.asset_id)?;
    if Some(backup.revision) != *expected_revision {
        return Err(HostError::conflict(format!(
            "backup {} revision changed from {} to {}",
            payload.asset_id,
            expected_revision.unwrap_or_default(),
            backup.revision
        )));
    }
    if backup.state != BackupState::BackedUp {
        return Err(HostError::new(
            "BACKUP_NOT_RESTORABLE",
            format!(
                "backup {} must be backedUp before restore; current state is {}",
                payload.asset_id,
                state_to_db(backup.state)
            ),
            false,
        ));
    }
    if !backup
        .content_sha256
        .eq_ignore_ascii_case(&payload.expected_sha256)
    {
        return Err(HostError::new(
            "BACKUP_RESTORE_HASH_MISMATCH",
            format!(
                "expectedSha256 does not match the backed-up manifest for asset {}",
                payload.asset_id
            ),
            false,
        ));
    }
    if backup
        .remote_object_key
        .as_deref()
        .is_none_or(|key| key.trim().is_empty())
    {
        return Err(HostError::new(
            "BACKUP_REMOTE_IDENTITY_MISSING",
            format!(
                "backup {} has no durable remote object identity",
                payload.asset_id
            ),
            false,
        ));
    }
    Ok(backup)
}

fn backup_from_row(row: &Row<'_>) -> rusqlite::Result<AssetBackupRecord> {
    let state: String = row.get(2)?;
    let state = state_from_db(&state).map_err(|message| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                message,
            )),
        )
    })?;
    Ok(AssetBackupRecord {
        asset_id: row.get(0)?,
        content_sha256: row.get(1)?,
        state,
        attempt_count: row.get(3)?,
        next_attempt_at: row.get(4)?,
        last_error: row.get(5)?,
        remote_object_key: row.get(6)?,
        remote_etag: row.get(7)?,
        revision: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        backed_up_at: row.get(11)?,
    })
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<BackupDomainEvent> {
    let event_type: String = row.get(2)?;
    let event_type = event_type_from_db(&event_type).map_err(|message| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                message,
            )),
        )
    })?;
    let payload_json: String = row.get(7)?;
    let backup: AssetBackupRecord = serde_json::from_str(&payload_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(BackupDomainEvent {
        sequence: row.get(0)?,
        event_id: row.get(1)?,
        event_type,
        asset_id: row.get(3)?,
        revision: row.get(4)?,
        occurred_at: row.get(5)?,
        trace_id: row.get(6)?,
        backup,
    })
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>, HostError> {
    rows.collect::<Result<Vec<_>, _>>().map_err(sql_error)
}

fn validate_backup_command(
    command: &BackupCommandEnvelope,
    queue_content_sha256: Option<&str>,
) -> Result<(), HostError> {
    let meta = backup_command_meta(command);
    if meta.protocol_version != BACKUP_PROTOCOL_VERSION {
        return Err(HostError::new(
            "PROTOCOL_VERSION_UNSUPPORTED",
            format!(
                "backup command protocol {} is unsupported; expected {}",
                meta.protocol_version, BACKUP_PROTOCOL_VERSION
            ),
            false,
        ));
    }
    Uuid::parse_str(&meta.command_id)
        .map_err(|_| HostError::validation("commandId must be a UUID"))?;
    if meta.idempotency_key.trim().len() < 8 || meta.idempotency_key.len() > 160 {
        return Err(HostError::validation(
            "idempotencyKey length must be 8..160",
        ));
    }
    validate_trace_id(&meta.context.trace_id)?;
    if meta.context.actor_id.trim().is_empty() || meta.context.window_id.trim().is_empty() {
        return Err(HostError::validation(
            "actorId, windowId, and traceId are required",
        ));
    }

    match command {
        BackupCommandEnvelope::Queue {
            payload,
            expected_revision,
            ..
        } => {
            validate_asset_id(&payload.asset_id)?;
            let hash = queue_content_sha256.ok_or_else(|| {
                HostError::validation("backup.queue requires resolved Local Vault contentSha256")
            })?;
            validate_sha256(hash)?;
            if expected_revision.is_some() {
                return Err(HostError::validation(
                    "backup.queue rejects expectedRevision",
                ));
            }
        }
        BackupCommandEnvelope::Retry {
            payload,
            expected_revision,
            ..
        } => {
            validate_asset_id(&payload.asset_id)?;
            validate_retry_or_cancel_revision(*expected_revision, queue_content_sha256)?;
        }
        BackupCommandEnvelope::Cancel {
            payload,
            expected_revision,
            ..
        } => {
            validate_asset_id(&payload.asset_id)?;
            validate_retry_or_cancel_revision(*expected_revision, queue_content_sha256)?;
        }
        BackupCommandEnvelope::Restore {
            payload,
            expected_revision,
            ..
        } => {
            validate_asset_id(&payload.asset_id)?;
            validate_sha256(&payload.expected_sha256)?;
            if expected_revision.unwrap_or(0) <= 0 {
                return Err(HostError::validation(
                    "backup.restore requires expectedRevision > 0",
                ));
            }
            if queue_content_sha256.is_some() {
                return Err(HostError::validation(
                    "contentSha256 is accepted only for backup.queue",
                ));
            }
        }
    }
    Ok(())
}

fn validate_retry_or_cancel_revision(
    expected_revision: Option<i64>,
    queue_content_sha256: Option<&str>,
) -> Result<(), HostError> {
    if expected_revision.unwrap_or(0) <= 0 {
        return Err(HostError::validation(
            "backup.retry and backup.cancel require expectedRevision > 0",
        ));
    }
    if queue_content_sha256.is_some() {
        return Err(HostError::validation(
            "contentSha256 is accepted only for backup.queue",
        ));
    }
    Ok(())
}

fn validate_deadline(deadline_at: Option<i64>) -> Result<(), HostError> {
    if deadline_at.is_some_and(|deadline| deadline < now_millis()) {
        return Err(HostError::new(
            "COMMAND_DEADLINE_EXCEEDED",
            "backup command deadline has elapsed",
            false,
        ));
    }
    Ok(())
}

fn validate_asset_id(asset_id: &str) -> Result<(), HostError> {
    let length = asset_id.trim().chars().count();
    if length == 0 || length > 240 {
        return Err(HostError::validation("assetId length must be 1..240"));
    }
    Ok(())
}

fn validate_sha256(content_sha256: &str) -> Result<(), HostError> {
    if content_sha256.len() != 64 || !content_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(HostError::validation(
            "contentSha256 must be 64 hexadecimal characters",
        ));
    }
    Ok(())
}

fn validate_trace_id(trace_id: &str) -> Result<(), HostError> {
    let length = trace_id.trim().chars().count();
    if length == 0 || length > 160 {
        return Err(HostError::validation("traceId length must be 1..160"));
    }
    Ok(())
}

fn validate_expected_revision(expected_revision: i64) -> Result<(), HostError> {
    if expected_revision <= 0 {
        return Err(HostError::validation(
            "expectedRevision must be greater than zero",
        ));
    }
    Ok(())
}

fn validate_limit(limit: usize) -> Result<(), HostError> {
    if limit == 0 || limit > MAX_PAGE_SIZE {
        return Err(HostError::validation(format!(
            "backup list limit must be 1..{MAX_PAGE_SIZE}"
        )));
    }
    Ok(())
}

fn validate_hash_matches(
    backup: &AssetBackupRecord,
    content_sha256: &str,
) -> Result<(), HostError> {
    if backup.content_sha256.eq_ignore_ascii_case(content_sha256) {
        Ok(())
    } else {
        Err(HostError::new(
            "BACKUP_ASSET_HASH_CONFLICT",
            format!(
                "asset {} is already registered with a different content hash",
                backup.asset_id
            ),
            false,
        ))
    }
}

fn validate_revision(backup: &AssetBackupRecord, expected_revision: i64) -> Result<(), HostError> {
    if backup.revision == expected_revision {
        Ok(())
    } else {
        Err(HostError::new(
            "BACKUP_REVISION_CONFLICT",
            format!(
                "backup {} revision {} does not match expected {}",
                backup.asset_id, backup.revision, expected_revision
            ),
            false,
        ))
    }
}

fn ensure_changed(changed: usize, asset_id: &str) -> Result<(), HostError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(HostError::new(
            "BACKUP_REVISION_CONFLICT",
            format!("backup {asset_id} changed during transaction"),
            false,
        ))
    }
}

fn backup_not_found(asset_id: &str) -> HostError {
    HostError::new(
        "BACKUP_NOT_FOUND",
        format!("backup for asset {asset_id} was not found"),
        false,
    )
}

fn invalid_state(asset_id: &str, state: BackupState, operation: &str) -> HostError {
    HostError::new(
        "BACKUP_STATE_TRANSITION_INVALID",
        format!(
            "cannot {operation} backup {asset_id} while state is {}",
            state_to_db(state)
        ),
        false,
    )
}

fn backup_command_meta(command: &BackupCommandEnvelope) -> BackupCommandMeta {
    match command {
        BackupCommandEnvelope::Queue {
            command_id,
            protocol_version,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        }
        | BackupCommandEnvelope::Retry {
            command_id,
            protocol_version,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        }
        | BackupCommandEnvelope::Cancel {
            command_id,
            protocol_version,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        }
        | BackupCommandEnvelope::Restore {
            command_id,
            protocol_version,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => BackupCommandMeta {
            command_id: command_id.clone(),
            protocol_version: protocol_version.clone(),
            context: context.clone(),
            idempotency_key: idempotency_key.clone(),
            expected_revision: *expected_revision,
            deadline_at: *deadline_at,
        },
    }
}

fn backup_command_type(command: &BackupCommandEnvelope) -> &'static str {
    match command {
        BackupCommandEnvelope::Queue { .. } => "backup.queue",
        BackupCommandEnvelope::Retry { .. } => "backup.retry",
        BackupCommandEnvelope::Cancel { .. } => "backup.cancel",
        BackupCommandEnvelope::Restore { .. } => "backup.restore",
    }
}

fn backup_command_fingerprint(
    command: &BackupCommandEnvelope,
    queue_content_sha256: Option<&str>,
) -> Result<String, HostError> {
    let meta = backup_command_meta(command);
    let context = serde_json::json!({
        "actorId": meta.context.actor_id,
        "accountId": meta.context.account_id,
        "projectId": meta.context.project_id,
    });
    let value = match command {
        BackupCommandEnvelope::Queue { payload, .. } => serde_json::json!({
            "commandType": backup_command_type(command),
            "context": context,
            "expectedRevision": meta.expected_revision,
            "payload": payload,
            "contentSha256": queue_content_sha256,
        }),
        BackupCommandEnvelope::Retry { payload, .. } => serde_json::json!({
            "commandType": backup_command_type(command),
            "context": context,
            "expectedRevision": meta.expected_revision,
            "payload": payload,
        }),
        BackupCommandEnvelope::Cancel { payload, .. } => serde_json::json!({
            "commandType": backup_command_type(command),
            "context": context,
            "expectedRevision": meta.expected_revision,
            "payload": payload,
        }),
        BackupCommandEnvelope::Restore { payload, .. } => serde_json::json!({
            "commandType": backup_command_type(command),
            "context": context,
            "expectedRevision": meta.expected_revision,
            "payload": payload,
        }),
    };
    serde_json::to_string(&value).map_err(json_error)
}

fn state_to_db(state: BackupState) -> &'static str {
    match state {
        BackupState::NotScheduled => "notScheduled",
        BackupState::Queued => "queued",
        BackupState::Uploading => "uploading",
        BackupState::BackedUp => "backedUp",
        BackupState::Failed => "failed",
        BackupState::Cancelled => "cancelled",
    }
}

fn state_from_db(value: &str) -> Result<BackupState, String> {
    match value {
        "notScheduled" => Ok(BackupState::NotScheduled),
        "queued" => Ok(BackupState::Queued),
        "uploading" => Ok(BackupState::Uploading),
        "backedUp" => Ok(BackupState::BackedUp),
        "failed" => Ok(BackupState::Failed),
        "cancelled" => Ok(BackupState::Cancelled),
        _ => Err(format!("unknown backup state: {value}")),
    }
}

fn event_type_to_db(event_type: &BackupEventType) -> &'static str {
    match event_type {
        BackupEventType::Queued => "backup.queued",
        BackupEventType::Uploading => "backup.uploading",
        BackupEventType::BackedUp => "backup.backedUp",
        BackupEventType::Failed => "backup.failed",
        BackupEventType::Cancelled => "backup.cancelled",
        BackupEventType::Restored => "backup.restored",
    }
}

fn event_type_from_db(value: &str) -> Result<BackupEventType, String> {
    match value {
        "backup.queued" => Ok(BackupEventType::Queued),
        "backup.uploading" => Ok(BackupEventType::Uploading),
        "backup.backedUp" => Ok(BackupEventType::BackedUp),
        "backup.failed" => Ok(BackupEventType::Failed),
        "backup.cancelled" => Ok(BackupEventType::Cancelled),
        "backup.restored" => Ok(BackupEventType::Restored),
        _ => Err(format!("unknown backup event type: {value}")),
    }
}

fn immediate(connection: &mut Connection) -> Result<Transaction<'_>, HostError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn sql_error(error: rusqlite::Error) -> HostError {
    HostError::internal(format!("backup outbox sqlite error: {error}"))
}

fn json_error(error: serde_json::Error) -> HostError {
    HostError::internal(format!("backup outbox json error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        CancelAssetBackupPayload, QueueAssetBackupPayload, RestoreAssetBackupPayload,
        RetryAssetBackupPayload,
    };
    use tempfile::tempdir;

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn context(trace_id: &str) -> OperationContext {
        OperationContext {
            actor_id: "tester".to_string(),
            account_id: Some("account-1".to_string()),
            project_id: Some("project-1".to_string()),
            window_id: "window-1".to_string(),
            trace_id: trace_id.to_string(),
        }
    }

    fn queue_command(asset_id: &str, key: &str) -> BackupCommandEnvelope {
        BackupCommandEnvelope::Queue {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
            context: context("trace:queue"),
            payload: QueueAssetBackupPayload {
                asset_id: asset_id.to_string(),
            },
            idempotency_key: key.to_string(),
            expected_revision: None,
            deadline_at: None,
        }
    }

    fn retry_command(asset_id: &str, revision: i64, key: &str) -> BackupCommandEnvelope {
        BackupCommandEnvelope::Retry {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
            context: context("trace:retry"),
            payload: RetryAssetBackupPayload {
                asset_id: asset_id.to_string(),
            },
            idempotency_key: key.to_string(),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }

    fn cancel_command(asset_id: &str, revision: i64, key: &str) -> BackupCommandEnvelope {
        BackupCommandEnvelope::Cancel {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
            context: context("trace:cancel"),
            payload: CancelAssetBackupPayload {
                asset_id: asset_id.to_string(),
            },
            idempotency_key: key.to_string(),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }

    fn restore_command(
        asset_id: &str,
        sha256: &str,
        revision: i64,
        key: &str,
    ) -> BackupCommandEnvelope {
        BackupCommandEnvelope::Restore {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
            context: context("trace:restore"),
            payload: RestoreAssetBackupPayload {
                asset_id: asset_id.to_string(),
                expected_sha256: sha256.to_string(),
            },
            idempotency_key: key.to_string(),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }
    fn in_memory_outbox() -> BackupOutbox {
        BackupOutbox::from_connection_with_retry_policy(
            Connection::open_in_memory().unwrap(),
            BackupRetryPolicy {
                base_backoff_millis: 100,
                max_backoff_millis: 800,
            },
        )
        .unwrap()
    }

    fn complete_backup_at(
        outbox: &BackupOutbox,
        asset_id: &str,
        sha256: &str,
        remote_object_key: &str,
        etag: Option<&str>,
        backed_up_at: i64,
    ) {
        outbox
            .queue(
                queue_command(asset_id, &format!("queue-key-{asset_id}")),
                sha256,
            )
            .unwrap();
        let claimed = outbox
            .claim_next_at(i64::MAX / 4, &format!("trace:claim:{asset_id}"))
            .unwrap()
            .backup
            .unwrap();
        assert_eq!(claimed.asset_id, asset_id);
        outbox
            .mark_backed_up_at(
                asset_id,
                sha256,
                claimed.revision,
                remote_object_key,
                etag,
                &format!("trace:backed-up:{asset_id}"),
                backed_up_at,
            )
            .unwrap();
    }

    #[test]
    fn reusable_backed_up_object_selects_deterministic_latest_success_for_sha256() {
        let outbox = in_memory_outbox();
        complete_backup_at(
            &outbox,
            "asset-old",
            HASH_A,
            "backup/hash-a/old",
            Some("etag-old"),
            1_000,
        );
        complete_backup_at(
            &outbox,
            "asset-new-z",
            HASH_A,
            "backup/hash-a/new-z",
            Some("etag-new-z"),
            2_000,
        );
        complete_backup_at(
            &outbox,
            "asset-new-a",
            HASH_A,
            "backup/hash-a/new-a",
            Some("etag-new-a"),
            2_000,
        );

        assert_eq!(
            outbox.find_latest_backed_up_by_sha256(HASH_A).unwrap(),
            Some(ReusableBackedUpObject {
                asset_id: "asset-new-a".to_string(),
                remote_object_key: "backup/hash-a/new-a".to_string(),
                etag: Some("etag-new-a".to_string()),
            })
        );
    }

    #[test]
    fn reusable_backed_up_object_ignores_non_successful_records() {
        let outbox = in_memory_outbox();
        outbox
            .queue(queue_command("asset-failed", "queue-key-failed"), HASH_A)
            .unwrap();
        let claimed = outbox
            .claim_next_at(i64::MAX / 4, "trace:claim:failed")
            .unwrap()
            .backup
            .unwrap();
        outbox
            .mark_failed_at(
                "asset-failed",
                HASH_A,
                claimed.revision,
                "upload failed",
                "trace:failed",
                1_000,
            )
            .unwrap();

        assert_eq!(
            outbox.find_latest_backed_up_by_sha256(HASH_A).unwrap(),
            None
        );
    }

    #[test]
    fn reusable_backed_up_object_does_not_cross_sha256_values() {
        let outbox = in_memory_outbox();
        complete_backup_at(
            &outbox,
            "asset-hash-b",
            HASH_B,
            "backup/hash-b/object",
            Some("etag-hash-b"),
            1_000,
        );

        assert_eq!(
            outbox.find_latest_backed_up_by_sha256(HASH_A).unwrap(),
            None
        );
        assert_eq!(
            outbox.find_latest_backed_up_by_sha256(HASH_B).unwrap(),
            Some(ReusableBackedUpObject {
                asset_id: "asset-hash-b".to_string(),
                remote_object_key: "backup/hash-b/object".to_string(),
                etag: Some("etag-hash-b".to_string()),
            })
        );
    }

    #[test]
    fn queue_is_deduplicated_by_asset_and_hash_and_receipts_replay() {
        let outbox = in_memory_outbox();
        let command = queue_command("asset-1", "queue-key-0001");
        let first = outbox.queue(command.clone(), HASH_A).unwrap();
        assert_eq!(first.response.backup.state, BackupState::Queued);
        assert_eq!(first.response.backup.revision, 1);
        assert_eq!(first.emitted_events.len(), 1);

        let replayed = outbox.queue(command, HASH_A).unwrap();
        assert!(replayed.response.replayed);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(replayed.response.receipt, first.response.receipt);

        let duplicate = outbox
            .queue(queue_command("asset-1", "queue-key-0002"), HASH_A)
            .unwrap();
        assert!(!duplicate.response.replayed);
        assert_eq!(duplicate.response.backup.revision, 1);
        assert!(duplicate.emitted_events.is_empty());
        assert_eq!(outbox.list(10).unwrap().len(), 1);
        assert_eq!(outbox.replay_events(0, 10).unwrap().len(), 1);

        let error = outbox
            .queue(queue_command("asset-1", "queue-key-0003"), HASH_B)
            .unwrap_err();
        assert_eq!(error.code, "BACKUP_ASSET_HASH_CONFLICT");
        assert_eq!(outbox.list(10).unwrap().len(), 1);
    }

    #[test]
    fn failed_upload_uses_exponential_backoff_before_becoming_claimable() {
        let outbox = in_memory_outbox();
        outbox
            .queue(queue_command("asset-retry", "queue-key-retry"), HASH_A)
            .unwrap();

        let first_claim = outbox
            .claim_next_at(i64::MAX / 4, "trace:claim-1")
            .unwrap()
            .backup
            .unwrap();
        assert_eq!(first_claim.attempt_count, 1);
        let first_failure = outbox
            .mark_failed_at(
                "asset-retry",
                HASH_A,
                first_claim.revision,
                "network unavailable",
                "trace:failed-1",
                1_000,
            )
            .unwrap()
            .backup;
        assert_eq!(first_failure.state, BackupState::Failed);
        assert_eq!(first_failure.next_attempt_at, Some(1_100));
        assert!(outbox
            .claim_next_at(1_099, "trace:not-due")
            .unwrap()
            .backup
            .is_none());

        let second_claim = outbox
            .claim_next_at(1_100, "trace:claim-2")
            .unwrap()
            .backup
            .unwrap();
        assert_eq!(second_claim.attempt_count, 2);
        let second_failure = outbox
            .mark_failed_at(
                "asset-retry",
                HASH_A,
                second_claim.revision,
                "still unavailable",
                "trace:failed-2",
                2_000,
            )
            .unwrap()
            .backup;
        assert_eq!(second_failure.next_attempt_at, Some(2_200));
    }

    #[test]
    fn startup_recovery_requeues_uploading_records_and_fences_stale_workers() {
        let directory = tempdir().unwrap();
        let database_path = directory.path().join("backup.sqlite3");
        let stale_revision;
        {
            let outbox = BackupOutbox::open_with_retry_policy(
                &database_path,
                BackupRetryPolicy {
                    base_backoff_millis: 100,
                    max_backoff_millis: 800,
                },
            )
            .unwrap();
            outbox
                .queue(queue_command("asset-restart", "queue-key-restart"), HASH_A)
                .unwrap();
            let claimed = outbox
                .claim_next_at(i64::MAX / 4, "trace:before-restart")
                .unwrap()
                .backup
                .unwrap();
            stale_revision = claimed.revision;
            assert_eq!(claimed.state, BackupState::Uploading);
        }

        let reopened = BackupOutbox::open_with_retry_policy(
            &database_path,
            BackupRetryPolicy {
                base_backoff_millis: 100,
                max_backoff_millis: 800,
            },
        )
        .unwrap();
        let recovered = reopened.get("asset-restart").unwrap().unwrap();
        assert_eq!(recovered.state, BackupState::Queued);
        assert_eq!(recovered.attempt_count, 1);
        assert_eq!(recovered.revision, stale_revision + 1);
        assert_eq!(
            recovered.last_error.as_deref(),
            Some("upload interrupted by application restart")
        );
        let stale_error = reopened
            .mark_backed_up(
                "asset-restart",
                HASH_A,
                stale_revision,
                "vault/asset-restart",
                Some("etag-stale"),
                "trace:stale-worker",
            )
            .unwrap_err();
        assert_eq!(stale_error.code, "BACKUP_REVISION_CONFLICT");

        let events = reopened.replay_events(0, 10).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, BackupEventType::Queued);
        assert_eq!(events[1].event_type, BackupEventType::Uploading);
        assert_eq!(events[2].event_type, BackupEventType::Queued);
    }

    #[test]
    fn cancellation_is_durable_idempotent_and_not_claimable() {
        let outbox = in_memory_outbox();
        let queued = outbox
            .queue(queue_command("asset-cancel", "queue-key-cancel"), HASH_A)
            .unwrap()
            .response
            .backup;
        let command = cancel_command("asset-cancel", queued.revision, "cancel-key-0001");
        let cancelled = outbox.cancel(command.clone()).unwrap();
        assert_eq!(cancelled.response.backup.state, BackupState::Cancelled);
        assert_eq!(cancelled.emitted_events.len(), 1);
        assert!(outbox
            .claim_next_at(i64::MAX / 4, "trace:after-cancel")
            .unwrap()
            .backup
            .is_none());

        let replayed = outbox.cancel(command).unwrap();
        assert!(replayed.response.replayed);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(replayed.response.receipt, cancelled.response.receipt);

        let retried = outbox
            .retry(retry_command(
                "asset-cancel",
                cancelled.response.backup.revision,
                "retry-key-cancel",
            ))
            .unwrap();
        assert_eq!(retried.response.backup.state, BackupState::Queued);
    }

    #[test]
    fn restore_receipt_and_event_are_durable_idempotent_without_mutating_backup_state() {
        let outbox = in_memory_outbox();
        outbox
            .queue(queue_command("asset-restore", "queue-key-restore"), HASH_A)
            .unwrap();
        let claimed = outbox
            .claim_next_at(i64::MAX / 4, "trace:restore-claim")
            .unwrap()
            .backup
            .unwrap();
        let backed_up = outbox
            .mark_backed_up(
                "asset-restore",
                HASH_A,
                claimed.revision,
                "backup/v1/assets/aa/asset-restore/hash-a",
                Some("etag-restore"),
                "trace:restore-backed-up",
            )
            .unwrap()
            .backup;
        let command = restore_command(
            "asset-restore",
            HASH_A,
            backed_up.revision,
            "restore-key-0001",
        );

        let prepared = outbox.prepare_restore(&command).unwrap();
        assert!(prepared.replayed_response.is_none());
        assert_eq!(prepared.backup, backed_up);
        let completed = outbox.complete_restore(command.clone()).unwrap();
        assert!(!completed.response.replayed);
        assert_eq!(completed.response.backup.state, BackupState::BackedUp);
        assert_eq!(completed.response.backup.revision, backed_up.revision);
        assert_eq!(completed.emitted_events.len(), 1);
        assert_eq!(
            completed.emitted_events[0].event_type,
            BackupEventType::Restored
        );

        let replayed_preparation = outbox.prepare_restore(&command).unwrap();
        let replayed = replayed_preparation.replayed_response.unwrap();
        assert!(replayed.replayed);
        assert_eq!(replayed.receipt, completed.response.receipt);
        let replayed_completion = outbox.complete_restore(command).unwrap();
        assert!(replayed_completion.response.replayed);
        assert!(replayed_completion.emitted_events.is_empty());
        assert_eq!(
            outbox
                .replay_events(0, 100)
                .unwrap()
                .iter()
                .filter(|event| event.event_type == BackupEventType::Restored)
                .count(),
            1
        );
    }

    #[test]
    fn restore_requires_backed_up_state_matching_revision_and_hash() {
        let outbox = in_memory_outbox();
        let queued = outbox
            .queue(
                queue_command("asset-restore-guard", "queue-key-guard"),
                HASH_A,
            )
            .unwrap()
            .response
            .backup;
        let queued_command = restore_command(
            "asset-restore-guard",
            HASH_A,
            queued.revision,
            "restore-queued-0001",
        );
        assert_eq!(
            outbox.prepare_restore(&queued_command).unwrap_err().code,
            "BACKUP_NOT_RESTORABLE"
        );

        let claimed = outbox
            .claim_next_at(i64::MAX / 4, "trace:restore-guard-claim")
            .unwrap()
            .backup
            .unwrap();
        let backed_up = outbox
            .mark_backed_up(
                "asset-restore-guard",
                HASH_A,
                claimed.revision,
                "backup/v1/assets/aa/asset-restore-guard/hash-a",
                Some("etag-guard"),
                "trace:restore-guard-backed-up",
            )
            .unwrap()
            .backup;
        let stale_revision = restore_command(
            "asset-restore-guard",
            HASH_A,
            backed_up.revision - 1,
            "restore-stale-0001",
        );
        assert_eq!(
            outbox.prepare_restore(&stale_revision).unwrap_err().code,
            "REVISION_CONFLICT"
        );
        let wrong_hash = restore_command(
            "asset-restore-guard",
            HASH_B,
            backed_up.revision,
            "restore-hash-0001",
        );
        assert_eq!(
            outbox.prepare_restore(&wrong_hash).unwrap_err().code,
            "BACKUP_RESTORE_HASH_MISMATCH"
        );
    }
    #[test]
    fn events_are_strictly_ordered_across_queue_failure_retry_and_success() {
        let outbox = in_memory_outbox();
        let queued = outbox
            .queue(queue_command("asset-events", "queue-key-events"), HASH_A)
            .unwrap()
            .response
            .backup;
        assert_eq!(queued.revision, 1);
        let first_claim = outbox
            .claim_next_at(i64::MAX / 4, "trace:event-claim-1")
            .unwrap()
            .backup
            .unwrap();
        let failed = outbox
            .mark_failed_at(
                "asset-events",
                HASH_A,
                first_claim.revision,
                "temporary failure",
                "trace:event-failed",
                10_000,
            )
            .unwrap()
            .backup;
        let retried = outbox
            .retry(retry_command(
                "asset-events",
                failed.revision,
                "retry-key-events",
            ))
            .unwrap()
            .response
            .backup;
        let second_claim = outbox
            .claim_next_at(i64::MAX / 4, "trace:event-claim-2")
            .unwrap()
            .backup
            .unwrap();
        let backed_up = outbox
            .mark_backed_up_at(
                "asset-events",
                HASH_A,
                second_claim.revision,
                "vault/project/asset-events",
                Some("etag-1"),
                "trace:event-success",
                20_000,
            )
            .unwrap()
            .backup;
        assert_eq!(retried.state, BackupState::Queued);
        assert_eq!(backed_up.state, BackupState::BackedUp);

        let events = outbox.replay_events(0, 20).unwrap();
        assert_eq!(events.len(), 6);
        assert_eq!(
            events
                .iter()
                .map(|event| &event.event_type)
                .collect::<Vec<_>>(),
            vec![
                &BackupEventType::Queued,
                &BackupEventType::Uploading,
                &BackupEventType::Failed,
                &BackupEventType::Queued,
                &BackupEventType::Uploading,
                &BackupEventType::BackedUp,
            ]
        );
        assert!(events
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence));
        assert_eq!(
            events
                .iter()
                .map(|event| event.revision)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5, 6]
        );
        assert!(events.iter().all(|event| {
            event.asset_id == "asset-events"
                && event.revision == event.backup.revision
                && event.sequence > 0
        }));
    }
}
