use crate::module_registry::ModuleRegistry;
use crate::protocol::{
    is_legacy_surface_protocol_supported, ApprovalRecord, BriefRecord, CommandEnvelope,
    CommandReceipt, CommandResponse, DomainEvent, HostError, HostStatus, ModuleAvailability,
    PermissionDecision, ProjectEventType, ProjectRecord, ProjectStage, ResolveApprovalPayload,
    LEGACY_PROTOCOL_VERSION, PREVIOUS_PROTOCOL_VERSION, PROTOCOL_1_3_VERSION, PROTOCOL_VERSION,
};
use crate::security::{self, OperationEffect};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde_json::json;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub struct BackendHost {
    connection: Mutex<Connection>,
    vault_ready: bool,
    module_registry: ModuleRegistry,
}

#[derive(Debug)]
pub struct ExecuteOutcome {
    pub response: CommandResponse,
    pub emitted_events: Vec<DomainEvent>,
}

struct CommandMeta<'a> {
    command_id: &'a str,
    protocol_version: &'a str,
    actor_id: &'a str,
    project_id: Option<&'a str>,
    trace_id: &'a str,
    idempotency_key: &'a str,
    expected_revision: Option<i64>,
    deadline_at: Option<i64>,
}

impl BackendHost {
    pub fn open(database_path: &Path, vault_path: &Path) -> Result<Self, HostError> {
        if let Some(parent) = database_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| HostError::internal(format!("create data directory failed: {e}")))?;
        }
        fs::create_dir_all(vault_path)
            .map_err(|e| HostError::internal(format!("create Vault failed: {e}")))?;
        let connection = Connection::open(database_path)
            .map_err(|e| HostError::internal(format!("open SQLite failed: {e}")))?;
        Self::from_connection(connection, true)
    }

    #[cfg(test)]
    fn open_in_memory(vault_path: &Path) -> Result<Self, HostError> {
        fs::create_dir_all(vault_path)
            .map_err(|e| HostError::internal(format!("create test Vault failed: {e}")))?;
        let connection = Connection::open_in_memory()
            .map_err(|e| HostError::internal(format!("open memory SQLite failed: {e}")))?;
        Self::from_connection(connection, true)
    }

    fn from_connection(connection: Connection, vault_ready: bool) -> Result<Self, HostError> {
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(sql_error)?;
        security::migrate(&connection)?;
        crate::brain_store::migrate(&connection)?;
        connection
            .execute_batch(
                r#"
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                client_name TEXT NOT NULL,
                brief_json TEXT NOT NULL,
                stage TEXT NOT NULL,
                revision INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL,
                aggregate_type TEXT NOT NULL,
                aggregate_id TEXT NOT NULL,
                revision INTEGER NOT NULL,
                occurred_at INTEGER NOT NULL,
                trace_id TEXT NOT NULL,
                payload_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_events_aggregate ON events(aggregate_id, sequence);
            CREATE TABLE IF NOT EXISTS command_receipts (
                idempotency_key TEXT PRIMARY KEY NOT NULL,
                command_id TEXT NOT NULL UNIQUE,
                command_type TEXT NOT NULL,
                request_fingerprint TEXT NOT NULL,
                response_json TEXT NOT NULL,
                completed_at INTEGER NOT NULL
            );
        "#,
            )
            .map_err(sql_error)?;
        Ok(Self {
            connection: Mutex::new(connection),
            vault_ready,
            module_registry: ModuleRegistry::desktop(),
        })
    }

    pub fn execute(&self, command: CommandEnvelope) -> Result<ExecuteOutcome, HostError> {
        validate_command(&command)?;
        let fingerprint = command_fingerprint(&command)?;
        let meta = command_meta(&command);
        let command_id = meta.command_id.to_string();
        let idempotency_key = meta.idempotency_key.to_string();
        let command_type = command_type(&command).to_string();
        let trace_id = meta.trace_id.to_string();
        let deadline_at = meta.deadline_at;

        let mut connection = self
            .connection
            .lock()
            .map_err(|_| HostError::internal("SQLite host lock is poisoned"))?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        let decision = security::authorize(
            &transaction,
            meta.actor_id,
            &command_type,
            "project",
            meta.project_id,
            OperationEffect::ReversibleWrite,
            None,
        )?;
        if !decision.allowed {
            return Err(HostError::new(
                "PERMISSION_DENIED",
                decision
                    .reason
                    .unwrap_or_else(|| "operation denied".to_string()),
                false,
            ));
        }

        if let Some(response) =
            find_existing_receipt(&transaction, &command_id, &idempotency_key, &fingerprint)?
        {
            transaction.commit().map_err(sql_error)?;
            return Ok(ExecuteOutcome {
                response,
                emitted_events: Vec::new(),
            });
        }

        validate_deadline(deadline_at)?;

        let (project, event_type) = match command {
            CommandEnvelope::ProjectCreate {
                payload,
                expected_revision,
                ..
            } => {
                if expected_revision.is_some() {
                    return Err(HostError::validation(
                        "project.create rejects expectedRevision",
                    ));
                }
                let now = now_millis();
                let project = ProjectRecord {
                    id: Uuid::new_v4().to_string(),
                    name: normalize_required("project name", &payload.name, 2, 120)?,
                    client_name: normalize_required("client name", &payload.client_name, 1, 120)?,
                    brief: BriefRecord::default(),
                    stage: ProjectStage::Intake,
                    revision: 1,
                    created_at: now,
                    updated_at: now,
                };
                insert_project(&transaction, &project)?;
                (project, ProjectEventType::ProjectCreated)
            }
            CommandEnvelope::ProjectUpdateBrief {
                payload,
                expected_revision,
                ..
            } => {
                let expected = expected_revision.ok_or_else(|| {
                    HostError::validation("project.updateBrief requires expectedRevision")
                })?;
                ensure_project_brief_not_superseded(&transaction, &payload.project_id)?;
                validate_brief(&payload.brief)?;
                let mut project = load_project(&transaction, &payload.project_id)?;
                ensure_revision(&project, expected)?;
                project.brief = normalize_brief(payload.brief);
                project.revision += 1;
                project.updated_at = now_millis();
                update_project(&transaction, &project, expected)?;
                (project, ProjectEventType::ProjectBriefUpdated)
            }
            CommandEnvelope::ProjectChangeStage {
                payload,
                expected_revision,
                ..
            } => {
                let expected = expected_revision.ok_or_else(|| {
                    HostError::validation("project.changeStage requires expectedRevision")
                })?;
                let mut project = load_project(&transaction, &payload.project_id)?;
                ensure_revision(&project, expected)?;
                project.stage = payload.stage;
                project.revision += 1;
                project.updated_at = now_millis();
                update_project(&transaction, &project, expected)?;
                (project, ProjectEventType::ProjectStageChanged)
            }
        };

        let event = append_event(&transaction, event_type, &project, &trace_id)?;
        let completed_at = now_millis();
        let response = CommandResponse {
            receipt: CommandReceipt {
                command_id: command_id.clone(),
                idempotency_key: idempotency_key.clone(),
                command_type: command_type.clone(),
                aggregate_id: project.id.clone(),
                revision: project.revision,
                last_event_sequence: event.sequence,
                completed_at,
            },
            project,
            replayed: false,
        };
        let response_json = serde_json::to_string(&response).map_err(json_error)?;
        transaction.execute(
            "INSERT INTO command_receipts
             (idempotency_key, command_id, command_type, request_fingerprint, response_json, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![idempotency_key, command_id, command_type, fingerprint, response_json, completed_at],
        ).map_err(sql_error)?;
        transaction.commit().map_err(sql_error)?;
        Ok(ExecuteOutcome {
            response,
            emitted_events: vec![event],
        })
    }

    pub fn set_module_availability(&mut self, id: &str, availability: ModuleAvailability) {
        self.module_registry.set_availability(id, availability);
    }

    pub fn authorize_operation(
        &self,
        actor_id: &str,
        operation: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        effect: OperationEffect,
        approval_id: Option<&str>,
    ) -> Result<PermissionDecision, HostError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| HostError::internal("SQLite host lock is poisoned"))?;
        security::authorize(
            &connection,
            actor_id,
            operation,
            resource_type,
            resource_id,
            effect,
            approval_id,
        )
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectRecord>, HostError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| HostError::internal("SQLite host lock is poisoned"))?;
        let mut statement = connection
            .prepare(
                "SELECT id, name, client_name, brief_json, stage, revision, created_at, updated_at
             FROM projects ORDER BY updated_at DESC, id ASC",
            )
            .map_err(sql_error)?;
        let rows = statement
            .query_map([], project_from_row)
            .map_err(sql_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(sql_error)
    }

    pub fn replay_events(
        &self,
        after_sequence: i64,
        limit: u32,
    ) -> Result<Vec<DomainEvent>, HostError> {
        let safe_limit = limit.clamp(1, 500);
        let connection = self
            .connection
            .lock()
            .map_err(|_| HostError::internal("SQLite host lock is poisoned"))?;
        let mut statement = connection
            .prepare(
                "SELECT sequence, event_id, event_type, aggregate_type, aggregate_id,
                    revision, occurred_at, trace_id, payload_json
             FROM events WHERE sequence > ?1 ORDER BY sequence ASC LIMIT ?2",
            )
            .map_err(sql_error)?;
        let rows = statement
            .query_map(
                params![after_sequence.max(0), i64::from(safe_limit)],
                event_from_row,
            )
            .map_err(sql_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(sql_error)
    }

    pub fn status(&self) -> Result<HostStatus, HostError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| HostError::internal("SQLite host lock is poisoned"))?;
        let project_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .map_err(sql_error)?;
        let task_count = table_row_count(&connection, "tasks")?;
        let asset_count = table_row_count(&connection, "assets")?;
        let last_event_sequence: i64 = connection
            .query_row("SELECT COALESCE(MAX(sequence), 0) FROM events", [], |row| {
                row.get(0)
            })
            .map_err(sql_error)?;
        Ok(HostStatus {
            protocol_version: PROTOCOL_VERSION.to_string(),
            database_ready: true,
            vault_ready: self.vault_ready,
            project_count,
            task_count,
            asset_count,
            last_event_sequence,
            runtime: "rust-backend-host".to_string(),
            modules: self.module_registry.manifests(),
        })
    }

    pub fn list_pending_approvals(&self) -> Result<Vec<ApprovalRecord>, HostError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| HostError::internal("SQLite host lock is poisoned"))?;
        security::list_pending(&connection)
    }

    pub fn resolve_approval(
        &self,
        resolved_by: &str,
        payload: &ResolveApprovalPayload,
    ) -> Result<ApprovalRecord, HostError> {
        if resolved_by.trim().is_empty() {
            return Err(HostError::validation(
                "approval resolver identity must not be empty",
            ));
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| HostError::internal("SQLite host lock is poisoned"))?;
        security::resolve(&connection, resolved_by, payload)
    }
}

