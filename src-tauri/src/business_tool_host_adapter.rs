use crate::asset_service::{self, GeneratedArtifactSource};
use crate::backup_outbox::BackupOutbox;
use crate::business_tool_registry::{
    ApprovalRequestInput, ApprovalRequestOutput, ApprovalRequestStatus, ArtifactCompareInput,
    ArtifactCompareMode, ArtifactCompareOutput, ArtifactContentMode, ArtifactCreateFormat,
    ArtifactCreateInput, ArtifactCreateOutput, ArtifactDifference, ArtifactReadInput,
    ArtifactReadOutput, BusinessArtifactContent, BusinessArtifactView,
    BusinessDocumentFormat as ToolDocumentFormat, BusinessDocumentType, BusinessProjectBriefView,
    BusinessProjectView, BusinessSourceKind, BusinessToolContext, BusinessToolDispatchAdapter,
    BusinessToolError, BusinessWorkspaceView, CalculationInput, CalculationLineOutput,
    CalculationOutput, DocumentExtractInput, DocumentExtractOutput, DocumentFieldInput,
    DocumentFieldValue, DocumentGenerateInput, DocumentGenerateOutput, DocumentPageView,
    DocumentTableView, DocumentValidateInput, DocumentValidateOutput, DocumentValidationCheck,
    DocumentValidationIssue, LedgerEntryKind, LedgerEntryStatus, LedgerEntryView, LedgerReadInput,
    LedgerReadOutput, ProjectReadInput, ProjectReadOutput, ProjectWriteInput, ProjectWriteOutput,
    SourceLocateInput, SourceLocateOutput, SourceMatch, TaskPlanInput, TaskPlanItem,
    TaskPlanOutput, TaskPlanPriority, TemplateFieldDefinition, TemplateReadInput,
    TemplateReadOutput,
};
use crate::business_workspace_service;
use crate::contract_review_service;
use crate::document_engine;
use crate::document_intelligence::DocumentIntelligence;
use crate::host::BackendHost;
use crate::protocol::{
    BackupCommandEnvelope, BriefRecord, BusinessDocumentKind, BusinessDocumentStatus,
    BusinessPaymentStatus, BusinessReceiptKind, ChangeProjectStagePayload, CommandEnvelope,
    CreateTaskPayload, HostError, ListContractReviewsRequest, OperationContext,
    QueueAssetBackupPayload, TaskCommandEnvelope, TaskPriority, TaskReplayPolicy,
    UpdateProjectBriefPayload, BACKUP_PROTOCOL_VERSION, PROTOCOL_VERSION,
};
use crate::security::OperationEffect;
use crate::task_engine::TaskEngine;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_TEXT_READ_BYTES_PER_CHAR: u64 = 4;

pub struct BusinessToolHostAdapter {
    host: BackendHost,
    connection: Arc<Mutex<Connection>>,
    vault_root: PathBuf,
    staging_root: PathBuf,
    backup_outbox: Arc<BackupOutbox>,
    tasks: Arc<TaskEngine>,
}

