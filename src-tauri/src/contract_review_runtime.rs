use crate::asset_service::{self, GeneratedArtifactSource};
use crate::backup_outbox::BackupOutbox;
use crate::codex_runtime::CancellationToken;
use crate::contract_review_agent::ContractAgentReviewer;
use crate::contract_review_rules::{ContractReviewRuleEngine, RULE_SET_VERSION};
use crate::contract_review_service;
use crate::document_intelligence::DocumentIntelligence;
use crate::protocol::{
    BackupCommandEnvelope, BackupDomainEvent, ContractReviewCommandEnvelope,
    ContractReviewCommandResponse, ContractReviewDomainEvent, ContractReviewFailure,
    ContractReviewRecord, ContractReviewStage, ContractReviewStatus, HostError, OperationContext,
    QueueAssetBackupPayload, ReviewReportFormat, ReviewReportRecord, BACKUP_PROTOCOL_VERSION,
};
use crate::review_report;
use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_PIPELINE_TRANSITIONS: usize = 8;
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ActiveReviewKey {
    database_path: PathBuf,
    review_id: String,
}

#[derive(Debug, Clone)]
struct ActiveReview {
    generation_id: String,
    cancellation: CancellationToken,
}

fn active_reviews() -> &'static Mutex<HashMap<ActiveReviewKey, ActiveReview>> {
    static ACTIVE_REVIEWS: OnceLock<Mutex<HashMap<ActiveReviewKey, ActiveReview>>> =
        OnceLock::new();
    ACTIVE_REVIEWS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug)]
pub struct ContractReviewRuntimeOutcome {
    pub response: ContractReviewCommandResponse,
    pub contract_events: Vec<ContractReviewDomainEvent>,
    pub backup_events: Vec<BackupDomainEvent>,
    /// Backup is deliberately sidecar-only. These warnings are diagnostic and
    /// never downgrade a locally completed contract review.
    pub backup_warnings: Vec<String>,
}

/// Executes a durable contract-review command and drives the supported local
/// stages to their next human boundary. Local Vault + SQLite are authoritative;
/// R2 receives only asynchronous outbox intents after the local report commits.
#[cfg(test)]
pub fn execute_contract_review_command(
    connection: &mut Connection,
    vault_root: &Path,
    staging_root: &Path,
    backup_outbox: &BackupOutbox,
    command: ContractReviewCommandEnvelope,
) -> Result<ContractReviewRuntimeOutcome, HostError> {
    execute_contract_review_command_with_agent(
        connection,
        vault_root,
        staging_root,
        backup_outbox,
        command,
        &RuleOnlyTestAgent,
    )
}

pub fn execute_contract_review_command_with_agent(
    connection: &mut Connection,
    vault_root: &Path,
    staging_root: &Path,
    backup_outbox: &BackupOutbox,
    command: ContractReviewCommandEnvelope,
    agent: &dyn ContractAgentReviewer,
) -> Result<ContractReviewRuntimeOutcome, HostError> {
    let context = command_context(&command).clone();
    let requested_report_format = command_report_format(&command);
    let should_drive = matches!(
        command,
        ContractReviewCommandEnvelope::Start { .. }
            | ContractReviewCommandEnvelope::GenerateReport { .. }
            | ContractReviewCommandEnvelope::RetryStage { .. }
    );
    let is_cancel = matches!(command, ContractReviewCommandEnvelope::Cancel { .. });
    let database_path = sqlite_main_database_path(connection)?;

    let initial = contract_review_service::execute_command(connection, command)?;
    let review_id = initial.response.contract_review.session.id.clone();
    let response = initial.response;
    let mut contract_events = initial.emitted_events;
    let mut backup_events = Vec::new();
    let mut backup_warnings = Vec::new();

    if is_cancel {
        if let Some(database_path) = database_path.as_ref() {
            cancel_active_review(database_path, &review_id);
        }
    }

    let command_was_new = !contract_events.is_empty();
    if should_drive && command_was_new {
        if let (Some(database_path), Some(detached_agent)) =
            (database_path.clone(), agent.detached_clone())
        {
            let generation_id = Uuid::new_v4().to_string();
            let cancellation = CancellationToken::new();
            register_active_review(
                &database_path,
                &review_id,
                &generation_id,
                cancellation.clone(),
            );
            let worker = DetachedReviewWorker {
                database_path: database_path.clone(),
                vault_root: vault_root.to_path_buf(),
                staging_root: staging_root.to_path_buf(),
                review_id: review_id.clone(),
                generation_id: generation_id.clone(),
                operation: context.clone(),
                requested_report_format,
                cancellation,
                agent: detached_agent,
            };
            if let Err(error) = thread::Builder::new()
                .name(format!("contract-review-{review_id}"))
                .spawn(move || run_detached_review(worker))
            {
                unregister_active_review(&database_path, &review_id, &generation_id);
                let spawn_error = HostError::new(
                    "CONTRACT_REVIEW_WORKER_START_FAILED",
                    format!("unable to start contract review worker: {error}"),
                    true,
                );
                persist_runtime_failure(
                    connection,
                    &review_id,
                    &context.trace_id,
                    &spawn_error,
                    &mut contract_events,
                )?;
            } else {
                return Ok(ContractReviewRuntimeOutcome {
                    response,
                    contract_events,
                    backup_events,
                    backup_warnings,
                });
            }
        } else {
            let cancellation = CancellationToken::new();
            let runtime = ReviewPipelineRuntime {
                vault_root,
                staging_root,
                operation: &context,
                requested_report_format,
                cancellation: &cancellation,
                agent,
            };
            if let Err(error) =
                drive_review_pipeline(connection, &review_id, &runtime, &mut contract_events)
            {
                persist_runtime_failure(
                    connection,
                    &review_id,
                    &context.trace_id,
                    &error,
                    &mut contract_events,
                )?;
            }
        }
    }

    let latest = contract_review_service::get_review(connection, &review_id)?;
    if latest.session.status == ContractReviewStatus::Completed {
        queue_completed_review_backups(
            connection,
            backup_outbox,
            &latest,
            &context,
            &mut backup_events,
            &mut backup_warnings,
        );
    }

    let mut response = response;
    response.contract_review = latest;
    Ok(ContractReviewRuntimeOutcome {
        response,
        contract_events,
        backup_events,
        backup_warnings,
    })
}

struct DetachedReviewWorker {
    database_path: PathBuf,
    vault_root: PathBuf,
    staging_root: PathBuf,
    review_id: String,
    generation_id: String,
    operation: OperationContext,
    requested_report_format: Option<ReviewReportFormat>,
    cancellation: CancellationToken,
    agent: Box<dyn ContractAgentReviewer + Send + Sync>,
}

fn run_detached_review(worker: DetachedReviewWorker) {
    let database_path = worker.database_path.clone();
    let review_id = worker.review_id.clone();
    let generation_id = worker.generation_id.clone();
    let trace_id = worker.operation.trace_id.clone();
    let outcome = catch_unwind(AssertUnwindSafe(|| run_detached_review_inner(&worker)));
    let failure = match outcome {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => Some(HostError::new(
            "CONTRACT_REVIEW_WORKER_PANICKED",
            "contract review worker panicked before reaching a durable boundary",
            true,
        )),
    };
    if let Some(error) = failure {
        if let Ok(mut connection) = open_worker_connection(&database_path) {
            let mut ignored_events = Vec::new();
            if let Err(persist_error) = persist_runtime_failure(
                &mut connection,
                &review_id,
                &trace_id,
                &error,
                &mut ignored_events,
            ) {
                eprintln!(
                    "contract review {review_id} failure could not be persisted: {persist_error}"
                );
            }
        } else {
            eprintln!("contract review {review_id} worker failed before SQLite could be reopened");
        }
    }
    unregister_active_review(&database_path, &review_id, &generation_id);
}

fn run_detached_review_inner(worker: &DetachedReviewWorker) -> Result<(), HostError> {
    worker.cancellation.check_cancelled()?;
    let mut connection = open_worker_connection(&worker.database_path)?;
    let mut emitted_events = Vec::new();
    let runtime = ReviewPipelineRuntime {
        vault_root: &worker.vault_root,
        staging_root: &worker.staging_root,
        operation: &worker.operation,
        requested_report_format: worker.requested_report_format,
        cancellation: &worker.cancellation,
        agent: worker.agent.as_ref(),
    };
    if let Err(error) = drive_review_pipeline(
        &mut connection,
        &worker.review_id,
        &runtime,
        &mut emitted_events,
    ) {
        if worker.cancellation.is_cancelled()
            || error.code == "CONTRACT_REVIEW_CANCELLED"
            || error.code == "DOCUMENT_EXTRACTION_CANCELLED"
        {
            let latest = contract_review_service::get_review(&connection, &worker.review_id)?;
            if latest.session.status == ContractReviewStatus::Cancelled {
                return Ok(());
            }
        }
        return Err(error);
    }

    worker.cancellation.check_cancelled()?;
    let latest = contract_review_service::get_review(&connection, &worker.review_id)?;
    if latest.session.status == ContractReviewStatus::Completed {
        match BackupOutbox::open(&worker.database_path) {
            Ok(outbox) => {
                let mut backup_events = Vec::new();
                let mut backup_warnings = Vec::new();
                queue_completed_review_backups(
                    &connection,
                    &outbox,
                    &latest,
                    &worker.operation,
                    &mut backup_events,
                    &mut backup_warnings,
                );
                for warning in backup_warnings {
                    eprintln!("contract review backup warning: {warning}");
                }
            }
            Err(error) => eprintln!(
                "contract review {} completed locally; backup outbox unavailable: {}",
                worker.review_id, error
            ),
        }
    }
    Ok(())
}

