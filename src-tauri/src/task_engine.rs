use crate::protocol::{
    is_legacy_surface_protocol_supported, CancelTaskPayload, CommandReceipt, CreateTaskPayload,
    HostError, OperationContext, RetryTaskPayload, TaskCommandEnvelope, TaskCommandResponse,
    TaskDependency, TaskDomainEvent, TaskEventType, TaskPriority, TaskRecord, TaskReplayPolicy,
    TaskStatus, LEGACY_PROTOCOL_VERSION, PREVIOUS_PROTOCOL_VERSION, PROTOCOL_1_3_VERSION,
    PROTOCOL_VERSION,
};
use rusqlite::{
    params, params_from_iter, Connection, OptionalExtension, Row, Transaction, TransactionBehavior,
};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const TASK_COLUMNS: &str = "id, kind, project_id, input_json, output_json, status, priority, \
    replay_policy, progress, attempt, max_attempts, revision, created_at, updated_at, \
    started_at, finished_at, last_error";
const LIFECYCLE_TRACE_ID: &str = "task-engine:lifecycle";

/// SQLite-backed task authority. Every mutation uses an IMMEDIATE transaction so claiming,
/// revision checks, attempt fencing, and state history are committed atomically.
pub struct TaskEngine {
    connection: Mutex<Connection>,
}

pub type DurableTaskEngine = TaskEngine;

#[derive(Debug)]
pub struct TaskCommandOutcome {
    pub response: TaskCommandResponse,
    pub emitted_events: Vec<TaskDomainEvent>,
}

#[derive(Debug)]
pub struct TaskLifecycleOutcome {
    pub task: TaskRecord,
    pub emitted_events: Vec<TaskDomainEvent>,
}

#[derive(Debug)]
pub struct TaskClaimOutcome {
    pub task: Option<TaskRecord>,
    pub emitted_events: Vec<TaskDomainEvent>,
}

#[derive(Debug)]
pub struct TaskRecoveryOutcome {
    pub tasks: Vec<TaskRecord>,
    pub emitted_events: Vec<TaskDomainEvent>,
}

#[derive(Debug)]
struct TaskCommandMeta {
    command_id: String,
    protocol_version: String,
    context: OperationContext,
    idempotency_key: String,
    expected_revision: Option<i64>,
    deadline_at: Option<i64>,
}

#[derive(Debug, Clone)]
struct NewTask {
    id: String,
    kind: String,
    project_id: Option<String>,
    input: Value,
    priority: TaskPriority,
    replay_policy: TaskReplayPolicy,
    max_attempts: u32,
    dependencies: Vec<DependencyInput>,
}

#[derive(Debug, Clone)]
struct DependencyInput {
    task_id: String,
    wire_value: Value,
}

#[derive(Debug)]
struct StoredTask {
    id: String,
    kind: String,
    project_id: Option<String>,
    input: Value,
    output: Option<Value>,
    status: TaskStatus,
    priority: TaskPriority,
    replay_policy: TaskReplayPolicy,
    progress: u8,
    attempt: u32,
    max_attempts: u32,
    revision: i64,
    created_at: i64,
    updated_at: i64,
    started_at: Option<i64>,
    finished_at: Option<i64>,
    last_error: Option<String>,
}

