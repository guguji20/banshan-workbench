use crate::protocol::{
    is_legacy_surface_protocol_supported, CaseCommandEnvelope, CaseCommandResponse,
    CaseContentType, CaseDomainEvent, CaseEventType, CasePresentation, CaseQualityTier, CaseRecord,
    CommandReceipt, CreateCasePayload, HostError, OperationContext, UpdateCasePayload,
    LEGACY_PROTOCOL_VERSION, PREVIOUS_PROTOCOL_VERSION, PROTOCOL_1_3_VERSION, PROTOCOL_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_TITLE_CHARS: usize = 200;
const MAX_CLIENT_NAME_CHARS: usize = 160;
const MAX_NOTES_CHARS: usize = 16_000;
const MAX_TAGS: usize = 32;
const MAX_TAG_CHARS: usize = 64;
const MAX_CONTEXT_CHARS: usize = 160;
const MAX_PROJECT_ID_CHARS: usize = 128;

#[derive(Debug)]
pub struct CaseCommandOutcome {
    pub response: CaseCommandResponse,
    pub emitted_events: Vec<CaseDomainEvent>,
}

pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS cases (
                id TEXT PRIMARY KEY NOT NULL,
                asset_id TEXT NOT NULL,
                project_id TEXT,
                title TEXT NOT NULL,
                client_name TEXT NOT NULL,
                content_type TEXT NOT NULL CHECK(content_type IN
                    ('brand','property','interview','lifestyle','product','event',
                     'documentary','narrative','other')),
                presentation TEXT NOT NULL CHECK(presentation IN
                    ('liveAction','animation','mixedMedia','aigc','graphic','other')),
                has_actors INTEGER NOT NULL CHECK(has_actors IN (0, 1)),
                is_aigc INTEGER NOT NULL CHECK(is_aigc IN (0, 1)),
                quality_tier TEXT NOT NULL CHECK(quality_tier IN
                    ('reference','featured','premium')),
                tags_json TEXT NOT NULL,
                notes TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE RESTRICT,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_cases_project_updated
                ON cases(project_id, updated_at DESC, id DESC);
            CREATE INDEX IF NOT EXISTS idx_cases_asset
                ON cases(asset_id, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_cases_taxonomy
                ON cases(content_type, presentation, quality_tier, updated_at DESC);
            CREATE TABLE IF NOT EXISTS case_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL CHECK(event_type IN ('case.created','case.updated')),
                aggregate_id TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                occurred_at INTEGER NOT NULL,
                trace_id TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                FOREIGN KEY(aggregate_id) REFERENCES cases(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_case_events_aggregate
                ON case_events(aggregate_id, sequence);
            CREATE TABLE IF NOT EXISTS case_command_receipts (
                idempotency_key TEXT PRIMARY KEY NOT NULL,
                command_id TEXT NOT NULL UNIQUE,
                command_type TEXT NOT NULL CHECK(command_type IN ('case.create','case.update')),
                protocol_version TEXT NOT NULL,
                deadline_at INTEGER,
                request_fingerprint TEXT NOT NULL CHECK(length(request_fingerprint) = 64),
                response_json TEXT NOT NULL,
                completed_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_case_command_receipts_completed
                ON case_command_receipts(completed_at);
            "#,
        )
        .map_err(sql_error)
}

pub fn execute_command(
    connection: &mut Connection,
    command: CaseCommandEnvelope,
) -> Result<CaseCommandOutcome, HostError> {
    let command = normalize_command(command)?;
    let fingerprint = command_fingerprint(&command)?;
    let meta = command.meta();
    let command_type = command.command_type();

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;

    // Committed work remains replayable after a deadline or asset-state change.
    if let Some(response) = find_existing_receipt(
        &transaction,
        &meta.command_id,
        &meta.idempotency_key,
        &fingerprint,
    )? {
        transaction.commit().map_err(sql_error)?;
        return Ok(CaseCommandOutcome {
            response,
            emitted_events: Vec::new(),
        });
    }

    validate_deadline(meta.deadline_at)?;
    let (case_record, event_type) = match &command {
        NormalizedCaseCommand::Create { payload, .. } => {
            (create_case(&transaction, payload)?, CaseEventType::Created)
        }
        NormalizedCaseCommand::Update { payload, meta } => (
            update_case(
                &transaction,
                payload,
                meta.expected_revision
                    .expect("normalized update has expected revision"),
                meta.context.project_id.as_deref(),
            )?,
            CaseEventType::Updated,
        ),
    };
    let event = append_event(
        &transaction,
        event_type,
        &case_record,
        &meta.context.trace_id,
    )?;
    let completed_at = now_millis();
    let response = CaseCommandResponse {
        receipt: CommandReceipt {
            command_id: meta.command_id.clone(),
            idempotency_key: meta.idempotency_key.clone(),
            command_type: command_type.to_string(),
            aggregate_id: case_record.id.clone(),
            revision: case_record.revision,
            last_event_sequence: event.sequence,
            completed_at,
        },
        case_record,
        replayed: false,
    };
    let response_json = serde_json::to_string(&response).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO case_command_receipts
             (idempotency_key, command_id, command_type, protocol_version, deadline_at,
              request_fingerprint, response_json, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                meta.idempotency_key,
                meta.command_id,
                command_type,
                meta.protocol_version,
                meta.deadline_at,
                fingerprint,
                response_json,
                completed_at,
            ],
        )
        .map_err(sql_error)?;

    match transaction.commit() {
        Ok(()) => Ok(CaseCommandOutcome {
            response,
            emitted_events: vec![event],
        }),
        Err(error) => match find_existing_receipt(
            connection,
            &meta.command_id,
            &meta.idempotency_key,
            &fingerprint,
        ) {
            Ok(Some(mut persisted)) => {
                persisted.replayed = false;
                let persisted_event =
                    load_event(connection, persisted.receipt.last_event_sequence)?;
                Ok(CaseCommandOutcome {
                    response: persisted,
                    emitted_events: vec![persisted_event],
                })
            }
            Ok(None) => Err(sql_error(error)),
            Err(lookup_error) => Err(lookup_error),
        },
    }
}

pub fn list(connection: &Connection) -> Result<Vec<CaseRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, asset_id, project_id, title, client_name, content_type,
                    presentation, has_actors, is_aigc, quality_tier, tags_json, notes,
                    revision, created_at, updated_at
             FROM cases ORDER BY updated_at DESC, id DESC",
        )
        .map_err(sql_error)?;
    let records = statement
        .query_map([], case_from_row)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(records)
}

