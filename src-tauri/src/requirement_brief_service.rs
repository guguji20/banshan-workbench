use crate::protocol::{
    is_protocol_1_3_surface_supported, BriefRecord, ChangeRequirementBriefStatusPayload,
    CommandReceipt, CreateRequirementBriefPayload, HostError, OperationContext,
    RequirementAnswerDisposition, RequirementAnswerInput, RequirementBriefCommandEnvelope,
    RequirementBriefCommandResponse, RequirementBriefContent, RequirementBriefDomainEvent,
    RequirementBriefEventType, RequirementBriefRecord, RequirementBriefStatus,
    RequirementQuestionAnswer, UpdateRequirementBriefPayload, PREVIOUS_PROTOCOL_VERSION,
    PROTOCOL_1_3_VERSION, PROTOCOL_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const QUESTION_SET_VERSION: &str = "requirement-brief.v1";
const MAX_TEXT_CHARS: usize = 8_000;
const MAX_ANSWER_CHARS: usize = 16_000;
const MAX_LIST_ITEMS: usize = 100;
const MAX_LIST_ITEM_CHARS: usize = 500;
const MAX_CONTEXT_CHARS: usize = 160;
const MAX_QUESTION_ID_CHARS: usize = 80;

#[derive(Clone, Copy)]
struct QuestionDefinition {
    id: &'static str,
    prompt: &'static str,
    required: bool,
}

const QUESTION_SET: &[QuestionDefinition] = &[
    QuestionDefinition {
        id: "objective",
        prompt: "这次内容最重要的业务目标是什么？",
        required: true,
    },
    QuestionDefinition {
        id: "audience",
        prompt: "核心受众是谁，他们处于什么场景和认知阶段？",
        required: true,
    },
    QuestionDefinition {
        id: "key-message",
        prompt: "观众最终必须记住的核心信息是什么？",
        required: true,
    },
    QuestionDefinition {
        id: "deliverables",
        prompt: "需要交付哪些成片、版本和尺寸？",
        required: true,
    },
    QuestionDefinition {
        id: "channels",
        prompt: "内容会发布或使用在哪些渠道？",
        required: true,
    },
    QuestionDefinition {
        id: "style-keywords",
        prompt: "客户说的高级或调性，具体指材质、构图、节奏、色彩还是文案？",
        required: false,
    },
    QuestionDefinition {
        id: "mandatory-items",
        prompt: "哪些人物、产品、品牌标识或表述必须出现？",
        required: false,
    },
    QuestionDefinition {
        id: "constraints",
        prompt: "有哪些合规、拍摄、技术或品牌限制？",
        required: false,
    },
    QuestionDefinition {
        id: "acceptance-criteria",
        prompt: "谁负责最终验收，什么结果才算通过？",
        required: true,
    },
    QuestionDefinition {
        id: "risks",
        prompt: "哪些风险、依赖或未决事项会影响交付？",
        required: false,
    },
    QuestionDefinition {
        id: "deadline",
        prompt: "交付日期和关键里程碑是什么？",
        required: false,
    },
    QuestionDefinition {
        id: "budget",
        prompt: "需要记录哪些预算假设或上限？",
        required: false,
    },
    QuestionDefinition {
        id: "reference-cases",
        prompt: "哪些已通过案例可作为参考？",
        required: false,
    },
    QuestionDefinition {
        id: "reference-notes",
        prompt: "参考中要借鉴什么，又必须避开什么？",
        required: false,
    },
];

#[derive(Debug)]
pub struct RequirementBriefCommandOutcome {
    pub response: RequirementBriefCommandResponse,
    pub emitted_events: Vec<RequirementBriefDomainEvent>,
}

pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS requirement_briefs (
                id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT NOT NULL UNIQUE,
                question_set_version TEXT NOT NULL,
                answers_json TEXT NOT NULL,
                content_json TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('interviewing','review','confirmed')),
                confirmed_at INTEGER,
                confirmed_by TEXT,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                CHECK(
                    (status = 'confirmed' AND confirmed_at IS NOT NULL AND confirmed_by IS NOT NULL)
                    OR
                    (status != 'confirmed' AND confirmed_at IS NULL AND confirmed_by IS NULL)
                ),
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_requirement_briefs_updated
                ON requirement_briefs(updated_at DESC, id DESC);
            CREATE TABLE IF NOT EXISTS requirement_brief_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL CHECK(event_type IN
                    ('requirementBrief.created','requirementBrief.updated',
                     'requirementBrief.statusChanged')),
                aggregate_id TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                occurred_at INTEGER NOT NULL,
                trace_id TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                FOREIGN KEY(aggregate_id) REFERENCES requirement_briefs(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_requirement_brief_events_aggregate
                ON requirement_brief_events(aggregate_id, sequence);
            CREATE TABLE IF NOT EXISTS requirement_brief_command_receipts (
                idempotency_key TEXT PRIMARY KEY NOT NULL,
                command_id TEXT NOT NULL UNIQUE,
                command_type TEXT NOT NULL CHECK(command_type IN
                    ('requirementBrief.create','requirementBrief.update',
                     'requirementBrief.changeStatus')),
                protocol_version TEXT NOT NULL,
                deadline_at INTEGER,
                request_fingerprint TEXT NOT NULL CHECK(length(request_fingerprint) = 64),
                response_json TEXT NOT NULL,
                completed_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_requirement_brief_receipts_completed
                ON requirement_brief_command_receipts(completed_at);
            "#,
        )
        .map_err(sql_error)
}

pub fn execute_command(
    connection: &mut Connection,
    command: RequirementBriefCommandEnvelope,
) -> Result<RequirementBriefCommandOutcome, HostError> {
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
        return Ok(RequirementBriefCommandOutcome {
            response,
            emitted_events: Vec::new(),
        });
    }

    validate_deadline(meta.deadline_at)?;
    let (record, event_type) = match &command {
        NormalizedCommand::Create { payload, .. } => (
            create_requirement_brief(&transaction, payload)?,
            RequirementBriefEventType::Created,
        ),
        NormalizedCommand::Update { payload, meta } => (
            update_requirement_brief(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized update revision"),
                &meta.context.project_id,
            )?,
            RequirementBriefEventType::Updated,
        ),
        NormalizedCommand::ChangeStatus { payload, meta } => (
            change_status(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized status revision"),
                &meta.context.project_id,
                &meta.context.actor_id,
            )?,
            RequirementBriefEventType::StatusChanged,
        ),
    };
    let event = append_event(&transaction, event_type, &record, &meta.context.trace_id)?;
    let completed_at = now_millis();
    let response = RequirementBriefCommandResponse {
        receipt: CommandReceipt {
            command_id: meta.command_id.clone(),
            idempotency_key: meta.idempotency_key.clone(),
            command_type: command_type.to_string(),
            aggregate_id: record.id.clone(),
            revision: record.revision,
            last_event_sequence: event.sequence,
            completed_at,
        },
        requirement_brief: record,
        replayed: false,
    };
    transaction
        .execute(
            "INSERT INTO requirement_brief_command_receipts
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
        Ok(()) => Ok(RequirementBriefCommandOutcome {
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
                Ok(RequirementBriefCommandOutcome {
                    response: persisted,
                    emitted_events: vec![persisted_event],
                })
            }
            Ok(None) => Err(sql_error(error)),
            Err(lookup_error) => Err(lookup_error),
        },
    }
}