fn drive_review_pipeline(
    connection: &mut Connection,
    review_id: &str,
    runtime: &ReviewPipelineRuntime<'_>,
    emitted_events: &mut Vec<ContractReviewDomainEvent>,
) -> Result<(), HostError> {
    runtime.cancellation.check_cancelled()?;
    let review = contract_review_service::get_review(connection, review_id)?;
    if review.session.status == ContractReviewStatus::Cancelled {
        return Ok(());
    }
    let appending_report_to_completed = runtime.requested_report_format.is_some()
        && review.session.status == ContractReviewStatus::Completed
        && review.session.stage == ContractReviewStage::Completed;
    if appending_report_to_completed {
        let outcome = run_report_stage(
            connection,
            runtime.vault_root,
            runtime.staging_root,
            &review,
            runtime
                .requested_report_format
                .unwrap_or(ReviewReportFormat::Html),
            &runtime.operation.trace_id,
            runtime.cancellation,
        )?;
        emitted_events.extend(outcome.emitted_events);
        Ok(())
    } else {
        resume_supported_stages(connection, review_id, runtime, emitted_events)
    }
}

fn sqlite_main_database_path(connection: &Connection) -> Result<Option<PathBuf>, HostError> {
    let mut statement = connection
        .prepare("PRAGMA database_list")
        .map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        let name: String = row.get(1).map_err(sql_error)?;
        let file: String = row.get(2).map_err(sql_error)?;
        if name == "main" {
            if file.trim().is_empty() {
                return Ok(None);
            }
            let path = PathBuf::from(file);
            return Ok(Some(fs::canonicalize(&path).unwrap_or(path)));
        }
    }
    Ok(None)
}

fn open_worker_connection(database_path: &Path) -> Result<Connection, HostError> {
    let connection = Connection::open(database_path).map_err(sql_error)?;
    connection
        .busy_timeout(SQLITE_BUSY_TIMEOUT)
        .map_err(sql_error)?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(sql_error)?;
    Ok(connection)
}

fn register_active_review(
    database_path: &Path,
    review_id: &str,
    generation_id: &str,
    cancellation: CancellationToken,
) {
    let key = ActiveReviewKey {
        database_path: database_path.to_path_buf(),
        review_id: review_id.to_string(),
    };
    let active = ActiveReview {
        generation_id: generation_id.to_string(),
        cancellation,
    };
    let previous = active_reviews()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(key, active);
    if let Some(previous) = previous {
        previous.cancellation.cancel();
    }
}

fn cancel_active_review(database_path: &Path, review_id: &str) -> bool {
    let key = ActiveReviewKey {
        database_path: database_path.to_path_buf(),
        review_id: review_id.to_string(),
    };
    let cancellation = active_reviews()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&key)
        .map(|active| active.cancellation.clone());
    if let Some(cancellation) = cancellation {
        cancellation.cancel();
        true
    } else {
        false
    }
}

fn sql_error(error: rusqlite::Error) -> HostError {
    HostError::internal(format!(
        "contract review runtime SQLite operation failed: {error}"
    ))
}

fn unregister_active_review(database_path: &Path, review_id: &str, generation_id: &str) {
    let key = ActiveReviewKey {
        database_path: database_path.to_path_buf(),
        review_id: review_id.to_string(),
    };
    let mut active = active_reviews()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if active
        .get(&key)
        .is_some_and(|entry| entry.generation_id == generation_id)
    {
        active.remove(&key);
    }
}

struct ReviewPipelineRuntime<'a> {
    vault_root: &'a Path,
    staging_root: &'a Path,
    operation: &'a OperationContext,
    requested_report_format: Option<ReviewReportFormat>,
    cancellation: &'a CancellationToken,
    agent: &'a dyn ContractAgentReviewer,
}

fn resume_supported_stages(
    connection: &mut Connection,
    review_id: &str,
    runtime: &ReviewPipelineRuntime<'_>,
    emitted_events: &mut Vec<ContractReviewDomainEvent>,
) -> Result<(), HostError> {
    for _ in 0..MAX_PIPELINE_TRANSITIONS {
        runtime.cancellation.check_cancelled()?;
        let review = contract_review_service::get_review(connection, review_id)?;
        if matches!(
            review.session.status,
            ContractReviewStatus::Cancelled | ContractReviewStatus::Failed
        ) {
            return Ok(());
        }
        match review.session.stage {
            ContractReviewStage::Extracting => {
                let outcome = run_extraction_stage(
                    connection,
                    runtime.vault_root,
                    runtime.staging_root,
                    &review,
                    &runtime.operation.trace_id,
                    runtime.cancellation,
                )?;
                emitted_events.extend(outcome.emitted_events);
            }
            ContractReviewStage::ReviewingRules => {
                let extraction = review.extraction.as_ref().ok_or_else(|| {
                    HostError::new(
                        "EXTRACTION_REQUIRED",
                        "rule review cannot continue without a persisted extraction",
                        false,
                    )
                })?;
                runtime.cancellation.check_cancelled()?;
                let output = ContractReviewRuleEngine.evaluate(review_id, extraction, now_ms()?);
                runtime.cancellation.check_cancelled()?;
                let outcome = contract_review_service::replace_rule_evaluations_and_findings(
                    connection,
                    review_id,
                    &output.evaluations,
                    &output.findings,
                    &output.evidence,
                    review.session.revision,
                    &runtime.operation.trace_id,
                )?;
                emitted_events.extend(outcome.emitted_events);
                runtime.cancellation.check_cancelled()?;
                let outcome = contract_review_service::begin_agent_review(
                    connection,
                    review_id,
                    outcome.contract_review.session.revision,
                    &runtime.operation.trace_id,
                )?;
                emitted_events.extend(outcome.emitted_events);
            }
            ContractReviewStage::GeneratingReport => {
                let format = runtime
                    .requested_report_format
                    .unwrap_or(ReviewReportFormat::Html);
                let outcome = run_report_stage(
                    connection,
                    runtime.vault_root,
                    runtime.staging_root,
                    &review,
                    format,
                    &runtime.operation.trace_id,
                    runtime.cancellation,
                )?;
                emitted_events.extend(outcome.emitted_events);
            }
            ContractReviewStage::ReviewingAgent | ContractReviewStage::MergingFindings => {
                let extraction = review.extraction.as_ref().ok_or_else(|| {
                    HostError::new(
                        "EXTRACTION_REQUIRED",
                        "Agent review cannot continue without a persisted extraction",
                        false,
                    )
                })?;
                let outcome = match runtime.agent.review_with_cancellation(
                    &review,
                    extraction,
                    runtime.cancellation,
                ) {
                    Ok(result) => {
                        contract_review_service::replace_agent_findings_and_await_confirmation(
                            connection,
                            review_id,
                            &result.findings,
                            &result.evidence,
                            review.session.revision,
                            &runtime.operation.trace_id,
                        )?
                    }
                    Err(error)
                        if runtime.cancellation.is_cancelled()
                            || error.code == "CONTRACT_REVIEW_CANCELLED" =>
                    {
                        return Err(error);
                    }
                    Err(error) => {
                        let failure = ContractReviewFailure {
                            code: error.code,
                            message: error.message,
                            retryable: error.retryable,
                            stage: ContractReviewStage::ReviewingAgent,
                        };
                        contract_review_service::complete_agent_review_degraded(
                            connection,
                            review_id,
                            &failure,
                            review.session.revision,
                            &runtime.operation.trace_id,
                        )?
                    }
                };
                emitted_events.extend(outcome.emitted_events);
            }
            ContractReviewStage::Created
            | ContractReviewStage::AwaitingOcr
            | ContractReviewStage::AwaitingConfirmation
            | ContractReviewStage::Completed => return Ok(()),
        }
    }
    Err(HostError::internal(
        "contract review exceeded the local pipeline transition limit",
    ))
}

#[cfg(test)]
struct RuleOnlyTestAgent;

