use crate::protocol::{
    is_legacy_surface_protocol_supported, ChangeExecutionBriefStatusPayload, CommandReceipt,
    CreateExecutionBriefPayload, ExecutionBriefCommandEnvelope, ExecutionBriefCommandResponse,
    ExecutionBriefContent, ExecutionBriefDomainEvent, ExecutionBriefEventType,
    ExecutionBriefRecord, ExecutionBriefStatus, HostError, OperationContext,
    UpdateExecutionBriefPayload, LEGACY_PROTOCOL_VERSION, PREVIOUS_PROTOCOL_VERSION,
    PROTOCOL_1_3_VERSION, PROTOCOL_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_TEXT_CHARS: usize = 8_000;
const MAX_LIST_ITEMS: usize = 100;
const MAX_LIST_ITEM_CHARS: usize = 500;
const MAX_CONTEXT_CHARS: usize = 160;

#[derive(Debug)]
pub struct ExecutionBriefCommandOutcome {
    pub response: ExecutionBriefCommandResponse,
    pub emitted_events: Vec<ExecutionBriefDomainEvent>,
}

pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS execution_briefs (
                id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT NOT NULL UNIQUE,
                content_json TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('draft','ready')),
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_execution_briefs_updated
                ON execution_briefs(updated_at DESC, id DESC);
            CREATE TABLE IF NOT EXISTS execution_brief_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL CHECK(event_type IN
                    ('executionBrief.created','executionBrief.updated',
                     'executionBrief.statusChanged')),
                aggregate_id TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                occurred_at INTEGER NOT NULL,
                trace_id TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                FOREIGN KEY(aggregate_id) REFERENCES execution_briefs(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_execution_brief_events_aggregate
                ON execution_brief_events(aggregate_id, sequence);
            CREATE TABLE IF NOT EXISTS execution_brief_command_receipts (
                idempotency_key TEXT PRIMARY KEY NOT NULL,
                command_id TEXT NOT NULL UNIQUE,
                command_type TEXT NOT NULL CHECK(command_type IN
                    ('executionBrief.create','executionBrief.update',
                     'executionBrief.changeStatus')),
                protocol_version TEXT NOT NULL,
                deadline_at INTEGER,
                request_fingerprint TEXT NOT NULL CHECK(length(request_fingerprint) = 64),
                response_json TEXT NOT NULL,
                completed_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_execution_brief_receipts_completed
                ON execution_brief_command_receipts(completed_at);
            "#,
        )
        .map_err(sql_error)
}

pub fn execute_command(
    connection: &mut Connection,
    command: ExecutionBriefCommandEnvelope,
) -> Result<ExecutionBriefCommandOutcome, HostError> {
    let command = normalize_command(command)?;
    let fingerprint = command_fingerprint(&command)?;
    let meta = command.meta();
    let command_type = command.command_type();
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
        return Ok(ExecutionBriefCommandOutcome {
            response,
            emitted_events: Vec::new(),
        });
    }

    validate_deadline(meta.deadline_at)?;
    let (record, event_type) = match &command {
        NormalizedCommand::Create { payload, .. } => (
            create_execution_brief(&transaction, payload)?,
            ExecutionBriefEventType::Created,
        ),
        NormalizedCommand::Update { payload, meta } => (
            update_execution_brief(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized update revision"),
                &meta.context.project_id,
            )?,
            ExecutionBriefEventType::Updated,
        ),
        NormalizedCommand::ChangeStatus { payload, meta } => (
            change_status(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized status revision"),
                &meta.context.project_id,
            )?,
            ExecutionBriefEventType::StatusChanged,
        ),
    };
    let event = append_event(&transaction, event_type, &record, &meta.context.trace_id)?;
    let completed_at = now_millis();
    let response = ExecutionBriefCommandResponse {
        receipt: CommandReceipt {
            command_id: meta.command_id.clone(),
            idempotency_key: meta.idempotency_key.clone(),
            command_type: command_type.to_string(),
            aggregate_id: record.id.clone(),
            revision: record.revision,
            last_event_sequence: event.sequence,
            completed_at,
        },
        execution_brief: record,
        replayed: false,
    };
    transaction
        .execute(
            "INSERT INTO execution_brief_command_receipts
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
                serde_json::to_string(&response).map_err(json_error)?,
                completed_at,
            ],
        )
        .map_err(sql_error)?;
    match transaction.commit() {
        Ok(()) => Ok(ExecutionBriefCommandOutcome {
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
                Ok(ExecutionBriefCommandOutcome {
                    response: persisted,
                    emitted_events: vec![persisted_event],
                })
            }
            Ok(None) => Err(sql_error(error)),
            Err(lookup_error) => Err(lookup_error),
        },
    }
}

pub fn list(connection: &Connection) -> Result<Vec<ExecutionBriefRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, project_id, content_json, status, revision, created_at, updated_at
             FROM execution_briefs ORDER BY updated_at DESC, id DESC",
        )
        .map_err(sql_error)?;
    let records = statement
        .query_map([], execution_brief_from_row)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(records)
}

pub fn replay_events(
    connection: &Connection,
    after_sequence: i64,
    limit: u32,
) -> Result<Vec<ExecutionBriefDomainEvent>, HostError> {
    if after_sequence < 0 {
        return Err(HostError::validation("afterSequence cannot be negative"));
    }
    let mut statement = connection
        .prepare(
            "SELECT sequence, event_id, event_type, aggregate_id, revision,
                    occurred_at, trace_id, payload_json
             FROM execution_brief_events
             WHERE sequence > ?1 ORDER BY sequence ASC LIMIT ?2",
        )
        .map_err(sql_error)?;
    let events = statement
        .query_map(
            params![after_sequence, limit.clamp(1, 1_000)],
            event_from_row,
        )
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(events)
}