impl BusinessToolHostAdapter {
    pub fn new(
        host: BackendHost,
        connection: Arc<Mutex<Connection>>,
        vault_root: PathBuf,
        staging_root: PathBuf,
        backup_outbox: Arc<BackupOutbox>,
        tasks: Arc<TaskEngine>,
    ) -> Result<Self, HostError> {
        fs::create_dir_all(&staging_root).map_err(|error| {
            HostError::internal(format!(
                "create business tool staging directory failed: {error}"
            ))
        })?;
        let staging_root = staging_root.canonicalize().map_err(|error| {
            HostError::internal(format!(
                "resolve business tool staging directory failed: {error}"
            ))
        })?;
        if !staging_root.is_absolute() {
            return Err(HostError::validation(
                "business tool staging directory must be absolute",
            ));
        }
        {
            let connection = connection.lock().map_err(|_| {
                HostError::internal("business tool storage lock is poisoned during migration")
            })?;
            migrate_business_tool_storage(&connection)?;
        }
        Ok(Self {
            host,
            connection,
            vault_root,
            staging_root,
            backup_outbox,
            tasks,
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, BusinessToolError> {
        self.connection.lock().map_err(|_| {
            BusinessToolError::new(
                "BUSINESS_TOOL_STORAGE_UNAVAILABLE",
                "business storage is temporarily unavailable",
                true,
            )
        })
    }

    fn comparable_text(
        &self,
        context: &BusinessToolContext,
        asset_id: &str,
        max_chars: u32,
        role: &str,
    ) -> Result<(String, bool), BusinessToolError> {
        let (asset, source_path) = {
            let connection = self.lock()?;
            let (asset, source_path) = asset_service::verify_ready_asset_integrity(
                &connection,
                &self.vault_root,
                asset_id,
            )
            .map_err(map_host_error)?;
            ensure_optional_project_scope(context, asset.project_id.as_deref())?;
            (asset, source_path)
        };
        if is_text_mime(&asset.mime_type) {
            let content =
                read_text_content(&source_path, &asset.mime_type, max_chars, &asset.sha256)?;
            return Ok((content.text, content.truncated));
        }
        let extraction = DocumentIntelligence::with_defaults()
            .extract(
                &format!("{}-{role}", context.call_id),
                &asset.id,
                &asset.sha256,
                &asset.mime_type,
                &source_path,
                now_millis(),
            )
            .map_err(map_host_error)?;
        let joined = extraction
            .pages
            .iter()
            .map(|page| page.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let total_chars = joined.chars().count();
        Ok((
            joined.chars().take(max_chars as usize).collect(),
            total_chars > max_chars as usize,
        ))
    }

    fn find_generated_document(
        &self,
        input: &DocumentGenerateInput,
    ) -> Result<Option<DocumentGenerateOutput>, BusinessToolError> {
        let connection = self.lock()?;
        let row = connection
            .query_row(
                "SELECT document_id, document_type, asset_id
                 FROM business_tool_generated_documents
                 WHERE project_id = ?1 AND idempotency_key = ?2",
                params![input.project_id, input.idempotency_key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(map_sql_error)?;
        let Some((document_id, document_type, asset_id)) = row else {
            return Ok(None);
        };
        if document_type != document_type_wire(input.document_type) {
            return Err(BusinessToolError::new(
                "BUSINESS_DOCUMENT_IDEMPOTENCY_CONFLICT",
                "document idempotency key was reused for a different document type",
                false,
            ));
        }
        let asset = asset_service::get_asset(&connection, &asset_id).map_err(map_host_error)?;
        Ok(Some(DocumentGenerateOutput {
            document_id,
            project_id: input.project_id.clone(),
            document_type: input.document_type,
            artifact: asset_view(&asset),
            idempotency_key: input.idempotency_key.clone(),
        }))
    }

    fn persist_generated_document(
        &self,
        document_id: &str,
        input: &DocumentGenerateInput,
        template_id: &str,
        asset_id: &str,
    ) -> Result<(), BusinessToolError> {
        let fields_json = serde_json::to_string(&input.fields).map_err(|_| {
            BusinessToolError::new(
                "BUSINESS_DOCUMENT_METADATA_INVALID",
                "generated document fields could not be serialized",
                false,
            )
        })?;
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT OR IGNORE INTO business_tool_generated_documents
                 (document_id, project_id, document_type, template_id, fields_json,
                  asset_id, idempotency_key, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    document_id,
                    input.project_id,
                    document_type_wire(input.document_type),
                    template_id,
                    fields_json,
                    asset_id,
                    input.idempotency_key,
                    now_millis()
                ],
            )
            .map_err(map_sql_error)?;
        let persisted_asset: String = connection
            .query_row(
                "SELECT asset_id FROM business_tool_generated_documents
                 WHERE project_id = ?1 AND idempotency_key = ?2",
                params![input.project_id, input.idempotency_key],
                |row| row.get(0),
            )
            .map_err(map_sql_error)?;
        if persisted_asset != asset_id {
            return Err(BusinessToolError::new(
                "BUSINESS_DOCUMENT_IDEMPOTENCY_CONFLICT",
                "document idempotency key already belongs to another artifact",
                false,
            ));
        }
        Ok(())
    }
}

impl BusinessToolDispatchAdapter for BusinessToolHostAdapter {
    fn project_read(
        &self,
        context: &BusinessToolContext,
        input: ProjectReadInput,
    ) -> Result<ProjectReadOutput, BusinessToolError> {
        ensure_project_scope(context, &input.project_id)?;
        let project = self
            .host
            .list_projects()
            .map_err(map_host_error)?
            .into_iter()
            .find(|project| project.id == input.project_id)
            .ok_or_else(|| {
                BusinessToolError::new(
                    "BUSINESS_PROJECT_NOT_FOUND",
                    "business project does not exist",
                    false,
                )
            })?;

        let business_workspace = if input.include_business_workspace {
            let connection = self.lock()?;
            business_workspace_service::list(&connection)
                .map_err(map_host_error)?
                .into_iter()
                .find(|workspace| workspace.project_id == project.id)
                .map(|workspace| BusinessWorkspaceView {
                    id: workspace.id,
                    project_id: workspace.project_id,
                    status: enum_wire_name(&workspace.status),
                    lifecycle_stage: enum_wire_name(&workspace.lifecycle_stage),
                    revision: workspace.revision,
                    current_document_ids: [
                        workspace.current_documents.quote_document_id,
                        workspace.current_documents.contract_document_id,
                        workspace.current_documents.payment_request_document_id,
                        workspace.current_documents.acceptance_document_id,
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                    outstanding_cents: workspace.financial_summary.outstanding_cents,
                })
        } else {
            None
        };

        Ok(ProjectReadOutput {
            project: BusinessProjectView {
                id: project.id,
                name: project.name,
                client_name: project.client_name,
                stage: project.stage.as_db_str().to_string(),
                revision: project.revision,
                updated_at: project.updated_at,
                brief: BusinessProjectBriefView {
                    objective: project.brief.objective,
                    audience: project.brief.audience,
                    deliverables: project.brief.deliverables,
                    mandatory_items: project.brief.mandatory_items,
                    constraints: project.brief.constraints,
                    risks: project.brief.risks,
                },
            },
            business_workspace,
        })
    }

    fn artifact_read(
        &self,
        context: &BusinessToolContext,
        input: ArtifactReadInput,
    ) -> Result<ArtifactReadOutput, BusinessToolError> {
        let (asset, source_path) = {
            let connection = self.lock()?;
            let asset =
                asset_service::get_asset(&connection, &input.asset_id).map_err(map_host_error)?;
            ensure_optional_project_scope(context, asset.project_id.as_deref())?;
            let source_path = if input.content_mode == ArtifactContentMode::Text {
                Some(
                    asset_service::verify_ready_asset_integrity(
                        &connection,
                        &self.vault_root,
                        &asset.id,
                    )
                    .map_err(map_host_error)?
                    .1,
                )
            } else {
                None
            };
            (asset, source_path)
        };

        let content = match source_path {
            Some(path) => Some(read_text_content(
                &path,
                &asset.mime_type,
                input.max_chars,
                &asset.sha256,
            )?),
            None => None,
        };
        Ok(ArtifactReadOutput {
            artifact: asset_view(&asset),
            content,
        })
    }

    fn document_extract(
        &self,
        context: &BusinessToolContext,
        input: DocumentExtractInput,
    ) -> Result<DocumentExtractOutput, BusinessToolError> {
        let (asset, source_path, persisted) = {
            let connection = self.lock()?;
            let (asset, source_path) = asset_service::verify_ready_asset_integrity(
                &connection,
                &self.vault_root,
                &input.asset_id,
            )
            .map_err(map_host_error)?;
            ensure_optional_project_scope(context, asset.project_id.as_deref())?;
            let persisted = input
                .review_id
                .as_deref()
                .map(|review_id| contract_review_service::get_review(&connection, review_id))
                .transpose()
                .map_err(map_host_error)?
                .and_then(|review| review.extraction)
                .map(|extraction| {
                    if extraction.source_asset_id != asset.id {
                        Err(BusinessToolError::new(
                            "BUSINESS_TOOL_REVIEW_ASSET_MISMATCH",
                            "contract review extraction belongs to a different asset",
                            false,
                        ))
                    } else {
                        Ok(extraction)
                    }
                })
                .transpose()?;
            (asset, source_path, persisted)
        };

        let extraction = match persisted {
            Some(extraction) => extraction,
            None => DocumentIntelligence::with_defaults()
                .extract(
                    input
                        .review_id
                        .as_deref()
                        .unwrap_or(context.call_id.as_str()),
                    &asset.id,
                    &asset.sha256,
                    &asset.mime_type,
                    &source_path,
                    now_millis(),
                )
                .map_err(map_host_error)?,
        };
        extraction_view(extraction, &input)
    }

    fn artifact_create(
        &self,
        context: &BusinessToolContext,
        input: ArtifactCreateInput,
    ) -> Result<ArtifactCreateOutput, BusinessToolError> {
        ensure_project_scope(context, &input.project_id)?;
        {
            let connection = self.lock()?;
            validate_lineage_sources(&connection, &input.project_id, &input.source_artifact_ids)?;
        }
        let operation_dir = self.staging_root.join(Uuid::new_v4().to_string());
        fs::create_dir(&operation_dir).map_err(|_| {
            BusinessToolError::new(
                "BUSINESS_ARTIFACT_STAGING_FAILED",
                "unable to prepare local artifact staging",
                true,
            )
        })?;
        let extension = match input.format.mime_type() {
            "text/markdown" => "md",
            "text/plain" => "txt",
            "application/json" => "json",
            _ => "txt",
        };
        let stem = Path::new(&input.display_name)
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("artifact");
        let source_name = format!("{stem}.{extension}");
        let source_path = operation_dir.join(source_name);
        let result = (|| {
            let mut file = File::create(&source_path).map_err(|_| {
                BusinessToolError::new(
                    "BUSINESS_ARTIFACT_STAGING_FAILED",
                    "unable to create local artifact staging file",
                    true,
                )
            })?;
            file.write_all(input.content.as_bytes()).map_err(|_| {
                BusinessToolError::new(
                    "BUSINESS_ARTIFACT_STAGING_FAILED",
                    "unable to write local artifact staging file",
                    true,
                )
            })?;
            file.sync_all().map_err(|_| {
                BusinessToolError::new(
                    "BUSINESS_ARTIFACT_STAGING_FAILED",
                    "unable to commit local artifact staging file",
                    true,
                )
            })?;

            let asset = {
                let mut connection = self.lock()?;
                asset_service::import_generated_artifact(
                    &mut connection,
                    &self.vault_root,
                    &input.project_id,
                    &source_path,
                    GeneratedArtifactSource::ReviewReport,
                    &artifact_source_ref(&input.project_id, &context.call_id),
                )
                .map_err(map_host_error)?
            };

            {
                let connection = self.lock()?;
                record_artifact_lineage(
                    &connection,
                    &asset.id,
                    &input.source_artifact_ids,
                    now_millis(),
                )?;
            }

            queue_backup_best_effort(&self.backup_outbox, context, &asset);
            Ok(ArtifactCreateOutput {
                artifact: asset_view(&asset),
                idempotency_key: context.call_id.clone(),
            })
        })();
        let _ = fs::remove_dir_all(&operation_dir);
        result
    }

    fn approval_request(
        &self,
        context: &BusinessToolContext,
        input: ApprovalRequestInput,
    ) -> Result<ApprovalRequestOutput, BusinessToolError> {
        let decision = self
            .host
            .authorize_operation(
                &context.actor_id,
                input.action.operation(),
                input.resource.resource_type(),
                Some(&input.resource_id),
                OperationEffect::Irreversible,
                None,
            )
            .map_err(map_host_error)?;
        let approval_id = decision.approval_id.unwrap_or_default();
        let expires_at = if approval_id.is_empty() {
            None
        } else {
            self.host
                .list_pending_approvals()
                .map_err(map_host_error)?
                .into_iter()
                .find(|approval| approval.id == approval_id)
                .map(|approval| approval.expires_at)
        };
        let status = if decision.allowed {
            ApprovalRequestStatus::AlreadyApproved
        } else if decision.approval_required {
            ApprovalRequestStatus::Pending
        } else {
            ApprovalRequestStatus::Denied
        };
        Ok(ApprovalRequestOutput {
            approval_id,
            status,
            operation: input.action.operation().to_string(),
            resource_type: input.resource.resource_type().to_string(),
            resource_id: input.resource_id,
            expires_at,
            reason: decision.reason.or(Some(input.summary)),
        })
    }

    fn artifact_compare(
        &self,
        context: &BusinessToolContext,
        input: ArtifactCompareInput,
    ) -> Result<ArtifactCompareOutput, BusinessToolError> {
        let (left, left_truncated) =
            self.comparable_text(context, &input.left_asset_id, input.max_chars, "left")?;
        let (right, right_truncated) =
            self.comparable_text(context, &input.right_asset_id, input.max_chars, "right")?;
        let diff = TextDiff::from_lines(&left, &right);
        let mut differences = Vec::new();
        let mut observed = 0_u32;
        let mut old_line = 1_u32;
        let mut new_line = 1_u32;
        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Equal => {
                    old_line = old_line.saturating_add(1);
                    new_line = new_line.saturating_add(1);
                }
                ChangeTag::Delete => {
                    observed = observed.saturating_add(1);
                    if differences.len() < input.max_differences as usize {
                        differences.push(ArtifactDifference {
                            kind: "removed".to_string(),
                            location: format!("leftLine:{old_line}"),
                            left_text: Some(bounded_model_text(change.value(), 2_000)),
                            right_text: None,
                            severity: "changed".to_string(),
                        });
                    }
                    old_line = old_line.saturating_add(1);
                }
                ChangeTag::Insert => {
                    observed = observed.saturating_add(1);
                    if differences.len() < input.max_differences as usize {
                        differences.push(ArtifactDifference {
                            kind: "added".to_string(),
                            location: format!("rightLine:{new_line}"),
                            left_text: None,
                            right_text: Some(bounded_model_text(change.value(), 2_000)),
                            severity: "changed".to_string(),
                        });
                    }
                    new_line = new_line.saturating_add(1);
                }
            }
        }
        let semantic_fallback = input.mode == ArtifactCompareMode::Semantic;
        Ok(ArtifactCompareOutput {
            comparison_id: deterministic_uuid(&format!(
                "compare:{}:{}:{:?}",
                input.left_asset_id, input.right_asset_id, input.mode
            )),
            left_asset_id: input.left_asset_id,
            right_asset_id: input.right_asset_id,
            mode: input.mode,
            status: if semantic_fallback {
                "lexicalEvidenceOnly".to_string()
            } else {
                "completed".to_string()
            },
            summary: if semantic_fallback {
                format!(
                    "发现 {observed} 处文本变化；当前仅提供确定性词法证据，语义判断交由 Agent 复核。"
                )
            } else {
                format!("发现 {observed} 处文本变化。")
            },
            differences,
            truncated: left_truncated || right_truncated || observed > input.max_differences,
        })
    }

    fn source_locate(
        &self,
        context: &BusinessToolContext,
        input: SourceLocateInput,
    ) -> Result<SourceLocateOutput, BusinessToolError> {
        if let Some(project_id) = input.project_id.as_deref() {
            ensure_project_scope(context, project_id)?;
        }
        let mut matches = Vec::new();
        let requested = input.kinds.clone();
        let accepts = |kind: BusinessSourceKind| requested.is_empty() || requested.contains(&kind);
        let query = input.query.trim().to_lowercase();

        if accepts(BusinessSourceKind::Project) {
            for project in self.host.list_projects().map_err(map_host_error)? {
                if input
                    .project_id
                    .as_deref()
                    .is_some_and(|id| id != project.id)
                {
                    continue;
                }
                let haystack = format!(
                    "{} {} {} {}",
                    project.name,
                    project.client_name,
                    project.brief.objective,
                    project.brief.audience
                );
                if let Some(relevance) = relevance_score(&query, &haystack) {
                    matches.push(SourceMatch {
                        source_id: project.id.clone(),
                        project_id: Some(project.id),
                        display_name: project.name,
                        kind: BusinessSourceKind::Project,
                        relevance,
                        excerpt: input.include_excerpt.then(|| {
                            bounded_model_text(&project.brief.objective, input.max_excerpt_chars)
                        }),
                    });
                }
            }
        }

        let connection = self.lock()?;
        if accepts(BusinessSourceKind::Artifact) {
            for asset in asset_service::list_assets(&connection, input.project_id.as_deref())
                .map_err(map_host_error)?
            {
                if let Some(relevance) = relevance_score(&query, &asset.original_name) {
                    matches.push(SourceMatch {
                        source_id: asset.id,
                        project_id: asset.project_id,
                        display_name: asset.original_name,
                        kind: BusinessSourceKind::Artifact,
                        relevance,
                        excerpt: input
                            .include_excerpt
                            .then(|| format!("{} · {} bytes", asset.mime_type, asset.size_bytes)),
                    });
                }
            }
        }
        let workspaces = business_workspace_service::list(&connection).map_err(map_host_error)?;
        for workspace in workspaces {
            if input
                .project_id
                .as_deref()
                .is_some_and(|id| id != workspace.project_id)
            {
                continue;
            }
            for document in &workspace.documents {
                let kind = match document.kind {
                    BusinessDocumentKind::Quote => BusinessSourceKind::Quote,
                    BusinessDocumentKind::Acceptance => BusinessSourceKind::Acceptance,
                    BusinessDocumentKind::PaymentRequest => BusinessSourceKind::Payment,
                    BusinessDocumentKind::Contract => BusinessSourceKind::Artifact,
                };
                if !accepts(kind) {
                    continue;
                }
                let haystack = format!(
                    "{} {} {}",
                    document.title, document.document_number, workspace.profile.customer_name
                );
                if let Some(relevance) = relevance_score(&query, &haystack) {
                    matches.push(SourceMatch {
                        source_id: document.id.clone(),
                        project_id: Some(workspace.project_id.clone()),
                        display_name: document.title.clone(),
                        kind,
                        relevance,
                        excerpt: input.include_excerpt.then(|| {
                            bounded_model_text(&document.document_number, input.max_excerpt_chars)
                        }),
                    });
                }
            }
            if accepts(BusinessSourceKind::ContractReview) {
                let reviews = contract_review_service::list_reviews(
                    &connection,
                    &ListContractReviewsRequest {
                        workspace_id: Some(workspace.id.clone()),
                        status: None,
                        limit: Some(input.max_results),
                    },
                )
                .map_err(map_host_error)?;
                for review in reviews {
                    let haystack = format!(
                        "{} {}",
                        review.session.source_file_name,
                        enum_wire_name(&review.session.status)
                    );
                    if let Some(relevance) = relevance_score(&query, &haystack) {
                        matches.push(SourceMatch {
                            source_id: review.session.id,
                            project_id: Some(workspace.project_id.clone()),
                            display_name: review.session.source_file_name,
                            kind: BusinessSourceKind::ContractReview,
                            relevance,
                            excerpt: input
                                .include_excerpt
                                .then(|| format!("{} 项风险", review.findings.len())),
                        });
                    }
                }
            }
        }
        drop(connection);

        if accepts(BusinessSourceKind::Template) {
            for template_id in business_template_ids() {
                let template = template_output(template_id, input.max_excerpt_chars)?;
                let haystack = format!("{} {}", template.display_name, template.template_id);
                if let Some(relevance) = relevance_score(&query, &haystack) {
                    matches.push(SourceMatch {
                        source_id: template.template_id,
                        project_id: None,
                        display_name: template.display_name,
                        kind: BusinessSourceKind::Template,
                        relevance,
                        excerpt: input.include_excerpt.then(|| {
                            bounded_model_text(&template.content, input.max_excerpt_chars)
                        }),
                    });
                }
            }
        }

        matches.sort_by(|left, right| {
            right
                .relevance
                .total_cmp(&left.relevance)
                .then_with(|| left.display_name.cmp(&right.display_name))
        });
        let total_matches = matches.len() as u32;
        matches.truncate(input.max_results as usize);
        Ok(SourceLocateOutput {
            query: input.query,
            truncated: total_matches > matches.len() as u32,
            total_matches,
            matches,
        })
    }

    fn template_read(
        &self,
        _context: &BusinessToolContext,
        input: TemplateReadInput,
    ) -> Result<TemplateReadOutput, BusinessToolError> {
        template_output(&input.template_id, input.max_chars)
    }

    fn calculation(
        &self,
        _context: &BusinessToolContext,
        input: CalculationInput,
    ) -> Result<CalculationOutput, BusinessToolError> {
        let mut subtotal = 0_i64;
        let mut lines = Vec::with_capacity(input.lines.len());
        for line in &input.lines {
            let amount = line
                .quantity_milli
                .checked_mul(line.unit_price_cents)
                .and_then(|value| value.checked_div(1_000))
                .and_then(|value| value.checked_sub(line.discount_cents))
                .ok_or_else(calculation_overflow)?;
            subtotal = subtotal
                .checked_add(amount)
                .ok_or_else(calculation_overflow)?;
            lines.push(CalculationLineOutput {
                key: line.key.clone(),
                amount_cents: amount,
            });
        }
        let taxable_cents = subtotal
            .checked_sub(input.discount_cents)
            .ok_or_else(calculation_overflow)?;
        let tax_cents = taxable_cents
            .checked_mul(i64::from(input.tax_rate_basis_points))
            .and_then(|value| value.checked_div(10_000))
            .ok_or_else(calculation_overflow)?;
        let total_cents = taxable_cents
            .checked_add(tax_cents)
            .ok_or_else(calculation_overflow)?;
        Ok(CalculationOutput {
            calculation_id: input.calculation_id,
            mode: input.mode,
            currency: input.currency,
            lines,
            subtotal_cents: subtotal,
            discount_cents: input.discount_cents,
            taxable_cents,
            tax_cents,
            total_cents,
        })
    }

    fn ledger_read(
        &self,
        context: &BusinessToolContext,
        input: LedgerReadInput,
    ) -> Result<LedgerReadOutput, BusinessToolError> {
        ensure_project_scope(context, &input.project_id)?;
        let connection = self.lock()?;
        let workspace = business_workspace_service::list(&connection)
            .map_err(map_host_error)?
            .into_iter()
            .find(|workspace| workspace.project_id == input.project_id)
            .ok_or_else(|| {
                BusinessToolError::new(
                    "BUSINESS_WORKSPACE_NOT_FOUND",
                    "business workspace does not exist for this project",
                    false,
                )
            })?;
        let currency = if workspace.profile.currency.trim().len() == 3 {
            workspace.profile.currency.clone()
        } else {
            "CNY".to_string()
        };
        let mut entries = ledger_entries(&workspace, &currency);
        if !input.kinds.is_empty() {
            entries.retain(|entry| input.kinds.contains(&entry.kind));
        }
        if !input.statuses.is_empty() {
            entries.retain(|entry| input.statuses.contains(&entry.status));
        }
        entries.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        let total_amount_cents = entries
            .iter()
            .try_fold(0_i64, |total, entry| total.checked_add(entry.amount_cents))
            .ok_or_else(calculation_overflow)?;
        let truncated = entries.len() > input.max_entries as usize;
        entries.truncate(input.max_entries as usize);
        Ok(LedgerReadOutput {
            project_id: input.project_id,
            entries,
            total_amount_cents,
            outstanding_amount_cents: workspace.financial_summary.outstanding_cents,
            truncated,
        })
    }

    fn project_write(
        &self,
        context: &BusinessToolContext,
        input: ProjectWriteInput,
    ) -> Result<ProjectWriteOutput, BusinessToolError> {
        ensure_project_scope(context, &input.project_id)?;
        if input.patch.name.is_some() || input.patch.client_name.is_some() {
            return Err(BusinessToolError::new(
                "BUSINESS_PROJECT_FIELD_UNSUPPORTED",
                "project name and client name remain human-managed fields in this version",
                false,
            ));
        }
        let mut current = self
            .host
            .list_projects()
            .map_err(map_host_error)?
            .into_iter()
            .find(|project| project.id == input.project_id)
            .ok_or_else(|| {
                BusinessToolError::new(
                    "BUSINESS_PROJECT_NOT_FOUND",
                    "business project does not exist",
                    false,
                )
            })?;
        if current.revision != input.expected_revision {
            return Err(BusinessToolError::new(
                "REVISION_CONFLICT",
                "project revision changed before the write",
                true,
            ));
        }
        let mut changed_fields = Vec::new();
        if let Some(brief) = input.patch.brief {
            let command = CommandEnvelope::ProjectUpdateBrief {
                command_id: deterministic_uuid(&format!("{}:brief", context.call_id)),
                protocol_version: PROTOCOL_VERSION.to_string(),
                context: operation_context(context, Some(input.project_id.clone())),
                payload: UpdateProjectBriefPayload {
                    project_id: input.project_id.clone(),
                    brief: BriefRecord {
                        objective: brief.objective,
                        audience: brief.audience,
                        deliverables: brief.deliverables,
                        style_keywords: current.brief.style_keywords.clone(),
                        mandatory_items: brief.mandatory_items,
                        constraints: brief.constraints,
                        risks: brief.risks,
                        reference_notes: current.brief.reference_notes.clone(),
                    },
                },
                idempotency_key: format!("{}:brief", context.call_id),
                expected_revision: Some(current.revision),
                deadline_at: None,
            };
            current = self
                .host
                .execute(command)
                .map_err(map_host_error)?
                .response
                .project;
            changed_fields.push("brief".to_string());
        }
        if let Some(stage) = input.patch.stage {
            let stage = crate::protocol::ProjectStage::from_db_str(&stage).ok_or_else(|| {
                BusinessToolError::new(
                    "BUSINESS_PROJECT_STAGE_INVALID",
                    "project stage is not supported",
                    false,
                )
            })?;
            let command = CommandEnvelope::ProjectChangeStage {
                command_id: deterministic_uuid(&format!("{}:stage", context.call_id)),
                protocol_version: PROTOCOL_VERSION.to_string(),
                context: operation_context(context, Some(input.project_id.clone())),
                payload: ChangeProjectStagePayload {
                    project_id: input.project_id.clone(),
                    stage,
                },
                idempotency_key: format!("{}:stage", context.call_id),
                expected_revision: Some(current.revision),
                deadline_at: None,
            };
            current = self
                .host
                .execute(command)
                .map_err(map_host_error)?
                .response
                .project;
            changed_fields.push("stage".to_string());
        }
        Ok(ProjectWriteOutput {
            project: project_view(current),
            changed_fields,
            idempotency_key: context.call_id.clone(),
        })
    }

    fn task_plan(
        &self,
        context: &BusinessToolContext,
        input: TaskPlanInput,
    ) -> Result<TaskPlanOutput, BusinessToolError> {
        ensure_project_scope(context, &input.project_id)?;
        let mut keys = HashSet::new();
        for step in &input.steps {
            if !keys.insert(step.key.clone()) {
                return Err(BusinessToolError::new(
                    "BUSINESS_TASK_PLAN_INVALID",
                    "task plan contains duplicate step keys",
                    false,
                ));
            }
        }
        let plan_id = deterministic_uuid(&format!(
            "plan:{}:{}",
            input.project_id, input.idempotency_key
        ));
        let mut task_ids = HashMap::<String, String>::new();
        let mut tasks = Vec::with_capacity(input.steps.len());
        for (position, step) in input.steps.iter().enumerate() {
            let dependencies = step
                .depends_on
                .iter()
                .map(|key| {
                    task_ids.get(key).cloned().ok_or_else(|| {
                        BusinessToolError::new(
                            "BUSINESS_TASK_PLAN_INVALID",
                            "task dependencies must reference an earlier plan step",
                            false,
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let command_key = format!("{}:{}", input.idempotency_key, step.key);
            let outcome = self
                .tasks
                .execute_command(TaskCommandEnvelope::Create {
                    command_id: deterministic_uuid(&format!("task:{command_key}")),
                    protocol_version: PROTOCOL_VERSION.to_string(),
                    context: operation_context(context, Some(input.project_id.clone())),
                    payload: CreateTaskPayload {
                        kind: format!("business.plan.{}", step.owner_role),
                        project_id: Some(input.project_id.clone()),
                        input: json!({
                            "planId": plan_id,
                            "stepKey": step.key,
                            "title": step.title,
                            "instructions": step.instructions,
                            "ownerRole": step.owner_role,
                            "objective": input.objective
                        }),
                        priority: task_priority(input.priority),
                        replay_policy: TaskReplayPolicy::Manual,
                        max_attempts: 1,
                        dependency_task_ids: dependencies,
                    },
                    idempotency_key: command_key,
                    expected_revision: None,
                    deadline_at: None,
                })
                .map_err(map_host_error)?;
            task_ids.insert(step.key.clone(), outcome.response.task.id.clone());
            tasks.push(TaskPlanItem {
                task_id: outcome.response.task.id,
                key: step.key.clone(),
                title: step.title.clone(),
                status: outcome.response.task.status.as_db_str().to_string(),
                position: position as u32,
            });
        }
        Ok(TaskPlanOutput {
            plan_id,
            project_id: input.project_id,
            title: input.title,
            status: "planned".to_string(),
            revision: 1,
            tasks,
            idempotency_key: input.idempotency_key,
        })
    }

    fn document_generate(
        &self,
        context: &BusinessToolContext,
        input: DocumentGenerateInput,
    ) -> Result<DocumentGenerateOutput, BusinessToolError> {
        ensure_project_scope(context, &input.project_id)?;
        if let Some(existing) = self.find_generated_document(&input)? {
            return Ok(existing);
        }
        let template_id = input
            .template_id
            .clone()
            .unwrap_or_else(|| default_template_id(input.document_type).to_string());
        if template_id != default_template_id(input.document_type) {
            return Err(BusinessToolError::new(
                "BUSINESS_TEMPLATE_TYPE_MISMATCH",
                "selected template does not match the requested document type",
                false,
            ));
        }
        let template = template_output(&template_id, 120_000)?;
        ensure_required_document_fields(&template, &input.fields)?;
        let content =
            render_document_content(input.document_type, input.format, &template, &input.fields)?;
        let document_id = deterministic_uuid(&format!(
            "document:{}:{}",
            input.project_id, input.idempotency_key
        ));
        let artifact_format = match input.format {
            ToolDocumentFormat::Markdown => ArtifactCreateFormat::Markdown,
            ToolDocumentFormat::PlainText => ArtifactCreateFormat::PlainText,
            ToolDocumentFormat::Json => ArtifactCreateFormat::Json,
        };
        let extension = match input.format {
            ToolDocumentFormat::Markdown => "md",
            ToolDocumentFormat::PlainText => "txt",
            ToolDocumentFormat::Json => "json",
        };
        let generated_context = BusinessToolContext {
            call_id: input.idempotency_key.clone(),
            actor_id: context.actor_id.clone(),
            account_id: context.account_id.clone(),
            project_id: context.project_id.clone(),
            trace_id: context.trace_id.clone(),
        };
        let created = self.artifact_create(
            &generated_context,
            ArtifactCreateInput {
                project_id: input.project_id.clone(),
                display_name: format!(
                    "{}-{}.{}",
                    document_type_wire(input.document_type),
                    &document_id[..8],
                    extension
                ),
                format: artifact_format,
                content,
                source_artifact_ids: input.source_artifact_ids.clone(),
            },
        )?;
        self.persist_generated_document(
            &document_id,
            &input,
            &template_id,
            &created.artifact.asset_id,
        )?;
        Ok(DocumentGenerateOutput {
            document_id,
            project_id: input.project_id,
            document_type: input.document_type,
            artifact: created.artifact,
            idempotency_key: input.idempotency_key,
        })
    }

    fn document_validate(
        &self,
        context: &BusinessToolContext,
        input: DocumentValidateInput,
    ) -> Result<DocumentValidateOutput, BusinessToolError> {
        let (asset, source_path, metadata) = {
            let connection = self.lock()?;
            let (asset, source_path) = asset_service::verify_ready_asset_integrity(
                &connection,
                &self.vault_root,
                &input.artifact_id,
            )
            .map_err(map_host_error)?;
            ensure_optional_project_scope(context, asset.project_id.as_deref())?;
            let metadata = load_generated_document_metadata(&connection, &asset.id)?;
            (asset, source_path, metadata)
        };
        let checks = if input.checks.is_empty() {
            vec![
                DocumentValidationCheck::RequiredFields,
                DocumentValidationCheck::ProjectBinding,
                DocumentValidationCheck::Amounts,
                DocumentValidationCheck::Dates,
                DocumentValidationCheck::SourceEvidence,
                DocumentValidationCheck::Formatting,
            ]
        } else {
            input.checks.clone()
        };
        let mut issues = Vec::new();
        if checks.contains(&DocumentValidationCheck::ProjectBinding) && asset.project_id.is_none() {
            issues.push(validation_issue(
                "DOCUMENT_PROJECT_MISSING",
                "error",
                None,
                "文档未绑定商务项目。",
            ));
        }
        if checks.contains(&DocumentValidationCheck::Formatting) {
            if asset.kind != crate::protocol::AssetKind::Document || asset.size_bytes == 0 {
                issues.push(validation_issue(
                    "DOCUMENT_FORMAT_INVALID",
                    "error",
                    None,
                    "文档格式不可用或内容为空。",
                ));
            } else if asset.mime_type == "application/json"
                && serde_json::from_slice::<Value>(&fs::read(&source_path).map_err(|_| {
                    BusinessToolError::new(
                        "BUSINESS_DOCUMENT_VALIDATION_FAILED",
                        "unable to read generated document",
                        true,
                    )
                })?)
                .is_err()
            {
                issues.push(validation_issue(
                    "DOCUMENT_JSON_INVALID",
                    "error",
                    None,
                    "JSON 文档无法重新解析。",
                ));
            }
        }
        match metadata {
            Some(metadata) => {
                if metadata.document_type != document_type_wire(input.document_type) {
                    issues.push(validation_issue(
                        "DOCUMENT_TYPE_MISMATCH",
                        "error",
                        None,
                        "文档类型与生成记录不一致。",
                    ));
                }
                let fields: Vec<DocumentFieldInput> = serde_json::from_str(&metadata.fields_json)
                    .map_err(|_| {
                    BusinessToolError::new(
                        "BUSINESS_DOCUMENT_METADATA_INVALID",
                        "generated document metadata is invalid",
                        false,
                    )
                })?;
                if checks.contains(&DocumentValidationCheck::RequiredFields) {
                    let template = template_output(&metadata.template_id, 120_000)?;
                    append_missing_field_issues(&template, &fields, &mut issues);
                }
                if checks.contains(&DocumentValidationCheck::Amounts) {
                    for field in &fields {
                        if matches!(field.value, DocumentFieldValue::MoneyCents(value) if value < 0)
                        {
                            issues.push(validation_issue(
                                "DOCUMENT_AMOUNT_INVALID",
                                "error",
                                Some(field.key.clone()),
                                "金额不能为负数。",
                            ));
                        }
                    }
                }
                if checks.contains(&DocumentValidationCheck::Dates) {
                    for field in &fields {
                        if let DocumentFieldValue::Date(value) = &field.value {
                            if !valid_iso_date(value) {
                                issues.push(validation_issue(
                                    "DOCUMENT_DATE_INVALID",
                                    "error",
                                    Some(field.key.clone()),
                                    "日期必须使用 YYYY-MM-DD。",
                                ));
                            }
                        }
                    }
                }
            }
            None if checks.contains(&DocumentValidationCheck::RequiredFields) => {
                issues.push(validation_issue(
                    "DOCUMENT_STRUCTURE_UNAVAILABLE",
                    "warning",
                    None,
                    "该文档不是工作台生成件，无法核对结构化必填字段。",
                ));
            }
            None => {}
        }
        if checks.contains(&DocumentValidationCheck::SourceEvidence) {
            let connection = self.lock()?;
            let source_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM business_artifact_lineage WHERE asset_id = ?1",
                    [&asset.id],
                    |row| row.get(0),
                )
                .map_err(map_sql_error)?;
            if source_count == 0 {
                issues.push(validation_issue(
                    "DOCUMENT_SOURCE_EVIDENCE_MISSING",
                    "warning",
                    None,
                    "未登记来源 Artifact，需要人工确认依据。",
                ));
            }
        }
        let valid = issues.iter().all(|issue| issue.severity != "error");
        Ok(DocumentValidateOutput {
            artifact_id: input.artifact_id,
            document_type: input.document_type,
            valid,
            issues,
            checked_at: now_millis(),
        })
    }
}

#[derive(Debug)]
struct GeneratedDocumentMetadata {
    document_type: String,
    template_id: String,
    fields_json: String,
}

fn migrate_business_tool_storage(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS business_artifact_lineage (
                asset_id TEXT NOT NULL,
                source_asset_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY(asset_id, source_asset_id),
                FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE,
                FOREIGN KEY(source_asset_id) REFERENCES assets(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_artifact_lineage_source
                ON business_artifact_lineage(source_asset_id, asset_id);

            CREATE TABLE IF NOT EXISTS business_tool_generated_documents (
                document_id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT NOT NULL,
                document_type TEXT NOT NULL,
                template_id TEXT NOT NULL,
                fields_json TEXT NOT NULL,
                asset_id TEXT NOT NULL UNIQUE,
                idempotency_key TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                UNIQUE(project_id, idempotency_key),
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE RESTRICT,
                FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_tool_documents_project
                ON business_tool_generated_documents(project_id, created_at DESC);
            "#,
        )
        .map_err(|error| {
            HostError::internal(format!("migrate business tool storage failed: {error}"))
        })
}

fn validate_lineage_sources(
    connection: &Connection,
    project_id: &str,
    source_asset_ids: &[String],
) -> Result<(), BusinessToolError> {
    for source_asset_id in source_asset_ids {
        let source =
            asset_service::get_asset(connection, source_asset_id).map_err(map_host_error)?;
        if source.project_id.as_deref() != Some(project_id) {
            return Err(BusinessToolError::new(
                "BUSINESS_ARTIFACT_SOURCE_SCOPE_DENIED",
                "source artifact is outside the generated artifact project",
                false,
            ));
        }
    }
    Ok(())
}

fn record_artifact_lineage(
    connection: &Connection,
    asset_id: &str,
    source_asset_ids: &[String],
    created_at: i64,
) -> Result<(), BusinessToolError> {
    for source_asset_id in source_asset_ids {
        connection
            .execute(
                "INSERT OR IGNORE INTO business_artifact_lineage
                 (asset_id, source_asset_id, created_at) VALUES (?1, ?2, ?3)",
                params![asset_id, source_asset_id, created_at],
            )
            .map_err(map_sql_error)?;
    }
    Ok(())
}

fn load_generated_document_metadata(
    connection: &Connection,
    asset_id: &str,
) -> Result<Option<GeneratedDocumentMetadata>, BusinessToolError> {
    connection
        .query_row(
            "SELECT document_type, template_id, fields_json
             FROM business_tool_generated_documents WHERE asset_id = ?1",
            [asset_id],
            |row| {
                Ok(GeneratedDocumentMetadata {
                    document_type: row.get(0)?,
                    template_id: row.get(1)?,
                    fields_json: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(map_sql_error)
}

fn is_text_mime(mime_type: &str) -> bool {
    mime_type.starts_with("text/")
        || matches!(
            mime_type,
            "application/json" | "application/xml" | "application/yaml" | "application/x-yaml"
        )
}

fn relevance_score(query: &str, candidate: &str) -> Option<f32> {
    let candidate = candidate.to_lowercase();
    if candidate == query {
        return Some(1.0);
    }
    if candidate.contains(query) {
        return Some(0.9);
    }
    let tokens = query
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return None;
    }
    let matched = tokens
        .iter()
        .filter(|token| candidate.contains(**token))
        .count();
    (matched > 0).then(|| 0.45 + 0.4 * matched as f32 / tokens.len() as f32)
}

fn business_template_ids() -> [&'static str; 7] {
    [
        document_engine::QUOTE_TEMPLATE_KEY,
        document_engine::CONTRACT_TEMPLATE_KEY,
        document_engine::PAYMENT_REQUEST_TEMPLATE_KEY,
        document_engine::ACCEPTANCE_TEMPLATE_KEY,
        "builtin.tender-checklist.standard.v1",
        "builtin.business-brief.standard.v1",
        "builtin.review-report.standard.v1",
    ]
}

fn template_output(
    template_id: &str,
    max_chars: u32,
) -> Result<TemplateReadOutput, BusinessToolError> {
    let (display_name, fields) = match template_id {
        document_engine::QUOTE_TEMPLATE_KEY => (
            "标准报价单",
            vec![
                template_field("projectTitle", "项目名称", true, "text"),
                template_field("customerName", "客户名称", true, "text"),
                template_field("currency", "币种", true, "text"),
                template_field("totalCents", "含税总额", true, "moneyCents"),
                template_field("validUntil", "报价有效期", false, "date"),
            ],
        ),
        document_engine::CONTRACT_TEMPLATE_KEY => (
            "视频制作服务合同",
            vec![
                template_field("projectTitle", "项目名称", true, "text"),
                template_field("customerLegalName", "客户主体", true, "text"),
                template_field("supplierLegalName", "供应方主体", true, "text"),
                template_field("scope", "服务范围", true, "text"),
                template_field("totalCents", "合同总额", true, "moneyCents"),
                template_field("paymentTerms", "付款条款", true, "text"),
                template_field("acceptanceTerms", "验收条款", true, "text"),
            ],
        ),
        document_engine::PAYMENT_REQUEST_TEMPLATE_KEY => (
            "标准请款单",
            vec![
                template_field("projectTitle", "项目名称", true, "text"),
                template_field("customerLegalName", "客户主体", true, "text"),
                template_field("paymentAmountCents", "请款金额", true, "moneyCents"),
                template_field("dueDate", "付款日期", true, "date"),
                template_field("supplierBankAccount", "收款账户", true, "text"),
            ],
        ),
        document_engine::ACCEPTANCE_TEMPLATE_KEY => (
            "标准验收单",
            vec![
                template_field("projectTitle", "项目名称", true, "text"),
                template_field("acceptanceSummary", "验收内容", true, "text"),
                template_field("completionDate", "完成日期", true, "date"),
                template_field("deliverables", "交付清单", false, "textList"),
            ],
        ),
        "builtin.tender-checklist.standard.v1" => (
            "投标材料检查表",
            vec![
                template_field("projectTitle", "项目名称", true, "text"),
                template_field("deadline", "截止日期", true, "date"),
                template_field("requiredMaterials", "必交材料", true, "textList"),
                template_field("risks", "风险项", false, "textList"),
            ],
        ),
        "builtin.business-brief.standard.v1" => (
            "商务需求 Brief",
            vec![
                template_field("objective", "目标", true, "text"),
                template_field("audience", "受众", true, "text"),
                template_field("deliverables", "交付物", true, "textList"),
                template_field("constraints", "限制", false, "textList"),
            ],
        ),
        "builtin.review-report.standard.v1" => (
            "商务审查报告",
            vec![
                template_field("conclusion", "结论", true, "text"),
                template_field("risks", "风险项", true, "textList"),
                template_field("decisions", "人工决策", true, "textList"),
                template_field("evidence", "证据索引", false, "textList"),
            ],
        ),
        _ => {
            return Err(BusinessToolError::new(
                "BUSINESS_TEMPLATE_NOT_FOUND",
                "business template does not exist",
                false,
            ));
        }
    };
    let full_content = format!(
        "# {display_name}\n\n{}",
        fields
            .iter()
            .map(|field| format!(
                "- {{{{{}}}}}：{}{}",
                field.key,
                field.label,
                if field.required { "（必填）" } else { "" }
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let total_chars = full_content.chars().count();
    Ok(TemplateReadOutput {
        template_id: template_id.to_string(),
        display_name: display_name.to_string(),
        version: "1.0.0".to_string(),
        format: "structured-markdown".to_string(),
        content: full_content.chars().take(max_chars as usize).collect(),
        fields,
        truncated: total_chars > max_chars as usize,
    })
}

fn template_field(
    key: &str,
    label: &str,
    required: bool,
    value_kind: &str,
) -> TemplateFieldDefinition {
    TemplateFieldDefinition {
        key: key.to_string(),
        label: label.to_string(),
        required,
        value_kind: value_kind.to_string(),
    }
}

fn ledger_entries(
    workspace: &crate::protocol::BusinessWorkspaceRecord,
    currency: &str,
) -> Vec<LedgerEntryView> {
    let mut entries = Vec::new();
    let mut linked_payment_ids = HashSet::new();
    for document in &workspace.documents {
        let kind = match document.kind {
            BusinessDocumentKind::Quote => LedgerEntryKind::Quote,
            BusinessDocumentKind::Contract => LedgerEntryKind::Contract,
            BusinessDocumentKind::PaymentRequest => LedgerEntryKind::PaymentRequest,
            BusinessDocumentKind::Acceptance => LedgerEntryKind::Acceptance,
        };
        let amount_cents = document
            .snapshot
            .payment
            .as_ref()
            .map(|payment| {
                linked_payment_ids.insert(payment.id.clone());
                payment.amount_cents
            })
            .unwrap_or_else(|| {
                document
                    .snapshot
                    .profile
                    .line_items
                    .iter()
                    .fold(0_i64, |total, item| total.saturating_add(item.amount_cents))
            });
        entries.push(LedgerEntryView {
            entry_id: document.id.clone(),
            project_id: workspace.project_id.clone(),
            kind,
            status: document_ledger_status(&document.status),
            document_id: Some(document.id.clone()),
            amount_cents,
            currency: currency.to_string(),
            due_at: document
                .snapshot
                .payment
                .as_ref()
                .and_then(|payment| payment.due_at),
            updated_at: document.updated_at,
        });
    }
    for payment in &workspace.payments {
        if linked_payment_ids.contains(&payment.id) {
            continue;
        }
        entries.push(LedgerEntryView {
            entry_id: payment.id.clone(),
            project_id: workspace.project_id.clone(),
            kind: LedgerEntryKind::PaymentRequest,
            status: payment_ledger_status(&payment.status),
            document_id: None,
            amount_cents: payment.amount_cents,
            currency: currency.to_string(),
            due_at: payment.due_at,
            updated_at: payment.updated_at,
        });
    }
    for receipt in &workspace.receipts {
        entries.push(LedgerEntryView {
            entry_id: receipt.id.clone(),
            project_id: workspace.project_id.clone(),
            kind: if receipt.kind == BusinessReceiptKind::Receipt {
                LedgerEntryKind::Receipt
            } else {
                LedgerEntryKind::Adjustment
            },
            status: LedgerEntryStatus::Paid,
            document_id: None,
            amount_cents: if receipt.kind == BusinessReceiptKind::Receipt {
                receipt.amount_cents
            } else {
                -receipt.amount_cents
            },
            currency: currency.to_string(),
            due_at: None,
            updated_at: receipt.created_at,
        });
    }
    entries
}

fn document_ledger_status(status: &BusinessDocumentStatus) -> LedgerEntryStatus {
    match status {
        BusinessDocumentStatus::Draft => LedgerEntryStatus::Draft,
        BusinessDocumentStatus::InReview => LedgerEntryStatus::Submitted,
        BusinessDocumentStatus::Approved
        | BusinessDocumentStatus::Generated
        | BusinessDocumentStatus::Effective => LedgerEntryStatus::Approved,
        BusinessDocumentStatus::Voided => LedgerEntryStatus::Voided,
    }
}

fn payment_ledger_status(status: &BusinessPaymentStatus) -> LedgerEntryStatus {
    match status {
        BusinessPaymentStatus::Planned => LedgerEntryStatus::Draft,
        BusinessPaymentStatus::Requested | BusinessPaymentStatus::PartiallyReceived => {
            LedgerEntryStatus::Submitted
        }
        BusinessPaymentStatus::Received => LedgerEntryStatus::Paid,
        BusinessPaymentStatus::Canceled => LedgerEntryStatus::Voided,
    }
}

fn project_view(project: crate::protocol::ProjectRecord) -> BusinessProjectView {
    BusinessProjectView {
        id: project.id,
        name: project.name,
        client_name: project.client_name,
        stage: project.stage.as_db_str().to_string(),
        revision: project.revision,
        updated_at: project.updated_at,
        brief: BusinessProjectBriefView {
            objective: project.brief.objective,
            audience: project.brief.audience,
            deliverables: project.brief.deliverables,
            mandatory_items: project.brief.mandatory_items,
            constraints: project.brief.constraints,
            risks: project.brief.risks,
        },
    }
}

fn operation_context(
    context: &BusinessToolContext,
    project_id: Option<String>,
) -> OperationContext {
    OperationContext {
        actor_id: context.actor_id.clone(),
        account_id: context.account_id.clone(),
        project_id,
        window_id: "brain-host".to_string(),
        trace_id: context.trace_id.clone(),
    }
}

fn task_priority(priority: TaskPlanPriority) -> TaskPriority {
    match priority {
        TaskPlanPriority::Low => TaskPriority::Low,
        TaskPlanPriority::Normal => TaskPriority::Normal,
        TaskPlanPriority::High => TaskPriority::High,
        TaskPlanPriority::Urgent => TaskPriority::Critical,
    }
}

fn deterministic_uuid(value: &str) -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, value.as_bytes()).to_string()
}

fn default_template_id(document_type: BusinessDocumentType) -> &'static str {
    match document_type {
        BusinessDocumentType::Quote => document_engine::QUOTE_TEMPLATE_KEY,
        BusinessDocumentType::Contract => document_engine::CONTRACT_TEMPLATE_KEY,
        BusinessDocumentType::PaymentRequest => document_engine::PAYMENT_REQUEST_TEMPLATE_KEY,
        BusinessDocumentType::Acceptance => document_engine::ACCEPTANCE_TEMPLATE_KEY,
        BusinessDocumentType::TenderChecklist => "builtin.tender-checklist.standard.v1",
        BusinessDocumentType::Brief => "builtin.business-brief.standard.v1",
        BusinessDocumentType::ReviewReport => "builtin.review-report.standard.v1",
    }
}

fn document_type_wire(document_type: BusinessDocumentType) -> &'static str {
    match document_type {
        BusinessDocumentType::Quote => "quote",
        BusinessDocumentType::Contract => "contract",
        BusinessDocumentType::PaymentRequest => "paymentRequest",
        BusinessDocumentType::Acceptance => "acceptance",
        BusinessDocumentType::TenderChecklist => "tenderChecklist",
        BusinessDocumentType::Brief => "brief",
        BusinessDocumentType::ReviewReport => "reviewReport",
    }
}

fn ensure_required_document_fields(
    template: &TemplateReadOutput,
    fields: &[DocumentFieldInput],
) -> Result<(), BusinessToolError> {
    let present = fields
        .iter()
        .map(|field| field.key.as_str())
        .collect::<HashSet<_>>();
    let missing = template
        .fields
        .iter()
        .filter(|field| field.required && !present.contains(field.key.as_str()))
        .map(|field| field.key.clone())
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(BusinessToolError::new(
            "BUSINESS_DOCUMENT_FIELDS_MISSING",
            format!(
                "generated document is missing required fields: {}",
                missing.join(", ")
            ),
            false,
        ))
    }
}

fn render_document_content(
    document_type: BusinessDocumentType,
    format: ToolDocumentFormat,
    template: &TemplateReadOutput,
    fields: &[DocumentFieldInput],
) -> Result<String, BusinessToolError> {
    match format {
        ToolDocumentFormat::Json => {
            let mut values = Map::new();
            for field in fields {
                values.insert(field.key.clone(), document_field_json(&field.value));
            }
            serde_json::to_string_pretty(&json!({
                "documentType": document_type_wire(document_type),
                "templateId": template.template_id,
                "fields": values
            }))
            .map_err(|_| {
                BusinessToolError::new(
                    "BUSINESS_DOCUMENT_GENERATION_FAILED",
                    "generated JSON document could not be serialized",
                    false,
                )
            })
        }
        ToolDocumentFormat::Markdown => Ok(format!(
            "# {}\n\n{}",
            template.display_name,
            fields
                .iter()
                .map(|field| format!("## {}\n{}", field.key, document_field_text(&field.value)))
                .collect::<Vec<_>>()
                .join("\n\n")
        )),
        ToolDocumentFormat::PlainText => Ok(format!(
            "{}\n\n{}",
            template.display_name,
            fields
                .iter()
                .map(|field| format!("{}: {}", field.key, document_field_text(&field.value)))
                .collect::<Vec<_>>()
                .join("\n")
        )),
    }
}

fn document_field_json(value: &DocumentFieldValue) -> Value {
    match value {
        DocumentFieldValue::Text(value) | DocumentFieldValue::Date(value) => {
            Value::String(value.clone())
        }
        DocumentFieldValue::Number(value) | DocumentFieldValue::MoneyCents(value) => {
            Value::Number((*value).into())
        }
        DocumentFieldValue::Boolean(value) => Value::Bool(*value),
        DocumentFieldValue::TextList(values) => {
            Value::Array(values.iter().cloned().map(Value::String).collect())
        }
    }
}

fn document_field_text(value: &DocumentFieldValue) -> String {
    match value {
        DocumentFieldValue::Text(value) | DocumentFieldValue::Date(value) => value.clone(),
        DocumentFieldValue::Number(value) => value.to_string(),
        DocumentFieldValue::MoneyCents(value) => format!("{:.2}", *value as f64 / 100.0),
        DocumentFieldValue::Boolean(value) => if *value { "是" } else { "否" }.to_string(),
        DocumentFieldValue::TextList(values) => values.join("；"),
    }
}

fn append_missing_field_issues(
    template: &TemplateReadOutput,
    fields: &[DocumentFieldInput],
    issues: &mut Vec<DocumentValidationIssue>,
) {
    let present = fields
        .iter()
        .map(|field| field.key.as_str())
        .collect::<HashSet<_>>();
    for field in template
        .fields
        .iter()
        .filter(|field| field.required && !present.contains(field.key.as_str()))
    {
        issues.push(validation_issue(
            "DOCUMENT_REQUIRED_FIELD_MISSING",
            "error",
            Some(field.key.clone()),
            "缺少必填字段。",
        ));
    }
}

fn validation_issue(
    code: &str,
    severity: &str,
    field: Option<String>,
    message: &str,
) -> DocumentValidationIssue {
    DocumentValidationIssue {
        code: code.to_string(),
        severity: severity.to_string(),
        field,
        message: message.to_string(),
    }
}

fn valid_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
        && value[5..7]
            .parse::<u8>()
            .is_ok_and(|month| (1..=12).contains(&month))
        && value[8..10]
            .parse::<u8>()
            .is_ok_and(|day| (1..=31).contains(&day))
}

fn calculation_overflow() -> BusinessToolError {
    BusinessToolError::new(
        "BUSINESS_CALCULATION_OVERFLOW",
        "business calculation exceeds the supported numeric range",
        false,
    )
}

fn bounded_model_text(value: &str, max_chars: u32) -> String {
    value
        .split_whitespace()
        .map(|token| {
            let bytes = token.as_bytes();
            let looks_like_path = token.contains("://")
                || token.starts_with("file:")
                || token.starts_with("\\\\")
                || token.starts_with('/')
                || (bytes.len() >= 3
                    && bytes[0].is_ascii_alphabetic()
                    && bytes[1] == b':'
                    && matches!(bytes[2], b'\\' | b'/'));
            if looks_like_path {
                "[redacted]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars as usize)
        .collect()
}

fn ensure_project_scope(
    context: &BusinessToolContext,
    project_id: &str,
) -> Result<(), BusinessToolError> {
    if context
        .project_id
        .as_deref()
        .is_some_and(|expected| expected != project_id)
    {
        return Err(BusinessToolError::new(
            "BUSINESS_TOOL_PROJECT_SCOPE_DENIED",
            "business tool target is outside the active project",
            false,
        ));
    }
    Ok(())
}

fn ensure_optional_project_scope(
    context: &BusinessToolContext,
    project_id: Option<&str>,
) -> Result<(), BusinessToolError> {
    if let Some(expected) = context.project_id.as_deref() {
        if project_id != Some(expected) {
            return Err(BusinessToolError::new(
                "BUSINESS_TOOL_PROJECT_SCOPE_DENIED",
                "business tool target is outside the active project",
                false,
            ));
        }
    }
    Ok(())
}

fn asset_view(asset: &crate::protocol::AssetRecord) -> BusinessArtifactView {
    BusinessArtifactView {
        asset_id: asset.id.clone(),
        project_id: asset.project_id.clone(),
        display_name: asset.original_name.clone(),
        kind: asset.kind.as_db_str().to_string(),
        mime_type: asset.mime_type.clone(),
        size_bytes: asset.size_bytes,
        sha256: asset.sha256.clone(),
        revision: asset.revision,
        preview_available: asset.preview_available,
    }
}

fn read_text_content(
    path: &Path,
    mime_type: &str,
    max_chars: u32,
    source_sha256: &str,
) -> Result<BusinessArtifactContent, BusinessToolError> {
    let text_like = mime_type.starts_with("text/")
        || matches!(
            mime_type,
            "application/json" | "application/xml" | "application/yaml" | "application/x-yaml"
        );
    if !text_like {
        return Err(BusinessToolError::new(
            "BUSINESS_ARTIFACT_TEXT_UNSUPPORTED",
            "artifact content is not plain text; use document extraction",
            false,
        ));
    }
    let byte_limit = u64::from(max_chars)
        .saturating_mul(MAX_TEXT_READ_BYTES_PER_CHAR)
        .saturating_add(1);
    let file = File::open(path).map_err(|_| {
        BusinessToolError::new(
            "BUSINESS_ARTIFACT_READ_FAILED",
            "unable to read local artifact content",
            true,
        )
    })?;
    let source_bytes = file
        .metadata()
        .map(|metadata| metadata.len())
        .map_err(|_| {
            BusinessToolError::new(
                "BUSINESS_ARTIFACT_READ_FAILED",
                "unable to inspect local artifact content",
                true,
            )
        })?;
    let mut bytes = Vec::new();
    file.take(byte_limit).read_to_end(&mut bytes).map_err(|_| {
        BusinessToolError::new(
            "BUSINESS_ARTIFACT_READ_FAILED",
            "unable to read local artifact content",
            true,
        )
    })?;
    let read_was_bounded = source_bytes > bytes.len() as u64;
    let decoded = if read_was_bounded {
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        String::from_utf8(bytes).map_err(|_| {
            BusinessToolError::new(
                "BUSINESS_ARTIFACT_TEXT_INVALID",
                "artifact content is not valid UTF-8 text",
                false,
            )
        })?
    };
    let total_chars = decoded.chars().count();
    let truncated = read_was_bounded || total_chars > max_chars as usize;
    let text = if truncated {
        decoded.chars().take(max_chars as usize).collect()
    } else {
        decoded
    };
    Ok(BusinessArtifactContent {
        format: mime_type.to_string(),
        text,
        content_sha256: Some(source_sha256.to_string()),
        truncated,
    })
}

fn extraction_view(
    extraction: crate::protocol::DocumentExtractionRecord,
    input: &DocumentExtractInput,
) -> Result<DocumentExtractOutput, BusinessToolError> {
    let start = input.start_page as usize;
    let end = start.saturating_add(input.max_pages as usize);
    let mut remaining_chars = input.max_chars as usize;
    let mut truncated = extraction.pages.len() > end;
    let mut pages = Vec::new();
    for page in extraction
        .pages
        .iter()
        .skip(start)
        .take(input.max_pages as usize)
    {
        if remaining_chars == 0 {
            truncated = true;
            break;
        }
        let page_chars = page.text.chars().count();
        let text = if page_chars > remaining_chars {
            truncated = true;
            page.text.chars().take(remaining_chars).collect()
        } else {
            page.text.clone()
        };
        remaining_chars = remaining_chars.saturating_sub(text.chars().count());
        pages.push(DocumentPageView {
            page_index: page.page_index,
            text,
            text_sha256: page.text_sha256.clone(),
        });
    }
    let tables = extraction
        .tables
        .iter()
        .filter(|table| {
            let page = table.page_index.max(0) as usize;
            page >= start && page < end
        })
        .map(|table| DocumentTableView {
            page_index: table.page_index,
            order_index: table.order_index,
            markdown: table.markdown.clone(),
        })
        .collect();
    Ok(DocumentExtractOutput {
        extraction_id: extraction.id,
        source_asset_id: extraction.source_asset_id,
        source_asset_sha256: extraction.source_asset_sha256,
        status: enum_wire_name(&extraction.status),
        parser_name: extraction.parser.name,
        parser_version: extraction.parser.version,
        page_count: extraction.page_count,
        content_sha256: extraction.content_sha256,
        snapshot_asset_id: extraction.snapshot_asset_id,
        pages,
        tables,
        truncated,
    })
}

fn queue_backup_best_effort(
    outbox: &BackupOutbox,
    context: &BusinessToolContext,
    asset: &crate::protocol::AssetRecord,
) {
    let command = BackupCommandEnvelope::Queue {
        command_id: Uuid::new_v4().to_string(),
        protocol_version: BACKUP_PROTOCOL_VERSION.to_string(),
        context: OperationContext {
            actor_id: context.actor_id.clone(),
            account_id: context.account_id.clone(),
            project_id: asset.project_id.clone(),
            window_id: "brain-host".to_string(),
            trace_id: context.trace_id.clone(),
        },
        payload: QueueAssetBackupPayload {
            asset_id: asset.id.clone(),
        },
        idempotency_key: format!("business-tool-backup:{}:{}", asset.id, asset.sha256),
        expected_revision: None,
        deadline_at: None,
    };
    if let Err(error) = outbox.queue(command, &asset.sha256) {
        eprintln!(
            "business tool artifact committed locally; backup queue deferred: code={} retryable={}",
            error.code, error.retryable
        );
    }
}

fn enum_wire_name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn artifact_source_ref(project_id: &str, call_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project_id.as_bytes());
    hasher.update([0]);
    hasher.update(call_id.as_bytes());
    format!("business-tool:{:x}", hasher.finalize())
}

fn map_host_error(error: HostError) -> BusinessToolError {
    BusinessToolError::new(
        error.code,
        "business backend rejected the request",
        error.retryable,
    )
}

fn map_sql_error(_error: rusqlite::Error) -> BusinessToolError {
    BusinessToolError::new(
        "BUSINESS_TOOL_STORAGE_FAILED",
        "business tool storage operation failed",
        true,
    )
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::business_tool_registry::{
        ArtifactCompareMode, CalculationLineInput, CalculationMode, DocumentValidationCheck,
        TaskPlanStepInput,
    };
    use crate::protocol::{CreateProjectPayload, ProjectStage};

    fn project_create_command() -> CommandEnvelope {
        CommandEnvelope::ProjectCreate {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            context: OperationContext {
                actor_id: "operator".to_string(),
                account_id: None,
                project_id: None,
                window_id: "test".to_string(),
                trace_id: "trace-create".to_string(),
            },
            payload: CreateProjectPayload {
                name: "商务闭环测试".to_string(),
                client_name: "测试客户".to_string(),
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: None,
            deadline_at: None,
        }
    }

    #[test]
    fn real_adapter_closes_core_business_tool_paths_without_exposing_native_paths() {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("ledger").join("bsaigc.sqlite3");
        let vault = root.path().join("vault");
        let host = BackendHost::open(&database, &vault).unwrap();
        let project = host
            .execute(project_create_command())
            .unwrap()
            .response
            .project;

        let mut connection = Connection::open(&database).unwrap();
        asset_service::migrate(&connection).unwrap();
        business_workspace_service::migrate(&connection).unwrap();
        contract_review_service::migrate(&connection).unwrap();
        let left_source = root.path().join("left.txt");
        let right_source = root.path().join("right.txt");
        fs::write(&left_source, "第一条\n旧付款条款\n").unwrap();
        fs::write(&right_source, "第一条\n新付款条款\n").unwrap();
        let left =
            asset_service::import_file(&mut connection, &vault, Some(&project.id), &left_source)
                .unwrap();
        let right =
            asset_service::import_file(&mut connection, &vault, Some(&project.id), &right_source)
                .unwrap();
        let connection = Arc::new(Mutex::new(connection));
        let tasks = Arc::new(TaskEngine::open(&database).unwrap());
        let outbox = Arc::new(BackupOutbox::open(&database).unwrap());
        let adapter = BusinessToolHostAdapter::new(
            host,
            Arc::clone(&connection),
            vault.clone(),
            root.path().join("staging").join("brain-artifacts"),
            outbox,
            tasks,
        )
        .unwrap();
        let context = BusinessToolContext {
            call_id: "call-project-write".to_string(),
            actor_id: "operator".to_string(),
            account_id: None,
            project_id: Some(project.id.clone()),
            trace_id: "trace-business-tools".to_string(),
        };

        let compared = adapter
            .artifact_compare(
                &context,
                ArtifactCompareInput {
                    left_asset_id: left.id.clone(),
                    right_asset_id: right.id.clone(),
                    mode: ArtifactCompareMode::Text,
                    max_differences: 20,
                    max_chars: 10_000,
                },
            )
            .unwrap();
        assert!(!compared.differences.is_empty());

        let located = adapter
            .source_locate(
                &context,
                SourceLocateInput {
                    query: "left".to_string(),
                    project_id: Some(project.id.clone()),
                    kinds: vec![BusinessSourceKind::Artifact],
                    max_results: 10,
                    include_excerpt: true,
                    max_excerpt_chars: 200,
                },
            )
            .unwrap();
        assert_eq!(located.matches[0].source_id, left.id);

        let calculated = adapter
            .calculation(
                &context,
                CalculationInput {
                    calculation_id: "calc-1".to_string(),
                    mode: CalculationMode::Quote,
                    currency: "CNY".to_string(),
                    lines: vec![CalculationLineInput {
                        key: "shoot".to_string(),
                        description: "拍摄".to_string(),
                        quantity_milli: 2_000,
                        unit_price_cents: 50_000,
                        discount_cents: 0,
                    }],
                    discount_cents: 0,
                    tax_rate_basis_points: 600,
                },
            )
            .unwrap();
        assert_eq!(calculated.total_cents, 106_000);

        let written = adapter
            .project_write(
                &context,
                ProjectWriteInput {
                    project_id: project.id.clone(),
                    expected_revision: project.revision,
                    patch: crate::business_tool_registry::ProjectWritePatch {
                        stage: Some(ProjectStage::Briefing.as_db_str().to_string()),
                        brief: Some(BusinessProjectBriefView {
                            objective: "交付商务闭环".to_string(),
                            audience: "客户经理".to_string(),
                            deliverables: vec!["报价单".to_string()],
                            mandatory_items: vec!["金额复核".to_string()],
                            constraints: Vec::new(),
                            risks: Vec::new(),
                        }),
                        ..Default::default()
                    },
                    reason: "已确认需求".to_string(),
                },
            )
            .unwrap();
        assert_eq!(written.project.stage, "briefing");
        assert_eq!(written.project.revision, 3);

        let planned = adapter
            .task_plan(
                &context,
                TaskPlanInput {
                    project_id: project.id.clone(),
                    title: "合同处理".to_string(),
                    objective: "完成审查与报告".to_string(),
                    priority: TaskPlanPriority::High,
                    steps: vec![
                        TaskPlanStepInput {
                            key: "extract".to_string(),
                            title: "提取合同".to_string(),
                            instructions: "提取正文".to_string(),
                            owner_role: "business".to_string(),
                            depends_on: Vec::new(),
                        },
                        TaskPlanStepInput {
                            key: "review".to_string(),
                            title: "审查合同".to_string(),
                            instructions: "输出风险".to_string(),
                            owner_role: "reviewer".to_string(),
                            depends_on: vec!["extract".to_string()],
                        },
                    ],
                    idempotency_key: "plan-contract-1".to_string(),
                },
            )
            .unwrap();
        assert_eq!(planned.tasks.len(), 2);

        let fields = vec![
            field(
                "projectTitle",
                DocumentFieldValue::Text("测试项目".to_string()),
            ),
            field(
                "customerName",
                DocumentFieldValue::Text("测试客户".to_string()),
            ),
            field("currency", DocumentFieldValue::Text("CNY".to_string())),
            field("totalCents", DocumentFieldValue::MoneyCents(106_000)),
        ];
        let generated = adapter
            .document_generate(
                &context,
                DocumentGenerateInput {
                    project_id: project.id.clone(),
                    document_type: BusinessDocumentType::Quote,
                    format: ToolDocumentFormat::Markdown,
                    template_id: None,
                    fields: fields.clone(),
                    source_artifact_ids: vec![right.id.clone()],
                    idempotency_key: "document-quote-1".to_string(),
                },
            )
            .unwrap();
        let replayed = adapter
            .document_generate(
                &context,
                DocumentGenerateInput {
                    project_id: project.id.clone(),
                    document_type: BusinessDocumentType::Quote,
                    format: ToolDocumentFormat::Markdown,
                    template_id: None,
                    fields,
                    source_artifact_ids: vec![right.id],
                    idempotency_key: "document-quote-1".to_string(),
                },
            )
            .unwrap();
        assert_eq!(generated.artifact.asset_id, replayed.artifact.asset_id);

        let validated = adapter
            .document_validate(
                &context,
                DocumentValidateInput {
                    artifact_id: generated.artifact.asset_id,
                    document_type: BusinessDocumentType::Quote,
                    checks: vec![
                        DocumentValidationCheck::RequiredFields,
                        DocumentValidationCheck::ProjectBinding,
                        DocumentValidationCheck::Amounts,
                        DocumentValidationCheck::SourceEvidence,
                        DocumentValidationCheck::Formatting,
                    ],
                },
            )
            .unwrap();
        assert!(validated.valid, "{:?}", validated.issues);
    }

    fn field(key: &str, value: DocumentFieldValue) -> DocumentFieldInput {
        DocumentFieldInput {
            key: key.to_string(),
            value,
        }
    }
}