fn table_row_count(connection: &Connection, table: &str) -> Result<i64, HostError> {
    let exists: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    if exists == 0 {
        return Ok(0);
    }
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .map_err(sql_error)
}

fn validate_command(command: &CommandEnvelope) -> Result<(), HostError> {
    let meta = command_meta(command);
    if !is_legacy_surface_protocol_supported(meta.protocol_version) {
        return Err(HostError::new(
            "PROTOCOL_VERSION_MISMATCH",
            format!(
                "client protocol {}, supported protocols {}, {}, {}, and {}",
                meta.protocol_version,
                LEGACY_PROTOCOL_VERSION,
                PROTOCOL_1_3_VERSION,
                PREVIOUS_PROTOCOL_VERSION,
                PROTOCOL_VERSION
            ),
            false,
        ));
    }
    Uuid::parse_str(meta.command_id)
        .map_err(|_| HostError::validation("commandId must be a UUID"))?;
    if meta.idempotency_key.trim().len() < 8 || meta.idempotency_key.len() > 160 {
        return Err(HostError::validation(
            "idempotencyKey length must be 8..160",
        ));
    }
    if meta.actor_id.trim().is_empty() || meta.trace_id.trim().is_empty() {
        return Err(HostError::validation("actorId and traceId are required"));
    }
    Ok(())
}