#[derive(Debug, Clone)]
struct NormalizedContext {
    actor_id: String,
    account_id: Option<String>,
    project_id: String,
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
enum NormalizedCommand {
    Create {
        meta: CommandMeta,
        payload: CreateExecutionBriefPayload,
    },
    Update {
        meta: CommandMeta,
        payload: UpdateExecutionBriefPayload,
    },
    ChangeStatus {
        meta: CommandMeta,
        payload: ChangeExecutionBriefStatusPayload,
    },
}

impl NormalizedCommand {
    fn meta(&self) -> &CommandMeta {
        match self {
            Self::Create { meta, .. }
            | Self::Update { meta, .. }
            | Self::ChangeStatus { meta, .. } => meta,
        }
    }

    fn command_type(&self) -> &'static str {
        match self {
            Self::Create { .. } => "executionBrief.create",
            Self::Update { .. } => "executionBrief.update",
            Self::ChangeStatus { .. } => "executionBrief.changeStatus",
        }
    }
}

fn normalize_command(
    command: ExecutionBriefCommandEnvelope,
) -> Result<NormalizedCommand, HostError> {
    match command {
        ExecutionBriefCommandEnvelope::Create {
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
                    "executionBrief.create rejects expectedRevision",
                ));
            }
            let payload = CreateExecutionBriefPayload {
                project_id: normalize_uuid("projectId", payload.project_id)?,
                content: normalize_content(payload.content)?,
            };
            let context = normalize_context(context)?;
            if context.project_id != payload.project_id {
                return Err(HostError::validation(
                    "context projectId must match execution brief projectId",
                ));
            }
            Ok(NormalizedCommand::Create {
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
        ExecutionBriefCommandEnvelope::Update {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            Ok(NormalizedCommand::Update {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    normalize_context(context)?,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload: UpdateExecutionBriefPayload {
                    brief_id: normalize_uuid("briefId", payload.brief_id)?,
                    content: normalize_content(payload.content)?,
                },
            })
        }
        ExecutionBriefCommandEnvelope::ChangeStatus {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            Ok(NormalizedCommand::ChangeStatus {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    normalize_context(context)?,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload: ChangeExecutionBriefStatusPayload {
                    brief_id: normalize_uuid("briefId", payload.brief_id)?,
                    status: payload.status,
                },
            })
        }
    }
}

fn validate_expected_revision(revision: Option<i64>) -> Result<(), HostError> {
    if revision.is_none_or(|value| value <= 0) {
        Err(HostError::validation(
            "execution brief mutation requires expectedRevision > 0",
        ))
    } else {
        Ok(())
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
            "PROTOCOL_VERSION_MISMATCH",
            format!(
                "expected protocolVersion {LEGACY_PROTOCOL_VERSION}, {PROTOCOL_1_3_VERSION}, {PREVIOUS_PROTOCOL_VERSION}, or {PROTOCOL_VERSION}"
            ),
            false,
        ));
    }
    let command_id = normalize_uuid("commandId", command_id)?;
    let idempotency_key = idempotency_key.trim().to_string();
    if !(8..=160).contains(&idempotency_key.chars().count()) {
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
    let project_id = context
        .project_id
        .ok_or_else(|| HostError::validation("execution brief context requires projectId"))?;
    normalize_required("windowId", context.window_id, MAX_CONTEXT_CHARS)?;
    Ok(NormalizedContext {
        actor_id: normalize_required("actorId", context.actor_id, MAX_CONTEXT_CHARS)?,
        account_id: normalize_optional("accountId", context.account_id, MAX_CONTEXT_CHARS)?,
        project_id: normalize_uuid("projectId", project_id)?,
        trace_id: normalize_required("traceId", context.trace_id, MAX_CONTEXT_CHARS)?,
    })
}

fn normalize_content(content: ExecutionBriefContent) -> Result<ExecutionBriefContent, HostError> {
    if content.shoot_at.is_some_and(|value| value <= 0) {
        return Err(HostError::validation(
            "shootAt must be a positive timestamp",
        ));
    }
    Ok(ExecutionBriefContent {
        shoot_at: content.shoot_at,
        client_goal: normalize_text("clientGoal", content.client_goal)?,
        visual_style: normalize_text("visualStyle", content.visual_style)?,
        primary_shots: normalize_list("primaryShots", content.primary_shots)?,
        secondary_shots: normalize_list("secondaryShots", content.secondary_shots)?,
        required_shots: normalize_list("requiredShots", content.required_shots)?,
        fallback_shots: normalize_list("fallbackShots", content.fallback_shots)?,
        risk_points: normalize_list("riskPoints", content.risk_points)?,
        waiting_time_actions: normalize_list("waitingTimeActions", content.waiting_time_actions)?,
        equipment_notes: normalize_text("equipmentNotes", content.equipment_notes)?,
        post_shoot_highlights: normalize_list(
            "postShootHighlights",
            content.post_shoot_highlights,
        )?,
    })
}