pub fn replay_events(
    connection: &Connection,
    after_sequence: i64,
    limit: u32,
) -> Result<Vec<CaseDomainEvent>, HostError> {
    if after_sequence < 0 {
        return Err(HostError::validation("afterSequence cannot be negative"));
    }
    let limit = limit.clamp(1, 1_000) as i64;
    let mut statement = connection
        .prepare(
            "SELECT sequence, event_id, event_type, aggregate_id, revision,
                    occurred_at, trace_id, payload_json
             FROM case_events WHERE sequence > ?1 ORDER BY sequence ASC LIMIT ?2",
        )
        .map_err(sql_error)?;
    let events = statement
        .query_map(params![after_sequence, limit], event_from_row)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(events)
}

#[derive(Debug, Clone)]
struct NormalizedContext {
    actor_id: String,
    account_id: Option<String>,
    project_id: Option<String>,
    trace_id: String,
}

#[derive(Debug, Clone)]
struct CommandMeta {
    command_id: String,
    protocol_version: String,
    context: NormalizedContext,
    idempotency_key: String,
    expected_revision: Option<i64>,
    deadline_at: Option<i64>,
}

#[derive(Debug, Clone)]
enum NormalizedCaseCommand {
    Create {
        meta: CommandMeta,
        payload: CreateCasePayload,
    },
    Update {
        meta: CommandMeta,
        payload: UpdateCasePayload,
    },
}

impl NormalizedCaseCommand {
    fn meta(&self) -> &CommandMeta {
        match self {
            Self::Create { meta, .. } | Self::Update { meta, .. } => meta,
        }
    }

    fn command_type(&self) -> &'static str {
        match self {
            Self::Create { .. } => "case.create",
            Self::Update { .. } => "case.update",
        }
    }
}

fn normalize_command(command: CaseCommandEnvelope) -> Result<NormalizedCaseCommand, HostError> {
    match command {
        CaseCommandEnvelope::Create {
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
                    "case.create rejects expectedRevision",
                ));
            }
            let context = normalize_context(context)?;
            let payload = normalize_create_payload(payload)?;
            if context.project_id != payload.project_id {
                return Err(HostError::validation(
                    "context projectId must match case payload projectId",
                ));
            }
            Ok(NormalizedCaseCommand::Create {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    context,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload,
            })
        }
        CaseCommandEnvelope::Update {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            if expected_revision.is_none_or(|revision| revision <= 0) {
                return Err(HostError::validation(
                    "case.update requires expectedRevision > 0",
                ));
            }
            let context = normalize_context(context)?;
            Ok(NormalizedCaseCommand::Update {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    context,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload: normalize_update_payload(payload)?,
            })
        }
    }
}

fn normalize_meta(
    command_id: String,
    protocol_version: String,
    context: NormalizedContext,
    idempotency_key: String,
    expected_revision: Option<i64>,
    deadline_at: Option<i64>,
) -> Result<CommandMeta, HostError> {
    if !is_legacy_surface_protocol_supported(&protocol_version) {
        return Err(HostError::new(
            "PROTOCOL_VERSION_UNSUPPORTED",
            format!(
                "expected protocolVersion {LEGACY_PROTOCOL_VERSION}, {PROTOCOL_1_3_VERSION}, {PREVIOUS_PROTOCOL_VERSION}, or {PROTOCOL_VERSION}, received {protocol_version}"
            ),
            false,
        ));
    }
    let command_id = Uuid::parse_str(command_id.trim())
        .map_err(|_| HostError::validation("commandId must be a UUID"))?
        .to_string();
    let idempotency_key = idempotency_key.trim().to_string();
    let idempotency_length = idempotency_key.chars().count();
    if !(8..=160).contains(&idempotency_length) {
        return Err(HostError::validation(
            "idempotencyKey length must be 8..160",
        ));
    }
    Ok(CommandMeta {
        command_id,
        protocol_version,
        context,
        idempotency_key,
        expected_revision,
        deadline_at,
    })
}

fn normalize_context(context: OperationContext) -> Result<NormalizedContext, HostError> {
    normalize_required("windowId", context.window_id, 1, MAX_CONTEXT_CHARS)?;
    Ok(NormalizedContext {
        actor_id: normalize_required("actorId", context.actor_id, 1, MAX_CONTEXT_CHARS)?,
        account_id: normalize_optional("accountId", context.account_id, MAX_CONTEXT_CHARS)?,
        project_id: normalize_optional("projectId", context.project_id, MAX_PROJECT_ID_CHARS)?,
        trace_id: normalize_required("traceId", context.trace_id, 1, MAX_CONTEXT_CHARS)?,
    })
}

fn normalize_create_payload(payload: CreateCasePayload) -> Result<CreateCasePayload, HostError> {
    Ok(CreateCasePayload {
        asset_id: normalize_uuid("assetId", payload.asset_id)?,
        project_id: normalize_optional("projectId", payload.project_id, MAX_PROJECT_ID_CHARS)?,
        title: normalize_required("title", payload.title, 1, MAX_TITLE_CHARS)?,
        client_name: normalize_required(
            "clientName",
            payload.client_name,
            1,
            MAX_CLIENT_NAME_CHARS,
        )?,
        content_type: payload.content_type,
        presentation: payload.presentation,
        has_actors: payload.has_actors,
        is_aigc: payload.is_aigc,
        quality_tier: payload.quality_tier,
        tags: normalize_tags(payload.tags)?,
        notes: normalize_bounded("notes", payload.notes, MAX_NOTES_CHARS)?,
    })
}

