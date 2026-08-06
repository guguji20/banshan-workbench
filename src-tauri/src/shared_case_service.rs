use crate::asset_service;
use crate::backup_outbox::BackupOutbox;
use crate::protocol::{
    BackupCommandEnvelope, CommandReceipt, HostError, OperationContext, QueueAssetBackupPayload,
    SharedCaseCommandEnvelope, SharedCaseCommandResponse, SharedCaseDomainEvent,
    SharedCaseEventType, SharedCaseGrant, SharedCasePermission, SharedCasePublicationRecord,
    SharedCasePublicationStatus, BACKUP_PROTOCOL_VERSION, SHARED_CASE_PROTOCOL_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_USERNAME_CHARS: usize = 128;
const MAX_CONTEXT_CHARS: usize = 160;

#[derive(Debug)]
pub struct SharedCaseCommandOutcome {
    pub response: SharedCaseCommandResponse,
    pub emitted_events: Vec<SharedCaseDomainEvent>,
}

pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection.execute_batch(r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS shared_case_publications (
            id TEXT PRIMARY KEY NOT NULL,
            case_id TEXT NOT NULL UNIQUE,
            asset_id TEXT NOT NULL,
            project_id TEXT,
            title TEXT NOT NULL,
            client_name TEXT NOT NULL,
            content_sha256 TEXT NOT NULL CHECK(length(content_sha256) = 64),
            remote_object_key TEXT,
            remote_etag TEXT,
            status TEXT NOT NULL CHECK(status IN ('pendingBackup','published','withdrawn')),
            publisher_username TEXT NOT NULL,
            revision INTEGER NOT NULL CHECK(revision >= 1),
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            published_at INTEGER,
            withdrawn_at INTEGER,
            FOREIGN KEY(case_id) REFERENCES cases(id) ON DELETE RESTRICT,
            FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE RESTRICT
        );
        CREATE INDEX IF NOT EXISTS idx_shared_case_publications_status_updated
            ON shared_case_publications(status, updated_at DESC, id DESC);
        CREATE INDEX IF NOT EXISTS idx_shared_case_publications_sha
            ON shared_case_publications(content_sha256, updated_at DESC, id DESC);
        CREATE TABLE IF NOT EXISTS shared_case_grants (
            publication_id TEXT NOT NULL,
            username TEXT NOT NULL,
            permission TEXT NOT NULL CHECK(permission IN ('discover','preview','reference','download')),
            created_at INTEGER NOT NULL,
            PRIMARY KEY(publication_id, username, permission),
            FOREIGN KEY(publication_id) REFERENCES shared_case_publications(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_shared_case_grants_username
            ON shared_case_grants(username, permission, publication_id);
        CREATE TABLE IF NOT EXISTS shared_case_events (
            sequence INTEGER PRIMARY KEY AUTOINCREMENT,
            event_id TEXT NOT NULL UNIQUE,
            event_type TEXT NOT NULL CHECK(event_type IN
                ('sharedCase.published','sharedCase.grantsUpdated','sharedCase.withdrawn')),
            aggregate_id TEXT NOT NULL,
            revision INTEGER NOT NULL CHECK(revision >= 1),
            occurred_at INTEGER NOT NULL,
            trace_id TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            FOREIGN KEY(aggregate_id) REFERENCES shared_case_publications(id) ON DELETE RESTRICT
        );
        CREATE INDEX IF NOT EXISTS idx_shared_case_events_aggregate
            ON shared_case_events(aggregate_id, sequence);
        CREATE TABLE IF NOT EXISTS shared_case_command_receipts (
            idempotency_key TEXT PRIMARY KEY NOT NULL,
            command_id TEXT NOT NULL UNIQUE,
            command_type TEXT NOT NULL CHECK(command_type IN
                ('sharedCase.publish','sharedCase.updateGrants','sharedCase.withdraw')),
            protocol_version TEXT NOT NULL,
            deadline_at INTEGER,
            request_fingerprint TEXT NOT NULL CHECK(length(request_fingerprint) = 64),
            response_json TEXT NOT NULL,
            completed_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shared_case_command_receipts_completed
            ON shared_case_command_receipts(completed_at);
    "#).map_err(sql_error)
}

pub fn execute_command(
    connection: &mut Connection,
    vault_root: &Path,
    backup_outbox: &BackupOutbox,
    command: SharedCaseCommandEnvelope,
) -> Result<SharedCaseCommandOutcome, HostError> {
    let command = normalize_command(command)?;
    let fingerprint = command_fingerprint(&command)?;
    let meta = command.meta();
    if let Some(response) = find_existing_receipt(
        connection,
        &meta.command_id,
        &meta.idempotency_key,
        &fingerprint,
    )? {
        return Ok(SharedCaseCommandOutcome {
            response,
            emitted_events: Vec::new(),
        });
    }
    validate_deadline(meta.deadline_at)?;
    let prepared_publish = match &command {
        NormalizedSharedCaseCommand::Publish { payload, meta } => Some(prepare_publish(
            connection,
            vault_root,
            backup_outbox,
            payload,
            meta,
        )?),
        _ => None,
    };
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    if let Some(response) = find_existing_receipt(
        &transaction,
        &meta.command_id,
        &meta.idempotency_key,
        &fingerprint,
    )? {
        transaction.commit().map_err(sql_error)?;
        return Ok(SharedCaseCommandOutcome {
            response,
            emitted_events: Vec::new(),
        });
    }
    validate_deadline(meta.deadline_at)?;
    let (publication, event_type) = match (&command, prepared_publish.as_ref()) {
        (NormalizedSharedCaseCommand::Publish { payload, meta }, Some(prepared)) => (
            publish(&transaction, payload, meta, prepared)?,
            SharedCaseEventType::Published,
        ),
        (NormalizedSharedCaseCommand::UpdateGrants { payload, meta }, _) => (
            update_grants(&transaction, payload, meta.expected_revision.unwrap())?,
            SharedCaseEventType::GrantsUpdated,
        ),
        (NormalizedSharedCaseCommand::Withdraw { payload, meta }, _) => (
            withdraw(&transaction, payload, meta.expected_revision.unwrap())?,
            SharedCaseEventType::Withdrawn,
        ),
        _ => unreachable!("publish preparation matches publish command"),
    };
    let event = append_event(
        &transaction,
        event_type,
        &publication,
        &meta.context.trace_id,
    )?;
    let completed_at = now_millis();
    let response = SharedCaseCommandResponse {
        receipt: CommandReceipt {
            command_id: meta.command_id.clone(),
            idempotency_key: meta.idempotency_key.clone(),
            command_type: command.command_type().to_string(),
            aggregate_id: publication.id.clone(),
            revision: publication.revision,
            last_event_sequence: event.sequence,
            completed_at,
        },
        publication,
        replayed: false,
    };
    let response_json = serde_json::to_string(&response).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO shared_case_command_receipts
         (idempotency_key, command_id, command_type, protocol_version, deadline_at,
          request_fingerprint, response_json, completed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                meta.idempotency_key,
                meta.command_id,
                command.command_type(),
                meta.protocol_version,
                meta.deadline_at,
                fingerprint,
                response_json,
                completed_at
            ],
        )
        .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)?;
    Ok(SharedCaseCommandOutcome {
        response,
        emitted_events: vec![event],
    })
}

pub fn list_authorized(
    connection: &Connection,
    username: &str,
) -> Result<Vec<SharedCasePublicationRecord>, HostError> {
    let username = normalize_username(username.to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT p.id, p.case_id, p.asset_id, p.project_id, p.title, p.client_name,
                p.content_sha256, p.remote_object_key, p.remote_etag, p.status,
                p.publisher_username, p.revision, p.created_at, p.updated_at,
                p.published_at, p.withdrawn_at
         FROM shared_case_publications p
         WHERE p.status = 'published' AND EXISTS (
             SELECT 1 FROM shared_case_grants g
             WHERE g.publication_id = p.id AND g.username = ?1 AND g.permission = 'discover'
         ) ORDER BY p.updated_at DESC, p.id DESC",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map([username], publication_from_row)
        .map_err(sql_error)?;
    let mut publications = Vec::new();
    for row in rows {
        let mut publication = row.map_err(sql_error)?;
        publication.grants = load_grants(connection, &publication.id)?;
        publications.push(publication);
    }
    Ok(publications)
}

pub fn reconcile_pending(
    connection: &mut Connection,
    backup_outbox: &BackupOutbox,
    trace_id: &str,
) -> Result<Vec<SharedCaseDomainEvent>, HostError> {
    let trace_id = trace_id.trim();
    if trace_id.is_empty() || trace_id.chars().count() > MAX_CONTEXT_CHARS {
        return Err(HostError::validation(
            "shared case reconciliation traceId is invalid",
        ));
    }
    let pending = {
        let mut statement = connection
            .prepare(
                "SELECT id, content_sha256 FROM shared_case_publications
                 WHERE status = 'pendingBackup' ORDER BY updated_at ASC, id ASC",
            )
            .map_err(sql_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql_error)?;
        let mut pending = Vec::new();
        for row in rows {
            pending.push(row.map_err(sql_error)?);
        }
        pending
    };
    let mut completed = Vec::new();
    for (publication_id, content_sha256) in pending {
        if let Some(remote) = backup_outbox.find_latest_backed_up_by_sha256(&content_sha256)? {
            completed.push((
                publication_id,
                content_sha256,
                remote.remote_object_key,
                remote.etag,
            ));
        }
    }
    if completed.is_empty() {
        return Ok(Vec::new());
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let mut events = Vec::new();
    for (publication_id, content_sha256, remote_object_key, remote_etag) in completed {
        let published_at = now_millis();
        let changed = transaction
            .execute(
                "UPDATE shared_case_publications
                 SET remote_object_key = ?1, remote_etag = ?2, status = 'published',
                     revision = revision + 1, updated_at = ?3,
                     published_at = COALESCE(published_at, ?3)
                 WHERE id = ?4 AND content_sha256 = ?5 AND status = 'pendingBackup'",
                params![
                    remote_object_key,
                    remote_etag,
                    published_at,
                    publication_id,
                    content_sha256
                ],
            )
            .map_err(sql_error)?;
        if changed == 0 {
            continue;
        }
        let publication = find_publication(&transaction, &publication_id)?
            .ok_or_else(|| HostError::internal("reconciled shared case publication disappeared"))?;
        events.push(append_event(
            &transaction,
            SharedCaseEventType::Published,
            &publication,
            trace_id,
        )?);
    }
    transaction.commit().map_err(sql_error)?;
    Ok(events)
}

pub fn replay_events(
    connection: &Connection,
    after_sequence: i64,
    limit: u32,
) -> Result<Vec<SharedCaseDomainEvent>, HostError> {
    if after_sequence < 0 {
        return Err(HostError::validation("afterSequence must be >= 0"));
    }
    if !(1..=1_000).contains(&limit) {
        return Err(HostError::validation(
            "shared case event limit must be 1..=1000",
        ));
    }
    let mut statement = connection
        .prepare(
            "SELECT sequence, event_id, event_type, aggregate_id, revision,
                occurred_at, trace_id, payload_json
         FROM shared_case_events WHERE sequence > ?1 ORDER BY sequence ASC LIMIT ?2",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map(params![after_sequence, i64::from(limit)], event_from_row)
        .map_err(sql_error)?;
    let mut events = Vec::new();
    for row in rows {
        events.push(row.map_err(sql_error)?);
    }
    Ok(events)
}

#[derive(Debug, Clone)]
struct CommandMeta {
    command_id: String,
    protocol_version: String,
    context: OperationContext,
    idempotency_key: String,
    expected_revision: Option<i64>,
    deadline_at: Option<i64>,
}

#[derive(Debug, Clone)]
struct PublishPayload {
    case_id: String,
    grants: Vec<SharedCaseGrant>,
}

#[derive(Debug, Clone)]
struct UpdateGrantsPayload {
    publication_id: String,
    grants: Vec<SharedCaseGrant>,
}

#[derive(Debug, Clone)]
struct WithdrawPayload {
    publication_id: String,
}

#[derive(Debug, Clone)]
enum NormalizedSharedCaseCommand {
    Publish {
        meta: CommandMeta,
        payload: PublishPayload,
    },
    UpdateGrants {
        meta: CommandMeta,
        payload: UpdateGrantsPayload,
    },
    Withdraw {
        meta: CommandMeta,
        payload: WithdrawPayload,
    },
}

impl NormalizedSharedCaseCommand {
    fn meta(&self) -> &CommandMeta {
        match self {
            Self::Publish { meta, .. }
            | Self::UpdateGrants { meta, .. }
            | Self::Withdraw { meta, .. } => meta,
        }
    }

    fn command_type(&self) -> &'static str {
        match self {
            Self::Publish { .. } => "sharedCase.publish",
            Self::UpdateGrants { .. } => "sharedCase.updateGrants",
            Self::Withdraw { .. } => "sharedCase.withdraw",
        }
    }
}

fn normalize_command(
    command: SharedCaseCommandEnvelope,
) -> Result<NormalizedSharedCaseCommand, HostError> {
    match command {
        SharedCaseCommandEnvelope::Publish {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            if expected_revision.is_some() {
                return Err(HostError::validation(
                    "sharedCase.publish rejects expectedRevision",
                ));
            }
            Ok(NormalizedSharedCaseCommand::Publish {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    context,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload: PublishPayload {
                    case_id: normalize_uuid("caseId", payload.case_id)?,
                    grants: normalize_grants(payload.grants)?,
                },
            })
        }
        SharedCaseCommandEnvelope::UpdateGrants {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            require_expected_revision("sharedCase.updateGrants", expected_revision)?;
            Ok(NormalizedSharedCaseCommand::UpdateGrants {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    context,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload: UpdateGrantsPayload {
                    publication_id: normalize_uuid("publicationId", payload.publication_id)?,
                    grants: normalize_grants(payload.grants)?,
                },
            })
        }
        SharedCaseCommandEnvelope::Withdraw {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            require_expected_revision("sharedCase.withdraw", expected_revision)?;
            Ok(NormalizedSharedCaseCommand::Withdraw {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    context,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload: WithdrawPayload {
                    publication_id: normalize_uuid("publicationId", payload.publication_id)?,
                },
            })
        }
    }
}

fn normalize_meta(
    command_id: String,
    protocol_version: String,
    context: OperationContext,
    idempotency_key: String,
    expected_revision: Option<i64>,
    deadline_at: Option<i64>,
) -> Result<CommandMeta, HostError> {
    if protocol_version != SHARED_CASE_PROTOCOL_VERSION {
        return Err(HostError::new(
            "PROTOCOL_VERSION_UNSUPPORTED",
            format!("shared case protocol {protocol_version} is unsupported; expected {SHARED_CASE_PROTOCOL_VERSION}"),
            false,
        ));
    }
    let context = OperationContext {
        actor_id: normalize_username(context.actor_id)?,
        account_id: context.account_id.map(normalize_username).transpose()?,
        project_id: context
            .project_id
            .map(|value| normalize_required("projectId", value, 1, 128))
            .transpose()?,
        window_id: normalize_required("windowId", context.window_id, 1, MAX_CONTEXT_CHARS)?,
        trace_id: normalize_required("traceId", context.trace_id, 1, MAX_CONTEXT_CHARS)?,
    };
    Ok(CommandMeta {
        command_id: normalize_uuid("commandId", command_id)?,
        protocol_version,
        context,
        idempotency_key: normalize_required("idempotencyKey", idempotency_key, 8, 160)?,
        expected_revision,
        deadline_at,
    })
}

fn require_expected_revision(
    command_type: &str,
    expected_revision: Option<i64>,
) -> Result<(), HostError> {
    if expected_revision.is_none_or(|revision| revision <= 0) {
        return Err(HostError::validation(format!(
            "{command_type} requires expectedRevision > 0"
        )));
    }
    Ok(())
}

fn normalize_grants(grants: Vec<SharedCaseGrant>) -> Result<Vec<SharedCaseGrant>, HostError> {
    let mut normalized: BTreeMap<String, Vec<SharedCasePermission>> = BTreeMap::new();
    for grant in grants {
        let username = normalize_username(grant.username)?;
        let permissions = normalized.entry(username).or_default();
        for permission in grant.permissions {
            if !permissions.contains(&permission) {
                permissions.push(permission);
            }
        }
    }
    Ok(normalized
        .into_iter()
        .map(|(username, permissions)| SharedCaseGrant {
            username,
            permissions,
        })
        .collect())
}

fn normalize_username(value: String) -> Result<String, HostError> {
    let value = value.trim().to_string();
    let chars = value.chars().count();
    if !(2..=MAX_USERNAME_CHARS).contains(&chars) {
        return Err(HostError::validation(format!(
            "username must be 2..{MAX_USERNAME_CHARS} characters"
        )));
    }
    if value
        .chars()
        .any(|character| character.is_control() || "/\\\"'<>".contains(character))
    {
        return Err(HostError::validation(
            "username contains unsupported characters",
        ));
    }
    Ok(value)
}

fn normalize_required(
    field: &str,
    value: String,
    min: usize,
    max: usize,
) -> Result<String, HostError> {
    let value = value.trim().to_string();
    let chars = value.chars().count();
    if chars < min || chars > max {
        return Err(HostError::validation(format!(
            "{field} must be {min}..{max} characters"
        )));
    }
    Ok(value)
}

fn normalize_uuid(field: &str, value: String) -> Result<String, HostError> {
    Uuid::parse_str(value.trim())
        .map(|value| value.to_string())
        .map_err(|_| HostError::validation(format!("{field} must be a UUID")))
}

fn command_fingerprint(command: &NormalizedSharedCaseCommand) -> Result<String, HostError> {
    let meta = command.meta();
    let context = serde_json::json!({
        "actorId": meta.context.actor_id,
        "accountId": meta.context.account_id,
        "projectId": meta.context.project_id,
    });
    let value = match command {
        NormalizedSharedCaseCommand::Publish { payload, .. } => serde_json::json!({
            "commandType": command.command_type(), "protocolVersion": meta.protocol_version,
            "context": context, "expectedRevision": meta.expected_revision,
            "payload": { "caseId": payload.case_id, "grants": payload.grants },
        }),
        NormalizedSharedCaseCommand::UpdateGrants { payload, .. } => serde_json::json!({
            "commandType": command.command_type(), "protocolVersion": meta.protocol_version,
            "context": context, "expectedRevision": meta.expected_revision,
            "payload": { "publicationId": payload.publication_id, "grants": payload.grants },
        }),
        NormalizedSharedCaseCommand::Withdraw { payload, .. } => serde_json::json!({
            "commandType": command.command_type(), "protocolVersion": meta.protocol_version,
            "context": context, "expectedRevision": meta.expected_revision,
            "payload": { "publicationId": payload.publication_id },
        }),
    };
    let bytes = serde_json::to_vec(&value).map_err(json_error)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn validate_deadline(deadline_at: Option<i64>) -> Result<(), HostError> {
    if deadline_at.is_some_and(|deadline| deadline < now_millis()) {
        return Err(HostError::new(
            "COMMAND_DEADLINE_EXCEEDED",
            "shared case command deadline has elapsed",
            false,
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct PreparedPublish {
    case_id: String,
    asset_id: String,
    project_id: Option<String>,
    title: String,
    client_name: String,
    content_sha256: String,
    remote_object_key: Option<String>,
    remote_etag: Option<String>,
    status: SharedCasePublicationStatus,
}

fn prepare_publish(
    connection: &Connection,
    vault_root: &Path,
    backup_outbox: &BackupOutbox,
    payload: &PublishPayload,
    meta: &CommandMeta,
) -> Result<PreparedPublish, HostError> {
    let case_row = connection
        .query_row(
            "SELECT id, asset_id, project_id, title, client_name FROM cases WHERE id = ?1",
            [&payload.case_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::new("CASE_NOT_FOUND", "case record does not exist", false))?;
    if meta.context.project_id.as_deref() != case_row.2.as_deref() {
        return Err(HostError::new(
            "SHARED_CASE_PROJECT_MISMATCH",
            "case record belongs to a different project",
            false,
        ));
    }
    if find_publication_by_case(connection, &payload.case_id)?.is_some() {
        return Err(HostError::conflict(
            "case already has a shared publication; update grants or withdraw it instead",
        ));
    }
    let (asset, _) =
        asset_service::verify_ready_asset_integrity(connection, vault_root, &case_row.1)?;
    if let Some(reusable) = backup_outbox.find_latest_backed_up_by_sha256(&asset.sha256)? {
        return Ok(PreparedPublish {
            case_id: case_row.0,
            asset_id: asset.id,
            project_id: case_row.2,
            title: case_row.3,
            client_name: case_row.4,
            content_sha256: asset.sha256,
            remote_object_key: Some(reusable.remote_object_key),
            remote_etag: reusable.etag,
            status: SharedCasePublicationStatus::Published,
        });
    }
    let backup_identity = format!("shared-case-backup:{}:{}", asset.id, asset.sha256);
    let backup_command_id =
        Uuid::new_v5(&Uuid::NAMESPACE_OID, backup_identity.as_bytes()).to_string();
    backup_outbox.queue(
        BackupCommandEnvelope::Queue {
            command_id: backup_command_id,
            protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
            context: meta.context.clone(),
            payload: QueueAssetBackupPayload {
                asset_id: asset.id.clone(),
            },
            idempotency_key: backup_identity,
            expected_revision: None,
            deadline_at: None,
        },
        &asset.sha256,
    )?;
    Ok(PreparedPublish {
        case_id: case_row.0,
        asset_id: asset.id,
        project_id: case_row.2,
        title: case_row.3,
        client_name: case_row.4,
        content_sha256: asset.sha256,
        remote_object_key: None,
        remote_etag: None,
        status: SharedCasePublicationStatus::PendingBackup,
    })
}

fn publish(
    transaction: &Transaction<'_>,
    payload: &PublishPayload,
    meta: &CommandMeta,
    prepared: &PreparedPublish,
) -> Result<SharedCasePublicationRecord, HostError> {
    if payload.case_id != prepared.case_id {
        return Err(HostError::internal(
            "prepared shared case does not match publish payload",
        ));
    }
    if find_publication_by_case(transaction, &payload.case_id)?.is_some() {
        return Err(HostError::conflict(
            "case already has a shared publication; update grants or withdraw it instead",
        ));
    }
    let id = Uuid::new_v4().to_string();
    let now = now_millis();
    let published_at =
        matches!(prepared.status, SharedCasePublicationStatus::Published).then_some(now);
    transaction
        .execute(
            "INSERT INTO shared_case_publications
         (id, case_id, asset_id, project_id, title, client_name, content_sha256,
          remote_object_key, remote_etag, status, publisher_username, revision,
          created_at, updated_at, published_at, withdrawn_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, ?12, ?12, ?13, NULL)",
            params![
                id,
                prepared.case_id,
                prepared.asset_id,
                prepared.project_id,
                prepared.title,
                prepared.client_name,
                prepared.content_sha256,
                prepared.remote_object_key,
                prepared.remote_etag,
                publication_status_to_db(&prepared.status),
                meta.context.actor_id,
                now,
                published_at
            ],
        )
        .map_err(sql_error)?;
    replace_grants(transaction, &id, &payload.grants, now)?;
    find_publication(transaction, &id)?
        .ok_or_else(|| HostError::internal("shared case publication disappeared after insert"))
}

fn update_grants(
    transaction: &Transaction<'_>,
    payload: &UpdateGrantsPayload,
    expected_revision: i64,
) -> Result<SharedCasePublicationRecord, HostError> {
    let current = find_publication(transaction, &payload.publication_id)?.ok_or_else(|| {
        HostError::new(
            "SHARED_CASE_NOT_FOUND",
            "shared case publication does not exist",
            false,
        )
    })?;
    if current.revision != expected_revision {
        return Err(HostError::conflict(format!(
            "shared case {} revision is {}, request expected {}",
            current.id, current.revision, expected_revision
        )));
    }
    if matches!(current.status, SharedCasePublicationStatus::Withdrawn) {
        return Err(HostError::new(
            "SHARED_CASE_WITHDRAWN",
            "withdrawn shared case grants cannot be changed",
            false,
        ));
    }
    let now = now_millis();
    let changed = transaction
        .execute(
            "UPDATE shared_case_publications SET revision = revision + 1, updated_at = ?1
         WHERE id = ?2 AND revision = ?3",
            params![now, payload.publication_id, expected_revision],
        )
        .map_err(sql_error)?;
    if changed != 1 {
        return Err(HostError::conflict(format!(
            "shared case {} changed during grant update",
            payload.publication_id
        )));
    }
    replace_grants(transaction, &payload.publication_id, &payload.grants, now)?;
    find_publication(transaction, &payload.publication_id)?.ok_or_else(|| {
        HostError::internal("shared case publication disappeared after grant update")
    })
}

fn withdraw(
    transaction: &Transaction<'_>,
    payload: &WithdrawPayload,
    expected_revision: i64,
) -> Result<SharedCasePublicationRecord, HostError> {
    let current = find_publication(transaction, &payload.publication_id)?.ok_or_else(|| {
        HostError::new(
            "SHARED_CASE_NOT_FOUND",
            "shared case publication does not exist",
            false,
        )
    })?;
    if current.revision != expected_revision {
        return Err(HostError::conflict(format!(
            "shared case {} revision is {}, request expected {}",
            current.id, current.revision, expected_revision
        )));
    }
    if matches!(current.status, SharedCasePublicationStatus::Withdrawn) {
        return Err(HostError::new(
            "SHARED_CASE_WITHDRAWN",
            "shared case publication is already withdrawn",
            false,
        ));
    }
    let now = now_millis();
    let changed = transaction
        .execute(
            "UPDATE shared_case_publications
         SET status = 'withdrawn', revision = revision + 1,
             updated_at = ?1, withdrawn_at = ?1
         WHERE id = ?2 AND revision = ?3",
            params![now, payload.publication_id, expected_revision],
        )
        .map_err(sql_error)?;
    if changed != 1 {
        return Err(HostError::conflict(format!(
            "shared case {} changed during withdrawal",
            payload.publication_id
        )));
    }
    find_publication(transaction, &payload.publication_id)?
        .ok_or_else(|| HostError::internal("shared case publication disappeared after withdrawal"))
}

fn replace_grants(
    transaction: &Transaction<'_>,
    publication_id: &str,
    grants: &[SharedCaseGrant],
    created_at: i64,
) -> Result<(), HostError> {
    transaction
        .execute(
            "DELETE FROM shared_case_grants WHERE publication_id = ?1",
            [publication_id],
        )
        .map_err(sql_error)?;
    for grant in grants {
        for permission in &grant.permissions {
            transaction
                .execute(
                    "INSERT INTO shared_case_grants
                 (publication_id, username, permission, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                    params![
                        publication_id,
                        grant.username,
                        permission_to_db(permission),
                        created_at
                    ],
                )
                .map_err(sql_error)?;
        }
    }
    Ok(())
}

fn append_event(
    transaction: &Transaction<'_>,
    event_type: SharedCaseEventType,
    publication: &SharedCasePublicationRecord,
    trace_id: &str,
) -> Result<SharedCaseDomainEvent, HostError> {
    let event_id = Uuid::new_v4().to_string();
    let occurred_at = now_millis();
    let payload_json = serde_json::to_string(publication).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO shared_case_events
         (event_id, event_type, aggregate_id, revision, occurred_at, trace_id, payload_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event_id,
                event_type_to_wire(&event_type),
                publication.id,
                publication.revision,
                occurred_at,
                trace_id,
                payload_json
            ],
        )
        .map_err(sql_error)?;
    let sequence = transaction.last_insert_rowid();
    Ok(SharedCaseDomainEvent {
        sequence,
        event_id,
        event_type,
        aggregate_id: publication.id.clone(),
        revision: publication.revision,
        occurred_at,
        trace_id: trace_id.to_string(),
        publication: publication.clone(),
    })
}

fn find_publication_by_case(
    connection: &Connection,
    case_id: &str,
) -> Result<Option<SharedCasePublicationRecord>, HostError> {
    load_publication(
        connection,
        "SELECT id, case_id, asset_id, project_id, title, client_name, content_sha256,
                remote_object_key, remote_etag, status, publisher_username, revision,
                created_at, updated_at, published_at, withdrawn_at
         FROM shared_case_publications WHERE case_id = ?1",
        case_id,
    )
}

fn find_publication(
    connection: &Connection,
    publication_id: &str,
) -> Result<Option<SharedCasePublicationRecord>, HostError> {
    load_publication(
        connection,
        "SELECT id, case_id, asset_id, project_id, title, client_name, content_sha256,
                remote_object_key, remote_etag, status, publisher_username, revision,
                created_at, updated_at, published_at, withdrawn_at
         FROM shared_case_publications WHERE id = ?1",
        publication_id,
    )
}

fn load_publication(
    connection: &Connection,
    sql: &str,
    key: &str,
) -> Result<Option<SharedCasePublicationRecord>, HostError> {
    connection
        .query_row(sql, [key], publication_from_row)
        .optional()
        .map_err(sql_error)?
        .map(|mut publication| {
            publication.grants = load_grants(connection, &publication.id)?;
            Ok(publication)
        })
        .transpose()
}

fn publication_from_row(row: &Row<'_>) -> rusqlite::Result<SharedCasePublicationRecord> {
    let status: String = row.get(9)?;
    Ok(SharedCasePublicationRecord {
        id: row.get(0)?,
        case_id: row.get(1)?,
        asset_id: row.get(2)?,
        project_id: row.get(3)?,
        title: row.get(4)?,
        client_name: row.get(5)?,
        content_sha256: row.get(6)?,
        remote_object_key: row.get(7)?,
        remote_etag: row.get(8)?,
        status: publication_status_from_db(&status)
            .ok_or_else(|| conversion_error("status", &status))?,
        publisher_username: row.get(10)?,
        grants: Vec::new(),
        revision: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        published_at: row.get(14)?,
        withdrawn_at: row.get(15)?,
    })
}

fn load_grants(
    connection: &Connection,
    publication_id: &str,
) -> Result<Vec<SharedCaseGrant>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT username, permission FROM shared_case_grants
         WHERE publication_id = ?1 ORDER BY username ASC, permission ASC",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map([publication_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(sql_error)?;
    let mut grants: BTreeMap<String, Vec<SharedCasePermission>> = BTreeMap::new();
    for row in rows {
        let (username, permission) = row.map_err(sql_error)?;
        let permission = permission_from_db(&permission)
            .ok_or_else(|| HostError::internal("invalid shared case permission in SQLite"))?;
        grants.entry(username).or_default().push(permission);
    }
    Ok(grants
        .into_iter()
        .map(|(username, permissions)| SharedCaseGrant {
            username,
            permissions,
        })
        .collect())
}

#[derive(Debug)]
struct StoredReceipt {
    command_id: String,
    idempotency_key: String,
    fingerprint: String,
    response_json: String,
}

fn find_existing_receipt(
    connection: &Connection,
    command_id: &str,
    idempotency_key: &str,
    fingerprint: &str,
) -> Result<Option<SharedCaseCommandResponse>, HostError> {
    let by_key = load_receipt_by_key(connection, idempotency_key)?;
    let by_command = load_receipt_by_command(connection, command_id)?;
    if by_key
        .as_ref()
        .is_some_and(|receipt| receipt.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "IDEMPOTENCY_KEY_REUSED",
            "idempotencyKey reused for a different shared case request",
            false,
        ));
    }
    if by_command
        .as_ref()
        .is_some_and(|receipt| receipt.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "COMMAND_ID_REUSED",
            "commandId reused for a different shared case request",
            false,
        ));
    }
    if let (Some(key_receipt), Some(command_receipt)) = (&by_key, &by_command) {
        if key_receipt.command_id != command_receipt.command_id
            || key_receipt.idempotency_key != command_receipt.idempotency_key
        {
            return Err(HostError::new(
                "COMMAND_IDENTITY_COLLISION",
                "commandId and idempotencyKey identify different committed shared case commands",
                false,
            ));
        }
    }
    by_key
        .or(by_command)
        .map(|receipt| {
            let mut response: SharedCaseCommandResponse =
                serde_json::from_str(&receipt.response_json).map_err(json_error)?;
            response.replayed = true;
            Ok(response)
        })
        .transpose()
}

fn load_receipt_by_key(
    connection: &Connection,
    idempotency_key: &str,
) -> Result<Option<StoredReceipt>, HostError> {
    connection
        .query_row(
            "SELECT command_id, idempotency_key, request_fingerprint, response_json
         FROM shared_case_command_receipts WHERE idempotency_key = ?1",
            [idempotency_key],
            receipt_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn load_receipt_by_command(
    connection: &Connection,
    command_id: &str,
) -> Result<Option<StoredReceipt>, HostError> {
    connection
        .query_row(
            "SELECT command_id, idempotency_key, request_fingerprint, response_json
         FROM shared_case_command_receipts WHERE command_id = ?1",
            [command_id],
            receipt_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn receipt_from_row(row: &Row<'_>) -> rusqlite::Result<StoredReceipt> {
    Ok(StoredReceipt {
        command_id: row.get(0)?,
        idempotency_key: row.get(1)?,
        fingerprint: row.get(2)?,
        response_json: row.get(3)?,
    })
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<SharedCaseDomainEvent> {
    let event_type: String = row.get(2)?;
    let payload_json: String = row.get(7)?;
    Ok(SharedCaseDomainEvent {
        sequence: row.get(0)?,
        event_id: row.get(1)?,
        event_type: event_type_from_wire(&event_type)
            .ok_or_else(|| conversion_error("event_type", &event_type))?,
        aggregate_id: row.get(3)?,
        revision: row.get(4)?,
        occurred_at: row.get(5)?,
        trace_id: row.get(6)?,
        publication: serde_json::from_str(&payload_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                payload_json.len(),
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
    })
}

fn permission_to_db(permission: &SharedCasePermission) -> &'static str {
    match permission {
        SharedCasePermission::Discover => "discover",
        SharedCasePermission::Preview => "preview",
        SharedCasePermission::Reference => "reference",
        SharedCasePermission::Download => "download",
    }
}

fn permission_from_db(value: &str) -> Option<SharedCasePermission> {
    Some(match value {
        "discover" => SharedCasePermission::Discover,
        "preview" => SharedCasePermission::Preview,
        "reference" => SharedCasePermission::Reference,
        "download" => SharedCasePermission::Download,
        _ => return None,
    })
}

fn publication_status_to_db(status: &SharedCasePublicationStatus) -> &'static str {
    match status {
        SharedCasePublicationStatus::PendingBackup => "pendingBackup",
        SharedCasePublicationStatus::Published => "published",
        SharedCasePublicationStatus::Withdrawn => "withdrawn",
    }
}

fn publication_status_from_db(value: &str) -> Option<SharedCasePublicationStatus> {
    Some(match value {
        "pendingBackup" => SharedCasePublicationStatus::PendingBackup,
        "published" => SharedCasePublicationStatus::Published,
        "withdrawn" => SharedCasePublicationStatus::Withdrawn,
        _ => return None,
    })
}

fn event_type_to_wire(event_type: &SharedCaseEventType) -> &'static str {
    match event_type {
        SharedCaseEventType::Published => "sharedCase.published",
        SharedCaseEventType::GrantsUpdated => "sharedCase.grantsUpdated",
        SharedCaseEventType::Withdrawn => "sharedCase.withdrawn",
    }
}

fn event_type_from_wire(value: &str) -> Option<SharedCaseEventType> {
    Some(match value {
        "sharedCase.published" => SharedCaseEventType::Published,
        "sharedCase.grantsUpdated" => SharedCaseEventType::GrantsUpdated,
        "sharedCase.withdrawn" => SharedCaseEventType::Withdrawn,
        _ => return None,
    })
}

fn conversion_error(field: &str, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        value.len(),
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid shared case {field} database value: {value}"),
        )),
    )
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn sql_error(error: rusqlite::Error) -> HostError {
    HostError::internal(format!("shared case SQLite operation failed: {error}"))
}

fn json_error(error: serde_json::Error) -> HostError {
    HostError::internal(format!("shared case JSON operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::case_library;
    use crate::protocol::{
        CaseCommandEnvelope, CaseContentType, CasePresentation, CaseQualityTier, CreateCasePayload,
        PublishSharedCasePayload, UpdateSharedCaseGrantsPayload, WithdrawSharedCasePayload,
        PROTOCOL_VERSION,
    };
    use std::fs;
    use std::path::PathBuf;
    use tempfile::{tempdir, TempDir};

    const PROJECT_ID: &str = "project-shared-cases";

    struct Fixture {
        _directory: TempDir,
        connection: Connection,
        backup_outbox: BackupOutbox,
        vault_root: PathBuf,
        asset_id: String,
        asset_sha256: String,
        case_id: String,
    }

    impl Fixture {
        fn new(contents: &[u8]) -> Self {
            let directory = tempdir().unwrap();
            let database_path = directory.path().join("ledger.sqlite");
            let vault_root = directory.path().join("vault");
            let source_path = directory.path().join("shared-case-source.mp4");
            fs::write(&source_path, contents).unwrap();
            let mut connection = Connection::open(&database_path).unwrap();
            connection
                .execute_batch(
                    "PRAGMA foreign_keys = ON;
                     CREATE TABLE projects (id TEXT PRIMARY KEY NOT NULL);
                     INSERT INTO projects (id) VALUES ('project-shared-cases');",
                )
                .unwrap();
            asset_service::migrate(&connection).unwrap();
            case_library::migrate(&connection).unwrap();
            migrate(&connection).unwrap();

            let asset = asset_service::import_file(
                &mut connection,
                &vault_root,
                Some(PROJECT_ID),
                &source_path,
            )
            .unwrap();
            let case = case_library::execute_command(
                &mut connection,
                CaseCommandEnvelope::Create {
                    command_id: Uuid::new_v4().to_string(),
                    protocol_version: PROTOCOL_VERSION.to_string(),
                    context: context(),
                    payload: CreateCasePayload {
                        asset_id: asset.id.clone(),
                        project_id: Some(PROJECT_ID.to_string()),
                        title: "Shared reference film".to_string(),
                        client_name: "Client Alpha".to_string(),
                        content_type: CaseContentType::Brand,
                        presentation: CasePresentation::MixedMedia,
                        has_actors: true,
                        is_aigc: true,
                        quality_tier: CaseQualityTier::Premium,
                        tags: vec!["reference".to_string()],
                        notes: "approved for internal reuse".to_string(),
                    },
                    idempotency_key: format!("case-create-{}", Uuid::new_v4()),
                    expected_revision: None,
                    deadline_at: None,
                },
            )
            .unwrap()
            .response
            .case_record;
            let backup_outbox = BackupOutbox::open(&database_path).unwrap();
            Self {
                _directory: directory,
                connection,
                backup_outbox,
                vault_root,
                asset_id: asset.id,
                asset_sha256: asset.sha256,
                case_id: case.id,
            }
        }

        fn publish(
            &mut self,
            command_id: &str,
            idempotency_key: &str,
            grants: Vec<SharedCaseGrant>,
        ) -> Result<SharedCaseCommandOutcome, HostError> {
            execute_command(
                &mut self.connection,
                &self.vault_root,
                &self.backup_outbox,
                publish_command(command_id, idempotency_key, &self.case_id, grants),
            )
        }

        fn complete_backup(&self, remote_object_key: &str, etag: Option<&str>) {
            self.backup_outbox
                .queue(
                    BackupCommandEnvelope::Queue {
                        command_id: Uuid::new_v4().to_string(),
                        protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
                        context: context(),
                        payload: QueueAssetBackupPayload {
                            asset_id: self.asset_id.clone(),
                        },
                        idempotency_key: format!("fixture-backup-{}", Uuid::new_v4()),
                        expected_revision: None,
                        deadline_at: None,
                    },
                    &self.asset_sha256,
                )
                .unwrap();
            let claimed = self
                .backup_outbox
                .claim_next(&format!("trace-claim-{}", Uuid::new_v4()))
                .unwrap()
                .backup
                .unwrap();
            self.backup_outbox
                .mark_backed_up(
                    &self.asset_id,
                    &self.asset_sha256,
                    claimed.revision,
                    remote_object_key,
                    etag,
                    &format!("trace-complete-{}", Uuid::new_v4()),
                )
                .unwrap();
        }
    }

    fn context() -> OperationContext {
        OperationContext {
            actor_id: "admin-user".to_string(),
            account_id: Some("admin-user".to_string()),
            project_id: Some(PROJECT_ID.to_string()),
            window_id: "window-shared-cases".to_string(),
            trace_id: format!("trace-shared-case-{}", Uuid::new_v4()),
        }
    }

    fn grant(username: &str, permissions: &[SharedCasePermission]) -> SharedCaseGrant {
        SharedCaseGrant {
            username: username.to_string(),
            permissions: permissions.to_vec(),
        }
    }

    fn publish_command(
        command_id: &str,
        idempotency_key: &str,
        case_id: &str,
        grants: Vec<SharedCaseGrant>,
    ) -> SharedCaseCommandEnvelope {
        SharedCaseCommandEnvelope::Publish {
            command_id: command_id.to_string(),
            protocol_version: SHARED_CASE_PROTOCOL_VERSION.to_string(),
            context: context(),
            payload: PublishSharedCasePayload {
                case_id: case_id.to_string(),
                grants,
            },
            idempotency_key: idempotency_key.to_string(),
            expected_revision: None,
            deadline_at: None,
        }
    }

    fn update_grants_command(
        publication_id: &str,
        expected_revision: i64,
        grants: Vec<SharedCaseGrant>,
    ) -> SharedCaseCommandEnvelope {
        SharedCaseCommandEnvelope::UpdateGrants {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: SHARED_CASE_PROTOCOL_VERSION.to_string(),
            context: context(),
            payload: UpdateSharedCaseGrantsPayload {
                publication_id: publication_id.to_string(),
                grants,
            },
            idempotency_key: format!("shared-grants-{}", Uuid::new_v4()),
            expected_revision: Some(expected_revision),
            deadline_at: None,
        }
    }

    fn withdraw_command(publication_id: &str, expected_revision: i64) -> SharedCaseCommandEnvelope {
        SharedCaseCommandEnvelope::Withdraw {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: SHARED_CASE_PROTOCOL_VERSION.to_string(),
            context: context(),
            payload: WithdrawSharedCasePayload {
                publication_id: publication_id.to_string(),
            },
            idempotency_key: format!("shared-withdraw-{}", Uuid::new_v4()),
            expected_revision: Some(expected_revision),
            deadline_at: None,
        }
    }

    fn table_count(connection: &Connection, table: &str) -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    #[test]
    fn publish_without_reusable_sha_queues_backup_and_returns_pending_backup() {
        let mut fixture = Fixture::new(b"pending backup fixture");
        let outcome = fixture
            .publish(
                &Uuid::new_v4().to_string(),
                "shared-publish-pending",
                vec![grant("alice", &[SharedCasePermission::Discover])],
            )
            .unwrap();

        assert_eq!(
            outcome.response.publication.status,
            SharedCasePublicationStatus::PendingBackup
        );
        assert_eq!(outcome.response.publication.remote_object_key, None);
        assert_eq!(outcome.response.publication.remote_etag, None);
        assert_eq!(outcome.response.publication.published_at, None);
        assert_eq!(outcome.response.publication.revision, 1);
        assert_eq!(outcome.emitted_events.len(), 1);
        assert_eq!(
            outcome.emitted_events[0].event_type,
            SharedCaseEventType::Published
        );
        let backup: (String, String) = fixture
            .connection
            .query_row(
                "SELECT content_sha256, state FROM asset_backups WHERE asset_id = ?1",
                [&fixture.asset_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(backup.0, fixture.asset_sha256);
        assert_eq!(backup.1, "queued");
        assert!(list_authorized(&fixture.connection, "alice")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn pending_publication_reconciles_after_backup_completion() {
        let mut fixture = Fixture::new(b"reconciliation fixture");
        let pending = fixture
            .publish(
                &Uuid::new_v4().to_string(),
                "shared-publish-reconcile",
                vec![grant("alice", &[SharedCasePermission::Discover])],
            )
            .unwrap()
            .response
            .publication;
        fixture.complete_backup("shared/reconciled/reference.mp4", Some("etag-reconciled"));

        let events = reconcile_pending(
            &mut fixture.connection,
            &fixture.backup_outbox,
            "trace-shared-case-reconcile",
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, SharedCaseEventType::Published);
        assert_eq!(events[0].revision, pending.revision + 1);
        assert_eq!(
            events[0].publication.status,
            SharedCasePublicationStatus::Published
        );
        assert_eq!(
            events[0].publication.remote_object_key.as_deref(),
            Some("shared/reconciled/reference.mp4")
        );
        assert_eq!(
            list_authorized(&fixture.connection, "alice").unwrap().len(),
            1
        );
        assert!(reconcile_pending(
            &mut fixture.connection,
            &fixture.backup_outbox,
            "trace-shared-case-reconcile-repeat",
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn publish_reuses_successful_sha_and_returns_published_remote_identity() {
        let mut fixture = Fixture::new(b"reusable backup fixture");
        fixture.complete_backup("shared/sha/reference.mp4", Some("etag-reference"));
        let outcome = fixture
            .publish(
                &Uuid::new_v4().to_string(),
                "shared-publish-reuse",
                vec![grant("alice", &[SharedCasePermission::Discover])],
            )
            .unwrap();

        assert_eq!(
            outcome.response.publication.status,
            SharedCasePublicationStatus::Published
        );
        assert_eq!(
            outcome.response.publication.remote_object_key.as_deref(),
            Some("shared/sha/reference.mp4")
        );
        assert_eq!(
            outcome.response.publication.remote_etag.as_deref(),
            Some("etag-reference")
        );
        assert!(outcome.response.publication.published_at.is_some());
        assert_eq!(table_count(&fixture.connection, "asset_backups"), 1);
    }

    #[test]
    fn authorized_list_requires_discover_permission() {
        let mut fixture = Fixture::new(b"discover permission fixture");
        fixture.complete_backup("shared/discover/reference.mp4", Some("etag-discover"));
        fixture
            .publish(
                &Uuid::new_v4().to_string(),
                "shared-publish-discover",
                vec![
                    grant("alice", &[SharedCasePermission::Preview]),
                    grant(
                        "bob",
                        &[
                            SharedCasePermission::Discover,
                            SharedCasePermission::Download,
                        ],
                    ),
                    grant("charlie", &[SharedCasePermission::Reference]),
                ],
            )
            .unwrap();

        assert!(list_authorized(&fixture.connection, "alice")
            .unwrap()
            .is_empty());
        assert_eq!(
            list_authorized(&fixture.connection, "bob").unwrap().len(),
            1
        );
        assert!(list_authorized(&fixture.connection, "charlie")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn replay_events_validates_and_applies_requested_limit() {
        let mut fixture = Fixture::new(b"replay limit fixture");
        let publication = fixture
            .publish(
                &Uuid::new_v4().to_string(),
                "shared-publish-replay-limit",
                vec![grant("alice", &[SharedCasePermission::Discover])],
            )
            .unwrap()
            .response
            .publication;
        execute_command(
            &mut fixture.connection,
            &fixture.vault_root,
            &fixture.backup_outbox,
            update_grants_command(
                &publication.id,
                publication.revision,
                vec![grant("bob", &[SharedCasePermission::Discover])],
            ),
        )
        .unwrap();

        let first_page = replay_events(&fixture.connection, 0, 1).unwrap();
        assert_eq!(first_page.len(), 1);
        let second_page = replay_events(&fixture.connection, first_page[0].sequence, 1).unwrap();
        assert_eq!(second_page.len(), 1);
        assert!(second_page[0].sequence > first_page[0].sequence);
        assert_eq!(
            replay_events(&fixture.connection, 0, 0).unwrap_err().code,
            "VALIDATION_FAILED"
        );
        assert_eq!(
            replay_events(&fixture.connection, 0, 1_001)
                .unwrap_err()
                .code,
            "VALIDATION_FAILED"
        );
    }

    #[test]
    fn update_grants_replaces_atomically_and_rejects_stale_revision() {
        let mut fixture = Fixture::new(b"grant replacement fixture");
        fixture.complete_backup("shared/grants/reference.mp4", Some("etag-grants"));
        let publication = fixture
            .publish(
                &Uuid::new_v4().to_string(),
                "shared-publish-grants",
                vec![grant(
                    "alice",
                    &[
                        SharedCasePermission::Discover,
                        SharedCasePermission::Preview,
                    ],
                )],
            )
            .unwrap()
            .response
            .publication;

        let updated = execute_command(
            &mut fixture.connection,
            &fixture.vault_root,
            &fixture.backup_outbox,
            update_grants_command(
                &publication.id,
                publication.revision,
                vec![grant(
                    "bob",
                    &[
                        SharedCasePermission::Discover,
                        SharedCasePermission::Download,
                    ],
                )],
            ),
        )
        .unwrap()
        .response
        .publication;
        assert_eq!(updated.revision, 2);
        assert_eq!(
            updated.grants,
            vec![grant(
                "bob",
                &[
                    SharedCasePermission::Discover,
                    SharedCasePermission::Download,
                ],
            )]
        );
        assert!(list_authorized(&fixture.connection, "alice")
            .unwrap()
            .is_empty());
        assert_eq!(
            list_authorized(&fixture.connection, "bob").unwrap().len(),
            1
        );

        let stale_error = execute_command(
            &mut fixture.connection,
            &fixture.vault_root,
            &fixture.backup_outbox,
            update_grants_command(
                &publication.id,
                publication.revision,
                vec![grant("charlie", &[SharedCasePermission::Discover])],
            ),
        )
        .unwrap_err();
        assert_eq!(stale_error.code, "REVISION_CONFLICT");
        let stored = find_publication(&fixture.connection, &publication.id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.revision, 2);
        assert_eq!(stored.grants, updated.grants);
        assert!(list_authorized(&fixture.connection, "charlie")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn withdraw_hides_publication_from_authorized_list() {
        let mut fixture = Fixture::new(b"withdraw fixture");
        fixture.complete_backup("shared/withdraw/reference.mp4", Some("etag-withdraw"));
        let publication = fixture
            .publish(
                &Uuid::new_v4().to_string(),
                "shared-publish-withdraw",
                vec![grant("alice", &[SharedCasePermission::Discover])],
            )
            .unwrap()
            .response
            .publication;
        assert_eq!(
            list_authorized(&fixture.connection, "alice").unwrap().len(),
            1
        );

        let withdrawn = execute_command(
            &mut fixture.connection,
            &fixture.vault_root,
            &fixture.backup_outbox,
            withdraw_command(&publication.id, publication.revision),
        )
        .unwrap()
        .response
        .publication;
        assert_eq!(withdrawn.status, SharedCasePublicationStatus::Withdrawn);
        assert_eq!(withdrawn.revision, 2);
        assert!(withdrawn.withdrawn_at.is_some());
        assert!(list_authorized(&fixture.connection, "alice")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn command_receipts_replay_and_reject_identity_fingerprint_conflicts() {
        let mut fixture = Fixture::new(b"receipt replay fixture");
        let command_id = Uuid::new_v4().to_string();
        let idempotency_key = "shared-publish-receipt";
        let command = publish_command(
            &command_id,
            idempotency_key,
            &fixture.case_id,
            vec![grant("alice", &[SharedCasePermission::Discover])],
        );
        let first = execute_command(
            &mut fixture.connection,
            &fixture.vault_root,
            &fixture.backup_outbox,
            command.clone(),
        )
        .unwrap();
        let replayed = execute_command(
            &mut fixture.connection,
            &fixture.vault_root,
            &fixture.backup_outbox,
            command,
        )
        .unwrap();
        assert!(replayed.response.replayed);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(replayed.response.receipt, first.response.receipt);
        assert_eq!(table_count(&fixture.connection, "shared_case_events"), 1);

        let key_conflict = publish_command(
            &Uuid::new_v4().to_string(),
            idempotency_key,
            &fixture.case_id,
            vec![grant("bob", &[SharedCasePermission::Discover])],
        );
        assert_eq!(
            execute_command(
                &mut fixture.connection,
                &fixture.vault_root,
                &fixture.backup_outbox,
                key_conflict,
            )
            .unwrap_err()
            .code,
            "IDEMPOTENCY_KEY_REUSED"
        );

        let command_conflict = publish_command(
            &command_id,
            "shared-publish-receipt-conflict",
            &fixture.case_id,
            vec![grant("bob", &[SharedCasePermission::Discover])],
        );
        assert_eq!(
            execute_command(
                &mut fixture.connection,
                &fixture.vault_root,
                &fixture.backup_outbox,
                command_conflict,
            )
            .unwrap_err()
            .code,
            "COMMAND_ID_REUSED"
        );
    }

    #[test]
    fn publish_rejects_tampered_vault_bytes_before_backup_or_publication() {
        let mut fixture = Fixture::new(b"authoritative shared case bytes");
        let storage_rel_path: String = fixture
            .connection
            .query_row(
                "SELECT storage_rel_path FROM assets WHERE id = ?1",
                [&fixture.asset_id],
                |row| row.get(0),
            )
            .unwrap();
        fs::write(
            fixture.vault_root.join(storage_rel_path),
            b"tampered shared case bytes",
        )
        .unwrap();

        let error = fixture
            .publish(
                &Uuid::new_v4().to_string(),
                "shared-publish-tampered",
                vec![grant("alice", &[SharedCasePermission::Discover])],
            )
            .unwrap_err();
        assert_eq!(error.code, "VAULT_ASSET_INTEGRITY_MISMATCH");
        assert_eq!(
            table_count(&fixture.connection, "shared_case_publications"),
            0
        );
        assert_eq!(table_count(&fixture.connection, "asset_backups"), 0);
    }
}