fn validate_deadline(deadline_at: Option<i64>) -> Result<(), HostError> {
    if deadline_at.is_some_and(|deadline| deadline < now_millis()) {
        return Err(HostError::new(
            "COMMAND_DEADLINE_EXCEEDED",
            "command deadline has elapsed",
            false,
        ));
    }
    Ok(())
}

fn command_meta(command: &CommandEnvelope) -> CommandMeta<'_> {
    match command {
        CommandEnvelope::ProjectCreate {
            command_id,
            protocol_version,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        }
        | CommandEnvelope::ProjectUpdateBrief {
            command_id,
            protocol_version,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        }
        | CommandEnvelope::ProjectChangeStage {
            command_id,
            protocol_version,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => CommandMeta {
            command_id,
            protocol_version,
            actor_id: &context.actor_id,
            project_id: context.project_id.as_deref(),
            trace_id: &context.trace_id,
            idempotency_key,
            expected_revision: *expected_revision,
            deadline_at: *deadline_at,
        },
    }
}

fn command_type(command: &CommandEnvelope) -> &'static str {
    match command {
        CommandEnvelope::ProjectCreate { .. } => "project.create",
        CommandEnvelope::ProjectUpdateBrief { .. } => "project.updateBrief",
        CommandEnvelope::ProjectChangeStage { .. } => "project.changeStage",
    }
}

