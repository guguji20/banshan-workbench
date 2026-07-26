use crate::protocol::{
    CancelContractReviewPayload, CommandReceipt, ContractReviewCommandEnvelope,
    ContractReviewCommandResponse, ContractReviewDomainEvent, ContractReviewEventType,
    ContractReviewFailure, ContractReviewRecord, ContractReviewSessionRecord, ContractReviewStage,
    ContractReviewStatus, CreateContractReviewPayload, DecideReviewFindingPayload,
    DocumentBlockRecord, DocumentExtractionRecord, DocumentExtractionStatus, DocumentPageRecord,
    DocumentTableRecord, EvidenceAnchor, EvidenceContext, FindingDecisionRecord,
    GenerateReviewReportPayload, HostError, ListContractReviewsRequest, OperationContext,
    RetryContractReviewStagePayload, ReviewFindingDecision, ReviewFindingRecord,
    ReviewFindingSource, ReviewFindingStatus, ReviewReportRecord, RuleEvaluationRecord,
    StartContractReviewPayload, CONTRACT_REVIEW_PROTOCOL_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_REPLAY_LIMIT: u32 = 1_000;
const MAX_LIST_LIMIT: u32 = 500;
const DEFAULT_LIST_LIMIT: u32 = 100;
const MAX_REASON_CHARS: usize = 4_000;
const MAX_COMMENT_CHARS: usize = 16_000;
const STARTUP_RECOVERY_TRACE_ID: &str = "contract-review:startup-recovery";

#[derive(Debug)]
pub struct ContractReviewCommandOutcome {
    pub response: ContractReviewCommandResponse,
    pub emitted_events: Vec<ContractReviewDomainEvent>,
}

#[derive(Debug)]
pub struct ContractReviewMutationOutcome {
    pub contract_review: ContractReviewRecord,
    pub emitted_events: Vec<ContractReviewDomainEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletedContractReviewBinding {
    pub source_asset_id: String,
    pub review_id: String,
    pub report_asset_id: String,
}

pub(crate) fn completed_review_binding(
    connection: &Connection,
    workspace_id: &str,
    review_id: &str,
    report_asset_id: &str,
) -> Result<CompletedContractReviewBinding, HostError> {
    let review = load_review_required(connection, review_id)?;
    let session = &review.session;
    if session.workspace_id != workspace_id {
        return Err(HostError::new(
            "CONTRACT_REVIEW_WORKSPACE_MISMATCH",
            "completed contract review belongs to another business workspace",
            false,
        ));
    }
    if session.status != ContractReviewStatus::Completed
        || session.stage != ContractReviewStage::Completed
        || session.completed_at.is_none()
    {
        return Err(HostError::new(
            "CONTRACT_REVIEW_NOT_COMPLETED",
            "contract review must be completed before it can become a business contract",
            false,
        ));
    }
    let report = review
        .reports
        .iter()
        .find(|report| report.report_asset_id == report_asset_id)
        .ok_or_else(|| {
            HostError::new(
                "CONTRACT_REVIEW_REPORT_NOT_FOUND",
                "selected report asset does not belong to the completed contract review",
                false,
            )
        })?;
    if report.review_id != session.id
        || report.source_asset_id != session.source_asset_id
        || report.source_asset_sha256 != session.source_asset_sha256
    {
        return Err(HostError::new(
            "CONTRACT_REVIEW_BINDING_INVALID",
            "contract review report no longer matches its frozen source contract",
            false,
        ));
    }
    let source_asset = load_ready_document_asset(connection, &session.source_asset_id)?;
    if source_asset.sha256 != session.source_asset_sha256 {
        return Err(HostError::new(
            "CONTRACT_REVIEW_SOURCE_HASH_MISMATCH",
            "source contract hash no longer matches the completed review",
            false,
        ));
    }
    let report_asset = load_ready_asset(connection, report_asset_id)?;
    if report_asset.sha256 != report.report_asset_sha256 {
        return Err(HostError::new(
            "CONTRACT_REVIEW_REPORT_HASH_MISMATCH",
            "selected report hash no longer matches the completed review report",
            false,
        ));
    }
    Ok(CompletedContractReviewBinding {
        source_asset_id: session.source_asset_id.clone(),
        review_id: session.id.clone(),
        report_asset_id: report.report_asset_id.clone(),
    })
}

/// Creates the contract-review domain schema. This service stores only stable
/// Local Vault asset IDs and hashes. It deliberately owns no R2 state.
pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS contract_review_sessions (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                source_asset_id TEXT NOT NULL,
                source_asset_sha256 TEXT NOT NULL CHECK(length(source_asset_sha256) = 64),
                source_file_name TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN (
                    'draft','running','awaitingConfirmation','completed','failed','cancelled'
                )),
                stage TEXT NOT NULL CHECK(stage IN (
                    'created','extracting','awaitingOcr','reviewingRules','reviewingAgent',
                    'mergingFindings','awaitingConfirmation','generatingReport','completed'
                )),
                extraction_id TEXT,
                report_asset_id TEXT,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                completed_at INTEGER,
                failure_json TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_contract_review_sessions_workspace_updated
                ON contract_review_sessions(workspace_id, updated_at DESC, id DESC);
            CREATE INDEX IF NOT EXISTS idx_contract_review_sessions_status_updated
                ON contract_review_sessions(status, updated_at DESC, id DESC);
            CREATE INDEX IF NOT EXISTS idx_contract_review_sessions_source_asset
                ON contract_review_sessions(source_asset_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS document_extractions (
                id TEXT PRIMARY KEY NOT NULL,
                review_id TEXT NOT NULL UNIQUE,
                source_asset_id TEXT NOT NULL,
                source_asset_sha256 TEXT NOT NULL CHECK(length(source_asset_sha256) = 64),
                status TEXT NOT NULL CHECK(status IN (
                    'pending','running','awaitingOcr','completed','failed','cancelled'
                )),
                record_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                completed_at INTEGER,
                FOREIGN KEY(review_id) REFERENCES contract_review_sessions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_document_extractions_source_asset
                ON document_extractions(source_asset_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS document_pages (
                id TEXT PRIMARY KEY NOT NULL,
                extraction_id TEXT NOT NULL,
                page_index INTEGER NOT NULL CHECK(page_index >= 0),
                record_json TEXT NOT NULL,
                UNIQUE(extraction_id, page_index),
                FOREIGN KEY(extraction_id) REFERENCES document_extractions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_document_pages_extraction_order
                ON document_pages(extraction_id, page_index ASC);

            CREATE TABLE IF NOT EXISTS document_blocks (
                id TEXT PRIMARY KEY NOT NULL,
                extraction_id TEXT NOT NULL,
                page_id TEXT NOT NULL,
                page_index INTEGER NOT NULL CHECK(page_index >= 0),
                order_index INTEGER NOT NULL CHECK(order_index >= 0),
                record_json TEXT NOT NULL,
                UNIQUE(extraction_id, page_index, order_index),
                FOREIGN KEY(extraction_id) REFERENCES document_extractions(id) ON DELETE CASCADE,
                FOREIGN KEY(page_id) REFERENCES document_pages(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_document_blocks_extraction_order
                ON document_blocks(extraction_id, page_index ASC, order_index ASC);

            CREATE TABLE IF NOT EXISTS document_tables (
                id TEXT PRIMARY KEY NOT NULL,
                extraction_id TEXT NOT NULL,
                page_id TEXT NOT NULL,
                page_index INTEGER NOT NULL CHECK(page_index >= 0),
                order_index INTEGER NOT NULL CHECK(order_index >= 0),
                record_json TEXT NOT NULL,
                UNIQUE(extraction_id, page_index, order_index),
                FOREIGN KEY(extraction_id) REFERENCES document_extractions(id) ON DELETE CASCADE,
                FOREIGN KEY(page_id) REFERENCES document_pages(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_document_tables_extraction_order
                ON document_tables(extraction_id, page_index ASC, order_index ASC);

            CREATE TABLE IF NOT EXISTS contract_review_evidence (
                id TEXT PRIMARY KEY NOT NULL,
                review_id TEXT NOT NULL,
                extraction_id TEXT NOT NULL,
                source_asset_id TEXT NOT NULL,
                page_index INTEGER NOT NULL CHECK(page_index >= 0),
                record_json TEXT NOT NULL,
                FOREIGN KEY(review_id) REFERENCES contract_review_sessions(id) ON DELETE CASCADE,
                FOREIGN KEY(extraction_id) REFERENCES document_extractions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_contract_review_evidence_review_page
                ON contract_review_evidence(review_id, page_index ASC, id ASC);
            CREATE INDEX IF NOT EXISTS idx_contract_review_evidence_extraction
                ON contract_review_evidence(extraction_id, page_index ASC, id ASC);

            CREATE TABLE IF NOT EXISTS contract_review_findings (
                id TEXT PRIMARY KEY NOT NULL,
                review_id TEXT NOT NULL,
                source TEXT NOT NULL CHECK(source IN ('rule','agent','merged','manual')),
                status TEXT NOT NULL CHECK(status IN ('open','decided','superseded')),
                decision TEXT NOT NULL CHECK(decision IN (
                    'unreviewed','confirmed','rejected','acceptedRisk','needsRevision'
                )),
                revision INTEGER NOT NULL CHECK(revision >= 1),
                record_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY(review_id) REFERENCES contract_review_sessions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_contract_review_findings_review_status
                ON contract_review_findings(review_id, status, created_at ASC, id ASC);
            CREATE INDEX IF NOT EXISTS idx_contract_review_findings_source
                ON contract_review_findings(review_id, source, created_at ASC, id ASC);

            CREATE TABLE IF NOT EXISTS contract_review_rule_evaluations (
                id TEXT PRIMARY KEY NOT NULL,
                review_id TEXT NOT NULL,
                rule_id TEXT NOT NULL,
                record_json TEXT NOT NULL,
                evaluated_at INTEGER NOT NULL,
                FOREIGN KEY(review_id) REFERENCES contract_review_sessions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_contract_review_rule_evaluations_review
                ON contract_review_rule_evaluations(review_id, evaluated_at ASC, id ASC);

            CREATE TABLE IF NOT EXISTS contract_review_decisions (
                id TEXT PRIMARY KEY NOT NULL,
                review_id TEXT NOT NULL,
                finding_id TEXT NOT NULL,
                finding_revision INTEGER NOT NULL CHECK(finding_revision >= 1),
                record_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(review_id) REFERENCES contract_review_sessions(id) ON DELETE CASCADE,
                FOREIGN KEY(finding_id) REFERENCES contract_review_findings(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_contract_review_decisions_finding
                ON contract_review_decisions(finding_id, finding_revision ASC, created_at ASC);
            CREATE INDEX IF NOT EXISTS idx_contract_review_decisions_review
                ON contract_review_decisions(review_id, created_at ASC, id ASC);

            CREATE TABLE IF NOT EXISTS contract_review_reports (
                id TEXT PRIMARY KEY NOT NULL,
                review_id TEXT NOT NULL,
                report_asset_id TEXT NOT NULL,
                format TEXT NOT NULL CHECK(format IN ('json','html','docx')),
                record_json TEXT NOT NULL,
                generated_at INTEGER NOT NULL,
                UNIQUE(review_id, format, report_asset_id),
                FOREIGN KEY(review_id) REFERENCES contract_review_sessions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_contract_review_reports_review
                ON contract_review_reports(review_id, generated_at ASC, id ASC);

            CREATE TABLE IF NOT EXISTS contract_review_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL CHECK(event_type IN (
                    'contractReview.created','contractReview.started',
                    'contractReview.stageChanged','contractReview.extractionCompleted',
                    'contractReview.ocrRequired','contractReview.findingAdded',
                    'contractReview.findingUpdated','contractReview.findingDecided',
                    'contractReview.reportGenerated','contractReview.completed',
                    'contractReview.failed','contractReview.cancelled'
                )),
                aggregate_id TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                occurred_at INTEGER NOT NULL,
                trace_id TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                FOREIGN KEY(aggregate_id) REFERENCES contract_review_sessions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_contract_review_events_aggregate
                ON contract_review_events(aggregate_id, sequence ASC);

            CREATE TABLE IF NOT EXISTS contract_review_command_receipts (
                command_id TEXT PRIMARY KEY NOT NULL,
                idempotency_key TEXT NOT NULL UNIQUE,
                request_fingerprint TEXT NOT NULL,
                command_type TEXT NOT NULL,
                aggregate_id TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                last_event_sequence INTEGER NOT NULL CHECK(last_event_sequence >= 1),
                response_json TEXT NOT NULL,
                completed_at INTEGER NOT NULL,
                FOREIGN KEY(aggregate_id) REFERENCES contract_review_sessions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_contract_review_command_receipts_completed
                ON contract_review_command_receipts(completed_at DESC);
            "#,
        )
        .map_err(sql_error)?;
    recover_interrupted_reviews(connection)
}

fn recover_interrupted_reviews(connection: &Connection) -> Result<(), HostError> {
    let interrupted_ids = {
        let mut statement = connection
            .prepare(
                "SELECT id FROM contract_review_sessions \
                 WHERE status = 'running' ORDER BY updated_at ASC, id ASC",
            )
            .map_err(sql_error)?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        ids
    };
    if interrupted_ids.is_empty() {
        return Ok(());
    }

    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    for review_id in interrupted_ids {
        let mut session = load_session_required(&transaction, &review_id)?;
        if session.status != ContractReviewStatus::Running {
            continue;
        }
        let now = now_ms()?;
        let interrupted_stage = session.stage;
        session.status = ContractReviewStatus::Failed;
        session.failure = Some(ContractReviewFailure {
            code: "CONTRACT_REVIEW_INTERRUPTED".to_string(),
            message: "contract review was interrupted before the desktop host stopped".to_string(),
            retryable: true,
            stage: interrupted_stage,
        });
        session.revision += 1;
        session.updated_at = now;
        persist_session(&transaction, &session)?;
        finalize_single_event_mutation(
            &transaction,
            session.id,
            ContractReviewEventType::Failed,
            STARTUP_RECOVERY_TRACE_ID,
            now,
        )?;
    }
    transaction.commit().map_err(sql_error)
}

pub fn execute_command(
    connection: &mut Connection,
    command: ContractReviewCommandEnvelope,
) -> Result<ContractReviewCommandOutcome, HostError> {
    let command = normalize_command(command)?;
    let fingerprint = command_fingerprint(&command)?;
    let meta = command.meta().clone();
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
        return Ok(ContractReviewCommandOutcome {
            response,
            emitted_events: Vec::new(),
        });
    }
    validate_deadline(meta.deadline_at)?;

    let mutation = match &command {
        NormalizedCommand::Create { payload, .. } => {
            create_review_tx(&transaction, payload, &meta.context)?
        }
        NormalizedCommand::Start { payload, .. } => start_review_tx(
            &transaction,
            payload,
            required_expected_revision(meta.expected_revision)?,
            &meta.context.trace_id,
        )?,
        NormalizedCommand::Cancel { payload, .. } => cancel_review_tx(
            &transaction,
            payload,
            required_expected_revision(meta.expected_revision)?,
            &meta.context.trace_id,
        )?,
        NormalizedCommand::DecideFinding { payload, .. } => decide_finding_tx(
            &transaction,
            payload,
            required_expected_revision(meta.expected_revision)?,
            &meta.context.actor_id,
            &meta.context.trace_id,
        )?,
        NormalizedCommand::GenerateReport { payload, .. } => prepare_report_tx(
            &transaction,
            payload,
            required_expected_revision(meta.expected_revision)?,
            &meta.context.trace_id,
        )?,
        NormalizedCommand::RetryStage { payload, .. } => retry_stage_tx(
            &transaction,
            payload,
            required_expected_revision(meta.expected_revision)?,
            &meta.context.trace_id,
        )?,
    };

    let last_event_sequence = mutation
        .emitted_events
        .last()
        .map(|event| event.sequence)
        .ok_or_else(|| HostError::internal("contract review command emitted no durable event"))?;
    let receipt = CommandReceipt {
        command_id: meta.command_id.clone(),
        idempotency_key: meta.idempotency_key.clone(),
        command_type: command.command_type().to_string(),
        aggregate_id: mutation.contract_review.session.id.clone(),
        revision: mutation.contract_review.session.revision,
        last_event_sequence,
        completed_at: now_ms()?,
    };
    let response = ContractReviewCommandResponse {
        receipt,
        contract_review: mutation.contract_review.clone(),
        replayed: false,
    };
    transaction
        .execute(
            "INSERT INTO contract_review_command_receipts
             (command_id, idempotency_key, request_fingerprint, command_type,
              aggregate_id, revision, last_event_sequence, response_json, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                meta.command_id,
                meta.idempotency_key,
                fingerprint,
                command.command_type(),
                mutation.contract_review.session.id,
                mutation.contract_review.session.revision,
                last_event_sequence,
                to_json(&response)?,
                response.receipt.completed_at,
            ],
        )
        .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)?;
    Ok(ContractReviewCommandOutcome {
        response,
        emitted_events: mutation.emitted_events,
    })
}

pub fn get_review(
    connection: &Connection,
    review_id: &str,
) -> Result<ContractReviewRecord, HostError> {
    let review_id = normalize_uuid("reviewId", review_id.to_string())?;
    load_review(connection, &review_id)?.ok_or_else(|| review_not_found(&review_id))
}

pub fn list_review_findings(
    connection: &Connection,
    review_id: &str,
    status: Option<ReviewFindingStatus>,
) -> Result<Vec<ReviewFindingRecord>, HostError> {
    let review_id = normalize_uuid("reviewId", review_id.to_string())?;
    load_session_required(connection, &review_id)?;
    let findings = load_review_findings(connection, &review_id)?;
    Ok(match status {
        Some(status) => findings
            .into_iter()
            .filter(|finding| finding.status == status)
            .collect(),
        None => findings,
    })
}

pub fn get_evidence_context(
    connection: &Connection,
    evidence_id: &str,
) -> Result<EvidenceContext, HostError> {
    let evidence_id = normalize_uuid("evidenceId", evidence_id.to_string())?;
    let stored: Option<(String, String, String, i64, String)> = connection
        .query_row(
            "SELECT review_id, extraction_id, source_asset_id, page_index, record_json
             FROM contract_review_evidence WHERE id = ?1",
            [&evidence_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(sql_error)?;
    let Some((review_id, extraction_id, source_asset_id, page_index, record_json)) = stored else {
        return Err(HostError::new(
            "EVIDENCE_NOT_FOUND",
            format!("evidence {evidence_id} was not found"),
            false,
        ));
    };
    let evidence: EvidenceAnchor = from_json(&record_json)?;
    if evidence.id != evidence_id
        || evidence.extraction_id != extraction_id
        || evidence.source_asset_id != source_asset_id
        || evidence.page_index != page_index
    {
        return Err(evidence_context_inconsistent(
            &evidence_id,
            "stored evidence metadata does not match its serialized record",
        ));
    }

    let session = load_session_required(connection, &review_id)?;
    if session.extraction_id.as_deref() != Some(extraction_id.as_str())
        || session.source_asset_id != source_asset_id
    {
        return Err(evidence_context_inconsistent(
            &evidence_id,
            "evidence source identity does not match its review session",
        ));
    }
    let extraction = load_extraction_for_review(connection, &review_id)?.ok_or_else(|| {
        evidence_context_inconsistent(&evidence_id, "review extraction is missing")
    })?;
    if extraction.id != extraction_id
        || extraction.review_id != review_id
        || extraction.source_asset_id != source_asset_id
        || extraction.source_asset_sha256 != session.source_asset_sha256
    {
        return Err(evidence_context_inconsistent(
            &evidence_id,
            "evidence source identity does not match its extraction",
        ));
    }

    let page = extraction
        .pages
        .iter()
        .find(|page| page.page_index == evidence.page_index)
        .cloned()
        .ok_or_else(|| {
            HostError::new(
                "EVIDENCE_PAGE_NOT_FOUND",
                format!(
                    "page {} for evidence {evidence_id} was not found",
                    evidence.page_index
                ),
                false,
            )
        })?;
    if page.extraction_id != evidence.extraction_id || page.page_index != evidence.page_index {
        return Err(evidence_context_inconsistent(
            &evidence_id,
            "evidence page does not belong to the expected extraction",
        ));
    }

    let block = match evidence.block_id.as_deref() {
        Some(block_id) => {
            let block = extraction
                .blocks
                .iter()
                .find(|block| block.id == block_id)
                .cloned()
                .ok_or_else(|| {
                    HostError::new(
                        "EVIDENCE_BLOCK_NOT_FOUND",
                        format!("block {block_id} for evidence {evidence_id} was not found"),
                        false,
                    )
                })?;
            if block.extraction_id != evidence.extraction_id
                || block.page_id != page.id
                || block.page_index != evidence.page_index
            {
                return Err(evidence_context_inconsistent(
                    &evidence_id,
                    "evidence block does not belong to the resolved page",
                ));
            }
            Some(block)
        }
        None => None,
    };

    Ok(EvidenceContext {
        evidence,
        page,
        block,
    })
}

pub fn list_reviews(
    connection: &Connection,
    request: &ListContractReviewsRequest,
) -> Result<Vec<ContractReviewRecord>, HostError> {
    let workspace_id = request
        .workspace_id
        .clone()
        .map(|value| normalize_uuid("workspaceId", value))
        .transpose()?;
    let status = request.status.map(|value| enum_to_db(&value)).transpose()?;
    let limit = request
        .limit
        .unwrap_or(DEFAULT_LIST_LIMIT)
        .clamp(1, MAX_LIST_LIMIT);
    let ids = {
        let mut statement = connection
            .prepare(
                "SELECT id FROM contract_review_sessions
                 WHERE (?1 IS NULL OR workspace_id = ?1)
                   AND (?2 IS NULL OR status = ?2)
                 ORDER BY updated_at DESC, id DESC
                 LIMIT ?3",
            )
            .map_err(sql_error)?;
        let rows = statement
            .query_map(params![workspace_id, status, i64::from(limit)], |row| {
                row.get::<_, String>(0)
            })
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        rows
    };
    ids.into_iter()
        .map(|id| {
            load_review(connection, &id)?.ok_or_else(|| {
                HostError::internal(format!("contract review {id} disappeared while listing"))
            })
        })
        .collect()
}

pub fn replay_events(
    connection: &Connection,
    after_sequence: i64,
    limit: u32,
) -> Result<Vec<ContractReviewDomainEvent>, HostError> {
    if after_sequence < 0 {
        return Err(HostError::validation("afterSequence cannot be negative"));
    }
    if limit == 0 {
        return Err(HostError::validation("limit must be at least 1"));
    }
    let mut statement = connection
        .prepare(
            "SELECT sequence, event_id, event_type, aggregate_id, revision,
                    occurred_at, trace_id, payload_json
             FROM contract_review_events
             WHERE sequence > ?1
             ORDER BY sequence ASC
             LIMIT ?2",
        )
        .map_err(sql_error)?;
    let events = statement
        .query_map(
            params![after_sequence, i64::from(limit.min(MAX_REPLAY_LIMIT))],
            event_from_row,
        )
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(events)
}

pub fn save_extraction(
    connection: &mut Connection,
    extraction: &DocumentExtractionRecord,
    evidence: &[EvidenceAnchor],
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let trace_id = normalize_required("traceId", trace_id, 256)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let outcome = save_extraction_tx(
        &transaction,
        extraction,
        evidence,
        expected_session_revision,
        &trace_id,
    )?;
    transaction.commit().map_err(sql_error)?;
    Ok(outcome)
}

pub fn replace_rule_evaluations_and_findings(
    connection: &mut Connection,
    review_id: &str,
    evaluations: &[RuleEvaluationRecord],
    findings: &[ReviewFindingRecord],
    evidence: &[EvidenceAnchor],
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let review_id = normalize_uuid("reviewId", review_id.to_string())?;
    let trace_id = normalize_required("traceId", trace_id, 256)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let outcome = replace_rule_results_tx(
        &transaction,
        &review_id,
        evaluations,
        findings,
        evidence,
        expected_session_revision,
        &trace_id,
    )?;
    transaction.commit().map_err(sql_error)?;
    Ok(outcome)
}

pub fn begin_agent_review(
    connection: &mut Connection,
    review_id: &str,
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let review_id = normalize_uuid("reviewId", review_id.to_string())?;
    let trace_id = normalize_required("traceId", trace_id, 256)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let mut session = load_session_required(&transaction, &review_id)?;
    require_session_revision(&session, expected_session_revision)?;
    ensure_not_terminal(&session)?;
    if session.stage != ContractReviewStage::AwaitingConfirmation {
        return Err(invalid_state(
            &session,
            "Agent review can start only after deterministic rules are persisted",
        ));
    }
    let decision_count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM contract_review_decisions WHERE review_id = ?1",
            [&review_id],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    if decision_count > 0 {
        return Err(HostError::new(
            "CONTRACT_AGENT_REVIEW_ALREADY_DECIDED",
            "cannot start Agent review after a human decision exists",
            false,
        ));
    }
    let now = now_ms()?;
    session.status = ContractReviewStatus::Running;
    session.stage = ContractReviewStage::ReviewingAgent;
    session.failure = None;
    session.updated_at = now;
    session.revision += 1;
    persist_session(&transaction, &session)?;
    let outcome = finalize_single_event_mutation(
        &transaction,
        session.id,
        ContractReviewEventType::StageChanged,
        &trace_id,
        now,
    )?;
    transaction.commit().map_err(sql_error)?;
    Ok(outcome)
}

pub fn replace_agent_findings_and_await_confirmation(
    connection: &mut Connection,
    review_id: &str,
    findings: &[ReviewFindingRecord],
    evidence: &[EvidenceAnchor],
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let review_id = normalize_uuid("reviewId", review_id.to_string())?;
    let trace_id = normalize_required("traceId", trace_id, 256)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let mut session = load_session_required(&transaction, &review_id)?;
    require_session_revision(&session, expected_session_revision)?;
    ensure_not_terminal(&session)?;
    if session.stage != ContractReviewStage::ReviewingAgent
        && session.stage != ContractReviewStage::MergingFindings
    {
        return Err(invalid_state(
            &session,
            "Agent findings can be saved only during Agent review",
        ));
    }
    let extraction = load_extraction_for_review(&transaction, &review_id)?.ok_or_else(|| {
        HostError::new(
            "EXTRACTION_REQUIRED",
            "Agent findings cannot be saved before extraction",
            false,
        )
    })?;
    let page_indexes = extraction
        .pages
        .iter()
        .map(|page| page.page_index)
        .collect::<HashSet<_>>();
    let block_ids = extraction
        .blocks
        .iter()
        .map(|block| block.id.clone())
        .collect::<HashSet<_>>();
    let mut evidence_ids = HashSet::new();
    for anchor in evidence {
        validate_evidence(
            anchor,
            &extraction.id,
            &extraction.source_asset_id,
            &page_indexes,
            &block_ids,
        )?;
        if !evidence_ids.insert(anchor.id.clone()) {
            return Err(HostError::validation("duplicate Agent evidence ID"));
        }
        upsert_evidence(&transaction, &review_id, anchor)?;
    }
    let existing_evidence_ids = {
        let mut statement = transaction
            .prepare("SELECT id FROM contract_review_evidence WHERE review_id = ?1")
            .map_err(sql_error)?;
        let ids = statement
            .query_map([&review_id], |row| row.get::<_, String>(0))
            .map_err(sql_error)?
            .collect::<Result<HashSet<_>, _>>()
            .map_err(sql_error)?;
        ids
    };
    let mut finding_ids = HashSet::new();
    for finding in findings {
        validate_agent_finding(finding, &review_id)?;
        if !finding_ids.insert(finding.id.clone()) {
            return Err(HostError::validation("duplicate Agent finding ID"));
        }
        for evidence_id in &finding.evidence_ids {
            if !existing_evidence_ids.contains(evidence_id) {
                return Err(HostError::validation(format!(
                    "Agent finding references unknown evidence {evidence_id}"
                )));
            }
        }
    }
    let decided_agent_findings: i64 = transaction
        .query_row(
            "SELECT COUNT(*)
             FROM contract_review_decisions decision
             JOIN contract_review_findings finding ON finding.id = decision.finding_id
             WHERE finding.review_id = ?1 AND finding.source = 'agent'",
            [&review_id],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    if decided_agent_findings > 0 {
        return Err(HostError::new(
            "AGENT_RESULTS_ALREADY_DECIDED",
            "cannot replace Agent findings after a human decision exists",
            false,
        ));
    }
    transaction
        .execute(
            "DELETE FROM contract_review_findings WHERE review_id = ?1 AND source = 'agent'",
            [&review_id],
        )
        .map_err(sql_error)?;
    let now = now_ms()?;
    for finding in findings {
        let mut finding = finding.clone();
        finding.revision = 1;
        finding.status = ReviewFindingStatus::Open;
        finding.decision = ReviewFindingDecision::Unreviewed;
        finding.created_at = now;
        finding.updated_at = now;
        insert_finding(&transaction, &finding)?;
    }
    session.status = ContractReviewStatus::AwaitingConfirmation;
    session.stage = ContractReviewStage::AwaitingConfirmation;
    session.failure = None;
    session.updated_at = now;
    session.revision += 1;
    persist_session(&transaction, &session)?;
    let event_type = if findings.is_empty() {
        ContractReviewEventType::StageChanged
    } else {
        ContractReviewEventType::FindingAdded
    };
    let outcome =
        finalize_single_event_mutation(&transaction, session.id, event_type, &trace_id, now)?;
    transaction.commit().map_err(sql_error)?;
    Ok(outcome)
}

pub fn complete_agent_review_degraded(
    connection: &mut Connection,
    review_id: &str,
    failure: &ContractReviewFailure,
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let review_id = normalize_uuid("reviewId", review_id.to_string())?;
    let trace_id = normalize_required("traceId", trace_id, 256)?;
    validate_failure(failure)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let mut session = load_session_required(&transaction, &review_id)?;
    require_session_revision(&session, expected_session_revision)?;
    ensure_not_terminal(&session)?;
    if session.stage != ContractReviewStage::ReviewingAgent
        && session.stage != ContractReviewStage::MergingFindings
    {
        return Err(invalid_state(
            &session,
            "degraded Agent completion requires the Agent review stage",
        ));
    }
    let now = now_ms()?;
    session.status = ContractReviewStatus::AwaitingConfirmation;
    session.stage = ContractReviewStage::AwaitingConfirmation;
    session.failure = Some(failure.clone());
    session.updated_at = now;
    session.revision += 1;
    persist_session(&transaction, &session)?;
    let outcome = finalize_single_event_mutation(
        &transaction,
        session.id,
        ContractReviewEventType::StageChanged,
        &trace_id,
        now,
    )?;
    transaction.commit().map_err(sql_error)?;
    Ok(outcome)
}

pub fn decide_finding(
    connection: &mut Connection,
    payload: &DecideReviewFindingPayload,
    expected_finding_revision: i64,
    actor_id: &str,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let payload = normalize_decision_payload(payload.clone())?;
    let actor_id = normalize_required("actorId", actor_id, 256)?;
    let trace_id = normalize_required("traceId", trace_id, 256)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let outcome = decide_finding_tx(
        &transaction,
        &payload,
        validate_positive_revision(expected_finding_revision)?,
        &actor_id,
        &trace_id,
    )?;
    transaction.commit().map_err(sql_error)?;
    Ok(outcome)
}

pub fn save_report_and_complete(
    connection: &mut Connection,
    report: &ReviewReportRecord,
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let trace_id = normalize_required("traceId", trace_id, 256)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let outcome =
        save_report_and_complete_tx(&transaction, report, expected_session_revision, &trace_id)?;
    transaction.commit().map_err(sql_error)?;
    Ok(outcome)
}

pub fn update_stage(
    connection: &mut Connection,
    review_id: &str,
    stage: ContractReviewStage,
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let review_id = normalize_uuid("reviewId", review_id.to_string())?;
    let trace_id = normalize_required("traceId", trace_id, 256)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let outcome = update_stage_tx(
        &transaction,
        &review_id,
        stage,
        expected_session_revision,
        &trace_id,
        ContractReviewEventType::StageChanged,
    )?;
    transaction.commit().map_err(sql_error)?;
    Ok(outcome)
}

pub fn fail_review(
    connection: &mut Connection,
    review_id: &str,
    failure: &ContractReviewFailure,
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let review_id = normalize_uuid("reviewId", review_id.to_string())?;
    validate_failure(failure)?;
    let trace_id = normalize_required("traceId", trace_id, 256)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let outcome = fail_review_tx(
        &transaction,
        &review_id,
        failure,
        expected_session_revision,
        &trace_id,
    )?;
    transaction.commit().map_err(sql_error)?;
    Ok(outcome)
}

pub fn cancel_review(
    connection: &mut Connection,
    payload: &CancelContractReviewPayload,
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let payload = normalize_cancel_payload(payload.clone())?;
    let trace_id = normalize_required("traceId", trace_id, 256)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let outcome = cancel_review_tx(&transaction, &payload, expected_session_revision, &trace_id)?;
    transaction.commit().map_err(sql_error)?;
    Ok(outcome)
}

fn create_review_tx(
    transaction: &Transaction<'_>,
    payload: &CreateContractReviewPayload,
    context: &OperationContext,
) -> Result<ContractReviewMutationOutcome, HostError> {
    ensure_workspace_exists(transaction, &payload.workspace_id)?;
    let source = load_ready_document_asset(transaction, &payload.source_asset_id)?;
    let now = now_ms()?;
    let session = ContractReviewSessionRecord {
        id: Uuid::new_v4().to_string(),
        workspace_id: payload.workspace_id.clone(),
        source_asset_id: payload.source_asset_id.clone(),
        source_asset_sha256: source.sha256,
        source_file_name: source.original_name,
        status: ContractReviewStatus::Draft,
        stage: ContractReviewStage::Created,
        extraction_id: None,
        report_asset_id: None,
        revision: 1,
        created_at: now,
        updated_at: now,
        completed_at: None,
        failure: None,
    };
    insert_session(transaction, &session)?;
    let record = load_review_required(transaction, &session.id)?;
    let event = append_event(
        transaction,
        ContractReviewEventType::Created,
        &record,
        &context.trace_id,
        now,
    )?;
    Ok(ContractReviewMutationOutcome {
        contract_review: record,
        emitted_events: vec![event],
    })
}

fn start_review_tx(
    transaction: &Transaction<'_>,
    payload: &StartContractReviewPayload,
    expected_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let mut session = load_session_required(transaction, &payload.review_id)?;
    require_session_revision(&session, expected_revision)?;
    if session.status != ContractReviewStatus::Draft
        || session.stage != ContractReviewStage::Created
    {
        return Err(invalid_state(
            &session,
            "only a draft review at created stage can be started",
        ));
    }
    let now = now_ms()?;
    session.status = ContractReviewStatus::Running;
    session.stage = ContractReviewStage::Extracting;
    session.revision += 1;
    session.updated_at = now;
    session.failure = None;
    persist_session(transaction, &session)?;
    finalize_single_event_mutation(
        transaction,
        session.id,
        ContractReviewEventType::Started,
        trace_id,
        now,
    )
}

fn update_stage_tx(
    transaction: &Transaction<'_>,
    review_id: &str,
    stage: ContractReviewStage,
    expected_revision: i64,
    trace_id: &str,
    event_type: ContractReviewEventType,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let mut session = load_session_required(transaction, review_id)?;
    require_session_revision(&session, expected_revision)?;
    if matches!(
        session.status,
        ContractReviewStatus::Completed | ContractReviewStatus::Cancelled
    ) {
        return Err(invalid_state(
            &session,
            "terminal review cannot change stage",
        ));
    }
    let now = now_ms()?;
    session.stage = stage;
    session.status = status_for_stage(stage);
    session.completed_at = (stage == ContractReviewStage::Completed).then_some(now);
    session.failure = None;
    session.revision += 1;
    session.updated_at = now;
    persist_session(transaction, &session)?;
    finalize_single_event_mutation(transaction, session.id, event_type, trace_id, now)
}

fn fail_review_tx(
    transaction: &Transaction<'_>,
    review_id: &str,
    failure: &ContractReviewFailure,
    expected_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let mut session = load_session_required(transaction, review_id)?;
    require_session_revision(&session, expected_revision)?;
    if matches!(
        session.status,
        ContractReviewStatus::Completed | ContractReviewStatus::Cancelled
    ) {
        return Err(invalid_state(&session, "terminal review cannot fail"));
    }
    let now = now_ms()?;
    session.status = ContractReviewStatus::Failed;
    session.stage = failure.stage;
    session.failure = Some(failure.clone());
    session.revision += 1;
    session.updated_at = now;
    persist_session(transaction, &session)?;
    finalize_single_event_mutation(
        transaction,
        session.id,
        ContractReviewEventType::Failed,
        trace_id,
        now,
    )
}

fn cancel_review_tx(
    transaction: &Transaction<'_>,
    payload: &CancelContractReviewPayload,
    expected_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let mut session = load_session_required(transaction, &payload.review_id)?;
    validate_positive_revision(expected_revision)?;
    if expected_revision > session.revision {
        return Err(HostError::conflict(format!(
            "contract review revision conflict: expected {expected_revision}, actual {}",
            session.revision
        )));
    }
    if session.status == ContractReviewStatus::Completed {
        return Err(invalid_state(
            &session,
            "completed review cannot be cancelled",
        ));
    }
    if session.status == ContractReviewStatus::Cancelled {
        return Err(invalid_state(&session, "review is already cancelled"));
    }
    let now = now_ms()?;
    session.status = ContractReviewStatus::Cancelled;
    session.failure = Some(ContractReviewFailure {
        code: "REVIEW_CANCELLED".to_string(),
        message: payload.reason.clone(),
        retryable: false,
        stage: session.stage,
    });
    session.revision += 1;
    session.updated_at = now;
    persist_session(transaction, &session)?;
    finalize_single_event_mutation(
        transaction,
        session.id,
        ContractReviewEventType::Cancelled,
        trace_id,
        now,
    )
}

fn save_extraction_tx(
    transaction: &Transaction<'_>,
    extraction: &DocumentExtractionRecord,
    evidence: &[EvidenceAnchor],
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    validate_extraction(extraction, evidence)?;
    let mut session = load_session_required(transaction, &extraction.review_id)?;
    require_session_revision(&session, expected_session_revision)?;
    ensure_not_terminal(&session)?;
    if extraction.source_asset_id != session.source_asset_id
        || extraction.source_asset_sha256 != session.source_asset_sha256
    {
        return Err(HostError::validation(
            "extraction source asset identity does not match review session",
        ));
    }
    if let Some(existing_id) = &session.extraction_id {
        if existing_id != &extraction.id {
            return Err(HostError::new(
                "EXTRACTION_ID_CONFLICT",
                "review already references a different extraction",
                false,
            ));
        }
        let finding_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM contract_review_findings WHERE review_id = ?1",
                [&session.id],
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        if finding_count > 0 {
            return Err(HostError::new(
                "EXTRACTION_ALREADY_REVIEWED",
                "cannot replace extraction after findings have been persisted",
                false,
            ));
        }
    }

    let mut stored_extraction = extraction.clone();
    stored_extraction.pages.clear();
    stored_extraction.blocks.clear();
    stored_extraction.tables.clear();
    transaction
        .execute(
            "INSERT INTO document_extractions
             (id, review_id, source_asset_id, source_asset_sha256, status,
              record_json, created_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                source_asset_id = excluded.source_asset_id,
                source_asset_sha256 = excluded.source_asset_sha256,
                status = excluded.status,
                record_json = excluded.record_json,
                created_at = excluded.created_at,
                completed_at = excluded.completed_at",
            params![
                extraction.id,
                extraction.review_id,
                extraction.source_asset_id,
                extraction.source_asset_sha256,
                enum_to_db(&extraction.status)?,
                to_json(&stored_extraction)?,
                extraction.created_at,
                extraction.completed_at,
            ],
        )
        .map_err(sql_error)?;

    transaction
        .execute(
            "DELETE FROM contract_review_evidence WHERE extraction_id = ?1",
            [&extraction.id],
        )
        .map_err(sql_error)?;
    transaction
        .execute(
            "DELETE FROM document_blocks WHERE extraction_id = ?1",
            [&extraction.id],
        )
        .map_err(sql_error)?;
    transaction
        .execute(
            "DELETE FROM document_tables WHERE extraction_id = ?1",
            [&extraction.id],
        )
        .map_err(sql_error)?;
    transaction
        .execute(
            "DELETE FROM document_pages WHERE extraction_id = ?1",
            [&extraction.id],
        )
        .map_err(sql_error)?;

    for page in &extraction.pages {
        insert_page(transaction, page)?;
    }
    for block in &extraction.blocks {
        insert_block(transaction, block)?;
    }
    for table in &extraction.tables {
        insert_table(transaction, table)?;
    }
    for anchor in evidence {
        insert_evidence(transaction, &session.id, anchor)?;
    }

    let now = now_ms()?;
    session.extraction_id = Some(extraction.id.clone());
    session.updated_at = now;
    session.revision += 1;
    let event_type = match extraction.status {
        DocumentExtractionStatus::Pending | DocumentExtractionStatus::Running => {
            session.status = ContractReviewStatus::Running;
            session.stage = ContractReviewStage::Extracting;
            session.failure = None;
            ContractReviewEventType::StageChanged
        }
        DocumentExtractionStatus::AwaitingOcr => {
            session.status = ContractReviewStatus::Running;
            session.stage = ContractReviewStage::AwaitingOcr;
            session.failure = None;
            ContractReviewEventType::OcrRequired
        }
        DocumentExtractionStatus::Completed => {
            session.status = ContractReviewStatus::Running;
            session.stage = ContractReviewStage::ReviewingRules;
            session.failure = None;
            ContractReviewEventType::ExtractionCompleted
        }
        DocumentExtractionStatus::Failed => {
            session.status = ContractReviewStatus::Failed;
            session.stage = extraction
                .failure
                .as_ref()
                .map(|failure| failure.stage)
                .unwrap_or(ContractReviewStage::Extracting);
            session.failure = extraction.failure.clone().or_else(|| {
                Some(ContractReviewFailure {
                    code: "DOCUMENT_EXTRACTION_FAILED".to_string(),
                    message: "document extraction failed without an engine error".to_string(),
                    retryable: true,
                    stage: ContractReviewStage::Extracting,
                })
            });
            ContractReviewEventType::Failed
        }
        DocumentExtractionStatus::Cancelled => {
            session.status = ContractReviewStatus::Cancelled;
            session.stage = ContractReviewStage::Extracting;
            session.failure = extraction.failure.clone().or_else(|| {
                Some(ContractReviewFailure {
                    code: "DOCUMENT_EXTRACTION_CANCELLED".to_string(),
                    message: "document extraction was cancelled".to_string(),
                    retryable: false,
                    stage: ContractReviewStage::Extracting,
                })
            });
            ContractReviewEventType::Cancelled
        }
    };
    persist_session(transaction, &session)?;
    finalize_single_event_mutation(transaction, session.id, event_type, trace_id, now)
}

fn replace_rule_results_tx(
    transaction: &Transaction<'_>,
    review_id: &str,
    evaluations: &[RuleEvaluationRecord],
    findings: &[ReviewFindingRecord],
    evidence: &[EvidenceAnchor],
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let mut session = load_session_required(transaction, review_id)?;
    require_session_revision(&session, expected_session_revision)?;
    ensure_not_terminal(&session)?;
    let extraction_id = session.extraction_id.clone().ok_or_else(|| {
        HostError::new(
            "EXTRACTION_REQUIRED",
            "rule results cannot be saved before extraction",
            false,
        )
    })?;

    validate_rule_results(review_id, &extraction_id, evaluations, findings, evidence)?;
    let decided_rule_findings: i64 = transaction
        .query_row(
            "SELECT COUNT(*)
             FROM contract_review_decisions decision
             JOIN contract_review_findings finding ON finding.id = decision.finding_id
             WHERE finding.review_id = ?1 AND finding.source = 'rule'",
            [review_id],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    if decided_rule_findings > 0 {
        return Err(HostError::new(
            "RULE_RESULTS_ALREADY_DECIDED",
            "cannot replace rule findings after a human decision exists",
            false,
        ));
    }

    for anchor in evidence {
        upsert_evidence(transaction, review_id, anchor)?;
    }
    for finding in findings {
        for evidence_id in &finding.evidence_ids {
            let exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM contract_review_evidence
                     WHERE id = ?1 AND review_id = ?2)",
                    params![evidence_id, review_id],
                    |row| row.get(0),
                )
                .map_err(sql_error)?;
            if !exists {
                return Err(HostError::validation(format!(
                    "finding references unknown evidence {evidence_id}"
                )));
            }
        }
    }
    transaction
        .execute(
            "DELETE FROM contract_review_rule_evaluations WHERE review_id = ?1",
            [review_id],
        )
        .map_err(sql_error)?;
    transaction
        .execute(
            "DELETE FROM contract_review_findings WHERE review_id = ?1 AND source = 'rule'",
            [review_id],
        )
        .map_err(sql_error)?;

    let now = now_ms()?;
    let mut normalized_findings = Vec::with_capacity(findings.len());
    for finding in findings {
        let mut finding = finding.clone();
        finding.revision = 1;
        finding.status = ReviewFindingStatus::Open;
        finding.decision = ReviewFindingDecision::Unreviewed;
        finding.created_at = now;
        finding.updated_at = now;
        insert_finding(transaction, &finding)?;
        normalized_findings.push(finding);
    }
    for evaluation in evaluations {
        let mut evaluation = evaluation.clone();
        if evaluation.evaluated_at <= 0 {
            evaluation.evaluated_at = now;
        }
        insert_rule_evaluation(transaction, &evaluation)?;
    }

    session.status = ContractReviewStatus::AwaitingConfirmation;
    session.stage = ContractReviewStage::AwaitingConfirmation;
    session.failure = None;
    session.updated_at = now;
    session.revision += 1;
    persist_session(transaction, &session)?;
    let event_type = if normalized_findings.is_empty() {
        ContractReviewEventType::StageChanged
    } else {
        ContractReviewEventType::FindingAdded
    };
    finalize_single_event_mutation(transaction, session.id, event_type, trace_id, now)
}

fn decide_finding_tx(
    transaction: &Transaction<'_>,
    payload: &DecideReviewFindingPayload,
    expected_finding_revision: i64,
    actor_id: &str,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    if payload.decision == ReviewFindingDecision::Unreviewed {
        return Err(HostError::validation(
            "human decision cannot be set back to unreviewed",
        ));
    }
    let mut session = load_session_required(transaction, &payload.review_id)?;
    ensure_not_terminal(&session)?;
    let mut finding = load_finding_required(transaction, &payload.finding_id)?;
    if finding.review_id != payload.review_id {
        return Err(HostError::validation(
            "finding does not belong to the requested review",
        ));
    }
    if finding.status == ReviewFindingStatus::Superseded {
        return Err(HostError::new(
            "FINDING_SUPERSEDED",
            "superseded finding cannot receive a decision",
            false,
        ));
    }
    if finding.revision != expected_finding_revision {
        return Err(HostError::conflict(format!(
            "finding revision conflict: expected {expected_finding_revision}, actual {}",
            finding.revision
        )));
    }

    let now = now_ms()?;
    let next_finding_revision = finding.revision + 1;
    finding.decision = payload.decision;
    finding.status = ReviewFindingStatus::Decided;
    finding.revision = next_finding_revision;
    finding.updated_at = now;
    let changed = transaction
        .execute(
            "UPDATE contract_review_findings
             SET status = ?1, decision = ?2, revision = ?3, record_json = ?4, updated_at = ?5
             WHERE id = ?6 AND revision = ?7",
            params![
                enum_to_db(&finding.status)?,
                enum_to_db(&finding.decision)?,
                finding.revision,
                to_json(&finding)?,
                finding.updated_at,
                finding.id,
                expected_finding_revision,
            ],
        )
        .map_err(sql_error)?;
    if changed != 1 {
        return Err(HostError::conflict(
            "finding revision changed while applying human decision",
        ));
    }

    let decision = FindingDecisionRecord {
        id: Uuid::new_v4().to_string(),
        review_id: payload.review_id.clone(),
        finding_id: payload.finding_id.clone(),
        decision: payload.decision,
        comment: payload.comment.clone(),
        actor_id: actor_id.to_string(),
        finding_revision: next_finding_revision,
        created_at: now,
    };
    insert_decision(transaction, &decision)?;

    session.updated_at = now;
    session.revision += 1;
    persist_session(transaction, &session)?;
    finalize_single_event_mutation(
        transaction,
        session.id,
        ContractReviewEventType::FindingDecided,
        trace_id,
        now,
    )
}

fn prepare_report_tx(
    transaction: &Transaction<'_>,
    payload: &GenerateReviewReportPayload,
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let mut session = load_session_required(transaction, &payload.review_id)?;
    require_session_revision(&session, expected_session_revision)?;
    if session.status == ContractReviewStatus::Cancelled {
        return Err(invalid_state(
            &session,
            "cancelled review cannot generate reports",
        ));
    }
    if session.extraction_id.is_none() {
        return Err(HostError::new(
            "EXTRACTION_REQUIRED",
            "report generation requires a persisted extraction",
            false,
        ));
    }
    let undecided: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM contract_review_findings
             WHERE review_id = ?1 AND status = 'open'",
            [&payload.review_id],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    if undecided > 0 {
        return Err(HostError::new(
            "FINDINGS_AWAITING_DECISION",
            format!("{undecided} finding(s) still require human confirmation"),
            false,
        ));
    }
    let now = now_ms()?;
    if session.status != ContractReviewStatus::Completed {
        ensure_not_terminal(&session)?;
        session.status = ContractReviewStatus::Running;
        session.stage = ContractReviewStage::GeneratingReport;
        session.failure = None;
    } else if session.stage != ContractReviewStage::Completed {
        return Err(invalid_state(
            &session,
            "completed review must remain in completed stage",
        ));
    }
    session.updated_at = now;
    session.revision += 1;
    persist_session(transaction, &session)?;
    finalize_single_event_mutation(
        transaction,
        session.id,
        ContractReviewEventType::StageChanged,
        trace_id,
        now,
    )
}

fn retry_stage_tx(
    transaction: &Transaction<'_>,
    payload: &RetryContractReviewStagePayload,
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let mut session = load_session_required(transaction, &payload.review_id)?;
    require_session_revision(&session, expected_session_revision)?;
    let retrying_failed_stage = session.status == ContractReviewStatus::Failed;
    let retrying_degraded_agent = session.status == ContractReviewStatus::AwaitingConfirmation
        && session.stage == ContractReviewStage::AwaitingConfirmation
        && payload.stage == ContractReviewStage::ReviewingAgent
        && session.failure.as_ref().is_some_and(|failure| {
            matches!(
                failure.stage,
                ContractReviewStage::ReviewingAgent | ContractReviewStage::MergingFindings
            )
        });
    if !retrying_failed_stage && !retrying_degraded_agent {
        return Err(invalid_state(
            &session,
            "only a failed stage or degraded Agent review can be retried",
        ));
    }
    if session
        .failure
        .as_ref()
        .is_some_and(|failure| !failure.retryable)
    {
        return Err(HostError::new(
            "REVIEW_FAILURE_NOT_RETRYABLE",
            "the persisted failure is not retryable",
            false,
        ));
    }
    if retrying_degraded_agent {
        let decision_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM contract_review_decisions WHERE review_id = ?1",
                [&payload.review_id],
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        if decision_count > 0 {
            return Err(HostError::new(
                "CONTRACT_AGENT_REVIEW_ALREADY_DECIDED",
                "cannot retry Agent review after a human decision exists",
                false,
            ));
        }
    }
    if payload.stage == ContractReviewStage::Completed {
        return Err(HostError::validation("completed stage cannot be retried"));
    }
    let now = now_ms()?;
    session.status = status_for_stage(payload.stage);
    session.stage = payload.stage;
    session.failure = None;
    session.updated_at = now;
    session.revision += 1;
    persist_session(transaction, &session)?;
    finalize_single_event_mutation(
        transaction,
        session.id,
        ContractReviewEventType::StageChanged,
        trace_id,
        now,
    )
}

fn save_report_and_complete_tx(
    transaction: &Transaction<'_>,
    report: &ReviewReportRecord,
    expected_session_revision: i64,
    trace_id: &str,
) -> Result<ContractReviewMutationOutcome, HostError> {
    validate_report(report)?;
    let mut session = load_session_required(transaction, &report.review_id)?;
    require_session_revision(&session, expected_session_revision)?;
    let appending_to_completed = session.status == ContractReviewStatus::Completed
        && session.stage == ContractReviewStage::Completed;
    if session.status == ContractReviewStatus::Cancelled {
        return Err(invalid_state(
            &session,
            "cancelled review cannot accept reports",
        ));
    }
    if !appending_to_completed {
        ensure_not_terminal(&session)?;
        if session.stage != ContractReviewStage::GeneratingReport {
            return Err(invalid_state(
                &session,
                "report can only be committed from generatingReport stage",
            ));
        }
    }
    if report.review_revision != session.revision {
        return Err(HostError::conflict(format!(
            "report was generated from review revision {}, current revision is {}",
            report.review_revision, session.revision
        )));
    }
    if report.source_asset_id != session.source_asset_id
        || report.source_asset_sha256 != session.source_asset_sha256
    {
        return Err(HostError::validation(
            "report source identity does not match review session",
        ));
    }
    if session.extraction_id.as_deref() != Some(report.extraction_id.as_str()) {
        return Err(HostError::validation(
            "report extraction does not match review session",
        ));
    }
    let report_asset = load_ready_asset(transaction, &report.report_asset_id)?;
    if report_asset.sha256 != report.report_asset_sha256 {
        return Err(HostError::validation(
            "report asset hash does not match Local Vault asset record",
        ));
    }
    insert_report(transaction, report)?;

    let now = now_ms()?;
    session.report_asset_id = Some(report.report_asset_id.clone());
    if !appending_to_completed {
        session.status = ContractReviewStatus::Completed;
        session.stage = ContractReviewStage::Completed;
        session.completed_at = Some(now);
    }
    session.failure = None;
    session.updated_at = now;
    session.revision += 1;
    persist_session(transaction, &session)?;
    let record = load_review_required(transaction, &session.id)?;
    let generated = append_event(
        transaction,
        ContractReviewEventType::ReportGenerated,
        &record,
        trace_id,
        now,
    )?;
    let mut emitted_events = vec![generated];
    if !appending_to_completed {
        emitted_events.push(append_event(
            transaction,
            ContractReviewEventType::Completed,
            &record,
            trace_id,
            now,
        )?);
    }
    Ok(ContractReviewMutationOutcome {
        contract_review: record,
        emitted_events,
    })
}

fn load_review(
    connection: &Connection,
    review_id: &str,
) -> Result<Option<ContractReviewRecord>, HostError> {
    let Some(session) = load_session(connection, review_id)? else {
        return Ok(None);
    };
    let extraction = load_extraction_for_review(connection, review_id)?;
    let evidence = load_json_records(
        connection,
        "SELECT record_json FROM contract_review_evidence
         WHERE review_id = ?1 ORDER BY page_index ASC, id ASC",
        review_id,
    )?;
    let findings = load_review_findings(connection, review_id)?;
    let rule_evaluations = load_json_records(
        connection,
        "SELECT record_json FROM contract_review_rule_evaluations
         WHERE review_id = ?1 ORDER BY evaluated_at ASC, id ASC",
        review_id,
    )?;
    let decisions = load_json_records(
        connection,
        "SELECT record_json FROM contract_review_decisions
         WHERE review_id = ?1 ORDER BY created_at ASC, id ASC",
        review_id,
    )?;
    let reports = load_json_records(
        connection,
        "SELECT record_json FROM contract_review_reports
         WHERE review_id = ?1 ORDER BY generated_at ASC, id ASC",
        review_id,
    )?;
    Ok(Some(ContractReviewRecord {
        session,
        extraction,
        evidence,
        findings,
        rule_evaluations,
        decisions,
        reports,
    }))
}

fn load_review_required(
    connection: &Connection,
    review_id: &str,
) -> Result<ContractReviewRecord, HostError> {
    load_review(connection, review_id)?.ok_or_else(|| review_not_found(review_id))
}

fn load_session(
    connection: &Connection,
    review_id: &str,
) -> Result<Option<ContractReviewSessionRecord>, HostError> {
    connection
        .query_row(
            "SELECT id, workspace_id, source_asset_id, source_asset_sha256,
                    source_file_name, status, stage, extraction_id, report_asset_id,
                    revision, created_at, updated_at, completed_at, failure_json
             FROM contract_review_sessions WHERE id = ?1",
            [review_id],
            session_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn load_session_required(
    connection: &Connection,
    review_id: &str,
) -> Result<ContractReviewSessionRecord, HostError> {
    load_session(connection, review_id)?.ok_or_else(|| review_not_found(review_id))
}

fn session_from_row(row: &Row<'_>) -> rusqlite::Result<ContractReviewSessionRecord> {
    let status: String = row.get(5)?;
    let stage: String = row.get(6)?;
    let failure_json: Option<String> = row.get(13)?;
    Ok(ContractReviewSessionRecord {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        source_asset_id: row.get(2)?,
        source_asset_sha256: row.get(3)?,
        source_file_name: row.get(4)?,
        status: enum_from_sql(&status, 5)?,
        stage: enum_from_sql(&stage, 6)?,
        extraction_id: row.get(7)?,
        report_asset_id: row.get(8)?,
        revision: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        completed_at: row.get(12)?,
        failure: failure_json
            .map(|json| json_from_sql(&json, 13))
            .transpose()?,
    })
}

fn load_extraction_for_review(
    connection: &Connection,
    review_id: &str,
) -> Result<Option<DocumentExtractionRecord>, HostError> {
    let stored_json: Option<String> = connection
        .query_row(
            "SELECT record_json FROM document_extractions WHERE review_id = ?1",
            [review_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    let Some(stored_json) = stored_json else {
        return Ok(None);
    };
    let mut extraction: DocumentExtractionRecord = from_json(&stored_json)?;
    extraction.pages = load_json_records(
        connection,
        "SELECT record_json FROM document_pages
         WHERE extraction_id = ?1 ORDER BY page_index ASC, id ASC",
        &extraction.id,
    )?;
    extraction.blocks = load_json_records(
        connection,
        "SELECT record_json FROM document_blocks
         WHERE extraction_id = ?1 ORDER BY page_index ASC, order_index ASC, id ASC",
        &extraction.id,
    )?;
    extraction.tables = load_json_records(
        connection,
        "SELECT record_json FROM document_tables
         WHERE extraction_id = ?1 ORDER BY page_index ASC, order_index ASC, id ASC",
        &extraction.id,
    )?;
    Ok(Some(extraction))
}

fn load_review_findings(
    connection: &Connection,
    review_id: &str,
) -> Result<Vec<ReviewFindingRecord>, HostError> {
    load_json_records(
        connection,
        "SELECT record_json FROM contract_review_findings
         WHERE review_id = ?1 ORDER BY created_at ASC, id ASC",
        review_id,
    )
}

fn load_json_records<T: DeserializeOwned>(
    connection: &Connection,
    sql: &str,
    id: &str,
) -> Result<Vec<T>, HostError> {
    let json_values = {
        let mut statement = connection.prepare(sql).map_err(sql_error)?;
        let rows = statement
            .query_map([id], |row| row.get::<_, String>(0))
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        rows
    };
    json_values
        .into_iter()
        .map(|json| from_json(&json))
        .collect()
}

fn load_finding_required(
    connection: &Connection,
    finding_id: &str,
) -> Result<ReviewFindingRecord, HostError> {
    let json: Option<String> = connection
        .query_row(
            "SELECT record_json FROM contract_review_findings WHERE id = ?1",
            [finding_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    json.map(|value| from_json(&value))
        .transpose()?
        .ok_or_else(|| {
            HostError::new(
                "REVIEW_FINDING_NOT_FOUND",
                format!("review finding {finding_id} was not found"),
                false,
            )
        })
}

fn insert_session(
    transaction: &Transaction<'_>,
    session: &ContractReviewSessionRecord,
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO contract_review_sessions
             (id, workspace_id, source_asset_id, source_asset_sha256, source_file_name,
              status, stage, extraction_id, report_asset_id, revision, created_at,
              updated_at, completed_at, failure_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                session.id,
                session.workspace_id,
                session.source_asset_id,
                session.source_asset_sha256,
                session.source_file_name,
                enum_to_db(&session.status)?,
                enum_to_db(&session.stage)?,
                session.extraction_id,
                session.report_asset_id,
                session.revision,
                session.created_at,
                session.updated_at,
                session.completed_at,
                session.failure.as_ref().map(to_json).transpose()?,
            ],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn persist_session(
    transaction: &Transaction<'_>,
    session: &ContractReviewSessionRecord,
) -> Result<(), HostError> {
    let changed = transaction
        .execute(
            "UPDATE contract_review_sessions
             SET workspace_id = ?1, source_asset_id = ?2, source_asset_sha256 = ?3,
                 source_file_name = ?4, status = ?5, stage = ?6, extraction_id = ?7,
                 report_asset_id = ?8, revision = ?9, created_at = ?10, updated_at = ?11,
                 completed_at = ?12, failure_json = ?13
             WHERE id = ?14",
            params![
                session.workspace_id,
                session.source_asset_id,
                session.source_asset_sha256,
                session.source_file_name,
                enum_to_db(&session.status)?,
                enum_to_db(&session.stage)?,
                session.extraction_id,
                session.report_asset_id,
                session.revision,
                session.created_at,
                session.updated_at,
                session.completed_at,
                session.failure.as_ref().map(to_json).transpose()?,
                session.id,
            ],
        )
        .map_err(sql_error)?;
    if changed != 1 {
        return Err(HostError::internal(format!(
            "contract review {} disappeared while updating",
            session.id
        )));
    }
    Ok(())
}

fn insert_page(transaction: &Transaction<'_>, page: &DocumentPageRecord) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO document_pages (id, extraction_id, page_index, record_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![page.id, page.extraction_id, page.page_index, to_json(page)?],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn insert_block(
    transaction: &Transaction<'_>,
    block: &DocumentBlockRecord,
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO document_blocks
             (id, extraction_id, page_id, page_index, order_index, record_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                block.id,
                block.extraction_id,
                block.page_id,
                block.page_index,
                block.order_index,
                to_json(block)?,
            ],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn insert_table(
    transaction: &Transaction<'_>,
    table: &DocumentTableRecord,
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO document_tables
             (id, extraction_id, page_id, page_index, order_index, record_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                table.id,
                table.extraction_id,
                table.page_id,
                table.page_index,
                table.order_index,
                to_json(table)?,
            ],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn insert_evidence(
    transaction: &Transaction<'_>,
    review_id: &str,
    evidence: &EvidenceAnchor,
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO contract_review_evidence
             (id, review_id, extraction_id, source_asset_id, page_index, record_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                evidence.id,
                review_id,
                evidence.extraction_id,
                evidence.source_asset_id,
                evidence.page_index,
                to_json(evidence)?,
            ],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn upsert_evidence(
    transaction: &Transaction<'_>,
    review_id: &str,
    evidence: &EvidenceAnchor,
) -> Result<(), HostError> {
    let owner: Option<String> = transaction
        .query_row(
            "SELECT review_id FROM contract_review_evidence WHERE id = ?1",
            [&evidence.id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    if owner.as_deref().is_some_and(|owner| owner != review_id) {
        return Err(HostError::new(
            "EVIDENCE_ID_CONFLICT",
            "evidence ID already belongs to another review",
            false,
        ));
    }
    transaction
        .execute(
            "INSERT INTO contract_review_evidence
             (id, review_id, extraction_id, source_asset_id, page_index, record_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                extraction_id = excluded.extraction_id,
                source_asset_id = excluded.source_asset_id,
                page_index = excluded.page_index,
                record_json = excluded.record_json",
            params![
                evidence.id,
                review_id,
                evidence.extraction_id,
                evidence.source_asset_id,
                evidence.page_index,
                to_json(evidence)?,
            ],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn insert_finding(
    transaction: &Transaction<'_>,
    finding: &ReviewFindingRecord,
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO contract_review_findings
             (id, review_id, source, status, decision, revision, record_json,
              created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                finding.id,
                finding.review_id,
                enum_to_db(&finding.source)?,
                enum_to_db(&finding.status)?,
                enum_to_db(&finding.decision)?,
                finding.revision,
                to_json(finding)?,
                finding.created_at,
                finding.updated_at,
            ],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn insert_rule_evaluation(
    transaction: &Transaction<'_>,
    evaluation: &RuleEvaluationRecord,
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO contract_review_rule_evaluations
             (id, review_id, rule_id, record_json, evaluated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                evaluation.id,
                evaluation.review_id,
                evaluation.rule_id,
                to_json(evaluation)?,
                evaluation.evaluated_at,
            ],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn insert_decision(
    transaction: &Transaction<'_>,
    decision: &FindingDecisionRecord,
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO contract_review_decisions
             (id, review_id, finding_id, finding_revision, record_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                decision.id,
                decision.review_id,
                decision.finding_id,
                decision.finding_revision,
                to_json(decision)?,
                decision.created_at,
            ],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn insert_report(
    transaction: &Transaction<'_>,
    report: &ReviewReportRecord,
) -> Result<(), HostError> {
    transaction
        .execute(
            "INSERT INTO contract_review_reports
             (id, review_id, report_asset_id, format, record_json, generated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                report.id,
                report.review_id,
                report.report_asset_id,
                enum_to_db(&report.format)?,
                to_json(report)?,
                report.generated_at,
            ],
        )
        .map(|_| ())
        .map_err(sql_error)
}

fn append_event(
    transaction: &Transaction<'_>,
    event_type: ContractReviewEventType,
    contract_review: &ContractReviewRecord,
    trace_id: &str,
    occurred_at: i64,
) -> Result<ContractReviewDomainEvent, HostError> {
    let event_id = Uuid::new_v4().to_string();
    transaction
        .execute(
            "INSERT INTO contract_review_events
             (event_id, event_type, aggregate_id, revision, occurred_at, trace_id, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event_id,
                event_type.as_wire_str(),
                contract_review.session.id,
                contract_review.session.revision,
                occurred_at,
                trace_id,
                to_json(contract_review)?,
            ],
        )
        .map_err(sql_error)?;
    let sequence = transaction.last_insert_rowid();
    Ok(ContractReviewDomainEvent {
        sequence,
        event_id,
        event_type,
        aggregate_id: contract_review.session.id.clone(),
        revision: contract_review.session.revision,
        occurred_at,
        trace_id: trace_id.to_string(),
        contract_review: contract_review.clone(),
    })
}

fn finalize_single_event_mutation(
    transaction: &Transaction<'_>,
    review_id: String,
    event_type: ContractReviewEventType,
    trace_id: &str,
    occurred_at: i64,
) -> Result<ContractReviewMutationOutcome, HostError> {
    let record = load_review_required(transaction, &review_id)?;
    let event = append_event(transaction, event_type, &record, trace_id, occurred_at)?;
    Ok(ContractReviewMutationOutcome {
        contract_review: record,
        emitted_events: vec![event],
    })
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<ContractReviewDomainEvent> {
    let event_type: String = row.get(2)?;
    let payload_json: String = row.get(7)?;
    Ok(ContractReviewDomainEvent {
        sequence: row.get(0)?,
        event_id: row.get(1)?,
        event_type: contract_review_event_type_from_wire(&event_type, 2)?,
        aggregate_id: row.get(3)?,
        revision: row.get(4)?,
        occurred_at: row.get(5)?,
        trace_id: row.get(6)?,
        contract_review: json_from_sql(&payload_json, 7)?,
    })
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

#[derive(Debug)]
enum NormalizedCommand {
    Create {
        meta: CommandMeta,
        payload: CreateContractReviewPayload,
    },
    Start {
        meta: CommandMeta,
        payload: StartContractReviewPayload,
    },
    Cancel {
        meta: CommandMeta,
        payload: CancelContractReviewPayload,
    },
    DecideFinding {
        meta: CommandMeta,
        payload: DecideReviewFindingPayload,
    },
    GenerateReport {
        meta: CommandMeta,
        payload: GenerateReviewReportPayload,
    },
    RetryStage {
        meta: CommandMeta,
        payload: RetryContractReviewStagePayload,
    },
}

impl NormalizedCommand {
    fn meta(&self) -> &CommandMeta {
        match self {
            Self::Create { meta, .. }
            | Self::Start { meta, .. }
            | Self::Cancel { meta, .. }
            | Self::DecideFinding { meta, .. }
            | Self::GenerateReport { meta, .. }
            | Self::RetryStage { meta, .. } => meta,
        }
    }

    fn command_type(&self) -> &'static str {
        match self {
            Self::Create { .. } => "contractReview.create",
            Self::Start { .. } => "contractReview.start",
            Self::Cancel { .. } => "contractReview.cancel",
            Self::DecideFinding { .. } => "contractReview.decideFinding",
            Self::GenerateReport { .. } => "contractReview.generateReport",
            Self::RetryStage { .. } => "contractReview.retryStage",
        }
    }
}

fn normalize_command(
    command: ContractReviewCommandEnvelope,
) -> Result<NormalizedCommand, HostError> {
    match command {
        ContractReviewCommandEnvelope::Create {
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
                    "contractReview.create rejects expectedRevision",
                ));
            }
            Ok(NormalizedCommand::Create {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    context,
                    idempotency_key,
                    None,
                    deadline_at,
                )?,
                payload: CreateContractReviewPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    source_asset_id: normalize_uuid("sourceAssetId", payload.source_asset_id)?,
                },
            })
        }
        ContractReviewCommandEnvelope::Start {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => Ok(NormalizedCommand::Start {
            meta: normalize_meta(
                command_id,
                protocol_version,
                context,
                idempotency_key,
                Some(validate_expected_revision(expected_revision)?),
                deadline_at,
            )?,
            payload: StartContractReviewPayload {
                review_id: normalize_uuid("reviewId", payload.review_id)?,
            },
        }),
        ContractReviewCommandEnvelope::Cancel {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => Ok(NormalizedCommand::Cancel {
            meta: normalize_meta(
                command_id,
                protocol_version,
                context,
                idempotency_key,
                Some(validate_expected_revision(expected_revision)?),
                deadline_at,
            )?,
            payload: normalize_cancel_payload(payload)?,
        }),
        ContractReviewCommandEnvelope::DecideFinding {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => Ok(NormalizedCommand::DecideFinding {
            meta: normalize_meta(
                command_id,
                protocol_version,
                context,
                idempotency_key,
                Some(validate_expected_revision(expected_revision)?),
                deadline_at,
            )?,
            payload: normalize_decision_payload(payload)?,
        }),
        ContractReviewCommandEnvelope::GenerateReport {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => Ok(NormalizedCommand::GenerateReport {
            meta: normalize_meta(
                command_id,
                protocol_version,
                context,
                idempotency_key,
                Some(validate_expected_revision(expected_revision)?),
                deadline_at,
            )?,
            payload: GenerateReviewReportPayload {
                review_id: normalize_uuid("reviewId", payload.review_id)?,
                format: payload.format,
            },
        }),
        ContractReviewCommandEnvelope::RetryStage {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => Ok(NormalizedCommand::RetryStage {
            meta: normalize_meta(
                command_id,
                protocol_version,
                context,
                idempotency_key,
                Some(validate_expected_revision(expected_revision)?),
                deadline_at,
            )?,
            payload: RetryContractReviewStagePayload {
                review_id: normalize_uuid("reviewId", payload.review_id)?,
                stage: payload.stage,
            },
        }),
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
    let command_id = normalize_uuid("commandId", command_id)?;
    let idempotency_key = normalize_required("idempotencyKey", &idempotency_key, 512)?;
    if protocol_version != CONTRACT_REVIEW_PROTOCOL_VERSION {
        return Err(HostError::new(
            "PROTOCOL_VERSION_UNSUPPORTED",
            format!(
                "contract review requires protocolVersion {CONTRACT_REVIEW_PROTOCOL_VERSION}, got {protocol_version}"
            ),
            false,
        ));
    }
    Ok(CommandMeta {
        command_id,
        protocol_version,
        context: normalize_context(context)?,
        idempotency_key,
        expected_revision,
        deadline_at,
    })
}

fn normalize_context(context: OperationContext) -> Result<OperationContext, HostError> {
    Ok(OperationContext {
        actor_id: normalize_required("context.actorId", &context.actor_id, 256)?,
        account_id: context
            .account_id
            .map(|value| normalize_required("context.accountId", &value, 256))
            .transpose()?,
        project_id: context
            .project_id
            .map(|value| normalize_uuid("context.projectId", value))
            .transpose()?,
        window_id: normalize_required("context.windowId", &context.window_id, 256)?,
        trace_id: normalize_required("context.traceId", &context.trace_id, 256)?,
    })
}

fn normalize_cancel_payload(
    payload: CancelContractReviewPayload,
) -> Result<CancelContractReviewPayload, HostError> {
    Ok(CancelContractReviewPayload {
        review_id: normalize_uuid("reviewId", payload.review_id)?,
        reason: normalize_required("reason", &payload.reason, MAX_REASON_CHARS)?,
    })
}

fn normalize_decision_payload(
    payload: DecideReviewFindingPayload,
) -> Result<DecideReviewFindingPayload, HostError> {
    let comment = payload.comment.trim().to_string();
    if comment.chars().count() > MAX_COMMENT_CHARS {
        return Err(HostError::validation(format!(
            "comment exceeds {MAX_COMMENT_CHARS} characters"
        )));
    }
    Ok(DecideReviewFindingPayload {
        review_id: normalize_uuid("reviewId", payload.review_id)?,
        finding_id: normalize_uuid("findingId", payload.finding_id)?,
        decision: payload.decision,
        comment,
    })
}

fn command_fingerprint(command: &NormalizedCommand) -> Result<String, HostError> {
    let meta = command.meta();
    let payload = match command {
        NormalizedCommand::Create { payload, .. } => serde_json::to_value(payload),
        NormalizedCommand::Start { payload, .. } => serde_json::to_value(payload),
        NormalizedCommand::Cancel { payload, .. } => serde_json::to_value(payload),
        NormalizedCommand::DecideFinding { payload, .. } => serde_json::to_value(payload),
        NormalizedCommand::GenerateReport { payload, .. } => serde_json::to_value(payload),
        NormalizedCommand::RetryStage { payload, .. } => serde_json::to_value(payload),
    }
    .map_err(json_error)?;
    let value = serde_json::json!({
        "commandType": command.command_type(),
        "protocolVersion": meta.protocol_version,
        "actorId": &meta.context.actor_id,
        "accountId": &meta.context.account_id,
        "projectId": &meta.context.project_id,
        "payload": payload,
        "expectedRevision": meta.expected_revision,
    });
    let encoded = serde_json::to_vec(&value).map_err(json_error)?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
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
) -> Result<Option<ContractReviewCommandResponse>, HostError> {
    let by_key = load_receipt(connection, "idempotency_key", idempotency_key)?;
    let by_command = load_receipt(connection, "command_id", command_id)?;
    if by_key
        .as_ref()
        .is_some_and(|receipt| receipt.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "IDEMPOTENCY_KEY_REUSED",
            "idempotencyKey reused for a different contract review request",
            false,
        ));
    }
    if by_command
        .as_ref()
        .is_some_and(|receipt| receipt.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "COMMAND_ID_REUSED",
            "commandId reused for a different contract review request",
            false,
        ));
    }
    if let (Some(left), Some(right)) = (&by_key, &by_command) {
        if left.command_id != right.command_id || left.idempotency_key != right.idempotency_key {
            return Err(HostError::new(
                "COMMAND_IDENTITY_COLLISION",
                "command identities resolve to different contract review requests",
                false,
            ));
        }
    }
    by_key
        .or(by_command)
        .map(|receipt| {
            let mut response: ContractReviewCommandResponse = from_json(&receipt.response_json)?;
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
                 FROM contract_review_command_receipts WHERE {column} = ?1"
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

#[derive(Debug)]
struct AssetIdentity {
    original_name: String,
    sha256: String,
}

fn ensure_workspace_exists(connection: &Connection, workspace_id: &str) -> Result<(), HostError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM business_workspaces WHERE id = ?1)",
            [workspace_id],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    if exists {
        Ok(())
    } else {
        Err(HostError::new(
            "BUSINESS_WORKSPACE_NOT_FOUND",
            format!("business workspace {workspace_id} was not found"),
            false,
        ))
    }
}

fn load_ready_document_asset(
    connection: &Connection,
    asset_id: &str,
) -> Result<AssetIdentity, HostError> {
    let row: Option<(String, String, String, String)> = connection
        .query_row(
            "SELECT original_name, sha256, kind, status FROM assets WHERE id = ?1",
            [asset_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(sql_error)?;
    let Some((original_name, sha256, kind, status)) = row else {
        return Err(HostError::new(
            "ASSET_NOT_FOUND",
            format!("Local Vault asset {asset_id} was not found"),
            false,
        ));
    };
    if status != "ready" {
        return Err(HostError::new(
            "ASSET_NOT_READY",
            "source contract asset is not ready in Local Vault",
            false,
        ));
    }
    if kind != "document" {
        return Err(HostError::validation(
            "contract review source asset must be a document",
        ));
    }
    Ok(AssetIdentity {
        original_name,
        sha256,
    })
}

fn load_ready_asset(connection: &Connection, asset_id: &str) -> Result<AssetIdentity, HostError> {
    let row: Option<(String, String, String)> = connection
        .query_row(
            "SELECT original_name, sha256, status FROM assets WHERE id = ?1",
            [asset_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(sql_error)?;
    let Some((original_name, sha256, status)) = row else {
        return Err(HostError::new(
            "ASSET_NOT_FOUND",
            format!("Local Vault asset {asset_id} was not found"),
            false,
        ));
    };
    if status != "ready" {
        return Err(HostError::new(
            "ASSET_NOT_READY",
            "artifact asset is not ready in Local Vault",
            false,
        ));
    }
    Ok(AssetIdentity {
        original_name,
        sha256,
    })
}

fn validate_extraction(
    extraction: &DocumentExtractionRecord,
    evidence: &[EvidenceAnchor],
) -> Result<(), HostError> {
    normalize_uuid("extraction.id", extraction.id.clone())?;
    normalize_uuid("extraction.reviewId", extraction.review_id.clone())?;
    normalize_uuid(
        "extraction.sourceAssetId",
        extraction.source_asset_id.clone(),
    )?;
    validate_sha256(
        "extraction.sourceAssetSha256",
        &extraction.source_asset_sha256,
    )?;
    if extraction.page_count < 0 {
        return Err(HostError::validation(
            "extraction.pageCount cannot be negative",
        ));
    }
    if extraction.page_count != extraction.pages.len() as i64 {
        return Err(HostError::validation(format!(
            "extraction.pageCount {} does not match {} persisted pages",
            extraction.page_count,
            extraction.pages.len()
        )));
    }
    if let Some(hash) = &extraction.content_sha256 {
        validate_sha256("extraction.contentSha256", hash)?;
    }
    if let Some(asset_id) = &extraction.snapshot_asset_id {
        normalize_uuid("extraction.snapshotAssetId", asset_id.clone())?;
    }
    normalize_required("extraction.parser.name", &extraction.parser.name, 256)?;
    normalize_required("extraction.parser.version", &extraction.parser.version, 256)?;
    normalize_required("extraction.parser.mode", &extraction.parser.mode, 256)?;
    if let Some(ocr) = &extraction.ocr {
        normalize_required("extraction.ocr.engine", &ocr.engine, 256)?;
        normalize_required("extraction.ocr.version", &ocr.version, 256)?;
        normalize_required("extraction.ocr.language", &ocr.language, 256)?;
    }
    if extraction.created_at < 0 || extraction.completed_at.is_some_and(|value| value < 0) {
        return Err(HostError::validation(
            "extraction timestamps cannot be negative",
        ));
    }
    if extraction.status == DocumentExtractionStatus::Failed && extraction.failure.is_none() {
        return Err(HostError::validation(
            "failed extraction must include failure details",
        ));
    }
    if let Some(failure) = &extraction.failure {
        validate_failure(failure)?;
    }

    let mut page_ids = HashSet::new();
    let mut page_indexes = HashSet::new();
    let mut page_index_by_id = HashMap::new();
    for page in &extraction.pages {
        normalize_uuid("page.id", page.id.clone())?;
        if page.extraction_id != extraction.id {
            return Err(HostError::validation(
                "page extractionId does not match extraction",
            ));
        }
        if page.page_index < 0 {
            return Err(HostError::validation("pageIndex cannot be negative"));
        }
        validate_sha256("page.textSha256", &page.text_sha256)?;
        if let Some(preview_asset_id) = &page.preview_asset_id {
            normalize_uuid("page.previewAssetId", preview_asset_id.clone())?;
        }
        if !page_ids.insert(page.id.clone()) || !page_indexes.insert(page.page_index) {
            return Err(HostError::validation(
                "document pages contain duplicate IDs or page indexes",
            ));
        }
        if page
            .width
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
            || page
                .height
                .is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(HostError::validation(
                "page width and height must be finite positive values",
            ));
        }
        page_index_by_id.insert(page.id.clone(), page.page_index);
    }

    let mut block_ids = HashSet::new();
    let mut block_positions = HashSet::new();
    for block in &extraction.blocks {
        normalize_uuid("block.id", block.id.clone())?;
        if block.extraction_id != extraction.id {
            return Err(HostError::validation(
                "block extractionId does not match extraction",
            ));
        }
        let Some(expected_page_index) = page_index_by_id.get(&block.page_id) else {
            return Err(HostError::validation(
                "block references a page outside the extraction",
            ));
        };
        if expected_page_index != &block.page_index {
            return Err(HostError::validation(
                "block pageIndex does not match its page",
            ));
        }
        if block.order_index < 0 || block.char_start < 0 || block.char_end < block.char_start {
            return Err(HostError::validation(
                "block ordering and character offsets are invalid",
            ));
        }
        if let Some(bbox) = &block.bbox {
            validate_bbox(bbox.x, bbox.y, bbox.width, bbox.height)?;
        }
        if !block_ids.insert(block.id.clone())
            || !block_positions.insert((block.page_index, block.order_index))
        {
            return Err(HostError::validation(
                "document blocks contain duplicate IDs or positions",
            ));
        }
    }

    let mut table_ids = HashSet::new();
    let mut table_positions = HashSet::new();
    for table in &extraction.tables {
        normalize_uuid("table.id", table.id.clone())?;
        if table.extraction_id != extraction.id {
            return Err(HostError::validation(
                "table extractionId does not match extraction",
            ));
        }
        let Some(expected_page_index) = page_index_by_id.get(&table.page_id) else {
            return Err(HostError::validation(
                "table references a page outside the extraction",
            ));
        };
        if expected_page_index != &table.page_index || table.order_index < 0 {
            return Err(HostError::validation(
                "table page or ordering metadata is invalid",
            ));
        }
        if let Some(bbox) = &table.bbox {
            validate_bbox(bbox.x, bbox.y, bbox.width, bbox.height)?;
        }
        if !table_ids.insert(table.id.clone())
            || !table_positions.insert((table.page_index, table.order_index))
        {
            return Err(HostError::validation(
                "document tables contain duplicate IDs or positions",
            ));
        }
    }

    let mut evidence_ids = HashSet::new();
    for anchor in evidence {
        validate_evidence(
            anchor,
            &extraction.id,
            &extraction.source_asset_id,
            &page_indexes,
            &block_ids,
        )?;
        if !evidence_ids.insert(anchor.id.clone()) {
            return Err(HostError::validation("evidence contains duplicate IDs"));
        }
    }
    Ok(())
}

fn validate_rule_results(
    review_id: &str,
    extraction_id: &str,
    evaluations: &[RuleEvaluationRecord],
    findings: &[ReviewFindingRecord],
    evidence: &[EvidenceAnchor],
) -> Result<(), HostError> {
    let mut evidence_ids = HashSet::new();
    for anchor in evidence {
        normalize_uuid("evidence.id", anchor.id.clone())?;
        if anchor.extraction_id != extraction_id {
            return Err(HostError::validation(
                "rule evidence extractionId does not match review extraction",
            ));
        }
        if !evidence_ids.insert(anchor.id.clone()) {
            return Err(HostError::validation("duplicate evidence ID"));
        }
    }
    let mut finding_ids = HashSet::new();
    for finding in findings {
        normalize_uuid("finding.id", finding.id.clone())?;
        if finding.review_id != review_id {
            return Err(HostError::validation(
                "finding reviewId does not match target review",
            ));
        }
        if finding.source != ReviewFindingSource::Rule {
            return Err(HostError::validation(
                "rule result replacement accepts only rule findings",
            ));
        }
        if finding
            .rule_id
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
            || finding
                .rule_version
                .as_ref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err(HostError::validation(
                "rule finding requires ruleId and ruleVersion",
            ));
        }
        normalize_required("finding.category", &finding.category, 256)?;
        normalize_required("finding.title", &finding.title, 1_000)?;
        normalize_required("finding.description", &finding.description, 16_000)?;
        normalize_required("finding.recommendation", &finding.recommendation, 16_000)?;
        if finding.evidence_ids.is_empty()
            && finding
                .missing_evidence_reason
                .as_ref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err(HostError::validation(
                "finding requires evidenceIds or missingEvidenceReason",
            ));
        }
        for evidence_id in &finding.evidence_ids {
            normalize_uuid("finding.evidenceId", evidence_id.clone())?;
        }
        if !finding_ids.insert(finding.id.clone()) {
            return Err(HostError::validation("duplicate finding ID"));
        }
    }

    let mut evaluation_ids = HashSet::new();
    for evaluation in evaluations {
        normalize_uuid("evaluation.id", evaluation.id.clone())?;
        if evaluation.review_id != review_id {
            return Err(HostError::validation(
                "rule evaluation reviewId does not match target review",
            ));
        }
        normalize_required("evaluation.ruleId", &evaluation.rule_id, 256)?;
        normalize_required("evaluation.ruleVersion", &evaluation.rule_version, 256)?;
        for finding_id in &evaluation.finding_ids {
            if !finding_ids.contains(finding_id) {
                return Err(HostError::validation(
                    "rule evaluation references a finding outside the replacement set",
                ));
            }
        }
        if !evaluation_ids.insert(evaluation.id.clone()) {
            return Err(HostError::validation("duplicate rule evaluation ID"));
        }
    }
    Ok(())
}

fn validate_agent_finding(finding: &ReviewFindingRecord, review_id: &str) -> Result<(), HostError> {
    normalize_uuid("finding.id", finding.id.clone())?;
    if finding.review_id != review_id {
        return Err(HostError::validation(
            "Agent finding reviewId does not match target review",
        ));
    }
    if finding.source != ReviewFindingSource::Agent {
        return Err(HostError::validation(
            "Agent result replacement accepts only Agent findings",
        ));
    }
    if finding.rule_id.is_some() || finding.rule_version.is_some() {
        return Err(HostError::validation(
            "Agent finding cannot claim deterministic rule provenance",
        ));
    }
    let agent_run_id = finding
        .agent_run_id
        .as_deref()
        .ok_or_else(|| HostError::validation("Agent finding requires agentRunId provenance"))?;
    normalize_required("finding.agentRunId", agent_run_id, 256)?;
    normalize_required("finding.category", &finding.category, 256)?;
    normalize_required("finding.title", &finding.title, 1_000)?;
    normalize_required("finding.description", &finding.description, 16_000)?;
    normalize_required("finding.recommendation", &finding.recommendation, 16_000)?;
    if finding.evidence_ids.is_empty()
        && finding
            .missing_evidence_reason
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err(HostError::validation(
            "Agent finding requires evidenceIds or missingEvidenceReason",
        ));
    }
    if let Some(reason) = finding.missing_evidence_reason.as_deref() {
        normalize_required("finding.missingEvidenceReason", reason, MAX_REASON_CHARS)?;
    }
    for evidence_id in &finding.evidence_ids {
        normalize_uuid("finding.evidenceId", evidence_id.clone())?;
    }
    Ok(())
}

fn validate_evidence(
    evidence: &EvidenceAnchor,
    extraction_id: &str,
    source_asset_id: &str,
    page_indexes: &HashSet<i64>,
    block_ids: &HashSet<String>,
) -> Result<(), HostError> {
    normalize_uuid("evidence.id", evidence.id.clone())?;
    if evidence.extraction_id != extraction_id || evidence.source_asset_id != source_asset_id {
        return Err(HostError::validation(
            "evidence source identity does not match extraction",
        ));
    }
    if !page_indexes.contains(&evidence.page_index) {
        return Err(HostError::validation(
            "evidence references a page outside the extraction",
        ));
    }
    if evidence
        .block_id
        .as_ref()
        .is_some_and(|block_id| !block_ids.contains(block_id))
    {
        return Err(HostError::validation(
            "evidence references a block outside the extraction",
        ));
    }
    match (evidence.char_start, evidence.char_end) {
        (Some(start), Some(end)) if start >= 0 && end >= start => {}
        (None, None) => {}
        _ => {
            return Err(HostError::validation(
                "evidence character offsets must be both absent or a valid range",
            ));
        }
    }
    if let Some(bbox) = &evidence.bbox {
        validate_bbox(bbox.x, bbox.y, bbox.width, bbox.height)?;
    }
    validate_sha256("evidence.quotedTextSha256", &evidence.quoted_text_sha256)?;
    Ok(())
}

fn validate_report(report: &ReviewReportRecord) -> Result<(), HostError> {
    normalize_uuid("report.id", report.id.clone())?;
    normalize_uuid("report.reviewId", report.review_id.clone())?;
    normalize_uuid("report.sourceAssetId", report.source_asset_id.clone())?;
    normalize_uuid("report.extractionId", report.extraction_id.clone())?;
    normalize_uuid("report.reportAssetId", report.report_asset_id.clone())?;
    validate_positive_revision(report.review_revision)?;
    validate_sha256("report.sourceAssetSha256", &report.source_asset_sha256)?;
    validate_sha256("report.reportAssetSha256", &report.report_asset_sha256)?;
    normalize_required("report.ruleSetVersion", &report.rule_set_version, 256)?;
    for agent_run_id in &report.agent_run_ids {
        normalize_required("report.agentRunId", agent_run_id, 256)?;
    }
    if report.generated_at < 0 {
        return Err(HostError::validation(
            "report.generatedAt cannot be negative",
        ));
    }
    Ok(())
}

fn validate_failure(failure: &ContractReviewFailure) -> Result<(), HostError> {
    normalize_required("failure.code", &failure.code, 256)?;
    normalize_required("failure.message", &failure.message, 16_000)?;
    Ok(())
}

fn validate_bbox(x: f64, y: f64, width: f64, height: f64) -> Result<(), HostError> {
    if !x.is_finite()
        || !y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || x < 0.0
        || y < 0.0
        || width <= 0.0
        || height <= 0.0
    {
        return Err(HostError::validation(
            "evidence bounding box must contain finite non-negative coordinates and positive size",
        ));
    }
    Ok(())
}

fn require_session_revision(
    session: &ContractReviewSessionRecord,
    expected_revision: i64,
) -> Result<(), HostError> {
    validate_positive_revision(expected_revision)?;
    if session.revision != expected_revision {
        return Err(HostError::conflict(format!(
            "contract review revision conflict: expected {expected_revision}, actual {}",
            session.revision
        )));
    }
    Ok(())
}

fn validate_expected_revision(value: Option<i64>) -> Result<i64, HostError> {
    value
        .ok_or_else(|| HostError::validation("expectedRevision is required"))
        .and_then(validate_positive_revision)
}

fn required_expected_revision(value: Option<i64>) -> Result<i64, HostError> {
    value.ok_or_else(|| HostError::internal("normalized command lost expectedRevision"))
}

fn validate_positive_revision(value: i64) -> Result<i64, HostError> {
    if value < 1 {
        Err(HostError::validation(
            "expected revision must be at least 1",
        ))
    } else {
        Ok(value)
    }
}

fn ensure_not_terminal(session: &ContractReviewSessionRecord) -> Result<(), HostError> {
    if matches!(
        session.status,
        ContractReviewStatus::Completed | ContractReviewStatus::Cancelled
    ) {
        Err(invalid_state(session, "terminal review cannot be modified"))
    } else {
        Ok(())
    }
}

fn status_for_stage(stage: ContractReviewStage) -> ContractReviewStatus {
    match stage {
        ContractReviewStage::Created => ContractReviewStatus::Draft,
        ContractReviewStage::AwaitingConfirmation => ContractReviewStatus::AwaitingConfirmation,
        ContractReviewStage::Completed => ContractReviewStatus::Completed,
        ContractReviewStage::Extracting
        | ContractReviewStage::AwaitingOcr
        | ContractReviewStage::ReviewingRules
        | ContractReviewStage::ReviewingAgent
        | ContractReviewStage::MergingFindings
        | ContractReviewStage::GeneratingReport => ContractReviewStatus::Running,
    }
}

fn invalid_state(session: &ContractReviewSessionRecord, message: &str) -> HostError {
    HostError::new(
        "CONTRACT_REVIEW_INVALID_STATE",
        format!(
            "{message}; current status {:?}, stage {:?}",
            session.status, session.stage
        ),
        false,
    )
}

fn review_not_found(review_id: &str) -> HostError {
    HostError::new(
        "CONTRACT_REVIEW_NOT_FOUND",
        format!("contract review {review_id} was not found"),
        false,
    )
}

fn evidence_context_inconsistent(evidence_id: &str, detail: &str) -> HostError {
    HostError::new(
        "EVIDENCE_CONTEXT_INCONSISTENT",
        format!("evidence context for {evidence_id} is inconsistent: {detail}"),
        false,
    )
}

fn normalize_uuid(field: &str, value: String) -> Result<String, HostError> {
    let value = value.trim().to_string();
    Uuid::parse_str(&value)
        .map(|_| value)
        .map_err(|_| HostError::validation(format!("{field} must be a UUID")))
}

fn normalize_required(field: &str, value: &str, max_chars: usize) -> Result<String, HostError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(HostError::validation(format!("{field} is required")));
    }
    if value.chars().count() > max_chars {
        return Err(HostError::validation(format!(
            "{field} exceeds {max_chars} characters"
        )));
    }
    Ok(value)
}

fn validate_sha256(field: &str, value: &str) -> Result<(), HostError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(HostError::validation(format!(
            "{field} must be a 64-character hexadecimal SHA-256"
        )));
    }
    Ok(())
}

fn validate_deadline(deadline_at: Option<i64>) -> Result<(), HostError> {
    if let Some(deadline) = deadline_at {
        if deadline < now_ms()? {
            return Err(HostError::new(
                "COMMAND_DEADLINE_EXCEEDED",
                "contract review command deadline has elapsed",
                true,
            ));
        }
    }
    Ok(())
}

fn now_ms() -> Result<i64, HostError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            HostError::internal(format!("system clock is before unix epoch: {error}"))
        })?;
    i64::try_from(duration.as_millis())
        .map_err(|_| HostError::internal("system timestamp exceeded i64"))
}

fn to_json<T: Serialize>(value: &T) -> Result<String, HostError> {
    serde_json::to_string(value).map_err(json_error)
}

fn from_json<T: DeserializeOwned>(value: &str) -> Result<T, HostError> {
    serde_json::from_str(value).map_err(json_error)
}

fn enum_to_db<T: Serialize>(value: &T) -> Result<String, HostError> {
    serde_json::to_value(value)
        .map_err(json_error)?
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| HostError::internal("enum did not serialize as a string"))
}

fn enum_from_sql<T: DeserializeOwned>(value: &str, column: usize) -> rusqlite::Result<T> {
    serde_json::from_value(serde_json::Value::String(value.to_string())).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn json_from_sql<T: DeserializeOwned>(value: &str, column: usize) -> rusqlite::Result<T> {
    serde_json::from_str(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn contract_review_event_type_from_wire(
    value: &str,
    column: usize,
) -> rusqlite::Result<ContractReviewEventType> {
    let event_type = match value {
        "contractReview.created" => ContractReviewEventType::Created,
        "contractReview.started" => ContractReviewEventType::Started,
        "contractReview.stageChanged" => ContractReviewEventType::StageChanged,
        "contractReview.extractionCompleted" => ContractReviewEventType::ExtractionCompleted,
        "contractReview.ocrRequired" => ContractReviewEventType::OcrRequired,
        "contractReview.findingAdded" => ContractReviewEventType::FindingAdded,
        "contractReview.findingUpdated" => ContractReviewEventType::FindingUpdated,
        "contractReview.findingDecided" => ContractReviewEventType::FindingDecided,
        "contractReview.reportGenerated" => ContractReviewEventType::ReportGenerated,
        "contractReview.completed" => ContractReviewEventType::Completed,
        "contractReview.failed" => ContractReviewEventType::Failed,
        "contractReview.cancelled" => ContractReviewEventType::Cancelled,
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                column,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown contract review event type {value}"),
                )),
            ));
        }
    };
    Ok(event_type)
}

fn sql_error(error: rusqlite::Error) -> HostError {
    match &error {
        rusqlite::Error::SqliteFailure(details, _)
            if details.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            HostError::new(
                "CONTRACT_REVIEW_PERSISTENCE_CONFLICT",
                format!("contract review persistence constraint failed: {error}"),
                false,
            )
        }
        _ => HostError::internal(format!("contract review database error: {error}")),
    }
}

fn json_error(error: serde_json::Error) -> HostError {
    HostError::internal(format!("contract review JSON error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        DocumentBlockKind, EvidenceBoundingBox, ParserProvenance, ReviewReportFormat,
        ReviewSeverity, RuleEvaluationStatus,
    };
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const SOURCE_HASH: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const REPORT_HASH: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    struct TestStore {
        _temp: TempDir,
        path: PathBuf,
        workspace_id: String,
        source_asset_id: String,
    }

    fn setup_store() -> TestStore {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("contract-review.sqlite3");
        let workspace_id = Uuid::new_v4().to_string();
        let source_asset_id = Uuid::new_v4().to_string();
        let connection = open_store(&path);
        connection
            .execute(
                "INSERT INTO business_workspaces (id) VALUES (?1)",
                [&workspace_id],
            )
            .unwrap();
        insert_asset(
            &connection,
            &source_asset_id,
            "customer-contract.pdf",
            SOURCE_HASH,
            "document",
        );
        drop(connection);
        TestStore {
            _temp: temp,
            path,
            workspace_id,
            source_asset_id,
        }
    }

    fn open_store(path: &Path) -> Connection {
        let connection = Connection::open(path).unwrap();
        connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys = ON;
                CREATE TABLE IF NOT EXISTS business_workspaces (
                    id TEXT PRIMARY KEY NOT NULL
                );
                CREATE TABLE IF NOT EXISTS assets (
                    id TEXT PRIMARY KEY NOT NULL,
                    original_name TEXT NOT NULL,
                    sha256 TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    status TEXT NOT NULL
                );
                "#,
            )
            .unwrap();
        migrate(&connection).unwrap();
        connection
    }

    fn insert_asset(
        connection: &Connection,
        id: &str,
        original_name: &str,
        sha256: &str,
        kind: &str,
    ) {
        connection
            .execute(
                "INSERT INTO assets (id, original_name, sha256, kind, status)
                 VALUES (?1, ?2, ?3, ?4, 'ready')",
                params![id, original_name, sha256, kind],
            )
            .unwrap();
    }

    fn context(trace_id: &str) -> OperationContext {
        OperationContext {
            actor_id: "tester".to_string(),
            account_id: Some("account-test".to_string()),
            project_id: None,
            window_id: "window-test".to_string(),
            trace_id: trace_id.to_string(),
        }
    }

    fn create_command(
        workspace_id: &str,
        source_asset_id: &str,
        command_id: &str,
        idempotency_key: &str,
    ) -> ContractReviewCommandEnvelope {
        ContractReviewCommandEnvelope::Create {
            command_id: command_id.to_string(),
            protocol_version: CONTRACT_REVIEW_PROTOCOL_VERSION.to_string(),
            context: context("trace-create"),
            payload: CreateContractReviewPayload {
                workspace_id: workspace_id.to_string(),
                source_asset_id: source_asset_id.to_string(),
            },
            idempotency_key: idempotency_key.to_string(),
            expected_revision: None,
            deadline_at: None,
        }
    }

    fn start_command(review_id: &str, revision: i64) -> ContractReviewCommandEnvelope {
        ContractReviewCommandEnvelope::Start {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: CONTRACT_REVIEW_PROTOCOL_VERSION.to_string(),
            context: context("trace-start"),
            payload: StartContractReviewPayload {
                review_id: review_id.to_string(),
            },
            idempotency_key: format!("start-{review_id}"),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }

    fn cancel_command(review_id: &str, revision: i64) -> ContractReviewCommandEnvelope {
        ContractReviewCommandEnvelope::Cancel {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: CONTRACT_REVIEW_PROTOCOL_VERSION.to_string(),
            context: context("trace-cancel"),
            payload: CancelContractReviewPayload {
                review_id: review_id.to_string(),
                reason: "operator cancelled the review".to_string(),
            },
            idempotency_key: format!("cancel-{review_id}-{revision}-{}", Uuid::new_v4()),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }

    fn generate_report_command(review_id: &str, revision: i64) -> ContractReviewCommandEnvelope {
        ContractReviewCommandEnvelope::GenerateReport {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: CONTRACT_REVIEW_PROTOCOL_VERSION.to_string(),
            context: context("trace-generate-report"),
            payload: GenerateReviewReportPayload {
                review_id: review_id.to_string(),
                format: ReviewReportFormat::Html,
            },
            idempotency_key: format!("generate-report-{review_id}-{revision}"),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }

    fn extraction_for(
        review: &ContractReviewRecord,
    ) -> (DocumentExtractionRecord, Vec<EvidenceAnchor>) {
        let extraction_id = Uuid::new_v4().to_string();
        let page_id = Uuid::new_v4().to_string();
        let block_id = Uuid::new_v4().to_string();
        let table_id = Uuid::new_v4().to_string();
        let text = "合同金额为人民币壹万元，验收后十个工作日内付款。".to_string();
        let quoted_text = "验收后十个工作日内付款".to_string();
        let page = DocumentPageRecord {
            id: page_id.clone(),
            extraction_id: extraction_id.clone(),
            page_index: 0,
            text: text.clone(),
            text_sha256: digest(&text),
            width: Some(595.0),
            height: Some(842.0),
            preview_asset_id: None,
        };
        let block = DocumentBlockRecord {
            id: block_id.clone(),
            extraction_id: extraction_id.clone(),
            page_id: page_id.clone(),
            page_index: 0,
            order_index: 0,
            kind: DocumentBlockKind::Paragraph,
            text: text.clone(),
            char_start: 0,
            char_end: text.chars().count() as i64,
            bbox: Some(EvidenceBoundingBox {
                x: 30.0,
                y: 40.0,
                width: 500.0,
                height: 40.0,
            }),
        };
        let table = DocumentTableRecord {
            id: table_id,
            extraction_id: extraction_id.clone(),
            page_id,
            page_index: 0,
            order_index: 0,
            markdown: "| 项目 | 金额 |\n|---|---|\n| 服务 | 10000 |".to_string(),
            data: serde_json::json!([["项目", "金额"], ["服务", "10000"]]),
            bbox: Some(EvidenceBoundingBox {
                x: 30.0,
                y: 100.0,
                width: 500.0,
                height: 80.0,
            }),
        };
        let evidence = EvidenceAnchor {
            id: Uuid::new_v4().to_string(),
            extraction_id: extraction_id.clone(),
            source_asset_id: review.session.source_asset_id.clone(),
            page_index: 0,
            block_id: Some(block_id),
            char_start: Some(8),
            char_end: Some(20),
            bbox: Some(EvidenceBoundingBox {
                x: 100.0,
                y: 40.0,
                width: 180.0,
                height: 20.0,
            }),
            quoted_text: quoted_text.clone(),
            quoted_text_sha256: digest(&quoted_text),
            context_before: "合同金额为人民币壹万元，".to_string(),
            context_after: "。".to_string(),
        };
        (
            DocumentExtractionRecord {
                id: extraction_id,
                review_id: review.session.id.clone(),
                source_asset_id: review.session.source_asset_id.clone(),
                source_asset_sha256: review.session.source_asset_sha256.clone(),
                parser: ParserProvenance {
                    name: "test-parser".to_string(),
                    version: "1.0.0".to_string(),
                    mode: "text-pdf".to_string(),
                },
                ocr: None,
                status: DocumentExtractionStatus::Completed,
                page_count: 1,
                content_sha256: Some(digest(&text)),
                snapshot_asset_id: None,
                pages: vec![page],
                blocks: vec![block],
                tables: vec![table],
                created_at: now_ms().unwrap(),
                completed_at: Some(now_ms().unwrap()),
                failure: None,
            },
            vec![evidence],
        )
    }

    fn rule_results(
        review_id: &str,
        evidence_id: &str,
    ) -> (Vec<RuleEvaluationRecord>, Vec<ReviewFindingRecord>) {
        let finding_id = Uuid::new_v4().to_string();
        let finding = ReviewFindingRecord {
            id: finding_id.clone(),
            review_id: review_id.to_string(),
            source: ReviewFindingSource::Rule,
            rule_id: Some("payment-term-risk".to_string()),
            rule_version: Some("1.0.0".to_string()),
            agent_run_id: None,
            category: "payment".to_string(),
            severity: ReviewSeverity::High,
            title: "付款条件缺少发票前置条件".to_string(),
            description: "付款条款仅绑定验收，未说明合法发票。".to_string(),
            recommendation: "补充收到合法有效发票后付款。".to_string(),
            evidence_ids: vec![evidence_id.to_string()],
            missing_evidence_reason: None,
            status: ReviewFindingStatus::Open,
            decision: ReviewFindingDecision::Unreviewed,
            revision: 99,
            created_at: 0,
            updated_at: 0,
        };
        let evaluation = RuleEvaluationRecord {
            id: Uuid::new_v4().to_string(),
            review_id: review_id.to_string(),
            rule_id: "payment-term-risk".to_string(),
            rule_version: "1.0.0".to_string(),
            status: RuleEvaluationStatus::Finding,
            finding_ids: vec![finding_id],
            details: "payment clause matched but invoice condition missing".to_string(),
            evaluated_at: 0,
        };
        (vec![evaluation], vec![finding])
    }

    fn prepare_awaiting_confirmation(
        connection: &mut Connection,
        store: &TestStore,
    ) -> (ContractReviewRecord, EvidenceAnchor, ReviewFindingRecord) {
        let created = execute_command(
            connection,
            create_command(
                &store.workspace_id,
                &store.source_asset_id,
                &Uuid::new_v4().to_string(),
                &format!("create-{}", Uuid::new_v4()),
            ),
        )
        .unwrap();
        let started = execute_command(
            connection,
            start_command(
                &created.response.contract_review.session.id,
                created.response.contract_review.session.revision,
            ),
        )
        .unwrap();
        let (extraction, evidence) = extraction_for(&started.response.contract_review);
        let extracted = save_extraction(
            connection,
            &extraction,
            &evidence,
            started.response.contract_review.session.revision,
            "trace-extraction",
        )
        .unwrap();
        let (evaluations, findings) =
            rule_results(&extracted.contract_review.session.id, &evidence[0].id);
        let reviewed = replace_rule_evaluations_and_findings(
            connection,
            &extracted.contract_review.session.id,
            &evaluations,
            &findings,
            &evidence,
            extracted.contract_review.session.revision,
            "trace-rules",
        )
        .unwrap();
        (
            reviewed.contract_review.clone(),
            evidence[0].clone(),
            reviewed.contract_review.findings[0].clone(),
        )
    }

    fn digest(value: &str) -> String {
        format!("{:x}", Sha256::digest(value.as_bytes()))
    }

    fn test_finding(
        review_id: &str,
        id: &str,
        status: ReviewFindingStatus,
        created_at: i64,
    ) -> ReviewFindingRecord {
        ReviewFindingRecord {
            id: id.to_string(),
            review_id: review_id.to_string(),
            source: ReviewFindingSource::Manual,
            rule_id: None,
            rule_version: None,
            agent_run_id: None,
            category: "test".to_string(),
            severity: ReviewSeverity::Medium,
            title: format!("finding {id}"),
            description: "test finding".to_string(),
            recommendation: "review it".to_string(),
            evidence_ids: vec![],
            missing_evidence_reason: Some("test fixture".to_string()),
            status,
            decision: if status == ReviewFindingStatus::Decided {
                ReviewFindingDecision::Confirmed
            } else {
                ReviewFindingDecision::Unreviewed
            },
            revision: 1,
            created_at,
            updated_at: created_at,
        }
    }

    #[test]
    fn running_review_is_recovered_once_after_restart() {
        let store = setup_store();
        let mut connection = open_store(&store.path);
        let created = execute_command(
            &mut connection,
            create_command(
                &store.workspace_id,
                &store.source_asset_id,
                &Uuid::new_v4().to_string(),
                &format!("create-{}", Uuid::new_v4()),
            ),
        )
        .unwrap();
        let started = execute_command(
            &mut connection,
            start_command(
                &created.response.contract_review.session.id,
                created.response.contract_review.session.revision,
            ),
        )
        .unwrap()
        .response
        .contract_review;
        let review_id = started.session.id.clone();
        let running_revision = started.session.revision;
        assert_eq!(started.session.status, ContractReviewStatus::Running);
        assert_eq!(started.session.stage, ContractReviewStage::Extracting);
        drop(connection);

        let connection = open_store(&store.path);
        let recovered = get_review(&connection, &review_id).unwrap();
        assert_eq!(recovered.session.status, ContractReviewStatus::Failed);
        assert_eq!(recovered.session.stage, ContractReviewStage::Extracting);
        assert_eq!(recovered.session.revision, running_revision + 1);
        let failure = recovered.session.failure.as_ref().unwrap();
        assert_eq!(failure.code, "CONTRACT_REVIEW_INTERRUPTED");
        assert!(failure.retryable);
        let events = replay_events(&connection, 0, 100).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == ContractReviewEventType::Failed)
                .count(),
            1
        );
        let recovered_revision = recovered.session.revision;
        let event_count = events.len();
        drop(connection);

        let connection = open_store(&store.path);
        let recovered_again = get_review(&connection, &review_id).unwrap();
        assert_eq!(recovered_again.session.revision, recovered_revision);
        assert_eq!(
            replay_events(&connection, 0, 100).unwrap().len(),
            event_count
        );
    }

    #[test]
    fn cancel_accepts_stale_revision_and_rejects_future_revision() {
        let store = setup_store();
        let mut connection = open_store(&store.path);

        let stale_created = execute_command(
            &mut connection,
            create_command(
                &store.workspace_id,
                &store.source_asset_id,
                &Uuid::new_v4().to_string(),
                &format!("create-stale-{}", Uuid::new_v4()),
            ),
        )
        .unwrap();
        let stale_started = execute_command(
            &mut connection,
            start_command(
                &stale_created.response.contract_review.session.id,
                stale_created.response.contract_review.session.revision,
            ),
        )
        .unwrap()
        .response
        .contract_review;
        let stale_cancelled = execute_command(
            &mut connection,
            cancel_command(
                &stale_started.session.id,
                stale_created.response.contract_review.session.revision,
            ),
        )
        .unwrap()
        .response
        .contract_review;
        assert_eq!(
            stale_cancelled.session.status,
            ContractReviewStatus::Cancelled
        );
        assert_eq!(
            stale_cancelled.session.revision,
            stale_started.session.revision + 1
        );

        let future_created = execute_command(
            &mut connection,
            create_command(
                &store.workspace_id,
                &store.source_asset_id,
                &Uuid::new_v4().to_string(),
                &format!("create-future-{}", Uuid::new_v4()),
            ),
        )
        .unwrap();
        let future_started = execute_command(
            &mut connection,
            start_command(
                &future_created.response.contract_review.session.id,
                future_created.response.contract_review.session.revision,
            ),
        )
        .unwrap()
        .response
        .contract_review;
        let conflict = execute_command(
            &mut connection,
            cancel_command(
                &future_started.session.id,
                future_started.session.revision + 1,
            ),
        )
        .unwrap_err();
        assert_eq!(conflict.code, "REVISION_CONFLICT");
        let unchanged = get_review(&connection, &future_started.session.id).unwrap();
        assert_eq!(unchanged.session.status, ContractReviewStatus::Running);
        assert_eq!(unchanged.session.revision, future_started.session.revision);
    }

    #[test]
    fn evidence_context_rejects_extraction_source_hash_mismatch() {
        let store = setup_store();
        let mut connection = open_store(&store.path);
        let (awaiting, evidence, _finding) = prepare_awaiting_confirmation(&mut connection, &store);
        let extraction_id = awaiting.session.extraction_id.as_ref().unwrap();
        let stored_json: String = connection
            .query_row(
                "SELECT record_json FROM document_extractions WHERE id = ?1",
                [extraction_id],
                |row| row.get(0),
            )
            .unwrap();
        let mut extraction: DocumentExtractionRecord = serde_json::from_str(&stored_json).unwrap();
        extraction.source_asset_sha256 = "f".repeat(64);
        connection
            .execute(
                "UPDATE document_extractions SET record_json = ?1 WHERE id = ?2",
                params![serde_json::to_string(&extraction).unwrap(), extraction_id],
            )
            .unwrap();

        let error = get_evidence_context(&connection, &evidence.id).unwrap_err();
        assert_eq!(error.code, "EVIDENCE_CONTEXT_INCONSISTENT");
        assert!(!error.retryable);
    }

    #[test]
    fn list_review_findings_validates_review_filters_status_and_orders_stably() {
        let store = setup_store();
        let mut connection = open_store(&store.path);
        let created = execute_command(
            &mut connection,
            create_command(
                &store.workspace_id,
                &store.source_asset_id,
                &Uuid::new_v4().to_string(),
                "create-list-findings",
            ),
        )
        .unwrap();
        let review_id = created.response.contract_review.session.id;
        let first_id = "00000000-0000-4000-8000-000000000001";
        let second_id = "00000000-0000-4000-8000-000000000002";
        let third_id = "00000000-0000-4000-8000-000000000003";
        let findings = [
            test_finding(&review_id, third_id, ReviewFindingStatus::Open, 100),
            test_finding(&review_id, first_id, ReviewFindingStatus::Decided, 100),
            test_finding(&review_id, second_id, ReviewFindingStatus::Open, 100),
        ];
        let transaction = connection.transaction().unwrap();
        for finding in &findings {
            insert_finding(&transaction, finding).unwrap();
        }
        transaction.commit().unwrap();

        let all = list_review_findings(&connection, &review_id, None).unwrap();
        assert_eq!(
            all.iter()
                .map(|finding| finding.id.as_str())
                .collect::<Vec<_>>(),
            vec![first_id, second_id, third_id]
        );
        let open =
            list_review_findings(&connection, &review_id, Some(ReviewFindingStatus::Open)).unwrap();
        assert_eq!(
            open.iter()
                .map(|finding| finding.id.as_str())
                .collect::<Vec<_>>(),
            vec![second_id, third_id]
        );

        let missing =
            list_review_findings(&connection, &Uuid::new_v4().to_string(), None).unwrap_err();
        assert_eq!(missing.code, "CONTRACT_REVIEW_NOT_FOUND");
    }

    #[test]
    fn evidence_context_resolves_page_and_optional_block() {
        let store = setup_store();
        let mut connection = open_store(&store.path);
        let (awaiting, evidence, _finding) = prepare_awaiting_confirmation(&mut connection, &store);
        let extraction = awaiting.extraction.as_ref().unwrap();

        let context = get_evidence_context(&connection, &evidence.id).unwrap();
        assert_eq!(context.evidence, evidence);
        assert_eq!(context.page, extraction.pages[0]);
        assert_eq!(context.block.as_ref(), Some(&extraction.blocks[0]));

        let without_block = EvidenceAnchor {
            id: Uuid::new_v4().to_string(),
            block_id: None,
            ..context.evidence.clone()
        };
        let transaction = connection.transaction().unwrap();
        insert_evidence(&transaction, &awaiting.session.id, &without_block).unwrap();
        transaction.commit().unwrap();
        let without_block_context = get_evidence_context(&connection, &without_block.id).unwrap();
        assert_eq!(without_block_context.evidence, without_block);
        assert_eq!(without_block_context.page, extraction.pages[0]);
        assert_eq!(without_block_context.block, None);

        let missing = get_evidence_context(&connection, &Uuid::new_v4().to_string()).unwrap_err();
        assert_eq!(missing.code, "EVIDENCE_NOT_FOUND");
        assert!(!missing.retryable);
    }

    #[test]
    fn evidence_context_reports_missing_page_and_block() {
        let block_store = setup_store();
        let mut block_connection = open_store(&block_store.path);
        let (_awaiting, block_evidence, _finding) =
            prepare_awaiting_confirmation(&mut block_connection, &block_store);
        block_connection
            .execute(
                "DELETE FROM document_blocks WHERE id = ?1",
                [block_evidence.block_id.as_deref().unwrap()],
            )
            .unwrap();
        let missing_block =
            get_evidence_context(&block_connection, &block_evidence.id).unwrap_err();
        assert_eq!(missing_block.code, "EVIDENCE_BLOCK_NOT_FOUND");
        assert!(!missing_block.retryable);

        let page_store = setup_store();
        let mut page_connection = open_store(&page_store.path);
        let (awaiting, page_evidence, _finding) =
            prepare_awaiting_confirmation(&mut page_connection, &page_store);
        let page_id = awaiting.extraction.as_ref().unwrap().pages[0].id.clone();
        page_connection
            .execute("DELETE FROM document_pages WHERE id = ?1", [&page_id])
            .unwrap();
        let missing_page = get_evidence_context(&page_connection, &page_evidence.id).unwrap_err();
        assert_eq!(missing_page.code, "EVIDENCE_PAGE_NOT_FOUND");
        assert!(!missing_page.retryable);
    }

    #[test]
    fn evidence_context_rejects_inconsistent_relationship_metadata() {
        let store = setup_store();
        let mut connection = open_store(&store.path);
        let (_awaiting, evidence, _finding) =
            prepare_awaiting_confirmation(&mut connection, &store);
        connection
            .execute(
                "UPDATE contract_review_evidence SET page_index = page_index + 1 WHERE id = ?1",
                [&evidence.id],
            )
            .unwrap();

        let error = get_evidence_context(&connection, &evidence.id).unwrap_err();
        assert_eq!(error.code, "EVIDENCE_CONTEXT_INCONSISTENT");
        assert!(!error.retryable);
    }

    #[test]
    fn restart_preserves_complete_review_graph() {
        let store = setup_store();
        let mut connection = open_store(&store.path);
        let (awaiting, _evidence, finding) = prepare_awaiting_confirmation(&mut connection, &store);
        let decided = decide_finding(
            &mut connection,
            &DecideReviewFindingPayload {
                review_id: awaiting.session.id.clone(),
                finding_id: finding.id,
                decision: ReviewFindingDecision::Confirmed,
                comment: "确认该风险，要求合同补充发票条件。".to_string(),
            },
            finding.revision,
            "reviewer-1",
            "trace-decision",
        )
        .unwrap();
        let prepared = execute_command(
            &mut connection,
            generate_report_command(
                &decided.contract_review.session.id,
                decided.contract_review.session.revision,
            ),
        )
        .unwrap();
        let report_asset_id = Uuid::new_v4().to_string();
        insert_asset(
            &connection,
            &report_asset_id,
            "contract-review.html",
            REPORT_HASH,
            "document",
        );
        let report = ReviewReportRecord {
            id: Uuid::new_v4().to_string(),
            review_id: prepared.response.contract_review.session.id.clone(),
            review_revision: prepared.response.contract_review.session.revision,
            source_asset_id: prepared
                .response
                .contract_review
                .session
                .source_asset_id
                .clone(),
            source_asset_sha256: prepared
                .response
                .contract_review
                .session
                .source_asset_sha256
                .clone(),
            extraction_id: prepared
                .response
                .contract_review
                .session
                .extraction_id
                .clone()
                .unwrap(),
            rule_set_version: "2026-07-19".to_string(),
            agent_run_ids: vec![],
            format: ReviewReportFormat::Html,
            report_asset_id: report_asset_id.clone(),
            report_asset_sha256: REPORT_HASH.to_string(),
            generated_at: now_ms().unwrap(),
        };
        let completed = save_report_and_complete(
            &mut connection,
            &report,
            prepared.response.contract_review.session.revision,
            "trace-report-commit",
        )
        .unwrap();
        let review_id = completed.contract_review.session.id.clone();
        assert_eq!(
            completed.contract_review.session.status,
            ContractReviewStatus::Completed
        );
        drop(connection);

        let connection = open_store(&store.path);
        let restored = get_review(&connection, &review_id).unwrap();
        assert_eq!(restored.session.status, ContractReviewStatus::Completed);
        assert_eq!(
            restored.session.report_asset_id.as_deref(),
            Some(report_asset_id.as_str())
        );
        assert_eq!(restored.extraction.as_ref().unwrap().pages.len(), 1);
        assert_eq!(restored.extraction.as_ref().unwrap().blocks.len(), 1);
        assert_eq!(restored.extraction.as_ref().unwrap().tables.len(), 1);
        assert_eq!(restored.evidence.len(), 1);
        assert_eq!(restored.findings.len(), 1);
        assert_eq!(
            restored.findings[0].decision,
            ReviewFindingDecision::Confirmed
        );
        assert_eq!(restored.decisions.len(), 1);
        assert_eq!(restored.rule_evaluations.len(), 1);
        assert_eq!(restored.reports, vec![report]);
        let listed = list_reviews(
            &connection,
            &ListContractReviewsRequest {
                workspace_id: Some(store.workspace_id.clone()),
                status: Some(ContractReviewStatus::Completed),
                limit: Some(10),
            },
        )
        .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0], restored);
    }

    #[test]
    fn command_receipt_replays_without_duplicate_state_or_event() {
        let store = setup_store();
        let mut connection = open_store(&store.path);
        let command = create_command(
            &store.workspace_id,
            &store.source_asset_id,
            &Uuid::new_v4().to_string(),
            "stable-create-key",
        );
        let first = execute_command(&mut connection, command.clone()).unwrap();
        let replay = execute_command(&mut connection, command).unwrap();
        assert!(!first.response.replayed);
        assert!(replay.response.replayed);
        assert!(replay.emitted_events.is_empty());
        assert_eq!(
            first.response.contract_review,
            replay.response.contract_review
        );
        let sessions: i64 = connection
            .query_row("SELECT COUNT(*) FROM contract_review_sessions", [], |row| {
                row.get(0)
            })
            .unwrap();
        let events: i64 = connection
            .query_row("SELECT COUNT(*) FROM contract_review_events", [], |row| {
                row.get(0)
            })
            .unwrap();
        let receipts: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM contract_review_command_receipts",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((sessions, events, receipts), (1, 1, 1));
    }

    #[test]
    fn finding_decision_uses_revision_cas_and_preserves_first_decision() {
        let store = setup_store();
        let mut connection = open_store(&store.path);
        let (awaiting, _evidence, finding) = prepare_awaiting_confirmation(&mut connection, &store);
        let payload = DecideReviewFindingPayload {
            review_id: awaiting.session.id.clone(),
            finding_id: finding.id.clone(),
            decision: ReviewFindingDecision::Confirmed,
            comment: "first decision".to_string(),
        };
        let first = decide_finding(
            &mut connection,
            &payload,
            finding.revision,
            "reviewer-1",
            "trace-first-decision",
        )
        .unwrap();
        let event_count_before = replay_events(&connection, 0, 100).unwrap().len();
        let conflict = decide_finding(
            &mut connection,
            &DecideReviewFindingPayload {
                decision: ReviewFindingDecision::Rejected,
                comment: "stale decision".to_string(),
                ..payload
            },
            finding.revision,
            "reviewer-2",
            "trace-stale-decision",
        )
        .unwrap_err();
        assert_eq!(conflict.code, "REVISION_CONFLICT");
        let persisted = get_review(&connection, &awaiting.session.id).unwrap();
        assert_eq!(
            persisted.findings[0].decision,
            ReviewFindingDecision::Confirmed
        );
        assert_eq!(persisted.findings[0].revision, finding.revision + 1);
        assert_eq!(persisted.decisions.len(), 1);
        assert_eq!(persisted.decisions[0].comment, "first decision");
        assert_eq!(
            replay_events(&connection, 0, 100).unwrap().len(),
            event_count_before
        );
        assert_eq!(
            first.contract_review.session.revision,
            awaiting.session.revision + 1
        );
    }

    #[test]
    fn event_replay_is_strictly_ordered_across_full_lifecycle() {
        let store = setup_store();
        let mut connection = open_store(&store.path);
        let (awaiting, _evidence, finding) = prepare_awaiting_confirmation(&mut connection, &store);
        let decided = decide_finding(
            &mut connection,
            &DecideReviewFindingPayload {
                review_id: awaiting.session.id.clone(),
                finding_id: finding.id,
                decision: ReviewFindingDecision::AcceptedRisk,
                comment: "accepted for this delivery".to_string(),
            },
            finding.revision,
            "reviewer-1",
            "trace-decision-order",
        )
        .unwrap();
        let prepared = execute_command(
            &mut connection,
            generate_report_command(
                &decided.contract_review.session.id,
                decided.contract_review.session.revision,
            ),
        )
        .unwrap();
        let report_asset_id = Uuid::new_v4().to_string();
        insert_asset(
            &connection,
            &report_asset_id,
            "ordered-report.json",
            REPORT_HASH,
            "document",
        );
        let report = ReviewReportRecord {
            id: Uuid::new_v4().to_string(),
            review_id: prepared.response.contract_review.session.id.clone(),
            review_revision: prepared.response.contract_review.session.revision,
            source_asset_id: prepared
                .response
                .contract_review
                .session
                .source_asset_id
                .clone(),
            source_asset_sha256: SOURCE_HASH.to_string(),
            extraction_id: prepared
                .response
                .contract_review
                .session
                .extraction_id
                .clone()
                .unwrap(),
            rule_set_version: "rules-1".to_string(),
            agent_run_ids: vec![],
            format: ReviewReportFormat::Json,
            report_asset_id,
            report_asset_sha256: REPORT_HASH.to_string(),
            generated_at: now_ms().unwrap(),
        };
        save_report_and_complete(
            &mut connection,
            &report,
            prepared.response.contract_review.session.revision,
            "trace-complete-order",
        )
        .unwrap();

        let events = replay_events(&connection, 0, 100).unwrap();
        let expected = [
            ContractReviewEventType::Created,
            ContractReviewEventType::Started,
            ContractReviewEventType::ExtractionCompleted,
            ContractReviewEventType::FindingAdded,
            ContractReviewEventType::FindingDecided,
            ContractReviewEventType::StageChanged,
            ContractReviewEventType::ReportGenerated,
            ContractReviewEventType::Completed,
        ];
        assert_eq!(events.len(), expected.len());
        for (index, event) in events.iter().enumerate() {
            assert_eq!(event.sequence, index as i64 + 1);
            assert_eq!(event.event_type, expected[index]);
            if index > 0 {
                assert!(event.revision >= events[index - 1].revision);
            }
        }
        let tail = replay_events(&connection, 4, 100).unwrap();
        assert_eq!(tail.len(), 4);
        assert_eq!(tail[0].sequence, 5);
        assert_eq!(tail[3].event_type, ContractReviewEventType::Completed);
    }
}