fn normalize_update_payload(payload: UpdateCasePayload) -> Result<UpdateCasePayload, HostError> {
    Ok(UpdateCasePayload {
        case_id: normalize_uuid("caseId", payload.case_id)?,
        title: normalize_required("title", payload.title, 1, MAX_TITLE_CHARS)?,
        client_name: normalize_required(
            "clientName",
            payload.client_name,
            1,
            MAX_CLIENT_NAME_CHARS,
        )?,
        content_type: payload.content_type,
        presentation: payload.presentation,
        has_actors: payload.has_actors,
        is_aigc: payload.is_aigc,
        quality_tier: payload.quality_tier,
        tags: normalize_tags(payload.tags)?,
        notes: normalize_bounded("notes", payload.notes, MAX_NOTES_CHARS)?,
    })
}

fn normalize_required(
    field: &str,
    value: String,
    min: usize,
    max: usize,
) -> Result<String, HostError> {
    let value = value.trim().to_string();
    let length = value.chars().count();
    if length < min || length > max {
        return Err(HostError::validation(format!(
            "{field} length must be {min}..{max}"
        )));
    }
    Ok(value)
}

fn normalize_bounded(field: &str, value: String, max: usize) -> Result<String, HostError> {
    let value = value.trim().to_string();
    if value.chars().count() > max {
        return Err(HostError::validation(format!(
            "{field} exceeds {max} characters"
        )));
    }
    Ok(value)
}

fn normalize_optional(
    field: &str,
    value: Option<String>,
    max: usize,
) -> Result<Option<String>, HostError> {
    value
        .map(|value| {
            let value = value.trim().to_string();
            if value.is_empty() {
                return Ok(None);
            }
            if value.chars().count() > max {
                return Err(HostError::validation(format!(
                    "{field} exceeds {max} characters"
                )));
            }
            Ok(Some(value))
        })
        .transpose()
        .map(Option::flatten)
}

fn normalize_uuid(field: &str, value: String) -> Result<String, HostError> {
    Uuid::parse_str(value.trim())
        .map(|value| value.to_string())
        .map_err(|_| HostError::validation(format!("{field} must be a UUID")))
}

fn normalize_tags(tags: Vec<String>) -> Result<Vec<String>, HostError> {
    if tags.len() > MAX_TAGS {
        return Err(HostError::validation(format!(
            "tags cannot contain more than {MAX_TAGS} entries"
        )));
    }
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(tags.len());
    for tag in tags {
        let tag = normalize_required("tag", tag, 1, MAX_TAG_CHARS)?;
        let identity = tag.to_lowercase();
        if seen.insert(identity) {
            normalized.push(tag);
        }
    }
    Ok(normalized)
}