fn command_fingerprint(command: &CommandEnvelope) -> Result<String, HostError> {
    let meta = command_meta(command);
    let value = match command {
        CommandEnvelope::ProjectCreate { payload, .. } => json!({
            "commandType": command_type(command), "actorId": meta.actor_id,
            "expectedRevision": meta.expected_revision, "payload": payload,
        }),
        CommandEnvelope::ProjectUpdateBrief { payload, .. } => json!({
            "commandType": command_type(command), "actorId": meta.actor_id,
            "expectedRevision": meta.expected_revision, "payload": payload,
        }),
        CommandEnvelope::ProjectChangeStage { payload, .. } => json!({
            "commandType": command_type(command), "actorId": meta.actor_id,
            "expectedRevision": meta.expected_revision, "payload": payload,
        }),
    };
    serde_json::to_string(&value).map_err(json_error)
}

fn find_existing_receipt(
    transaction: &Transaction<'_>,
    command_id: &str,
    idempotency_key: &str,
    fingerprint: &str,
) -> Result<Option<CommandResponse>, HostError> {
    let by_key: Option<(String, String)> = transaction.query_row(
        "SELECT request_fingerprint, response_json FROM command_receipts WHERE idempotency_key = ?1",
        [idempotency_key],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(sql_error)?;
    if let Some((stored_fingerprint, response_json)) = by_key {
        if stored_fingerprint != fingerprint {
            return Err(HostError::new(
                "IDEMPOTENCY_KEY_REUSED",
                "idempotencyKey reused for a different request",
                false,
            ));
        }
        let mut response: CommandResponse =
            serde_json::from_str(&response_json).map_err(json_error)?;
        response.replayed = true;
        return Ok(Some(response));
    }

    let by_command: Option<(String, String)> = transaction
        .query_row(
            "SELECT request_fingerprint, response_json FROM command_receipts WHERE command_id = ?1",
            [command_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sql_error)?;
    if let Some((stored_fingerprint, response_json)) = by_command {
        if stored_fingerprint != fingerprint {
            return Err(HostError::new(
                "COMMAND_ID_REUSED",
                "commandId reused for a different request",
                false,
            ));
        }
        let mut response: CommandResponse =
            serde_json::from_str(&response_json).map_err(json_error)?;
        response.replayed = true;
        return Ok(Some(response));
    }
    Ok(None)
}

fn normalize_required(
    field: &str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<String, HostError> {
    let normalized = value.trim().to_string();
    let count = normalized.chars().count();
    if count < min || count > max {
        return Err(HostError::validation(format!(
            "{field} length must be {min}..{max}"
        )));
    }
    Ok(normalized)
}

fn validate_brief(brief: &BriefRecord) -> Result<(), HostError> {
    for (name, value, max) in [
        ("objective", &brief.objective, 4000usize),
        ("audience", &brief.audience, 2000usize),
        ("referenceNotes", &brief.reference_notes, 6000usize),
    ] {
        if value.chars().count() > max {
            return Err(HostError::validation(format!(
                "{name} exceeds {max} characters"
            )));
        }
    }
    for (name, values) in [
        ("deliverables", &brief.deliverables),
        ("styleKeywords", &brief.style_keywords),
        ("mandatoryItems", &brief.mandatory_items),
        ("constraints", &brief.constraints),
        ("risks", &brief.risks),
    ] {
        if values.len() > 100 || values.iter().any(|value| value.chars().count() > 500) {
            return Err(HostError::validation(format!(
                "{name} has too many or oversized items"
            )));
        }
    }
    Ok(())
}

fn normalize_brief(mut brief: BriefRecord) -> BriefRecord {
    brief.objective = brief.objective.trim().to_string();
    brief.audience = brief.audience.trim().to_string();
    brief.reference_notes = brief.reference_notes.trim().to_string();
    brief.deliverables = normalize_list(brief.deliverables);
    brief.style_keywords = normalize_list(brief.style_keywords);
    brief.mandatory_items = normalize_list(brief.mandatory_items);
    brief.constraints = normalize_list(brief.constraints);
    brief.risks = normalize_list(brief.risks);
    brief
}

fn normalize_list(values: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim().to_string();
        if !value.is_empty() && !normalized.iter().any(|existing| existing == &value) {
            normalized.push(value);
        }
    }
    normalized
}

fn ensure_revision(project: &ProjectRecord, expected: i64) -> Result<(), HostError> {
    if project.revision != expected {
        return Err(HostError::conflict(format!(
            "current revision is {}, request expected {}",
            project.revision, expected
        )));
    }
    Ok(())
}

fn ensure_project_brief_not_superseded(
    transaction: &Transaction<'_>,
    project_id: &str,
) -> Result<(), HostError> {
    let table_exists = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'requirement_briefs'
            )",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sql_error)?;
    if !table_exists {
        return Ok(());
    }

    let superseded = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM requirement_briefs WHERE project_id = ?1)",
            [project_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sql_error)?;
    if superseded {
        return Err(HostError::new(
            "PROJECT_BRIEF_SUPERSEDED",
            "project brief is owned by requirement_briefs",
            false,
        ));
    }
    Ok(())
}