#[cfg(test)]
impl ContractAgentReviewer for RuleOnlyTestAgent {
    fn review(
        &self,
        _review: &ContractReviewRecord,
        _extraction: &crate::protocol::DocumentExtractionRecord,
    ) -> Result<crate::contract_review_agent::ContractAgentReviewResult, HostError> {
        Ok(crate::contract_review_agent::ContractAgentReviewResult {
            thread_id: "test-thread".to_string(),
            agent_run_id: "test-run".to_string(),
            findings: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

fn run_extraction_stage(
    connection: &mut Connection,
    vault_root: &Path,
    staging_root: &Path,
    review: &ContractReviewRecord,
    trace_id: &str,
    cancellation: &CancellationToken,
) -> Result<contract_review_service::ContractReviewMutationOutcome, HostError> {
    cancellation.check_cancelled()?;
    let source_asset = asset_service::get_asset(connection, &review.session.source_asset_id)?;
    if !source_asset
        .sha256
        .eq_ignore_ascii_case(&review.session.source_asset_sha256)
    {
        return Err(HostError::new(
            "CONTRACT_REVIEW_SOURCE_HASH_MISMATCH",
            "Local Vault asset hash no longer matches the frozen contract review source",
            false,
        ));
    }
    let project_id = source_asset
        .project_id
        .as_deref()
        .ok_or_else(|| HostError::validation("contract source asset must belong to a project"))?;
    let source_path =
        asset_service::resolve_original_path(connection, vault_root, &source_asset.id)?;
    let actual_sha256 = sha256_file_with_cancel(&source_path, cancellation)?;
    if !actual_sha256.eq_ignore_ascii_case(&source_asset.sha256)
        || !actual_sha256.eq_ignore_ascii_case(&review.session.source_asset_sha256)
    {
        return Err(HostError::new(
            "CONTRACT_REVIEW_SOURCE_HASH_MISMATCH",
            "contract source bytes no longer match the immutable Local Vault hash",
            false,
        ));
    }
    cancellation.check_cancelled()?;
    let mut extraction = DocumentIntelligence::with_defaults().extract_with_cancel(
        &review.session.id,
        &source_asset.id,
        &review.session.source_asset_sha256,
        &source_asset.mime_type,
        &source_path,
        now_ms()?,
        || Ok(cancellation.is_cancelled()),
    )?;
    cancellation.check_cancelled()?;

    let snapshot_path = write_json_staging(
        staging_root,
        &format!("contract-extraction-{}.json", extraction.id),
        &extraction,
    )?;
    cancellation.check_cancelled()?;
    let imported = asset_service::import_generated_artifact(
        connection,
        vault_root,
        project_id,
        &snapshot_path,
        GeneratedArtifactSource::ExtractionSnapshot,
        &extraction.id,
    );
    if imported.is_ok() {
        let _ = fs::remove_file(&snapshot_path);
    }
    let snapshot_asset = imported?;
    extraction.snapshot_asset_id = Some(snapshot_asset.id);
    cancellation.check_cancelled()?;

    contract_review_service::save_extraction(
        connection,
        &extraction,
        &[],
        review.session.revision,
        trace_id,
    )
}

fn run_report_stage(
    connection: &mut Connection,
    vault_root: &Path,
    staging_root: &Path,
    review: &ContractReviewRecord,
    format: ReviewReportFormat,
    trace_id: &str,
    cancellation: &CancellationToken,
) -> Result<contract_review_service::ContractReviewMutationOutcome, HostError> {
    cancellation.check_cancelled()?;
    let source_asset = asset_service::get_asset(connection, &review.session.source_asset_id)?;
    let project_id = source_asset
        .project_id
        .as_deref()
        .ok_or_else(|| HostError::validation("contract source asset must belong to a project"))?;
    let extraction = review.extraction.as_ref().ok_or_else(|| {
        HostError::new(
            "EXTRACTION_REQUIRED",
            "report generation requires a persisted extraction",
            false,
        )
    })?;
    let report_staging_root = staging_root.join("review-reports");
    let report_id = stable_report_id(&review.session.id, review.session.revision, format);
    let generated = review_report::generate_review_report_with_id(
        review,
        format,
        &report_staging_root,
        &report_id,
    )?;
    cancellation.check_cancelled()?;
    let source_ref = format!(
        "{}:{}:{:?}",
        review.session.id, review.session.revision, generated.format
    );
    let imported = asset_service::import_generated_artifact(
        connection,
        vault_root,
        project_id,
        &generated.path,
        GeneratedArtifactSource::ReviewReport,
        &source_ref,
    );
    if imported.is_ok() {
        let _ = fs::remove_file(&generated.path);
    }
    let report_asset = imported?;
    cancellation.check_cancelled()?;
    let agent_run_ids = review
        .findings
        .iter()
        .filter_map(|finding| finding.agent_run_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let report = ReviewReportRecord {
        id: generated.report_id,
        review_id: review.session.id.clone(),
        review_revision: review.session.revision,
        source_asset_id: review.session.source_asset_id.clone(),
        source_asset_sha256: review.session.source_asset_sha256.clone(),
        extraction_id: extraction.id.clone(),
        rule_set_version: RULE_SET_VERSION.to_string(),
        agent_run_ids,
        format: generated.format,
        report_asset_id: report_asset.id,
        report_asset_sha256: report_asset.sha256,
        generated_at: now_ms()?,
    };
    cancellation.check_cancelled()?;
    contract_review_service::save_report_and_complete(
        connection,
        &report,
        review.session.revision,
        trace_id,
    )
}

fn sha256_file_with_cancel(
    path: &Path,
    cancellation: &CancellationToken,
) -> Result<String, HostError> {
    let mut file = File::open(path).map_err(|error| {
        HostError::new(
            "CONTRACT_REVIEW_SOURCE_UNAVAILABLE",
            format!("unable to open contract source for hash verification: {error}"),
            true,
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        cancellation.check_cancelled()?;
        let read = file.read(&mut buffer).map_err(|error| {
            HostError::new(
                "CONTRACT_REVIEW_SOURCE_UNAVAILABLE",
                format!("unable to read contract source for hash verification: {error}"),
                true,
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    cancellation.check_cancelled()?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn stable_report_id(review_id: &str, review_revision: i64, format: ReviewReportFormat) -> String {
    let format_name = match format {
        ReviewReportFormat::Json => "json",
        ReviewReportFormat::Html => "html",
        ReviewReportFormat::Docx => "docx",
    };
    let mut hasher = Sha256::new();
    for part in [
        "bsaigc.contract-review-report.v1",
        review_id,
        &review_revision.to_string(),
        format_name,
    ] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}
fn queue_completed_review_backups(
    connection: &Connection,
    backup_outbox: &BackupOutbox,
    review: &ContractReviewRecord,
    context: &OperationContext,
    emitted_events: &mut Vec<BackupDomainEvent>,
    warnings: &mut Vec<String>,
) {
    let mut asset_ids = BTreeSet::new();
    asset_ids.insert(review.session.source_asset_id.clone());
    if let Some(extraction) = &review.extraction {
        if let Some(snapshot_asset_id) = &extraction.snapshot_asset_id {
            asset_ids.insert(snapshot_asset_id.clone());
        }
        for page in &extraction.pages {
            if let Some(preview_asset_id) = &page.preview_asset_id {
                asset_ids.insert(preview_asset_id.clone());
            }
        }
    }
    for report in &review.reports {
        asset_ids.insert(report.report_asset_id.clone());
    }

    for asset_id in asset_ids {
        let asset = match asset_service::get_asset(connection, &asset_id) {
            Ok(asset) => asset,
            Err(error) => {
                warnings.push(format!("backup intent skipped for {asset_id}: {error}"));
                continue;
            }
        };
        match backup_outbox.get(&asset.id) {
            Ok(Some(existing)) if existing.content_sha256.eq_ignore_ascii_case(&asset.sha256) => {
                continue;
            }
            Ok(Some(existing)) => {
                warnings.push(format!(
                    "backup intent skipped for {}: existing hash {} does not match {}",
                    asset.id, existing.content_sha256, asset.sha256
                ));
                continue;
            }
            Ok(None) => {}
            Err(error) => {
                warnings.push(format!(
                    "backup intent lookup failed for {}: {}",
                    asset.id, error
                ));
                continue;
            }
        }
        let command = BackupCommandEnvelope::Queue {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
            context: context.clone(),
            payload: QueueAssetBackupPayload {
                asset_id: asset.id.clone(),
            },
            idempotency_key: backup_idempotency_key(&review.session.id, &asset.id, &asset.sha256),
            expected_revision: None,
            deadline_at: None,
        };
        match backup_outbox.queue(command, &asset.sha256) {
            Ok(outcome) => emitted_events.extend(outcome.emitted_events),
            Err(error) => warnings.push(format!(
                "local review completed; asynchronous R2 backup queue failed for {}: {}",
                asset.id, error
            )),
        }
    }
}

fn backup_idempotency_key(review_id: &str, asset_id: &str, asset_sha256: &str) -> String {
    let mut hasher = Sha256::new();
    for part in [
        "bsaigc.contract-review-backup.v1",
        review_id,
        asset_id,
        asset_sha256,
    ] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    format!("contract-backup:{:x}", hasher.finalize())
}

fn persist_runtime_failure(
    connection: &mut Connection,
    review_id: &str,
    trace_id: &str,
    error: &HostError,
    emitted_events: &mut Vec<ContractReviewDomainEvent>,
) -> Result<(), HostError> {
    let review = contract_review_service::get_review(connection, review_id)?;
    if matches!(
        review.session.status,
        ContractReviewStatus::Completed | ContractReviewStatus::Cancelled
    ) {
        return Ok(());
    }
    let failure = ContractReviewFailure {
        code: error.code.clone(),
        message: error.message.clone(),
        retryable: error.retryable,
        stage: review.session.stage,
    };
    let outcome = contract_review_service::fail_review(
        connection,
        review_id,
        &failure,
        review.session.revision,
        trace_id,
    )?;
    emitted_events.extend(outcome.emitted_events);
    Ok(())
}

fn command_context(command: &ContractReviewCommandEnvelope) -> &OperationContext {
    match command {
        ContractReviewCommandEnvelope::Create { context, .. }
        | ContractReviewCommandEnvelope::Start { context, .. }
        | ContractReviewCommandEnvelope::Cancel { context, .. }
        | ContractReviewCommandEnvelope::DecideFinding { context, .. }
        | ContractReviewCommandEnvelope::GenerateReport { context, .. }
        | ContractReviewCommandEnvelope::RetryStage { context, .. } => context,
    }
}

fn command_report_format(command: &ContractReviewCommandEnvelope) -> Option<ReviewReportFormat> {
    match command {
        ContractReviewCommandEnvelope::GenerateReport { payload, .. } => Some(payload.format),
        _ => None,
    }
}

fn write_json_staging<T: Serialize>(
    staging_root: &Path,
    file_name: &str,
    value: &T,
) -> Result<PathBuf, HostError> {
    fs::create_dir_all(staging_root).map_err(|error| {
        HostError::new(
            "CONTRACT_ARTIFACT_STAGING_FAILED",
            format!("unable to prepare generated artifact staging: {error}"),
            true,
        )
    })?;
    let final_path = staging_root.join(file_name);
    let temporary_path = staging_root.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        HostError::new(
            "CONTRACT_ARTIFACT_SERIALIZATION_FAILED",
            format!("unable to serialize generated contract artifact: {error}"),
            false,
        )
    })?;
    let mut file = File::create(&temporary_path).map_err(|error| {
        HostError::new(
            "CONTRACT_ARTIFACT_STAGING_FAILED",
            format!("unable to create generated artifact staging file: {error}"),
            true,
        )
    })?;
    file.write_all(&bytes).map_err(|error| {
        HostError::new(
            "CONTRACT_ARTIFACT_STAGING_FAILED",
            format!("unable to write generated artifact staging file: {error}"),
            true,
        )
    })?;
    file.sync_all().map_err(|error| {
        HostError::new(
            "CONTRACT_ARTIFACT_STAGING_FAILED",
            format!("unable to sync generated artifact staging file: {error}"),
            true,
        )
    })?;
    fs::rename(&temporary_path, &final_path).map_err(|error| {
        HostError::new(
            "CONTRACT_ARTIFACT_STAGING_FAILED",
            format!("unable to commit generated artifact staging file: {error}"),
            true,
        )
    })?;
    Ok(final_path)
}

fn now_ms() -> Result<i64, HostError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| HostError::internal(format!("system clock before epoch: {error}")))?
        .as_millis();
    i64::try_from(millis).map_err(|_| HostError::internal("current timestamp exceeds SQLite i64"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        BackupState, CancelContractReviewPayload, ContractReviewEventType,
        CreateContractReviewPayload, DecideReviewFindingPayload, GenerateReviewReportPayload,
        RetryContractReviewStagePayload, ReviewFindingDecision, ReviewFindingStatus,
        ReviewSeverity, StartContractReviewPayload, CONTRACT_REVIEW_PROTOCOL_VERSION,
    };
    use serde::Deserialize;
    use std::collections::BTreeMap;
    use std::io::{Cursor, Read};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::time::Instant;
    use tempfile::TempDir;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipArchive, ZipWriter};

    struct TestHarness {
        _temporary: TempDir,
        connection: Connection,
        vault_root: PathBuf,
        staging_root: PathBuf,
        backup_database_path: PathBuf,
        project_id: String,
        workspace_id: String,
        source_asset_id: String,
        source_bytes: Vec<u8>,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct QaFixtureManifest {
        schema_version: String,
        fixtures: Vec<QaFixtureSpec>,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct QaFixtureSpec {
        id: String,
        file: String,
        sha256: String,
        byte_size: u64,
        expected_risk_level: String,
        expected_risks: Vec<QaExpectedRisk>,
        required_business_sections: Vec<String>,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct QaExpectedRisk {
        code: String,
        severity: String,
        evidence_contains: Vec<String>,
    }

    struct FailingAgent;

    impl ContractAgentReviewer for FailingAgent {
        fn review(
            &self,
            _review: &ContractReviewRecord,
            _extraction: &crate::protocol::DocumentExtractionRecord,
        ) -> Result<crate::contract_review_agent::ContractAgentReviewResult, HostError> {
            Err(HostError::new(
                "BRAIN_RUNTIME_UNAVAILABLE",
                "test Agent unavailable",
                true,
            ))
        }
    }

    struct MissingCredentialAgent;

    impl ContractAgentReviewer for MissingCredentialAgent {
        fn review(
            &self,
            _review: &ContractReviewRecord,
            _extraction: &crate::protocol::DocumentExtractionRecord,
        ) -> Result<crate::contract_review_agent::ContractAgentReviewResult, HostError> {
            Err(HostError::new(
                "CONTRACT_AGENT_TURN_FAILED",
                "Missing environment variable: BSAIGC_CODEX_API_KEY",
                true,
            ))
        }
    }

    #[derive(Clone)]
    struct BlockingDetachedAgent {
        started: Arc<AtomicBool>,
        cancellation_observed: Arc<AtomicBool>,
    }

    impl BlockingDetachedAgent {
        fn new() -> Self {
            Self {
                started: Arc::new(AtomicBool::new(false)),
                cancellation_observed: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    impl ContractAgentReviewer for BlockingDetachedAgent {
        fn review(
            &self,
            _review: &ContractReviewRecord,
            _extraction: &crate::protocol::DocumentExtractionRecord,
        ) -> Result<crate::contract_review_agent::ContractAgentReviewResult, HostError> {
            Err(HostError::internal(
                "blocking detached test agent must use cancellation-aware review",
            ))
        }

        fn review_with_cancellation(
            &self,
            _review: &ContractReviewRecord,
            _extraction: &crate::protocol::DocumentExtractionRecord,
            cancellation: &CancellationToken,
        ) -> Result<crate::contract_review_agent::ContractAgentReviewResult, HostError> {
            self.started.store(true, Ordering::Release);
            let deadline = Instant::now() + std::time::Duration::from_secs(10);
            loop {
                if cancellation.is_cancelled() {
                    self.cancellation_observed.store(true, Ordering::Release);
                    return Err(HostError::new(
                        "CONTRACT_REVIEW_CANCELLED",
                        "blocking detached test agent observed cancellation",
                        false,
                    ));
                }
                if Instant::now() >= deadline {
                    return Err(HostError::new(
                        "TEST_AGENT_TIMEOUT",
                        "blocking detached test agent timed out",
                        false,
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        fn detached_clone(&self) -> Option<Box<dyn ContractAgentReviewer + Send + Sync>> {
            Some(Box::new(self.clone()))
        }
    }

    fn setup_harness() -> TestHarness {
        let temporary = tempfile::tempdir().unwrap();
        let vault_root = temporary.path().join("vault");
        let staging_root = temporary.path().join("staging");
        let backup_database_path = temporary.path().join("backup-outbox.sqlite3");
        let business_database_path = temporary.path().join("business.sqlite3");
        let source_path = temporary.path().join("customer-contract.docx");
        let project_id = Uuid::new_v4().to_string();
        let workspace_id = Uuid::new_v4().to_string();

        write_contract_docx(&source_path);
        let source_bytes = fs::read(&source_path).unwrap();

        let mut connection = Connection::open(business_database_path).unwrap();
        connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys = ON;
                CREATE TABLE business_workspaces (
                    id TEXT PRIMARY KEY NOT NULL
                );
                "#,
            )
            .unwrap();
        asset_service::migrate(&connection).unwrap();
        contract_review_service::migrate(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO business_workspaces (id) VALUES (?1)",
                [&workspace_id],
            )
            .unwrap();

        let source_asset = asset_service::import_file(
            &mut connection,
            &vault_root,
            Some(&project_id),
            &source_path,
        )
        .unwrap();
        fs::remove_file(&source_path).unwrap();

        TestHarness {
            _temporary: temporary,
            connection,
            vault_root,
            staging_root,
            backup_database_path,
            project_id,
            workspace_id,
            source_asset_id: source_asset.id,
            source_bytes,
        }
    }

    fn setup_fixture_harness(source_path: &Path) -> TestHarness {
        let temporary = tempfile::tempdir().unwrap();
        let vault_root = temporary.path().join("vault");
        let staging_root = temporary.path().join("staging");
        let backup_database_path = temporary.path().join("backup-outbox.sqlite3");
        let business_database_path = temporary.path().join("business.sqlite3");
        let project_id = Uuid::new_v4().to_string();
        let workspace_id = Uuid::new_v4().to_string();
        let source_bytes = fs::read(source_path).unwrap();

        let mut connection = Connection::open(business_database_path).unwrap();
        connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys = ON;
                CREATE TABLE business_workspaces (
                    id TEXT PRIMARY KEY NOT NULL
                );
                "#,
            )
            .unwrap();
        asset_service::migrate(&connection).unwrap();
        contract_review_service::migrate(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO business_workspaces (id) VALUES (?1)",
                [&workspace_id],
            )
            .unwrap();

        let source_asset = asset_service::import_file(
            &mut connection,
            &vault_root,
            Some(&project_id),
            source_path,
        )
        .unwrap();

        TestHarness {
            _temporary: temporary,
            connection,
            vault_root,
            staging_root,
            backup_database_path,
            project_id,
            workspace_id,
            source_asset_id: source_asset.id,
            source_bytes,
        }
    }

    fn write_contract_docx(path: &Path) {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>视频制作服务合同</w:t></w:r></w:p>
    <w:p><w:r><w:t>甲方：示例客户有限公司</w:t></w:r></w:p>
    <w:p><w:r><w:t>乙方：华邦文化传媒有限公司</w:t></w:r></w:p>
    <w:p><w:r><w:t>合同金额：人民币10000元，含税。</w:t></w:r></w:p>
    <w:p><w:r><w:t>付款安排：签约后支付50%，成片交付后支付剩余50%。</w:t></w:r></w:p>
    <w:p><w:r><w:t>双方按照确认的脚本完成拍摄、剪辑和成片交付。</w:t></w:r></w:p>
  </w:body>
</w:document>"#;
        let file = File::create(path).unwrap();
        let mut archive = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o644);
        archive.start_file("word/document.xml", options).unwrap();
        archive.write_all(document_xml.as_bytes()).unwrap();
        archive.finish().unwrap();
    }

    fn context(trace_id: &str, project_id: &str) -> OperationContext {
        OperationContext {
            actor_id: "contract-review-tester".to_string(),
            account_id: Some("account-local".to_string()),
            project_id: Some(project_id.to_string()),
            window_id: "window-contract-review".to_string(),
            trace_id: trace_id.to_string(),
        }
    }

    fn create_command(harness: &TestHarness) -> ContractReviewCommandEnvelope {
        ContractReviewCommandEnvelope::Create {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: CONTRACT_REVIEW_PROTOCOL_VERSION.to_string(),
            context: context("trace-contract-create", &harness.project_id),
            payload: CreateContractReviewPayload {
                workspace_id: harness.workspace_id.clone(),
                source_asset_id: harness.source_asset_id.clone(),
            },
            idempotency_key: format!("contract-create-{}", harness.source_asset_id),
            expected_revision: None,
            deadline_at: None,
        }
    }

    fn start_command(
        project_id: &str,
        review_id: &str,
        revision: i64,
    ) -> ContractReviewCommandEnvelope {
        ContractReviewCommandEnvelope::Start {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: CONTRACT_REVIEW_PROTOCOL_VERSION.to_string(),
            context: context("trace-contract-start", project_id),
            payload: StartContractReviewPayload {
                review_id: review_id.to_string(),
            },
            idempotency_key: format!("contract-start-{review_id}"),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }

    fn cancel_command(
        project_id: &str,
        review_id: &str,
        revision: i64,
    ) -> ContractReviewCommandEnvelope {
        ContractReviewCommandEnvelope::Cancel {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: CONTRACT_REVIEW_PROTOCOL_VERSION.to_string(),
            context: context("trace-contract-cancel", project_id),
            payload: CancelContractReviewPayload {
                review_id: review_id.to_string(),
                reason: "operator cancelled the running review".to_string(),
            },
            idempotency_key: format!("contract-cancel-{review_id}-{revision}"),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }

    fn decide_command(
        project_id: &str,
        review_id: &str,
        finding_id: &str,
        finding_revision: i64,
    ) -> ContractReviewCommandEnvelope {
        ContractReviewCommandEnvelope::DecideFinding {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: CONTRACT_REVIEW_PROTOCOL_VERSION.to_string(),
            context: context("trace-contract-decide", project_id),
            payload: DecideReviewFindingPayload {
                review_id: review_id.to_string(),
                finding_id: finding_id.to_string(),
                decision: ReviewFindingDecision::Confirmed,
                comment: "确认缺少验收条款，要求补充后签署。".to_string(),
            },
            idempotency_key: format!("contract-decide-{finding_id}-{finding_revision}"),
            expected_revision: Some(finding_revision),
            deadline_at: None,
        }
    }

    fn generate_report_command(
        project_id: &str,
        review_id: &str,
        revision: i64,
    ) -> ContractReviewCommandEnvelope {
        ContractReviewCommandEnvelope::GenerateReport {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: CONTRACT_REVIEW_PROTOCOL_VERSION.to_string(),
            context: context("trace-contract-report", project_id),
            payload: GenerateReviewReportPayload {
                review_id: review_id.to_string(),
                format: ReviewReportFormat::Html,
            },
            idempotency_key: format!("contract-report-{review_id}-{revision}"),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }

    fn generate_report_command_for_format(
        project_id: &str,
        review_id: &str,
        revision: i64,
        format: ReviewReportFormat,
    ) -> ContractReviewCommandEnvelope {
        ContractReviewCommandEnvelope::GenerateReport {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: CONTRACT_REVIEW_PROTOCOL_VERSION.to_string(),
            context: context("trace-contract-qa-report", project_id),
            payload: GenerateReviewReportPayload {
                review_id: review_id.to_string(),
                format,
            },
            idempotency_key: format!(
                "contract-qa-report-{review_id}-{revision}-{}",
                report_format_name(format)
            ),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }

    fn report_format_name(format: ReviewReportFormat) -> &'static str {
        match format {
            ReviewReportFormat::Json => "json",
            ReviewReportFormat::Html => "html",
            ReviewReportFormat::Docx => "docx",
        }
    }

    fn retry_agent_command(
        project_id: &str,
        review_id: &str,
        revision: i64,
    ) -> ContractReviewCommandEnvelope {
        ContractReviewCommandEnvelope::RetryStage {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: CONTRACT_REVIEW_PROTOCOL_VERSION.to_string(),
            context: context("trace-contract-agent-retry", project_id),
            payload: RetryContractReviewStagePayload {
                review_id: review_id.to_string(),
                stage: ContractReviewStage::ReviewingAgent,
            },
            idempotency_key: format!("contract-agent-retry-{review_id}-{revision}"),
            expected_revision: Some(revision),
            deadline_at: None,
        }
    }

    fn prepare_review_for_report(
        harness: &mut TestHarness,
        backup_outbox: &BackupOutbox,
    ) -> ContractReviewRecord {
        let create = create_command(harness);
        let created = execute_contract_review_command(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            backup_outbox,
            create,
        )
        .unwrap();
        let created_review = created.response.contract_review;
        let started = execute_contract_review_command(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            backup_outbox,
            start_command(
                &harness.project_id,
                &created_review.session.id,
                created_review.session.revision,
            ),
        )
        .unwrap();
        let mut review = started.response.contract_review;

        assert_eq!(
            review.session.status,
            ContractReviewStatus::AwaitingConfirmation
        );
        assert_eq!(
            review.session.stage,
            ContractReviewStage::AwaitingConfirmation
        );
        assert!(review.extraction.is_some());
        assert!(!review.findings.is_empty());
        assert!(!review.evidence.is_empty());
        assert!(review
            .rule_evaluations
            .iter()
            .any(|evaluation| !evaluation.finding_ids.is_empty()));

        let evidence = contract_review_service::get_evidence_context(
            &harness.connection,
            &review.evidence[0].id,
        )
        .unwrap();
        assert_eq!(evidence.evidence.source_asset_id, harness.source_asset_id);
        assert_eq!(evidence.page.page_index, evidence.evidence.page_index);
        assert!(evidence.block.is_some());
        assert!(!evidence.evidence.quoted_text.is_empty());

        for finding in review.findings.clone() {
            let decided = execute_contract_review_command(
                &mut harness.connection,
                &harness.vault_root,
                &harness.staging_root,
                backup_outbox,
                decide_command(
                    &harness.project_id,
                    &review.session.id,
                    &finding.id,
                    finding.revision,
                ),
            )
            .unwrap();
            review = decided.response.contract_review;
        }

        assert!(review.findings.iter().all(|finding| {
            finding.status == ReviewFindingStatus::Decided
                && finding.decision == ReviewFindingDecision::Confirmed
        }));
        assert_eq!(review.decisions.len(), review.findings.len());
        review
    }

    fn generate_completed_review(
        harness: &mut TestHarness,
        backup_outbox: &BackupOutbox,
        review: &ContractReviewRecord,
    ) -> ContractReviewRuntimeOutcome {
        execute_contract_review_command(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            backup_outbox,
            generate_report_command(
                &harness.project_id,
                &review.session.id,
                review.session.revision,
            ),
        )
        .unwrap()
    }

    fn assert_local_artifacts_are_readable(
        harness: &TestHarness,
        review: &ContractReviewRecord,
    ) -> BTreeSet<String> {
        let source_path = asset_service::resolve_original_path(
            &harness.connection,
            &harness.vault_root,
            &review.session.source_asset_id,
        )
        .unwrap();
        assert_eq!(fs::read(source_path).unwrap(), harness.source_bytes);

        let extraction = review.extraction.as_ref().unwrap();
        let snapshot_asset_id = extraction.snapshot_asset_id.as_ref().unwrap();
        let snapshot_path = asset_service::resolve_original_path(
            &harness.connection,
            &harness.vault_root,
            snapshot_asset_id,
        )
        .unwrap();
        let snapshot: crate::protocol::DocumentExtractionRecord =
            serde_json::from_slice(&fs::read(snapshot_path).unwrap()).unwrap();
        assert_eq!(snapshot.id, extraction.id);
        assert_eq!(snapshot.source_asset_id, review.session.source_asset_id);
        assert_eq!(
            snapshot.source_asset_sha256,
            review.session.source_asset_sha256
        );
        assert!(snapshot.snapshot_asset_id.is_none());

        assert_eq!(review.reports.len(), 1);
        let report = &review.reports[0];
        assert_eq!(
            report.id,
            stable_report_id(&review.session.id, report.review_revision, report.format)
        );
        assert_eq!(
            review.session.report_asset_id.as_deref(),
            Some(report.report_asset_id.as_str())
        );
        let report_path = asset_service::resolve_original_path(
            &harness.connection,
            &harness.vault_root,
            &report.report_asset_id,
        )
        .unwrap();
        let report_bytes = fs::read(report_path).unwrap();
        let report_sha256 = format!("{:x}", Sha256::digest(&report_bytes));
        assert_eq!(report_sha256, report.report_asset_sha256);
        let report_html = String::from_utf8(report_bytes).unwrap();
        assert!(report_html.contains(&report.id));
        assert!(report_html.contains("合同审查报告"));
        assert!(report_html.contains("Confirmed"));

        BTreeSet::from([
            review.session.source_asset_id.clone(),
            snapshot_asset_id.clone(),
            report.report_asset_id.clone(),
        ])
    }

    fn qa_fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(".runtime")
            .join("qa-fixtures")
    }

    fn load_qa_fixture(fixture_id: &str) -> QaFixtureSpec {
        let manifest_path = qa_fixture_root().join("manifest.json");
        let manifest: QaFixtureManifest =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        assert_eq!(
            manifest.schema_version,
            "bsaigc.business-qa-fixtures.v1",
            "unexpected QA fixture manifest schema at {}",
            manifest_path.display()
        );
        manifest
            .fixtures
            .into_iter()
            .find(|fixture| fixture.id == fixture_id)
            .unwrap_or_else(|| {
                panic!(
                    "fixture {fixture_id} missing from {}",
                    manifest_path.display()
                )
            })
    }

    fn parse_manifest_severity(value: &str) -> ReviewSeverity {
        match value {
            "info" => ReviewSeverity::Info,
            "low" => ReviewSeverity::Low,
            "medium" => ReviewSeverity::Medium,
            "high" => ReviewSeverity::High,
            "critical" => ReviewSeverity::Critical,
            other => panic!("unsupported manifest severity {other}"),
        }
    }

    fn review_risk_level(review: &ContractReviewRecord) -> &'static str {
        let rank = review
            .findings
            .iter()
            .map(|finding| match finding.severity {
                ReviewSeverity::Info => 0,
                ReviewSeverity::Low => 1,
                ReviewSeverity::Medium => 2,
                ReviewSeverity::High => 3,
                ReviewSeverity::Critical => 4,
            })
            .max()
            .unwrap_or(1);
        match rank {
            0 => "info",
            1 => "low",
            2 => "medium",
            3 => "high",
            _ => "critical",
        }
    }

    fn assert_fixture_extraction(
        harness: &TestHarness,
        fixture: &QaFixtureSpec,
        review: &ContractReviewRecord,
    ) {
        let extraction = review.extraction.as_ref().expect("DOCX extraction missing");
        assert_eq!(
            extraction.parser.name,
            crate::document_intelligence::DOCX_PARSER_NAME
        );
        assert_eq!(
            extraction.parser.version,
            crate::document_intelligence::DOCX_PARSER_VERSION
        );
        assert_eq!(extraction.source_asset_id, harness.source_asset_id);
        assert_eq!(extraction.source_asset_sha256, fixture.sha256);
        assert_eq!(review.session.source_asset_sha256, fixture.sha256);
        assert!(!extraction.pages.is_empty());
        assert!(!extraction.blocks.is_empty());
        let text = extraction
            .pages
            .iter()
            .map(|page| page.text.as_str())
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        assert!(
            text.chars()
                .any(|value| ('\u{4e00}'..='\u{9fff}').contains(&value)),
            "fixture {} lost Chinese text during DOCX extraction",
            fixture.id
        );
        for section in &fixture.required_business_sections {
            assert!(
                text.contains(section),
                "fixture {} extraction missing required section {section}",
                fixture.id
            );
        }
        for expected in &fixture.expected_risks {
            for phrase in &expected.evidence_contains {
                assert!(
                    text.contains(phrase),
                    "fixture {} extraction missing expected evidence phrase {phrase}",
                    fixture.id
                );
            }
        }
    }

    fn assert_manifest_findings(
        harness: &TestHarness,
        fixture: &QaFixtureSpec,
        review: &ContractReviewRecord,
    ) {
        assert_eq!(review_risk_level(review), fixture.expected_risk_level);
        assert!(review.findings.iter().all(|finding| {
            finding.source == crate::protocol::ReviewFindingSource::Rule
                && finding.status == ReviewFindingStatus::Open
                && finding.decision == ReviewFindingDecision::Unreviewed
        }));

        let expected_codes = fixture
            .expected_risks
            .iter()
            .map(|risk| risk.code.clone())
            .collect::<BTreeSet<_>>();
        let actual_by_code = review
            .findings
            .iter()
            .map(|finding| {
                (
                    finding
                        .rule_id
                        .clone()
                        .expect("rule finding must preserve manifest code as ruleId"),
                    finding,
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            actual_by_code.keys().cloned().collect::<BTreeSet<_>>(),
            expected_codes,
            "fixture {} produced an unexpected risk set",
            fixture.id
        );

        for expected in &fixture.expected_risks {
            let finding = actual_by_code
                .get(&expected.code)
                .unwrap_or_else(|| panic!("fixture {} missing risk {}", fixture.id, expected.code));
            assert_eq!(
                finding.severity,
                parse_manifest_severity(&expected.severity),
                "fixture {} risk {} severity mismatch",
                fixture.id,
                expected.code
            );
            assert!(!finding.evidence_ids.is_empty());
            for phrase in &expected.evidence_contains {
                let mut located = false;
                for evidence_id in &finding.evidence_ids {
                    let context = contract_review_service::get_evidence_context(
                        &harness.connection,
                        evidence_id,
                    )
                    .unwrap();
                    assert_eq!(context.evidence.source_asset_id, harness.source_asset_id);
                    assert_eq!(context.page.page_index, context.evidence.page_index);
                    assert!(context.block.is_some());
                    let original_context = format!(
                        "{}{}{}",
                        context.evidence.context_before,
                        context.evidence.quoted_text,
                        context.evidence.context_after
                    );
                    if original_context.contains(phrase) {
                        located = true;
                        break;
                    }
                }
                assert!(
                    located,
                    "fixture {} risk {} did not retain Evidence for {phrase}",
                    fixture.id, expected.code
                );
            }
        }
    }

    fn assert_fixture_artifacts_are_readable(
        harness: &TestHarness,
        fixture: &QaFixtureSpec,
        review: &ContractReviewRecord,
        format: ReviewReportFormat,
    ) -> BTreeSet<String> {
        let source_path = asset_service::resolve_original_path(
            &harness.connection,
            &harness.vault_root,
            &review.session.source_asset_id,
        )
        .unwrap();
        let source_bytes = fs::read(source_path).unwrap();
        assert_eq!(source_bytes, harness.source_bytes);
        assert_eq!(source_bytes.len() as u64, fixture.byte_size);
        assert_eq!(
            format!("{:x}", Sha256::digest(&source_bytes)),
            fixture.sha256
        );

        let extraction = review.extraction.as_ref().unwrap();
        let snapshot_asset_id = extraction.snapshot_asset_id.as_ref().unwrap();
        let snapshot_path = asset_service::resolve_original_path(
            &harness.connection,
            &harness.vault_root,
            snapshot_asset_id,
        )
        .unwrap();
        let snapshot: crate::protocol::DocumentExtractionRecord =
            serde_json::from_slice(&fs::read(snapshot_path).unwrap()).unwrap();
        assert_eq!(snapshot.id, extraction.id);
        assert_eq!(snapshot.source_asset_id, review.session.source_asset_id);
        assert_eq!(snapshot.source_asset_sha256, fixture.sha256);
        assert!(snapshot.snapshot_asset_id.is_none());

        assert_eq!(review.reports.len(), 1);
        let report = &review.reports[0];
        assert_eq!(report.format, format);
        assert_eq!(report.source_asset_sha256, fixture.sha256);
        assert_eq!(report.rule_set_version, RULE_SET_VERSION);
        let report_path = asset_service::resolve_original_path(
            &harness.connection,
            &harness.vault_root,
            &report.report_asset_id,
        )
        .unwrap();
        let report_bytes = fs::read(report_path).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&report_bytes)),
            report.report_asset_sha256
        );

        match format {
            ReviewReportFormat::Html => {
                let html = String::from_utf8(report_bytes).unwrap();
                assert!(html.contains("合同审查报告"));
                assert!(html.contains(&report.id));
                if !fixture.expected_risks.is_empty() {
                    assert!(html.contains("Confirmed"));
                }
            }
            ReviewReportFormat::Docx => {
                let mut archive = ZipArchive::new(Cursor::new(report_bytes)).unwrap();
                for required in [
                    "[Content_Types].xml",
                    "_rels/.rels",
                    "docProps/core.xml",
                    "word/document.xml",
                    "word/styles.xml",
                    "word/_rels/document.xml.rels",
                ] {
                    assert!(archive.by_name(required).is_ok(), "missing {required}");
                }
                let mut document_xml = String::new();
                archive
                    .by_name("word/document.xml")
                    .unwrap()
                    .read_to_string(&mut document_xml)
                    .unwrap();
                assert!(document_xml.contains("合同审查报告"));
                assert!(document_xml.contains(&report.id));
                assert!(document_xml.contains(&fixture.sha256));
                if !fixture.expected_risks.is_empty() {
                    assert!(document_xml.contains("Confirmed"));
                }
            }
            ReviewReportFormat::Json => panic!("QA fixture closure requires HTML or DOCX"),
        }

        BTreeSet::from([
            review.session.source_asset_id.clone(),
            snapshot_asset_id.clone(),
            report.report_asset_id.clone(),
        ])
    }

    fn run_real_docx_fixture_closed_loop(fixture_id: &str) {
        let fixture = load_qa_fixture(fixture_id);
        let fixture_path = qa_fixture_root().join(&fixture.file);
        let fixture_bytes = fs::read(&fixture_path).unwrap();
        assert_eq!(fixture_bytes.len() as u64, fixture.byte_size);
        assert_eq!(
            format!("{:x}", Sha256::digest(&fixture_bytes)),
            fixture.sha256
        );

        for format in [ReviewReportFormat::Html, ReviewReportFormat::Docx] {
            let mut harness = setup_fixture_harness(&fixture_path);
            let backup_outbox = BackupOutbox::open(&harness.backup_database_path).unwrap();
            let started = start_with_agent(&mut harness, &backup_outbox, &MissingCredentialAgent);

            assert_eq!(
                started.session.status,
                ContractReviewStatus::AwaitingConfirmation
            );
            assert_eq!(
                started.session.stage,
                ContractReviewStage::AwaitingConfirmation
            );
            let failure = started
                .session
                .failure
                .as_ref()
                .expect("missing API key must degrade instead of failing the review");
            assert_eq!(failure.code, "CONTRACT_AGENT_TURN_FAILED");
            assert_eq!(failure.stage, ContractReviewStage::ReviewingAgent);
            assert!(failure.retryable);
            assert_fixture_extraction(&harness, &fixture, &started);
            assert_manifest_findings(&harness, &fixture, &started);

            let decided = decide_all_findings_with_agent(
                &mut harness,
                &backup_outbox,
                &MissingCredentialAgent,
                started,
            );
            assert_eq!(decided.decisions.len(), decided.findings.len());
            assert!(decided.findings.iter().all(|finding| {
                finding.status == ReviewFindingStatus::Decided
                    && finding.decision == ReviewFindingDecision::Confirmed
            }));

            let outcome = execute_contract_review_command_with_agent(
                &mut harness.connection,
                &harness.vault_root,
                &harness.staging_root,
                &backup_outbox,
                generate_report_command_for_format(
                    &harness.project_id,
                    &decided.session.id,
                    decided.session.revision,
                    format,
                ),
                &MissingCredentialAgent,
            )
            .unwrap();
            let completed = &outcome.response.contract_review;
            assert_eq!(completed.session.status, ContractReviewStatus::Completed);
            assert_eq!(completed.session.stage, ContractReviewStage::Completed);
            assert!(completed.session.failure.is_none());
            assert!(completed.session.completed_at.is_some());

            let expected_asset_ids =
                assert_fixture_artifacts_are_readable(&harness, &fixture, completed, format);
            let backups = backup_outbox.list(10).unwrap();
            assert_eq!(backups.len(), expected_asset_ids.len());
            assert!(backups
                .iter()
                .all(|backup| backup.state == BackupState::Queued));
            assert_eq!(
                backups
                    .iter()
                    .map(|backup| backup.asset_id.clone())
                    .collect::<BTreeSet<_>>(),
                expected_asset_ids
            );
            assert_eq!(
                backups
                    .iter()
                    .find(|backup| backup.asset_id == harness.source_asset_id)
                    .unwrap()
                    .content_sha256,
                fixture.sha256
            );
            assert_eq!(outcome.backup_events.len(), backups.len());
            assert!(outcome.backup_warnings.is_empty());
        }
    }

    #[test]
    fn start_returns_quickly_and_second_connection_cancel_stops_worker_without_failed_event() {
        let mut harness = setup_harness();
        let backup_outbox = BackupOutbox::open(&harness.backup_database_path).unwrap();
        let agent = BlockingDetachedAgent::new();
        let create = create_command(&harness);
        let created = execute_contract_review_command_with_agent(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            &backup_outbox,
            create,
            &agent,
        )
        .unwrap()
        .response
        .contract_review;
        let database_path = sqlite_main_database_path(&harness.connection)
            .unwrap()
            .expect("test harness uses a file-backed SQLite database");

        let started_at = Instant::now();
        let start_outcome = execute_contract_review_command_with_agent(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            &backup_outbox,
            start_command(
                &harness.project_id,
                &created.session.id,
                created.session.revision,
            ),
            &agent,
        )
        .unwrap();
        assert!(
            started_at.elapsed() < std::time::Duration::from_secs(2),
            "detached Start took {:?}",
            started_at.elapsed()
        );
        assert_eq!(
            start_outcome.response.contract_review.session.status,
            ContractReviewStatus::Running
        );

        let agent_deadline = Instant::now() + std::time::Duration::from_secs(5);
        while !agent.started.load(Ordering::Acquire) && Instant::now() < agent_deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            agent.started.load(Ordering::Acquire),
            "detached agent did not reach the cancellable review stage"
        );

        let mut second_connection = open_worker_connection(&database_path).unwrap();
        let running =
            contract_review_service::get_review(&second_connection, &created.session.id).unwrap();
        assert_eq!(running.session.status, ContractReviewStatus::Running);
        let cancelled = execute_contract_review_command_with_agent(
            &mut second_connection,
            &harness.vault_root,
            &harness.staging_root,
            &backup_outbox,
            cancel_command(
                &harness.project_id,
                &running.session.id,
                running.session.revision,
            ),
            &agent,
        )
        .unwrap()
        .response
        .contract_review;
        assert_eq!(cancelled.session.status, ContractReviewStatus::Cancelled);

        let cancel_deadline = Instant::now() + std::time::Duration::from_secs(5);
        while !agent.cancellation_observed.load(Ordering::Acquire)
            && Instant::now() < cancel_deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            agent.cancellation_observed.load(Ordering::Acquire),
            "detached agent did not observe the shared cancellation token"
        );

        let active_key = ActiveReviewKey {
            database_path: database_path.clone(),
            review_id: created.session.id.clone(),
        };
        let unregister_deadline = Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let still_active = active_reviews()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .contains_key(&active_key);
            if !still_active || Instant::now() >= unregister_deadline {
                assert!(!still_active, "cancelled detached worker stayed registered");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let persisted =
            contract_review_service::get_review(&harness.connection, &created.session.id).unwrap();
        assert_eq!(persisted.session.status, ContractReviewStatus::Cancelled);
        let events = contract_review_service::replay_events(&harness.connection, 0, 100).unwrap();
        assert!(events
            .iter()
            .any(|event| event.event_type == ContractReviewEventType::Cancelled));
        assert!(!events
            .iter()
            .any(|event| event.event_type == ContractReviewEventType::Failed));
    }

    #[test]
    fn source_file_tampering_fails_before_extraction_is_persisted() {
        let mut harness = setup_harness();
        let backup_outbox = BackupOutbox::open(&harness.backup_database_path).unwrap();
        let create = create_command(&harness);
        let created = execute_contract_review_command(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            &backup_outbox,
            create,
        )
        .unwrap()
        .response
        .contract_review;
        let source_path = asset_service::resolve_original_path(
            &harness.connection,
            &harness.vault_root,
            &harness.source_asset_id,
        )
        .unwrap();
        fs::write(&source_path, b"tampered contract bytes").unwrap();

        let failed = execute_contract_review_command(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            &backup_outbox,
            start_command(
                &harness.project_id,
                &created.session.id,
                created.session.revision,
            ),
        )
        .unwrap()
        .response
        .contract_review;

        assert_eq!(failed.session.status, ContractReviewStatus::Failed);
        assert_eq!(
            failed.session.failure.as_ref().unwrap().code,
            "CONTRACT_REVIEW_SOURCE_HASH_MISMATCH"
        );
        assert!(failed.extraction.is_none());
        let events = contract_review_service::replay_events(&harness.connection, 0, 100).unwrap();
        assert!(events
            .iter()
            .any(|event| event.event_type == ContractReviewEventType::Failed));
        assert!(!events
            .iter()
            .any(|event| { event.event_type == ContractReviewEventType::ExtractionCompleted }));
    }

    #[test]
    fn real_standard_low_risk_docx_completes_html_and_docx_closure() {
        run_real_docx_fixture_closed_loop("contract-standard-low-risk");
    }

    #[test]
    fn real_high_risk_docx_matches_manifest_and_completes_html_and_docx_closure() {
        run_real_docx_fixture_closed_loop("contract-high-risk");
    }

    #[test]
    fn real_missing_fields_docx_matches_manifest_and_completes_html_and_docx_closure() {
        run_real_docx_fixture_closed_loop("contract-missing-fields");
    }

    #[test]
    fn completed_review_can_append_a_second_report_format() {
        let fixture = load_qa_fixture("contract-standard-low-risk");
        let fixture_path = qa_fixture_root().join(&fixture.file);
        let mut harness = setup_fixture_harness(&fixture_path);
        let backup_outbox = BackupOutbox::open(&harness.backup_database_path).unwrap();
        let started = start_with_agent(&mut harness, &backup_outbox, &MissingCredentialAgent);
        let decided = decide_all_findings_with_agent(
            &mut harness,
            &backup_outbox,
            &MissingCredentialAgent,
            started,
        );

        let html = execute_contract_review_command_with_agent(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            &backup_outbox,
            generate_report_command_for_format(
                &harness.project_id,
                &decided.session.id,
                decided.session.revision,
                ReviewReportFormat::Html,
            ),
            &MissingCredentialAgent,
        )
        .unwrap()
        .response
        .contract_review;
        let first_completed_at = html.session.completed_at;
        assert_eq!(html.reports.len(), 1);
        assert_eq!(html.reports[0].format, ReviewReportFormat::Html);

        let outcome = execute_contract_review_command_with_agent(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            &backup_outbox,
            generate_report_command_for_format(
                &harness.project_id,
                &html.session.id,
                html.session.revision,
                ReviewReportFormat::Docx,
            ),
            &MissingCredentialAgent,
        )
        .unwrap();
        let completed = &outcome.response.contract_review;
        assert_eq!(completed.session.status, ContractReviewStatus::Completed);
        assert_eq!(completed.session.stage, ContractReviewStage::Completed);
        assert_eq!(completed.session.completed_at, first_completed_at);
        assert_eq!(completed.reports.len(), 2);
        assert!(completed
            .reports
            .iter()
            .any(|report| report.format == ReviewReportFormat::Html));
        assert!(completed
            .reports
            .iter()
            .any(|report| report.format == ReviewReportFormat::Docx));
        for report in &completed.reports {
            let path = asset_service::resolve_original_path(
                &harness.connection,
                &harness.vault_root,
                &report.report_asset_id,
            )
            .unwrap();
            let bytes = fs::read(path).unwrap();
            assert!(!bytes.is_empty());
        }
        let backups = backup_outbox.list(10).unwrap();
        assert_eq!(backups.len(), 4);
        assert!(backups
            .iter()
            .all(|backup| backup.state == BackupState::Queued));
        assert!(outcome.backup_warnings.is_empty());
    }

    #[test]
    fn local_contract_review_closes_before_r2_and_queues_all_vault_artifacts() {
        let mut harness = setup_harness();
        let backup_outbox = BackupOutbox::open(&harness.backup_database_path).unwrap();
        let review = prepare_review_for_report(&mut harness, &backup_outbox);
        let outcome = generate_completed_review(&mut harness, &backup_outbox, &review);
        let completed = &outcome.response.contract_review;

        assert_eq!(
            completed.session.status,
            ContractReviewStatus::Completed,
            "runtime failure: {:?}",
            completed.session.failure
        );
        assert_eq!(completed.session.stage, ContractReviewStage::Completed);
        assert!(completed.session.completed_at.is_some());
        assert!(
            outcome.backup_warnings.is_empty(),
            "backup warnings: {:?}",
            outcome.backup_warnings
        );
        assert!(outcome
            .contract_events
            .iter()
            .any(|event| { event.event_type == ContractReviewEventType::ReportGenerated }));
        assert!(outcome
            .contract_events
            .iter()
            .any(|event| event.event_type == ContractReviewEventType::Completed));

        let expected_asset_ids = assert_local_artifacts_are_readable(&harness, completed);
        let backups = backup_outbox.list(10).unwrap();
        assert_eq!(backups.len(), expected_asset_ids.len());
        assert!(backups
            .iter()
            .all(|backup| backup.state == BackupState::Queued));
        assert_eq!(
            backups
                .iter()
                .map(|backup| backup.asset_id.clone())
                .collect::<BTreeSet<_>>(),
            expected_asset_ids
        );
        assert_eq!(outcome.backup_events.len(), backups.len());
    }

    #[test]
    fn backup_outbox_queue_failure_never_downgrades_completed_local_review() {
        let mut harness = setup_harness();
        let backup_outbox = BackupOutbox::open(&harness.backup_database_path).unwrap();
        let review = prepare_review_for_report(&mut harness, &backup_outbox);

        let fault_connection = Connection::open(&harness.backup_database_path).unwrap();
        fault_connection
            .execute_batch(
                r#"
                CREATE TRIGGER reject_contract_review_backup_queue
                BEFORE INSERT ON asset_backups
                BEGIN
                    SELECT RAISE(ABORT, 'simulated backup outbox unavailable');
                END;
                "#,
            )
            .unwrap();
        drop(fault_connection);

        let outcome = generate_completed_review(&mut harness, &backup_outbox, &review);
        let completed = &outcome.response.contract_review;

        assert_eq!(
            completed.session.status,
            ContractReviewStatus::Completed,
            "runtime failure: {:?}",
            completed.session.failure
        );
        assert_eq!(completed.session.stage, ContractReviewStage::Completed);
        assert!(completed.session.failure.is_none());
        assert!(completed.session.completed_at.is_some());
        let expected_asset_ids = assert_local_artifacts_are_readable(&harness, completed);

        assert!(outcome.backup_events.is_empty());
        assert_eq!(outcome.backup_warnings.len(), expected_asset_ids.len());
        assert!(
            outcome.backup_warnings.iter().all(|warning| {
                warning.contains("local review completed")
                    && warning.contains("simulated backup outbox unavailable")
            }),
            "backup warnings: {:?}",
            outcome.backup_warnings
        );
        assert!(backup_outbox.list(10).unwrap().is_empty());

        let reloaded =
            contract_review_service::get_review(&harness.connection, &completed.session.id)
                .unwrap();
        assert_eq!(reloaded.session.status, ContractReviewStatus::Completed);
        assert_local_artifacts_are_readable(&harness, &reloaded);
    }

    fn start_with_agent(
        harness: &mut TestHarness,
        backup_outbox: &BackupOutbox,
        agent: &dyn ContractAgentReviewer,
    ) -> ContractReviewRecord {
        let create = create_command(harness);
        let created = execute_contract_review_command_with_agent(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            backup_outbox,
            create,
            agent,
        )
        .unwrap()
        .response
        .contract_review;
        execute_contract_review_command_with_agent(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            backup_outbox,
            start_command(
                &harness.project_id,
                &created.session.id,
                created.session.revision,
            ),
            agent,
        )
        .unwrap()
        .response
        .contract_review
    }

    fn assert_degraded_agent_failure(
        review: &ContractReviewRecord,
        expected_code: &str,
        expected_message: &str,
    ) -> BTreeSet<String> {
        assert_eq!(
            review.session.status,
            ContractReviewStatus::AwaitingConfirmation
        );
        assert_eq!(
            review.session.stage,
            ContractReviewStage::AwaitingConfirmation
        );
        let failure = review.session.failure.as_ref().unwrap();
        assert_eq!(failure.code, expected_code);
        assert_eq!(failure.message, expected_message);
        assert!(failure.retryable);
        assert_eq!(failure.stage, ContractReviewStage::ReviewingAgent);
        assert!(!review.findings.is_empty());
        assert!(review
            .findings
            .iter()
            .all(|finding| finding.source == crate::protocol::ReviewFindingSource::Rule));
        review
            .findings
            .iter()
            .map(|finding| finding.id.clone())
            .collect()
    }

    fn decide_all_findings_with_agent(
        harness: &mut TestHarness,
        backup_outbox: &BackupOutbox,
        agent: &dyn ContractAgentReviewer,
        mut review: ContractReviewRecord,
    ) -> ContractReviewRecord {
        for finding in review.findings.clone() {
            review = execute_contract_review_command_with_agent(
                &mut harness.connection,
                &harness.vault_root,
                &harness.staging_root,
                backup_outbox,
                decide_command(
                    &harness.project_id,
                    &review.session.id,
                    &finding.id,
                    finding.revision,
                ),
                agent,
            )
            .unwrap()
            .response
            .contract_review;
        }
        review
    }

    fn assert_degraded_review_can_complete(
        agent: &dyn ContractAgentReviewer,
        expected_code: &str,
        expected_message: &str,
    ) {
        let mut harness = setup_harness();
        let backup_outbox = BackupOutbox::open(&harness.backup_database_path).unwrap();
        let started = start_with_agent(&mut harness, &backup_outbox, agent);
        let rule_finding_ids =
            assert_degraded_agent_failure(&started, expected_code, expected_message);

        let decided = decide_all_findings_with_agent(&mut harness, &backup_outbox, agent, started);
        assert!(decided.findings.iter().all(|finding| {
            finding.status == ReviewFindingStatus::Decided
                && finding.decision == ReviewFindingDecision::Confirmed
        }));
        assert_eq!(
            decided
                .findings
                .iter()
                .map(|finding| finding.id.clone())
                .collect::<BTreeSet<_>>(),
            rule_finding_ids
        );
        assert_eq!(
            decided
                .session
                .failure
                .as_ref()
                .map(|failure| failure.code.as_str()),
            Some(expected_code),
            "human decisions must not erase the degraded Agent warning"
        );

        let completed = execute_contract_review_command_with_agent(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            &backup_outbox,
            generate_report_command(
                &harness.project_id,
                &decided.session.id,
                decided.session.revision,
            ),
            agent,
        )
        .unwrap()
        .response
        .contract_review;
        assert_eq!(completed.session.status, ContractReviewStatus::Completed);
        assert_eq!(completed.session.stage, ContractReviewStage::Completed);
        assert!(completed.session.failure.is_none());
        assert_eq!(completed.reports.len(), 1);
        assert!(completed.session.report_asset_id.is_some());
        assert_local_artifacts_are_readable(&harness, &completed);
    }

    #[test]
    fn missing_codex_credentials_preserve_rules_and_local_report_closure() {
        assert_degraded_review_can_complete(
            &MissingCredentialAgent,
            "CONTRACT_AGENT_TURN_FAILED",
            "Missing environment variable: BSAIGC_CODEX_API_KEY",
        );
    }

    #[test]
    fn agent_error_preserves_rules_and_local_report_closure() {
        assert_degraded_review_can_complete(
            &FailingAgent,
            "BRAIN_RUNTIME_UNAVAILABLE",
            "test Agent unavailable",
        );
    }

    #[test]
    fn previously_failed_agent_session_retries_into_degraded_confirmation() {
        let mut harness = setup_harness();
        let backup_outbox = BackupOutbox::open(&harness.backup_database_path).unwrap();
        let awaiting = start_with_agent(&mut harness, &backup_outbox, &RuleOnlyTestAgent);
        let rule_finding_ids = awaiting
            .findings
            .iter()
            .map(|finding| finding.id.clone())
            .collect::<BTreeSet<_>>();
        let failed = contract_review_service::fail_review(
            &mut harness.connection,
            &awaiting.session.id,
            &ContractReviewFailure {
                code: "CONTRACT_AGENT_TURN_FAILED".to_string(),
                message: "Missing environment variable: BSAIGC_CODEX_API_KEY".to_string(),
                retryable: true,
                stage: ContractReviewStage::ReviewingAgent,
            },
            awaiting.session.revision,
            "trace-legacy-agent-failure",
        )
        .unwrap()
        .contract_review;
        assert_eq!(failed.session.status, ContractReviewStatus::Failed);

        let retried = execute_contract_review_command_with_agent(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            &backup_outbox,
            retry_agent_command(
                &harness.project_id,
                &failed.session.id,
                failed.session.revision,
            ),
            &MissingCredentialAgent,
        )
        .unwrap()
        .response
        .contract_review;

        assert_degraded_agent_failure(
            &retried,
            "CONTRACT_AGENT_TURN_FAILED",
            "Missing environment variable: BSAIGC_CODEX_API_KEY",
        );
        assert_eq!(
            retried
                .findings
                .iter()
                .map(|finding| finding.id.clone())
                .collect::<BTreeSet<_>>(),
            rule_finding_ids
        );
    }

    #[test]
    fn degraded_agent_review_can_retry_intelligent_review() {
        let mut harness = setup_harness();
        let backup_outbox = BackupOutbox::open(&harness.backup_database_path).unwrap();
        let degraded = start_with_agent(&mut harness, &backup_outbox, &FailingAgent);
        let rule_finding_ids = assert_degraded_agent_failure(
            &degraded,
            "BRAIN_RUNTIME_UNAVAILABLE",
            "test Agent unavailable",
        );

        let retried = execute_contract_review_command_with_agent(
            &mut harness.connection,
            &harness.vault_root,
            &harness.staging_root,
            &backup_outbox,
            retry_agent_command(
                &harness.project_id,
                &degraded.session.id,
                degraded.session.revision,
            ),
            &RuleOnlyTestAgent,
        )
        .unwrap()
        .response
        .contract_review;

        assert_eq!(
            retried.session.status,
            ContractReviewStatus::AwaitingConfirmation
        );
        assert_eq!(
            retried.session.stage,
            ContractReviewStage::AwaitingConfirmation
        );
        assert!(retried.session.failure.is_none());
        assert_eq!(
            retried
                .findings
                .iter()
                .map(|finding| finding.id.clone())
                .collect::<BTreeSet<_>>(),
            rule_finding_ids
        );
    }
}