pub fn list(connection: &Connection) -> Result<Vec<RequirementBriefRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, project_id, question_set_version, answers_json, content_json,
                    status, confirmed_at, confirmed_by, revision, created_at, updated_at
             FROM requirement_briefs ORDER BY updated_at DESC, id DESC",
        )
        .map_err(sql_error)?;
    let records = statement
        .query_map([], requirement_brief_from_row)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(records)
}

pub fn replay_events(
    connection: &Connection,
    after_sequence: i64,
    limit: u32,
) -> Result<Vec<RequirementBriefDomainEvent>, HostError> {
    if after_sequence < 0 {
        return Err(HostError::validation("afterSequence cannot be negative"));
    }
    let mut statement = connection
        .prepare(
            "SELECT sequence, event_id, event_type, aggregate_id, revision,
                    occurred_at, trace_id, payload_json
             FROM requirement_brief_events
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
        payload: CreateRequirementBriefPayload,
    },
    Update {
        meta: CommandMeta,
        payload: Box<UpdateRequirementBriefPayload>,
    },
    ChangeStatus {
        meta: CommandMeta,
        payload: ChangeRequirementBriefStatusPayload,
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
            Self::Create { .. } => "requirementBrief.create",
            Self::Update { .. } => "requirementBrief.update",
            Self::ChangeStatus { .. } => "requirementBrief.changeStatus",
        }
    }
}