fn insert_project(transaction: &Transaction<'_>, project: &ProjectRecord) -> Result<(), HostError> {
    let brief_json = serde_json::to_string(&project.brief).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO projects
         (id, name, client_name, brief_json, stage, revision, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                project.id,
                project.name,
                project.client_name,
                brief_json,
                project.stage.as_db_str(),
                project.revision,
                project.created_at,
                project.updated_at
            ],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn update_project(
    transaction: &Transaction<'_>,
    project: &ProjectRecord,
    expected_revision: i64,
) -> Result<(), HostError> {
    let brief_json = serde_json::to_string(&project.brief).map_err(json_error)?;
    let changed = transaction
        .execute(
            "UPDATE projects SET
             name = ?2, client_name = ?3, brief_json = ?4, stage = ?5,
             revision = ?6, updated_at = ?7
         WHERE id = ?1 AND revision = ?8",
            params![
                project.id,
                project.name,
                project.client_name,
                brief_json,
                project.stage.as_db_str(),
                project.revision,
                project.updated_at,
                expected_revision
            ],
        )
        .map_err(sql_error)?;
    if changed != 1 {
        return Err(HostError::conflict(
            "project changed while command was being written",
        ));
    }
    Ok(())
}

fn load_project(
    transaction: &Transaction<'_>,
    project_id: &str,
) -> Result<ProjectRecord, HostError> {
    transaction
        .query_row(
            "SELECT id, name, client_name, brief_json, stage, revision, created_at, updated_at
         FROM projects WHERE id = ?1",
            [project_id],
            project_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::new("PROJECT_NOT_FOUND", "project does not exist", false))
}

fn project_from_row(row: &Row<'_>) -> rusqlite::Result<ProjectRecord> {
    let brief_json: String = row.get(3)?;
    let brief = serde_json::from_str(&brief_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            brief_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    let stage_value: String = row.get(4)?;
    let stage = ProjectStage::from_db_str(&stage_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            stage_value.len(),
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown project stage: {stage_value}"),
            )),
        )
    })?;
    Ok(ProjectRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        client_name: row.get(2)?,
        brief,
        stage,
        revision: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn append_event(
    transaction: &Transaction<'_>,
    event_type: ProjectEventType,
    project: &ProjectRecord,
    trace_id: &str,
) -> Result<DomainEvent, HostError> {
    let event_id = Uuid::new_v4().to_string();
    let occurred_at = now_millis();
    let payload_json = serde_json::to_string(project).map_err(json_error)?;
    transaction.execute(
        "INSERT INTO events
         (event_id, event_type, aggregate_type, aggregate_id, revision, occurred_at, trace_id, payload_json)
         VALUES (?1, ?2, 'project', ?3, ?4, ?5, ?6, ?7)",
        params![
            event_id, event_type.as_wire_str(), project.id, project.revision,
            occurred_at, trace_id, payload_json
        ],
    ).map_err(sql_error)?;
    let sequence = transaction.last_insert_rowid();
    Ok(DomainEvent {
        sequence,
        event_id,
        event_type,
        aggregate_type: "project".to_string(),
        aggregate_id: project.id.clone(),
        revision: project.revision,
        occurred_at,
        trace_id: trace_id.to_string(),
        project: project.clone(),
    })
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<DomainEvent> {
    let event_type_value: String = row.get(2)?;
    let event_type = ProjectEventType::from_wire_str(&event_type_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            event_type_value.len(),
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown event type: {event_type_value}"),
            )),
        )
    })?;
    let payload_json: String = row.get(8)?;
    let project = serde_json::from_str(&payload_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            payload_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    Ok(DomainEvent {
        sequence: row.get(0)?,
        event_id: row.get(1)?,
        event_type,
        aggregate_type: row.get(3)?,
        aggregate_id: row.get(4)?,
        revision: row.get(5)?,
        occurred_at: row.get(6)?,
        trace_id: row.get(7)?,
        project,
    })
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn sql_error(error: rusqlite::Error) -> HostError {
    HostError::internal(format!("SQLite operation failed: {error}"))
}
fn json_error(error: serde_json::Error) -> HostError {
    HostError::internal(format!("JSON protocol operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        ChangeProjectStagePayload, CreateProjectPayload, OperationContext,
        UpdateProjectBriefPayload,
    };
    use tempfile::tempdir;

    fn context(trace_id: &str) -> OperationContext {
        OperationContext {
            actor_id: "operator-local".to_string(),
            account_id: None,
            project_id: None,
            window_id: "main".to_string(),
            trace_id: trace_id.to_string(),
        }
    }

    fn create_command(command_id: &str, idempotency_key: &str) -> CommandEnvelope {
        CommandEnvelope::ProjectCreate {
            command_id: command_id.to_string(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            context: context("trace-create"),
            payload: CreateProjectPayload {
                name: "Riverside brand film".to_string(),
                client_name: "Banshan Property".to_string(),
            },
            idempotency_key: idempotency_key.to_string(),
            expected_revision: None,
            deadline_at: Some(now_millis() + 30_000),
        }
    }

    fn update_brief_command(project_id: &str, idempotency_key: &str) -> CommandEnvelope {
        CommandEnvelope::ProjectUpdateBrief {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            context: context("trace-brief"),
            payload: UpdateProjectBriefPayload {
                project_id: project_id.to_string(),
                brief: BriefRecord {
                    objective: "Build premium lifestyle recognition".to_string(),
                    deliverables: vec!["90 second master".to_string()],
                    ..BriefRecord::default()
                },
            },
            idempotency_key: idempotency_key.to_string(),
            expected_revision: Some(1),
            deadline_at: None,
        }
    }

    #[test]
    fn create_is_durable_and_idempotent() {
        let directory = tempdir().unwrap();
        let database = directory.path().join("bsaigc.sqlite3");
        let vault = directory.path().join("vault");
        let command_id = Uuid::new_v4().to_string();
        let idempotency_key = "project-create-001";
        {
            let host = BackendHost::open(&database, &vault).unwrap();
            let first = host
                .execute(create_command(&command_id, idempotency_key))
                .unwrap();
            assert!(!first.response.replayed);
            assert_eq!(first.response.project.revision, 1);
            assert_eq!(first.emitted_events.len(), 1);
            let replay = host
                .execute(create_command(&Uuid::new_v4().to_string(), idempotency_key))
                .unwrap();
            assert!(replay.response.replayed);
            assert!(replay.emitted_events.is_empty());
            assert_eq!(replay.response.project.id, first.response.project.id);
            assert_eq!(host.list_projects().unwrap().len(), 1);
            assert_eq!(host.replay_events(0, 100).unwrap().len(), 1);
        }
        let reopened = BackendHost::open(&database, &vault).unwrap();
        assert_eq!(reopened.list_projects().unwrap().len(), 1);
        assert_eq!(reopened.status().unwrap().last_event_sequence, 1);
    }

    #[test]
    fn protocol_compatibility_is_bounded_and_1_2_receipts_replay() {
        let directory = tempdir().unwrap();
        let host = BackendHost::open_in_memory(&directory.path().join("vault")).unwrap();
        let mut legacy = create_command(&Uuid::new_v4().to_string(), "project-create-protocol-1-2");
        if let CommandEnvelope::ProjectCreate {
            protocol_version, ..
        } = &mut legacy
        {
            *protocol_version = LEGACY_PROTOCOL_VERSION.to_string();
        }

        let committed = host.execute(legacy.clone()).unwrap();
        let replayed = host.execute(legacy).unwrap();

        assert!(!committed.response.replayed);
        assert!(replayed.response.replayed);
        assert_eq!(replayed.response.receipt, committed.response.receipt);
        assert_eq!(replayed.response.project, committed.response.project);
        assert!(replayed.emitted_events.is_empty());

        for (supported_version, idempotency_key) in [
            (PROTOCOL_1_3_VERSION, "project-create-protocol-1-3"),
            (PREVIOUS_PROTOCOL_VERSION, "project-create-protocol-1-4"),
            (PROTOCOL_VERSION, "project-create-protocol-1-5"),
        ] {
            let mut supported = create_command(&Uuid::new_v4().to_string(), idempotency_key);
            if let CommandEnvelope::ProjectCreate {
                protocol_version, ..
            } = &mut supported
            {
                *protocol_version = supported_version.to_string();
            }
            host.execute(supported).unwrap();
        }

        for unsupported_version in ["1.1", "1.6"] {
            let mut unsupported = create_command(
                &Uuid::new_v4().to_string(),
                &format!("project-create-protocol-{unsupported_version}"),
            );
            if let CommandEnvelope::ProjectCreate {
                protocol_version, ..
            } = &mut unsupported
            {
                *protocol_version = unsupported_version.to_string();
            }
            let error = host.execute(unsupported).unwrap_err();
            assert_eq!(error.code, "PROTOCOL_VERSION_MISMATCH");
            assert_eq!(
                error.message,
                format!(
                    "client protocol {unsupported_version}, supported protocols {LEGACY_PROTOCOL_VERSION}, {PROTOCOL_1_3_VERSION}, {PREVIOUS_PROTOCOL_VERSION}, and {PROTOCOL_VERSION}"
                )
            );
        }
        assert_eq!(host.list_projects().unwrap().len(), 4);
        assert_eq!(host.replay_events(0, 100).unwrap().len(), 4);
    }
    #[test]
    fn committed_command_replays_after_its_deadline() {
        let directory = tempdir().unwrap();
        let host = BackendHost::open_in_memory(&directory.path().join("vault")).unwrap();
        let command_id = Uuid::new_v4().to_string();
        let idempotency_key = "project-create-expired-replay";
        let first = host
            .execute(create_command(&command_id, idempotency_key))
            .unwrap();

        let mut replay = create_command(&Uuid::new_v4().to_string(), idempotency_key);
        if let CommandEnvelope::ProjectCreate { deadline_at, .. } = &mut replay {
            *deadline_at = Some(now_millis() - 1);
        }
        let replayed = host.execute(replay).unwrap();

        assert!(replayed.response.replayed);
        assert_eq!(replayed.response.project.id, first.response.project.id);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(host.list_projects().unwrap().len(), 1);
    }

    #[test]
    fn update_uses_revision_compare_and_swap() {
        let directory = tempdir().unwrap();
        let host = BackendHost::open_in_memory(&directory.path().join("vault")).unwrap();
        let created = host
            .execute(create_command(
                &Uuid::new_v4().to_string(),
                "project-create-002",
            ))
            .unwrap()
            .response
            .project;
        let brief = BriefRecord {
            objective: "Build premium lifestyle recognition".to_string(),
            deliverables: vec!["90 second master".to_string(), "3 cutdowns".to_string()],
            ..BriefRecord::default()
        };
        let updated = host
            .execute(CommandEnvelope::ProjectUpdateBrief {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: PROTOCOL_VERSION.to_string(),
                context: context("trace-brief"),
                payload: UpdateProjectBriefPayload {
                    project_id: created.id.clone(),
                    brief,
                },
                idempotency_key: "project-brief-002".to_string(),
                expected_revision: Some(1),
                deadline_at: None,
            })
            .unwrap()
            .response
            .project;
        assert_eq!(updated.revision, 2);
        assert_eq!(updated.brief.deliverables.len(), 2);

        let error = host
            .execute(CommandEnvelope::ProjectChangeStage {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: PROTOCOL_VERSION.to_string(),
                context: context("trace-stage"),
                payload: ChangeProjectStagePayload {
                    project_id: created.id,
                    stage: ProjectStage::Creative,
                },
                idempotency_key: "project-stage-002".to_string(),
                expected_revision: Some(1),
                deadline_at: None,
            })
            .unwrap_err();
        assert_eq!(error.code, "REVISION_CONFLICT");
        assert_eq!(host.replay_events(0, 100).unwrap().len(), 2);
    }

    #[test]
    fn update_brief_rejects_project_owned_by_requirement_briefs() {
        let directory = tempdir().unwrap();
        let host = BackendHost::open_in_memory(&directory.path().join("vault")).unwrap();
        let created = host
            .execute(create_command(
                &Uuid::new_v4().to_string(),
                "project-create-superseded",
            ))
            .unwrap()
            .response
            .project;
        {
            let connection = host.connection.lock().unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE requirement_briefs (
                        id TEXT PRIMARY KEY NOT NULL,
                        project_id TEXT NOT NULL UNIQUE
                    );",
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO requirement_briefs (id, project_id) VALUES (?1, ?2)",
                    params![Uuid::new_v4().to_string(), created.id],
                )
                .unwrap();
        }

        let error = host
            .execute(update_brief_command(
                &created.id,
                "project-brief-superseded",
            ))
            .unwrap_err();

        assert_eq!(error.code, "PROJECT_BRIEF_SUPERSEDED");
        assert!(!error.retryable);
        assert_eq!(host.list_projects().unwrap()[0], created);
        assert_eq!(host.replay_events(0, 100).unwrap().len(), 1);
    }

    #[test]
    fn update_brief_allows_legacy_database_without_requirement_briefs_table() {
        let directory = tempdir().unwrap();
        let host = BackendHost::open_in_memory(&directory.path().join("vault")).unwrap();
        let created = host
            .execute(create_command(
                &Uuid::new_v4().to_string(),
                "project-create-legacy-brief",
            ))
            .unwrap()
            .response
            .project;
        let table_exists = host
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type = 'table' AND name = 'requirement_briefs'
                )",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();
        assert!(!table_exists);

        let updated = host
            .execute(update_brief_command(
                &created.id,
                "project-brief-legacy-database",
            ))
            .unwrap()
            .response
            .project;

        assert_eq!(updated.revision, 2);
        assert_eq!(
            updated.brief.objective,
            "Build premium lifestyle recognition"
        );
        assert_eq!(host.replay_events(0, 100).unwrap().len(), 2);
    }
}