fn command_fingerprint(command: &NormalizedCaseCommand) -> Result<String, HostError> {
    let meta = command.meta();
    let context = serde_json::json!({
        "actorId": meta.context.actor_id,
        "accountId": meta.context.account_id,
        "projectId": meta.context.project_id,
    });
    let value = match command {
        NormalizedCaseCommand::Create { payload, .. } => serde_json::json!({
            "commandType": command.command_type(),
            "protocolVersion": meta.protocol_version,
            "context": context,
            "expectedRevision": meta.expected_revision,
            "payload": payload,
        }),
        NormalizedCaseCommand::Update { payload, .. } => serde_json::json!({
            "commandType": command.command_type(),
            "protocolVersion": meta.protocol_version,
            "context": context,
            "expectedRevision": meta.expected_revision,
            "payload": payload,
        }),
    };
    let bytes = serde_json::to_vec(&value).map_err(json_error)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn validate_deadline(deadline_at: Option<i64>) -> Result<(), HostError> {
    if deadline_at.is_some_and(|deadline| deadline < now_millis()) {
        return Err(HostError::new(
            "COMMAND_DEADLINE_EXCEEDED",
            "case command deadline has elapsed",
            false,
        ));
    }
    Ok(())
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
) -> Result<Option<CaseCommandResponse>, HostError> {
    let by_key = load_receipt_by_key(connection, idempotency_key)?;
    let by_command = load_receipt_by_command(connection, command_id)?;

    if by_key
        .as_ref()
        .is_some_and(|receipt| receipt.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "IDEMPOTENCY_KEY_REUSED",
            "idempotencyKey reused for a different case request",
            false,
        ));
    }
    if by_command
        .as_ref()
        .is_some_and(|receipt| receipt.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "COMMAND_ID_REUSED",
            "commandId reused for a different case request",
            false,
        ));
    }
    if let (Some(key_receipt), Some(command_receipt)) = (&by_key, &by_command) {
        if key_receipt.command_id != command_receipt.command_id
            || key_receipt.idempotency_key != command_receipt.idempotency_key
        {
            return Err(HostError::new(
                "COMMAND_IDENTITY_COLLISION",
                "commandId and idempotencyKey identify different committed case commands",
                false,
            ));
        }
    }

    by_key
        .or(by_command)
        .map(|receipt| {
            let mut response: CaseCommandResponse =
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
             FROM case_command_receipts WHERE idempotency_key = ?1",
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
             FROM case_command_receipts WHERE command_id = ?1",
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

fn create_case(
    transaction: &Transaction<'_>,
    payload: &CreateCasePayload,
) -> Result<CaseRecord, HostError> {
    ensure_project_exists(transaction, payload.project_id.as_deref())?;
    ensure_asset_ready(
        transaction,
        &payload.asset_id,
        payload.project_id.as_deref(),
    )?;
    let now = now_millis();
    let case_record = CaseRecord {
        id: Uuid::new_v4().to_string(),
        asset_id: payload.asset_id.clone(),
        project_id: payload.project_id.clone(),
        title: payload.title.clone(),
        client_name: payload.client_name.clone(),
        content_type: payload.content_type.clone(),
        presentation: payload.presentation.clone(),
        has_actors: payload.has_actors,
        is_aigc: payload.is_aigc,
        quality_tier: payload.quality_tier.clone(),
        tags: payload.tags.clone(),
        notes: payload.notes.clone(),
        revision: 1,
        created_at: now,
        updated_at: now,
    };
    insert_case(transaction, &case_record)?;
    Ok(case_record)
}

fn update_case(
    transaction: &Transaction<'_>,
    payload: &UpdateCasePayload,
    expected_revision: i64,
    context_project_id: Option<&str>,
) -> Result<CaseRecord, HostError> {
    let current = find_case(transaction, &payload.case_id)?
        .ok_or_else(|| HostError::new("CASE_NOT_FOUND", "case record does not exist", false))?;
    if current.revision != expected_revision {
        return Err(HostError::conflict(format!(
            "case {} revision is {}, request expected {}",
            current.id, current.revision, expected_revision
        )));
    }
    if context_project_id != current.project_id.as_deref() {
        return Err(HostError::new(
            "CASE_PROJECT_MISMATCH",
            "case record belongs to a different project",
            false,
        ));
    }
    ensure_project_exists(transaction, current.project_id.as_deref())?;
    ensure_asset_ready(
        transaction,
        &current.asset_id,
        current.project_id.as_deref(),
    )?;

    let tags_json = serde_json::to_string(&payload.tags).map_err(json_error)?;
    let updated_at = now_millis();
    let changed = transaction
        .execute(
            "UPDATE cases SET title = ?1, client_name = ?2, content_type = ?3,
                    presentation = ?4, has_actors = ?5, is_aigc = ?6,
                    quality_tier = ?7, tags_json = ?8, notes = ?9,
                    revision = revision + 1, updated_at = ?10
             WHERE id = ?11 AND revision = ?12",
            params![
                payload.title,
                payload.client_name,
                content_type_to_db(&payload.content_type),
                presentation_to_db(&payload.presentation),
                bool_to_db(payload.has_actors),
                bool_to_db(payload.is_aigc),
                quality_tier_to_db(&payload.quality_tier),
                tags_json,
                payload.notes,
                updated_at,
                payload.case_id,
                expected_revision,
            ],
        )
        .map_err(sql_error)?;
    if changed != 1 {
        return Err(HostError::conflict(format!(
            "case {} changed during update",
            payload.case_id
        )));
    }
    find_case(transaction, &payload.case_id)?
        .ok_or_else(|| HostError::internal("updated case record could not be loaded"))
}

fn ensure_project_exists(
    connection: &Connection,
    project_id: Option<&str>,
) -> Result<(), HostError> {
    let Some(project_id) = project_id else {
        return Ok(());
    };
    let exists = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
            [project_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sql_error)?;
    if exists {
        Ok(())
    } else {
        Err(HostError::new(
            "PROJECT_NOT_FOUND",
            "case project does not exist",
            false,
        ))
    }
}

fn ensure_asset_ready(
    connection: &Connection,
    asset_id: &str,
    expected_project_id: Option<&str>,
) -> Result<(), HostError> {
    let asset: Option<(Option<String>, String)> = connection
        .query_row(
            "SELECT project_id, status FROM assets WHERE id = ?1",
            [asset_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sql_error)?;
    let (asset_project_id, status) =
        asset.ok_or_else(|| HostError::new("ASSET_NOT_FOUND", "asset does not exist", false))?;
    if status != "ready" {
        return Err(HostError::new(
            "ASSET_NOT_READY",
            "case asset is not ready",
            false,
        ));
    }
    if asset_project_id.as_deref() != expected_project_id {
        return Err(HostError::new(
            "CASE_ASSET_PROJECT_MISMATCH",
            "asset projectId must exactly match case projectId",
            false,
        ));
    }
    Ok(())
}

fn insert_case(transaction: &Transaction<'_>, case_record: &CaseRecord) -> Result<(), HostError> {
    let tags_json = serde_json::to_string(&case_record.tags).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO cases
             (id, asset_id, project_id, title, client_name, content_type, presentation,
              has_actors, is_aigc, quality_tier, tags_json, notes, revision,
              created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                case_record.id,
                case_record.asset_id,
                case_record.project_id,
                case_record.title,
                case_record.client_name,
                content_type_to_db(&case_record.content_type),
                presentation_to_db(&case_record.presentation),
                bool_to_db(case_record.has_actors),
                bool_to_db(case_record.is_aigc),
                quality_tier_to_db(&case_record.quality_tier),
                tags_json,
                case_record.notes,
                case_record.revision,
                case_record.created_at,
                case_record.updated_at,
            ],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn find_case(connection: &Connection, case_id: &str) -> Result<Option<CaseRecord>, HostError> {
    connection
        .query_row(
            "SELECT id, asset_id, project_id, title, client_name, content_type,
                    presentation, has_actors, is_aigc, quality_tier, tags_json, notes,
                    revision, created_at, updated_at
             FROM cases WHERE id = ?1",
            [case_id],
            case_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn case_from_row(row: &Row<'_>) -> rusqlite::Result<CaseRecord> {
    let content_type_value: String = row.get(5)?;
    let presentation_value: String = row.get(6)?;
    let quality_tier_value: String = row.get(9)?;
    let tags_json: String = row.get(10)?;
    Ok(CaseRecord {
        id: row.get(0)?,
        asset_id: row.get(1)?,
        project_id: row.get(2)?,
        title: row.get(3)?,
        client_name: row.get(4)?,
        content_type: content_type_from_db(&content_type_value)
            .ok_or_else(|| conversion_error("content_type", &content_type_value))?,
        presentation: presentation_from_db(&presentation_value)
            .ok_or_else(|| conversion_error("presentation", &presentation_value))?,
        has_actors: bool_from_db("has_actors", row.get(7)?)?,
        is_aigc: bool_from_db("is_aigc", row.get(8)?)?,
        quality_tier: quality_tier_from_db(&quality_tier_value)
            .ok_or_else(|| conversion_error("quality_tier", &quality_tier_value))?,
        tags: serde_json::from_str(&tags_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                tags_json.len(),
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        notes: row.get(11)?,
        revision: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn append_event(
    transaction: &Transaction<'_>,
    event_type: CaseEventType,
    case_record: &CaseRecord,
    trace_id: &str,
) -> Result<CaseDomainEvent, HostError> {
    let event_id = Uuid::new_v4().to_string();
    let occurred_at = now_millis();
    let payload_json = serde_json::to_string(case_record).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO case_events
             (event_id, event_type, aggregate_id, revision, occurred_at, trace_id, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event_id,
                event_type_to_wire(&event_type),
                case_record.id,
                case_record.revision,
                occurred_at,
                trace_id,
                payload_json,
            ],
        )
        .map_err(sql_error)?;
    Ok(CaseDomainEvent {
        sequence: transaction.last_insert_rowid(),
        event_id,
        event_type,
        aggregate_id: case_record.id.clone(),
        revision: case_record.revision,
        occurred_at,
        trace_id: trace_id.to_string(),
        case_record: case_record.clone(),
    })
}

fn load_event(connection: &Connection, sequence: i64) -> Result<CaseDomainEvent, HostError> {
    connection
        .query_row(
            "SELECT sequence, event_id, event_type, aggregate_id, revision,
                    occurred_at, trace_id, payload_json
             FROM case_events WHERE sequence = ?1",
            [sequence],
            event_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::internal("committed case event could not be recovered"))
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<CaseDomainEvent> {
    let event_type_value: String = row.get(2)?;
    let payload_json: String = row.get(7)?;
    Ok(CaseDomainEvent {
        sequence: row.get(0)?,
        event_id: row.get(1)?,
        event_type: event_type_from_wire(&event_type_value)
            .ok_or_else(|| conversion_error("event_type", &event_type_value))?,
        aggregate_id: row.get(3)?,
        revision: row.get(4)?,
        occurred_at: row.get(5)?,
        trace_id: row.get(6)?,
        case_record: serde_json::from_str(&payload_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                payload_json.len(),
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
    })
}

fn content_type_to_db(value: &CaseContentType) -> &'static str {
    match value {
        CaseContentType::Brand => "brand",
        CaseContentType::Property => "property",
        CaseContentType::Interview => "interview",
        CaseContentType::Lifestyle => "lifestyle",
        CaseContentType::Product => "product",
        CaseContentType::Event => "event",
        CaseContentType::Documentary => "documentary",
        CaseContentType::Narrative => "narrative",
        CaseContentType::Other => "other",
    }
}

fn content_type_from_db(value: &str) -> Option<CaseContentType> {
    Some(match value {
        "brand" => CaseContentType::Brand,
        "property" => CaseContentType::Property,
        "interview" => CaseContentType::Interview,
        "lifestyle" => CaseContentType::Lifestyle,
        "product" => CaseContentType::Product,
        "event" => CaseContentType::Event,
        "documentary" => CaseContentType::Documentary,
        "narrative" => CaseContentType::Narrative,
        "other" => CaseContentType::Other,
        _ => return None,
    })
}

fn presentation_to_db(value: &CasePresentation) -> &'static str {
    match value {
        CasePresentation::LiveAction => "liveAction",
        CasePresentation::Animation => "animation",
        CasePresentation::MixedMedia => "mixedMedia",
        CasePresentation::Aigc => "aigc",
        CasePresentation::Graphic => "graphic",
        CasePresentation::Other => "other",
    }
}

fn presentation_from_db(value: &str) -> Option<CasePresentation> {
    Some(match value {
        "liveAction" => CasePresentation::LiveAction,
        "animation" => CasePresentation::Animation,
        "mixedMedia" => CasePresentation::MixedMedia,
        "aigc" => CasePresentation::Aigc,
        "graphic" => CasePresentation::Graphic,
        "other" => CasePresentation::Other,
        _ => return None,
    })
}

fn quality_tier_to_db(value: &CaseQualityTier) -> &'static str {
    match value {
        CaseQualityTier::Reference => "reference",
        CaseQualityTier::Featured => "featured",
        CaseQualityTier::Premium => "premium",
    }
}

fn quality_tier_from_db(value: &str) -> Option<CaseQualityTier> {
    Some(match value {
        "reference" => CaseQualityTier::Reference,
        "featured" => CaseQualityTier::Featured,
        "premium" => CaseQualityTier::Premium,
        _ => return None,
    })
}

fn event_type_to_wire(value: &CaseEventType) -> &'static str {
    match value {
        CaseEventType::Created => "case.created",
        CaseEventType::Updated => "case.updated",
    }
}

fn event_type_from_wire(value: &str) -> Option<CaseEventType> {
    Some(match value {
        "case.created" => CaseEventType::Created,
        "case.updated" => CaseEventType::Updated,
        _ => return None,
    })
}

fn bool_to_db(value: bool) -> i64 {
    i64::from(value)
}

fn bool_from_db(field: &str, value: i64) -> rusqlite::Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(conversion_error(field, &value.to_string())),
    }
}

fn conversion_error(field: &str, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        value.len(),
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid case {field} database value: {value}"),
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
    HostError::internal(format!("case SQLite operation failed: {error}"))
}

fn json_error(error: serde_json::Error) -> HostError {
    HostError::internal(format!("case JSON operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT_A: &str = "project-a";
    const PROJECT_B: &str = "project-b";
    const SECRET_PATH: &str = r"C:\fixture-private\secret\source.mp4";

    fn database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys = ON;
                CREATE TABLE projects (
                    id TEXT PRIMARY KEY NOT NULL
                );
                CREATE TABLE assets (
                    id TEXT PRIMARY KEY NOT NULL,
                    project_id TEXT,
                    status TEXT NOT NULL,
                    storage_rel_path TEXT NOT NULL
                );
                INSERT INTO projects (id) VALUES ('project-a'), ('project-b');
                "#,
            )
            .unwrap();
        migrate(&connection).unwrap();
        connection
    }

    fn seed_asset(
        connection: &Connection,
        project_id: Option<&str>,
        status: &str,
        storage_path: &str,
    ) -> String {
        let asset_id = Uuid::new_v4().to_string();
        connection
            .execute(
                "INSERT INTO assets (id, project_id, status, storage_rel_path)
                 VALUES (?1, ?2, ?3, ?4)",
                params![asset_id, project_id, status, storage_path],
            )
            .unwrap();
        asset_id
    }

    fn context(project_id: Option<&str>) -> OperationContext {
        OperationContext {
            actor_id: " operator ".to_string(),
            account_id: Some("account-1".to_string()),
            project_id: project_id.map(str::to_string),
            window_id: "window-main".to_string(),
            trace_id: format!("trace-{}", Uuid::new_v4()),
        }
    }

    fn create_command(
        command_id: &str,
        idempotency_key: &str,
        asset_id: &str,
        project_id: Option<&str>,
        deadline_at: Option<i64>,
    ) -> CaseCommandEnvelope {
        CaseCommandEnvelope::Create {
            command_id: command_id.to_string(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: CreateCasePayload {
                asset_id: asset_id.to_string(),
                project_id: project_id.map(str::to_string),
                title: "  Riverside launch film  ".to_string(),
                client_name: "  Client X  ".to_string(),
                content_type: CaseContentType::Brand,
                presentation: CasePresentation::MixedMedia,
                has_actors: true,
                is_aigc: true,
                quality_tier: CaseQualityTier::Premium,
                tags: vec![
                    " Launch ".to_string(),
                    "launch".to_string(),
                    "AIGC".to_string(),
                ],
                notes: "  approved reference  ".to_string(),
            },
            idempotency_key: idempotency_key.to_string(),
            expected_revision: None,
            deadline_at,
        }
    }

    fn update_command(
        command_id: &str,
        idempotency_key: &str,
        case_id: &str,
        expected_revision: i64,
        project_id: Option<&str>,
    ) -> CaseCommandEnvelope {
        CaseCommandEnvelope::Update {
            command_id: command_id.to_string(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: UpdateCasePayload {
                case_id: case_id.to_string(),
                title: "Updated campaign film".to_string(),
                client_name: "Client X".to_string(),
                content_type: CaseContentType::Lifestyle,
                presentation: CasePresentation::LiveAction,
                has_actors: false,
                is_aigc: false,
                quality_tier: CaseQualityTier::Featured,
                tags: vec!["Lifestyle".to_string(), " lifestyle ".to_string()],
                notes: "new edit".to_string(),
            },
            idempotency_key: idempotency_key.to_string(),
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
    fn create_update_list_replay_and_idempotency_are_durable() {
        let mut connection = database();
        let asset_id = seed_asset(
            &connection,
            Some(PROJECT_A),
            "ready",
            "project-a/assets/source.mp4",
        );
        let create = create_command(
            &Uuid::new_v4().to_string(),
            "case-create-001",
            &asset_id,
            Some(PROJECT_A),
            None,
        );

        let created = execute_command(&mut connection, create.clone()).unwrap();
        assert!(!created.response.replayed);
        assert_eq!(created.response.case_record.revision, 1);
        assert_eq!(created.response.case_record.title, "Riverside launch film");
        assert_eq!(created.response.case_record.client_name, "Client X");
        assert_eq!(created.response.case_record.tags, vec!["Launch", "AIGC"]);
        assert_eq!(created.response.case_record.notes, "approved reference");
        assert_eq!(created.emitted_events.len(), 1);
        assert_eq!(created.emitted_events[0].event_type, CaseEventType::Created);

        let replayed = execute_command(&mut connection, create).unwrap();
        assert!(replayed.response.replayed);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(replayed.response.case_record, created.response.case_record);

        let updated = execute_command(
            &mut connection,
            update_command(
                &Uuid::new_v4().to_string(),
                "case-update-001",
                &created.response.case_record.id,
                1,
                Some(PROJECT_A),
            ),
        )
        .unwrap();
        assert_eq!(updated.response.case_record.revision, 2);
        assert_eq!(updated.response.case_record.tags, vec!["Lifestyle"]);
        assert_eq!(updated.emitted_events[0].event_type, CaseEventType::Updated);

        let records = list(&connection).unwrap();
        assert_eq!(records, vec![updated.response.case_record.clone()]);
        let events = replay_events(&connection, 0, 100).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].revision, 1);
        assert_eq!(events[1].revision, 2);
        assert_eq!(
            replay_events(&connection, events[0].sequence, 1)
                .unwrap()
                .len(),
            1
        );

        let stored: (String, String) = connection
            .query_row(
                "SELECT content_type, tags_json FROM cases WHERE id = ?1",
                [&created.response.case_record.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(stored.0, "lifestyle");
        assert_eq!(stored.1, r#"["Lifestyle"]"#);
    }

    #[test]
    fn protocol_compatibility_is_bounded_and_1_2_receipts_replay() {
        let mut connection = database();
        let asset_id = seed_asset(
            &connection,
            Some(PROJECT_A),
            "ready",
            "project-a/assets/legacy.mp4",
        );
        let mut legacy = create_command(
            &Uuid::new_v4().to_string(),
            "case-create-protocol-1-2",
            &asset_id,
            Some(PROJECT_A),
            None,
        );
        if let CaseCommandEnvelope::Create {
            protocol_version, ..
        } = &mut legacy
        {
            *protocol_version = LEGACY_PROTOCOL_VERSION.to_string();
        }

        let committed = execute_command(&mut connection, legacy.clone()).unwrap();
        let replayed = execute_command(&mut connection, legacy).unwrap();

        assert!(!committed.response.replayed);
        assert!(replayed.response.replayed);
        assert_eq!(replayed.response.receipt, committed.response.receipt);
        assert_eq!(
            replayed.response.case_record,
            committed.response.case_record
        );
        assert!(replayed.emitted_events.is_empty());
        let stored_version: String = connection
            .query_row(
                "SELECT protocol_version FROM case_command_receipts",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_version, LEGACY_PROTOCOL_VERSION);

        for (supported_version, version_label) in [
            (PROTOCOL_1_3_VERSION, "1-3"),
            (PREVIOUS_PROTOCOL_VERSION, "1-4"),
            (PROTOCOL_VERSION, "1-5"),
        ] {
            let mut supported = create_command(
                &Uuid::new_v4().to_string(),
                &format!("case-create-protocol-{version_label}"),
                &asset_id,
                Some(PROJECT_A),
                None,
            );
            if let CaseCommandEnvelope::Create {
                protocol_version, ..
            } = &mut supported
            {
                *protocol_version = supported_version.to_string();
            }
            execute_command(&mut connection, supported).unwrap();
        }

        for unsupported_version in ["1.1", "1.6"] {
            let mut unsupported = create_command(
                &Uuid::new_v4().to_string(),
                &format!("case-create-protocol-{unsupported_version}"),
                &asset_id,
                Some(PROJECT_A),
                None,
            );
            if let CaseCommandEnvelope::Create {
                protocol_version, ..
            } = &mut unsupported
            {
                *protocol_version = unsupported_version.to_string();
            }
            let error = execute_command(&mut connection, unsupported).unwrap_err();
            assert_eq!(error.code, "PROTOCOL_VERSION_UNSUPPORTED");
            assert_eq!(
                error.message,
                format!(
                    "expected protocolVersion {LEGACY_PROTOCOL_VERSION}, {PROTOCOL_1_3_VERSION}, {PREVIOUS_PROTOCOL_VERSION}, or {PROTOCOL_VERSION}, received {unsupported_version}"
                )
            );
        }
        assert_eq!(table_count(&connection, "cases"), 4);
        assert_eq!(table_count(&connection, "case_events"), 4);
        assert_eq!(table_count(&connection, "case_command_receipts"), 4);
    }
    #[test]
    fn receipt_lookup_precedes_deadline_and_asset_validation() {
        let mut connection = database();
        let asset_id = seed_asset(
            &connection,
            Some(PROJECT_A),
            "ready",
            "project-a/assets/source.mp4",
        );
        let command_id = Uuid::new_v4().to_string();
        let command = create_command(
            &command_id,
            "case-deadline-001",
            &asset_id,
            Some(PROJECT_A),
            Some(now_millis() + 60_000),
        );
        let committed = execute_command(&mut connection, command).unwrap();
        connection
            .execute(
                "UPDATE assets SET status = 'failed', project_id = ?1 WHERE id = ?2",
                params![PROJECT_B, asset_id],
            )
            .unwrap();
        let expired_retry = create_command(
            &command_id,
            "case-deadline-001",
            &asset_id,
            Some(PROJECT_A),
            Some(now_millis() - 1),
        );

        let replayed = execute_command(&mut connection, expired_retry).unwrap();
        assert!(replayed.response.replayed);
        assert_eq!(
            replayed.response.case_record,
            committed.response.case_record
        );
        assert!(replayed.emitted_events.is_empty());
    }

    #[test]
    fn command_and_idempotency_fingerprint_reuse_are_rejected() {
        let mut connection = database();
        let asset_id = seed_asset(
            &connection,
            Some(PROJECT_A),
            "ready",
            "project-a/assets/source.mp4",
        );
        let command_id = Uuid::new_v4().to_string();
        execute_command(
            &mut connection,
            create_command(
                &command_id,
                "case-fingerprint-001",
                &asset_id,
                Some(PROJECT_A),
                None,
            ),
        )
        .unwrap();

        let mut reused_key = create_command(
            &Uuid::new_v4().to_string(),
            "case-fingerprint-001",
            &asset_id,
            Some(PROJECT_A),
            None,
        );
        if let CaseCommandEnvelope::Create { payload, .. } = &mut reused_key {
            payload.title = "Different request".to_string();
        }
        assert_eq!(
            execute_command(&mut connection, reused_key)
                .unwrap_err()
                .code,
            "IDEMPOTENCY_KEY_REUSED"
        );

        let mut reused_command = create_command(
            &command_id,
            "case-fingerprint-002",
            &asset_id,
            Some(PROJECT_A),
            None,
        );
        if let CaseCommandEnvelope::Create { payload, .. } = &mut reused_command {
            payload.title = "Another request".to_string();
        }
        assert_eq!(
            execute_command(&mut connection, reused_command)
                .unwrap_err()
                .code,
            "COMMAND_ID_REUSED"
        );
        assert_eq!(table_count(&connection, "cases"), 1);
        assert_eq!(table_count(&connection, "case_events"), 1);
        assert_eq!(table_count(&connection, "case_command_receipts"), 1);
    }

    #[test]
    fn cross_project_asset_is_rejected_without_partial_commit() {
        let mut connection = database();
        let asset_id = seed_asset(
            &connection,
            Some(PROJECT_A),
            "ready",
            "project-a/assets/source.mp4",
        );
        let error = execute_command(
            &mut connection,
            create_command(
                &Uuid::new_v4().to_string(),
                "case-project-001",
                &asset_id,
                Some(PROJECT_B),
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "CASE_ASSET_PROJECT_MISMATCH");
        assert_eq!(table_count(&connection, "cases"), 0);
        assert_eq!(table_count(&connection, "case_events"), 0);
        assert_eq!(table_count(&connection, "case_command_receipts"), 0);
    }

    #[test]
    fn nonexistent_project_is_rejected_even_when_asset_claims_it() {
        let mut connection = database();
        let asset_id = seed_asset(
            &connection,
            Some("missing-project"),
            "ready",
            "missing-project/assets/source.mp4",
        );
        let error = execute_command(
            &mut connection,
            create_command(
                &Uuid::new_v4().to_string(),
                "case-project-missing-001",
                &asset_id,
                Some("missing-project"),
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "PROJECT_NOT_FOUND");
        assert_eq!(table_count(&connection, "cases"), 0);
        assert_eq!(table_count(&connection, "case_events"), 0);
        assert_eq!(table_count(&connection, "case_command_receipts"), 0);
    }

    #[test]
    fn project_context_must_exactly_match_case_ownership() {
        let mut connection = database();
        let asset_id = seed_asset(
            &connection,
            Some(PROJECT_A),
            "ready",
            "project-a/assets/source.mp4",
        );
        let mut command = create_command(
            &Uuid::new_v4().to_string(),
            "case-context-001",
            &asset_id,
            Some(PROJECT_A),
            None,
        );
        if let CaseCommandEnvelope::Create { context, .. } = &mut command {
            context.project_id = None;
        }
        let error = execute_command(&mut connection, command).unwrap_err();
        assert_eq!(error.code, "VALIDATION_FAILED");
        assert_eq!(table_count(&connection, "cases"), 0);
    }

    #[test]
    fn cas_failure_rolls_back_and_same_command_can_retry() {
        let mut connection = database();
        let asset_id = seed_asset(
            &connection,
            Some(PROJECT_A),
            "ready",
            "project-a/assets/source.mp4",
        );
        let created = execute_command(
            &mut connection,
            create_command(
                &Uuid::new_v4().to_string(),
                "case-cas-create-001",
                &asset_id,
                Some(PROJECT_A),
                None,
            ),
        )
        .unwrap();
        let update_id = Uuid::new_v4().to_string();
        let failed = update_command(
            &update_id,
            "case-cas-update-001",
            &created.response.case_record.id,
            99,
            Some(PROJECT_A),
        );
        assert_eq!(
            execute_command(&mut connection, failed).unwrap_err().code,
            "REVISION_CONFLICT"
        );
        assert_eq!(table_count(&connection, "case_events"), 1);
        assert_eq!(table_count(&connection, "case_command_receipts"), 1);
        let unchanged = list(&connection).unwrap().pop().unwrap();
        assert_eq!(unchanged.revision, 1);
        assert_eq!(unchanged.title, "Riverside launch film");

        let succeeded = execute_command(
            &mut connection,
            update_command(
                &update_id,
                "case-cas-update-001",
                &created.response.case_record.id,
                1,
                Some(PROJECT_A),
            ),
        )
        .unwrap();
        assert_eq!(succeeded.response.case_record.revision, 2);
        assert_eq!(table_count(&connection, "case_events"), 2);
        assert_eq!(table_count(&connection, "case_command_receipts"), 2);
    }

    #[test]
    fn asset_storage_path_never_enters_wire_records() {
        let mut connection = database();
        let asset_id = seed_asset(&connection, None, "ready", SECRET_PATH);
        let outcome = execute_command(
            &mut connection,
            create_command(
                &Uuid::new_v4().to_string(),
                "case-wire-path-001",
                &asset_id,
                None,
                None,
            ),
        )
        .unwrap();
        let records = list(&connection).unwrap();
        let events = replay_events(&connection, 0, 100).unwrap();
        let response_json = serde_json::to_string(&outcome.response).unwrap();
        let records_json = serde_json::to_string(&records).unwrap();
        let events_json = serde_json::to_string(&events).unwrap();
        let (stored_response, stored_event): (String, String) = connection
            .query_row(
                "SELECT r.response_json, e.payload_json
                 FROM case_command_receipts r JOIN case_events e
                   ON e.sequence = json_extract(r.response_json, '$.receipt.lastEventSequence')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        for wire in [
            response_json,
            records_json,
            events_json,
            stored_response,
            stored_event,
        ] {
            assert!(!wire.contains(SECRET_PATH));
            assert!(!wire.contains("storage_rel_path"));
            assert!(!wire.contains("storageRelPath"));
        }
    }

    #[test]
    fn invalid_inputs_and_replay_cursor_are_bounded() {
        let mut connection = database();
        let asset_id = seed_asset(&connection, None, "ready", "global/source.mp4");
        let mut too_many_tags = create_command(
            &Uuid::new_v4().to_string(),
            "case-bounds-001",
            &asset_id,
            None,
            None,
        );
        if let CaseCommandEnvelope::Create { payload, .. } = &mut too_many_tags {
            payload.tags = (0..=MAX_TAGS).map(|index| format!("tag-{index}")).collect();
        }
        assert_eq!(
            execute_command(&mut connection, too_many_tags)
                .unwrap_err()
                .code,
            "VALIDATION_FAILED"
        );
        assert_eq!(
            replay_events(&connection, -1, 100).unwrap_err().code,
            "VALIDATION_FAILED"
        );
        assert_eq!(table_count(&connection, "cases"), 0);
    }
}