impl TaskEngine {
    pub fn open(database_path: &Path) -> Result<Self, HostError> {
        if let Some(parent) = database_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                HostError::internal(format!("create task data directory failed: {error}"))
            })?;
        }
        let connection = Connection::open(database_path).map_err(sql_error)?;
        Self::from_connection(connection)
    }

    pub fn from_connection(connection: Connection) -> Result<Self, HostError> {
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(sql_error)?;
        migrate(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn migrate(connection: &Connection) -> Result<(), HostError> {
        migrate(connection)
    }

    pub fn execute_command(
        &self,
        command: TaskCommandEnvelope,
    ) -> Result<TaskCommandOutcome, HostError> {
        validate_task_command(&command)?;
        let fingerprint = task_command_fingerprint(&command)?;
        let command_type = task_command_type(&command);
        let meta = task_command_meta(&command);

        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        if let Some(response) = find_existing_task_receipt(
            &transaction,
            &meta.command_id,
            &meta.idempotency_key,
            &fingerprint,
        )? {
            transaction.commit().map_err(sql_error)?;
            return Ok(TaskCommandOutcome {
                response,
                emitted_events: Vec::new(),
            });
        }

        validate_deadline(meta.deadline_at)?;
        let (task, event_type) = match command {
            TaskCommandEnvelope::Create { payload, .. } => {
                let task = create_new_tx(&transaction, parse_create_payload(payload)?)?;
                (task, TaskEventType::Created)
            }
            TaskCommandEnvelope::Cancel { payload, .. } => {
                let expected_revision = meta.expected_revision.ok_or_else(|| {
                    HostError::validation("task.cancel requires expectedRevision")
                })?;
                let task = cancel_task_tx(
                    &transaction,
                    &payload.task_id,
                    expected_revision,
                    payload.reason.as_deref(),
                )?;
                (task, TaskEventType::Canceled)
            }
            TaskCommandEnvelope::Retry { payload, .. } => {
                if !payload.approved {
                    return Err(HostError::new(
                        "TASK_RETRY_NOT_APPROVED",
                        "retry command requires approved=true",
                        false,
                    ));
                }
                let expected_revision = meta
                    .expected_revision
                    .ok_or_else(|| HostError::validation("task.retry requires expectedRevision"))?;
                let task = retry_task_tx(&transaction, &payload.task_id, expected_revision)?;
                (task, TaskEventType::Retried)
            }
        };

        let event = append_task_event(&transaction, event_type, &task, &meta.context.trace_id)?;
        let completed_at = now_millis();
        let response = TaskCommandResponse {
            receipt: CommandReceipt {
                command_id: meta.command_id.clone(),
                idempotency_key: meta.idempotency_key.clone(),
                command_type: command_type.to_string(),
                aggregate_id: task.id.clone(),
                revision: task.revision,
                last_event_sequence: event.sequence,
                completed_at,
            },
            task,
            replayed: false,
        };
        let response_json = serde_json::to_string(&response).map_err(json_error)?;
        transaction
            .execute(
                "INSERT INTO task_command_receipts
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
        Ok(TaskCommandOutcome {
            response,
            emitted_events: vec![event],
        })
    }

    pub fn replay_events(
        &self,
        after_sequence: i64,
        limit: u32,
    ) -> Result<Vec<TaskDomainEvent>, HostError> {
        if after_sequence < 0 {
            return Err(HostError::validation("afterSequence cannot be negative"));
        }
        let limit = limit.clamp(1, 1_000) as i64;
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT sequence, event_id, event_type, aggregate_id, revision,
                        occurred_at, trace_id, payload_json
                 FROM task_events WHERE sequence > ?1 ORDER BY sequence ASC LIMIT ?2",
            )
            .map_err(sql_error)?;
        let events = statement
            .query_map(params![after_sequence, limit], task_event_from_row)
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        Ok(events)
    }

    pub fn create(&self, payload: CreateTaskPayload) -> Result<TaskRecord, HostError> {
        let input = parse_create_payload(payload)?;
        self.create_new(input)
    }

    pub fn create_task(&self, payload: CreateTaskPayload) -> Result<TaskRecord, HostError> {
        self.create(payload)
    }

    pub fn list(&self) -> Result<Vec<TaskRecord>, HostError> {
        self.list_matching(None, None)
    }

    pub fn list_for_project(&self, project_id: &str) -> Result<Vec<TaskRecord>, HostError> {
        self.list_matching(Some(project_id), None)
    }

    pub fn list_by_status(&self, status: TaskStatus) -> Result<Vec<TaskRecord>, HostError> {
        self.list_matching(None, Some(status))
    }

    pub fn list_matching(
        &self,
        project_id: Option<&str>,
        status: Option<TaskStatus>,
    ) -> Result<Vec<TaskRecord>, HostError> {
        let connection = self.lock()?;
        let status_value = status.as_ref().map(status_to_db);
        let mut statement = connection
            .prepare(&format!(
                "SELECT {TASK_COLUMNS} FROM tasks
                 WHERE (?1 IS NULL OR project_id = ?1)
                   AND (?2 IS NULL OR status = ?2)
                 ORDER BY created_at DESC, id DESC"
            ))
            .map_err(sql_error)?;
        let ids = statement
            .query_map(params![project_id, status_value], |row| {
                row.get::<_, String>(0)
            })
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        drop(statement);
        ids.iter()
            .map(|task_id| load_task(&connection, task_id))
            .collect()
    }

    pub fn get(&self, task_id: &str) -> Result<TaskRecord, HostError> {
        let connection = self.lock()?;
        load_task(&connection, task_id)
    }

    pub fn get_task(&self, task_id: &str) -> Result<TaskRecord, HostError> {
        self.get(task_id)
    }

    pub fn cancel(
        &self,
        payload: CancelTaskPayload,
        expected_revision: i64,
    ) -> Result<TaskRecord, HostError> {
        self.cancel_task_with_reason(
            &payload.task_id,
            expected_revision,
            payload.reason.as_deref(),
        )
    }

    pub fn cancel_task(
        &self,
        task_id: &str,
        expected_revision: i64,
    ) -> Result<TaskRecord, HostError> {
        self.cancel_task_with_reason(task_id, expected_revision, None)
    }

    fn cancel_task_with_reason(
        &self,
        task_id: &str,
        expected_revision: i64,
        reason: Option<&str>,
    ) -> Result<TaskRecord, HostError> {
        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        let task = cancel_task_tx(&transaction, task_id, expected_revision, reason)?;
        append_task_event(
            &transaction,
            TaskEventType::Canceled,
            &task,
            LIFECYCLE_TRACE_ID,
        )?;
        transaction.commit().map_err(sql_error)?;
        Ok(task)
    }

    pub fn retry(
        &self,
        payload: RetryTaskPayload,
        expected_revision: i64,
    ) -> Result<TaskRecord, HostError> {
        if !payload.approved {
            return Err(HostError::new(
                "TASK_RETRY_NOT_APPROVED",
                "retry command requires approved=true",
                false,
            ));
        }
        self.retry_task(&payload.task_id, expected_revision)
    }

    pub fn retry_task(
        &self,
        task_id: &str,
        expected_revision: i64,
    ) -> Result<TaskRecord, HostError> {
        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        let task = retry_task_tx(&transaction, task_id, expected_revision)?;
        append_task_event(
            &transaction,
            TaskEventType::Retried,
            &task,
            LIFECYCLE_TRACE_ID,
        )?;
        transaction.commit().map_err(sql_error)?;
        Ok(task)
    }

    /// Atomically claims the highest-priority runnable task. A dependency that is missing,
    /// failed, canceled, queued, running, or awaiting approval blocks the dependent task.
    pub fn claim_next_runnable(&self) -> Result<Option<TaskRecord>, HostError> {
        Ok(self
            .claim_next_runnable_with_events(LIFECYCLE_TRACE_ID)?
            .task)
    }

    /// `Progressed(progress=0)` is the frozen-protocol representation of a queued task being
    /// claimed and entering `running`; TaskEventType currently has no dedicated running event.
    pub fn claim_next_runnable_with_events(
        &self,
        trace_id: &str,
    ) -> Result<TaskClaimOutcome, HostError> {
        self.claim_next_runnable_internal(trace_id, None)
    }

    pub fn claim_next_runnable_for_kinds_with_events(
        &self,
        kinds: &[String],
        trace_id: &str,
    ) -> Result<TaskClaimOutcome, HostError> {
        if kinds.is_empty() {
            return Ok(TaskClaimOutcome {
                task: None,
                emitted_events: Vec::new(),
            });
        }
        self.claim_next_runnable_internal(trace_id, Some(kinds))
    }

    fn claim_next_runnable_internal(
        &self,
        trace_id: &str,
        allowed_kinds: Option<&[String]>,
    ) -> Result<TaskClaimOutcome, HostError> {
        validate_lifecycle_trace(trace_id)?;
        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        let query_prefix = "SELECT task.id FROM tasks task
                 WHERE task.status = 'queued'
                   AND NOT EXISTS (
                     SELECT 1 FROM task_dependencies dependency
                     LEFT JOIN tasks prerequisite ON prerequisite.id = dependency.depends_on_task_id
                     WHERE dependency.task_id = task.id
                        AND (prerequisite.id IS NULL OR prerequisite.status <> 'succeeded')
                   )";
        let query_suffix = "ORDER BY CASE task.priority
                     WHEN 'critical' THEN 4 WHEN 'high' THEN 3
                     WHEN 'normal' THEN 2 WHEN 'low' THEN 1 ELSE 0 END DESC,
                     task.created_at ASC, task.id ASC
                 LIMIT 1";
        let task_id = if let Some(kinds) = allowed_kinds {
            let placeholders = (1..=kinds.len())
                .map(|index| format!("?{index}"))
                .collect::<Vec<_>>()
                .join(", ");
            let query = format!("{query_prefix} AND task.kind IN ({placeholders}) {query_suffix}");
            transaction
                .query_row(&query, params_from_iter(kinds.iter()), |row| {
                    row.get::<_, String>(0)
                })
                .optional()
                .map_err(sql_error)?
        } else {
            let query = format!("{query_prefix} {query_suffix}");
            transaction
                .query_row(&query, [], |row| row.get::<_, String>(0))
                .optional()
                .map_err(sql_error)?
        };
        let Some(task_id) = task_id else {
            transaction.commit().map_err(sql_error)?;
            return Ok(TaskClaimOutcome {
                task: None,
                emitted_events: Vec::new(),
            });
        };
        let current = load_task(&transaction, &task_id)?;
        let now = now_millis();
        let next_attempt = current.attempt.saturating_add(1);
        let next_revision = current.revision + 1;
        let changed = transaction
            .execute(
                "UPDATE tasks SET status = 'running', attempt = ?2, revision = ?3,
                 updated_at = ?4, started_at = ?4, finished_at = NULL, last_error = NULL
                 WHERE id = ?1 AND status = 'queued' AND revision = ?5",
                params![task_id, next_attempt, next_revision, now, current.revision],
            )
            .map_err(sql_error)?;
        ensure_changed(changed, &task_id)?;
        transaction
            .execute(
                "INSERT INTO task_attempts
                 (task_id, attempt, status, started_at, finished_at, error, output_json)
                 VALUES (?1, ?2, 'running', ?3, NULL, NULL, NULL)",
                params![task_id, next_attempt, now],
            )
            .map_err(sql_error)?;
        append_history(
            &transaction,
            &task_id,
            Some(&TaskStatus::Queued),
            &TaskStatus::Running,
            next_revision,
            now,
            Some("claimed"),
        )?;
        let task = load_task(&transaction, &task_id)?;
        let event = append_task_event(&transaction, TaskEventType::Progressed, &task, trace_id)?;
        transaction.commit().map_err(sql_error)?;
        Ok(TaskClaimOutcome {
            task: Some(task),
            emitted_events: vec![event],
        })
    }

    pub fn update_progress(
        &self,
        task_id: &str,
        attempt: u32,
        progress: u8,
    ) -> Result<TaskRecord, HostError> {
        Ok(self
            .update_progress_with_events(task_id, attempt, progress, LIFECYCLE_TRACE_ID)?
            .task)
    }

    pub fn update_progress_with_events(
        &self,
        task_id: &str,
        attempt: u32,
        progress: u8,
        trace_id: &str,
    ) -> Result<TaskLifecycleOutcome, HostError> {
        if progress > 100 {
            return Err(HostError::validation("task progress must be in 0..=100"));
        }
        validate_lifecycle_trace(trace_id)?;
        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        let current = load_running_attempt(&transaction, task_id, attempt)?;
        if progress < current.progress {
            return Err(HostError::new(
                "TASK_PROGRESS_REGRESSION",
                "task progress cannot move backwards",
                false,
            ));
        }
        if progress == current.progress {
            transaction.commit().map_err(sql_error)?;
            return Ok(TaskLifecycleOutcome {
                task: current,
                emitted_events: Vec::new(),
            });
        }
        let now = now_millis();
        transaction
            .execute(
                "UPDATE tasks SET progress = ?2, revision = revision + 1, updated_at = ?3
                 WHERE id = ?1 AND status = 'running' AND attempt = ?4",
                params![task_id, progress, now, attempt],
            )
            .map_err(sql_error)?;
        let task = load_task(&transaction, task_id)?;
        let event = append_task_event(&transaction, TaskEventType::Progressed, &task, trace_id)?;
        transaction.commit().map_err(sql_error)?;
        Ok(TaskLifecycleOutcome {
            task,
            emitted_events: vec![event],
        })
    }

    /// `attempt` is a fencing token. Results from an interrupted or superseded worker are
    /// rejected instead of being written into a later execution of the same task.
    pub fn finish_success(
        &self,
        task_id: &str,
        attempt: u32,
        output: Value,
    ) -> Result<TaskRecord, HostError> {
        Ok(self
            .finish_success_with_events(task_id, attempt, output, LIFECYCLE_TRACE_ID)?
            .task)
    }

    pub fn finish_success_with_events(
        &self,
        task_id: &str,
        attempt: u32,
        output: Value,
        trace_id: &str,
    ) -> Result<TaskLifecycleOutcome, HostError> {
        validate_lifecycle_trace(trace_id)?;
        let output_json = serde_json::to_string(&output).map_err(json_error)?;
        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        let current = load_running_attempt(&transaction, task_id, attempt)?;
        let now = now_millis();
        let next_revision = current.revision + 1;
        let changed = transaction
            .execute(
                "UPDATE tasks SET output_json = ?2, status = 'succeeded', progress = 100,
                 revision = ?3, updated_at = ?4, finished_at = ?4, last_error = NULL
                 WHERE id = ?1 AND status = 'running' AND attempt = ?5 AND revision = ?6",
                params![
                    task_id,
                    output_json,
                    next_revision,
                    now,
                    attempt,
                    current.revision
                ],
            )
            .map_err(sql_error)?;
        ensure_attempt_changed(changed, task_id, attempt)?;
        close_attempt(
            &transaction,
            task_id,
            attempt,
            "succeeded",
            now,
            None,
            Some(&output_json),
        )?;
        append_history(
            &transaction,
            task_id,
            Some(&TaskStatus::Running),
            &TaskStatus::Succeeded,
            next_revision,
            now,
            Some("completed"),
        )?;
        let task = load_task(&transaction, task_id)?;
        let event = append_task_event(&transaction, TaskEventType::Succeeded, &task, trace_id)?;
        transaction.commit().map_err(sql_error)?;
        Ok(TaskLifecycleOutcome {
            task,
            emitted_events: vec![event],
        })
    }

    pub fn finish_failure(
        &self,
        task_id: &str,
        attempt: u32,
        error: impl Into<String>,
    ) -> Result<TaskRecord, HostError> {
        Ok(self
            .finish_failure_with_events(task_id, attempt, error, LIFECYCLE_TRACE_ID)?
            .task)
    }

    pub fn finish_failure_with_events(
        &self,
        task_id: &str,
        attempt: u32,
        error: impl Into<String>,
        trace_id: &str,
    ) -> Result<TaskLifecycleOutcome, HostError> {
        self.finish_failure_internal(task_id, attempt, error.into(), true, trace_id)
    }

    pub fn finish_handler_failure_with_events(
        &self,
        task_id: &str,
        attempt: u32,
        error: impl Into<String>,
        retryable: bool,
        trace_id: &str,
    ) -> Result<TaskLifecycleOutcome, HostError> {
        self.finish_failure_internal(task_id, attempt, error.into(), retryable, trace_id)
    }

    fn finish_failure_internal(
        &self,
        task_id: &str,
        attempt: u32,
        error: String,
        retryable: bool,
        trace_id: &str,
    ) -> Result<TaskLifecycleOutcome, HostError> {
        validate_lifecycle_trace(trace_id)?;
        let error = normalize_error(error)?;
        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        let current = load_running_attempt(&transaction, task_id, attempt)?;
        let attempts_remain = current.attempt < current.max_attempts;
        let next_status = if !retryable || !attempts_remain {
            TaskStatus::Failed
        } else {
            match current.replay_policy {
                TaskReplayPolicy::Safe => TaskStatus::Queued,
                TaskReplayPolicy::Manual => TaskStatus::AwaitingApproval,
                TaskReplayPolicy::Never => TaskStatus::Failed,
            }
        };
        let now = now_millis();
        let next_revision = current.revision + 1;
        let finished_at = matches!(&next_status, TaskStatus::Failed).then_some(now);
        let changed = transaction
            .execute(
                "UPDATE tasks SET status = ?2, progress = 0, revision = ?3, updated_at = ?4,
                 started_at = NULL, finished_at = ?5, last_error = ?6
                 WHERE id = ?1 AND status = 'running' AND attempt = ?7 AND revision = ?8",
                params![
                    task_id,
                    status_to_db(&next_status),
                    next_revision,
                    now,
                    finished_at,
                    error,
                    attempt,
                    current.revision
                ],
            )
            .map_err(sql_error)?;
        ensure_attempt_changed(changed, task_id, attempt)?;
        close_attempt(
            &transaction,
            task_id,
            attempt,
            "failed",
            now,
            Some(&error),
            None,
        )?;
        append_history(
            &transaction,
            task_id,
            Some(&TaskStatus::Running),
            &next_status,
            next_revision,
            now,
            Some("execution-failed"),
        )?;
        let task = load_task(&transaction, task_id)?;
        let event = append_task_event(
            &transaction,
            failure_event_type(&next_status),
            &task,
            trace_id,
        )?;
        transaction.commit().map_err(sql_error)?;
        Ok(TaskLifecycleOutcome {
            task,
            emitted_events: vec![event],
        })
    }

    /// Restores tasks left running after process termination. Only `safe` tasks are queued
    /// automatically. `manual` and `never` always require an explicit operator decision.
    pub fn recover_interrupted(&self) -> Result<Vec<TaskRecord>, HostError> {
        Ok(self
            .recover_interrupted_with_events(LIFECYCLE_TRACE_ID)?
            .tasks)
    }

    pub fn recover_interrupted_with_events(
        &self,
        trace_id: &str,
    ) -> Result<TaskRecoveryOutcome, HostError> {
        validate_lifecycle_trace(trace_id)?;
        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        let running_ids = {
            let mut statement = transaction
                .prepare("SELECT id FROM tasks WHERE status = 'running' ORDER BY created_at, id")
                .map_err(sql_error)?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(sql_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(sql_error)?
        };
        let mut recovered = Vec::with_capacity(running_ids.len());
        let mut emitted_events = Vec::with_capacity(running_ids.len());
        for task_id in running_ids {
            let current = load_task(&transaction, &task_id)?;
            let next_status = match current.replay_policy {
                TaskReplayPolicy::Safe => TaskStatus::Queued,
                TaskReplayPolicy::Manual | TaskReplayPolicy::Never => TaskStatus::AwaitingApproval,
            };
            let now = now_millis();
            let next_revision = current.revision + 1;
            let changed = transaction
                .execute(
                    "UPDATE tasks SET status = ?2, progress = 0, revision = ?3, updated_at = ?4,
                     started_at = NULL, finished_at = NULL, last_error = ?5
                     WHERE id = ?1 AND status = 'running' AND revision = ?6",
                    params![
                        task_id,
                        status_to_db(&next_status),
                        next_revision,
                        now,
                        "interrupted by host shutdown",
                        current.revision
                    ],
                )
                .map_err(sql_error)?;
            ensure_changed(changed, &task_id)?;
            close_attempt(
                &transaction,
                &task_id,
                current.attempt,
                "interrupted",
                now,
                Some("interrupted by host shutdown"),
                None,
            )?;
            append_history(
                &transaction,
                &task_id,
                Some(&TaskStatus::Running),
                &next_status,
                next_revision,
                now,
                Some("host-recovery"),
            )?;
            let task = load_task(&transaction, &task_id)?;
            emitted_events.push(append_task_event(
                &transaction,
                TaskEventType::Recovered,
                &task,
                trace_id,
            )?);
            recovered.push(task);
        }
        transaction.commit().map_err(sql_error)?;
        Ok(TaskRecoveryOutcome {
            tasks: recovered,
            emitted_events,
        })
    }

    fn create_new(&self, input: NewTask) -> Result<TaskRecord, HostError> {
        let mut connection = self.lock()?;
        let transaction = immediate(&mut connection)?;
        let task = create_new_tx(&transaction, input)?;
        append_task_event(
            &transaction,
            TaskEventType::Created,
            &task,
            LIFECYCLE_TRACE_ID,
        )?;
        transaction.commit().map_err(sql_error)?;
        Ok(task)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, HostError> {
        self.connection
            .lock()
            .map_err(|_| HostError::internal("task SQLite lock is poisoned"))
    }
}

fn create_new_tx(transaction: &Transaction<'_>, input: NewTask) -> Result<TaskRecord, HostError> {
    validate_new_task(&input)?;
    if load_stored_task_optional(transaction, &input.id)?.is_some() {
        return Err(HostError::new(
            "TASK_ID_REUSED",
            format!("task {} already exists", input.id),
            false,
        ));
    }
    validate_dependencies(transaction, &input)?;
    let now = now_millis();
    let input_json = serde_json::to_string(&input.input).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO tasks
             (id, kind, project_id, input_json, output_json, status, priority, replay_policy,
              progress, attempt, max_attempts, revision, created_at, updated_at,
              started_at, finished_at, last_error)
             VALUES (?1, ?2, ?3, ?4, NULL, 'queued', ?5, ?6, 0, 0, ?7, 1, ?8, ?8,
                     NULL, NULL, NULL)",
            params![
                input.id,
                input.kind,
                input.project_id,
                input_json,
                priority_to_db(&input.priority),
                replay_policy_to_db(&input.replay_policy),
                input.max_attempts,
                now
            ],
        )
        .map_err(sql_error)?;
    for dependency in &input.dependencies {
        transaction
            .execute(
                "INSERT INTO task_dependencies
                 (task_id, depends_on_task_id, dependency_json, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    input.id,
                    dependency.task_id,
                    serde_json::to_string(&dependency.wire_value).map_err(json_error)?,
                    now
                ],
            )
            .map_err(sql_error)?;
    }
    append_history(
        transaction,
        &input.id,
        None,
        &TaskStatus::Queued,
        1,
        now,
        Some("created"),
    )?;
    load_task(transaction, &input.id)
}

fn cancel_task_tx(
    transaction: &Transaction<'_>,
    task_id: &str,
    expected_revision: i64,
    reason: Option<&str>,
) -> Result<TaskRecord, HostError> {
    let current = load_task(transaction, task_id)?;
    ensure_revision(&current, expected_revision)?;
    if !matches!(
        &current.status,
        TaskStatus::Queued | TaskStatus::Running | TaskStatus::AwaitingApproval
    ) {
        return Err(HostError::new(
            "TASK_NOT_CANCELABLE",
            format!(
                "task {task_id} is not cancelable from {}",
                status_to_db(&current.status)
            ),
            false,
        ));
    }
    let now = now_millis();
    let next_revision = current.revision + 1;
    let changed = transaction
        .execute(
            "UPDATE tasks SET status = 'canceled', revision = ?2, updated_at = ?3,
             finished_at = ?3, last_error = NULL WHERE id = ?1 AND revision = ?4",
            params![task_id, next_revision, now, expected_revision],
        )
        .map_err(sql_error)?;
    ensure_changed(changed, task_id)?;
    if matches!(&current.status, TaskStatus::Running) {
        close_attempt(
            transaction,
            task_id,
            current.attempt,
            "canceled",
            now,
            None,
            None,
        )?;
    }
    append_history(
        transaction,
        task_id,
        Some(&current.status),
        &TaskStatus::Canceled,
        next_revision,
        now,
        reason.or(Some("cancel-requested")),
    )?;
    load_task(transaction, task_id)
}

fn retry_task_tx(
    transaction: &Transaction<'_>,
    task_id: &str,
    expected_revision: i64,
) -> Result<TaskRecord, HostError> {
    let current = load_task(transaction, task_id)?;
    ensure_revision(&current, expected_revision)?;
    if !matches!(
        &current.status,
        TaskStatus::Failed | TaskStatus::AwaitingApproval
    ) {
        return Err(HostError::new(
            "TASK_NOT_RETRYABLE",
            format!("task {task_id} is not awaiting a retry decision"),
            false,
        ));
    }
    if current.attempt >= current.max_attempts {
        return Err(HostError::new(
            "TASK_ATTEMPTS_EXHAUSTED",
            format!("task {task_id} has exhausted maxAttempts"),
            false,
        ));
    }
    let now = now_millis();
    let next_revision = current.revision + 1;
    let changed = transaction
        .execute(
            "UPDATE tasks SET status = 'queued', progress = 0, output_json = NULL,
             revision = ?2, updated_at = ?3, started_at = NULL, finished_at = NULL,
             last_error = NULL WHERE id = ?1 AND revision = ?4",
            params![task_id, next_revision, now, expected_revision],
        )
        .map_err(sql_error)?;
    ensure_changed(changed, task_id)?;
    append_history(
        transaction,
        task_id,
        Some(&current.status),
        &TaskStatus::Queued,
        next_revision,
        now,
        Some("retry-approved"),
    )?;
    load_task(transaction, task_id)
}

pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY NOT NULL,
                kind TEXT NOT NULL,
                project_id TEXT,
                input_json TEXT NOT NULL,
                output_json TEXT,
                status TEXT NOT NULL CHECK(status IN
                    ('queued','running','succeeded','failed','canceled','awaitingApproval')),
                priority TEXT NOT NULL CHECK(priority IN ('low','normal','high','critical')),
                replay_policy TEXT NOT NULL CHECK(replay_policy IN ('safe','manual','never')),
                progress INTEGER NOT NULL CHECK(progress BETWEEN 0 AND 100),
                attempt INTEGER NOT NULL CHECK(attempt >= 0),
                max_attempts INTEGER NOT NULL CHECK(max_attempts > 0),
                revision INTEGER NOT NULL CHECK(revision > 0),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                started_at INTEGER,
                finished_at INTEGER,
                last_error TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_tasks_runnable
                ON tasks(status, priority, created_at, id);
            CREATE INDEX IF NOT EXISTS idx_tasks_project
                ON tasks(project_id, updated_at DESC);
            CREATE TABLE IF NOT EXISTS task_dependencies (
                task_id TEXT NOT NULL,
                depends_on_task_id TEXT NOT NULL,
                dependency_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY(task_id, depends_on_task_id),
                FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE,
                FOREIGN KEY(depends_on_task_id) REFERENCES tasks(id) ON DELETE RESTRICT,
                CHECK(task_id <> depends_on_task_id)
            );
            CREATE INDEX IF NOT EXISTS idx_task_dependencies_prerequisite
                ON task_dependencies(depends_on_task_id, task_id);
            CREATE TABLE IF NOT EXISTS task_attempts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                attempt INTEGER NOT NULL,
                status TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                finished_at INTEGER,
                error TEXT,
                output_json TEXT,
                UNIQUE(task_id, attempt),
                FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_task_attempts_task
                ON task_attempts(task_id, attempt DESC);
            CREATE TABLE IF NOT EXISTS task_state_history (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                from_status TEXT,
                to_status TEXT NOT NULL,
                revision INTEGER NOT NULL,
                occurred_at INTEGER NOT NULL,
                reason TEXT,
                FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_task_state_history_task
                ON task_state_history(task_id, sequence);
            CREATE TABLE IF NOT EXISTS task_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL,
                aggregate_id TEXT NOT NULL,
                revision INTEGER NOT NULL,
                occurred_at INTEGER NOT NULL,
                trace_id TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                FOREIGN KEY(aggregate_id) REFERENCES tasks(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_task_events_aggregate
                ON task_events(aggregate_id, sequence);
            CREATE TABLE IF NOT EXISTS task_command_receipts (
                idempotency_key TEXT PRIMARY KEY NOT NULL,
                command_id TEXT NOT NULL UNIQUE,
                command_type TEXT NOT NULL,
                request_fingerprint TEXT NOT NULL,
                response_json TEXT NOT NULL,
                completed_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_task_command_receipts_completed
                ON task_command_receipts(completed_at);
            "#,
        )
        .map_err(sql_error)
}

fn validate_task_command(command: &TaskCommandEnvelope) -> Result<(), HostError> {
    let meta = task_command_meta(command);
    if !is_legacy_surface_protocol_supported(&meta.protocol_version) {
        return Err(HostError::new(
            "PROTOCOL_VERSION_UNSUPPORTED",
            format!(
                "task command protocol {} is unsupported; expected {}, {}, {}, or {}",
                meta.protocol_version,
                LEGACY_PROTOCOL_VERSION,
                PROTOCOL_1_3_VERSION,
                PREVIOUS_PROTOCOL_VERSION,
                PROTOCOL_VERSION
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
    if meta.context.actor_id.trim().is_empty()
        || meta.context.window_id.trim().is_empty()
        || meta.context.trace_id.trim().is_empty()
    {
        return Err(HostError::validation(
            "actorId, windowId, and traceId are required",
        ));
    }
    match command {
        TaskCommandEnvelope::Create {
            expected_revision, ..
        } if expected_revision.is_some() => Err(HostError::validation(
            "task.create rejects expectedRevision",
        )),
        TaskCommandEnvelope::Cancel {
            expected_revision, ..
        }
        | TaskCommandEnvelope::Retry {
            expected_revision, ..
        } if !expected_revision.is_some_and(|revision| revision > 0) => Err(HostError::validation(
            "task.cancel and task.retry require expectedRevision > 0",
        )),
        _ => Ok(()),
    }
}

fn validate_deadline(deadline_at: Option<i64>) -> Result<(), HostError> {
    if deadline_at.is_some_and(|deadline| deadline < now_millis()) {
        return Err(HostError::new(
            "COMMAND_DEADLINE_EXCEEDED",
            "task command deadline has elapsed",
            false,
        ));
    }
    Ok(())
}

fn validate_lifecycle_trace(trace_id: &str) -> Result<(), HostError> {
    let length = trace_id.trim().chars().count();
    if length == 0 || length > 160 {
        return Err(HostError::validation(
            "lifecycle traceId length must be 1..160",
        ));
    }
    Ok(())
}

fn failure_event_type(status: &TaskStatus) -> TaskEventType {
    match status {
        TaskStatus::Queued => TaskEventType::Recovered,
        TaskStatus::AwaitingApproval => TaskEventType::AwaitingApproval,
        TaskStatus::Failed => TaskEventType::Failed,
        _ => TaskEventType::Failed,
    }
}

fn task_command_meta(command: &TaskCommandEnvelope) -> TaskCommandMeta {
    match command {
        TaskCommandEnvelope::Create {
            command_id,
            protocol_version,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        }
        | TaskCommandEnvelope::Cancel {
            command_id,
            protocol_version,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        }
        | TaskCommandEnvelope::Retry {
            command_id,
            protocol_version,
            context,
            idempotency_key,
            expected_revision,
            deadline_at,
            ..
        } => TaskCommandMeta {
            command_id: command_id.clone(),
            protocol_version: protocol_version.clone(),
            context: context.clone(),
            idempotency_key: idempotency_key.clone(),
            expected_revision: *expected_revision,
            deadline_at: *deadline_at,
        },
    }
}

fn task_command_type(command: &TaskCommandEnvelope) -> &'static str {
    match command {
        TaskCommandEnvelope::Create { .. } => "task.create",
        TaskCommandEnvelope::Cancel { .. } => "task.cancel",
        TaskCommandEnvelope::Retry { .. } => "task.retry",
    }
}

fn task_command_fingerprint(command: &TaskCommandEnvelope) -> Result<String, HostError> {
    let meta = task_command_meta(command);
    let context = serde_json::json!({
        "actorId": meta.context.actor_id,
        "accountId": meta.context.account_id,
        "projectId": meta.context.project_id,
    });
    let value = match command {
        TaskCommandEnvelope::Create { payload, .. } => serde_json::json!({
            "commandType": task_command_type(command),
            "context": context,
            "expectedRevision": meta.expected_revision,
            "payload": payload,
        }),
        TaskCommandEnvelope::Cancel { payload, .. } => serde_json::json!({
            "commandType": task_command_type(command),
            "context": context,
            "expectedRevision": meta.expected_revision,
            "payload": payload,
        }),
        TaskCommandEnvelope::Retry { payload, .. } => serde_json::json!({
            "commandType": task_command_type(command),
            "context": context,
            "expectedRevision": meta.expected_revision,
            "payload": payload,
        }),
    };
    serde_json::to_string(&value).map_err(json_error)
}

struct StoredTaskReceipt {
    command_id: String,
    idempotency_key: String,
    fingerprint: String,
    response_json: String,
}

fn find_existing_task_receipt(
    transaction: &Transaction<'_>,
    command_id: &str,
    idempotency_key: &str,
    fingerprint: &str,
) -> Result<Option<TaskCommandResponse>, HostError> {
    let by_key = transaction
        .query_row(
            "SELECT command_id, idempotency_key, request_fingerprint, response_json
             FROM task_command_receipts WHERE idempotency_key = ?1",
            [idempotency_key],
            |row| {
                Ok(StoredTaskReceipt {
                    command_id: row.get(0)?,
                    idempotency_key: row.get(1)?,
                    fingerprint: row.get(2)?,
                    response_json: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(sql_error)?;
    let by_command = transaction
        .query_row(
            "SELECT command_id, idempotency_key, request_fingerprint, response_json
             FROM task_command_receipts WHERE command_id = ?1",
            [command_id],
            |row| {
                Ok(StoredTaskReceipt {
                    command_id: row.get(0)?,
                    idempotency_key: row.get(1)?,
                    fingerprint: row.get(2)?,
                    response_json: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(sql_error)?;

    if by_key
        .as_ref()
        .is_some_and(|stored| stored.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "IDEMPOTENCY_KEY_REUSED",
            "idempotencyKey reused for a different task request",
            false,
        ));
    }
    if by_command
        .as_ref()
        .is_some_and(|stored| stored.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "COMMAND_ID_REUSED",
            "commandId reused for a different task request",
            false,
        ));
    }
    if let (Some(key_receipt), Some(command_receipt)) = (&by_key, &by_command) {
        if key_receipt.command_id != command_receipt.command_id
            || key_receipt.idempotency_key != command_receipt.idempotency_key
        {
            return Err(HostError::new(
                "COMMAND_IDENTITY_COLLISION",
                "commandId and idempotencyKey identify different committed task commands",
                false,
            ));
        }
    }
    let stored = by_key.or(by_command);
    stored
        .map(|stored| {
            let mut response: TaskCommandResponse =
                serde_json::from_str(&stored.response_json).map_err(json_error)?;
            response.replayed = true;
            Ok(response)
        })
        .transpose()
}

fn append_task_event(
    transaction: &Transaction<'_>,
    event_type: TaskEventType,
    task: &TaskRecord,
    trace_id: &str,
) -> Result<TaskDomainEvent, HostError> {
    let event_id = Uuid::new_v4().to_string();
    let occurred_at = now_millis();
    let payload_json = serde_json::to_string(task).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO task_events
             (event_id, event_type, aggregate_id, revision, occurred_at, trace_id, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event_id,
                event_type.as_wire_str(),
                task.id,
                task.revision,
                occurred_at,
                trace_id,
                payload_json
            ],
        )
        .map_err(sql_error)?;
    Ok(TaskDomainEvent {
        sequence: transaction.last_insert_rowid(),
        event_id,
        event_type,
        aggregate_id: task.id.clone(),
        revision: task.revision,
        occurred_at,
        trace_id: trace_id.to_string(),
        task: task.clone(),
    })
}

fn task_event_from_row(row: &Row<'_>) -> rusqlite::Result<TaskDomainEvent> {
    let event_type_value: String = row.get(2)?;
    let event_type = task_event_type_from_wire(&event_type_value)
        .ok_or_else(|| invalid_sql(format!("unknown task event type: {event_type_value}")))?;
    let payload_json: String = row.get(7)?;
    let task = serde_json::from_str(&payload_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            payload_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    Ok(TaskDomainEvent {
        sequence: row.get(0)?,
        event_id: row.get(1)?,
        event_type,
        aggregate_id: row.get(3)?,
        revision: row.get(4)?,
        occurred_at: row.get(5)?,
        trace_id: row.get(6)?,
        task,
    })
}

fn task_event_type_from_wire(value: &str) -> Option<TaskEventType> {
    Some(match value {
        "task.created" => TaskEventType::Created,
        "task.canceled" => TaskEventType::Canceled,
        "task.retried" => TaskEventType::Retried,
        "task.progressed" => TaskEventType::Progressed,
        "task.succeeded" => TaskEventType::Succeeded,
        "task.failed" => TaskEventType::Failed,
        "task.awaitingApproval" => TaskEventType::AwaitingApproval,
        "task.recovered" => TaskEventType::Recovered,
        _ => return None,
    })
}

fn parse_create_payload(payload: CreateTaskPayload) -> Result<NewTask, HostError> {
    let dependencies = payload
        .dependency_task_ids
        .into_iter()
        .map(|task_id| DependencyInput {
            wire_value: serde_json::json!({ "taskId": task_id }),
            task_id,
        })
        .collect();
    Ok(NewTask {
        id: Uuid::new_v4().to_string(),
        kind: payload.kind,
        project_id: payload.project_id,
        input: payload.input,
        priority: payload.priority,
        replay_policy: payload.replay_policy,
        max_attempts: payload.max_attempts,
        dependencies,
    })
}

fn validate_new_task(input: &NewTask) -> Result<(), HostError> {
    Uuid::parse_str(&input.id).map_err(|_| HostError::validation("task id must be a UUID"))?;
    let kind_length = input.kind.trim().chars().count();
    if !(1..=160).contains(&kind_length) {
        return Err(HostError::validation("task kind length must be 1..160"));
    }
    if input.max_attempts == 0 || input.max_attempts > 100 {
        return Err(HostError::validation("maxAttempts must be in 1..=100"));
    }
    let input_size = serde_json::to_vec(&input.input).map_err(json_error)?.len();
    if input_size > 16 * 1024 * 1024 {
        return Err(HostError::validation("task input exceeds 16 MiB"));
    }
    let mut unique = HashSet::new();
    for dependency in &input.dependencies {
        Uuid::parse_str(&dependency.task_id)
            .map_err(|_| HostError::validation("dependency task id must be a UUID"))?;
        if dependency.task_id == input.id {
            return Err(HostError::new(
                "TASK_DAG_CYCLE",
                "task cannot depend on itself",
                false,
            ));
        }
        if !unique.insert(&dependency.task_id) {
            return Err(HostError::validation("duplicate task dependency"));
        }
    }
    Ok(())
}

fn validate_dependencies(transaction: &Transaction<'_>, input: &NewTask) -> Result<(), HostError> {
    for dependency in &input.dependencies {
        let exists = transaction
            .query_row(
                "SELECT 1 FROM tasks WHERE id = ?1",
                [&dependency.task_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(sql_error)?
            .is_some();
        if !exists {
            return Err(HostError::new(
                "TASK_DEPENDENCY_NOT_FOUND",
                format!("dependency task {} does not exist", dependency.task_id),
                false,
            ));
        }
        // Existing dependencies can never reach the newly generated task, but retain a
        // recursive check for imported IDs and future bulk-import callers.
        let creates_cycle = transaction
            .query_row(
                "WITH RECURSIVE ancestors(id) AS (
                    SELECT depends_on_task_id FROM task_dependencies WHERE task_id = ?1
                    UNION
                    SELECT dependency.depends_on_task_id
                    FROM task_dependencies dependency JOIN ancestors ON dependency.task_id = ancestors.id
                 ) SELECT 1 FROM ancestors WHERE id = ?2 LIMIT 1",
                params![dependency.task_id, input.id],
                |_| Ok(()),
            )
            .optional()
            .map_err(sql_error)?
            .is_some();
        if creates_cycle {
            return Err(HostError::new(
                "TASK_DAG_CYCLE",
                "task dependency would create a cycle",
                false,
            ));
        }
    }
    Ok(())
}

fn load_task(connection: &Connection, task_id: &str) -> Result<TaskRecord, HostError> {
    let stored = load_stored_task_optional(connection, task_id)?.ok_or_else(|| {
        HostError::new(
            "TASK_NOT_FOUND",
            format!("task {task_id} does not exist"),
            false,
        )
    })?;
    let dependencies = load_dependencies(connection, task_id)?;
    Ok(TaskRecord {
        id: stored.id,
        kind: stored.kind,
        project_id: stored.project_id,
        input: stored.input,
        output: stored.output,
        status: stored.status,
        priority: stored.priority,
        replay_policy: stored.replay_policy,
        progress: stored.progress,
        attempt: stored.attempt,
        max_attempts: stored.max_attempts,
        revision: stored.revision,
        created_at: stored.created_at,
        updated_at: stored.updated_at,
        started_at: stored.started_at,
        finished_at: stored.finished_at,
        last_error: stored.last_error,
        dependencies,
    })
}

fn load_stored_task_optional(
    connection: &Connection,
    task_id: &str,
) -> Result<Option<StoredTask>, HostError> {
    connection
        .query_row(
            &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1"),
            [task_id],
            stored_task_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn stored_task_from_row(row: &Row<'_>) -> rusqlite::Result<StoredTask> {
    let input_json: String = row.get(3)?;
    let output_json: Option<String> = row.get(4)?;
    let status: String = row.get(5)?;
    let priority: String = row.get(6)?;
    let replay_policy: String = row.get(7)?;
    let progress: i64 = row.get(8)?;
    let attempt: i64 = row.get(9)?;
    let max_attempts: i64 = row.get(10)?;
    Ok(StoredTask {
        id: row.get(0)?,
        kind: row.get(1)?,
        project_id: row.get(2)?,
        input: json_from_sql(&input_json)?,
        output: output_json.as_deref().map(json_from_sql).transpose()?,
        status: status_from_db(&status).map_err(invalid_sql)?,
        priority: priority_from_db(&priority).map_err(invalid_sql)?,
        replay_policy: replay_policy_from_db(&replay_policy).map_err(invalid_sql)?,
        progress: u8::try_from(progress).map_err(invalid_sql)?,
        attempt: u32::try_from(attempt).map_err(invalid_sql)?,
        max_attempts: u32::try_from(max_attempts).map_err(invalid_sql)?,
        revision: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        started_at: row.get(14)?,
        finished_at: row.get(15)?,
        last_error: row.get(16)?,
    })
}

fn load_dependencies(
    connection: &Connection,
    task_id: &str,
) -> Result<Vec<TaskDependency>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT depends_on_task_id, dependency_json FROM task_dependencies
             WHERE task_id = ?1 ORDER BY created_at, depends_on_task_id",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map([task_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    rows.into_iter()
        .map(|(dependency_id, wire_json)| {
            let original: Value = serde_json::from_str(&wire_json).map_err(json_error)?;
            deserialize_dependency(original, &dependency_id)
        })
        .collect()
}

fn deserialize_dependency(value: Value, dependency_id: &str) -> Result<TaskDependency, HostError> {
    let candidates = [
        value,
        Value::String(dependency_id.to_string()),
        serde_json::json!({ "taskId": dependency_id }),
        serde_json::json!({ "dependsOnTaskId": dependency_id }),
        serde_json::json!({ "dependencyTaskId": dependency_id }),
    ];
    for candidate in candidates {
        if let Ok(dependency) = serde_json::from_value(candidate) {
            return Ok(dependency);
        }
    }
    Err(HostError::internal(format!(
        "stored dependency {dependency_id} is incompatible with TaskDependency"
    )))
}

fn load_running_attempt(
    transaction: &Transaction<'_>,
    task_id: &str,
    attempt: u32,
) -> Result<TaskRecord, HostError> {
    let task = load_task(transaction, task_id)?;
    if !matches!(task.status, TaskStatus::Running) || task.attempt != attempt {
        return Err(HostError::new(
            "TASK_ATTEMPT_STALE",
            format!("task {task_id} is not running attempt {attempt}"),
            false,
        ));
    }
    Ok(task)
}

fn close_attempt(
    transaction: &Transaction<'_>,
    task_id: &str,
    attempt: u32,
    status: &str,
    finished_at: i64,
    error: Option<&str>,
    output_json: Option<&str>,
) -> Result<(), HostError> {
    let changed = transaction
        .execute(
            "UPDATE task_attempts SET status = ?3, finished_at = ?4, error = ?5,
             output_json = ?6 WHERE task_id = ?1 AND attempt = ?2 AND status = 'running'",
            params![task_id, attempt, status, finished_at, error, output_json],
        )
        .map_err(sql_error)?;
    if changed != 1 {
        return Err(HostError::new(
            "TASK_ATTEMPT_STALE",
            format!("attempt {attempt} for task {task_id} is no longer active"),
            false,
        ));
    }
    Ok(())
}

fn append_history(
    transaction: &Transaction<'_>,
    task_id: &str,
    from: Option<&TaskStatus>,
    to: &TaskStatus,
    revision: i64,
    occurred_at: i64,
    reason: Option<&str>,
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO task_state_history
             (task_id, from_status, to_status, revision, occurred_at, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                task_id,
                from.map(status_to_db),
                status_to_db(to),
                revision,
                occurred_at,
                reason
            ],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn ensure_revision(task: &TaskRecord, expected_revision: i64) -> Result<(), HostError> {
    if task.revision != expected_revision {
        return Err(HostError::new(
            "TASK_REVISION_CONFLICT",
            format!(
                "task {} revision is {}, request expected {}",
                task.id, task.revision, expected_revision
            ),
            false,
        ));
    }
    Ok(())
}

fn ensure_changed(changed: usize, task_id: &str) -> Result<(), HostError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(HostError::new(
            "TASK_REVISION_CONFLICT",
            format!("task {task_id} changed during transaction"),
            false,
        ))
    }
}

fn ensure_attempt_changed(changed: usize, task_id: &str, attempt: u32) -> Result<(), HostError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(HostError::new(
            "TASK_ATTEMPT_STALE",
            format!("task {task_id} attempt {attempt} changed during completion"),
            false,
        ))
    }
}

fn immediate(connection: &mut Connection) -> Result<Transaction<'_>, HostError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)
}

fn status_to_db(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Queued => "queued",
        TaskStatus::Running => "running",
        TaskStatus::Succeeded => "succeeded",
        TaskStatus::Failed => "failed",
        TaskStatus::Canceled => "canceled",
        TaskStatus::AwaitingApproval => "awaitingApproval",
    }
}

fn status_from_db(value: &str) -> Result<TaskStatus, String> {
    match value {
        "queued" => Ok(TaskStatus::Queued),
        "running" => Ok(TaskStatus::Running),
        "succeeded" => Ok(TaskStatus::Succeeded),
        "failed" => Ok(TaskStatus::Failed),
        "canceled" => Ok(TaskStatus::Canceled),
        "awaitingApproval" => Ok(TaskStatus::AwaitingApproval),
        _ => Err(format!("unknown task status: {value}")),
    }
}

fn priority_to_db(priority: &TaskPriority) -> &'static str {
    match priority {
        TaskPriority::Low => "low",
        TaskPriority::Normal => "normal",
        TaskPriority::High => "high",
        TaskPriority::Critical => "critical",
    }
}

fn priority_from_db(value: &str) -> Result<TaskPriority, String> {
    match value {
        "low" => Ok(TaskPriority::Low),
        "normal" => Ok(TaskPriority::Normal),
        "high" => Ok(TaskPriority::High),
        "critical" => Ok(TaskPriority::Critical),
        _ => Err(format!("unknown task priority: {value}")),
    }
}

fn replay_policy_to_db(policy: &TaskReplayPolicy) -> &'static str {
    match policy {
        TaskReplayPolicy::Safe => "safe",
        TaskReplayPolicy::Manual => "manual",
        TaskReplayPolicy::Never => "never",
    }
}

fn replay_policy_from_db(value: &str) -> Result<TaskReplayPolicy, String> {
    match value {
        "safe" => Ok(TaskReplayPolicy::Safe),
        "manual" => Ok(TaskReplayPolicy::Manual),
        "never" => Ok(TaskReplayPolicy::Never),
        _ => Err(format!("unknown task replay policy: {value}")),
    }
}

fn normalize_error(error: String) -> Result<String, HostError> {
    let error = error.trim().to_string();
    if error.is_empty() {
        return Err(HostError::validation("task failure error is required"));
    }
    if error.chars().count() > 16_000 {
        return Err(HostError::validation(
            "task failure error exceeds 16000 characters",
        ));
    }
    Ok(error)
}

fn json_from_sql(value: &str) -> rusqlite::Result<Value> {
    serde_json::from_str(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            value.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn invalid_sql(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
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
    HostError::internal(format!("task SQLite operation failed: {error}"))
}

fn json_error(error: serde_json::Error) -> HostError {
    HostError::internal(format!("task JSON operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> TaskEngine {
        TaskEngine::from_connection(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn new_task(
        kind: &str,
        priority: TaskPriority,
        replay_policy: TaskReplayPolicy,
        max_attempts: u32,
    ) -> NewTask {
        NewTask {
            id: Uuid::new_v4().to_string(),
            kind: kind.to_string(),
            project_id: None,
            input: serde_json::json!({ "kind": kind }),
            priority,
            replay_policy,
            max_attempts,
            dependencies: Vec::new(),
        }
    }

    fn dependency(task_id: &str) -> DependencyInput {
        DependencyInput {
            task_id: task_id.to_string(),
            wire_value: serde_json::json!({ "taskId": task_id }),
        }
    }

    fn context(trace_id: &str) -> OperationContext {
        OperationContext {
            actor_id: "task-test-operator".to_string(),
            account_id: None,
            project_id: None,
            window_id: "task-test-window".to_string(),
            trace_id: trace_id.to_string(),
        }
    }

    fn create_command(
        command_id: &str,
        idempotency_key: &str,
        kind: &str,
        deadline_at: Option<i64>,
    ) -> TaskCommandEnvelope {
        TaskCommandEnvelope::Create {
            command_id: command_id.to_string(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            context: context("trace-create-task"),
            payload: CreateTaskPayload {
                kind: kind.to_string(),
                project_id: None,
                input: serde_json::json!({ "prompt": kind }),
                priority: TaskPriority::Normal,
                replay_policy: TaskReplayPolicy::Safe,
                max_attempts: 2,
                dependency_task_ids: Vec::new(),
            },
            idempotency_key: idempotency_key.to_string(),
            expected_revision: None,
            deadline_at,
        }
    }

    #[test]
    fn command_gateway_is_idempotent_and_emits_event_once() {
        let engine = engine();
        let key = "task-create-idempotent-001";
        let first = engine
            .execute_command(create_command(
                &Uuid::new_v4().to_string(),
                key,
                "image.generate",
                Some(now_millis() + 30_000),
            ))
            .unwrap();
        let replay = engine
            .execute_command(create_command(
                &Uuid::new_v4().to_string(),
                key,
                "image.generate",
                Some(now_millis() + 30_000),
            ))
            .unwrap();

        assert!(!first.response.replayed);
        assert!(replay.response.replayed);
        assert_eq!(first.response.task.id, replay.response.task.id);
        assert_eq!(first.emitted_events.len(), 1);
        assert!(replay.emitted_events.is_empty());
        assert_eq!(engine.list().unwrap().len(), 1);
        let events = engine.replay_events(0, 100).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 1);
        assert!(matches!(events[0].event_type, TaskEventType::Created));
    }

    #[test]
    fn protocol_compatibility_is_bounded_and_1_2_receipts_replay() {
        let engine = engine();
        let mut legacy = create_command(
            &Uuid::new_v4().to_string(),
            "task-create-protocol-1-2",
            "legacy.render",
            None,
        );
        if let TaskCommandEnvelope::Create {
            protocol_version, ..
        } = &mut legacy
        {
            *protocol_version = LEGACY_PROTOCOL_VERSION.to_string();
        }

        let committed = engine.execute_command(legacy.clone()).unwrap();
        let replayed = engine.execute_command(legacy).unwrap();

        assert!(!committed.response.replayed);
        assert!(replayed.response.replayed);
        assert_eq!(replayed.response.receipt, committed.response.receipt);
        assert_eq!(replayed.response.task, committed.response.task);
        assert!(replayed.emitted_events.is_empty());

        for (supported_version, idempotency_key, kind) in [
            (
                PROTOCOL_1_3_VERSION,
                "task-create-protocol-1-3",
                "protocol-1-3.render",
            ),
            (
                PREVIOUS_PROTOCOL_VERSION,
                "task-create-protocol-1-4",
                "protocol-1-4.render",
            ),
            (
                PROTOCOL_VERSION,
                "task-create-protocol-1-5",
                "protocol-1-5.render",
            ),
        ] {
            let mut supported =
                create_command(&Uuid::new_v4().to_string(), idempotency_key, kind, None);
            if let TaskCommandEnvelope::Create {
                protocol_version, ..
            } = &mut supported
            {
                *protocol_version = supported_version.to_string();
            }
            engine.execute_command(supported).unwrap();
        }

        for unsupported_version in ["1.1", "1.6"] {
            let mut unsupported = create_command(
                &Uuid::new_v4().to_string(),
                &format!("task-create-protocol-{unsupported_version}"),
                "unsupported.render",
                None,
            );
            if let TaskCommandEnvelope::Create {
                protocol_version, ..
            } = &mut unsupported
            {
                *protocol_version = unsupported_version.to_string();
            }
            let error = engine.execute_command(unsupported).unwrap_err();
            assert_eq!(error.code, "PROTOCOL_VERSION_UNSUPPORTED");
            assert_eq!(
                error.message,
                format!(
                    "task command protocol {unsupported_version} is unsupported; expected {LEGACY_PROTOCOL_VERSION}, {PROTOCOL_1_3_VERSION}, {PREVIOUS_PROTOCOL_VERSION}, or {PROTOCOL_VERSION}"
                )
            );
        }
        assert_eq!(engine.list().unwrap().len(), 4);
        assert_eq!(engine.replay_events(0, 100).unwrap().len(), 4);
    }
    #[test]
    fn command_gateway_rejects_key_and_command_id_reuse() {
        let engine = engine();
        let command_id = Uuid::new_v4().to_string();
        let key = "task-create-reuse-001";
        engine
            .execute_command(create_command(&command_id, key, "video.generate", None))
            .unwrap();

        let key_error = engine
            .execute_command(create_command(
                &Uuid::new_v4().to_string(),
                key,
                "audio.generate",
                None,
            ))
            .unwrap_err();
        assert_eq!(key_error.code, "IDEMPOTENCY_KEY_REUSED");

        let command_error = engine
            .execute_command(create_command(
                &command_id,
                "task-create-reuse-002",
                "document.generate",
                None,
            ))
            .unwrap_err();
        assert_eq!(command_error.code, "COMMAND_ID_REUSED");
        assert_eq!(engine.list().unwrap().len(), 1);
        assert_eq!(engine.replay_events(0, 100).unwrap().len(), 1);
    }

    #[test]
    fn committed_command_replays_after_deadline() {
        let engine = engine();
        let key = "task-create-expired-replay";
        let first = engine
            .execute_command(create_command(
                &Uuid::new_v4().to_string(),
                key,
                "research.run",
                Some(now_millis() + 30_000),
            ))
            .unwrap();
        let replay = engine
            .execute_command(create_command(
                &Uuid::new_v4().to_string(),
                key,
                "research.run",
                Some(now_millis() - 1),
            ))
            .unwrap();

        assert!(replay.response.replayed);
        assert_eq!(replay.response.task.id, first.response.task.id);
        assert!(replay.emitted_events.is_empty());

        let error = engine
            .execute_command(create_command(
                &Uuid::new_v4().to_string(),
                "task-create-expired-new",
                "research.new",
                Some(now_millis() - 1),
            ))
            .unwrap_err();
        assert_eq!(error.code, "COMMAND_DEADLINE_EXCEEDED");
        assert_eq!(engine.list().unwrap().len(), 1);
    }

    #[test]
    fn command_cas_failure_rolls_back_event_and_receipt() {
        let engine = engine();
        let created = engine
            .execute_command(create_command(
                &Uuid::new_v4().to_string(),
                "task-create-cas-001",
                "media.transcode",
                None,
            ))
            .unwrap()
            .response
            .task;
        let cancel_id = Uuid::new_v4().to_string();
        let failed_cancel = TaskCommandEnvelope::Cancel {
            command_id: cancel_id.clone(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            context: context("trace-cancel-task"),
            payload: CancelTaskPayload {
                task_id: created.id.clone(),
                reason: Some("operator canceled".to_string()),
            },
            idempotency_key: "task-cancel-cas-001".to_string(),
            expected_revision: Some(created.revision + 1),
            deadline_at: None,
        };
        let error = engine.execute_command(failed_cancel).unwrap_err();
        assert_eq!(error.code, "TASK_REVISION_CONFLICT");
        assert_eq!(engine.replay_events(0, 100).unwrap().len(), 1);

        let canceled = engine
            .execute_command(TaskCommandEnvelope::Cancel {
                command_id: cancel_id,
                protocol_version: PROTOCOL_VERSION.to_string(),
                context: context("trace-cancel-task"),
                payload: CancelTaskPayload {
                    task_id: created.id,
                    reason: Some("operator canceled".to_string()),
                },
                idempotency_key: "task-cancel-cas-001".to_string(),
                expected_revision: Some(created.revision),
                deadline_at: None,
            })
            .unwrap();
        assert!(matches!(
            canceled.response.task.status,
            TaskStatus::Canceled
        ));
        assert_eq!(engine.replay_events(0, 100).unwrap().len(), 2);
    }

    #[test]
    fn retry_command_requires_explicit_approval() {
        let engine = engine();
        let mut command = create_command(
            &Uuid::new_v4().to_string(),
            "task-create-manual-001",
            "provider.paid-generation",
            None,
        );
        if let TaskCommandEnvelope::Create { payload, .. } = &mut command {
            payload.replay_policy = TaskReplayPolicy::Manual;
        }
        engine.execute_command(command).unwrap();
        let running = engine.claim_next_runnable().unwrap().unwrap();
        let waiting = engine
            .finish_failure(&running.id, running.attempt, "provider timeout")
            .unwrap();
        assert!(matches!(waiting.status, TaskStatus::AwaitingApproval));

        let rejected = engine
            .execute_command(TaskCommandEnvelope::Retry {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: PROTOCOL_VERSION.to_string(),
                context: context("trace-retry-task"),
                payload: RetryTaskPayload {
                    task_id: waiting.id.clone(),
                    approved: false,
                },
                idempotency_key: "task-retry-rejected-001".to_string(),
                expected_revision: Some(waiting.revision),
                deadline_at: None,
            })
            .unwrap_err();
        assert_eq!(rejected.code, "TASK_RETRY_NOT_APPROVED");

        let retried = engine
            .execute_command(TaskCommandEnvelope::Retry {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: PROTOCOL_VERSION.to_string(),
                context: context("trace-retry-task"),
                payload: RetryTaskPayload {
                    task_id: waiting.id,
                    approved: true,
                },
                idempotency_key: "task-retry-approved-001".to_string(),
                expected_revision: Some(waiting.revision),
                deadline_at: None,
            })
            .unwrap();
        assert!(matches!(retried.response.task.status, TaskStatus::Queued));
        assert_eq!(engine.replay_events(0, 100).unwrap().len(), 4);
    }

    #[test]
    fn success_lifecycle_events_track_revision_payload_and_sequence() {
        let engine = engine();
        let created = engine
            .execute_command(create_command(
                &Uuid::new_v4().to_string(),
                "task-lifecycle-success-001",
                "media.render",
                None,
            ))
            .unwrap()
            .response
            .task;
        assert_eq!(created.revision, 1);

        let claimed = engine
            .claim_next_runnable_with_events("trace-lifecycle-claim")
            .unwrap();
        let running = claimed.task.unwrap();
        assert_eq!(claimed.emitted_events.len(), 1);
        assert_eq!(claimed.emitted_events[0].sequence, 2);
        assert_eq!(claimed.emitted_events[0].revision, 2);
        assert_eq!(claimed.emitted_events[0].task, running);
        assert!(matches!(
            claimed.emitted_events[0].event_type,
            TaskEventType::Progressed
        ));
        assert!(matches!(&running.status, TaskStatus::Running));
        assert_eq!(running.progress, 0);

        let no_op = engine
            .update_progress_with_events(&running.id, running.attempt, 0, "trace-progress-noop")
            .unwrap();
        assert!(no_op.emitted_events.is_empty());
        assert_eq!(no_op.task.revision, running.revision);
        assert_eq!(engine.replay_events(0, 100).unwrap().len(), 2);

        let progressed = engine
            .update_progress_with_events(&running.id, running.attempt, 35, "trace-progress-35")
            .unwrap();
        assert_eq!(progressed.task.revision, 3);
        assert_eq!(progressed.task.progress, 35);
        assert_eq!(progressed.emitted_events[0].sequence, 3);
        assert_eq!(progressed.emitted_events[0].revision, 3);
        assert_eq!(progressed.emitted_events[0].task, progressed.task);
        assert!(matches!(
            progressed.emitted_events[0].event_type,
            TaskEventType::Progressed
        ));

        let succeeded = engine
            .finish_success_with_events(
                &running.id,
                running.attempt,
                serde_json::json!({ "assetId": "asset-1" }),
                "trace-success",
            )
            .unwrap();
        assert_eq!(succeeded.task.revision, 4);
        assert_eq!(succeeded.task.progress, 100);
        assert!(matches!(&succeeded.task.status, TaskStatus::Succeeded));
        assert_eq!(succeeded.emitted_events[0].sequence, 4);
        assert_eq!(succeeded.emitted_events[0].revision, 4);
        assert_eq!(succeeded.emitted_events[0].task, succeeded.task);
        assert!(matches!(
            succeeded.emitted_events[0].event_type,
            TaskEventType::Succeeded
        ));

        let replay = engine.replay_events(1, 100).unwrap();
        assert_eq!(
            replay
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
    }

    #[test]
    fn failure_lifecycle_event_matches_replay_policy_result() {
        let cases = [
            (
                TaskReplayPolicy::Safe,
                TaskStatus::Queued,
                TaskEventType::Recovered,
                2,
            ),
            (
                TaskReplayPolicy::Manual,
                TaskStatus::AwaitingApproval,
                TaskEventType::AwaitingApproval,
                2,
            ),
            (
                TaskReplayPolicy::Never,
                TaskStatus::Failed,
                TaskEventType::Failed,
                2,
            ),
        ];

        for (policy, expected_status, expected_event, max_attempts) in cases {
            let engine = engine();
            let created = engine
                .create_new(new_task(
                    "failure-policy",
                    TaskPriority::Normal,
                    policy,
                    max_attempts,
                ))
                .unwrap();
            let claimed = engine
                .claim_next_runnable_with_events("trace-failure-claim")
                .unwrap()
                .task
                .unwrap();
            let failed = engine
                .finish_failure_with_events(
                    &created.id,
                    claimed.attempt,
                    "provider failed",
                    "trace-failure-result",
                )
                .unwrap();

            assert_eq!(failed.task.revision, 3);
            assert_eq!(failed.task.status, expected_status);
            assert_eq!(failed.emitted_events.len(), 1);
            assert_eq!(failed.emitted_events[0].sequence, 3);
            assert_eq!(failed.emitted_events[0].revision, 3);
            assert_eq!(failed.emitted_events[0].event_type, expected_event);
            assert_eq!(failed.emitted_events[0].task, failed.task);
            assert_eq!(engine.replay_events(0, 100).unwrap().len(), 3);
        }
    }

    #[test]
    fn recovery_events_cover_every_interrupted_task_in_one_transaction() {
        let engine = engine();
        let safe = engine
            .create_new(new_task(
                "recover-safe",
                TaskPriority::High,
                TaskReplayPolicy::Safe,
                3,
            ))
            .unwrap();
        let manual = engine
            .create_new(new_task(
                "recover-manual",
                TaskPriority::Normal,
                TaskReplayPolicy::Manual,
                3,
            ))
            .unwrap();
        engine
            .claim_next_runnable_with_events("trace-recover-claim-safe")
            .unwrap();
        engine
            .claim_next_runnable_with_events("trace-recover-claim-manual")
            .unwrap();

        let recovered = engine
            .recover_interrupted_with_events("trace-host-recovery")
            .unwrap();
        assert_eq!(recovered.tasks.len(), 2);
        assert_eq!(recovered.emitted_events.len(), 2);
        assert_eq!(
            recovered
                .emitted_events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![5, 6]
        );
        for event in &recovered.emitted_events {
            assert!(matches!(event.event_type, TaskEventType::Recovered));
            assert_eq!(event.revision, 3);
            assert_eq!(event.task.revision, 3);
        }
        assert!(matches!(
            engine.get(&safe.id).unwrap().status,
            TaskStatus::Queued
        ));
        assert!(matches!(
            engine.get(&manual.id).unwrap().status,
            TaskStatus::AwaitingApproval
        ));
        assert_eq!(engine.replay_events(0, 100).unwrap().len(), 6);

        let no_op = engine
            .recover_interrupted_with_events("trace-host-recovery-noop")
            .unwrap();
        assert!(no_op.tasks.is_empty());
        assert!(no_op.emitted_events.is_empty());
        assert_eq!(engine.replay_events(0, 100).unwrap().len(), 6);
    }

    #[test]
    fn event_insert_failure_rolls_back_lifecycle_state() {
        let engine = engine();
        let created = engine
            .execute_command(create_command(
                &Uuid::new_v4().to_string(),
                "task-lifecycle-rollback-001",
                "rollback.test",
                None,
            ))
            .unwrap()
            .response
            .task;
        let running = engine
            .claim_next_runnable_with_events("trace-rollback-claim")
            .unwrap()
            .task
            .unwrap();
        assert_eq!(running.revision, 2);
        assert_eq!(engine.replay_events(0, 100).unwrap().len(), 2);

        {
            let connection = engine.lock().unwrap();
            connection
                .execute_batch(
                    "CREATE TRIGGER reject_lifecycle_event
                     BEFORE INSERT ON task_events
                     BEGIN SELECT RAISE(ABORT, 'forced task event failure'); END;",
                )
                .unwrap();
        }
        let error = engine
            .update_progress_with_events(
                &created.id,
                running.attempt,
                50,
                "trace-rollback-progress",
            )
            .unwrap_err();
        assert_eq!(error.code, "HOST_INTERNAL");

        let unchanged = engine.get(&created.id).unwrap();
        assert_eq!(unchanged.revision, 2);
        assert_eq!(unchanged.progress, 0);
        assert!(matches!(unchanged.status, TaskStatus::Running));
        assert_eq!(engine.replay_events(0, 100).unwrap().len(), 2);

        {
            let connection = engine.lock().unwrap();
            connection
                .execute_batch("DROP TRIGGER reject_lifecycle_event;")
                .unwrap();
        }
        let committed = engine
            .update_progress_with_events(
                &created.id,
                running.attempt,
                50,
                "trace-committed-progress",
            )
            .unwrap();
        assert_eq!(committed.task.revision, 3);
        assert_eq!(committed.emitted_events[0].sequence, 3);
    }

    #[test]
    fn dag_waits_for_all_dependencies_and_blocks_failed_dependencies() {
        let engine = engine();
        let parent_a = engine
            .create_new(new_task(
                "parent-a",
                TaskPriority::High,
                TaskReplayPolicy::Safe,
                1,
            ))
            .unwrap();
        let parent_b = engine
            .create_new(new_task(
                "parent-b",
                TaskPriority::Normal,
                TaskReplayPolicy::Never,
                1,
            ))
            .unwrap();
        let mut child = new_task("child", TaskPriority::Critical, TaskReplayPolicy::Safe, 1);
        child.dependencies = vec![dependency(&parent_a.id), dependency(&parent_b.id)];
        let child = engine.create_new(child).unwrap();

        let claimed_a = engine.claim_next_runnable().unwrap().unwrap();
        assert_eq!(claimed_a.id, parent_a.id);
        engine
            .finish_success(&claimed_a.id, claimed_a.attempt, serde_json::json!({}))
            .unwrap();
        let claimed_b = engine.claim_next_runnable().unwrap().unwrap();
        assert_eq!(claimed_b.id, parent_b.id);
        engine
            .finish_failure(&claimed_b.id, claimed_b.attempt, "permanent failure")
            .unwrap();

        assert!(engine.claim_next_runnable().unwrap().is_none());
        assert!(matches!(
            engine.get(&child.id).unwrap().status,
            TaskStatus::Queued
        ));
    }

    #[test]
    fn claim_uses_priority_then_fifo() {
        let engine = engine();
        let low = engine
            .create_new(new_task(
                "low",
                TaskPriority::Low,
                TaskReplayPolicy::Safe,
                1,
            ))
            .unwrap();
        let critical = engine
            .create_new(new_task(
                "critical",
                TaskPriority::Critical,
                TaskReplayPolicy::Safe,
                1,
            ))
            .unwrap();
        assert_eq!(
            engine.claim_next_runnable().unwrap().unwrap().id,
            critical.id
        );
        assert_eq!(engine.claim_next_runnable().unwrap().unwrap().id, low.id);
    }

    #[test]
    fn filtered_claim_leaves_unregistered_kinds_queued() {
        let engine = engine();
        let unavailable = engine
            .create_new(new_task(
                "provider.generate",
                TaskPriority::Critical,
                TaskReplayPolicy::Safe,
                1,
            ))
            .unwrap();
        let media = engine
            .create_new(new_task(
                "media.probe",
                TaskPriority::Normal,
                TaskReplayPolicy::Safe,
                1,
            ))
            .unwrap();

        let outcome = engine
            .claim_next_runnable_for_kinds_with_events(
                &["media.probe".to_string()],
                "trace-filtered-claim",
            )
            .unwrap();
        assert_eq!(outcome.task.unwrap().id, media.id);
        assert_eq!(
            engine.get(&unavailable.id).unwrap().status,
            TaskStatus::Queued
        );
    }

    #[test]
    fn cancel_and_retry_enforce_revision_cas() {
        let engine = engine();
        let queued = engine
            .create_new(new_task(
                "cancel",
                TaskPriority::Normal,
                TaskReplayPolicy::Safe,
                2,
            ))
            .unwrap();
        let error = engine
            .cancel_task(&queued.id, queued.revision + 1)
            .unwrap_err();
        assert_eq!(error.code, "TASK_REVISION_CONFLICT");
        let canceled = engine.cancel_task(&queued.id, queued.revision).unwrap();
        assert!(matches!(canceled.status, TaskStatus::Canceled));

        let manual = engine
            .create_new(new_task(
                "manual",
                TaskPriority::High,
                TaskReplayPolicy::Manual,
                2,
            ))
            .unwrap();
        let running = engine.claim_next_runnable().unwrap().unwrap();
        assert_eq!(running.id, manual.id);
        let waiting = engine
            .finish_failure(&running.id, running.attempt, "needs approval")
            .unwrap();
        assert!(matches!(waiting.status, TaskStatus::AwaitingApproval));
        let error = engine
            .retry_task(&waiting.id, waiting.revision - 1)
            .unwrap_err();
        assert_eq!(error.code, "TASK_REVISION_CONFLICT");
        let retried = engine.retry_task(&waiting.id, waiting.revision).unwrap();
        assert!(matches!(retried.status, TaskStatus::Queued));
    }

    #[test]
    fn failure_policy_controls_automatic_retry() {
        let engine = engine();
        let safe = engine
            .create_new(new_task(
                "safe",
                TaskPriority::Critical,
                TaskReplayPolicy::Safe,
                2,
            ))
            .unwrap();
        let first = engine.claim_next_runnable().unwrap().unwrap();
        assert_eq!(first.id, safe.id);
        let requeued = engine
            .finish_failure(&safe.id, first.attempt, "transient")
            .unwrap();
        assert!(matches!(requeued.status, TaskStatus::Queued));
        let second = engine.claim_next_runnable().unwrap().unwrap();
        let failed = engine
            .finish_failure(&safe.id, second.attempt, "still broken")
            .unwrap();
        assert!(matches!(failed.status, TaskStatus::Failed));
        assert_eq!(failed.attempt, 2);
    }

    #[test]
    fn non_retryable_handler_failure_is_terminal() {
        let engine = engine();
        let task = engine
            .create_new(new_task(
                "media.probe",
                TaskPriority::Normal,
                TaskReplayPolicy::Safe,
                5,
            ))
            .unwrap();
        let running = engine.claim_next_runnable().unwrap().unwrap();
        let outcome = engine
            .finish_handler_failure_with_events(
                &task.id,
                running.attempt,
                "invalid structured input",
                false,
                "trace-handler-terminal",
            )
            .unwrap();
        assert_eq!(outcome.task.status, TaskStatus::Failed);
        assert_eq!(outcome.task.attempt, 1);
        assert_eq!(outcome.emitted_events[0].event_type, TaskEventType::Failed);
    }

    #[test]
    fn recovery_only_auto_queues_safe_tasks() {
        let engine = engine();
        let safe = engine
            .create_new(new_task(
                "safe",
                TaskPriority::Critical,
                TaskReplayPolicy::Safe,
                3,
            ))
            .unwrap();
        let manual = engine
            .create_new(new_task(
                "manual",
                TaskPriority::High,
                TaskReplayPolicy::Manual,
                3,
            ))
            .unwrap();
        let never = engine
            .create_new(new_task(
                "never",
                TaskPriority::Normal,
                TaskReplayPolicy::Never,
                3,
            ))
            .unwrap();
        assert_eq!(engine.claim_next_runnable().unwrap().unwrap().id, safe.id);
        assert_eq!(engine.claim_next_runnable().unwrap().unwrap().id, manual.id);
        assert_eq!(engine.claim_next_runnable().unwrap().unwrap().id, never.id);

        let recovered = engine.recover_interrupted().unwrap();
        assert_eq!(recovered.len(), 3);
        assert!(matches!(
            engine.get(&safe.id).unwrap().status,
            TaskStatus::Queued
        ));
        assert!(matches!(
            engine.get(&manual.id).unwrap().status,
            TaskStatus::AwaitingApproval
        ));
        assert!(matches!(
            engine.get(&never.id).unwrap().status,
            TaskStatus::AwaitingApproval
        ));
    }

    #[test]
    fn stale_worker_cannot_finish_a_later_attempt() {
        let engine = engine();
        let task = engine
            .create_new(new_task(
                "fenced",
                TaskPriority::Normal,
                TaskReplayPolicy::Safe,
                3,
            ))
            .unwrap();
        let first = engine.claim_next_runnable().unwrap().unwrap();
        engine
            .finish_failure(&task.id, first.attempt, "retry")
            .unwrap();
        let second = engine.claim_next_runnable().unwrap().unwrap();
        let error = engine
            .finish_success(
                &task.id,
                first.attempt,
                serde_json::json!({ "stale": true }),
            )
            .unwrap_err();
        assert_eq!(error.code, "TASK_ATTEMPT_STALE");
        engine
            .finish_success(&task.id, second.attempt, serde_json::json!({ "ok": true }))
            .unwrap();
    }
}