fn normalize_command(
    command: RequirementBriefCommandEnvelope,
) -> Result<NormalizedCommand, HostError> {
    match command {
        RequirementBriefCommandEnvelope::Create {
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
                    "requirementBrief.create rejects expectedRevision",
                ));
            }
            let payload = CreateRequirementBriefPayload {
                project_id: normalize_uuid("projectId", payload.project_id)?,
            };
            let context = normalize_context(context)?;
            if context.project_id != payload.project_id {
                return Err(HostError::validation(
                    "context projectId must match requirement brief projectId",
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
        RequirementBriefCommandEnvelope::Update {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let UpdateRequirementBriefPayload {
                brief_id,
                answers,
                content,
            } = *payload;
            Ok(NormalizedCommand::Update {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    normalize_context(context)?,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload: Box::new(UpdateRequirementBriefPayload {
                    brief_id: normalize_uuid("briefId", brief_id)?,
                    answers: normalize_answer_inputs(answers)?,
                    content: normalize_content(content)?,
                }),
            })
        }
        RequirementBriefCommandEnvelope::ChangeStatus {
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
                payload: ChangeRequirementBriefStatusPayload {
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
            "requirement brief mutation requires expectedRevision > 0",
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
    if !is_protocol_1_3_surface_supported(&protocol_version) {
        return Err(HostError::new(
            "PROTOCOL_VERSION_MISMATCH",
            format!(
                "expected protocolVersion {PROTOCOL_1_3_VERSION}, {PREVIOUS_PROTOCOL_VERSION}, or {PROTOCOL_VERSION}"
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
        .ok_or_else(|| HostError::validation("requirement brief context requires projectId"))?;
    normalize_required("windowId", context.window_id, MAX_CONTEXT_CHARS)?;
    Ok(NormalizedContext {
        actor_id: normalize_required("actorId", context.actor_id, MAX_CONTEXT_CHARS)?,
        account_id: normalize_optional("accountId", context.account_id, MAX_CONTEXT_CHARS)?,
        project_id: normalize_uuid("projectId", project_id)?,
        trace_id: normalize_required("traceId", context.trace_id, MAX_CONTEXT_CHARS)?,
    })
}

fn normalize_answer_inputs(
    answers: Vec<RequirementAnswerInput>,
) -> Result<Vec<RequirementAnswerInput>, HostError> {
    if answers.len() != QUESTION_SET.len() {
        return Err(HostError::validation(format!(
            "answers must contain exactly {} fixed questions",
            QUESTION_SET.len()
        )));
    }
    let mut by_id = HashMap::with_capacity(answers.len());
    for answer in answers {
        let question_id =
            normalize_required("questionId", answer.question_id, MAX_QUESTION_ID_CHARS)?;
        if !QUESTION_SET
            .iter()
            .any(|question| question.id == question_id)
        {
            return Err(HostError::validation(format!(
                "unknown requirement questionId: {question_id}"
            )));
        }
        let answer_text = normalize_limited_text("answer", answer.answer, MAX_ANSWER_CHARS)?;
        match answer.disposition {
            RequirementAnswerDisposition::Unanswered if !answer_text.is_empty() => {
                return Err(HostError::validation(
                    "unanswered requirement question cannot contain an answer",
                ));
            }
            RequirementAnswerDisposition::Answered if answer_text.is_empty() => {
                return Err(HostError::validation(
                    "answered requirement question requires a non-empty answer",
                ));
            }
            _ => {}
        }
        let normalized = RequirementAnswerInput {
            question_id: question_id.clone(),
            answer: answer_text,
            disposition: answer.disposition,
        };
        if by_id.insert(question_id.clone(), normalized).is_some() {
            return Err(HostError::validation(format!(
                "duplicate requirement questionId: {question_id}"
            )));
        }
    }
    QUESTION_SET
        .iter()
        .map(|question| {
            by_id.remove(question.id).ok_or_else(|| {
                HostError::validation(format!(
                    "answers are missing requirement questionId: {}",
                    question.id
                ))
            })
        })
        .collect()
}

fn normalize_content(
    content: RequirementBriefContent,
) -> Result<RequirementBriefContent, HostError> {
    if content.deadline_at.is_some_and(|value| value <= 0) {
        return Err(HostError::validation(
            "deadlineAt must be a positive timestamp",
        ));
    }
    Ok(RequirementBriefContent {
        objective: normalize_text("objective", content.objective)?,
        audience: normalize_text("audience", content.audience)?,
        key_message: normalize_text("keyMessage", content.key_message)?,
        deliverables: normalize_list("deliverables", content.deliverables)?,
        channels: normalize_list("channels", content.channels)?,
        style_keywords: normalize_list("styleKeywords", content.style_keywords)?,
        mandatory_items: normalize_list("mandatoryItems", content.mandatory_items)?,
        constraints: normalize_list("constraints", content.constraints)?,
        acceptance_criteria: normalize_list("acceptanceCriteria", content.acceptance_criteria)?,
        risks: normalize_list("risks", content.risks)?,
        deadline_at: content.deadline_at,
        budget_notes: normalize_text("budgetNotes", content.budget_notes)?,
        reference_case_ids: normalize_reference_case_ids(content.reference_case_ids)?,
        reference_notes: normalize_text("referenceNotes", content.reference_notes)?,
    })
}

fn normalize_text(field: &str, value: String) -> Result<String, HostError> {
    normalize_limited_text(field, value, MAX_TEXT_CHARS)
}

fn normalize_limited_text(field: &str, value: String, max: usize) -> Result<String, HostError> {
    let value = value.trim().to_string();
    if value.chars().count() > max {
        return Err(HostError::validation(format!(
            "{field} exceeds {max} characters"
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

fn normalize_reference_case_ids(values: Vec<String>) -> Result<Vec<String>, HostError> {
    if values.len() > MAX_LIST_ITEMS {
        return Err(HostError::validation(format!(
            "referenceCaseIds cannot contain more than {MAX_LIST_ITEMS} items"
        )));
    }
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        let value = normalize_uuid("referenceCaseId", value)?;
        if seen.insert(value.clone()) {
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

fn create_requirement_brief(
    transaction: &Transaction<'_>,
    payload: &CreateRequirementBriefPayload,
) -> Result<RequirementBriefRecord, HostError> {
    if find_by_project(transaction, &payload.project_id)?.is_some() {
        return Err(HostError::new(
            "REQUIREMENT_BRIEF_EXISTS",
            "project already has a requirement brief",
            false,
        ));
    }
    let project_brief = load_project_brief(transaction, &payload.project_id)?;
    let content = normalize_content(prefill_content(project_brief))?;
    let inputs = blank_answer_inputs();
    let answers = rebuild_answers(inputs.clone()).map_err(question_set_host_error)?;
    let now = now_millis();
    let record = RequirementBriefRecord {
        id: Uuid::new_v4().to_string(),
        project_id: payload.project_id.clone(),
        question_set_version: QUESTION_SET_VERSION.to_string(),
        answers,
        content,
        status: RequirementBriefStatus::Interviewing,
        confirmed_at: None,
        confirmed_by: None,
        revision: 1,
        created_at: now,
        updated_at: now,
    };
    insert_record(transaction, &record, &inputs)?;
    Ok(record)
}

fn update_requirement_brief(
    transaction: &Transaction<'_>,
    payload: &UpdateRequirementBriefPayload,
    expected_revision: i64,
    context_project_id: &str,
) -> Result<RequirementBriefRecord, HostError> {
    let current = load_record(transaction, &payload.brief_id)?;
    ensure_owned(&current, expected_revision, context_project_id)?;
    if current.status == RequirementBriefStatus::Confirmed {
        return Err(HostError::new(
            "REQUIREMENT_BRIEF_CONFIRMED",
            "confirmed requirement brief must be reopened before update",
            false,
        ));
    }
    let answers = rebuild_answers(payload.answers.clone()).map_err(question_set_host_error)?;
    if current.status == RequirementBriefStatus::Review {
        ensure_complete(&answers, &payload.content)?;
    }
    ensure_reference_cases(
        transaction,
        context_project_id,
        &payload.content.reference_case_ids,
    )?;
    let changed = transaction
        .execute(
            "UPDATE requirement_briefs
             SET answers_json = ?1, content_json = ?2,
                 revision = revision + 1, updated_at = ?3
             WHERE id = ?4 AND revision = ?5",
            params![
                serde_json::to_string(&payload.answers).map_err(json_error)?,
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
    payload: &ChangeRequirementBriefStatusPayload,
    expected_revision: i64,
    context_project_id: &str,
    actor_id: &str,
) -> Result<RequirementBriefRecord, HostError> {
    let current = load_record(transaction, &payload.brief_id)?;
    ensure_owned(&current, expected_revision, context_project_id)?;
    if current.status == payload.status {
        return Err(HostError::validation(
            "requirement brief already has requested status",
        ));
    }

    let (confirmed_at, confirmed_by) = match (&current.status, &payload.status) {
        (RequirementBriefStatus::Interviewing, RequirementBriefStatus::Review) => {
            ensure_complete(&current.answers, &current.content)?;
            ensure_reference_cases(
                transaction,
                context_project_id,
                &current.content.reference_case_ids,
            )?;
            (None, None)
        }
        (RequirementBriefStatus::Review, RequirementBriefStatus::Confirmed) => {
            ensure_complete(&current.answers, &current.content)?;
            ensure_no_follow_up(&current.answers)?;
            ensure_reference_cases(
                transaction,
                context_project_id,
                &current.content.reference_case_ids,
            )?;
            (Some(now_millis()), Some(actor_id.to_string()))
        }
        (RequirementBriefStatus::Review, RequirementBriefStatus::Interviewing)
        | (RequirementBriefStatus::Confirmed, RequirementBriefStatus::Review) => (None, None),
        _ => {
            return Err(HostError::new(
                "REQUIREMENT_BRIEF_STATUS_TRANSITION_INVALID",
                "requested requirement brief status transition is not allowed",
                false,
            ));
        }
    };

    let changed = transaction
        .execute(
            "UPDATE requirement_briefs
             SET status = ?1, confirmed_at = ?2, confirmed_by = ?3,
                 revision = revision + 1, updated_at = ?4
             WHERE id = ?5 AND revision = ?6",
            params![
                status_to_db(&payload.status),
                confirmed_at,
                confirmed_by,
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
    record: &RequirementBriefRecord,
    expected_revision: i64,
    context_project_id: &str,
) -> Result<(), HostError> {
    if record.project_id != context_project_id {
        return Err(HostError::new(
            "REQUIREMENT_BRIEF_PROJECT_MISMATCH",
            "requirement brief belongs to a different project",
            false,
        ));
    }
    if record.revision != expected_revision {
        return Err(HostError::conflict(format!(
            "requirement brief revision is {}, request expected {}",
            record.revision, expected_revision
        )));
    }
    Ok(())
}

fn ensure_complete(
    answers: &[RequirementQuestionAnswer],
    content: &RequirementBriefContent,
) -> Result<(), HostError> {
    let mut missing = Vec::new();
    if content.objective.is_empty() {
        missing.push("objective".to_string());
    }
    if content.audience.is_empty() {
        missing.push("audience".to_string());
    }
    if content.key_message.is_empty() {
        missing.push("keyMessage".to_string());
    }
    if content.deliverables.is_empty() {
        missing.push("deliverables".to_string());
    }
    if content.channels.is_empty() {
        missing.push("channels".to_string());
    }
    if content.acceptance_criteria.is_empty() {
        missing.push("acceptanceCriteria".to_string());
    }
    for answer in answers {
        if answer.required && answer.disposition == RequirementAnswerDisposition::Unanswered {
            missing.push(format!("answer:{}", answer.question_id));
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(HostError::new(
            "REQUIREMENT_BRIEF_INCOMPLETE",
            format!("requirement brief is missing: {}", missing.join(", ")),
            false,
        ))
    }
}

fn ensure_no_follow_up(answers: &[RequirementQuestionAnswer]) -> Result<(), HostError> {
    let pending = answers
        .iter()
        .filter(|answer| answer.disposition == RequirementAnswerDisposition::FollowUp)
        .map(|answer| answer.question_id.as_str())
        .collect::<Vec<_>>();
    if pending.is_empty() {
        Ok(())
    } else {
        Err(HostError::new(
            "REQUIREMENT_BRIEF_FOLLOW_UP_PENDING",
            format!(
                "requirement brief has follow-up questions: {}",
                pending.join(", ")
            ),
            false,
        ))
    }
}

fn ensure_reference_cases(
    connection: &Connection,
    project_id: &str,
    reference_case_ids: &[String],
) -> Result<(), HostError> {
    if reference_case_ids.is_empty() {
        return Ok(());
    }
    let mut statement = connection
        .prepare("SELECT project_id FROM cases WHERE id = ?1")
        .map_err(sql_error)?;
    for case_id in reference_case_ids {
        let case_project_id = statement
            .query_row([case_id], |row| row.get::<_, Option<String>>(0))
            .optional()
            .map_err(sql_error)?
            .ok_or_else(|| {
                HostError::new(
                    "REFERENCE_CASE_NOT_FOUND",
                    format!("reference case does not exist: {case_id}"),
                    false,
                )
            })?;
        if case_project_id
            .as_deref()
            .is_some_and(|owner| owner != project_id)
        {
            return Err(HostError::new(
                "REFERENCE_CASE_PROJECT_MISMATCH",
                format!("reference case belongs to a different project: {case_id}"),
                false,
            ));
        }
    }
    Ok(())
}

fn load_project_brief(connection: &Connection, project_id: &str) -> Result<BriefRecord, HostError> {
    let brief_json = connection
        .query_row(
            "SELECT brief_json FROM projects WHERE id = ?1",
            [project_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| {
            HostError::new(
                "PROJECT_NOT_FOUND",
                "requirement brief project does not exist",
                false,
            )
        })?;
    serde_json::from_str(&brief_json).map_err(|error| {
        HostError::internal(format!(
            "requirement brief project data is invalid JSON: {error}"
        ))
    })
}

fn prefill_content(brief: BriefRecord) -> RequirementBriefContent {
    RequirementBriefContent {
        objective: brief.objective,
        audience: brief.audience,
        deliverables: brief.deliverables,
        style_keywords: brief.style_keywords,
        mandatory_items: brief.mandatory_items,
        constraints: brief.constraints,
        risks: brief.risks,
        reference_notes: brief.reference_notes,
        ..RequirementBriefContent::default()
    }
}

fn blank_answer_inputs() -> Vec<RequirementAnswerInput> {
    QUESTION_SET
        .iter()
        .map(|question| RequirementAnswerInput {
            question_id: question.id.to_string(),
            answer: String::new(),
            disposition: RequirementAnswerDisposition::Unanswered,
        })
        .collect()
}

fn rebuild_answers(
    inputs: Vec<RequirementAnswerInput>,
) -> Result<Vec<RequirementQuestionAnswer>, String> {
    if inputs.len() != QUESTION_SET.len() {
        return Err(format!(
            "expected {} answer records, found {}",
            QUESTION_SET.len(),
            inputs.len()
        ));
    }
    let mut by_id = HashMap::with_capacity(inputs.len());
    for input in inputs {
        let question_id = input.question_id.clone();
        match input.disposition {
            RequirementAnswerDisposition::Unanswered if !input.answer.is_empty() => {
                return Err(format!(
                    "unanswered question contains an answer: {question_id}"
                ));
            }
            RequirementAnswerDisposition::Answered if input.answer.is_empty() => {
                return Err(format!("answered question is empty: {question_id}"));
            }
            _ => {}
        }
        if by_id.insert(question_id.clone(), input).is_some() {
            return Err(format!("duplicate questionId: {question_id}"));
        }
    }
    QUESTION_SET
        .iter()
        .map(|question| {
            let input = by_id
                .remove(question.id)
                .ok_or_else(|| format!("missing questionId: {}", question.id))?;
            Ok(RequirementQuestionAnswer {
                question_id: question.id.to_string(),
                prompt: question.prompt.to_string(),
                required: question.required,
                answer: input.answer,
                disposition: input.disposition,
            })
        })
        .collect()
}

fn question_set_host_error(message: String) -> HostError {
    HostError::validation(format!("invalid fixed requirement question set: {message}"))
}

fn insert_record(
    transaction: &Transaction<'_>,
    record: &RequirementBriefRecord,
    inputs: &[RequirementAnswerInput],
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO requirement_briefs
             (id, project_id, question_set_version, answers_json, content_json, status,
              confirmed_at, confirmed_by, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                record.id,
                record.project_id,
                record.question_set_version,
                serde_json::to_string(inputs).map_err(json_error)?,
                serde_json::to_string(&record.content).map_err(json_error)?,
                status_to_db(&record.status),
                record.confirmed_at,
                record.confirmed_by,
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn load_record(
    connection: &Connection,
    brief_id: &str,
) -> Result<RequirementBriefRecord, HostError> {
    find_record(connection, brief_id)?.ok_or_else(|| {
        HostError::new(
            "REQUIREMENT_BRIEF_NOT_FOUND",
            "requirement brief does not exist",
            false,
        )
    })
}

fn find_record(
    connection: &Connection,
    brief_id: &str,
) -> Result<Option<RequirementBriefRecord>, HostError> {
    connection
        .query_row(
            "SELECT id, project_id, question_set_version, answers_json, content_json,
                    status, confirmed_at, confirmed_by, revision, created_at, updated_at
             FROM requirement_briefs WHERE id = ?1",
            [brief_id],
            requirement_brief_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn find_by_project(
    connection: &Connection,
    project_id: &str,
) -> Result<Option<RequirementBriefRecord>, HostError> {
    connection
        .query_row(
            "SELECT id, project_id, question_set_version, answers_json, content_json,
                    status, confirmed_at, confirmed_by, revision, created_at, updated_at
             FROM requirement_briefs WHERE project_id = ?1",
            [project_id],
            requirement_brief_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn ensure_changed(changed: usize) -> Result<(), HostError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(HostError::conflict(
            "requirement brief changed during mutation",
        ))
    }
}

fn append_event(
    transaction: &Transaction<'_>,
    event_type: RequirementBriefEventType,
    record: &RequirementBriefRecord,
    trace_id: &str,
) -> Result<RequirementBriefDomainEvent, HostError> {
    let event_id = Uuid::new_v4().to_string();
    let occurred_at = now_millis();
    transaction
        .execute(
            "INSERT INTO requirement_brief_events
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
    Ok(RequirementBriefDomainEvent {
        sequence: transaction.last_insert_rowid(),
        event_id,
        event_type,
        aggregate_id: record.id.clone(),
        revision: record.revision,
        occurred_at,
        trace_id: trace_id.to_string(),
        requirement_brief: record.clone(),
    })
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<RequirementBriefDomainEvent> {
    let event_type: String = row.get(2)?;
    let payload: String = row.get(7)?;
    Ok(RequirementBriefDomainEvent {
        sequence: row.get(0)?,
        event_id: row.get(1)?,
        event_type: event_type_from_db(&event_type)?,
        aggregate_id: row.get(3)?,
        revision: row.get(4)?,
        occurred_at: row.get(5)?,
        trace_id: row.get(6)?,
        requirement_brief: serde_json::from_str(&payload).map_err(|error| {
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
) -> Result<RequirementBriefDomainEvent, HostError> {
    connection
        .query_row(
            "SELECT sequence, event_id, event_type, aggregate_id, revision,
                    occurred_at, trace_id, payload_json
             FROM requirement_brief_events WHERE sequence = ?1",
            [sequence],
            event_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::internal("committed requirement brief event is missing"))
}

fn requirement_brief_from_row(row: &Row<'_>) -> rusqlite::Result<RequirementBriefRecord> {
    let question_set_version: String = row.get(2)?;
    if question_set_version != QUESTION_SET_VERSION {
        return Err(conversion_error(
            "question_set_version",
            &question_set_version,
        ));
    }
    let answers_json: String = row.get(3)?;
    let inputs: Vec<RequirementAnswerInput> =
        serde_json::from_str(&answers_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                answers_json.len(),
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let answers =
        rebuild_answers(inputs).map_err(|message| conversion_error("answers_json", &message))?;
    let content_json: String = row.get(4)?;
    let status: String = row.get(5)?;
    Ok(RequirementBriefRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        question_set_version,
        answers,
        content: serde_json::from_str(&content_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                content_json.len(),
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        status: status_from_db(&status)?,
        confirmed_at: row.get(6)?,
        confirmed_by: row.get(7)?,
        revision: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
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
) -> Result<Option<RequirementBriefCommandResponse>, HostError> {
    let by_key = load_receipt(connection, "idempotency_key", idempotency_key)?;
    let by_command = load_receipt(connection, "command_id", command_id)?;
    if by_key
        .as_ref()
        .is_some_and(|receipt| receipt.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "IDEMPOTENCY_KEY_REUSED",
            "idempotencyKey reused for a different requirement brief request",
            false,
        ));
    }
    if by_command
        .as_ref()
        .is_some_and(|receipt| receipt.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "COMMAND_ID_REUSED",
            "commandId reused for a different requirement brief request",
            false,
        ));
    }
    if let (Some(left), Some(right)) = (&by_key, &by_command) {
        if left.command_id != right.command_id || left.idempotency_key != right.idempotency_key {
            return Err(HostError::new(
                "COMMAND_IDENTITY_COLLISION",
                "command identities resolve to different requirement brief requests",
                false,
            ));
        }
    }
    by_key
        .or(by_command)
        .map(|receipt| {
            let mut response: RequirementBriefCommandResponse =
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
                 FROM requirement_brief_command_receipts WHERE {column} = ?1"
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
            "requirement brief command deadline has elapsed",
            false,
        ))
    } else {
        Ok(())
    }
}

fn status_to_db(status: &RequirementBriefStatus) -> &'static str {
    match status {
        RequirementBriefStatus::Interviewing => "interviewing",
        RequirementBriefStatus::Review => "review",
        RequirementBriefStatus::Confirmed => "confirmed",
    }
}

fn status_from_db(value: &str) -> rusqlite::Result<RequirementBriefStatus> {
    match value {
        "interviewing" => Ok(RequirementBriefStatus::Interviewing),
        "review" => Ok(RequirementBriefStatus::Review),
        "confirmed" => Ok(RequirementBriefStatus::Confirmed),
        _ => Err(conversion_error("status", value)),
    }
}

fn event_type_to_db(event_type: &RequirementBriefEventType) -> &'static str {
    match event_type {
        RequirementBriefEventType::Created => "requirementBrief.created",
        RequirementBriefEventType::Updated => "requirementBrief.updated",
        RequirementBriefEventType::StatusChanged => "requirementBrief.statusChanged",
    }
}

fn event_type_from_db(value: &str) -> rusqlite::Result<RequirementBriefEventType> {
    match value {
        "requirementBrief.created" => Ok(RequirementBriefEventType::Created),
        "requirementBrief.updated" => Ok(RequirementBriefEventType::Updated),
        "requirementBrief.statusChanged" => Ok(RequirementBriefEventType::StatusChanged),
        _ => Err(conversion_error("event type", value)),
    }
}

fn conversion_error(field: &str, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        value.len(),
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid requirement brief {field}: {value}"),
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
    HostError::internal(format!(
        "requirement brief SQLite operation failed: {error}"
    ))
}

fn json_error(error: serde_json::Error) -> HostError {
    HostError::internal(format!("requirement brief JSON operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const SECRET_PATH: &str = r#"C:\Users\operator\private\reference-master.mov"#;

    fn project_brief() -> BriefRecord {
        BriefRecord {
            objective: " Launch a premium riverside property ".into(),
            audience: " Urban families ".into(),
            deliverables: vec![" 90 second master ".into(), "3 cutdowns".into()],
            style_keywords: vec!["Natural light".into()],
            mandatory_items: vec!["Brand sign".into()],
            constraints: vec!["No drone after 18:00".into()],
            risks: vec!["Weather".into()],
            reference_notes: "Use restrained camera movement".into(),
        }
    }

    fn create_schema(connection: &Connection) {
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE projects (
                    id TEXT PRIMARY KEY NOT NULL,
                    name TEXT NOT NULL,
                    client_name TEXT NOT NULL,
                    brief_json TEXT NOT NULL,
                    stage TEXT NOT NULL,
                    revision INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE cases (
                    id TEXT PRIMARY KEY NOT NULL,
                    project_id TEXT,
                    storage_rel_path TEXT NOT NULL,
                    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE RESTRICT
                 );",
            )
            .unwrap();
    }

    fn seed_project(connection: &Connection, brief: &BriefRecord) -> String {
        let project_id = Uuid::new_v4().to_string();
        connection
            .execute(
                "INSERT INTO projects
                 (id, name, client_name, brief_json, stage, revision, created_at, updated_at)
                 VALUES (?1, 'Riverside', 'Client X', ?2, 'intake', 1, 1, 1)",
                params![project_id, serde_json::to_string(brief).unwrap()],
            )
            .unwrap();
        project_id
    }

    fn database() -> (Connection, String, String) {
        let connection = Connection::open_in_memory().unwrap();
        create_schema(&connection);
        let project_id = seed_project(&connection, &project_brief());
        let other_project_id = seed_project(&connection, &BriefRecord::default());
        migrate(&connection).unwrap();
        (connection, project_id, other_project_id)
    }

    fn context(project_id: &str, actor_id: &str) -> OperationContext {
        OperationContext {
            actor_id: actor_id.into(),
            account_id: Some("account-local".into()),
            project_id: Some(project_id.into()),
            window_id: "main".into(),
            trace_id: Uuid::new_v4().to_string(),
        }
    }

    fn create_command(project_id: &str) -> RequirementBriefCommandEnvelope {
        RequirementBriefCommandEnvelope::Create {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: PROTOCOL_VERSION.into(),
            context: context(project_id, "operator-local"),
            payload: CreateRequirementBriefPayload {
                project_id: project_id.into(),
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: None,
            deadline_at: None,
        }
    }

    fn complete_content() -> RequirementBriefContent {
        RequirementBriefContent {
            objective: "Build premium lifestyle recognition".into(),
            audience: "Urban families".into(),
            key_message: "Life beside the river is calm and connected".into(),
            deliverables: vec!["90 second master".into()],
            channels: vec!["Campaign site".into()],
            style_keywords: vec!["Natural light".into()],
            mandatory_items: vec!["Brand sign".into()],
            constraints: vec!["No unsupported claims".into()],
            acceptance_criteria: vec!["Client brand review passes".into()],
            risks: vec!["Weather".into()],
            deadline_at: Some(now_millis() + 86_400_000),
            budget_notes: "Approved production cap".into(),
            reference_case_ids: Vec::new(),
            reference_notes: "Use restrained movement".into(),
        }
    }

    fn complete_answers() -> Vec<RequirementAnswerInput> {
        QUESTION_SET
            .iter()
            .map(|question| RequirementAnswerInput {
                question_id: question.id.into(),
                answer: if question.required {
                    format!("Resolved answer for {}", question.id)
                } else {
                    String::new()
                },
                disposition: if question.required {
                    RequirementAnswerDisposition::Answered
                } else {
                    RequirementAnswerDisposition::NotApplicable
                },
            })
            .collect()
    }

    fn update_command(
        project_id: &str,
        brief_id: &str,
        revision: i64,
        answers: Vec<RequirementAnswerInput>,
        content: RequirementBriefContent,
    ) -> RequirementBriefCommandEnvelope {
        RequirementBriefCommandEnvelope::Update {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: PROTOCOL_VERSION.into(),
            context: context(project_id, "operator-local"),
            payload: Box::new(UpdateRequirementBriefPayload {
                brief_id: brief_id.into(),
                answers,
                content,
            }),
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }

    fn status_command(
        project_id: &str,
        brief_id: &str,
        revision: i64,
        status: RequirementBriefStatus,
        actor_id: &str,
    ) -> RequirementBriefCommandEnvelope {
        RequirementBriefCommandEnvelope::ChangeStatus {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: PROTOCOL_VERSION.into(),
            context: context(project_id, actor_id),
            payload: ChangeRequirementBriefStatusPayload {
                brief_id: brief_id.into(),
                status,
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(revision),
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

    fn set_deadline(command: &mut RequirementBriefCommandEnvelope, value: i64) {
        match command {
            RequirementBriefCommandEnvelope::Create { deadline_at, .. }
            | RequirementBriefCommandEnvelope::Update { deadline_at, .. }
            | RequirementBriefCommandEnvelope::ChangeStatus { deadline_at, .. } => {
                *deadline_at = Some(value);
            }
        }
    }

    fn set_identity(
        command: &mut RequirementBriefCommandEnvelope,
        command_id: Option<String>,
        idempotency_key: Option<String>,
    ) {
        let (candidate_command_id, candidate_key) = match command {
            RequirementBriefCommandEnvelope::Create {
                command_id,
                idempotency_key,
                ..
            }
            | RequirementBriefCommandEnvelope::Update {
                command_id,
                idempotency_key,
                ..
            }
            | RequirementBriefCommandEnvelope::ChangeStatus {
                command_id,
                idempotency_key,
                ..
            } => (command_id, idempotency_key),
        };
        if let Some(value) = command_id {
            *candidate_command_id = value;
        }
        if let Some(value) = idempotency_key {
            *candidate_key = value;
        }
    }

    fn identity(command: &RequirementBriefCommandEnvelope) -> (String, String) {
        match command {
            RequirementBriefCommandEnvelope::Create {
                command_id,
                idempotency_key,
                ..
            }
            | RequirementBriefCommandEnvelope::Update {
                command_id,
                idempotency_key,
                ..
            }
            | RequirementBriefCommandEnvelope::ChangeStatus {
                command_id,
                idempotency_key,
                ..
            } => (command_id.clone(), idempotency_key.clone()),
        }
    }

    #[test]
    fn create_prefills_project_brief_builds_fixed_questions_and_is_one_per_project() {
        let (mut connection, project_id, _) = database();
        let created = execute_command(&mut connection, create_command(&project_id)).unwrap();
        let record = &created.response.requirement_brief;

        assert_eq!(record.question_set_version, QUESTION_SET_VERSION);
        assert_eq!(record.status, RequirementBriefStatus::Interviewing);
        assert_eq!(
            record.content.objective,
            "Launch a premium riverside property"
        );
        assert_eq!(record.content.audience, "Urban families");
        assert_eq!(record.content.deliverables[0], "90 second master");
        assert_eq!(record.content.key_message, "");
        assert_eq!(record.answers.len(), QUESTION_SET.len());
        for (answer, question) in record.answers.iter().zip(QUESTION_SET) {
            assert_eq!(answer.question_id, question.id);
            assert_eq!(answer.prompt, question.prompt);
            assert_eq!(answer.required, question.required);
            assert_eq!(answer.disposition, RequirementAnswerDisposition::Unanswered);
        }
        assert_eq!(created.emitted_events.len(), 1);
        assert_eq!(list(&connection).unwrap(), vec![record.clone()]);
        assert_eq!(replay_events(&connection, 0, 100).unwrap().len(), 1);

        let duplicate = execute_command(&mut connection, create_command(&project_id)).unwrap_err();
        assert_eq!(duplicate.code, "REQUIREMENT_BRIEF_EXISTS");
        assert_eq!(table_count(&connection, "requirement_briefs"), 1);
        assert_eq!(table_count(&connection, "requirement_brief_events"), 1);
        assert_eq!(
            table_count(&connection, "requirement_brief_command_receipts"),
            1
        );
    }

    #[test]
    fn protocol_compatibility_is_bounded() {
        let (mut connection, project_id, other_project_id) = database();
        let current_project_id = seed_project(&connection, &BriefRecord::default());

        for (supported_version, target_project_id) in [
            (PROTOCOL_1_3_VERSION, &project_id),
            (PREVIOUS_PROTOCOL_VERSION, &other_project_id),
            (PROTOCOL_VERSION, &current_project_id),
        ] {
            let mut command = create_command(target_project_id);
            if let RequirementBriefCommandEnvelope::Create {
                protocol_version, ..
            } = &mut command
            {
                *protocol_version = supported_version.to_string();
            }
            execute_command(&mut connection, command).unwrap();
        }

        for unsupported_version in ["1.2", "1.6"] {
            let mut command = create_command(&project_id);
            if let RequirementBriefCommandEnvelope::Create {
                protocol_version, ..
            } = &mut command
            {
                *protocol_version = unsupported_version.to_string();
            }
            let error = execute_command(&mut connection, command).unwrap_err();
            assert_eq!(error.code, "PROTOCOL_VERSION_MISMATCH");
            assert_eq!(
                error.message,
                format!(
                    "expected protocolVersion {PROTOCOL_1_3_VERSION}, {PREVIOUS_PROTOCOL_VERSION}, or {PROTOCOL_VERSION}"
                )
            );
        }

        assert_eq!(table_count(&connection, "requirement_briefs"), 3);
        assert_eq!(table_count(&connection, "requirement_brief_events"), 3);
        assert_eq!(
            table_count(&connection, "requirement_brief_command_receipts"),
            3
        );
    }
    #[test]
    fn file_database_recovers_record_events_and_receipt_after_reopen() {
        let directory = tempdir().unwrap();
        let database_path = directory.path().join("requirement-brief.sqlite3");
        let command;
        let committed;

        {
            let mut connection = Connection::open(&database_path).unwrap();
            create_schema(&connection);
            let project_id = seed_project(&connection, &project_brief());
            migrate(&connection).unwrap();
            migrate(&connection).unwrap();
            command = create_command(&project_id);
            committed = execute_command(&mut connection, command.clone())
                .unwrap()
                .response
                .requirement_brief;
        }

        let mut reopened = Connection::open(&database_path).unwrap();
        migrate(&reopened).unwrap();
        assert_eq!(list(&reopened).unwrap(), vec![committed.clone()]);
        let events = replay_events(&reopened, 0, 100).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].requirement_brief, committed);
        let replayed = execute_command(&mut reopened, command).unwrap();
        assert!(replayed.response.replayed);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(
            table_count(&reopened, "requirement_brief_command_receipts"),
            1
        );
    }

    #[test]
    fn state_machine_enforces_completeness_follow_up_confirmation_and_reopen() {
        let (mut connection, project_id, _) = database();
        let created = execute_command(&mut connection, create_command(&project_id)).unwrap();
        let brief_id = created.response.requirement_brief.id;

        let incomplete_review = status_command(
            &project_id,
            &brief_id,
            1,
            RequirementBriefStatus::Review,
            "reviewer",
        );
        assert_eq!(
            execute_command(&mut connection, incomplete_review)
                .unwrap_err()
                .code,
            "REQUIREMENT_BRIEF_INCOMPLETE"
        );

        let mut follow_up_answers = complete_answers();
        let required = follow_up_answers
            .iter_mut()
            .find(|answer| answer.question_id == "objective")
            .unwrap();
        required.answer = "Client sign-off is pending".into();
        required.disposition = RequirementAnswerDisposition::FollowUp;
        let updated = execute_command(
            &mut connection,
            update_command(
                &project_id,
                &brief_id,
                1,
                follow_up_answers,
                complete_content(),
            ),
        )
        .unwrap();
        assert_eq!(updated.response.requirement_brief.revision, 2);

        let review = execute_command(
            &mut connection,
            status_command(
                &project_id,
                &brief_id,
                2,
                RequirementBriefStatus::Review,
                "reviewer",
            ),
        )
        .unwrap();
        assert_eq!(
            review.response.requirement_brief.status,
            RequirementBriefStatus::Review
        );

        let incomplete_review_update = update_command(
            &project_id,
            &brief_id,
            3,
            complete_answers(),
            RequirementBriefContent::default(),
        );
        assert_eq!(
            execute_command(&mut connection, incomplete_review_update)
                .unwrap_err()
                .code,
            "REQUIREMENT_BRIEF_INCOMPLETE"
        );
        assert_eq!(list(&connection).unwrap()[0].revision, 3);

        let pending_confirmation = status_command(
            &project_id,
            &brief_id,
            3,
            RequirementBriefStatus::Confirmed,
            "approver-1",
        );
        assert_eq!(
            execute_command(&mut connection, pending_confirmation)
                .unwrap_err()
                .code,
            "REQUIREMENT_BRIEF_FOLLOW_UP_PENDING"
        );

        let resolved = execute_command(
            &mut connection,
            update_command(
                &project_id,
                &brief_id,
                3,
                complete_answers(),
                complete_content(),
            ),
        )
        .unwrap();
        assert_eq!(resolved.response.requirement_brief.revision, 4);
        let confirmed = execute_command(
            &mut connection,
            status_command(
                &project_id,
                &brief_id,
                4,
                RequirementBriefStatus::Confirmed,
                " approver-1 ",
            ),
        )
        .unwrap();
        let confirmed_record = &confirmed.response.requirement_brief;
        assert_eq!(confirmed_record.confirmed_by.as_deref(), Some("approver-1"));
        assert!(confirmed_record.confirmed_at.is_some());

        let direct_update = update_command(
            &project_id,
            &brief_id,
            5,
            complete_answers(),
            complete_content(),
        );
        assert_eq!(
            execute_command(&mut connection, direct_update)
                .unwrap_err()
                .code,
            "REQUIREMENT_BRIEF_CONFIRMED"
        );
        let illegal = status_command(
            &project_id,
            &brief_id,
            5,
            RequirementBriefStatus::Interviewing,
            "approver-1",
        );
        assert_eq!(
            execute_command(&mut connection, illegal).unwrap_err().code,
            "REQUIREMENT_BRIEF_STATUS_TRANSITION_INVALID"
        );

        let reopened = execute_command(
            &mut connection,
            status_command(
                &project_id,
                &brief_id,
                5,
                RequirementBriefStatus::Review,
                "editor",
            ),
        )
        .unwrap();
        assert_eq!(reopened.response.requirement_brief.confirmed_at, None);
        assert_eq!(reopened.response.requirement_brief.confirmed_by, None);
        let interviewing = execute_command(
            &mut connection,
            status_command(
                &project_id,
                &brief_id,
                6,
                RequirementBriefStatus::Interviewing,
                "editor",
            ),
        )
        .unwrap();
        assert_eq!(
            interviewing.response.requirement_brief.status,
            RequirementBriefStatus::Interviewing
        );
    }

    #[test]
    fn question_set_rejects_unknown_or_missing_ids_and_rebuilds_server_metadata() {
        let (mut connection, project_id, _) = database();
        let created = execute_command(&mut connection, create_command(&project_id)).unwrap();
        let record = created.response.requirement_brief;

        let mut unknown = complete_answers();
        unknown[0].question_id = "client-supplied-question".into();
        let error = execute_command(
            &mut connection,
            update_command(&project_id, &record.id, 1, unknown, complete_content()),
        )
        .unwrap_err();
        assert_eq!(error.code, "VALIDATION_FAILED");

        let mut missing = complete_answers();
        missing.pop();
        let error = execute_command(
            &mut connection,
            update_command(&project_id, &record.id, 1, missing, complete_content()),
        )
        .unwrap_err();
        assert_eq!(error.code, "VALIDATION_FAILED");
        assert_eq!(table_count(&connection, "requirement_brief_events"), 1);

        let mut tampered = record.answers.clone();
        tampered[0].prompt = "Client-controlled prompt".into();
        tampered[0].required = false;
        connection
            .execute(
                "UPDATE requirement_briefs SET answers_json = ?1 WHERE id = ?2",
                params![serde_json::to_string(&tampered).unwrap(), record.id],
            )
            .unwrap();
        let loaded = list(&connection).unwrap().remove(0);
        assert_eq!(loaded.answers[0].prompt, QUESTION_SET[0].prompt);
        assert_eq!(loaded.answers[0].required, QUESTION_SET[0].required);
        assert!(!serde_json::to_string(&loaded)
            .unwrap()
            .contains("Client-controlled prompt"));
    }

    #[test]
    fn reference_cases_allow_global_or_same_project_and_reject_other_or_missing() {
        let (mut connection, project_id, other_project_id) = database();
        let global_case = Uuid::new_v4().to_string();
        let project_case = Uuid::new_v4().to_string();
        let other_case = Uuid::new_v4().to_string();
        for (case_id, owner) in [
            (&global_case, None),
            (&project_case, Some(project_id.as_str())),
            (&other_case, Some(other_project_id.as_str())),
        ] {
            connection
                .execute(
                    "INSERT INTO cases (id, project_id, storage_rel_path) VALUES (?1, ?2, ?3)",
                    params![case_id, owner, SECRET_PATH],
                )
                .unwrap();
        }
        let created = execute_command(&mut connection, create_command(&project_id)).unwrap();
        let brief_id = created.response.requirement_brief.id;
        let mut accepted_content = complete_content();
        accepted_content.reference_case_ids = vec![global_case.clone(), project_case.clone()];
        let accepted = execute_command(
            &mut connection,
            update_command(
                &project_id,
                &brief_id,
                1,
                complete_answers(),
                accepted_content,
            ),
        )
        .unwrap();
        assert_eq!(
            accepted
                .response
                .requirement_brief
                .content
                .reference_case_ids,
            vec![global_case, project_case]
        );

        let mut wrong_content = complete_content();
        wrong_content.reference_case_ids = vec![other_case];
        let wrong = execute_command(
            &mut connection,
            update_command(&project_id, &brief_id, 2, complete_answers(), wrong_content),
        )
        .unwrap_err();
        assert_eq!(wrong.code, "REFERENCE_CASE_PROJECT_MISMATCH");

        let mut missing_content = complete_content();
        missing_content.reference_case_ids = vec![Uuid::new_v4().to_string()];
        let missing = execute_command(
            &mut connection,
            update_command(
                &project_id,
                &brief_id,
                2,
                complete_answers(),
                missing_content,
            ),
        )
        .unwrap_err();
        assert_eq!(missing.code, "REFERENCE_CASE_NOT_FOUND");
        assert_eq!(list(&connection).unwrap()[0].revision, 2);
        assert_eq!(table_count(&connection, "requirement_brief_events"), 2);
        assert_eq!(
            table_count(&connection, "requirement_brief_command_receipts"),
            2
        );

        for wire in [
            serde_json::to_string(&accepted.response.requirement_brief).unwrap(),
            serde_json::to_string(&accepted.emitted_events[0]).unwrap(),
            serde_json::to_string(&accepted.response.receipt).unwrap(),
            serde_json::to_string(&accepted.response).unwrap(),
        ] {
            assert!(!wire.contains(SECRET_PATH));
            assert!(!wire.contains("storage_rel_path"));
            assert!(!wire.contains("storageRelPath"));
        }
    }

    #[test]
    fn command_identity_conflicts_and_deadlines_follow_receipt_first_semantics() {
        let (mut connection, project_id, other_project_id) = database();
        let mut command = create_command(&project_id);
        set_deadline(&mut command, now_millis() + 60_000);
        let (command_id, idempotency_key) = identity(&command);
        let committed = execute_command(&mut connection, command.clone()).unwrap();

        let mut reused_key = create_command(&other_project_id);
        set_identity(&mut reused_key, None, Some(idempotency_key));
        assert_eq!(
            execute_command(&mut connection, reused_key)
                .unwrap_err()
                .code,
            "IDEMPOTENCY_KEY_REUSED"
        );
        let mut reused_command = create_command(&other_project_id);
        set_identity(&mut reused_command, Some(command_id), None);
        assert_eq!(
            execute_command(&mut connection, reused_command)
                .unwrap_err()
                .code,
            "COMMAND_ID_REUSED"
        );

        set_deadline(&mut command, now_millis() - 1);
        let replayed = execute_command(&mut connection, command).unwrap();
        assert!(replayed.response.replayed);
        assert_eq!(
            replayed.response.requirement_brief,
            committed.response.requirement_brief
        );

        let third_project = seed_project(&connection, &BriefRecord::default());
        let mut expired = create_command(&third_project);
        set_deadline(&mut expired, now_millis() - 1);
        assert_eq!(
            execute_command(&mut connection, expired).unwrap_err().code,
            "COMMAND_DEADLINE_EXCEEDED"
        );
        assert_eq!(table_count(&connection, "requirement_briefs"), 1);
        assert_eq!(table_count(&connection, "requirement_brief_events"), 1);
        assert_eq!(
            table_count(&connection, "requirement_brief_command_receipts"),
            1
        );
    }

    #[test]
    fn event_and_receipt_failures_roll_back_the_entire_command() {
        let (mut connection, project_id, other_project_id) = database();
        let created = execute_command(&mut connection, create_command(&project_id)).unwrap();
        let update = update_command(
            &project_id,
            &created.response.requirement_brief.id,
            1,
            complete_answers(),
            complete_content(),
        );
        connection
            .execute_batch(
                "CREATE TRIGGER reject_requirement_brief_event
                 BEFORE INSERT ON requirement_brief_events
                 BEGIN SELECT RAISE(ABORT, 'forced event failure'); END;",
            )
            .unwrap();
        assert_eq!(
            execute_command(&mut connection, update.clone())
                .unwrap_err()
                .code,
            "HOST_INTERNAL"
        );
        assert_eq!(list(&connection).unwrap()[0].revision, 1);
        assert_eq!(table_count(&connection, "requirement_brief_events"), 1);
        assert_eq!(
            table_count(&connection, "requirement_brief_command_receipts"),
            1
        );
        connection
            .execute_batch("DROP TRIGGER reject_requirement_brief_event;")
            .unwrap();
        execute_command(&mut connection, update).unwrap();

        let create_other = create_command(&other_project_id);
        connection
            .execute_batch(
                "CREATE TRIGGER reject_requirement_brief_receipt
                 BEFORE INSERT ON requirement_brief_command_receipts
                 BEGIN SELECT RAISE(ABORT, 'forced receipt failure'); END;",
            )
            .unwrap();
        assert_eq!(
            execute_command(&mut connection, create_other.clone())
                .unwrap_err()
                .code,
            "HOST_INTERNAL"
        );
        assert_eq!(table_count(&connection, "requirement_briefs"), 1);
        assert_eq!(table_count(&connection, "requirement_brief_events"), 2);
        assert_eq!(
            table_count(&connection, "requirement_brief_command_receipts"),
            2
        );
        connection
            .execute_batch("DROP TRIGGER reject_requirement_brief_receipt;")
            .unwrap();
        execute_command(&mut connection, create_other).unwrap();
        assert_eq!(table_count(&connection, "requirement_briefs"), 2);
    }

    #[test]
    fn revision_and_exact_project_context_are_enforced_without_partial_commit() {
        let (mut connection, project_id, other_project_id) = database();
        let mut mismatched_create = create_command(&project_id);
        if let RequirementBriefCommandEnvelope::Create { context, .. } = &mut mismatched_create {
            context.project_id = Some(other_project_id.clone());
        }
        assert_eq!(
            execute_command(&mut connection, mismatched_create)
                .unwrap_err()
                .code,
            "VALIDATION_FAILED"
        );
        let created = execute_command(&mut connection, create_command(&project_id)).unwrap();
        let brief_id = created.response.requirement_brief.id;

        let stale = update_command(
            &project_id,
            &brief_id,
            99,
            complete_answers(),
            complete_content(),
        );
        assert_eq!(
            execute_command(&mut connection, stale).unwrap_err().code,
            "REVISION_CONFLICT"
        );
        let wrong_project = update_command(
            &other_project_id,
            &brief_id,
            1,
            complete_answers(),
            complete_content(),
        );
        assert_eq!(
            execute_command(&mut connection, wrong_project)
                .unwrap_err()
                .code,
            "REQUIREMENT_BRIEF_PROJECT_MISMATCH"
        );
        assert_eq!(list(&connection).unwrap()[0].revision, 1);
        assert_eq!(table_count(&connection, "requirement_brief_events"), 1);
        assert_eq!(
            table_count(&connection, "requirement_brief_command_receipts"),
            1
        );
    }

    #[test]
    fn answer_content_reference_and_replay_boundaries_are_validated() {
        let (mut connection, project_id, _) = database();
        let created = execute_command(&mut connection, create_command(&project_id)).unwrap();
        let brief_id = created.response.requirement_brief.id;

        let mut oversized_answer = complete_answers();
        oversized_answer[0].answer = "x".repeat(MAX_ANSWER_CHARS + 1);
        assert_eq!(
            execute_command(
                &mut connection,
                update_command(
                    &project_id,
                    &brief_id,
                    1,
                    oversized_answer,
                    complete_content(),
                ),
            )
            .unwrap_err()
            .code,
            "VALIDATION_FAILED"
        );

        let mut too_many = complete_content();
        too_many.deliverables = (0..=MAX_LIST_ITEMS)
            .map(|index| format!("deliverable-{index}"))
            .collect();
        assert_eq!(
            execute_command(
                &mut connection,
                update_command(&project_id, &brief_id, 1, complete_answers(), too_many,),
            )
            .unwrap_err()
            .code,
            "VALIDATION_FAILED"
        );

        let mut invalid_reference = complete_content();
        invalid_reference.reference_case_ids = vec!["not-a-uuid".into()];
        assert_eq!(
            execute_command(
                &mut connection,
                update_command(
                    &project_id,
                    &brief_id,
                    1,
                    complete_answers(),
                    invalid_reference,
                ),
            )
            .unwrap_err()
            .code,
            "VALIDATION_FAILED"
        );

        let mut invalid_deadline = complete_content();
        invalid_deadline.deadline_at = Some(0);
        assert_eq!(
            execute_command(
                &mut connection,
                update_command(
                    &project_id,
                    &brief_id,
                    1,
                    complete_answers(),
                    invalid_deadline,
                ),
            )
            .unwrap_err()
            .code,
            "VALIDATION_FAILED"
        );
        assert_eq!(
            replay_events(&connection, -1, 100).unwrap_err().code,
            "VALIDATION_FAILED"
        );
        assert_eq!(replay_events(&connection, 0, 0).unwrap().len(), 1);
        assert_eq!(list(&connection).unwrap()[0].revision, 1);
    }
}