fn normalize_text(field: &str, value: String) -> Result<String, HostError> {
    let value = value.trim().to_string();
    if value.chars().count() > MAX_TEXT_CHARS {
        return Err(HostError::validation(format!(
            "{field} exceeds {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok(value)
}

fn normalize_list(field: &str, values: Vec<String>) -> Result<Vec<String>, HostError> {
    if values.len() > MAX_LIST_ITEMS {
        return Err(HostError::validation(format!(
            "{field} cannot contain more than {MAX_LIST_ITEMS} items"
        )));
    }
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        let value = value.trim().to_string();
        if value.is_empty() {
            continue;
        }
        if value.chars().count() > MAX_LIST_ITEM_CHARS {
            return Err(HostError::validation(format!(
                "{field} item exceeds {MAX_LIST_ITEM_CHARS} characters"
            )));
        }
        if seen.insert(value.to_lowercase()) {
            normalized.push(value);
        }
    }
    Ok(normalized)
}

fn normalize_required(field: &str, value: String, max: usize) -> Result<String, HostError> {
    let value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > max {
        return Err(HostError::validation(format!(
            "{field} length must be 1..{max}"
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
                Ok(None)
            } else if value.chars().count() > max {
                Err(HostError::validation(format!(
                    "{field} exceeds {max} characters"
                )))
            } else {
                Ok(Some(value))
            }
        })
        .transpose()
        .map(Option::flatten)
}

fn normalize_uuid(field: &str, value: String) -> Result<String, HostError> {
    Uuid::parse_str(value.trim())
        .map(|value| value.to_string())
        .map_err(|_| HostError::validation(format!("{field} must be a UUID")))
}

fn create_execution_brief(
    transaction: &Transaction<'_>,
    payload: &CreateExecutionBriefPayload,
) -> Result<ExecutionBriefRecord, HostError> {
    ensure_project_exists(transaction, &payload.project_id)?;
    if find_by_project(transaction, &payload.project_id)?.is_some() {
        return Err(HostError::new(
            "EXECUTION_BRIEF_EXISTS",
            "project already has an execution brief",
            false,
        ));
    }
    let now = now_millis();
    let record = ExecutionBriefRecord {
        id: Uuid::new_v4().to_string(),
        project_id: payload.project_id.clone(),
        content: payload.content.clone(),
        status: ExecutionBriefStatus::Draft,
        revision: 1,
        created_at: now,
        updated_at: now,
    };
    insert_record(transaction, &record)?;
    Ok(record)
}

fn update_execution_brief(
    transaction: &Transaction<'_>,
    payload: &UpdateExecutionBriefPayload,
    expected_revision: i64,
    context_project_id: &str,
) -> Result<ExecutionBriefRecord, HostError> {
    let current = load_record(transaction, &payload.brief_id)?;
    ensure_owned(&current, expected_revision, context_project_id)?;
    if current.status == ExecutionBriefStatus::Ready {
        ensure_ready(&payload.content)?;
    }
    let changed = transaction
        .execute(
            "UPDATE execution_briefs
             SET content_json = ?1, revision = revision + 1, updated_at = ?2
             WHERE id = ?3 AND revision = ?4",
            params![
                serde_json::to_string(&payload.content).map_err(json_error)?,
                now_millis(),
                payload.brief_id,
                expected_revision,
            ],
        )
        .map_err(sql_error)?;
    ensure_changed(changed)?;
    load_record(transaction, &payload.brief_id)
}

fn change_status(
    transaction: &Transaction<'_>,
    payload: &ChangeExecutionBriefStatusPayload,
    expected_revision: i64,
    context_project_id: &str,
) -> Result<ExecutionBriefRecord, HostError> {
    let current = load_record(transaction, &payload.brief_id)?;
    ensure_owned(&current, expected_revision, context_project_id)?;
    if payload.status == current.status {
        return Err(HostError::validation(
            "execution brief already has requested status",
        ));
    }
    if payload.status == ExecutionBriefStatus::Ready {
        ensure_ready(&current.content)?;
    }
    let changed = transaction
        .execute(
            "UPDATE execution_briefs
             SET status = ?1, revision = revision + 1, updated_at = ?2
             WHERE id = ?3 AND revision = ?4",
            params![
                status_to_db(&payload.status),
                now_millis(),
                payload.brief_id,
                expected_revision,
            ],
        )
        .map_err(sql_error)?;
    ensure_changed(changed)?;
    load_record(transaction, &payload.brief_id)
}

fn ensure_owned(
    record: &ExecutionBriefRecord,
    expected_revision: i64,
    context_project_id: &str,
) -> Result<(), HostError> {
    if record.project_id != context_project_id {
        return Err(HostError::new(
            "EXECUTION_BRIEF_PROJECT_MISMATCH",
            "execution brief belongs to a different project",
            false,
        ));
    }
    if record.revision != expected_revision {
        return Err(HostError::conflict(format!(
            "execution brief revision is {}, request expected {}",
            record.revision, expected_revision
        )));
    }
    Ok(())
}

fn ensure_ready(content: &ExecutionBriefContent) -> Result<(), HostError> {
    let mut missing = Vec::new();
    if content.shoot_at.is_none() {
        missing.push("shootAt");
    }
    if content.client_goal.is_empty() {
        missing.push("clientGoal");
    }
    if content.visual_style.is_empty() {
        missing.push("visualStyle");
    }
    if content.primary_shots.is_empty() {
        missing.push("primaryShots");
    }
    if content.required_shots.is_empty() {
        missing.push("requiredShots");
    }
    if content.risk_points.is_empty() {
        missing.push("riskPoints");
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(HostError::new(
            "EXECUTION_BRIEF_INCOMPLETE",
            format!("execution brief is missing: {}", missing.join(", ")),
            false,
        ))
    }
}

fn ensure_project_exists(connection: &Connection, project_id: &str) -> Result<(), HostError> {
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
            "execution brief project does not exist",
            false,
        ))
    }
}

fn insert_record(
    transaction: &Transaction<'_>,
    record: &ExecutionBriefRecord,
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO execution_briefs
             (id, project_id, content_json, status, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.id,
                record.project_id,
                serde_json::to_string(&record.content).map_err(json_error)?,
                status_to_db(&record.status),
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn load_record(connection: &Connection, brief_id: &str) -> Result<ExecutionBriefRecord, HostError> {
    find_record(connection, brief_id)?.ok_or_else(|| {
        HostError::new(
            "EXECUTION_BRIEF_NOT_FOUND",
            "execution brief does not exist",
            false,
        )
    })
}

fn find_record(
    connection: &Connection,
    brief_id: &str,
) -> Result<Option<ExecutionBriefRecord>, HostError> {
    connection
        .query_row(
            "SELECT id, project_id, content_json, status, revision, created_at, updated_at
             FROM execution_briefs WHERE id = ?1",
            [brief_id],
            execution_brief_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn find_by_project(
    connection: &Connection,
    project_id: &str,
) -> Result<Option<ExecutionBriefRecord>, HostError> {
    connection
        .query_row(
            "SELECT id, project_id, content_json, status, revision, created_at, updated_at
             FROM execution_briefs WHERE project_id = ?1",
            [project_id],
            execution_brief_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn ensure_changed(changed: usize) -> Result<(), HostError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(HostError::conflict(
            "execution brief changed during mutation",
        ))
    }
}

fn append_event(
    transaction: &Transaction<'_>,
    event_type: ExecutionBriefEventType,
    record: &ExecutionBriefRecord,
    trace_id: &str,
) -> Result<ExecutionBriefDomainEvent, HostError> {
    let event_id = Uuid::new_v4().to_string();
    let occurred_at = now_millis();
    transaction
        .execute(
            "INSERT INTO execution_brief_events
             (event_id, event_type, aggregate_id, revision, occurred_at, trace_id, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event_id,
                event_type_to_db(&event_type),
                record.id,
                record.revision,
                occurred_at,
                trace_id,
                serde_json::to_string(record).map_err(json_error)?,
            ],
        )
        .map_err(sql_error)?;
    Ok(ExecutionBriefDomainEvent {
        sequence: transaction.last_insert_rowid(),
        event_id,
        event_type,
        aggregate_id: record.id.clone(),
        revision: record.revision,
        occurred_at,
        trace_id: trace_id.to_string(),
        execution_brief: record.clone(),
    })
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<ExecutionBriefDomainEvent> {
    let event_type: String = row.get(2)?;
    let payload: String = row.get(7)?;
    Ok(ExecutionBriefDomainEvent {
        sequence: row.get(0)?,
        event_id: row.get(1)?,
        event_type: event_type_from_db(&event_type)?,
        aggregate_id: row.get(3)?,
        revision: row.get(4)?,
        occurred_at: row.get(5)?,
        trace_id: row.get(6)?,
        execution_brief: serde_json::from_str(&payload).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                payload.len(),
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
    })
}

fn load_event(
    connection: &Connection,
    sequence: i64,
) -> Result<ExecutionBriefDomainEvent, HostError> {
    connection
        .query_row(
            "SELECT sequence, event_id, event_type, aggregate_id, revision,
                    occurred_at, trace_id, payload_json
             FROM execution_brief_events WHERE sequence = ?1",
            [sequence],
            event_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::internal("committed execution brief event is missing"))
}

fn execution_brief_from_row(row: &Row<'_>) -> rusqlite::Result<ExecutionBriefRecord> {
    let content: String = row.get(2)?;
    let status: String = row.get(3)?;
    Ok(ExecutionBriefRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        content: serde_json::from_str(&content).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                content.len(),
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        status: status_from_db(&status)?,
        revision: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn command_fingerprint(command: &NormalizedCommand) -> Result<String, HostError> {
    let meta = command.meta();
    let context = serde_json::json!({
        "actorId": meta.context.actor_id,
        "accountId": meta.context.account_id,
        "projectId": meta.context.project_id,
    });
    let payload = match command {
        NormalizedCommand::Create { payload, .. } => serde_json::to_value(payload),
        NormalizedCommand::Update { payload, .. } => serde_json::to_value(payload),
        NormalizedCommand::ChangeStatus { payload, .. } => serde_json::to_value(payload),
    }
    .map_err(json_error)?;
    let bytes = serde_json::to_vec(&serde_json::json!({
        "commandType": command.command_type(),
        "protocolVersion": meta.protocol_version,
        "context": context,
        "expectedRevision": meta.expected_revision,
        "payload": payload,
    }))
    .map_err(json_error)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
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
) -> Result<Option<ExecutionBriefCommandResponse>, HostError> {
    let by_key = load_receipt(connection, "idempotency_key", idempotency_key)?;
    let by_command = load_receipt(connection, "command_id", command_id)?;
    if by_key
        .as_ref()
        .is_some_and(|receipt| receipt.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "IDEMPOTENCY_KEY_REUSED",
            "idempotencyKey reused for a different execution brief request",
            false,
        ));
    }
    if by_command
        .as_ref()
        .is_some_and(|receipt| receipt.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "COMMAND_ID_REUSED",
            "commandId reused for a different execution brief request",
            false,
        ));
    }
    if let (Some(left), Some(right)) = (&by_key, &by_command) {
        if left.command_id != right.command_id || left.idempotency_key != right.idempotency_key {
            return Err(HostError::new(
                "COMMAND_IDENTITY_COLLISION",
                "command identities resolve to different execution brief requests",
                false,
            ));
        }
    }
    by_key
        .or(by_command)
        .map(|receipt| {
            let mut response: ExecutionBriefCommandResponse =
                serde_json::from_str(&receipt.response_json).map_err(json_error)?;
            response.replayed = true;
            Ok(response)
        })
        .transpose()
}

fn load_receipt(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<StoredReceipt>, HostError> {
    debug_assert!(matches!(column, "idempotency_key" | "command_id"));
    connection
        .query_row(
            &format!(
                "SELECT command_id, idempotency_key, request_fingerprint, response_json
                 FROM execution_brief_command_receipts WHERE {column} = ?1"
            ),
            [value],
            |row| {
                Ok(StoredReceipt {
                    command_id: row.get(0)?,
                    idempotency_key: row.get(1)?,
                    fingerprint: row.get(2)?,
                    response_json: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(sql_error)
}

fn validate_deadline(deadline_at: Option<i64>) -> Result<(), HostError> {
    if deadline_at.is_some_and(|deadline| deadline < now_millis()) {
        Err(HostError::new(
            "COMMAND_DEADLINE_EXCEEDED",
            "execution brief command deadline has elapsed",
            false,
        ))
    } else {
        Ok(())
    }
}

fn status_to_db(status: &ExecutionBriefStatus) -> &'static str {
    match status {
        ExecutionBriefStatus::Draft => "draft",
        ExecutionBriefStatus::Ready => "ready",
    }
}

fn status_from_db(value: &str) -> rusqlite::Result<ExecutionBriefStatus> {
    match value {
        "draft" => Ok(ExecutionBriefStatus::Draft),
        "ready" => Ok(ExecutionBriefStatus::Ready),
        _ => Err(conversion_error("status", value)),
    }
}

fn event_type_to_db(event_type: &ExecutionBriefEventType) -> &'static str {
    match event_type {
        ExecutionBriefEventType::Created => "executionBrief.created",
        ExecutionBriefEventType::Updated => "executionBrief.updated",
        ExecutionBriefEventType::StatusChanged => "executionBrief.statusChanged",
    }
}

fn event_type_from_db(value: &str) -> rusqlite::Result<ExecutionBriefEventType> {
    match value {
        "executionBrief.created" => Ok(ExecutionBriefEventType::Created),
        "executionBrief.updated" => Ok(ExecutionBriefEventType::Updated),
        "executionBrief.statusChanged" => Ok(ExecutionBriefEventType::StatusChanged),
        _ => Err(conversion_error("event type", value)),
    }
}

fn conversion_error(field: &str, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        value.len(),
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid execution brief {field}: {value}"),
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
    HostError::internal(format!("execution brief SQLite operation failed: {error}"))
}

fn json_error(error: serde_json::Error) -> HostError {
    HostError::internal(format!("execution brief JSON operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn database() -> (Connection, String) {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE projects (id TEXT PRIMARY KEY NOT NULL);",
            )
            .unwrap();
        let project_id = Uuid::new_v4().to_string();
        connection
            .execute("INSERT INTO projects (id) VALUES (?1)", [&project_id])
            .unwrap();
        migrate(&connection).unwrap();
        (connection, project_id)
    }

    fn content(complete: bool) -> ExecutionBriefContent {
        ExecutionBriefContent {
            shoot_at: complete.then(|| now_millis() + 86_400_000),
            client_goal: if complete {
                "Show the premium space"
            } else {
                ""
            }
            .into(),
            visual_style: if complete {
                "Restrained natural light"
            } else {
                ""
            }
            .into(),
            primary_shots: complete
                .then_some(vec!["Hero walk-through".into()])
                .unwrap_or_default(),
            secondary_shots: vec!["Material details".into()],
            required_shots: complete
                .then_some(vec!["Brand sign".into()])
                .unwrap_or_default(),
            fallback_shots: vec!["Exterior wide".into()],
            risk_points: complete
                .then_some(vec!["Weather".into()])
                .unwrap_or_default(),
            waiting_time_actions: vec!["Check light positions".into()],
            equipment_notes: "24-70mm and slider".into(),
            post_shoot_highlights: Vec::new(),
        }
    }

    fn context(project_id: &str) -> OperationContext {
        OperationContext {
            actor_id: "local-operator".into(),
            account_id: None,
            project_id: Some(project_id.into()),
            window_id: "main".into(),
            trace_id: Uuid::new_v4().to_string(),
        }
    }

    fn create_command(project_id: &str, complete: bool) -> ExecutionBriefCommandEnvelope {
        ExecutionBriefCommandEnvelope::Create {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: PROTOCOL_VERSION.into(),
            context: context(project_id),
            payload: CreateExecutionBriefPayload {
                project_id: project_id.into(),
                content: content(complete),
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: None,
            deadline_at: None,
        }
    }

    fn mutation(
        project_id: &str,
        brief_id: &str,
        revision: i64,
        status: Option<ExecutionBriefStatus>,
    ) -> ExecutionBriefCommandEnvelope {
        let base = (
            Uuid::new_v4().to_string(),
            PROTOCOL_VERSION.to_string(),
            context(project_id),
            Uuid::new_v4().to_string(),
            Some(revision),
            None,
        );
        if let Some(status) = status {
            ExecutionBriefCommandEnvelope::ChangeStatus {
                command_id: base.0,
                protocol_version: base.1,
                context: base.2,
                payload: ChangeExecutionBriefStatusPayload {
                    brief_id: brief_id.into(),
                    status,
                },
                idempotency_key: base.3,
                expected_revision: base.4,
                deadline_at: base.5,
            }
        } else {
            let mut next = content(true);
            next.post_shoot_highlights = vec!["Designed reflection shot".into()];
            ExecutionBriefCommandEnvelope::Update {
                command_id: base.0,
                protocol_version: base.1,
                context: base.2,
                payload: UpdateExecutionBriefPayload {
                    brief_id: brief_id.into(),
                    content: next,
                },
                idempotency_key: base.3,
                expected_revision: base.4,
                deadline_at: base.5,
            }
        }
    }

    fn table_count(connection: &Connection, table: &str) -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn create_identity(command: &ExecutionBriefCommandEnvelope) -> (&str, &str) {
        match command {
            ExecutionBriefCommandEnvelope::Create {
                command_id,
                idempotency_key,
                ..
            } => (command_id, idempotency_key),
            _ => unreachable!("expected execution brief create command"),
        }
    }

    fn set_create_deadline(command: &mut ExecutionBriefCommandEnvelope, deadline: i64) {
        match command {
            ExecutionBriefCommandEnvelope::Create { deadline_at, .. } => {
                *deadline_at = Some(deadline);
            }
            _ => unreachable!("expected execution brief create command"),
        }
    }

    #[test]
    fn create_update_ready_replay_and_list_are_durable() {
        let (mut connection, project_id) = database();
        let command = create_command(&project_id, true);
        let created = execute_command(&mut connection, command.clone()).unwrap();
        assert_eq!(created.response.execution_brief.revision, 1);
        assert_eq!(
            created.response.execution_brief.status,
            ExecutionBriefStatus::Draft
        );
        assert_eq!(created.emitted_events.len(), 1);

        let replayed = execute_command(&mut connection, command).unwrap();
        assert!(replayed.response.replayed);
        assert!(replayed.emitted_events.is_empty());

        let updated = execute_command(
            &mut connection,
            mutation(&project_id, &created.response.execution_brief.id, 1, None),
        )
        .unwrap();
        let ready = execute_command(
            &mut connection,
            mutation(
                &project_id,
                &created.response.execution_brief.id,
                2,
                Some(ExecutionBriefStatus::Ready),
            ),
        )
        .unwrap();
        assert_eq!(updated.response.execution_brief.revision, 2);
        assert_eq!(ready.response.execution_brief.revision, 3);
        assert_eq!(
            ready.response.execution_brief.status,
            ExecutionBriefStatus::Ready
        );
        assert_eq!(
            list(&connection).unwrap(),
            vec![ready.response.execution_brief]
        );
        assert_eq!(replay_events(&connection, 0, 100).unwrap().len(), 3);
    }

    #[test]
    fn file_database_recovers_records_events_and_receipts_after_reopen() {
        let directory = tempdir().unwrap();
        let database_path = directory.path().join("execution-brief.sqlite3");
        let project_id = Uuid::new_v4().to_string();
        let command = create_command(&project_id, true);
        let committed_record;

        {
            let mut connection = Connection::open(&database_path).unwrap();
            connection
                .execute_batch(
                    "PRAGMA foreign_keys = ON;
                     CREATE TABLE projects (id TEXT PRIMARY KEY NOT NULL);",
                )
                .unwrap();
            connection
                .execute("INSERT INTO projects (id) VALUES (?1)", [&project_id])
                .unwrap();
            migrate(&connection).unwrap();
            committed_record = execute_command(&mut connection, command.clone())
                .unwrap()
                .response
                .execution_brief;
        }

        let mut reopened = Connection::open(&database_path).unwrap();
        migrate(&reopened).unwrap();
        assert_eq!(list(&reopened).unwrap(), vec![committed_record.clone()]);
        let events = replay_events(&reopened, 0, 100).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].execution_brief, committed_record);

        let replayed = execute_command(&mut reopened, command).unwrap();
        assert!(replayed.response.replayed);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(
            table_count(&reopened, "execution_brief_command_receipts"),
            1
        );
    }

    #[test]
    fn ready_requires_backend_completeness_without_partial_commit() {
        let (mut connection, project_id) = database();
        let created = execute_command(&mut connection, create_command(&project_id, false)).unwrap();
        let error = execute_command(
            &mut connection,
            mutation(
                &project_id,
                &created.response.execution_brief.id,
                1,
                Some(ExecutionBriefStatus::Ready),
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "EXECUTION_BRIEF_INCOMPLETE");
        assert_eq!(list(&connection).unwrap()[0].revision, 1);
        assert_eq!(replay_events(&connection, 0, 100).unwrap().len(), 1);
    }

    #[test]
    fn ready_record_rejects_incomplete_content_update_without_partial_commit() {
        let (mut connection, project_id) = database();
        let created = execute_command(&mut connection, create_command(&project_id, true)).unwrap();
        let ready = execute_command(
            &mut connection,
            mutation(
                &project_id,
                &created.response.execution_brief.id,
                1,
                Some(ExecutionBriefStatus::Ready),
            ),
        )
        .unwrap();
        let mut incomplete_update =
            mutation(&project_id, &created.response.execution_brief.id, 2, None);
        if let ExecutionBriefCommandEnvelope::Update { payload, .. } = &mut incomplete_update {
            payload.content = content(false);
        }

        let error = execute_command(&mut connection, incomplete_update).unwrap_err();
        assert_eq!(error.code, "EXECUTION_BRIEF_INCOMPLETE");
        assert_eq!(
            list(&connection).unwrap(),
            vec![ready.response.execution_brief]
        );
        assert_eq!(table_count(&connection, "execution_brief_events"), 2);
        assert_eq!(
            table_count(&connection, "execution_brief_command_receipts"),
            2
        );
    }

    #[test]
    fn protocol_compatibility_is_bounded_and_1_2_receipts_replay() {
        let (mut connection, project_id) = database();
        let mut legacy = create_command(&project_id, true);
        if let ExecutionBriefCommandEnvelope::Create {
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
            replayed.response.execution_brief,
            committed.response.execution_brief
        );
        assert!(replayed.emitted_events.is_empty());
        let stored_version: String = connection
            .query_row(
                "SELECT protocol_version FROM execution_brief_command_receipts",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_version, LEGACY_PROTOCOL_VERSION);

        for supported_version in [
            PROTOCOL_1_3_VERSION,
            PREVIOUS_PROTOCOL_VERSION,
            PROTOCOL_VERSION,
        ] {
            let supported_project_id = Uuid::new_v4().to_string();
            connection
                .execute(
                    "INSERT INTO projects (id) VALUES (?1)",
                    [&supported_project_id],
                )
                .unwrap();
            let mut supported = create_command(&supported_project_id, true);
            if let ExecutionBriefCommandEnvelope::Create {
                protocol_version, ..
            } = &mut supported
            {
                *protocol_version = supported_version.to_string();
            }
            execute_command(&mut connection, supported).unwrap();
        }

        for unsupported_version in ["1.1", "1.6"] {
            let mut unsupported = create_command(&project_id, true);
            if let ExecutionBriefCommandEnvelope::Create {
                protocol_version, ..
            } = &mut unsupported
            {
                *protocol_version = unsupported_version.to_string();
            }
            let error = execute_command(&mut connection, unsupported).unwrap_err();
            assert_eq!(error.code, "PROTOCOL_VERSION_MISMATCH");
            assert_eq!(
                error.message,
                format!(
                    "expected protocolVersion {LEGACY_PROTOCOL_VERSION}, {PROTOCOL_1_3_VERSION}, {PREVIOUS_PROTOCOL_VERSION}, or {PROTOCOL_VERSION}"
                )
            );
        }
        assert_eq!(table_count(&connection, "execution_briefs"), 4);
        assert_eq!(table_count(&connection, "execution_brief_events"), 4);
        assert_eq!(
            table_count(&connection, "execution_brief_command_receipts"),
            4
        );
    }
    #[test]
    fn command_and_idempotency_key_reject_different_fingerprints() {
        let (mut connection, project_id) = database();
        let command = create_command(&project_id, true);
        let (command_id, idempotency_key) = {
            let (command_id, idempotency_key) = create_identity(&command);
            (command_id.to_string(), idempotency_key.to_string())
        };
        execute_command(&mut connection, command).unwrap();

        let mut reused_key = create_command(&project_id, false);
        if let ExecutionBriefCommandEnvelope::Create {
            idempotency_key: candidate_key,
            ..
        } = &mut reused_key
        {
            *candidate_key = idempotency_key;
        }
        assert_eq!(
            execute_command(&mut connection, reused_key)
                .unwrap_err()
                .code,
            "IDEMPOTENCY_KEY_REUSED"
        );

        let mut reused_command = create_command(&project_id, false);
        if let ExecutionBriefCommandEnvelope::Create {
            command_id: candidate_id,
            ..
        } = &mut reused_command
        {
            *candidate_id = command_id;
        }
        assert_eq!(
            execute_command(&mut connection, reused_command)
                .unwrap_err()
                .code,
            "COMMAND_ID_REUSED"
        );
        assert_eq!(table_count(&connection, "execution_briefs"), 1);
        assert_eq!(table_count(&connection, "execution_brief_events"), 1);
        assert_eq!(
            table_count(&connection, "execution_brief_command_receipts"),
            1
        );
    }

    #[test]
    fn first_expired_command_is_rejected_but_committed_command_replays_when_expired() {
        let (mut connection, project_id) = database();
        let mut expired = create_command(&project_id, true);
        set_create_deadline(&mut expired, now_millis() - 1);
        assert_eq!(
            execute_command(&mut connection, expired).unwrap_err().code,
            "COMMAND_DEADLINE_EXCEEDED"
        );
        assert_eq!(table_count(&connection, "execution_briefs"), 0);
        assert_eq!(table_count(&connection, "execution_brief_events"), 0);
        assert_eq!(
            table_count(&connection, "execution_brief_command_receipts"),
            0
        );

        let mut command = create_command(&project_id, true);
        set_create_deadline(&mut command, now_millis() + 60_000);
        let committed = execute_command(&mut connection, command.clone()).unwrap();
        set_create_deadline(&mut command, now_millis() - 1);
        let replayed = execute_command(&mut connection, command).unwrap();
        assert!(replayed.response.replayed);
        assert_eq!(
            replayed.response.execution_brief,
            committed.response.execution_brief
        );
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(table_count(&connection, "execution_briefs"), 1);
        assert_eq!(table_count(&connection, "execution_brief_events"), 1);
        assert_eq!(
            table_count(&connection, "execution_brief_command_receipts"),
            1
        );
    }

    #[test]
    fn event_insert_failure_rolls_back_record_update_and_receipt() {
        let (mut connection, project_id) = database();
        let created = execute_command(&mut connection, create_command(&project_id, true)).unwrap();
        let update = mutation(&project_id, &created.response.execution_brief.id, 1, None);
        connection
            .execute_batch(
                "CREATE TRIGGER reject_execution_brief_event
                 BEFORE INSERT ON execution_brief_events
                 BEGIN SELECT RAISE(ABORT, 'forced execution brief event failure'); END;",
            )
            .unwrap();

        let error = execute_command(&mut connection, update.clone()).unwrap_err();
        assert_eq!(error.code, "HOST_INTERNAL");
        assert_eq!(
            list(&connection).unwrap(),
            vec![created.response.execution_brief]
        );
        assert_eq!(table_count(&connection, "execution_brief_events"), 1);
        assert_eq!(
            table_count(&connection, "execution_brief_command_receipts"),
            1
        );

        connection
            .execute_batch("DROP TRIGGER reject_execution_brief_event;")
            .unwrap();
        let retried = execute_command(&mut connection, update).unwrap();
        assert_eq!(retried.response.execution_brief.revision, 2);
        assert_eq!(table_count(&connection, "execution_brief_events"), 2);
        assert_eq!(
            table_count(&connection, "execution_brief_command_receipts"),
            2
        );
    }

    #[test]
    fn receipt_insert_failure_rolls_back_record_and_event() {
        let (mut connection, project_id) = database();
        let command = create_command(&project_id, true);
        connection
            .execute_batch(
                "CREATE TRIGGER reject_execution_brief_receipt
                 BEFORE INSERT ON execution_brief_command_receipts
                 BEGIN SELECT RAISE(ABORT, 'forced execution brief receipt failure'); END;",
            )
            .unwrap();

        let error = execute_command(&mut connection, command.clone()).unwrap_err();
        assert_eq!(error.code, "HOST_INTERNAL");
        assert_eq!(table_count(&connection, "execution_briefs"), 0);
        assert_eq!(table_count(&connection, "execution_brief_events"), 0);
        assert_eq!(
            table_count(&connection, "execution_brief_command_receipts"),
            0
        );

        connection
            .execute_batch("DROP TRIGGER reject_execution_brief_receipt;")
            .unwrap();
        execute_command(&mut connection, command).unwrap();
        assert_eq!(table_count(&connection, "execution_briefs"), 1);
        assert_eq!(table_count(&connection, "execution_brief_events"), 1);
        assert_eq!(
            table_count(&connection, "execution_brief_command_receipts"),
            1
        );
    }

    #[test]
    fn protocol_version_mismatch_uses_project_error_code() {
        let (mut connection, project_id) = database();
        let mut command = create_command(&project_id, true);
        if let ExecutionBriefCommandEnvelope::Create {
            protocol_version, ..
        } = &mut command
        {
            *protocol_version = "unsupported".into();
        }
        assert_eq!(
            execute_command(&mut connection, command).unwrap_err().code,
            "PROTOCOL_VERSION_MISMATCH"
        );
    }

    #[test]
    fn revision_and_project_context_are_enforced() {
        let (mut connection, project_id) = database();
        let created = execute_command(&mut connection, create_command(&project_id, true)).unwrap();
        let id = &created.response.execution_brief.id;
        let stale = mutation(&project_id, id, 99, None);
        assert_eq!(
            execute_command(&mut connection, stale).unwrap_err().code,
            "REVISION_CONFLICT"
        );

        let other_project = Uuid::new_v4().to_string();
        connection
            .execute("INSERT INTO projects (id) VALUES (?1)", [&other_project])
            .unwrap();
        let wrong_context = mutation(&other_project, id, 1, None);
        assert_eq!(
            execute_command(&mut connection, wrong_context)
                .unwrap_err()
                .code,
            "EXECUTION_BRIEF_PROJECT_MISMATCH"
        );
    }

    #[test]
    fn missing_project_and_duplicate_project_are_rejected() {
        let (mut connection, project_id) = database();
        execute_command(&mut connection, create_command(&project_id, true)).unwrap();
        assert_eq!(
            execute_command(&mut connection, create_command(&project_id, true))
                .unwrap_err()
                .code,
            "EXECUTION_BRIEF_EXISTS"
        );
        let missing = Uuid::new_v4().to_string();
        assert_eq!(
            execute_command(&mut connection, create_command(&missing, true))
                .unwrap_err()
                .code,
            "PROJECT_NOT_FOUND"
        );
    }

    #[test]
    fn replay_cursor_and_content_bounds_are_validated() {
        let (mut connection, project_id) = database();
        let mut command = create_command(&project_id, true);
        if let ExecutionBriefCommandEnvelope::Create { payload, .. } = &mut command {
            payload.content.primary_shots = (0..=MAX_LIST_ITEMS)
                .map(|index| format!("shot-{index}"))
                .collect();
        }
        assert_eq!(
            execute_command(&mut connection, command).unwrap_err().code,
            "VALIDATION_FAILED"
        );
        assert_eq!(
            replay_events(&connection, -1, 20).unwrap_err().code,
            "VALIDATION_FAILED"
        );
    }
}
