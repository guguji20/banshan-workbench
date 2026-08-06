use crate::asset_service;
use crate::business_closure_service;
use crate::business_v1::legacy_doc_normalizer;
use crate::business_v1::production_result_confirmation_template::{
    ProductionResultConfirmationDeliveryItem, ProductionResultConfirmationImage,
    ProductionResultConfirmationShot, ProductionResultConfirmationStoryboard,
    ProductionResultConfirmationTemplateData,
};
use crate::business_v1::video_completion_acceptance_template::{
    VideoAssetReference, VideoBlock, VideoCompletionAcceptanceTemplateData, VideoDeliveryGroup,
    VideoScreenshot,
};
use crate::business_v1::{
    TemplateArtifact, TemplateConverter, TemplateVersion, TemplateVersionStatus,
};
use crate::contract_review_service;
use crate::document_engine;
use crate::protocol::{
    AdoptLatestConfirmedRequirementPayload, ApproveBusinessTemplateVersionPayload,
    AssetDomainEvent, AssignBusinessCustomerPayload, AttachBusinessInvoiceAssetPayload,
    BusinessAcceptanceBatchRecord, BusinessAcceptanceBatchStatus, BusinessAcceptanceBlocker,
    BusinessAcceptanceMaterialBinding, BusinessAcceptanceMaterialKind,
    BusinessAcceptanceMaterialRecord, BusinessAcceptanceOutputSpecRecord,
    BusinessAcceptanceReadiness, BusinessAcceptanceRequirementRecord,
    BusinessArchiveIntegrityStatus, BusinessContractSettlementData, BusinessCurrentDocuments,
    BusinessCustomerReceivableSummary, BusinessDocumentFormat, BusinessDocumentKind,
    BusinessDocumentRecord, BusinessDocumentSnapshot, BusinessDocumentStatus,
    BusinessEvidenceInput, BusinessEvidenceKind, BusinessEvidenceRecord, BusinessFinancialSummary,
    BusinessInvoiceKind, BusinessLifecycleStage, BusinessLineItem, BusinessLineItemInput,
    BusinessManualWaiverInput, BusinessManualWaiverRecord, BusinessPaymentApplicationData,
    BusinessPaymentApplicationInput, BusinessPaymentInput, BusinessPaymentRecord,
    BusinessPaymentSettlementItemData, BusinessPaymentStatus,
    BusinessProductionResultConfirmationAssetReference, BusinessProductionResultConfirmationData,
    BusinessProfile, BusinessProfileInput, BusinessQuotationTotals,
    BusinessQuoteConfirmationRecord, BusinessReceiptKind, BusinessReceiptRecord,
    BusinessServiceSettlementItemData, BusinessSettlementBatchRecord,
    BusinessSettlementBatchStatus, BusinessSettlementLineInput, BusinessSettlementLineRecord,
    BusinessTaxMode, BusinessTemplateVersionRecord, BusinessTemplateVersionStatus,
    BusinessVideoCompletionAcceptanceData, BusinessWorkspaceCommandEnvelope,
    BusinessWorkspaceCommandResponse, BusinessWorkspaceDomainEvent, BusinessWorkspaceEventType,
    BusinessWorkspacePrefillCandidate, BusinessWorkspacePrefillChange,
    BusinessWorkspacePrefillDecision, BusinessWorkspacePrefillField,
    BusinessWorkspacePrefillMatchKind, BusinessWorkspacePrefillPreview, BusinessWorkspaceRecord,
    BusinessWorkspaceStatus, ChangeBusinessDocumentStatusPayload,
    ChangeBusinessWorkspaceStatusPayload, CommandReceipt, ConfirmBusinessQuotePayload,
    CreateBusinessAcceptanceBatchPayload, CreateBusinessArchiveSnapshotPayload,
    CreateBusinessDocumentPayload, CreateBusinessWorkspacePayload, GenerateBusinessDocumentPayload,
    HostError, ListBusinessCustomersRequest, ListBusinessWorkspacePrefillCandidatesRequest,
    NormalizeBusinessLegacyTemplatePayload, OperationContext,
    PrepareBusinessAcceptanceDocumentsPayload, PreviewBusinessWorkspacePrefillRequest,
    PromoteReviewedContractPayload, RecordBusinessDeliverySentPayload,
    RecordBusinessDeliverySignoffPayload, RecordBusinessInvoiceIssuedPayload,
    RecordBusinessInvoiceRedCorrectionPayload, RecordBusinessReceiptPayload,
    RegisterBusinessDeliverableVersionPayload, RejectBusinessTemplateVersionPayload,
    RequirementBriefContent, ReverseBusinessReceiptPayload, UpdateBusinessProfilePayload,
    UpsertBusinessAcceptanceMaterialPayload, UpsertBusinessCustomerPayload,
    UpsertBusinessMilestonePayload, UpsertBusinessPaymentPayload,
    UpsertBusinessSettlementBatchPayload, VoidBusinessSettlementBatchPayload,
    BUSINESS_WORKSPACE_LEGACY_PROTOCOL_VERSION, BUSINESS_WORKSPACE_PREVIOUS_PROTOCOL_VERSION,
    BUSINESS_WORKSPACE_PROTOCOL_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_CONTEXT_CHARS: usize = 160;
const MAX_SHORT_CHARS: usize = 240;
const MAX_TEXT_CHARS: usize = 16_000;
const MAX_LINE_ITEMS: usize = 200;
const MAX_PAYMENTS_PER_WORKSPACE: i64 = 500;
const MAX_SETTLEMENT_BATCHES_PER_WORKSPACE: usize = 500;
const MAX_SETTLEMENT_LINES_PER_BATCH: usize = 500;
const MAX_DOCUMENTS_PER_WORKSPACE: i64 = 1_000;
const MAX_RECEIPTS_PER_WORKSPACE: i64 = 5_000;
const MAX_ACCEPTANCE_BATCHES_PER_WORKSPACE: i64 = 500;
const MAX_ACCEPTANCE_MATERIALS_PER_BATCH: i64 = 5_000;
const MAX_ACCEPTANCE_REQUIREMENTS_PER_BATCH: usize = 100;
const MAX_ACCEPTANCE_OUTPUT_SPECS_PER_BATCH: usize = 100;
const MAX_VIDEO_ACCEPTANCE_GROUPS: usize = 32;
const MAX_VIDEO_ACCEPTANCE_VIDEOS: usize = 128;
const MAX_VIDEO_ACCEPTANCE_SCREENSHOTS_PER_VIDEO: usize = 8;
const MAX_VIDEO_ACCEPTANCE_SCREENSHOT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PRODUCTION_RESULT_CONFIRMATION_DELIVERY_ITEMS: usize = 32;
const MAX_PRODUCTION_RESULT_CONFIRMATION_IMAGES: usize = 256;
const MAX_PRODUCTION_RESULT_CONFIRMATION_IMAGE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TEMPLATE_VERSIONS_PER_WORKSPACE: i64 = 500;
const DEFAULT_CUSTOMER_LIST_LIMIT: u32 = 100;
const MAX_CUSTOMER_LIST_LIMIT: u32 = 500;
const MAX_MONEY_CENTS: i64 = 9_000_000_000_000_000;
const MAX_QUANTITY_MILLIS: i64 = 1_000_000_000_000;
const MAX_REPLAY_LIMIT: u32 = 1_000;
const DEFAULT_PREFILL_CANDIDATE_LIMIT: u32 = 50;
const MAX_PREFILL_CANDIDATE_LIMIT: u32 = 100;
const SENSITIVE_BUSINESS_JOURNAL_FIELDS: [&str; 6] = [
    "supplierBankName",
    "supplierBankAccount",
    "supplier_bank_name",
    "supplier_bank_account",
    "supplierBankRoutingNumber",
    "supplier_bank_routing_number",
];
const REUSABLE_PREFILL_FIELDS: [BusinessWorkspacePrefillField; 15] = [
    BusinessWorkspacePrefillField::CustomerLegalName,
    BusinessWorkspacePrefillField::CustomerTaxId,
    BusinessWorkspacePrefillField::CustomerAddress,
    BusinessWorkspacePrefillField::CustomerContact,
    BusinessWorkspacePrefillField::CustomerPhone,
    BusinessWorkspacePrefillField::CustomerEmail,
    BusinessWorkspacePrefillField::SupplierLegalName,
    BusinessWorkspacePrefillField::SupplierTaxId,
    BusinessWorkspacePrefillField::SupplierAddress,
    BusinessWorkspacePrefillField::SupplierContact,
    BusinessWorkspacePrefillField::SupplierPhone,
    BusinessWorkspacePrefillField::SupplierBankName,
    BusinessWorkspacePrefillField::SupplierBankAccount,
    BusinessWorkspacePrefillField::Currency,
    BusinessWorkspacePrefillField::DefaultTaxRateBps,
];
#[derive(Debug)]
pub struct BusinessWorkspaceCommandOutcome {
    pub response: BusinessWorkspaceCommandResponse,
    pub emitted_events: Vec<BusinessWorkspaceDomainEvent>,
    pub emitted_asset_events: Vec<AssetDomainEvent>,
}

pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS business_workspaces (
                id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT NOT NULL UNIQUE,
                requirement_brief_id TEXT,
                requirement_brief_revision INTEGER,
                prefill_source_workspace_id TEXT,
                customer_name_key TEXT NOT NULL DEFAULT '',
                customer_legal_name_key TEXT NOT NULL DEFAULT '',
                profile_json TEXT NOT NULL,
                settlement_batches_json TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL CHECK(status IN ('active','archived')),
                archived_at INTEGER,
                archived_by TEXT,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE RESTRICT,
                FOREIGN KEY(requirement_brief_id) REFERENCES requirement_briefs(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_workspaces_updated
                ON business_workspaces(updated_at DESC, id DESC);

            CREATE TABLE IF NOT EXISTS business_documents (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('quote','contract','paymentRequest','acceptance')),
                sequence_number INTEGER NOT NULL CHECK(sequence_number >= 1),
                document_number TEXT NOT NULL,
                title TEXT NOT NULL,
                template_key TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('draft','inReview','approved','generated','effective','voided')),
                snapshot_json TEXT NOT NULL,
                output_asset_id TEXT,
                output_format TEXT CHECK(output_format IS NULL OR output_format IN ('docx','xlsx')),
                source_asset_id TEXT,
                review_id TEXT,
                report_asset_id TEXT,
                evidence_json TEXT,
                manual_waiver_json TEXT,
                acceptance_batch_id TEXT,
                acceptance_output_spec_id TEXT,
                voided_at INTEGER,
                voided_by TEXT,
                void_reason TEXT NOT NULL DEFAULT '',
                approved_at INTEGER,
                approved_by TEXT,
                generated_at INTEGER,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(workspace_id, kind, sequence_number),
                UNIQUE(workspace_id, document_number),
                CHECK(
                    (status IN ('approved','generated','effective') AND approved_at IS NOT NULL AND approved_by IS NOT NULL)
                    OR status NOT IN ('approved','generated','effective')
                ),
                CHECK(
                    (
                        source_asset_id IS NULL
                        AND review_id IS NULL
                        AND report_asset_id IS NULL
                    )
                    OR (
                        kind = 'contract'
                        AND status IN ('effective','voided')
                        AND source_asset_id IS NOT NULL
                        AND review_id IS NOT NULL
                        AND report_asset_id IS NOT NULL
                    )
                ),
                CHECK(
                    (
                        status = 'generated'
                        AND output_asset_id IS NOT NULL
                        AND output_format IS NOT NULL
                        AND generated_at IS NOT NULL
                    )
                    OR (
                        status = 'effective'
                        AND (
                            (
                                output_asset_id IS NOT NULL
                                AND output_format IS NOT NULL
                                AND generated_at IS NOT NULL
                            )
                            OR (
                                kind = 'contract'
                                AND source_asset_id IS NOT NULL
                                AND review_id IS NOT NULL
                                AND report_asset_id IS NOT NULL
                            )
                        )
                    )
                    OR status NOT IN ('generated','effective')
                ),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(acceptance_batch_id) REFERENCES business_acceptance_batches(id) ON DELETE RESTRICT,
                FOREIGN KEY(output_asset_id) REFERENCES assets(id) ON DELETE RESTRICT,
                FOREIGN KEY(source_asset_id) REFERENCES assets(id) ON DELETE RESTRICT,
                FOREIGN KEY(report_asset_id) REFERENCES assets(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_documents_workspace
                ON business_documents(workspace_id, created_at ASC, id ASC);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_business_documents_output_asset
                ON business_documents(output_asset_id) WHERE output_asset_id IS NOT NULL;
            CREATE TABLE IF NOT EXISTS business_acceptance_batches (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                label TEXT NOT NULL,
                requirements_json TEXT NOT NULL,
                output_specs_json TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_acceptance_batches_workspace
                ON business_acceptance_batches(workspace_id, created_at ASC, id ASC);

            CREATE TABLE IF NOT EXISTS business_template_versions (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                source_asset_id TEXT NOT NULL,
                source_sha256 TEXT NOT NULL CHECK(length(source_sha256) = 64),
                normalized_asset_id TEXT NOT NULL UNIQUE,
                normalized_sha256 TEXT NOT NULL CHECK(length(normalized_sha256) = 64),
                template_key TEXT NOT NULL,
                mapping_version TEXT NOT NULL,
                converter_engine TEXT NOT NULL,
                converter_version TEXT NOT NULL,
                converter_policy_version TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('pendingReview','approved','rejected')),
                reviewed_by TEXT,
                reviewed_at INTEGER,
                review_note TEXT NOT NULL DEFAULT '',
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(workspace_id, source_asset_id, source_sha256, template_key,
                       mapping_version, converter_policy_version),
                CHECK(
                    (status = 'pendingReview' AND reviewed_by IS NULL
                     AND reviewed_at IS NULL AND review_note = '')
                    OR
                    (status IN ('approved','rejected') AND reviewed_by IS NOT NULL
                     AND reviewed_at IS NOT NULL AND length(review_note) > 0)
                ),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(source_asset_id) REFERENCES assets(id) ON DELETE RESTRICT,
                FOREIGN KEY(normalized_asset_id) REFERENCES assets(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_template_versions_workspace
                ON business_template_versions(workspace_id, created_at ASC, id ASC);
            CREATE INDEX IF NOT EXISTS idx_business_template_versions_approved_binding
                ON business_template_versions(normalized_asset_id, normalized_sha256,
                                              template_key, mapping_version)
                WHERE status = 'approved';

            CREATE TABLE IF NOT EXISTS business_acceptance_materials (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                batch_id TEXT NOT NULL,
                requirement_id TEXT NOT NULL,
                asset_id TEXT NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN
                    ('script','video','screenshot','behindTheScenes','publishingData','invoice','proof','other')),
                group_key TEXT NOT NULL,
                confirmed INTEGER NOT NULL CHECK(confirmed IN (0,1)),
                duplicate_of_material_id TEXT,
                notes TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(batch_id, asset_id),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(batch_id) REFERENCES business_acceptance_batches(id) ON DELETE RESTRICT,
                FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE RESTRICT,
                FOREIGN KEY(duplicate_of_material_id) REFERENCES business_acceptance_materials(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_acceptance_materials_batch
                ON business_acceptance_materials(batch_id, created_at ASC, id ASC);

            CREATE TABLE IF NOT EXISTS business_payments (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                label TEXT NOT NULL,
                amount_cents INTEGER NOT NULL CHECK(amount_cents > 0),
                due_at INTEGER,
                occurred_at INTEGER,
                status TEXT NOT NULL CHECK(status IN ('planned','requested','partiallyReceived','received','canceled')),
                reference TEXT NOT NULL,
                notes TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                CHECK(status NOT IN ('partiallyReceived','received') OR occurred_at IS NOT NULL),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_payments_workspace
                ON business_payments(workspace_id, created_at ASC, id ASC);

            CREATE TABLE IF NOT EXISTS business_quote_confirmations (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                quote_document_id TEXT NOT NULL,
                quote_document_revision INTEGER NOT NULL CHECK(quote_document_revision >= 1),
                quote_asset_id TEXT NOT NULL,
                quote_sha256 TEXT NOT NULL CHECK(length(quote_sha256) = 64),
                confirmation_version TEXT NOT NULL,
                customer_representative TEXT NOT NULL,
                evidence_json TEXT NOT NULL,
                notes TEXT NOT NULL,
                confirmed_by TEXT NOT NULL,
                confirmed_at INTEGER NOT NULL,
                UNIQUE(workspace_id, quote_document_id, quote_document_revision,
                       quote_sha256, confirmation_version),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(quote_document_id) REFERENCES business_documents(id) ON DELETE RESTRICT,
                FOREIGN KEY(quote_asset_id) REFERENCES assets(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_quote_confirmations_workspace
                ON business_quote_confirmations(workspace_id, confirmed_at ASC, id ASC);

            CREATE TABLE IF NOT EXISTS business_receipts (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                payment_id TEXT NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('receipt','reversal')),
                amount_cents INTEGER NOT NULL CHECK(amount_cents > 0),
                occurred_at INTEGER NOT NULL,
                reference TEXT NOT NULL UNIQUE,
                notes TEXT NOT NULL,
                reverses_receipt_id TEXT,
                evidence_json TEXT,
                recorded_by TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                CHECK(
                    (kind = 'receipt' AND reverses_receipt_id IS NULL)
                    OR (kind = 'reversal' AND reverses_receipt_id IS NOT NULL)
                ),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(payment_id) REFERENCES business_payments(id) ON DELETE RESTRICT,
                FOREIGN KEY(reverses_receipt_id) REFERENCES business_receipts(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_receipts_workspace
                ON business_receipts(workspace_id, occurred_at ASC, created_at ASC, id ASC);
            CREATE INDEX IF NOT EXISTS idx_business_receipts_payment
                ON business_receipts(payment_id, occurred_at ASC, created_at ASC, id ASC);

            CREATE TABLE IF NOT EXISTS business_workspace_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL CHECK(event_type IN
                    ('businessWorkspace.created','businessWorkspace.profileUpdated',
                     'businessWorkspace.documentCreated','businessWorkspace.documentStatusChanged',
                     'businessWorkspace.documentGenerated','businessWorkspace.reviewedContractPromoted',
                     'businessWorkspace.paymentUpserted','businessWorkspace.settlementBatchUpserted',
                     'businessWorkspace.settlementBatchVoided','businessWorkspace.quoteConfirmed',
                     'businessWorkspace.receiptRecorded','businessWorkspace.receiptReversed',
                     'businessWorkspace.requirementAdopted','businessWorkspace.customerUpserted',
                     'businessWorkspace.customerAssigned','businessWorkspace.milestoneUpserted',
                     'businessWorkspace.deliverableVersionRegistered','businessWorkspace.deliverySent',
                     'businessWorkspace.deliverySignoffRecorded','businessWorkspace.invoiceIssued',
                     'businessWorkspace.invoiceRedCorrected','businessWorkspace.invoiceAssetAttached',
                     'businessWorkspace.acceptanceBatchCreated','businessWorkspace.acceptanceDocumentsPrepared',
                     'businessWorkspace.acceptanceMaterialUpserted',
                     'businessWorkspace.archiveSnapshotPrepared',
                     'businessWorkspace.templateVersionNormalized',
                     'businessWorkspace.templateVersionApproved',
                     'businessWorkspace.templateVersionRejected',
                     'businessWorkspace.statusChanged')),
                aggregate_id TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                occurred_at INTEGER NOT NULL,
                trace_id TEXT NOT NULL,
                actor_id TEXT NOT NULL DEFAULT '',
                command_id TEXT NOT NULL DEFAULT '',
                reason TEXT NOT NULL DEFAULT '',
                payload_json TEXT NOT NULL,
                FOREIGN KEY(aggregate_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_workspace_events_aggregate
                ON business_workspace_events(aggregate_id, sequence);

            CREATE TABLE IF NOT EXISTS business_workspace_command_receipts (
                idempotency_key TEXT PRIMARY KEY NOT NULL,
                command_id TEXT NOT NULL UNIQUE,
                command_type TEXT NOT NULL CHECK(command_type IN
                    ('businessWorkspace.create','businessWorkspace.updateProfile',
                     'businessWorkspace.createDocument','businessWorkspace.changeDocumentStatus',
                     'businessWorkspace.generateDocument','businessWorkspace.promoteReviewedContract',
                     'businessWorkspace.upsertPayment','businessWorkspace.upsertSettlementBatch',
                     'businessWorkspace.voidSettlementBatch','businessWorkspace.confirmQuote',
                     'businessWorkspace.recordReceipt','businessWorkspace.reverseReceipt',
                     'businessWorkspace.adoptLatestConfirmedRequirement','businessWorkspace.upsertCustomer',
                     'businessWorkspace.assignCustomer','businessWorkspace.upsertMilestone',
                     'businessWorkspace.registerDeliverableVersion','businessWorkspace.recordDeliverySent',
                     'businessWorkspace.recordDeliverySignoff','businessWorkspace.recordInvoiceIssued',
                     'businessWorkspace.recordInvoiceRedCorrection','businessWorkspace.attachInvoiceAsset',
                     'businessWorkspace.createAcceptanceBatch','businessWorkspace.prepareAcceptanceDocuments',
                     'businessWorkspace.upsertAcceptanceMaterial',
                     'businessWorkspace.createArchiveSnapshot',
                     'businessWorkspace.normalizeLegacyTemplate',
                     'businessWorkspace.approveTemplateVersion',
                     'businessWorkspace.rejectTemplateVersion',
                     'businessWorkspace.changeStatus')),
                protocol_version TEXT NOT NULL,
                deadline_at INTEGER,
                request_fingerprint TEXT NOT NULL CHECK(length(request_fingerprint) = 64),
                response_json TEXT NOT NULL,
                completed_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_business_workspace_receipts_completed
                ON business_workspace_command_receipts(completed_at);

            CREATE TABLE IF NOT EXISTS business_generated_asset_gc (
                asset_id TEXT PRIMARY KEY NOT NULL,
                storage_rel_path TEXT NOT NULL,
                queued_at INTEGER NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts >= 0),
                last_error TEXT NOT NULL DEFAULT '',
                updated_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_business_generated_asset_gc_updated
                ON business_generated_asset_gc(updated_at ASC, asset_id ASC);
            "#,
        )
        .map_err(sql_error)?;
    ensure_prefill_source_column(connection)?;
    ensure_prefill_identity_columns(connection)?;
    ensure_settlement_batches_column(connection)?;
    migrate_reviewed_contract_binding(connection)?;
    ensure_workspace_lifecycle_columns(connection)?;
    ensure_document_lifecycle_columns(connection)?;
    ensure_acceptance_document_schema(connection)?;
    migrate_payment_ledger_schema(connection)?;
    ensure_quote_confirmation_schema(connection)?;
    ensure_receipt_schema(connection)?;
    migrate_legacy_received_payments(connection)?;
    business_closure_service::migrate(connection)?;
    ensure_event_audit_columns(connection)?;
    migrate_event_type_constraint(connection)?;
    migrate_receipt_protocol_constraint(connection)?;
    redact_historical_business_journals(connection)
}

fn redact_historical_business_journals(connection: &Connection) -> Result<(), HostError> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    redact_historical_business_journal_column(
        &transaction,
        "business_workspace_events",
        "payload_json",
    )?;
    redact_historical_business_journal_column(
        &transaction,
        "business_workspace_command_receipts",
        "response_json",
    )?;
    transaction.commit().map_err(sql_error)
}

fn redact_historical_business_journal_column(
    transaction: &Transaction<'_>,
    table: &str,
    column: &str,
) -> Result<(), HostError> {
    debug_assert!(matches!(
        (table, column),
        ("business_workspace_events", "payload_json")
            | ("business_workspace_command_receipts", "response_json")
    ));
    let rows = {
        let mut statement = transaction
            .prepare(&format!("SELECT rowid, {column} FROM {table}"))
            .map_err(sql_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        rows
    };
    for (rowid, raw_json) in rows {
        let mut value: serde_json::Value = serde_json::from_str(&raw_json).map_err(json_error)?;
        if redact_sensitive_business_journal_fields(&mut value) {
            transaction
                .execute(
                    &format!("UPDATE {table} SET {column} = ?1 WHERE rowid = ?2"),
                    params![serde_json::to_string(&value).map_err(json_error)?, rowid],
                )
                .map_err(sql_error)?;
        }
    }
    Ok(())
}

fn redact_sensitive_business_journal_fields(value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(fields) => {
            let mut changed = false;
            for (key, value) in fields {
                if SENSITIVE_BUSINESS_JOURNAL_FIELDS.contains(&key.as_str()) {
                    if value.as_str() != Some("") {
                        *value = serde_json::Value::String(String::new());
                        changed = true;
                    }
                } else {
                    changed |= redact_sensitive_business_journal_fields(value);
                }
            }
            changed
        }
        serde_json::Value::Array(values) => values.iter_mut().fold(false, |changed, value| {
            redact_sensitive_business_journal_fields(value) || changed
        }),
        _ => false,
    }
}

fn serialize_business_journal<T: serde::Serialize>(value: &T) -> Result<String, HostError> {
    let mut value = serde_json::to_value(value).map_err(json_error)?;
    redact_sensitive_business_journal_fields(&mut value);
    serde_json::to_string(&value).map_err(json_error)
}

fn ensure_prefill_source_column(connection: &Connection) -> Result<(), HostError> {
    let exists = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM pragma_table_info('business_workspaces')
                 WHERE name = 'prefill_source_workspace_id'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sql_error)?;
    if !exists {
        connection
            .execute(
                "ALTER TABLE business_workspaces
                 ADD COLUMN prefill_source_workspace_id TEXT",
                [],
            )
            .map_err(sql_error)?;
    }
    Ok(())
}

fn ensure_settlement_batches_column(connection: &Connection) -> Result<(), HostError> {
    let exists = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM pragma_table_info('business_workspaces')
                 WHERE name = 'settlement_batches_json'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sql_error)?;
    if !exists {
        connection
            .execute(
                "ALTER TABLE business_workspaces
                 ADD COLUMN settlement_batches_json TEXT NOT NULL DEFAULT '[]'",
                [],
            )
            .map_err(sql_error)?;
    }
    Ok(())
}

fn ensure_prefill_identity_columns(connection: &Connection) -> Result<(), HostError> {
    for column in ["customer_name_key", "customer_legal_name_key"] {
        let exists = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM pragma_table_info('business_workspaces')
                     WHERE name = ?1
                 )",
                [column],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sql_error)?;
        if !exists {
            connection
                .execute(
                    &format!(
                        "ALTER TABLE business_workspaces ADD COLUMN {column} TEXT NOT NULL DEFAULT ''"
                    ),
                    [],
                )
                .map_err(sql_error)?;
        }
    }

    let rows = {
        let mut statement = connection
            .prepare(
                "SELECT id, profile_json, customer_name_key, customer_legal_name_key
                 FROM business_workspaces
                 WHERE customer_name_key = '' OR customer_legal_name_key = ''",
            )
            .map_err(sql_error)?;
        let rows = statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let profile_json: String = row.get(1)?;
                let profile: BusinessProfile = from_json_column(&profile_json)?;
                let customer_name_key: String = row.get(2)?;
                let customer_legal_name_key: String = row.get(3)?;
                Ok((id, profile, customer_name_key, customer_legal_name_key))
            })
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        rows
    };
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    for (workspace_id, profile, current_name_key, current_legal_name_key) in rows {
        let expected_name_key = normalized_business_identity(&profile.customer_name);
        let expected_legal_name_key = normalized_business_identity(&profile.customer_legal_name);
        if current_name_key != expected_name_key
            || current_legal_name_key != expected_legal_name_key
        {
            transaction
                .execute(
                    "UPDATE business_workspaces
                     SET customer_name_key = ?1, customer_legal_name_key = ?2
                     WHERE id = ?3",
                    params![expected_name_key, expected_legal_name_key, workspace_id],
                )
                .map_err(sql_error)?;
        }
    }
    transaction
        .execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_business_workspaces_customer_name_key_updated
                 ON business_workspaces(customer_name_key, updated_at DESC, id DESC);
             CREATE INDEX IF NOT EXISTS idx_business_workspaces_customer_legal_name_key_updated
                 ON business_workspaces(customer_legal_name_key, updated_at DESC, id DESC);",
        )
        .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)
}

fn ensure_table_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), HostError> {
    let exists = connection
        .query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1)"),
            [column],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sql_error)?;
    if !exists {
        connection
            .execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
                [],
            )
            .map_err(sql_error)?;
    }
    Ok(())
}

fn ensure_workspace_lifecycle_columns(connection: &Connection) -> Result<(), HostError> {
    ensure_table_column(
        connection,
        "business_workspaces",
        "requirement_brief_revision",
        "INTEGER",
    )?;
    ensure_table_column(connection, "business_workspaces", "archived_at", "INTEGER")?;
    ensure_table_column(connection, "business_workspaces", "archived_by", "TEXT")?;
    let requirement_briefs_exist = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'requirement_briefs'
             )",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(sql_error)?
        != 0;
    if requirement_briefs_exist {
        connection
            .execute(
                "UPDATE business_workspaces
                 SET requirement_brief_revision = (
                     SELECT revision FROM requirement_briefs
                     WHERE requirement_briefs.id = business_workspaces.requirement_brief_id
                 )
                 WHERE requirement_brief_id IS NOT NULL
                   AND requirement_brief_revision IS NULL",
                [],
            )
            .map_err(sql_error)?;
    }
    connection
        .execute(
            "UPDATE business_workspaces
             SET archived_at = COALESCE(archived_at, updated_at),
                 archived_by = COALESCE(archived_by, 'legacy-migration')
             WHERE status = 'archived'",
            [],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn ensure_document_lifecycle_columns(connection: &Connection) -> Result<(), HostError> {
    // Review binding columns are established by migrate_reviewed_contract_binding first.
    // Keep the dependent index here so legacy tables are upgraded before SQLite resolves review_id.
    connection
        .execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_business_documents_review
                 ON business_documents(review_id) WHERE review_id IS NOT NULL;",
        )
        .map_err(sql_error)?;
    ensure_table_column(connection, "business_documents", "evidence_json", "TEXT")?;
    ensure_table_column(
        connection,
        "business_documents",
        "manual_waiver_json",
        "TEXT",
    )?;
    ensure_table_column(connection, "business_documents", "voided_at", "INTEGER")?;
    ensure_table_column(connection, "business_documents", "voided_by", "TEXT")?;
    ensure_table_column(
        connection,
        "business_documents",
        "void_reason",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_table_column(
        connection,
        "business_documents",
        "acceptance_batch_id",
        "TEXT",
    )?;
    connection
        .execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_business_documents_acceptance_batch
                 ON business_documents(acceptance_batch_id)
                 WHERE acceptance_batch_id IS NOT NULL;",
        )
        .map_err(sql_error)?;
    Ok(())
}

fn ensure_acceptance_document_schema(connection: &Connection) -> Result<(), HostError> {
    ensure_table_column(
        connection,
        "business_documents",
        "acceptance_output_spec_id",
        "TEXT",
    )?;

    let legacy_links = {
        let mut statement = connection
            .prepare(
                "SELECT document.id, document.document_number, document.title,
                        document.template_key, document.snapshot_json, batch.revision,
                        batch.output_specs_json
                 FROM business_documents document
                 JOIN business_acceptance_batches batch
                   ON batch.id = document.acceptance_batch_id
                  AND batch.workspace_id = document.workspace_id
                 WHERE document.kind = 'acceptance'
                   AND document.acceptance_batch_id IS NOT NULL
                   AND document.acceptance_output_spec_id IS NULL",
            )
            .map_err(sql_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        rows
    };
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    for (
        document_id,
        document_number,
        title,
        template_key,
        snapshot_json,
        batch_revision,
        specs_json,
    ) in legacy_links
    {
        let specs: Vec<BusinessAcceptanceOutputSpecRecord> =
            serde_json::from_str(&specs_json).map_err(json_error)?;
        let Some(spec) = specs.iter().find(|spec| {
            spec.document_number == document_number
                && spec.title == title
                && spec.template_key == template_key
        }) else {
            continue;
        };
        let mut snapshot: BusinessDocumentSnapshot =
            serde_json::from_str(&snapshot_json).map_err(json_error)?;
        snapshot.acceptance_output_spec_id = Some(spec.id.clone());
        snapshot
            .acceptance_batch_revision
            .get_or_insert(batch_revision);
        transaction
            .execute(
                "UPDATE business_documents
                 SET acceptance_output_spec_id = ?1, snapshot_json = ?2
                 WHERE id = ?3 AND acceptance_output_spec_id IS NULL",
                params![
                    spec.id,
                    serde_json::to_string(&snapshot).map_err(json_error)?,
                    document_id,
                ],
            )
            .map_err(sql_error)?;
    }
    transaction
        .execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_business_documents_acceptance_output
                 ON business_documents(workspace_id, acceptance_batch_id, acceptance_output_spec_id)
                 WHERE acceptance_batch_id IS NOT NULL AND acceptance_output_spec_id IS NOT NULL;",
        )
        .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)
}

fn migrate_payment_ledger_schema(connection: &Connection) -> Result<(), HostError> {
    let table_sql = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'business_payments'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::internal("business payment table is missing after migration"))?;
    if table_sql.contains("partiallyReceived") {
        return Ok(());
    }
    connection
        .execute_batch(
            r#"
            BEGIN IMMEDIATE;
            DROP TABLE IF EXISTS business_receipts;
            DROP INDEX IF EXISTS idx_business_payments_workspace;
            ALTER TABLE business_payments RENAME TO business_payments_before_receipt_ledger;
            CREATE TABLE business_payments (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                label TEXT NOT NULL,
                amount_cents INTEGER NOT NULL CHECK(amount_cents > 0),
                due_at INTEGER,
                occurred_at INTEGER,
                status TEXT NOT NULL CHECK(status IN
                    ('planned','requested','partiallyReceived','received','canceled')),
                reference TEXT NOT NULL,
                notes TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                CHECK(status NOT IN ('partiallyReceived','received') OR occurred_at IS NOT NULL),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT
            );
            INSERT INTO business_payments
                (id, workspace_id, label, amount_cents, due_at, occurred_at, status,
                 reference, notes, revision, created_at, updated_at)
            SELECT id, workspace_id, label, amount_cents, due_at, occurred_at, status,
                   reference, notes, revision, created_at, updated_at
            FROM business_payments_before_receipt_ledger;
            DROP TABLE business_payments_before_receipt_ledger;
            CREATE INDEX idx_business_payments_workspace
                ON business_payments(workspace_id, created_at ASC, id ASC);
            COMMIT;
            "#,
        )
        .map_err(sql_error)
}

fn ensure_quote_confirmation_schema(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS business_quote_confirmations (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                quote_document_id TEXT NOT NULL,
                quote_document_revision INTEGER NOT NULL CHECK(quote_document_revision >= 1),
                quote_asset_id TEXT NOT NULL,
                quote_sha256 TEXT NOT NULL CHECK(length(quote_sha256) = 64),
                confirmation_version TEXT NOT NULL,
                customer_representative TEXT NOT NULL,
                evidence_json TEXT NOT NULL,
                notes TEXT NOT NULL,
                confirmed_by TEXT NOT NULL,
                confirmed_at INTEGER NOT NULL,
                UNIQUE(workspace_id, quote_document_id, quote_document_revision,
                       quote_sha256, confirmation_version),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(quote_document_id) REFERENCES business_documents(id) ON DELETE RESTRICT,
                FOREIGN KEY(quote_asset_id) REFERENCES assets(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_quote_confirmations_workspace
                ON business_quote_confirmations(workspace_id, confirmed_at ASC, id ASC);
            "#,
        )
        .map_err(sql_error)
}

fn ensure_receipt_schema(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS business_receipts (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                payment_id TEXT NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('receipt','reversal')),
                amount_cents INTEGER NOT NULL CHECK(amount_cents > 0),
                occurred_at INTEGER NOT NULL,
                reference TEXT NOT NULL UNIQUE,
                notes TEXT NOT NULL,
                reverses_receipt_id TEXT,
                evidence_json TEXT,
                recorded_by TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                CHECK(
                    (kind = 'receipt' AND reverses_receipt_id IS NULL)
                    OR (kind = 'reversal' AND reverses_receipt_id IS NOT NULL)
                ),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(payment_id) REFERENCES business_payments(id) ON DELETE RESTRICT,
                FOREIGN KEY(reverses_receipt_id) REFERENCES business_receipts(id) ON DELETE RESTRICT
            );
            CREATE INDEX IF NOT EXISTS idx_business_receipts_workspace
                ON business_receipts(workspace_id, occurred_at ASC, created_at ASC, id ASC);
            CREATE INDEX IF NOT EXISTS idx_business_receipts_payment
                ON business_receipts(payment_id, occurred_at ASC, created_at ASC, id ASC);
            "#,
        )
        .map_err(sql_error)
}

fn migrate_legacy_received_payments(connection: &Connection) -> Result<(), HostError> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    transaction
        .execute(
            "INSERT OR IGNORE INTO business_receipts
             (id, workspace_id, payment_id, kind, amount_cents, occurred_at, reference,
              notes, reverses_receipt_id, evidence_json, recorded_by, created_at)
             SELECT lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
                    substr(lower(hex(randomblob(2))), 2) || '-a' ||
                    substr(lower(hex(randomblob(2))), 2) || '-' || lower(hex(randomblob(6))),
                    workspace_id, id, 'receipt', amount_cents,
                    COALESCE(occurred_at, updated_at), 'legacy:' || id,
                    'Migrated from legacy received payment status', NULL, NULL,
                    'legacy-migration', COALESCE(occurred_at, updated_at)
             FROM business_payments
             WHERE status = 'received'
               AND NOT EXISTS (
                   SELECT 1 FROM business_receipts existing
                   WHERE existing.payment_id = business_payments.id
               )",
            [],
        )
        .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)
}

fn migrate_reviewed_contract_binding(connection: &Connection) -> Result<(), HostError> {
    let table_sql = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'business_documents'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(sql_error)?;
    let Some(table_sql) = table_sql else {
        return Err(HostError::internal(
            "business document table is missing after migration",
        ));
    };
    if table_sql.contains("'effective'")
        && ["source_asset_id", "review_id", "report_asset_id"]
            .iter()
            .all(|column| table_sql.contains(column))
    {
        return Ok(());
    }
    connection
        .execute_batch(
            r#"
            BEGIN IMMEDIATE;
            DROP INDEX IF EXISTS idx_business_documents_workspace;
            DROP INDEX IF EXISTS idx_business_documents_output_asset;
            DROP INDEX IF EXISTS idx_business_documents_review;
            ALTER TABLE business_documents RENAME TO business_documents_before_review_binding;
            CREATE TABLE business_documents (
                id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('quote','contract','paymentRequest','acceptance')),
                sequence_number INTEGER NOT NULL CHECK(sequence_number >= 1),
                document_number TEXT NOT NULL,
                title TEXT NOT NULL,
                template_key TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('draft','inReview','approved','generated','effective','voided')),
                snapshot_json TEXT NOT NULL,
                output_asset_id TEXT,
                output_format TEXT CHECK(output_format IS NULL OR output_format IN ('docx','xlsx')),
                source_asset_id TEXT,
                review_id TEXT,
                report_asset_id TEXT,
                approved_at INTEGER,
                approved_by TEXT,
                generated_at INTEGER,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(workspace_id, kind, sequence_number),
                UNIQUE(workspace_id, document_number),
                CHECK(
                    (status IN ('approved','generated','effective') AND approved_at IS NOT NULL AND approved_by IS NOT NULL)
                    OR status NOT IN ('approved','generated','effective')
                ),
                CHECK(
                    (
                        source_asset_id IS NULL
                        AND review_id IS NULL
                        AND report_asset_id IS NULL
                    )
                    OR (
                        kind = 'contract'
                        AND status IN ('effective','voided')
                        AND source_asset_id IS NOT NULL
                        AND review_id IS NOT NULL
                        AND report_asset_id IS NOT NULL
                    )
                ),
                CHECK(
                    (
                        status = 'generated'
                        AND output_asset_id IS NOT NULL
                        AND output_format IS NOT NULL
                        AND generated_at IS NOT NULL
                    )
                    OR (
                        status = 'effective'
                        AND (
                            (
                                output_asset_id IS NOT NULL
                                AND output_format IS NOT NULL
                                AND generated_at IS NOT NULL
                            )
                            OR (
                                kind = 'contract'
                                AND source_asset_id IS NOT NULL
                                AND review_id IS NOT NULL
                                AND report_asset_id IS NOT NULL
                            )
                        )
                    )
                    OR status NOT IN ('generated','effective')
                ),
                FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                FOREIGN KEY(output_asset_id) REFERENCES assets(id) ON DELETE RESTRICT,
                FOREIGN KEY(source_asset_id) REFERENCES assets(id) ON DELETE RESTRICT,
                FOREIGN KEY(report_asset_id) REFERENCES assets(id) ON DELETE RESTRICT
            );
            INSERT INTO business_documents
                (id, workspace_id, kind, sequence_number, document_number, title,
                 template_key, status, snapshot_json, output_asset_id, output_format,
                 source_asset_id, review_id, report_asset_id, approved_at, approved_by,
                 generated_at, revision, created_at, updated_at)
            SELECT id, workspace_id, kind, sequence_number, document_number, title,
                   template_key, status, snapshot_json, output_asset_id, output_format,
                   NULL, NULL, NULL, approved_at, approved_by, generated_at, revision,
                   created_at, updated_at
            FROM business_documents_before_review_binding;
            DROP TABLE business_documents_before_review_binding;
            CREATE INDEX idx_business_documents_workspace
                ON business_documents(workspace_id, created_at ASC, id ASC);
            CREATE UNIQUE INDEX idx_business_documents_output_asset
                ON business_documents(output_asset_id) WHERE output_asset_id IS NOT NULL;
            CREATE UNIQUE INDEX idx_business_documents_review
                ON business_documents(review_id) WHERE review_id IS NOT NULL;
            COMMIT;
            "#,
        )
        .map_err(sql_error)
}

fn ensure_event_audit_columns(connection: &Connection) -> Result<(), HostError> {
    for column in ["actor_id", "command_id", "reason"] {
        let exists = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM pragma_table_info('business_workspace_events')
                     WHERE name = ?1
                 )",
                [column],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sql_error)?;
        if !exists {
            connection
                .execute(
                    &format!(
                        "ALTER TABLE business_workspace_events ADD COLUMN {column} TEXT NOT NULL DEFAULT ''"
                    ),
                    [],
                )
                .map_err(sql_error)?;
        }
    }
    Ok(())
}
fn migrate_event_type_constraint(connection: &Connection) -> Result<(), HostError> {
    let table_sql = connection
        .query_row(
            "SELECT sql FROM sqlite_master
             WHERE type = 'table' AND name = 'business_workspace_events'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(sql_error)?;
    let Some(table_sql) = table_sql else {
        return Err(HostError::internal(
            "business workspace event table is missing after migration",
        ));
    };
    if table_sql.contains("businessWorkspace.reviewedContractPromoted")
        && table_sql.contains("businessWorkspace.settlementBatchUpserted")
        && table_sql.contains("businessWorkspace.settlementBatchVoided")
        && table_sql.contains("businessWorkspace.quoteConfirmed")
        && table_sql.contains("businessWorkspace.receiptRecorded")
        && table_sql.contains("businessWorkspace.receiptReversed")
        && table_sql.contains("businessWorkspace.requirementAdopted")
        && table_sql.contains("businessWorkspace.acceptanceBatchCreated")
        && table_sql.contains("businessWorkspace.acceptanceDocumentsPrepared")
        && table_sql.contains("businessWorkspace.acceptanceMaterialUpserted")
        && table_sql.contains("businessWorkspace.archiveSnapshotPrepared")
        && table_sql.contains("businessWorkspace.templateVersionNormalized")
        && table_sql.contains("businessWorkspace.templateVersionApproved")
        && table_sql.contains("businessWorkspace.templateVersionRejected")
    {
        return Ok(());
    }
    connection
        .execute_batch(
            r#"
            BEGIN IMMEDIATE;
            DROP INDEX IF EXISTS idx_business_workspace_events_aggregate;
            ALTER TABLE business_workspace_events
                RENAME TO business_workspace_events_before_review_promotion;
            CREATE TABLE business_workspace_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL CHECK(event_type IN
                    ('businessWorkspace.created','businessWorkspace.profileUpdated',
                     'businessWorkspace.documentCreated','businessWorkspace.documentStatusChanged',
                     'businessWorkspace.documentGenerated','businessWorkspace.reviewedContractPromoted',
                     'businessWorkspace.paymentUpserted','businessWorkspace.settlementBatchUpserted',
                     'businessWorkspace.settlementBatchVoided','businessWorkspace.quoteConfirmed',
                     'businessWorkspace.receiptRecorded','businessWorkspace.receiptReversed',
                     'businessWorkspace.requirementAdopted','businessWorkspace.customerUpserted',
                     'businessWorkspace.customerAssigned','businessWorkspace.milestoneUpserted',
                     'businessWorkspace.deliverableVersionRegistered','businessWorkspace.deliverySent',
                     'businessWorkspace.deliverySignoffRecorded','businessWorkspace.invoiceIssued',
                     'businessWorkspace.invoiceRedCorrected','businessWorkspace.invoiceAssetAttached',
                     'businessWorkspace.acceptanceBatchCreated','businessWorkspace.acceptanceDocumentsPrepared',
                     'businessWorkspace.acceptanceMaterialUpserted',
                     'businessWorkspace.archiveSnapshotPrepared',
                     'businessWorkspace.templateVersionNormalized',
                     'businessWorkspace.templateVersionApproved',
                     'businessWorkspace.templateVersionRejected',
                     'businessWorkspace.statusChanged')),
                aggregate_id TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision >= 1),
                occurred_at INTEGER NOT NULL,
                trace_id TEXT NOT NULL,
                actor_id TEXT NOT NULL DEFAULT '',
                command_id TEXT NOT NULL DEFAULT '',
                reason TEXT NOT NULL DEFAULT '',
                payload_json TEXT NOT NULL,
                FOREIGN KEY(aggregate_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT
            );
            INSERT INTO business_workspace_events
                (sequence, event_id, event_type, aggregate_id, revision, occurred_at,
                 trace_id, actor_id, command_id, reason, payload_json)
            SELECT sequence, event_id, event_type, aggregate_id, revision, occurred_at,
                   trace_id, actor_id, command_id, reason, payload_json
            FROM business_workspace_events_before_review_promotion;
            DROP TABLE business_workspace_events_before_review_promotion;
            CREATE INDEX idx_business_workspace_events_aggregate
                ON business_workspace_events(aggregate_id, sequence);
            COMMIT;
            "#,
        )
        .map_err(sql_error)
}

fn migrate_receipt_protocol_constraint(connection: &Connection) -> Result<(), HostError> {
    let table_sql = connection
        .query_row(
            "SELECT sql FROM sqlite_master
             WHERE type = 'table' AND name = 'business_workspace_command_receipts'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(sql_error)?;
    let Some(table_sql) = table_sql else {
        return Err(HostError::internal(
            "business workspace command receipt table is missing after migration",
        ));
    };
    if table_sql.contains("businessWorkspace.promoteReviewedContract")
        && table_sql.contains("businessWorkspace.upsertSettlementBatch")
        && table_sql.contains("businessWorkspace.voidSettlementBatch")
        && table_sql.contains("businessWorkspace.confirmQuote")
        && table_sql.contains("businessWorkspace.recordReceipt")
        && table_sql.contains("businessWorkspace.reverseReceipt")
        && table_sql.contains("businessWorkspace.adoptLatestConfirmedRequirement")
        && table_sql.contains("businessWorkspace.createAcceptanceBatch")
        && table_sql.contains("businessWorkspace.prepareAcceptanceDocuments")
        && table_sql.contains("businessWorkspace.upsertAcceptanceMaterial")
        && table_sql.contains("businessWorkspace.createArchiveSnapshot")
        && table_sql.contains("businessWorkspace.normalizeLegacyTemplate")
        && table_sql.contains("businessWorkspace.approveTemplateVersion")
        && table_sql.contains("businessWorkspace.rejectTemplateVersion")
        && !table_sql.contains("CHECK(protocol_version = '1.4')")
    {
        return Ok(());
    }
    connection
        .execute_batch(
            r#"
            BEGIN IMMEDIATE;
            DROP INDEX IF EXISTS idx_business_workspace_receipts_completed;
            ALTER TABLE business_workspace_command_receipts
                RENAME TO business_workspace_command_receipts_before_review_promotion;
            CREATE TABLE business_workspace_command_receipts (
                idempotency_key TEXT PRIMARY KEY NOT NULL,
                command_id TEXT NOT NULL UNIQUE,
                command_type TEXT NOT NULL CHECK(command_type IN
                    ('businessWorkspace.create','businessWorkspace.updateProfile',
                     'businessWorkspace.createDocument','businessWorkspace.changeDocumentStatus',
                     'businessWorkspace.generateDocument','businessWorkspace.promoteReviewedContract',
                     'businessWorkspace.upsertPayment','businessWorkspace.upsertSettlementBatch',
                     'businessWorkspace.voidSettlementBatch','businessWorkspace.confirmQuote',
                     'businessWorkspace.recordReceipt','businessWorkspace.reverseReceipt',
                     'businessWorkspace.adoptLatestConfirmedRequirement','businessWorkspace.upsertCustomer',
                     'businessWorkspace.assignCustomer','businessWorkspace.upsertMilestone',
                     'businessWorkspace.registerDeliverableVersion','businessWorkspace.recordDeliverySent',
                     'businessWorkspace.recordDeliverySignoff','businessWorkspace.recordInvoiceIssued',
                     'businessWorkspace.recordInvoiceRedCorrection','businessWorkspace.attachInvoiceAsset',
                     'businessWorkspace.createAcceptanceBatch','businessWorkspace.prepareAcceptanceDocuments',
                     'businessWorkspace.upsertAcceptanceMaterial',
                     'businessWorkspace.createArchiveSnapshot',
                     'businessWorkspace.normalizeLegacyTemplate',
                     'businessWorkspace.approveTemplateVersion',
                     'businessWorkspace.rejectTemplateVersion',
                     'businessWorkspace.changeStatus')),
                protocol_version TEXT NOT NULL,
                deadline_at INTEGER,
                request_fingerprint TEXT NOT NULL CHECK(length(request_fingerprint) = 64),
                response_json TEXT NOT NULL,
                completed_at INTEGER NOT NULL
            );
            INSERT INTO business_workspace_command_receipts
                (idempotency_key, command_id, command_type, protocol_version, deadline_at,
                 request_fingerprint, response_json, completed_at)
            SELECT idempotency_key, command_id, command_type, protocol_version, deadline_at,
                   request_fingerprint, response_json, completed_at
            FROM business_workspace_command_receipts_before_review_promotion;
            DROP TABLE business_workspace_command_receipts_before_review_promotion;
            CREATE INDEX idx_business_workspace_receipts_completed
                ON business_workspace_command_receipts(completed_at);
            COMMIT;
            "#,
        )
        .map_err(sql_error)
}

pub fn execute_command(
    connection: &mut Connection,
    vault_root: &Path,
    command: BusinessWorkspaceCommandEnvelope,
) -> Result<BusinessWorkspaceCommandOutcome, HostError> {
    let command = normalize_command(command)?;
    let fingerprint = command_fingerprint(&command)?;
    let meta = command.meta();
    {
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
            return Ok(BusinessWorkspaceCommandOutcome {
                response,
                emitted_events: Vec::new(),
                emitted_asset_events: Vec::new(),
            });
        }
        validate_deadline(meta.deadline_at)?;
        transaction.commit().map_err(sql_error)?;
    }
    if let Err(error) = reconcile_generated_assets(connection, vault_root) {
        eprintln!("business generated asset reconciliation deferred: {error}");
    }
    if matches!(command, NormalizedCommand::NormalizeLegacyTemplate { .. }) {
        execute_normalize_legacy_template(connection, vault_root, command, fingerprint)
    } else if matches!(command, NormalizedCommand::GenerateDocument { .. }) {
        execute_generate_document(connection, vault_root, command, fingerprint)
    } else if matches!(command, NormalizedCommand::CreateArchiveSnapshot { .. }) {
        execute_create_archive_snapshot(connection, vault_root, command, fingerprint)
    } else if matches!(
        &command,
        NormalizedCommand::ChangeStatus { payload, .. }
            if payload.status == BusinessWorkspaceStatus::Archived
    ) {
        execute_archive_status_change(connection, vault_root, command, fingerprint)
    } else {
        execute_transactional_command(connection, vault_root, command, fingerprint)
    }
}

pub fn list(connection: &Connection) -> Result<Vec<BusinessWorkspaceRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id FROM business_workspaces
             ORDER BY updated_at DESC, id DESC",
        )
        .map_err(sql_error)?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    drop(statement);
    ids.iter()
        .map(|id| load_workspace(connection, id))
        .collect()
}

/// Returns the customer-level receivables ledger keyed by stable customer master-data IDs.
pub fn list_customers(
    connection: &Connection,
    request: &ListBusinessCustomersRequest,
) -> Result<Vec<BusinessCustomerReceivableSummary>, HostError> {
    let limit = request.limit.unwrap_or(DEFAULT_CUSTOMER_LIST_LIMIT);
    if !(1..=MAX_CUSTOMER_LIST_LIMIT).contains(&limit) {
        return Err(HostError::validation(format!(
            "limit must be in 1..={MAX_CUSTOMER_LIST_LIMIT}"
        )));
    }
    if request.query.chars().count() > MAX_SHORT_CHARS {
        return Err(HostError::validation(format!(
            "query must contain at most {MAX_SHORT_CHARS} characters"
        )));
    }

    let workspaces = list(connection)?;
    let customers = business_closure_service::list_customers(connection)?;
    let mut by_customer = HashMap::<String, Vec<&BusinessWorkspaceRecord>>::new();
    for workspace in &workspaces {
        by_customer
            .entry(workspace.customer_id.clone())
            .or_default()
            .push(workspace);
    }
    let normalized_query = normalized_business_identity(request.query.trim());
    let mut summaries = Vec::with_capacity(customers.len());
    for customer in customers {
        let mut members = by_customer.remove(&customer.id).unwrap_or_default();
        members.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| right.id.cmp(&left.id))
        });
        let matches_customer_query = normalized_query.is_empty()
            || [
                customer.id.as_str(),
                customer.display_name.as_str(),
                customer.legal_name.as_str(),
                customer.tax_id.as_str(),
                customer.primary_contact_name.as_str(),
                customer.primary_phone.as_str(),
                customer.primary_email.as_str(),
            ]
            .iter()
            .map(|value| normalized_business_identity(value))
            .any(|value| value.contains(&normalized_query));
        if !matches_customer_query {
            continue;
        }

        let mut workspace_ids = Vec::with_capacity(members.len());
        let mut active_workspace_count = 0_i64;
        let mut contract_cents = 0_i64;
        let mut requested_cents = 0_i64;
        let mut received_cents = 0_i64;
        let mut outstanding_cents = 0_i64;
        let mut updated_at = 0_i64;
        for workspace in members {
            workspace_ids.push(workspace.id.clone());
            if workspace.status == BusinessWorkspaceStatus::Active {
                active_workspace_count = active_workspace_count.saturating_add(1);
            }
            contract_cents =
                contract_cents.saturating_add(workspace.financial_summary.contract_cents);
            requested_cents =
                requested_cents.saturating_add(workspace.financial_summary.requested_cents);
            received_cents =
                received_cents.saturating_add(workspace.financial_summary.received_cents);
            outstanding_cents =
                outstanding_cents.saturating_add(workspace.financial_summary.outstanding_cents);
            updated_at = updated_at.max(workspace.updated_at);
        }

        summaries.push(BusinessCustomerReceivableSummary {
            customer_id: customer.id.clone(),
            customer_key: customer.id.clone(),
            customer_name: customer.display_name,
            customer_legal_name: customer.legal_name,
            customer_tax_id: customer.tax_id,
            customer_contact: customer.primary_contact_name,
            customer_phone: customer.primary_phone,
            customer_email: customer.primary_email,
            customer_status: customer.status,
            customer_revision: customer.revision,
            workspace_count: i64::try_from(workspace_ids.len()).unwrap_or(i64::MAX),
            active_workspace_count,
            contract_cents,
            requested_cents,
            received_cents,
            outstanding_cents,
            workspace_ids,
            updated_at: updated_at.max(customer.updated_at),
        });
    }
    summaries.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.customer_key.cmp(&right.customer_key))
    });
    summaries.truncate(limit as usize);
    Ok(summaries)
}

pub fn list_prefill_candidates(
    connection: &Connection,
    request: &ListBusinessWorkspacePrefillCandidatesRequest,
) -> Result<Vec<BusinessWorkspacePrefillCandidate>, HostError> {
    let limit = request.limit.unwrap_or(DEFAULT_PREFILL_CANDIDATE_LIMIT);
    if !(1..=MAX_PREFILL_CANDIDATE_LIMIT).contains(&limit) {
        return Err(HostError::validation(format!(
            "limit must be in 1..={MAX_PREFILL_CANDIDATE_LIMIT}"
        )));
    }
    let target_project_id = normalize_uuid("targetProjectId", request.target_project_id.clone())?;
    let context = load_workspace_creation_context(connection, &target_project_id)?;
    let customer_key = normalized_business_identity(&context.client_name);
    if customer_key.is_empty() {
        return Ok(Vec::new());
    }
    let sources = {
        let mut statement = connection
            .prepare(
                "SELECT workspace.id, workspace.project_id, project.name,
                        workspace.profile_json, workspace.status, workspace.revision,
                        workspace.updated_at
                 FROM business_workspaces workspace
                 JOIN projects project ON project.id = workspace.project_id
                 WHERE workspace.customer_name_key = ?1
                    OR workspace.customer_legal_name_key = ?1
                 ORDER BY workspace.updated_at DESC, workspace.id DESC
                 LIMIT ?2",
            )
            .map_err(sql_error)?;
        let rows = statement
            .query_map(
                params![customer_key, i64::from(limit)],
                prefill_source_from_row,
            )
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        rows
    };
    sources
        .into_iter()
        .map(|source| {
            let match_kind = prefill_match_kind(&source.profile, &context.client_name)?;
            Ok(BusinessWorkspacePrefillCandidate {
                source_workspace_id: source.id,
                source_project_id: source.project_id,
                source_project_title: source.project_title,
                customer_name: source.profile.customer_name.clone(),
                customer_legal_name: source.profile.customer_legal_name.clone(),
                supplier_legal_name: source.profile.supplier_legal_name.clone(),
                match_kind,
                populated_fields: populated_reusable_fields(&source.profile),
                status: source.status,
                source_revision: source.revision,
                source_updated_at: source.updated_at,
            })
        })
        .collect()
}

pub fn preview_prefill(
    connection: &Connection,
    request: &PreviewBusinessWorkspacePrefillRequest,
) -> Result<BusinessWorkspacePrefillPreview, HostError> {
    let target_project_id = normalize_uuid("targetProjectId", request.target_project_id.clone())?;
    let source_workspace_id =
        normalize_uuid("sourceWorkspaceId", request.source_workspace_id.clone())?;
    let context = load_workspace_creation_context(connection, &target_project_id)?;
    let source = load_prefill_source_workspace(connection, &source_workspace_id)?;
    let match_kind = prefill_match_kind(&source.profile, &context.client_name)?;
    let target_project_title = context.project_name.clone();
    let target_customer_name = context.client_name.clone();
    let (target_requirement_brief_id, target_profile) = prefill_profile(
        &target_project_id,
        context.project_name,
        context.client_name,
        context.confirmed,
    )?;
    Ok(BusinessWorkspacePrefillPreview {
        target_project_id,
        target_project_title,
        target_customer_name,
        target_requirement_brief_id,
        source_workspace_id: source.id,
        source_project_id: source.project_id,
        source_project_title: source.project_title,
        match_kind,
        source_revision: source.revision,
        source_updated_at: source.updated_at,
        changes: reusable_prefill_changes(&target_profile, &source.profile),
    })
}

pub fn replay_events(
    connection: &Connection,
    after_sequence: i64,
    limit: u32,
) -> Result<Vec<BusinessWorkspaceDomainEvent>, HostError> {
    if after_sequence < 0 {
        return Err(HostError::validation("afterSequence cannot be negative"));
    }
    if limit == 0 {
        return Err(HostError::validation("limit must be at least 1"));
    }
    let mut statement = connection
        .prepare(
            "SELECT sequence, event_id, event_type, aggregate_id, revision,
                    occurred_at, trace_id, actor_id, command_id, reason, payload_json
             FROM business_workspace_events
             WHERE sequence > ?1 ORDER BY sequence ASC LIMIT ?2",
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

/// Removes interrupted document staging directories and generated assets that
/// were imported but never linked by the final event/receipt transaction.
pub fn reconcile_generated_assets(
    connection: &mut Connection,
    vault_root: &Path,
) -> Result<usize, HostError> {
    document_engine::reconcile_staging(vault_root)?;
    retry_pending_generated_asset_deletes(connection, vault_root, None)?;
    let assets = {
        let mut statement = connection
            .prepare(
                "SELECT DISTINCT a.id, a.original_name, origin.origin FROM assets a
                 JOIN asset_origins origin ON origin.asset_id = a.id
                 LEFT JOIN business_documents d ON d.output_asset_id = a.id
                 LEFT JOIN business_archive_snapshots snapshot
                   ON snapshot.manifest_asset_id = a.id OR snapshot.package_asset_id = a.id
                 LEFT JOIN business_template_versions template
                   ON template.normalized_asset_id = a.id
                 WHERE (origin.origin = 'businessDocument' AND d.id IS NULL)
                    OR (origin.origin IN ('generatedArchiveManifest','generatedArchivePackage')
                        AND snapshot.id IS NULL)
                    OR (origin.origin = 'normalizedTemplate' AND template.id IS NULL)",
            )
            .map_err(sql_error)?;
        let collected = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        collected
    };
    let mut cleaned = 0;
    for (asset_id, original_name, origin) in assets {
        if matches!(origin.as_str(), "businessDocument" | "normalizedTemplate")
            && document_engine::generation_is_active(vault_root, &original_name)?
        {
            continue;
        }
        cleanup_generated_asset(connection, vault_root, &asset_id)?;
        cleaned += 1;
    }
    Ok(cleaned)
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
        payload: CreateBusinessWorkspacePayload,
    },
    UpdateProfile {
        meta: CommandMeta,
        payload: Box<UpdateBusinessProfilePayload>,
    },
    CreateDocument {
        meta: CommandMeta,
        payload: CreateBusinessDocumentPayload,
    },
    CreateAcceptanceBatch {
        meta: CommandMeta,
        payload: CreateBusinessAcceptanceBatchPayload,
    },
    PrepareAcceptanceDocuments {
        meta: CommandMeta,
        payload: PrepareBusinessAcceptanceDocumentsPayload,
    },
    UpsertAcceptanceMaterial {
        meta: CommandMeta,
        payload: UpsertBusinessAcceptanceMaterialPayload,
    },
    PromoteReviewedContract {
        meta: CommandMeta,
        payload: PromoteReviewedContractPayload,
    },
    ChangeDocumentStatus {
        meta: CommandMeta,
        payload: ChangeBusinessDocumentStatusPayload,
    },
    GenerateDocument {
        meta: CommandMeta,
        payload: GenerateBusinessDocumentPayload,
    },
    UpsertPayment {
        meta: CommandMeta,
        payload: UpsertBusinessPaymentPayload,
    },
    UpsertSettlementBatch {
        meta: CommandMeta,
        payload: UpsertBusinessSettlementBatchPayload,
    },
    VoidSettlementBatch {
        meta: CommandMeta,
        payload: VoidBusinessSettlementBatchPayload,
    },
    ConfirmQuote {
        meta: CommandMeta,
        payload: ConfirmBusinessQuotePayload,
    },
    RecordReceipt {
        meta: CommandMeta,
        payload: RecordBusinessReceiptPayload,
    },
    ReverseReceipt {
        meta: CommandMeta,
        payload: ReverseBusinessReceiptPayload,
    },
    AdoptLatestConfirmedRequirement {
        meta: CommandMeta,
        payload: AdoptLatestConfirmedRequirementPayload,
    },
    UpsertCustomer {
        meta: CommandMeta,
        payload: UpsertBusinessCustomerPayload,
    },
    AssignCustomer {
        meta: CommandMeta,
        payload: AssignBusinessCustomerPayload,
    },
    UpsertMilestone {
        meta: CommandMeta,
        payload: UpsertBusinessMilestonePayload,
    },
    RegisterDeliverableVersion {
        meta: CommandMeta,
        payload: RegisterBusinessDeliverableVersionPayload,
    },
    RecordDeliverySent {
        meta: CommandMeta,
        payload: RecordBusinessDeliverySentPayload,
    },
    RecordDeliverySignoff {
        meta: CommandMeta,
        payload: RecordBusinessDeliverySignoffPayload,
    },
    RecordInvoiceIssued {
        meta: CommandMeta,
        payload: RecordBusinessInvoiceIssuedPayload,
    },
    RecordInvoiceRedCorrection {
        meta: CommandMeta,
        payload: RecordBusinessInvoiceRedCorrectionPayload,
    },
    AttachInvoiceAsset {
        meta: CommandMeta,
        payload: AttachBusinessInvoiceAssetPayload,
    },
    CreateArchiveSnapshot {
        meta: CommandMeta,
        payload: CreateBusinessArchiveSnapshotPayload,
    },
    NormalizeLegacyTemplate {
        meta: CommandMeta,
        payload: NormalizeBusinessLegacyTemplatePayload,
    },
    ApproveTemplateVersion {
        meta: CommandMeta,
        payload: ApproveBusinessTemplateVersionPayload,
    },
    RejectTemplateVersion {
        meta: CommandMeta,
        payload: RejectBusinessTemplateVersionPayload,
    },
    ChangeStatus {
        meta: CommandMeta,
        payload: ChangeBusinessWorkspaceStatusPayload,
    },
}

impl NormalizedCommand {
    fn meta(&self) -> &CommandMeta {
        match self {
            Self::Create { meta, .. }
            | Self::UpdateProfile { meta, .. }
            | Self::CreateDocument { meta, .. }
            | Self::CreateAcceptanceBatch { meta, .. }
            | Self::PrepareAcceptanceDocuments { meta, .. }
            | Self::UpsertAcceptanceMaterial { meta, .. }
            | Self::PromoteReviewedContract { meta, .. }
            | Self::ChangeDocumentStatus { meta, .. }
            | Self::GenerateDocument { meta, .. }
            | Self::UpsertPayment { meta, .. }
            | Self::UpsertSettlementBatch { meta, .. }
            | Self::VoidSettlementBatch { meta, .. }
            | Self::ConfirmQuote { meta, .. }
            | Self::RecordReceipt { meta, .. }
            | Self::ReverseReceipt { meta, .. }
            | Self::AdoptLatestConfirmedRequirement { meta, .. }
            | Self::UpsertCustomer { meta, .. }
            | Self::AssignCustomer { meta, .. }
            | Self::UpsertMilestone { meta, .. }
            | Self::RegisterDeliverableVersion { meta, .. }
            | Self::RecordDeliverySent { meta, .. }
            | Self::RecordDeliverySignoff { meta, .. }
            | Self::RecordInvoiceIssued { meta, .. }
            | Self::RecordInvoiceRedCorrection { meta, .. }
            | Self::AttachInvoiceAsset { meta, .. }
            | Self::CreateArchiveSnapshot { meta, .. }
            | Self::NormalizeLegacyTemplate { meta, .. }
            | Self::ApproveTemplateVersion { meta, .. }
            | Self::RejectTemplateVersion { meta, .. }
            | Self::ChangeStatus { meta, .. } => meta,
        }
    }

    fn command_type(&self) -> &'static str {
        match self {
            Self::Create { .. } => "businessWorkspace.create",
            Self::UpdateProfile { .. } => "businessWorkspace.updateProfile",
            Self::CreateDocument { .. } => "businessWorkspace.createDocument",
            Self::CreateAcceptanceBatch { .. } => "businessWorkspace.createAcceptanceBatch",
            Self::PrepareAcceptanceDocuments { .. } => {
                "businessWorkspace.prepareAcceptanceDocuments"
            }
            Self::UpsertAcceptanceMaterial { .. } => "businessWorkspace.upsertAcceptanceMaterial",
            Self::PromoteReviewedContract { .. } => "businessWorkspace.promoteReviewedContract",
            Self::ChangeDocumentStatus { .. } => "businessWorkspace.changeDocumentStatus",
            Self::GenerateDocument { .. } => "businessWorkspace.generateDocument",
            Self::UpsertPayment { .. } => "businessWorkspace.upsertPayment",
            Self::UpsertSettlementBatch { .. } => "businessWorkspace.upsertSettlementBatch",
            Self::VoidSettlementBatch { .. } => "businessWorkspace.voidSettlementBatch",
            Self::ConfirmQuote { .. } => "businessWorkspace.confirmQuote",
            Self::RecordReceipt { .. } => "businessWorkspace.recordReceipt",
            Self::ReverseReceipt { .. } => "businessWorkspace.reverseReceipt",
            Self::AdoptLatestConfirmedRequirement { .. } => {
                "businessWorkspace.adoptLatestConfirmedRequirement"
            }
            Self::UpsertCustomer { .. } => "businessWorkspace.upsertCustomer",
            Self::AssignCustomer { .. } => "businessWorkspace.assignCustomer",
            Self::UpsertMilestone { .. } => "businessWorkspace.upsertMilestone",
            Self::RegisterDeliverableVersion { .. } => {
                "businessWorkspace.registerDeliverableVersion"
            }
            Self::RecordDeliverySent { .. } => "businessWorkspace.recordDeliverySent",
            Self::RecordDeliverySignoff { .. } => "businessWorkspace.recordDeliverySignoff",
            Self::RecordInvoiceIssued { .. } => "businessWorkspace.recordInvoiceIssued",
            Self::RecordInvoiceRedCorrection { .. } => {
                "businessWorkspace.recordInvoiceRedCorrection"
            }
            Self::AttachInvoiceAsset { .. } => "businessWorkspace.attachInvoiceAsset",
            Self::CreateArchiveSnapshot { .. } => "businessWorkspace.createArchiveSnapshot",
            Self::NormalizeLegacyTemplate { .. } => "businessWorkspace.normalizeLegacyTemplate",
            Self::ApproveTemplateVersion { .. } => "businessWorkspace.approveTemplateVersion",
            Self::RejectTemplateVersion { .. } => "businessWorkspace.rejectTemplateVersion",
            Self::ChangeStatus { .. } => "businessWorkspace.changeStatus",
        }
    }
}

fn normalize_command(
    command: BusinessWorkspaceCommandEnvelope,
) -> Result<NormalizedCommand, HostError> {
    match command {
        BusinessWorkspaceCommandEnvelope::Create {
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
                    "businessWorkspace.create rejects expectedRevision",
                ));
            }
            let context = normalize_context(context)?;
            let project_id = normalize_uuid("projectId", payload.project_id)?;
            let customer_id = payload
                .customer_id
                .map(|value| normalize_uuid("customerId", value))
                .transpose()?;
            let prefill_source_workspace_id = payload
                .prefill_source_workspace_id
                .map(|value| normalize_uuid("prefillSourceWorkspaceId", value))
                .transpose()?;
            if context.project_id != project_id {
                return Err(HostError::validation(
                    "context projectId must match business workspace projectId",
                ));
            }
            let meta = normalize_meta(
                command_id,
                protocol_version,
                context,
                idempotency_key,
                None,
                deadline_at,
            )?;
            if meta.protocol_version == BUSINESS_WORKSPACE_LEGACY_PROTOCOL_VERSION
                && prefill_source_workspace_id.is_some()
            {
                return Err(HostError::new(
                    "BUSINESS_PREFILL_PROTOCOL_UNSUPPORTED",
                    format!(
                        "prefillSourceWorkspaceId requires business workspace protocolVersion {BUSINESS_WORKSPACE_PROTOCOL_VERSION}"
                    ),
                    false,
                ));
            }
            Ok(NormalizedCommand::Create {
                meta,
                payload: CreateBusinessWorkspacePayload {
                    project_id,
                    customer_id,
                    prefill_source_workspace_id,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::UpdateProfile {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let UpdateBusinessProfilePayload {
                workspace_id,
                profile,
            } = *payload;
            Ok(NormalizedCommand::UpdateProfile {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    normalize_context(context)?,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload: Box::new(UpdateBusinessProfilePayload {
                    workspace_id: normalize_uuid("workspaceId", workspace_id)?,
                    profile,
                }),
            })
        }
        BusinessWorkspaceCommandEnvelope::CreateDocument {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let template_key =
                normalize_required("templateKey", payload.template_key, MAX_SHORT_CHARS)?;
            document_engine::validate_template(&payload.kind, &template_key)?;
            let payment_id = payload
                .payment_id
                .map(|id| normalize_uuid("paymentId", id))
                .transpose()?;
            match (&payload.kind, &payment_id) {
                (BusinessDocumentKind::PaymentRequest, None) => {
                    return Err(HostError::new(
                        "BUSINESS_PAYMENT_REQUIRED",
                        "paymentRequest document requires paymentId",
                        false,
                    ));
                }
                (BusinessDocumentKind::PaymentRequest, Some(_)) | (_, None) => {}
                (_, Some(_)) => {
                    return Err(HostError::new(
                        "BUSINESS_PAYMENT_NOT_ALLOWED",
                        "paymentId is only valid for paymentRequest documents",
                        false,
                    ));
                }
            }
            Ok(NormalizedCommand::CreateDocument {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    normalize_context(context)?,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload: CreateBusinessDocumentPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    kind: payload.kind,
                    document_number: normalize_required(
                        "documentNumber",
                        payload.document_number,
                        MAX_SHORT_CHARS,
                    )?,
                    title: normalize_required("title", payload.title, MAX_SHORT_CHARS)?,
                    template_key,
                    payment_id,
                    acceptance_batch_id: payload
                        .acceptance_batch_id
                        .map(|id| normalize_uuid("acceptanceBatchId", id))
                        .transpose()?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            Ok(NormalizedCommand::CreateAcceptanceBatch {
                meta,
                payload: normalize_create_acceptance_batch_payload(payload)?,
            })
        }
        BusinessWorkspaceCommandEnvelope::PrepareAcceptanceDocuments {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            Ok(NormalizedCommand::PrepareAcceptanceDocuments {
                meta,
                payload: PrepareBusinessAcceptanceDocumentsPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    batch_id: normalize_uuid("acceptanceBatchId", payload.batch_id)?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::UpsertAcceptanceMaterial {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            Ok(NormalizedCommand::UpsertAcceptanceMaterial {
                meta,
                payload: normalize_upsert_acceptance_material_payload(payload)?,
            })
        }
        BusinessWorkspaceCommandEnvelope::PromoteReviewedContract {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            if meta.protocol_version == BUSINESS_WORKSPACE_LEGACY_PROTOCOL_VERSION {
                return Err(HostError::new(
                    "BUSINESS_REVIEW_PROMOTION_PROTOCOL_UNSUPPORTED",
                    format!(
                        "reviewed contract promotion requires business workspace protocolVersion {BUSINESS_WORKSPACE_PROTOCOL_VERSION}"
                    ),
                    false,
                ));
            }
            Ok(NormalizedCommand::PromoteReviewedContract {
                meta,
                payload: PromoteReviewedContractPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    review_id: normalize_uuid("reviewId", payload.review_id)?,
                    report_asset_id: normalize_uuid("reportAssetId", payload.report_asset_id)?,
                    document_number: normalize_required(
                        "documentNumber",
                        payload.document_number,
                        MAX_SHORT_CHARS,
                    )?,
                    title: normalize_required("title", payload.title, MAX_SHORT_CHARS)?,
                    evidence: payload.evidence.map(normalize_evidence_input).transpose()?,
                    manual_waiver: payload
                        .manual_waiver
                        .map(normalize_manual_waiver_input)
                        .transpose()?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::ChangeDocumentStatus {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            Ok(NormalizedCommand::ChangeDocumentStatus {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    normalize_context(context)?,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload: ChangeBusinessDocumentStatusPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    document_id: normalize_uuid("documentId", payload.document_id)?,
                    status: payload.status,
                    evidence: payload.evidence.map(normalize_evidence_input).transpose()?,
                    manual_waiver: payload
                        .manual_waiver
                        .map(normalize_manual_waiver_input)
                        .transpose()?,
                    reason: normalize_text("reason", payload.reason, MAX_TEXT_CHARS)?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::GenerateDocument {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            Ok(NormalizedCommand::GenerateDocument {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    normalize_context(context)?,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload: GenerateBusinessDocumentPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    document_id: normalize_uuid("documentId", payload.document_id)?,
                    format: payload.format,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::UpsertPayment {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            Ok(NormalizedCommand::UpsertPayment {
                meta: normalize_meta(
                    command_id,
                    protocol_version,
                    normalize_context(context)?,
                    idempotency_key,
                    expected_revision,
                    deadline_at,
                )?,
                payload: UpsertBusinessPaymentPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    payment: normalize_payment_input(payload.payment)?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::UpsertSettlementBatch {
            command_id,
            protocol_version,
            context,
            mut payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            payload.workspace_id = normalize_uuid("workspaceId", payload.workspace_id)?;
            payload.batch.id = payload
                .batch
                .id
                .map(|value| normalize_uuid("batch.id", value))
                .transpose()?;
            for line in &mut payload.batch.lines {
                line.deliverable_id = normalize_uuid("deliverableId", line.deliverable_id.clone())?;
            }
            Ok(NormalizedCommand::UpsertSettlementBatch { meta, payload })
        }
        BusinessWorkspaceCommandEnvelope::VoidSettlementBatch {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            Ok(NormalizedCommand::VoidSettlementBatch {
                meta,
                payload: VoidBusinessSettlementBatchPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    batch_id: normalize_uuid("batchId", payload.batch_id)?,
                    reason: payload.reason,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::ConfirmQuote {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            Ok(NormalizedCommand::ConfirmQuote {
                meta,
                payload: ConfirmBusinessQuotePayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    quote_document_id: normalize_uuid(
                        "quoteDocumentId",
                        payload.quote_document_id,
                    )?,
                    confirmation_version: normalize_required(
                        "confirmationVersion",
                        payload.confirmation_version,
                        MAX_SHORT_CHARS,
                    )?,
                    customer_representative: normalize_required(
                        "customerRepresentative",
                        payload.customer_representative,
                        MAX_SHORT_CHARS,
                    )?,
                    evidence: normalize_evidence_input(payload.evidence)?,
                    notes: normalize_text("notes", payload.notes, MAX_TEXT_CHARS)?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::RecordReceipt {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            validate_money("amountCents", payload.amount_cents)?;
            validate_timestamp("occurredAt", Some(payload.occurred_at))?;
            Ok(NormalizedCommand::RecordReceipt {
                meta,
                payload: RecordBusinessReceiptPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    payment_id: normalize_uuid("paymentId", payload.payment_id)?,
                    amount_cents: payload.amount_cents,
                    occurred_at: payload.occurred_at,
                    reference: normalize_required("reference", payload.reference, MAX_SHORT_CHARS)?,
                    notes: normalize_text("notes", payload.notes, MAX_TEXT_CHARS)?,
                    evidence: payload.evidence.map(normalize_evidence_input).transpose()?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::ReverseReceipt {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            validate_money("amountCents", payload.amount_cents)?;
            validate_timestamp("occurredAt", Some(payload.occurred_at))?;
            Ok(NormalizedCommand::ReverseReceipt {
                meta,
                payload: ReverseBusinessReceiptPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    receipt_id: normalize_uuid("receiptId", payload.receipt_id)?,
                    amount_cents: payload.amount_cents,
                    occurred_at: payload.occurred_at,
                    reference: normalize_required("reference", payload.reference, MAX_SHORT_CHARS)?,
                    reason: normalize_required("reason", payload.reason, MAX_TEXT_CHARS)?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::AdoptLatestConfirmedRequirement {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            Ok(NormalizedCommand::AdoptLatestConfirmedRequirement {
                meta,
                payload: AdoptLatestConfirmedRequirementPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::UpsertCustomer {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            Ok(NormalizedCommand::UpsertCustomer {
                meta,
                payload: UpsertBusinessCustomerPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    customer_id: payload
                        .customer_id
                        .map(|value| normalize_uuid("customerId", value))
                        .transpose()?,
                    customer: payload.customer,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::AssignCustomer {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            Ok(NormalizedCommand::AssignCustomer {
                meta,
                payload: AssignBusinessCustomerPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    customer_id: normalize_uuid("customerId", payload.customer_id)?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::UpsertMilestone {
            command_id,
            protocol_version,
            context,
            mut payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            payload.workspace_id = normalize_uuid("workspaceId", payload.workspace_id)?;
            payload.milestone.id = payload
                .milestone
                .id
                .map(|value| normalize_uuid("milestone.id", value))
                .transpose()?;
            Ok(NormalizedCommand::UpsertMilestone { meta, payload })
        }
        BusinessWorkspaceCommandEnvelope::RegisterDeliverableVersion {
            command_id,
            protocol_version,
            context,
            mut payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            payload.workspace_id = normalize_uuid("workspaceId", payload.workspace_id)?;
            payload.milestone_id = normalize_uuid("milestoneId", payload.milestone_id)?;
            payload.deliverable_id = payload
                .deliverable_id
                .map(|value| normalize_uuid("deliverableId", value))
                .transpose()?;
            payload.asset_id = normalize_uuid("assetId", payload.asset_id)?;
            Ok(NormalizedCommand::RegisterDeliverableVersion { meta, payload })
        }
        BusinessWorkspaceCommandEnvelope::RecordDeliverySent {
            command_id,
            protocol_version,
            context,
            mut payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            payload.workspace_id = normalize_uuid("workspaceId", payload.workspace_id)?;
            payload.milestone_id = normalize_uuid("milestoneId", payload.milestone_id)?;
            payload.version_ids = payload
                .version_ids
                .into_iter()
                .map(|value| normalize_uuid("versionId", value))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(NormalizedCommand::RecordDeliverySent { meta, payload })
        }
        BusinessWorkspaceCommandEnvelope::RecordDeliverySignoff {
            command_id,
            protocol_version,
            context,
            mut payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            payload.workspace_id = normalize_uuid("workspaceId", payload.workspace_id)?;
            payload.submission_id = normalize_uuid("submissionId", payload.submission_id)?;
            payload.accepted_version_ids = payload
                .accepted_version_ids
                .into_iter()
                .map(|value| normalize_uuid("acceptedVersionId", value))
                .collect::<Result<Vec<_>, _>>()?;
            payload.rejected_version_ids = payload
                .rejected_version_ids
                .into_iter()
                .map(|value| normalize_uuid("rejectedVersionId", value))
                .collect::<Result<Vec<_>, _>>()?;
            payload.evidence = payload.evidence.map(normalize_evidence_input).transpose()?;
            Ok(NormalizedCommand::RecordDeliverySignoff { meta, payload })
        }
        BusinessWorkspaceCommandEnvelope::RecordInvoiceIssued {
            command_id,
            protocol_version,
            context,
            mut payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            payload.workspace_id = normalize_uuid("workspaceId", payload.workspace_id)?;
            payload.payment_id = payload
                .payment_id
                .map(|value| normalize_uuid("paymentId", value))
                .transpose()?;
            payload.asset_ids = payload
                .asset_ids
                .into_iter()
                .map(|value| normalize_uuid("assetId", value))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(NormalizedCommand::RecordInvoiceIssued { meta, payload })
        }
        BusinessWorkspaceCommandEnvelope::RecordInvoiceRedCorrection {
            command_id,
            protocol_version,
            context,
            mut payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            payload.workspace_id = normalize_uuid("workspaceId", payload.workspace_id)?;
            payload.original_invoice_id =
                normalize_uuid("originalInvoiceId", payload.original_invoice_id)?;
            payload.asset_ids = payload
                .asset_ids
                .into_iter()
                .map(|value| normalize_uuid("assetId", value))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(NormalizedCommand::RecordInvoiceRedCorrection { meta, payload })
        }
        BusinessWorkspaceCommandEnvelope::AttachInvoiceAsset {
            command_id,
            protocol_version,
            context,
            mut payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            payload.workspace_id = normalize_uuid("workspaceId", payload.workspace_id)?;
            payload.invoice_id = normalize_uuid("invoiceId", payload.invoice_id)?;
            payload.asset_id = normalize_uuid("assetId", payload.asset_id)?;
            Ok(NormalizedCommand::AttachInvoiceAsset { meta, payload })
        }
        BusinessWorkspaceCommandEnvelope::CreateArchiveSnapshot {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            Ok(NormalizedCommand::CreateArchiveSnapshot {
                meta,
                payload: CreateBusinessArchiveSnapshotPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::NormalizeLegacyTemplate {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            let template_key =
                normalize_required("templateKey", payload.template_key, MAX_SHORT_CHARS)?;
            if template_key
                != document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY
            {
                return Err(HostError::new(
                    "BUSINESS_LEGACY_TEMPLATE_UNSUPPORTED",
                    "legacy DOC normalization is only registered for the payment application and settlement calculation template",
                    false,
                ));
            }
            let mapping_version =
                normalize_required("mappingVersion", payload.mapping_version, MAX_SHORT_CHARS)?;
            if document_engine::expected_template_mapping_version(&template_key)
                != Some(mapping_version.as_str())
            {
                return Err(HostError::new(
                    "BUSINESS_TEMPLATE_MAPPING_VERSION_MISMATCH",
                    "mappingVersion does not match the registered template renderer",
                    false,
                ));
            }
            Ok(NormalizedCommand::NormalizeLegacyTemplate {
                meta,
                payload: NormalizeBusinessLegacyTemplatePayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    source_asset_id: normalize_uuid("sourceAssetId", payload.source_asset_id)?,
                    expected_source_sha256: normalize_sha256(
                        "expectedSourceSha256",
                        payload.expected_source_sha256,
                    )?,
                    template_key,
                    mapping_version,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::ApproveTemplateVersion {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            Ok(NormalizedCommand::ApproveTemplateVersion {
                meta,
                payload: ApproveBusinessTemplateVersionPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    template_version_id: normalize_uuid(
                        "templateVersionId",
                        payload.template_version_id,
                    )?,
                    note: normalize_required("note", payload.note, MAX_TEXT_CHARS)?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::RejectTemplateVersion {
            command_id,
            protocol_version,
            context,
            payload,
            idempotency_key,
            expected_revision,
            deadline_at,
        } => {
            validate_expected_revision(expected_revision)?;
            let meta = normalize_meta(
                command_id,
                protocol_version,
                normalize_context(context)?,
                idempotency_key,
                expected_revision,
                deadline_at,
            )?;
            ensure_current_business_protocol(&meta)?;
            Ok(NormalizedCommand::RejectTemplateVersion {
                meta,
                payload: RejectBusinessTemplateVersionPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    template_version_id: normalize_uuid(
                        "templateVersionId",
                        payload.template_version_id,
                    )?,
                    note: normalize_required("note", payload.note, MAX_TEXT_CHARS)?,
                },
            })
        }
        BusinessWorkspaceCommandEnvelope::ChangeStatus {
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
                payload: ChangeBusinessWorkspaceStatusPayload {
                    workspace_id: normalize_uuid("workspaceId", payload.workspace_id)?,
                    status: payload.status,
                },
            })
        }
    }
}

fn ensure_current_business_protocol(meta: &CommandMeta) -> Result<(), HostError> {
    if meta.protocol_version == BUSINESS_WORKSPACE_PROTOCOL_VERSION
        || meta.protocol_version == BUSINESS_WORKSPACE_PREVIOUS_PROTOCOL_VERSION
    {
        Ok(())
    } else {
        Err(HostError::new(
            "BUSINESS_LIFECYCLE_PROTOCOL_UNSUPPORTED",
            format!(
                "this business lifecycle command requires protocolVersion {BUSINESS_WORKSPACE_PROTOCOL_VERSION}"
            ),
            false,
        ))
    }
}

fn normalize_create_acceptance_batch_payload(
    mut payload: CreateBusinessAcceptanceBatchPayload,
) -> Result<CreateBusinessAcceptanceBatchPayload, HostError> {
    payload.workspace_id = normalize_uuid("workspaceId", payload.workspace_id)?;
    payload.label = normalize_required("acceptance batch label", payload.label, MAX_SHORT_CHARS)?;
    if payload.requirements.is_empty()
        || payload.requirements.len() > MAX_ACCEPTANCE_REQUIREMENTS_PER_BATCH
    {
        return Err(HostError::new(
            "BUSINESS_ACCEPTANCE_REQUIREMENTS_INVALID",
            format!(
                "acceptance batch requires 1..={MAX_ACCEPTANCE_REQUIREMENTS_PER_BATCH} configurable content slots"
            ),
            false,
        ));
    }
    if payload.output_specs.is_empty()
        || payload.output_specs.len() > MAX_ACCEPTANCE_OUTPUT_SPECS_PER_BATCH
    {
        return Err(HostError::new(
            "BUSINESS_ACCEPTANCE_OUTPUT_SPECS_INVALID",
            format!(
                "acceptance batch requires 1..={MAX_ACCEPTANCE_OUTPUT_SPECS_PER_BATCH} configurable output specs"
            ),
            false,
        ));
    }
    let mut requirement_ids = HashSet::new();
    for requirement in &mut payload.requirements {
        requirement.id = requirement
            .id
            .take()
            .map(|id| normalize_uuid("acceptance requirement id", id))
            .transpose()?;
        requirement.label = normalize_required(
            "acceptance requirement label",
            requirement.label.clone(),
            MAX_SHORT_CHARS,
        )?;
        if requirement.required_group_count == 0 || requirement.required_group_count > 10_000 {
            return Err(HostError::new(
                "BUSINESS_ACCEPTANCE_REQUIRED_GROUP_COUNT_INVALID",
                "acceptance requiredGroupCount must be in 1..=10000",
                false,
            ));
        }
        if requirement
            .id
            .as_ref()
            .is_some_and(|id| !requirement_ids.insert(id.clone()))
        {
            return Err(HostError::new(
                "BUSINESS_ACCEPTANCE_REQUIREMENT_DUPLICATE",
                "acceptance requirement ids must be unique",
                false,
            ));
        }
    }
    let mut output_ids = HashSet::new();
    let mut output_codes = HashSet::new();
    let mut document_numbers = HashSet::new();
    for output in &mut payload.output_specs {
        output.id = output
            .id
            .take()
            .map(|id| normalize_uuid("acceptance output spec id", id))
            .transpose()?;
        output.output_code = normalize_required(
            "acceptance output outputCode",
            output.output_code.clone(),
            MAX_SHORT_CHARS,
        )?;
        output.document_number = normalize_required(
            "acceptance output documentNumber",
            output.document_number.clone(),
            MAX_SHORT_CHARS,
        )?;
        output.title = normalize_required(
            "acceptance output title",
            output.title.clone(),
            MAX_SHORT_CHARS,
        )?;
        output.template_key = normalize_required(
            "acceptance output templateKey",
            output.template_key.clone(),
            MAX_SHORT_CHARS,
        )?;
        document_engine::validate_template_for_format(
            &BusinessDocumentKind::Acceptance,
            &output.template_key,
            &output.format,
        )?;
        output.template_asset_id = output
            .template_asset_id
            .take()
            .map(|id| normalize_uuid("acceptance output templateAssetId", id))
            .transpose()?;
        output.template_source_sha256 = normalize_optional(
            "acceptance output templateSourceSha256",
            output.template_source_sha256.take(),
            64,
        )?
        .map(|sha256| normalize_sha256("acceptance output templateSourceSha256", sha256))
        .transpose()?;
        output.template_mapping_version = normalize_text(
            "acceptance output templateMappingVersion",
            output.template_mapping_version.clone(),
            MAX_SHORT_CHARS,
        )?;
        let source_binding_count = usize::from(output.template_asset_id.is_some())
            + usize::from(output.template_source_sha256.is_some())
            + usize::from(!output.template_mapping_version.is_empty());
        if document_engine::template_requires_source_asset(&output.template_key) {
            if source_binding_count != 3 {
                return Err(HostError::new(
                    "BUSINESS_TEMPLATE_SOURCE_REQUIRED",
                    "registered customer templates require templateAssetId, templateSourceSha256 and templateMappingVersion",
                    false,
                ));
            }
            if let Some(expected_sha256) =
                document_engine::expected_template_source_sha256(&output.template_key)
            {
                if output.template_source_sha256.as_deref() != Some(expected_sha256) {
                    return Err(HostError::new(
                        "BUSINESS_TEMPLATE_SOURCE_HASH_MISMATCH",
                        "templateSourceSha256 does not match the registered template version",
                        false,
                    ));
                }
            }
            if output.template_mapping_version.as_str()
                != document_engine::expected_template_mapping_version(&output.template_key)
                    .expect("source-backed template must register a mapping version")
            {
                return Err(HostError::new(
                    "BUSINESS_TEMPLATE_MAPPING_VERSION_MISMATCH",
                    "templateMappingVersion does not match the registered template renderer",
                    false,
                ));
            }
        } else if source_binding_count != 0 {
            return Err(HostError::new(
                "BUSINESS_TEMPLATE_SOURCE_UNEXPECTED",
                "builtin templates cannot bind an external template source",
                false,
            ));
        }
        output.contract_settlement = output
            .contract_settlement
            .take()
            .map(normalize_contract_settlement_data)
            .transpose()?;
        output.service_settlement_items = output
            .service_settlement_items
            .drain(..)
            .enumerate()
            .map(|(index, item)| normalize_service_settlement_item(index, item))
            .collect::<Result<Vec<_>, _>>()?;
        output.payment_application = output
            .payment_application
            .take()
            .map(normalize_payment_application_input)
            .transpose()?;
        output.video_completion_acceptance = output
            .video_completion_acceptance
            .take()
            .map(normalize_video_completion_acceptance_data)
            .transpose()?;
        output.production_result_confirmation = output
            .production_result_confirmation
            .take()
            .map(normalize_production_result_confirmation_data)
            .transpose()?;
        match output.template_key.as_str() {
            document_engine::BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY => {
                if output.output_code != "video-completion-acceptance" {
                    return Err(HostError::new(
                        "BUSINESS_ACCEPTANCE_OUTPUT_CODE_MISMATCH",
                        "video completion acceptance template requires outputCode video-completion-acceptance",
                        false,
                    ));
                }
                if output.video_completion_acceptance.is_none()
                    || output.production_result_confirmation.is_some()
                    || output.contract_settlement.is_some()
                    || !output.service_settlement_items.is_empty()
                    || output.payment_application.is_some()
                {
                    return Err(HostError::new(
                        "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_DATA_REQUIRED",
                        "video completion acceptance template requires videoCompletionAcceptance and forbids settlement payloads",
                        false,
                    ));
                }
            }
            document_engine::BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY => {
                if output.output_code != "production-result-confirmation" {
                    return Err(HostError::new(
                        "BUSINESS_ACCEPTANCE_OUTPUT_CODE_MISMATCH",
                        "production result confirmation template requires outputCode production-result-confirmation",
                        false,
                    ));
                }
                if output.production_result_confirmation.is_none()
                    || output.video_completion_acceptance.is_some()
                    || output.contract_settlement.is_some()
                    || !output.service_settlement_items.is_empty()
                    || output.payment_application.is_some()
                {
                    return Err(HostError::new(
                        "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_DATA_REQUIRED",
                        "production result confirmation template requires productionResultConfirmation and forbids other specialized payloads",
                        false,
                    ));
                }
            }
            document_engine::BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY => {
                if output.output_code != "contract-settlement" {
                    return Err(HostError::new(
                        "BUSINESS_ACCEPTANCE_OUTPUT_CODE_MISMATCH",
                        "contract settlement template requires outputCode contract-settlement",
                        false,
                    ));
                }
                if output.contract_settlement.is_none()
                    || !output.service_settlement_items.is_empty()
                    || output.payment_application.is_some()
                {
                    return Err(HostError::new(
                        "BUSINESS_CONTRACT_SETTLEMENT_DATA_REQUIRED",
                        "contract settlement template requires contractSettlement and forbids serviceSettlementItems",
                        false,
                    ));
                }
            }
            document_engine::BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY => {
                if output.output_code != "service-settlement-list" {
                    return Err(HostError::new(
                        "BUSINESS_ACCEPTANCE_OUTPUT_CODE_MISMATCH",
                        "service settlement template requires outputCode service-settlement-list",
                        false,
                    ));
                }
                if output.contract_settlement.is_some()
                    || output.service_settlement_items.is_empty()
                    || output.payment_application.is_some()
                {
                    return Err(HostError::new(
                        "BUSINESS_SERVICE_SETTLEMENT_DATA_REQUIRED",
                        "service settlement template requires serviceSettlementItems and forbids contractSettlement",
                        false,
                    ));
                }
            }
            document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY => {
                if output.output_code != "payment-application-settlement-calculation" {
                    return Err(HostError::new(
                        "BUSINESS_ACCEPTANCE_OUTPUT_CODE_MISMATCH",
                        "payment application template requires outputCode payment-application-settlement-calculation",
                        false,
                    ));
                }
                if output.contract_settlement.is_some()
                    || !output.service_settlement_items.is_empty()
                    || output.payment_application.is_none()
                {
                    return Err(HostError::new(
                        "BUSINESS_PAYMENT_APPLICATION_DATA_REQUIRED",
                        "payment application template requires paymentApplication and forbids other settlement payloads",
                        false,
                    ));
                }
            }
            _ if output.contract_settlement.is_some()
                || !output.service_settlement_items.is_empty()
                || output.payment_application.is_some()
                || output.video_completion_acceptance.is_some()
                || output.production_result_confirmation.is_some() =>
            {
                return Err(HostError::new(
                    "BUSINESS_TEMPLATE_DATA_UNEXPECTED",
                    "specialized settlement data is only valid for its registered template",
                    false,
                ));
            }
            _ => {}
        }
        if output.requirement_ids.is_empty() {
            return Err(HostError::new(
                "BUSINESS_ACCEPTANCE_OUTPUT_REQUIREMENTS_INVALID",
                "acceptance output requirementIds must not be empty",
                false,
            ));
        }
        let mut output_requirement_ids = HashSet::new();
        for requirement_id in &mut output.requirement_ids {
            *requirement_id =
                normalize_uuid("acceptance output requirementId", requirement_id.clone())?;
            if !requirement_ids.contains(requirement_id) {
                return Err(HostError::new(
                    "BUSINESS_ACCEPTANCE_OUTPUT_REQUIREMENT_NOT_FOUND",
                    "acceptance output requirementIds must reference requirements in the same batch",
                    false,
                ));
            }
            if !output_requirement_ids.insert(requirement_id.clone()) {
                return Err(HostError::new(
                    "BUSINESS_ACCEPTANCE_OUTPUT_REQUIREMENT_DUPLICATE",
                    "acceptance output requirementIds must be unique",
                    false,
                ));
            }
        }
        if output
            .id
            .as_ref()
            .is_some_and(|id| !output_ids.insert(id.clone()))
            || !output_codes.insert(output.output_code.clone())
            || !document_numbers.insert(output.document_number.clone())
        {
            return Err(HostError::new(
                "BUSINESS_ACCEPTANCE_OUTPUT_SPEC_DUPLICATE",
                "acceptance output ids, output codes and document numbers must be unique",
                false,
            ));
        }
    }
    Ok(payload)
}

fn normalize_video_completion_acceptance_data(
    mut data: BusinessVideoCompletionAcceptanceData,
) -> Result<BusinessVideoCompletionAcceptanceData, HostError> {
    data.contract_title = normalize_required(
        "videoCompletionAcceptance.contractTitle",
        data.contract_title,
        MAX_SHORT_CHARS,
    )?;
    data.project_title = normalize_required(
        "videoCompletionAcceptance.projectTitle",
        data.project_title,
        MAX_SHORT_CHARS,
    )?;
    data.completion_date = normalize_required(
        "videoCompletionAcceptance.completionDate",
        data.completion_date,
        MAX_SHORT_CHARS,
    )?;
    data.acceptance_conclusion = normalize_required(
        "videoCompletionAcceptance.acceptanceConclusion",
        data.acceptance_conclusion,
        MAX_TEXT_CHARS,
    )?;
    if data.delivery_groups.is_empty() || data.delivery_groups.len() > MAX_VIDEO_ACCEPTANCE_GROUPS {
        return Err(HostError::new(
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_GROUPS_INVALID",
            format!(
                "video completion acceptance requires 1..={MAX_VIDEO_ACCEPTANCE_GROUPS} delivery groups"
            ),
            false,
        ));
    }

    let mut group_keys = HashSet::new();
    let mut video_count = 0_usize;
    for (group_index, group) in data.delivery_groups.iter_mut().enumerate() {
        group.group_key = normalize_required(
            &format!("videoCompletionAcceptance.deliveryGroups[{group_index}].groupKey"),
            group.group_key.clone(),
            MAX_SHORT_CHARS,
        )?;
        if !group_keys.insert(group.group_key.clone()) {
            return Err(HostError::new(
                "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_GROUP_DUPLICATE",
                "video completion acceptance groupKey values must be unique",
                false,
            ));
        }
        group.name = normalize_required(
            &format!("videoCompletionAcceptance.deliveryGroups[{group_index}].name"),
            group.name.clone(),
            MAX_SHORT_CHARS,
        )?;
        group.service_description = normalize_required(
            &format!("videoCompletionAcceptance.deliveryGroups[{group_index}].serviceDescription"),
            group.service_description.clone(),
            MAX_TEXT_CHARS,
        )?;
        if group.videos.is_empty() {
            return Err(HostError::new(
                "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_VIDEOS_INVALID",
                "each video completion acceptance delivery group requires at least one video",
                false,
            ));
        }
        for (video_index, video) in group.videos.iter_mut().enumerate() {
            video_count += 1;
            if video_count > MAX_VIDEO_ACCEPTANCE_VIDEOS {
                return Err(HostError::new(
                    "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_VIDEOS_INVALID",
                    format!(
                        "video completion acceptance cannot exceed {MAX_VIDEO_ACCEPTANCE_VIDEOS} videos"
                    ),
                    false,
                ));
            }
            let prefix = format!(
                "videoCompletionAcceptance.deliveryGroups[{group_index}].videos[{video_index}]"
            );
            video.title = normalize_required(
                &format!("{prefix}.title"),
                video.title.clone(),
                MAX_SHORT_CHARS,
            )?;
            video.video_type = normalize_required(
                &format!("{prefix}.videoType"),
                video.video_type.clone(),
                MAX_SHORT_CHARS,
            )?;
            video.content = normalize_required(
                &format!("{prefix}.content"),
                video.content.clone(),
                MAX_TEXT_CHARS,
            )?;
            video.duration = normalize_required(
                &format!("{prefix}.duration"),
                video.duration.clone(),
                MAX_SHORT_CHARS,
            )?;
            video.asset_reference.asset_id = normalize_uuid(
                &format!("{prefix}.assetReference.assetId"),
                video.asset_reference.asset_id.clone(),
            )?;
            video.asset_reference.file_name = normalize_required(
                &format!("{prefix}.assetReference.fileName"),
                video.asset_reference.file_name.clone(),
                MAX_SHORT_CHARS,
            )?;
            video.asset_reference.sha256 = normalize_sha256(
                &format!("{prefix}.assetReference.sha256"),
                video.asset_reference.sha256.clone(),
            )?;
            video.asset_reference.external_link = normalize_optional(
                &format!("{prefix}.assetReference.externalLink"),
                video.asset_reference.external_link.take(),
                MAX_TEXT_CHARS,
            )?;
            if video
                .asset_reference
                .external_link
                .as_deref()
                .is_some_and(|link| !(link.starts_with("https://") || link.starts_with("http://")))
            {
                return Err(HostError::new(
                    "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_LINK_INVALID",
                    "video completion acceptance externalLink must use HTTP(S)",
                    false,
                ));
            }
            if video.screenshots.is_empty()
                || video.screenshots.len() > MAX_VIDEO_ACCEPTANCE_SCREENSHOTS_PER_VIDEO
            {
                return Err(HostError::new(
                    "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_SCREENSHOTS_INVALID",
                    format!(
                        "each video requires 1..={MAX_VIDEO_ACCEPTANCE_SCREENSHOTS_PER_VIDEO} screenshots"
                    ),
                    false,
                ));
            }
            for (screenshot_index, screenshot) in video.screenshots.iter_mut().enumerate() {
                let screenshot_prefix = format!("{prefix}.screenshots[{screenshot_index}]");
                screenshot.asset_id = normalize_uuid(
                    &format!("{screenshot_prefix}.assetId"),
                    screenshot.asset_id.clone(),
                )?;
                screenshot.sha256 = normalize_sha256(
                    &format!("{screenshot_prefix}.sha256"),
                    screenshot.sha256.clone(),
                )?;
                screenshot.caption = normalize_text(
                    &format!("{screenshot_prefix}.caption"),
                    screenshot.caption.clone(),
                    MAX_SHORT_CHARS,
                )?;
            }
        }
    }
    Ok(data)
}

fn normalize_production_result_confirmation_data(
    mut data: BusinessProductionResultConfirmationData,
) -> Result<BusinessProductionResultConfirmationData, HostError> {
    for (field, value, max) in [
        (
            "productionResultConfirmation.attachmentLabel",
            &mut data.attachment_label,
            MAX_SHORT_CHARS,
        ),
        (
            "productionResultConfirmation.contractTitle",
            &mut data.contract_title,
            MAX_SHORT_CHARS,
        ),
        (
            "productionResultConfirmation.projectTitle",
            &mut data.project_title,
            MAX_SHORT_CHARS,
        ),
        (
            "productionResultConfirmation.category",
            &mut data.category,
            MAX_SHORT_CHARS,
        ),
        (
            "productionResultConfirmation.contractDeliverableSummary",
            &mut data.contract_deliverable_summary,
            MAX_TEXT_CHARS,
        ),
        (
            "productionResultConfirmation.supplierLegalName",
            &mut data.supplier_legal_name,
            MAX_SHORT_CHARS,
        ),
        (
            "productionResultConfirmation.procurementPeriod",
            &mut data.procurement_period,
            MAX_SHORT_CHARS,
        ),
        (
            "productionResultConfirmation.acceptanceDescription",
            &mut data.acceptance_description,
            MAX_TEXT_CHARS,
        ),
        (
            "productionResultConfirmation.penaltyOrAddition",
            &mut data.penalty_or_addition,
            MAX_TEXT_CHARS,
        ),
        (
            "productionResultConfirmation.completionDate",
            &mut data.completion_date,
            MAX_SHORT_CHARS,
        ),
        (
            "productionResultConfirmation.acceptanceDate",
            &mut data.acceptance_date,
            MAX_SHORT_CHARS,
        ),
    ] {
        *value = normalize_required(field, std::mem::take(value), max)?;
    }
    if !(0..=MAX_MONEY_CENTS).contains(&data.payment_amount_cents) {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_AMOUNT_INVALID",
            "production result confirmation paymentAmountCents is outside the supported range",
            false,
        ));
    }
    if !data.clean_highlights_confirmed {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_HIGHLIGHT_CONFIRMATION_REQUIRED",
            "production result confirmation requires explicit cleanHighlightsConfirmed before generation",
            false,
        ));
    }
    if !data.manually_confirmed {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_CONFIRMATION_REQUIRED",
            "production result confirmation requires explicit manual confirmation before generation",
            false,
        ));
    }
    if data.delivery_items.is_empty()
        || data.delivery_items.len() > MAX_PRODUCTION_RESULT_CONFIRMATION_DELIVERY_ITEMS
    {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_DELIVERY_ITEMS_INVALID",
            format!(
                "production result confirmation requires 1..={} delivery items",
                MAX_PRODUCTION_RESULT_CONFIRMATION_DELIVERY_ITEMS
            ),
            false,
        ));
    }

    let mut item_keys = HashSet::new();
    let mut storyboard_numbers = HashSet::new();
    let mut shot_numbers = HashSet::new();
    let mut reference_keys = HashSet::new();
    let mut storyboard_count = 0_usize;
    let mut shot_count = 0_usize;
    let mut image_count = 0_usize;
    for (item_index, item) in data.delivery_items.iter_mut().enumerate() {
        let item_prefix = format!("productionResultConfirmation.deliveryItems[{item_index}]");
        item.item_key = normalize_required(
            &format!("{item_prefix}.itemKey"),
            std::mem::take(&mut item.item_key),
            MAX_SHORT_CHARS,
        )?;
        if !item_keys.insert(item.item_key.clone()) {
            return Err(HostError::new(
                "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_DELIVERY_ITEM_DUPLICATE",
                "production result confirmation delivery item keys must be unique",
                false,
            ));
        }
        item.title = normalize_required(
            &format!("{item_prefix}.title"),
            std::mem::take(&mut item.title),
            MAX_SHORT_CHARS,
        )?;
        item.deliverable_summary = normalize_required(
            &format!("{item_prefix}.deliverableSummary"),
            std::mem::take(&mut item.deliverable_summary),
            MAX_TEXT_CHARS,
        )?;
        for (image_index, image) in item.evidence_images.iter_mut().enumerate() {
            normalize_production_result_confirmation_asset_reference(
                &format!("{item_prefix}.evidenceImages[{image_index}]"),
                image,
                &mut reference_keys,
            )?;
            image_count += 1;
        }
        for (storyboard_index, storyboard) in item.storyboards.iter_mut().enumerate() {
            storyboard_count += 1;
            let storyboard_prefix = format!("{item_prefix}.storyboards[{storyboard_index}]");
            storyboard.storyboard_number = normalize_required(
                &format!("{storyboard_prefix}.storyboardNumber"),
                std::mem::take(&mut storyboard.storyboard_number),
                MAX_SHORT_CHARS,
            )?;
            if !storyboard_numbers.insert(storyboard.storyboard_number.clone()) {
                return Err(HostError::new(
                    "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_STORYBOARD_DUPLICATE",
                    "production result confirmation storyboard numbers must be unique",
                    false,
                ));
            }
            storyboard.title = normalize_required(
                &format!("{storyboard_prefix}.title"),
                std::mem::take(&mut storyboard.title),
                MAX_SHORT_CHARS,
            )?;
            storyboard.description = normalize_required(
                &format!("{storyboard_prefix}.description"),
                std::mem::take(&mut storyboard.description),
                MAX_TEXT_CHARS,
            )?;
            if storyboard.shots.is_empty() {
                return Err(HostError::new(
                    "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_SHOTS_INVALID",
                    "each production result confirmation storyboard requires shots",
                    false,
                ));
            }
            for (shot_index, shot) in storyboard.shots.iter_mut().enumerate() {
                shot_count += 1;
                let shot_prefix = format!("{storyboard_prefix}.shots[{shot_index}]");
                shot.shot_number = normalize_required(
                    &format!("{shot_prefix}.shotNumber"),
                    std::mem::take(&mut shot.shot_number),
                    MAX_SHORT_CHARS,
                )?;
                if !shot_numbers.insert(shot.shot_number.clone()) {
                    return Err(HostError::new(
                        "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_SHOT_DUPLICATE",
                        "production result confirmation shot numbers must be unique",
                        false,
                    ));
                }
                shot.shot_description = normalize_required(
                    &format!("{shot_prefix}.shotDescription"),
                    std::mem::take(&mut shot.shot_description),
                    MAX_TEXT_CHARS,
                )?;
                if !(1..=3).contains(&shot.images.len()) {
                    return Err(HostError::new(
                        "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_IMAGES_INVALID",
                        "each production result confirmation shot requires 1..=3 images",
                        false,
                    ));
                }
                for (image_index, image) in shot.images.iter_mut().enumerate() {
                    normalize_production_result_confirmation_asset_reference(
                        &format!("{shot_prefix}.images[{image_index}]"),
                        image,
                        &mut reference_keys,
                    )?;
                    image_count += 1;
                }
            }
        }
    }
    if storyboard_count != 4
        || shot_count != 54
        || image_count > MAX_PRODUCTION_RESULT_CONFIRMATION_IMAGES
    {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_SCRIPT_INVALID",
            "production result confirmation requires four storyboards, 54 shots and at most 256 images",
            false,
        ));
    }
    Ok(data)
}

fn normalize_production_result_confirmation_asset_reference(
    field: &str,
    reference: &mut BusinessProductionResultConfirmationAssetReference,
    keys: &mut HashSet<(String, String, String)>,
) -> Result<(), HostError> {
    reference.asset_id = normalize_uuid(
        &format!("{field}.assetId"),
        std::mem::take(&mut reference.asset_id),
    )?;
    reference.sha256 = normalize_sha256(
        &format!("{field}.sha256"),
        std::mem::take(&mut reference.sha256),
    )?;
    reference.group_key = normalize_required(
        &format!("{field}.groupKey"),
        std::mem::take(&mut reference.group_key),
        MAX_SHORT_CHARS,
    )?;
    reference.file_name = normalize_required(
        &format!("{field}.fileName"),
        std::mem::take(&mut reference.file_name),
        MAX_SHORT_CHARS,
    )?;
    reference.caption = normalize_text(
        &format!("{field}.caption"),
        std::mem::take(&mut reference.caption),
        MAX_SHORT_CHARS,
    )?;
    if !keys.insert((
        reference.asset_id.clone(),
        reference.sha256.to_ascii_uppercase(),
        reference.group_key.clone(),
    )) {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_MATERIAL_DUPLICATE",
            "production result confirmation references the same bound material more than once",
            false,
        ));
    }
    Ok(())
}

fn normalize_contract_settlement_data(
    mut data: BusinessContractSettlementData,
) -> Result<BusinessContractSettlementData, HostError> {
    data.contract_title = normalize_required(
        "contractSettlement.contractTitle",
        data.contract_title,
        MAX_SHORT_CHARS,
    )?;
    data.contract_number = normalize_required(
        "contractSettlement.contractNumber",
        data.contract_number,
        MAX_SHORT_CHARS,
    )?;
    if !(1..=MAX_MONEY_CENTS).contains(&data.original_contract_amount_cents)
        || !(-MAX_MONEY_CENTS..=MAX_MONEY_CENTS).contains(&data.contract_adjustment_cents)
        || !(1..=MAX_MONEY_CENTS).contains(&data.final_settlement_amount_cents)
    {
        return Err(HostError::new(
            "BUSINESS_CONTRACT_SETTLEMENT_AMOUNT_INVALID",
            "contract settlement amounts are outside the supported range",
            false,
        ));
    }
    let calculated_final = data
        .original_contract_amount_cents
        .checked_add(data.contract_adjustment_cents)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_CONTRACT_SETTLEMENT_AMOUNT_INVALID",
                "contract settlement amount calculation overflowed",
                false,
            )
        })?;
    if calculated_final != data.final_settlement_amount_cents {
        return Err(HostError::new(
            "BUSINESS_CONTRACT_SETTLEMENT_TOTAL_MISMATCH",
            "finalSettlementAmountCents must equal originalContractAmountCents plus contractAdjustmentCents",
            false,
        ));
    }
    if data.final_settlement_amount_cents % 100 != 0 {
        return Err(HostError::new(
            "BUSINESS_CONTRACT_SETTLEMENT_FRACTIONAL_CNY_UNCONFIRMED",
            "contract settlement template currently requires a whole-yuan final amount",
            false,
        ));
    }
    if data.retention_rate_bps.is_some_and(|value| value > 10_000) {
        return Err(HostError::new(
            "BUSINESS_CONTRACT_SETTLEMENT_RETENTION_INVALID",
            "retentionRateBps must be in 0..=10000",
            false,
        ));
    }
    if data.retention_rate_bps == Some(0) {
        data.retention_rate_bps = None;
    }
    Ok(data)
}

fn normalize_service_settlement_item(
    index: usize,
    mut item: BusinessServiceSettlementItemData,
) -> Result<BusinessServiceSettlementItemData, HostError> {
    if index >= 3 {
        return Err(HostError::new(
            "BUSINESS_SERVICE_SETTLEMENT_PAGINATION_REQUIRED",
            "service settlement template currently supports at most three verified rows",
            false,
        ));
    }
    item.service_name = normalize_required(
        &format!("serviceSettlementItems[{index}].serviceName"),
        item.service_name,
        MAX_SHORT_CHARS,
    )?;
    item.period = normalize_required(
        &format!("serviceSettlementItems[{index}].period"),
        item.period,
        MAX_SHORT_CHARS,
    )?;
    item.description = normalize_required(
        &format!("serviceSettlementItems[{index}].description"),
        item.description,
        MAX_TEXT_CHARS,
    )?;
    item.evidence_label = normalize_required(
        &format!("serviceSettlementItems[{index}].evidenceLabel"),
        item.evidence_label,
        MAX_SHORT_CHARS,
    )?;
    item.remarks = normalize_text(
        &format!("serviceSettlementItems[{index}].remarks"),
        item.remarks,
        MAX_SHORT_CHARS,
    )?;
    Ok(item)
}

fn normalize_payment_application_input(
    mut data: BusinessPaymentApplicationInput,
) -> Result<BusinessPaymentApplicationInput, HostError> {
    data.payment_id = normalize_uuid("paymentApplication.paymentId", data.payment_id)?;
    data.contract_title = normalize_required(
        "paymentApplication.contractTitle",
        data.contract_title,
        MAX_SHORT_CHARS,
    )?;
    data.contract_number = normalize_required(
        "paymentApplication.contractNumber",
        data.contract_number,
        MAX_SHORT_CHARS,
    )?;
    data.work_summary = normalize_required(
        "paymentApplication.workSummary",
        data.work_summary,
        MAX_TEXT_CHARS,
    )?;
    for (field, value) in [
        (
            "paymentApplication.paymentPeriodStart",
            &mut data.payment_period_start,
        ),
        (
            "paymentApplication.paymentPeriodEnd",
            &mut data.payment_period_end,
        ),
        (
            "paymentApplication.applicationDate",
            &mut data.application_date,
        ),
    ] {
        *value = normalize_required(field, std::mem::take(value), 10)?;
        validate_iso_date(field, value)?;
    }
    data.settlement_period = normalize_required(
        "paymentApplication.settlementPeriod",
        data.settlement_period,
        MAX_SHORT_CHARS,
    )?;
    data.supplier_bank_routing_number = normalize_required(
        "paymentApplication.supplierBankRoutingNumber",
        data.supplier_bank_routing_number,
        MAX_SHORT_CHARS,
    )?;
    if !data
        .supplier_bank_routing_number
        .chars()
        .all(|character| character.is_ascii_digit() || matches!(character, ' ' | '-'))
    {
        return Err(HostError::validation(
            "supplierBankRoutingNumber may contain only digits, spaces or hyphens",
        ));
    }
    if data.payment_sequence == 0 {
        return Err(HostError::validation(
            "paymentApplication.paymentSequence must be positive",
        ));
    }
    for amount in [
        data.invoice_amount_cents,
        data.cumulative_recognized_amount_cents,
        data.withheld_amount_cents,
    ] {
        if !(0..=MAX_MONEY_CENTS).contains(&amount) {
            return Err(HostError::validation(
                "payment application amounts are outside the supported range",
            ));
        }
    }
    if data.settlement_items.is_empty() || data.settlement_items.len() > 32 {
        return Err(HostError::validation(
            "paymentApplication.settlementItems requires 1..=32 rows",
        ));
    }
    data.settlement_items = data
        .settlement_items
        .into_iter()
        .enumerate()
        .map(|(index, mut item)| {
            item.name = normalize_required(
                &format!("paymentApplication.settlementItems[{index}].name"),
                item.name,
                MAX_SHORT_CHARS,
            )?;
            item.unit = normalize_required(
                &format!("paymentApplication.settlementItems[{index}].unit"),
                item.unit,
                MAX_SHORT_CHARS,
            )?;
            item.remarks = normalize_text(
                &format!("paymentApplication.settlementItems[{index}].remarks"),
                item.remarks,
                MAX_SHORT_CHARS,
            )?;
            if !(0..=MAX_MONEY_CENTS).contains(&item.contract_unit_price_cents)
                || !(0..=MAX_QUANTITY_MILLIS).contains(&item.original_quantity_millis)
                || !(0..=MAX_QUANTITY_MILLIS).contains(&item.settlement_quantity_millis)
            {
                return Err(HostError::validation(
                    "payment settlement item values are outside the supported range",
                ));
            }
            payment_settlement_item_amount(&item, false)?;
            payment_settlement_item_amount(&item, true)?;
            Ok(item)
        })
        .collect::<Result<Vec<_>, HostError>>()?;
    Ok(data)
}

fn validate_iso_date(field: &str, value: &str) -> Result<(), HostError> {
    let bytes = value.as_bytes();
    let valid_shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit());
    if !valid_shape {
        return Err(HostError::validation(format!(
            "{field} must use YYYY-MM-DD"
        )));
    }
    let month = value[5..7].parse::<u32>().ok();
    let day = value[8..10].parse::<u32>().ok();
    if !month.is_some_and(|value| (1..=12).contains(&value))
        || !day.is_some_and(|value| (1..=31).contains(&value))
    {
        return Err(HostError::validation(format!(
            "{field} must use YYYY-MM-DD"
        )));
    }
    Ok(())
}

fn payment_settlement_item_amount(
    item: &BusinessPaymentSettlementItemData,
    settlement: bool,
) -> Result<i64, HostError> {
    let quantity = if settlement {
        item.settlement_quantity_millis
    } else {
        item.original_quantity_millis
    };
    let product = i128::from(item.contract_unit_price_cents)
        .checked_mul(i128::from(quantity))
        .ok_or_else(|| HostError::validation("payment settlement line amount overflowed"))?;
    if product % 1_000 != 0 {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_LINE_AMOUNT_FRACTIONAL_CENT",
            "unit price multiplied by quantity must resolve to whole cents",
            false,
        ));
    }
    let amount = i64::try_from(product / 1_000)
        .map_err(|_| HostError::validation("payment settlement line amount overflowed"))?;
    if amount > MAX_MONEY_CENTS {
        return Err(HostError::validation(
            "payment settlement line amount is outside the supported range",
        ));
    }
    Ok(amount)
}

fn normalize_upsert_acceptance_material_payload(
    mut payload: UpsertBusinessAcceptanceMaterialPayload,
) -> Result<UpsertBusinessAcceptanceMaterialPayload, HostError> {
    payload.workspace_id = normalize_uuid("workspaceId", payload.workspace_id)?;
    payload.batch_id = normalize_uuid("acceptanceBatchId", payload.batch_id)?;
    payload.material.id = payload
        .material
        .id
        .map(|value| normalize_uuid("acceptance material id", value))
        .transpose()?;
    payload.material.requirement_id =
        normalize_uuid("acceptance requirement id", payload.material.requirement_id)?;
    payload.material.asset_id =
        normalize_uuid("acceptance material assetId", payload.material.asset_id)?;
    payload.material.group_key = normalize_required(
        "acceptance material groupKey",
        payload.material.group_key,
        MAX_SHORT_CHARS,
    )?;
    payload.material.duplicate_of_material_id = payload
        .material
        .duplicate_of_material_id
        .map(|value| normalize_uuid("duplicateOfMaterialId", value))
        .transpose()?;
    payload.material.notes = normalize_text(
        "acceptance material notes",
        payload.material.notes,
        MAX_TEXT_CHARS,
    )?;
    Ok(payload)
}

fn normalize_evidence_input(
    input: BusinessEvidenceInput,
) -> Result<BusinessEvidenceInput, HostError> {
    validate_timestamp("evidence occurredAt", input.occurred_at)?;
    Ok(BusinessEvidenceInput {
        asset_id: normalize_uuid("evidence assetId", input.asset_id)?,
        occurred_at: input.occurred_at,
        note: normalize_text("evidence note", input.note, MAX_TEXT_CHARS)?,
    })
}

fn normalize_manual_waiver_input(
    input: BusinessManualWaiverInput,
) -> Result<BusinessManualWaiverInput, HostError> {
    Ok(BusinessManualWaiverInput {
        reason: normalize_required("manual waiver reason", input.reason, MAX_TEXT_CHARS)?,
    })
}

fn validate_money(field: &str, value: i64) -> Result<(), HostError> {
    if (1..=MAX_MONEY_CENTS).contains(&value) {
        Ok(())
    } else {
        Err(HostError::validation(format!(
            "{field} must be in 1..={MAX_MONEY_CENTS}"
        )))
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
    if protocol_version != BUSINESS_WORKSPACE_PROTOCOL_VERSION
        && protocol_version != BUSINESS_WORKSPACE_PREVIOUS_PROTOCOL_VERSION
        && protocol_version != BUSINESS_WORKSPACE_LEGACY_PROTOCOL_VERSION
    {
        return Err(HostError::new(
            "PROTOCOL_VERSION_UNSUPPORTED",
            format!(
                "business workspace requires protocolVersion {BUSINESS_WORKSPACE_PROTOCOL_VERSION}, {BUSINESS_WORKSPACE_PREVIOUS_PROTOCOL_VERSION}, or {BUSINESS_WORKSPACE_LEGACY_PROTOCOL_VERSION}, received {protocol_version}"
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
    if deadline_at.is_some_and(|value| value <= 0) {
        return Err(HostError::validation(
            "deadlineAt must be a positive timestamp",
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
    normalize_required("windowId", context.window_id, MAX_CONTEXT_CHARS)?;
    Ok(NormalizedContext {
        actor_id: normalize_required("actorId", context.actor_id, MAX_CONTEXT_CHARS)?,
        account_id: normalize_optional("accountId", context.account_id, MAX_CONTEXT_CHARS)?,
        project_id: normalize_uuid(
            "context projectId",
            context.project_id.ok_or_else(|| {
                HostError::validation("business workspace context requires projectId")
            })?,
        )?,
        trace_id: normalize_required("traceId", context.trace_id, MAX_CONTEXT_CHARS)?,
    })
}

fn validate_expected_revision(value: Option<i64>) -> Result<(), HostError> {
    if value.is_some_and(|revision| revision >= 1) {
        Ok(())
    } else {
        Err(HostError::validation(
            "business workspace mutation requires expectedRevision >= 1",
        ))
    }
}

fn execute_transactional_command(
    connection: &mut Connection,
    vault_root: &Path,
    command: NormalizedCommand,
    fingerprint: String,
) -> Result<BusinessWorkspaceCommandOutcome, HostError> {
    let meta = command.meta().clone();
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
        return Ok(BusinessWorkspaceCommandOutcome {
            response,
            emitted_events: Vec::new(),
            emitted_asset_events: Vec::new(),
        });
    }
    validate_deadline(meta.deadline_at)?;

    let (workspace, event_type) = match &command {
        NormalizedCommand::Create { payload, .. } => (
            create_workspace(&transaction, payload, &meta.context.actor_id)?,
            BusinessWorkspaceEventType::Created,
        ),
        NormalizedCommand::UpdateProfile { payload, .. } => (
            update_profile(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context.project_id,
            )?,
            BusinessWorkspaceEventType::ProfileUpdated,
        ),
        NormalizedCommand::CreateDocument { payload, .. } => (
            create_document(
                &transaction,
                vault_root,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context.project_id,
            )?,
            BusinessWorkspaceEventType::DocumentCreated,
        ),
        NormalizedCommand::CreateAcceptanceBatch { payload, .. } => (
            create_acceptance_batch(
                &transaction,
                vault_root,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context.project_id,
            )?,
            BusinessWorkspaceEventType::AcceptanceBatchCreated,
        ),
        NormalizedCommand::PrepareAcceptanceDocuments { payload, .. } => (
            prepare_acceptance_documents(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context.project_id,
            )?,
            BusinessWorkspaceEventType::AcceptanceDocumentsPrepared,
        ),
        NormalizedCommand::UpsertAcceptanceMaterial { payload, .. } => (
            upsert_acceptance_material(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context.project_id,
            )?,
            BusinessWorkspaceEventType::AcceptanceMaterialUpserted,
        ),
        NormalizedCommand::PromoteReviewedContract { payload, .. } => (
            promote_reviewed_contract(
                &transaction,
                vault_root,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
            )?,
            BusinessWorkspaceEventType::ReviewedContractPromoted,
        ),
        NormalizedCommand::ChangeDocumentStatus { payload, .. } => (
            change_document_status(
                &transaction,
                vault_root,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
            )?,
            BusinessWorkspaceEventType::DocumentStatusChanged,
        ),
        NormalizedCommand::UpsertPayment { payload, .. } => (
            upsert_payment(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context.project_id,
            )?,
            BusinessWorkspaceEventType::PaymentUpserted,
        ),
        NormalizedCommand::UpsertSettlementBatch { payload, .. } => (
            upsert_settlement_batch(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context.project_id,
            )?,
            BusinessWorkspaceEventType::SettlementBatchUpserted,
        ),
        NormalizedCommand::VoidSettlementBatch { payload, .. } => (
            void_settlement_batch(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
            )?,
            BusinessWorkspaceEventType::SettlementBatchVoided,
        ),
        NormalizedCommand::ConfirmQuote { payload, .. } => (
            confirm_quote(
                &transaction,
                vault_root,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
            )?,
            BusinessWorkspaceEventType::QuoteConfirmed,
        ),
        NormalizedCommand::RecordReceipt { payload, .. } => (
            record_receipt(
                &transaction,
                vault_root,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
            )?,
            BusinessWorkspaceEventType::ReceiptRecorded,
        ),
        NormalizedCommand::ReverseReceipt { payload, .. } => (
            reverse_receipt(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
            )?,
            BusinessWorkspaceEventType::ReceiptReversed,
        ),
        NormalizedCommand::AdoptLatestConfirmedRequirement { payload, .. } => (
            adopt_latest_confirmed_requirement(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
            )?,
            BusinessWorkspaceEventType::RequirementAdopted,
        ),
        NormalizedCommand::UpsertCustomer { payload, .. } => (
            mutate_closure_workspace(
                &transaction,
                vault_root,
                &payload.workspace_id,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
                |transaction, _, workspace, now| {
                    business_closure_service::upsert_customer(
                        transaction,
                        workspace,
                        payload,
                        &meta.context.actor_id,
                        now,
                    )
                },
            )?,
            BusinessWorkspaceEventType::CustomerUpserted,
        ),
        NormalizedCommand::AssignCustomer { payload, .. } => (
            mutate_closure_workspace(
                &transaction,
                vault_root,
                &payload.workspace_id,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
                |transaction, _, workspace, now| {
                    business_closure_service::assign_customer(
                        transaction,
                        workspace,
                        payload,
                        &meta.context.actor_id,
                        now,
                    )
                },
            )?,
            BusinessWorkspaceEventType::CustomerAssigned,
        ),
        NormalizedCommand::UpsertMilestone { payload, .. } => (
            mutate_closure_workspace(
                &transaction,
                vault_root,
                &payload.workspace_id,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
                |transaction, _, workspace, now| {
                    business_closure_service::upsert_milestone(transaction, workspace, payload, now)
                },
            )?,
            BusinessWorkspaceEventType::MilestoneUpserted,
        ),
        NormalizedCommand::RegisterDeliverableVersion { payload, .. } => (
            mutate_closure_workspace(
                &transaction,
                vault_root,
                &payload.workspace_id,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
                |transaction, vault_root, workspace, now| {
                    business_closure_service::register_deliverable_version(
                        transaction,
                        vault_root,
                        workspace,
                        payload,
                        &meta.context.actor_id,
                        now,
                    )
                },
            )?,
            BusinessWorkspaceEventType::DeliverableVersionRegistered,
        ),
        NormalizedCommand::RecordDeliverySent { payload, .. } => (
            mutate_closure_workspace(
                &transaction,
                vault_root,
                &payload.workspace_id,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
                |transaction, _, workspace, now| {
                    business_closure_service::record_delivery_sent(
                        transaction,
                        workspace,
                        payload,
                        &meta.context.actor_id,
                        now,
                    )
                },
            )?,
            BusinessWorkspaceEventType::DeliverySent,
        ),
        NormalizedCommand::RecordDeliverySignoff { payload, .. } => (
            mutate_closure_workspace(
                &transaction,
                vault_root,
                &payload.workspace_id,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
                |transaction, vault_root, workspace, now| {
                    business_closure_service::record_delivery_signoff(
                        transaction,
                        vault_root,
                        workspace,
                        payload,
                        &meta.context.actor_id,
                        now,
                    )
                },
            )?,
            BusinessWorkspaceEventType::DeliverySignoffRecorded,
        ),
        NormalizedCommand::RecordInvoiceIssued { payload, .. } => (
            mutate_closure_workspace(
                &transaction,
                vault_root,
                &payload.workspace_id,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
                |transaction, vault_root, workspace, now| {
                    business_closure_service::record_invoice_issued(
                        transaction,
                        vault_root,
                        workspace,
                        payload,
                        &meta.context.actor_id,
                        now,
                    )
                },
            )?,
            BusinessWorkspaceEventType::InvoiceIssued,
        ),
        NormalizedCommand::RecordInvoiceRedCorrection { payload, .. } => (
            mutate_closure_workspace(
                &transaction,
                vault_root,
                &payload.workspace_id,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
                |transaction, vault_root, workspace, now| {
                    business_closure_service::record_invoice_red_correction(
                        transaction,
                        vault_root,
                        workspace,
                        payload,
                        &meta.context.actor_id,
                        now,
                    )
                },
            )?,
            BusinessWorkspaceEventType::InvoiceRedCorrected,
        ),
        NormalizedCommand::AttachInvoiceAsset { payload, .. } => (
            mutate_closure_workspace(
                &transaction,
                vault_root,
                &payload.workspace_id,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
                |transaction, vault_root, workspace, _| {
                    business_closure_service::attach_invoice_asset(
                        transaction,
                        vault_root,
                        workspace,
                        payload,
                    )
                },
            )?,
            BusinessWorkspaceEventType::InvoiceAssetAttached,
        ),
        NormalizedCommand::CreateArchiveSnapshot { .. } => {
            unreachable!("archive snapshot dispatcher handled separately")
        }
        NormalizedCommand::NormalizeLegacyTemplate { .. } => {
            unreachable!("legacy template normalization dispatcher handled separately")
        }
        NormalizedCommand::ApproveTemplateVersion { payload, .. } => (
            review_template_version(
                &transaction,
                vault_root,
                TemplateVersionReview {
                    workspace_id: payload.workspace_id.as_str(),
                    template_version_id: payload.template_version_id.as_str(),
                    target: BusinessTemplateVersionStatus::Approved,
                    note: payload.note.as_str(),
                },
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
            )?,
            BusinessWorkspaceEventType::TemplateVersionApproved,
        ),
        NormalizedCommand::RejectTemplateVersion { payload, .. } => (
            review_template_version(
                &transaction,
                vault_root,
                TemplateVersionReview {
                    workspace_id: payload.workspace_id.as_str(),
                    template_version_id: payload.template_version_id.as_str(),
                    target: BusinessTemplateVersionStatus::Rejected,
                    note: payload.note.as_str(),
                },
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
            )?,
            BusinessWorkspaceEventType::TemplateVersionRejected,
        ),
        NormalizedCommand::ChangeStatus { payload, .. } => (
            change_workspace_status(
                &transaction,
                payload,
                meta.expected_revision.expect("normalized revision"),
                &meta.context,
            )?,
            BusinessWorkspaceEventType::StatusChanged,
        ),
        NormalizedCommand::GenerateDocument { .. } => unreachable!("handled separately"),
    };
    let (response, event) = prepare_persist(
        &transaction,
        &meta,
        command_type,
        &fingerprint,
        workspace,
        event_type,
    )?;
    let commit_result = transaction.commit();
    complete_commit(
        connection,
        &meta,
        &fingerprint,
        commit_result,
        response,
        event,
    )
}

fn execute_normalize_legacy_template(
    connection: &mut Connection,
    vault_root: &Path,
    command: NormalizedCommand,
    fingerprint: String,
) -> Result<BusinessWorkspaceCommandOutcome, HostError> {
    let NormalizedCommand::NormalizeLegacyTemplate { meta, payload } = command else {
        unreachable!("legacy template dispatcher received another command")
    };

    {
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
            return Ok(BusinessWorkspaceCommandOutcome {
                response,
                emitted_events: Vec::new(),
                emitted_asset_events: Vec::new(),
            });
        }
        validate_deadline(meta.deadline_at)?;
        let workspace = load_workspace(&transaction, &payload.workspace_id)?;
        ensure_workspace_mutable(
            &workspace,
            meta.expected_revision.expect("normalized revision"),
            &meta.context.project_id,
        )?;
        ensure_template_version_absent(&transaction, &payload)?;
        transaction.commit().map_err(sql_error)?;
    }

    let (source_asset, source_path) = asset_service::verify_ready_asset_integrity(
        connection,
        vault_root,
        &payload.source_asset_id,
    )?;
    ensure_legacy_template_source_asset(
        &source_asset,
        &meta.context.project_id,
        &payload.expected_source_sha256,
    )?;

    let template_version_id = Uuid::new_v4().to_string();
    let staged = document_engine::stage_normalized_template(vault_root, &template_version_id)?;
    let normalization = legacy_doc_normalizer::normalize_legacy_doc(
        &source_path,
        &payload.expected_source_sha256,
        staged.path(),
    )?;
    let normalized_asset = asset_service::import_generated_artifact(
        connection,
        vault_root,
        &meta.context.project_id,
        staged.path(),
        asset_service::GeneratedArtifactSource::NormalizedTemplate,
        &meta.command_id,
    )?;
    if normalized_asset.sha256 != normalization.output_sha256
        || normalized_asset.size_bytes != normalization.output_size_bytes as i64
    {
        let _ = cleanup_generated_asset(connection, vault_root, &normalized_asset.id);
        return Err(HostError::new(
            "BUSINESS_NORMALIZED_TEMPLATE_ASSET_MISMATCH",
            "normalized template Asset does not match the audited DOCX output",
            false,
        ));
    }

    let final_result = (|| {
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
            return Ok(BusinessWorkspaceCommandOutcome {
                response,
                emitted_events: Vec::new(),
                emitted_asset_events: Vec::new(),
            });
        }
        validate_deadline(meta.deadline_at)?;
        let workspace = load_workspace(&transaction, &payload.workspace_id)?;
        ensure_workspace_mutable(
            &workspace,
            meta.expected_revision.expect("normalized revision"),
            &meta.context.project_id,
        )?;
        ensure_template_version_absent(&transaction, &payload)?;
        let source_asset = asset_service::get_asset(&transaction, &payload.source_asset_id)?;
        ensure_legacy_template_source_asset(
            &source_asset,
            &meta.context.project_id,
            &payload.expected_source_sha256,
        )?;
        ensure_normalized_template_asset(
            &normalized_asset,
            &meta.context.project_id,
            &normalization.output_sha256,
        )?;
        let count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM business_template_versions WHERE workspace_id = ?1",
                [&payload.workspace_id],
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        if count >= MAX_TEMPLATE_VERSIONS_PER_WORKSPACE {
            return Err(HostError::new(
                "BUSINESS_TEMPLATE_VERSION_LIMIT_REACHED",
                "business workspace template version limit reached",
                false,
            ));
        }
        let domain = TemplateVersion::new(
            &template_version_id,
            TemplateArtifact::new(&source_asset.id, &normalization.source_sha256)
                .map_err(template_domain_error)?,
            TemplateArtifact::new(&normalized_asset.id, &normalization.output_sha256)
                .map_err(template_domain_error)?,
            &payload.template_key,
            &payload.mapping_version,
            TemplateConverter::new(
                &normalization.converter_engine,
                &normalization.converter_version,
                &normalization.converter_policy_version,
            )
            .map_err(template_domain_error)?,
        )
        .map_err(template_domain_error)?;
        let now = now_millis();
        transaction
            .execute(
                "INSERT INTO business_template_versions
                 (id, workspace_id, source_asset_id, source_sha256,
                  normalized_asset_id, normalized_sha256, template_key, mapping_version,
                  converter_engine, converter_version, converter_policy_version,
                  status, reviewed_by, reviewed_at, review_note, revision, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                         'pendingReview', NULL, NULL, '', 1, ?12, ?12)",
                params![
                    domain.id(),
                    payload.workspace_id,
                    domain.source().asset_id(),
                    domain.source().sha256(),
                    domain.normalized().asset_id(),
                    domain.normalized().sha256(),
                    domain.template_key(),
                    domain.mapping_version(),
                    domain.converter().engine(),
                    domain.converter().version(),
                    domain.converter().policy(),
                    now,
                ],
            )
            .map_err(sql_error)?;
        bump_workspace(
            &transaction,
            &payload.workspace_id,
            meta.expected_revision.expect("normalized revision"),
            now,
        )?;
        let workspace = load_workspace(&transaction, &payload.workspace_id)?;
        let asset_event = asset_service::append_asset_event(
            &transaction,
            &normalized_asset,
            &meta.context.trace_id,
        )?;
        let (response, event) = prepare_persist(
            &transaction,
            &meta,
            "businessWorkspace.normalizeLegacyTemplate",
            &fingerprint,
            workspace,
            BusinessWorkspaceEventType::TemplateVersionNormalized,
        )?;
        let commit_result = transaction.commit();
        let mut outcome = complete_commit(
            connection,
            &meta,
            &fingerprint,
            commit_result,
            response,
            event,
        )?;
        if outcome
            .response
            .business_workspace
            .template_versions
            .iter()
            .any(|version| version.normalized_asset_id == normalized_asset.id)
        {
            outcome.emitted_asset_events.push(asset_event);
        }
        Ok(outcome)
    })();

    let linked = final_result.as_ref().is_ok_and(|outcome| {
        outcome
            .response
            .business_workspace
            .template_versions
            .iter()
            .any(|version| version.normalized_asset_id == normalized_asset.id)
    });
    let outcome = if linked {
        final_result
    } else {
        let cleanup_result = cleanup_generated_asset(connection, vault_root, &normalized_asset.id);
        match (final_result, cleanup_result) {
            (Err(error), _) => Err(error),
            (Ok(outcome), Ok(())) => Ok(outcome),
            (Ok(_), Err(error)) => Err(error),
        }
    };
    drop(staged);
    outcome
}

fn ensure_template_version_absent(
    connection: &Connection,
    payload: &NormalizeBusinessLegacyTemplatePayload,
) -> Result<(), HostError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM business_template_versions
                 WHERE workspace_id = ?1 AND source_asset_id = ?2 AND source_sha256 = ?3
                   AND template_key = ?4 AND mapping_version = ?5
             )",
            params![
                payload.workspace_id,
                payload.source_asset_id,
                payload.expected_source_sha256,
                payload.template_key,
                payload.mapping_version,
            ],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    if exists {
        return Err(HostError::conflict(
            "this legacy template source and mapping already have a version",
        ));
    }
    Ok(())
}

fn ensure_legacy_template_source_asset(
    asset: &crate::protocol::AssetRecord,
    project_id: &str,
    expected_sha256: &str,
) -> Result<(), HostError> {
    if asset.project_id.as_deref() != Some(project_id) {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_ASSET_PROJECT_MISMATCH",
            "legacy template Asset belongs to a different project",
            false,
        ));
    }
    if asset.kind != crate::protocol::AssetKind::Document {
        return Err(HostError::new(
            "BUSINESS_LEGACY_DOC_SOURCE_INVALID",
            "legacy template source must be a document Asset",
            false,
        ));
    }
    if !asset
        .original_name
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("doc"))
    {
        return Err(HostError::new(
            "BUSINESS_LEGACY_DOC_SOURCE_INVALID",
            "legacy template source must use the .doc extension",
            false,
        ));
    }
    if !asset.sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(HostError::new(
            "BUSINESS_LEGACY_DOC_SOURCE_SHA_MISMATCH",
            "legacy template source SHA-256 does not match the command",
            false,
        ));
    }
    Ok(())
}

fn ensure_normalized_template_asset(
    asset: &crate::protocol::AssetRecord,
    project_id: &str,
    expected_sha256: &str,
) -> Result<(), HostError> {
    if asset.project_id.as_deref() != Some(project_id)
        || asset.kind != crate::protocol::AssetKind::Document
        || !asset.sha256.eq_ignore_ascii_case(expected_sha256)
        || !asset
            .original_name
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("docx"))
    {
        return Err(HostError::new(
            "BUSINESS_NORMALIZED_TEMPLATE_ASSET_MISMATCH",
            "normalized template Asset metadata does not match the audited DOCX",
            false,
        ));
    }
    Ok(())
}

struct TemplateVersionReview<'a> {
    workspace_id: &'a str,
    template_version_id: &'a str,
    target: BusinessTemplateVersionStatus,
    note: &'a str,
}

fn review_template_version(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    review: TemplateVersionReview<'_>,
    expected_workspace_revision: i64,
    context: &NormalizedContext,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let TemplateVersionReview {
        workspace_id,
        template_version_id,
        target,
        note,
    } = review;
    let workspace = load_workspace(transaction, workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_workspace_revision, &context.project_id)?;
    let record = transaction
        .query_row(
            "SELECT id, workspace_id, source_asset_id, source_sha256,
                    normalized_asset_id, normalized_sha256, template_key, mapping_version,
                    converter_engine, converter_version, converter_policy_version,
                    status, reviewed_by, reviewed_at, review_note,
                    revision, created_at, updated_at
             FROM business_template_versions WHERE id = ?1 AND workspace_id = ?2",
            params![template_version_id, workspace_id],
            template_version_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_TEMPLATE_VERSION_NOT_FOUND",
                "template version does not exist in this workspace",
                false,
            )
        })?;
    if record.status != BusinessTemplateVersionStatus::PendingReview || record.revision != 1 {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_VERSION_TERMINAL",
            "reviewed template versions cannot transition again",
            false,
        ));
    }
    if document_engine::expected_template_mapping_version(&record.template_key)
        != Some(record.mapping_version.as_str())
    {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_MAPPING_VERSION_MISMATCH",
            "template version mapping no longer matches the registered renderer",
            false,
        ));
    }
    let (source_asset, _) = asset_service::verify_ready_asset_integrity(
        transaction,
        vault_root,
        &record.source_asset_id,
    )?;
    ensure_legacy_template_source_asset(&source_asset, &context.project_id, &record.source_sha256)?;
    let (normalized_asset, normalized_bytes) = asset_service::read_verified_template_asset(
        transaction,
        vault_root,
        &record.normalized_asset_id,
    )?;
    ensure_normalized_template_asset(
        &normalized_asset,
        &context.project_id,
        &record.normalized_sha256,
    )?;
    let source = asset_service::get_asset_source(transaction, &record.normalized_asset_id)?;
    if source.source != asset_service::AssetSourceKind::NormalizedTemplate {
        return Err(HostError::new(
            "BUSINESS_NORMALIZED_TEMPLATE_ASSET_MISMATCH",
            "template version does not reference a normalizedTemplate Asset",
            false,
        ));
    }
    if target == BusinessTemplateVersionStatus::Approved {
        document_engine::preflight_normalized_template(
            &record.template_key,
            &normalized_bytes,
            &record.normalized_sha256,
        )?;
    }

    let mut domain = TemplateVersion::new(
        &record.id,
        TemplateArtifact::new(&record.source_asset_id, &record.source_sha256)
            .map_err(template_domain_error)?,
        TemplateArtifact::new(&record.normalized_asset_id, &record.normalized_sha256)
            .map_err(template_domain_error)?,
        &record.template_key,
        &record.mapping_version,
        TemplateConverter::new(
            &record.converter_engine,
            &record.converter_version,
            &record.converter_policy_version,
        )
        .map_err(template_domain_error)?,
    )
    .map_err(template_domain_error)?;
    let reviewed_at = now_millis();
    match target {
        BusinessTemplateVersionStatus::Approved => domain
            .approve(1, &context.actor_id, reviewed_at, note)
            .map_err(template_domain_error)?,
        BusinessTemplateVersionStatus::Rejected => domain
            .reject(1, &context.actor_id, reviewed_at, note)
            .map_err(template_domain_error)?,
        BusinessTemplateVersionStatus::PendingReview => {
            return Err(HostError::validation(
                "template review target must be approved or rejected",
            ));
        }
    }
    debug_assert!(matches!(
        domain.status(),
        TemplateVersionStatus::Approved | TemplateVersionStatus::Rejected
    ));
    let changed = transaction
        .execute(
            "UPDATE business_template_versions
             SET status = ?1, reviewed_by = ?2, reviewed_at = ?3, review_note = ?4,
                 revision = revision + 1, updated_at = ?3
             WHERE id = ?5 AND workspace_id = ?6 AND status = 'pendingReview' AND revision = 1",
            params![
                template_version_status_to_db(&target),
                domain.decision().expect("review decision").actor(),
                domain
                    .decision()
                    .expect("review decision")
                    .timestamp_millis(),
                domain.decision().expect("review decision").note(),
                record.id,
                workspace_id,
            ],
        )
        .map_err(sql_error)?;
    ensure_changed(changed)?;
    bump_workspace(
        transaction,
        workspace_id,
        expected_workspace_revision,
        reviewed_at,
    )?;
    load_workspace(transaction, workspace_id)
}

fn template_domain_error(error: crate::business_v1::DomainError) -> HostError {
    HostError::new(
        "BUSINESS_TEMPLATE_VERSION_INVALID",
        error.to_string(),
        false,
    )
}

fn execute_generate_document(
    connection: &mut Connection,
    vault_root: &Path,
    command: NormalizedCommand,
    fingerprint: String,
) -> Result<BusinessWorkspaceCommandOutcome, HostError> {
    let NormalizedCommand::GenerateDocument { meta, payload } = command else {
        unreachable!("generate dispatcher received another command")
    };

    let (workspace, document) = {
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
            return Ok(BusinessWorkspaceCommandOutcome {
                response,
                emitted_events: Vec::new(),
                emitted_asset_events: Vec::new(),
            });
        }
        validate_deadline(meta.deadline_at)?;
        let workspace = load_workspace(&transaction, &payload.workspace_id)?;
        ensure_workspace_mutable(
            &workspace,
            meta.expected_revision.expect("normalized revision"),
            &meta.context.project_id,
        )?;
        let document = workspace
            .documents
            .iter()
            .find(|document| document.id == payload.document_id)
            .cloned()
            .ok_or_else(|| {
                HostError::new(
                    "BUSINESS_DOCUMENT_NOT_FOUND",
                    "business document does not exist in workspace",
                    false,
                )
            })?;
        ensure_acceptance_output_format(&workspace, &document, &payload.format)?;
        ensure_document_generatable(&document, &payload.format)?;
        ensure_document_prerequisites(&workspace, &document.kind)?;
        if let Some(batch_id) = document.snapshot.acceptance_batch_id.as_deref() {
            ensure_acceptance_ready(&workspace, batch_id)?;
        }
        if document.kind == BusinessDocumentKind::Contract {
            ensure_current_quote_confirmed(&transaction, vault_root, &workspace)?;
        }
        ensure_positive_document_total(&document)?;
        ensure_payment_request_target(&workspace, &document)?;
        ensure_payment_application_current(&workspace, &document)?;
        transaction.commit().map_err(sql_error)?;
        (workspace, document)
    };

    let template_source = load_generation_template_source(
        connection,
        vault_root,
        &meta.context.project_id,
        &payload.workspace_id,
        &document,
        &payload.format,
    )?;
    let video_completion_acceptance = load_video_completion_acceptance_generation_data(
        connection,
        vault_root,
        &meta.context.project_id,
        &workspace,
        &document,
    )?;
    let production_result_confirmation = load_production_result_confirmation_generation_data(
        connection,
        vault_root,
        &meta.context.project_id,
        &workspace,
        &document,
    )?;
    let resources = document_engine::DocumentGenerationResources {
        video_completion_acceptance: video_completion_acceptance.as_ref(),
        production_result_confirmation: production_result_confirmation.as_ref(),
    };
    let generation_id = Uuid::new_v4().to_string();
    let staged = document_engine::generate_document_with_template_and_resources(
        vault_root,
        &document,
        &payload.format,
        template_source.as_deref(),
        resources,
    )?;
    let asset = asset_service::import_business_document(
        connection,
        vault_root,
        &meta.context.project_id,
        staged.path(),
        &generation_id,
    )?;

    let final_result = (|| {
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
            return Ok(BusinessWorkspaceCommandOutcome {
                response,
                emitted_events: Vec::new(),
                emitted_asset_events: Vec::new(),
            });
        }
        validate_deadline(meta.deadline_at)?;
        let workspace = finalize_generation(
            &transaction,
            vault_root,
            &payload,
            meta.expected_revision.expect("normalized revision"),
            &meta.context.project_id,
            &asset.id,
        )?;
        let asset_event =
            asset_service::append_asset_event(&transaction, &asset, &meta.context.trace_id)?;
        let (response, event) = prepare_persist(
            &transaction,
            &meta,
            "businessWorkspace.generateDocument",
            &fingerprint,
            workspace,
            BusinessWorkspaceEventType::DocumentGenerated,
        )?;
        let commit_result = transaction.commit();
        let mut outcome = complete_commit(
            connection,
            &meta,
            &fingerprint,
            commit_result,
            response,
            event,
        )?;
        if outcome
            .response
            .business_workspace
            .documents
            .iter()
            .any(|value| value.output_asset_id.as_deref() == Some(asset.id.as_str()))
        {
            outcome.emitted_asset_events.push(asset_event);
        }
        Ok(outcome)
    })();

    let outcome = if final_result.as_ref().is_ok_and(|outcome| {
        outcome
            .response
            .business_workspace
            .documents
            .iter()
            .any(|value| value.output_asset_id.as_deref() == Some(asset.id.as_str()))
    }) {
        final_result
    } else {
        // This includes a same-command race whose durable receipt points at the
        // winner's stable assetId.
        let cleanup_result = cleanup_generated_asset(connection, vault_root, &asset.id);
        match (final_result, cleanup_result) {
            (Err(error), _) => Err(error),
            (Ok(outcome), Ok(())) => Ok(outcome),
            (Ok(_), Err(error)) => Err(error),
        }
    };
    drop(staged);
    outcome
}

fn load_generation_template_source(
    connection: &Connection,
    vault_root: &Path,
    project_id: &str,
    workspace_id: &str,
    document: &BusinessDocumentRecord,
    format: &BusinessDocumentFormat,
) -> Result<Option<Vec<u8>>, HostError> {
    if !document_engine::template_requires_source_asset(&document.template_key) {
        return Ok(None);
    }
    let template_asset_id = document
        .snapshot
        .template_asset_id
        .as_deref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_TEMPLATE_SOURCE_REQUIRED",
                "document snapshot is missing templateAssetId",
                false,
            )
        })?;
    let snapshot_sha256 = document
        .snapshot
        .template_source_sha256
        .as_deref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_TEMPLATE_SOURCE_REQUIRED",
                "document snapshot is missing templateSourceSha256",
                false,
            )
        })?;
    let (asset, bytes) =
        asset_service::read_verified_template_asset(connection, vault_root, template_asset_id)?;
    if asset.project_id.as_deref() != Some(project_id) {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_ASSET_PROJECT_MISMATCH",
            "template asset belongs to a different project",
            false,
        ));
    }
    if asset.kind != crate::protocol::AssetKind::Document {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_ASSET_KIND_INVALID",
            "template asset must be a document",
            false,
        ));
    }
    if !asset.sha256.eq_ignore_ascii_case(snapshot_sha256) {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_ASSET_HASH_MISMATCH",
            "template asset SHA-256 does not match the frozen document snapshot",
            false,
        ));
    }
    if document.template_key
        == document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY
    {
        ensure_approved_template_binding(
            connection,
            workspace_id,
            template_asset_id,
            snapshot_sha256,
            &document.template_key,
            &document.snapshot.template_mapping_version,
        )?;
    }
    if let Some(expected_sha256) =
        document_engine::expected_template_source_sha256(&document.template_key)
    {
        if !snapshot_sha256.eq_ignore_ascii_case(expected_sha256) {
            return Err(HostError::new(
                "BUSINESS_TEMPLATE_SOURCE_HASH_MISMATCH",
                "frozen template SHA-256 does not match the registered template version",
                false,
            ));
        }
    }
    let expected_extension = match format {
        BusinessDocumentFormat::Docx => "docx",
        BusinessDocumentFormat::Xlsx => "xlsx",
    };
    if !asset
        .original_name
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case(expected_extension))
    {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_ASSET_FORMAT_MISMATCH",
            format!("template asset must use .{expected_extension}"),
            false,
        ));
    }
    Ok(Some(bytes))
}

fn load_video_completion_acceptance_generation_data(
    connection: &Connection,
    vault_root: &Path,
    project_id: &str,
    workspace: &BusinessWorkspaceRecord,
    document: &BusinessDocumentRecord,
) -> Result<Option<VideoCompletionAcceptanceTemplateData>, HostError> {
    if document.template_key != document_engine::BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY {
        return Ok(None);
    }
    let frozen = document
        .snapshot
        .video_completion_acceptance
        .as_ref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_DATA_REQUIRED",
                "video completion acceptance document snapshot is missing frozen data",
                false,
            )
        })?;
    if !frozen.manually_confirmed {
        return Err(HostError::new(
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_CONFIRMATION_REQUIRED",
            "video completion acceptance requires explicit manual confirmation before generation",
            false,
        ));
    }
    let batch_id = document
        .snapshot
        .acceptance_batch_id
        .as_deref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_BATCH_NOT_FOUND",
                "video completion acceptance document is not linked to an acceptance batch",
                false,
            )
        })?;
    let output_spec_id = document
        .snapshot
        .acceptance_output_spec_id
        .as_deref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_OUTPUT_SPEC_NOT_FOUND",
                "video completion acceptance document is not linked to an output spec",
                false,
            )
        })?;
    let batch = workspace
        .acceptance_batches
        .iter()
        .find(|batch| batch.id == batch_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_BATCH_NOT_FOUND",
                "video completion acceptance batch no longer exists",
                false,
            )
        })?;
    let output_spec = batch
        .output_specs
        .iter()
        .find(|output_spec| output_spec.id == output_spec_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_OUTPUT_SPEC_NOT_FOUND",
                "video completion acceptance output spec no longer exists",
                false,
            )
        })?;
    if output_spec.video_completion_acceptance.as_ref() != Some(frozen) {
        return Err(HostError::new(
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_SNAPSHOT_STALE",
            "video completion acceptance data changed after the document snapshot was frozen",
            false,
        ));
    }
    if document.snapshot.acceptance_batch_revision != Some(batch.revision) {
        return Err(HostError::new(
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_SNAPSHOT_STALE",
            "acceptance materials changed after the document snapshot was frozen",
            false,
        ));
    }

    let current_bindings = load_acceptance_material_bindings(connection, project_id, batch_id)?
        .into_iter()
        .filter(|binding| {
            output_spec
                .requirement_ids
                .contains(&binding.requirement_id)
        })
        .collect::<Vec<_>>();
    if current_bindings != document.snapshot.material_bindings {
        return Err(HostError::new(
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_MATERIALS_CHANGED",
            "current acceptance material bindings do not match the frozen document snapshot",
            false,
        ));
    }
    for binding in &current_bindings {
        let requirement = batch
            .requirements
            .iter()
            .find(|requirement| requirement.id == binding.requirement_id)
            .ok_or_else(|| {
                HostError::new(
                    "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_REQUIREMENT_MISMATCH",
                    "video completion acceptance material references a missing requirement",
                    false,
                )
            })?;
        if requirement.kind != binding.kind {
            return Err(HostError::new(
                "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_REQUIREMENT_MISMATCH",
                "video completion acceptance material kind does not match its requirement",
                false,
            ));
        }
    }

    let expected_keys = current_bindings
        .iter()
        .filter(|binding| {
            matches!(
                binding.kind,
                BusinessAcceptanceMaterialKind::Video | BusinessAcceptanceMaterialKind::Screenshot
            )
        })
        .map(video_completion_binding_key)
        .collect::<HashSet<_>>();
    if expected_keys.len()
        != current_bindings
            .iter()
            .filter(|binding| {
                matches!(
                    binding.kind,
                    BusinessAcceptanceMaterialKind::Video
                        | BusinessAcceptanceMaterialKind::Screenshot
                )
            })
            .count()
    {
        return Err(HostError::new(
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_MATERIAL_AMBIGUOUS",
            "video completion acceptance material bindings contain duplicate asset/group entries",
            false,
        ));
    }

    let mut used_keys = HashSet::new();
    let mut delivery_groups = Vec::with_capacity(frozen.delivery_groups.len());
    for group in &frozen.delivery_groups {
        let mut videos = Vec::with_capacity(group.videos.len());
        for video in &group.videos {
            let video_key = video_completion_reference_key(
                &BusinessAcceptanceMaterialKind::Video,
                &video.asset_reference.asset_id,
                &video.asset_reference.sha256,
                &group.group_key,
            );
            ensure_video_completion_binding(&expected_keys, &mut used_keys, video_key)?;
            let (video_asset, _) = asset_service::verify_ready_asset_integrity(
                connection,
                vault_root,
                &video.asset_reference.asset_id,
            )?;
            ensure_video_completion_asset(
                &video_asset,
                project_id,
                &crate::protocol::AssetKind::Video,
                &video.asset_reference.sha256,
            )?;
            if video_asset.original_name != video.asset_reference.file_name {
                return Err(HostError::new(
                    "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_FILE_NAME_MISMATCH",
                    "video fileName does not match the authoritative Asset record",
                    false,
                ));
            }

            let mut screenshots = Vec::with_capacity(video.screenshots.len());
            for screenshot in &video.screenshots {
                let screenshot_key = video_completion_reference_key(
                    &BusinessAcceptanceMaterialKind::Screenshot,
                    &screenshot.asset_id,
                    &screenshot.sha256,
                    &group.group_key,
                );
                ensure_video_completion_binding(&expected_keys, &mut used_keys, screenshot_key)?;
                let (asset, image_bytes) = asset_service::read_verified_asset_limited(
                    connection,
                    vault_root,
                    &screenshot.asset_id,
                    MAX_VIDEO_ACCEPTANCE_SCREENSHOT_BYTES,
                )?;
                ensure_video_completion_asset(
                    &asset,
                    project_id,
                    &crate::protocol::AssetKind::Image,
                    &screenshot.sha256,
                )?;
                let (width_px, height_px) =
                    video_completion_image_dimensions(&asset.mime_type, &image_bytes)?;
                screenshots.push(VideoScreenshot {
                    asset_id: screenshot.asset_id.clone(),
                    sha256: screenshot.sha256.clone(),
                    caption: screenshot.caption.clone(),
                    mime_type: asset.mime_type,
                    image_bytes,
                    width_px,
                    height_px,
                });
            }
            videos.push(VideoBlock {
                title: video.title.clone(),
                video_type: video.video_type.clone(),
                content: video.content.clone(),
                duration: video.duration.clone(),
                asset_reference: VideoAssetReference {
                    asset_id: video.asset_reference.asset_id.clone(),
                    file_name: video.asset_reference.file_name.clone(),
                    sha256: video.asset_reference.sha256.clone(),
                    external_link: video.asset_reference.external_link.clone(),
                },
                screenshots,
            });
        }
        delivery_groups.push(VideoDeliveryGroup {
            name: group.name.clone(),
            service_description: group.service_description.clone(),
            videos,
        });
    }
    if used_keys != expected_keys {
        return Err(HostError::new(
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_MATERIAL_MISMATCH",
            "frozen video completion acceptance data does not cover the current video and screenshot material bindings",
            false,
        ));
    }

    Ok(Some(VideoCompletionAcceptanceTemplateData {
        contract_title: frozen.contract_title.clone(),
        project_title: frozen.project_title.clone(),
        completion_date: frozen.completion_date.clone(),
        delivery_groups,
        acceptance_conclusion: frozen.acceptance_conclusion.clone(),
        manually_confirmed: frozen.manually_confirmed,
    }))
}

fn load_production_result_confirmation_generation_data(
    connection: &Connection,
    vault_root: &Path,
    project_id: &str,
    workspace: &BusinessWorkspaceRecord,
    document: &BusinessDocumentRecord,
) -> Result<Option<ProductionResultConfirmationTemplateData>, HostError> {
    if document.template_key != document_engine::BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY
    {
        return Ok(None);
    }
    let frozen = document
        .snapshot
        .production_result_confirmation
        .as_ref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_DATA_REQUIRED",
                "production result confirmation document snapshot is missing frozen data",
                false,
            )
        })?;
    if !frozen.manually_confirmed {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_CONFIRMATION_REQUIRED",
            "production result confirmation requires explicit manual confirmation before generation",
            false,
        ));
    }
    if !frozen.clean_highlights_confirmed {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_HIGHLIGHT_CONFIRMATION_REQUIRED",
            "production result confirmation requires explicit clean-highlights confirmation before generation",
            false,
        ));
    }
    let batch_id = document
        .snapshot
        .acceptance_batch_id
        .as_deref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_BATCH_NOT_FOUND",
                "production result confirmation document is not linked to an acceptance batch",
                false,
            )
        })?;
    let output_spec_id = document
        .snapshot
        .acceptance_output_spec_id
        .as_deref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_OUTPUT_SPEC_NOT_FOUND",
                "production result confirmation document is not linked to an output spec",
                false,
            )
        })?;
    let batch = workspace
        .acceptance_batches
        .iter()
        .find(|batch| batch.id == batch_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_BATCH_NOT_FOUND",
                "production result confirmation batch no longer exists",
                false,
            )
        })?;
    let output_spec = batch
        .output_specs
        .iter()
        .find(|output_spec| output_spec.id == output_spec_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_OUTPUT_SPEC_NOT_FOUND",
                "production result confirmation output spec no longer exists",
                false,
            )
        })?;
    if output_spec.production_result_confirmation.as_ref() != Some(frozen)
        || document.snapshot.acceptance_batch_revision != Some(batch.revision)
    {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_SNAPSHOT_STALE",
            "production result confirmation data or acceptance revision changed after the document snapshot was frozen",
            false,
        ));
    }

    let current_bindings = load_acceptance_material_bindings(connection, project_id, batch_id)?
        .into_iter()
        .filter(|binding| {
            output_spec
                .requirement_ids
                .contains(&binding.requirement_id)
        })
        .collect::<Vec<_>>();
    if current_bindings != document.snapshot.material_bindings {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_MATERIALS_CHANGED",
            "current acceptance material bindings do not match the frozen document snapshot",
            false,
        ));
    }
    for binding in &current_bindings {
        let requirement = batch
            .requirements
            .iter()
            .find(|requirement| requirement.id == binding.requirement_id)
            .ok_or_else(|| {
                HostError::new(
                    "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_REQUIREMENT_MISMATCH",
                    "production result confirmation material references a missing requirement",
                    false,
                )
            })?;
        if requirement.kind != binding.kind
            || binding.kind != BusinessAcceptanceMaterialKind::Screenshot
        {
            return Err(HostError::new(
                "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_REQUIREMENT_MISMATCH",
                "production result confirmation requires image material bindings matching their requirements",
                false,
            ));
        }
    }
    let expected_keys = current_bindings
        .iter()
        .map(production_result_confirmation_binding_key)
        .collect::<HashSet<_>>();
    if expected_keys.len() != current_bindings.len() {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_MATERIAL_MISMATCH",
            "production result confirmation material bindings contain duplicates",
            false,
        ));
    }

    let mut used_keys = HashSet::new();
    let mut delivery_items = Vec::with_capacity(frozen.delivery_items.len());
    let mut storyboards = Vec::new();
    for item in &frozen.delivery_items {
        let images = item
            .evidence_images
            .iter()
            .map(|reference| {
                hydrate_production_result_confirmation_image(
                    connection,
                    vault_root,
                    project_id,
                    reference,
                    &expected_keys,
                    &mut used_keys,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        delivery_items.push(ProductionResultConfirmationDeliveryItem {
            item_id: item.item_key.clone(),
            name: item.title.clone(),
            specification: item.deliverable_summary.clone(),
            required_quantity: "1".to_string(),
            unit: "项".to_string(),
            received_quantity: "1".to_string(),
            acceptance_note: item.deliverable_summary.clone(),
            images,
        });
        for storyboard in &item.storyboards {
            let shots = storyboard
                .shots
                .iter()
                .map(|shot| {
                    let images = shot
                        .images
                        .iter()
                        .map(|reference| {
                            hydrate_production_result_confirmation_image(
                                connection,
                                vault_root,
                                project_id,
                                reference,
                                &expected_keys,
                                &mut used_keys,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(ProductionResultConfirmationShot {
                        shot_number: shot.shot_number.clone(),
                        scene: storyboard.storyboard_number.clone(),
                        description: shot.shot_description.clone(),
                        on_screen_copy: String::new(),
                        remarks: String::new(),
                        source_highlighted: false,
                        images,
                    })
                })
                .collect::<Result<Vec<_>, HostError>>()?;
            storyboards.push(ProductionResultConfirmationStoryboard {
                title: storyboard.title.clone(),
                specification: storyboard.description.clone(),
                production_format: frozen.category.clone(),
                duration: "详见分镜".to_string(),
                shots,
            });
        }
    }
    if used_keys != expected_keys {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_MATERIAL_MISMATCH",
            "frozen production result confirmation data does not cover the current image material bindings",
            false,
        ));
    }
    Ok(Some(ProductionResultConfirmationTemplateData {
        attachment_label: frozen.attachment_label.clone(),
        document_title: document.title.clone(),
        category: frozen.category.clone(),
        project_name: frozen.project_title.clone(),
        contract_title: frozen.contract_title.clone(),
        payment_amount_cents: frozen.payment_amount_cents,
        contract_deliverable_summary: frozen.contract_deliverable_summary.clone(),
        supplier_legal_name: frozen.supplier_legal_name.clone(),
        procurement_period: frozen.procurement_period.clone(),
        acceptance_description: frozen.acceptance_description.clone(),
        penalty_or_additions: frozen.penalty_or_addition.clone(),
        delivery_items,
        execution_completed_date: frozen.completion_date.clone(),
        acceptance_date: frozen.acceptance_date.clone(),
        handler_signoff: String::new(),
        professional_lead_signoff: String::new(),
        other_department_signoff: String::new(),
        supplier_handler_signoff: String::new(),
        storyboards,
        clean_highlights: Some(true),
    }))
}

fn production_result_confirmation_binding_key(
    binding: &BusinessAcceptanceMaterialBinding,
) -> (String, String, String, String) {
    production_result_confirmation_reference_key(
        &binding.asset_id,
        &binding.sha256,
        &binding.group_key,
    )
}

fn production_result_confirmation_reference_key(
    asset_id: &str,
    sha256: &str,
    group_key: &str,
) -> (String, String, String, String) {
    (
        acceptance_material_kind_to_db(&BusinessAcceptanceMaterialKind::Screenshot).to_string(),
        asset_id.to_string(),
        sha256.to_ascii_uppercase(),
        group_key.to_string(),
    )
}

fn hydrate_production_result_confirmation_image(
    connection: &Connection,
    vault_root: &Path,
    project_id: &str,
    reference: &BusinessProductionResultConfirmationAssetReference,
    expected_keys: &HashSet<(String, String, String, String)>,
    used_keys: &mut HashSet<(String, String, String, String)>,
) -> Result<ProductionResultConfirmationImage, HostError> {
    let key = production_result_confirmation_reference_key(
        &reference.asset_id,
        &reference.sha256,
        &reference.group_key,
    );
    if !expected_keys.contains(&key) || !used_keys.insert(key) {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_MATERIAL_MISMATCH",
            "production result confirmation assetId, sha256 or groupKey does not match an unused image material binding",
            false,
        ));
    }
    let (asset, image_bytes) = asset_service::read_verified_asset_limited(
        connection,
        vault_root,
        &reference.asset_id,
        MAX_PRODUCTION_RESULT_CONFIRMATION_IMAGE_BYTES,
    )?;
    if asset.project_id.as_deref() != Some(project_id)
        || asset.kind != crate::protocol::AssetKind::Image
        || !asset.sha256.eq_ignore_ascii_case(&reference.sha256)
        || asset.original_name != reference.file_name
    {
        return Err(HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_ASSET_MISMATCH",
            "production result confirmation image does not match its authoritative project Asset record",
            false,
        ));
    }
    let (width_px, height_px) = video_completion_image_dimensions(&asset.mime_type, &image_bytes)
        .map_err(|_| {
        HostError::new(
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_IMAGE_INVALID",
            "production result confirmation images must be valid PNG or JPEG files",
            false,
        )
    })?;
    Ok(ProductionResultConfirmationImage {
        asset_id: reference.asset_id.clone(),
        sha256: reference.sha256.clone(),
        mime_type: asset.mime_type,
        width_px,
        height_px,
        alt_text: reference.caption.clone(),
        image_bytes,
    })
}
fn video_completion_binding_key(
    binding: &BusinessAcceptanceMaterialBinding,
) -> (String, String, String, String) {
    video_completion_reference_key(
        &binding.kind,
        &binding.asset_id,
        &binding.sha256,
        &binding.group_key,
    )
}

fn video_completion_reference_key(
    kind: &BusinessAcceptanceMaterialKind,
    asset_id: &str,
    sha256: &str,
    group_key: &str,
) -> (String, String, String, String) {
    (
        acceptance_material_kind_to_db(kind).to_string(),
        asset_id.to_string(),
        sha256.to_ascii_uppercase(),
        group_key.to_string(),
    )
}

fn ensure_video_completion_binding(
    expected_keys: &HashSet<(String, String, String, String)>,
    used_keys: &mut HashSet<(String, String, String, String)>,
    key: (String, String, String, String),
) -> Result<(), HostError> {
    if !expected_keys.contains(&key) {
        return Err(HostError::new(
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_MATERIAL_MISMATCH",
            "video completion acceptance assetId, sha256 or groupKey does not match the current batch material bindings",
            false,
        ));
    }
    if !used_keys.insert(key) {
        return Err(HostError::new(
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_MATERIAL_DUPLICATE",
            "video completion acceptance data references the same bound material more than once",
            false,
        ));
    }
    Ok(())
}

fn ensure_video_completion_asset(
    asset: &crate::protocol::AssetRecord,
    project_id: &str,
    expected_kind: &crate::protocol::AssetKind,
    expected_sha256: &str,
) -> Result<(), HostError> {
    if asset.project_id.as_deref() != Some(project_id) {
        return Err(HostError::new(
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_ASSET_PROJECT_MISMATCH",
            "video completion acceptance asset belongs to a different project",
            false,
        ));
    }
    if &asset.kind != expected_kind {
        return Err(HostError::new(
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_ASSET_KIND_INVALID",
            "video completion acceptance asset kind does not match its declared role",
            false,
        ));
    }
    if !asset.sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(HostError::new(
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_ASSET_HASH_MISMATCH",
            "video completion acceptance asset SHA-256 does not match frozen data",
            false,
        ));
    }
    Ok(())
}

fn video_completion_image_dimensions(
    mime_type: &str,
    bytes: &[u8],
) -> Result<(u32, u32), HostError> {
    match mime_type.trim().to_ascii_lowercase().as_str() {
        "image/png" => {
            if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                return Err(video_completion_image_error());
            }
            let width = u32::from_be_bytes(bytes[16..20].try_into().expect("PNG width slice"));
            let height = u32::from_be_bytes(bytes[20..24].try_into().expect("PNG height slice"));
            if width == 0 || height == 0 {
                return Err(video_completion_image_error());
            }
            Ok((width, height))
        }
        "image/jpeg" => video_completion_jpeg_dimensions(bytes),
        _ => Err(video_completion_image_error()),
    }
}

fn video_completion_jpeg_dimensions(bytes: &[u8]) -> Result<(u32, u32), HostError> {
    if bytes.len() < 4 || !bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Err(video_completion_image_error());
    }
    let mut cursor = 2_usize;
    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor] == 0xff {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            break;
        }
        let marker = bytes[cursor];
        cursor += 1;
        if marker == 0xd9 || marker == 0xda {
            break;
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        if cursor + 2 > bytes.len() {
            return Err(video_completion_image_error());
        }
        let segment_length = u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]) as usize;
        if segment_length < 2 || cursor + segment_length > bytes.len() {
            return Err(video_completion_image_error());
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            if segment_length < 7 {
                return Err(video_completion_image_error());
            }
            let height = u16::from_be_bytes([bytes[cursor + 3], bytes[cursor + 4]]) as u32;
            let width = u16::from_be_bytes([bytes[cursor + 5], bytes[cursor + 6]]) as u32;
            if width == 0 || height == 0 {
                return Err(video_completion_image_error());
            }
            return Ok((width, height));
        }
        cursor += segment_length;
    }
    Err(video_completion_image_error())
}

fn video_completion_image_error() -> HostError {
    HostError::new(
        "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_IMAGE_INVALID",
        "video completion acceptance screenshots must be valid PNG or JPEG images",
        false,
    )
}

fn prepare_archive_snapshot_outside_transaction(
    connection: &Connection,
    vault_root: &Path,
    workspace: &BusinessWorkspaceRecord,
    payload: &CreateBusinessArchiveSnapshotPayload,
    snapshot_id: &str,
    actor_id: &str,
    generated_at: i64,
) -> Result<business_closure_service::PreparedBusinessArchiveSnapshot, HostError> {
    if !connection.is_autocommit() {
        return Err(HostError::internal(
            "archive snapshot preparation must run outside a SQLite transaction",
        ));
    }
    business_closure_service::prepare_archive_snapshot(
        connection,
        vault_root,
        workspace,
        payload,
        snapshot_id,
        actor_id,
        generated_at,
    )
}

fn execute_create_archive_snapshot(
    connection: &mut Connection,
    vault_root: &Path,
    command: NormalizedCommand,
    fingerprint: String,
) -> Result<BusinessWorkspaceCommandOutcome, HostError> {
    let NormalizedCommand::CreateArchiveSnapshot { meta, payload } = command else {
        unreachable!("archive snapshot dispatcher received another command")
    };
    let snapshot_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("bsaigc://business-archive/{}", meta.command_id).as_bytes(),
    )
    .to_string();

    // Phase 1: take a short, read-only snapshot and release the SQLite
    // transaction before traversing or hashing Vault files.
    let workspace = {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(sql_error)?;
        if let Some(response) = find_existing_receipt(
            &transaction,
            &meta.command_id,
            &meta.idempotency_key,
            &fingerprint,
        )? {
            transaction.commit().map_err(sql_error)?;
            return Ok(BusinessWorkspaceCommandOutcome {
                response,
                emitted_events: Vec::new(),
                emitted_asset_events: Vec::new(),
            });
        }
        validate_deadline(meta.deadline_at)?;
        let workspace = load_workspace(&transaction, &payload.workspace_id)?;
        ensure_workspace_mutable(
            &workspace,
            meta.expected_revision.expect("normalized revision"),
            &meta.context.project_id,
        )?;
        transaction.commit().map_err(sql_error)?;
        workspace
    };

    // Phase 2: all expensive file traversal, hashing, manifest creation, and
    // ZIP compression happens in autocommit mode. The final transaction below
    // performs the authoritative revision CAS before linking these artifacts.
    let mut prepared = prepare_archive_snapshot_outside_transaction(
        connection,
        vault_root,
        &workspace,
        &payload,
        &snapshot_id,
        &meta.context.actor_id,
        now_millis(),
    )?;

    let manifest_asset = asset_service::import_generated_artifact(
        connection,
        vault_root,
        &meta.context.project_id,
        &prepared.manifest_path,
        asset_service::GeneratedArtifactSource::ArchiveManifest,
        &format!("{snapshot_id}:manifest"),
    )?;
    let package_asset = match asset_service::import_generated_artifact(
        connection,
        vault_root,
        &meta.context.project_id,
        &prepared.package_path,
        asset_service::GeneratedArtifactSource::ArchivePackage,
        &format!("{snapshot_id}:package"),
    ) {
        Ok(asset) => asset,
        Err(error) => {
            let _ = cleanup_generated_asset(connection, vault_root, &manifest_asset.id);
            return Err(error);
        }
    };
    prepared.snapshot.manifest_asset_id = Some(manifest_asset.id.clone());
    prepared.snapshot.package_asset_id = Some(package_asset.id.clone());

    // Phase 3: re-check idempotency and workspace revision, then atomically
    // bind snapshot, assets, domain event, and command receipt.
    let final_result = (|| {
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
            return Ok(BusinessWorkspaceCommandOutcome {
                response,
                emitted_events: Vec::new(),
                emitted_asset_events: Vec::new(),
            });
        }
        validate_deadline(meta.deadline_at)?;
        let workspace = mutate_closure_workspace(
            &transaction,
            vault_root,
            &payload.workspace_id,
            meta.expected_revision.expect("normalized revision"),
            &meta.context,
            |transaction, vault_root, workspace, _| {
                business_closure_service::persist_archive_snapshot(
                    transaction,
                    vault_root,
                    workspace,
                    &prepared.snapshot,
                )
            },
        )?;
        let manifest_event = asset_service::append_asset_event(
            &transaction,
            &manifest_asset,
            &meta.context.trace_id,
        )?;
        let package_event = asset_service::append_asset_event(
            &transaction,
            &package_asset,
            &meta.context.trace_id,
        )?;
        let (response, event) = prepare_persist(
            &transaction,
            &meta,
            "businessWorkspace.createArchiveSnapshot",
            &fingerprint,
            workspace,
            BusinessWorkspaceEventType::ArchiveSnapshotPrepared,
        )?;
        let commit_result = transaction.commit();
        let mut outcome = complete_commit(
            connection,
            &meta,
            &fingerprint,
            commit_result,
            response,
            event,
        )?;
        if outcome
            .response
            .business_workspace
            .archive_snapshots
            .iter()
            .any(|snapshot| snapshot.id == snapshot_id)
        {
            outcome.emitted_asset_events.push(manifest_event);
            outcome.emitted_asset_events.push(package_event);
        }
        Ok(outcome)
    })();

    let linked = final_result.as_ref().is_ok_and(|outcome| {
        outcome
            .response
            .business_workspace
            .archive_snapshots
            .iter()
            .any(|snapshot| snapshot.id == snapshot_id)
    });
    if linked {
        final_result
    } else {
        let package_cleanup = cleanup_generated_asset(connection, vault_root, &package_asset.id);
        let manifest_cleanup = cleanup_generated_asset(connection, vault_root, &manifest_asset.id);
        match (final_result, package_cleanup, manifest_cleanup) {
            (Err(error), _, _) => Err(error),
            (Ok(outcome), Ok(()), Ok(())) => Ok(outcome),
            (Ok(_), Err(error), _) | (Ok(_), Ok(()), Err(error)) => Err(error),
        }
    }
}

fn execute_archive_status_change(
    connection: &mut Connection,
    vault_root: &Path,
    command: NormalizedCommand,
    fingerprint: String,
) -> Result<BusinessWorkspaceCommandOutcome, HostError> {
    let NormalizedCommand::ChangeStatus { meta, payload } = command else {
        unreachable!("archive status dispatcher received another command")
    };
    if payload.status != BusinessWorkspaceStatus::Archived {
        return Err(HostError::internal(
            "archive status dispatcher requires the Archived target state",
        ));
    }
    let expected_revision = meta.expected_revision.expect("normalized revision");

    // Phase 1: capture the exact workspace and archive snapshot under a short
    // read transaction. Expensive Vault and ZIP verification must not hold a
    // SQLite writer transaction.
    let (workspace, verified_snapshot) = {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(sql_error)?;
        if let Some(response) = find_existing_receipt(
            &transaction,
            &meta.command_id,
            &meta.idempotency_key,
            &fingerprint,
        )? {
            transaction.commit().map_err(sql_error)?;
            return Ok(BusinessWorkspaceCommandOutcome {
                response,
                emitted_events: Vec::new(),
                emitted_asset_events: Vec::new(),
            });
        }
        validate_deadline(meta.deadline_at)?;
        let workspace = load_workspace(&transaction, &payload.workspace_id)?;
        ensure_workspace_owned(&workspace, expected_revision, &meta.context.project_id)?;
        if workspace.status == BusinessWorkspaceStatus::Archived {
            return Err(HostError::validation(
                "business workspace already has requested status",
            ));
        }
        ensure_workspace_archivable(&workspace)?;
        let snapshot = workspace.archive_snapshots.last().cloned().ok_or_else(|| {
            HostError::new(
                business_closure_service::BUSINESS_ARCHIVE_INTEGRITY_ERROR_CODE,
                "archive snapshot is missing before archive status transition",
                false,
            )
        })?;
        transaction.commit().map_err(sql_error)?;
        (workspace, snapshot)
    };

    // Phase 2: verify both durable Assets plus the complete ZIP payload while
    // SQLite is in autocommit mode.
    business_closure_service::verify_archive_snapshot_integrity(
        connection,
        vault_root,
        &workspace,
        &verified_snapshot,
    )?;

    // Phase 3: re-check idempotency, revision, archivable state, and the exact
    // snapshot identity before atomically writing status, event, and receipt.
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
        return Ok(BusinessWorkspaceCommandOutcome {
            response,
            emitted_events: Vec::new(),
            emitted_asset_events: Vec::new(),
        });
    }
    validate_deadline(meta.deadline_at)?;
    let current = load_workspace(&transaction, &payload.workspace_id)?;
    ensure_workspace_owned(&current, expected_revision, &meta.context.project_id)?;
    if current.status == BusinessWorkspaceStatus::Archived {
        return Err(HostError::validation(
            "business workspace already has requested status",
        ));
    }
    ensure_workspace_archivable(&current)?;
    let current_snapshot = current.archive_snapshots.last().ok_or_else(|| {
        HostError::new(
            business_closure_service::BUSINESS_ARCHIVE_INTEGRITY_ERROR_CODE,
            "archive snapshot disappeared before archive status transition",
            false,
        )
    })?;
    if current_snapshot != &verified_snapshot {
        return Err(HostError::conflict(
            "archive snapshot changed while integrity verification was in progress",
        ));
    }

    let workspace =
        change_workspace_status(&transaction, &payload, expected_revision, &meta.context)?;
    let (response, event) = prepare_persist(
        &transaction,
        &meta,
        "businessWorkspace.changeStatus",
        &fingerprint,
        workspace,
        BusinessWorkspaceEventType::StatusChanged,
    )?;
    let commit_result = transaction.commit();
    complete_commit(
        connection,
        &meta,
        &fingerprint,
        commit_result,
        response,
        event,
    )
}

pub(crate) fn verify_archive_package_for_export(
    connection: &Connection,
    vault_root: &Path,
    asset_id: &str,
) -> Result<(), HostError> {
    let references = {
        let mut statement = connection
            .prepare(
                "SELECT id, workspace_id
                 FROM business_archive_snapshots
                 WHERE package_asset_id = ?1
                 ORDER BY id ASC",
            )
            .map_err(sql_error)?;
        let rows = statement
            .query_map([asset_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        drop(statement);
        rows
    };

    let Some((snapshot_id, workspace_id)) = references.first() else {
        return Ok(());
    };
    if references.len() != 1 {
        return Err(HostError::new(
            business_closure_service::BUSINESS_ARCHIVE_INTEGRITY_ERROR_CODE,
            "archive package is linked to multiple snapshots",
            false,
        ));
    }

    let workspace = load_workspace(connection, workspace_id)?;
    let snapshot = workspace
        .archive_snapshots
        .iter()
        .find(|snapshot| snapshot.id == *snapshot_id)
        .ok_or_else(|| {
            HostError::new(
                business_closure_service::BUSINESS_ARCHIVE_INTEGRITY_ERROR_CODE,
                "archive package snapshot cannot be loaded",
                false,
            )
        })?;
    if snapshot.package_asset_id.as_deref() != Some(asset_id) {
        return Err(HostError::new(
            business_closure_service::BUSINESS_ARCHIVE_INTEGRITY_ERROR_CODE,
            "archive package association changed before export",
            false,
        ));
    }
    business_closure_service::verify_archive_snapshot_integrity(
        connection, vault_root, &workspace, snapshot,
    )
}

fn prepare_persist(
    transaction: &Transaction<'_>,
    meta: &CommandMeta,
    command_type: &str,
    fingerprint: &str,
    workspace: BusinessWorkspaceRecord,
    event_type: BusinessWorkspaceEventType,
) -> Result<
    (
        BusinessWorkspaceCommandResponse,
        BusinessWorkspaceDomainEvent,
    ),
    HostError,
> {
    let event = append_event(transaction, event_type, &workspace, meta, command_type)?;
    let completed_at = now_millis();
    let response = BusinessWorkspaceCommandResponse {
        receipt: CommandReceipt {
            command_id: meta.command_id.clone(),
            idempotency_key: meta.idempotency_key.clone(),
            command_type: command_type.to_string(),
            aggregate_id: workspace.id.clone(),
            revision: workspace.revision,
            last_event_sequence: event.sequence,
            completed_at,
        },
        business_workspace: workspace,
        replayed: false,
    };
    transaction
        .execute(
            "INSERT INTO business_workspace_command_receipts
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
                serialize_business_journal(&response)?,
                completed_at,
            ],
        )
        .map_err(sql_error)?;

    Ok((response, event))
}

fn complete_commit(
    connection: &Connection,
    meta: &CommandMeta,
    fingerprint: &str,
    commit_result: rusqlite::Result<()>,
    response: BusinessWorkspaceCommandResponse,
    event: BusinessWorkspaceDomainEvent,
) -> Result<BusinessWorkspaceCommandOutcome, HostError> {
    match commit_result {
        Ok(()) => Ok(BusinessWorkspaceCommandOutcome {
            response,
            emitted_events: vec![event],
            emitted_asset_events: Vec::new(),
        }),
        Err(commit_error) => match find_existing_receipt(
            connection,
            &meta.command_id,
            &meta.idempotency_key,
            fingerprint,
        ) {
            Ok(Some(mut persisted)) => {
                persisted.replayed = false;
                let persisted_event =
                    load_event(connection, persisted.receipt.last_event_sequence)?;
                Ok(BusinessWorkspaceCommandOutcome {
                    response: persisted,
                    emitted_events: vec![persisted_event],
                    emitted_asset_events: Vec::new(),
                })
            }
            Ok(None) => Err(sql_error(commit_error)),
            Err(error) => Err(error),
        },
    }
}

fn create_workspace(
    transaction: &Transaction<'_>,
    payload: &CreateBusinessWorkspacePayload,
    actor_id: &str,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let context = load_workspace_creation_context(transaction, &payload.project_id)?;
    let source = payload
        .prefill_source_workspace_id
        .as_deref()
        .map(|workspace_id| load_prefill_source_workspace(transaction, workspace_id))
        .transpose()?;
    if let Some(source) = source.as_ref() {
        prefill_match_kind(&source.profile, &context.client_name)?;
    }
    let (requirement_brief_id, mut profile) = prefill_profile(
        &payload.project_id,
        context.project_name,
        context.client_name,
        context.confirmed,
    )?;
    if let Some(source) = source.as_ref() {
        apply_reusable_master_data(&mut profile, &source.profile);
    }
    let now = now_millis();
    let workspace = BusinessWorkspaceRecord {
        id: Uuid::new_v4().to_string(),
        project_id: payload.project_id.clone(),
        customer_id: String::new(),
        customer: Default::default(),
        requirement_brief_id,
        requirement_brief_revision: context.confirmed_revision,
        prefill_source_workspace_id: payload.prefill_source_workspace_id.clone(),
        profile,
        documents: Vec::new(),
        acceptance_batches: Vec::new(),
        template_versions: Vec::new(),
        payments: Vec::new(),
        quote_confirmations: Vec::new(),
        receipts: Vec::new(),
        milestones: Vec::new(),
        settlement_batches: Vec::new(),
        delivery_submissions: Vec::new(),
        invoices: Vec::new(),
        archive_snapshots: Vec::new(),
        archive_integrity_status: BusinessArchiveIntegrityStatus::NotCaptured,
        status: BusinessWorkspaceStatus::Active,
        archived_at: None,
        archived_by: None,
        lifecycle_stage: BusinessLifecycleStage::Draft,
        financial_summary: BusinessFinancialSummary::default(),
        current_documents: BusinessCurrentDocuments::default(),
        revision: 1,
        created_at: now,
        updated_at: now,
    };
    transaction
        .execute(
            "INSERT INTO business_workspaces
             (id, project_id, requirement_brief_id, requirement_brief_revision,
              prefill_source_workspace_id, customer_name_key, customer_legal_name_key,
              profile_json, status, archived_at, archived_by, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                     'active', NULL, NULL, 1, ?9, ?9)",
            params![
                workspace.id,
                workspace.project_id,
                workspace.requirement_brief_id,
                workspace.requirement_brief_revision,
                workspace.prefill_source_workspace_id,
                normalized_business_identity(&workspace.profile.customer_name),
                normalized_business_identity(&workspace.profile.customer_legal_name),
                serde_json::to_string(&workspace.profile).map_err(json_error)?,
                now,
            ],
        )
        .map_err(map_workspace_insert_error)?;
    business_closure_service::attach_customer_for_new_workspace(
        transaction,
        &workspace.id,
        &workspace.profile,
        payload.customer_id.as_deref(),
        actor_id,
        now,
    )?;
    load_workspace(transaction, &workspace.id)
}

#[derive(Debug, Clone)]
struct WorkspaceCreationContext {
    project_name: String,
    client_name: String,
    confirmed: Option<(String, RequirementBriefContent)>,
    confirmed_revision: Option<i64>,
}

fn load_workspace_creation_context(
    connection: &Connection,
    project_id: &str,
) -> Result<WorkspaceCreationContext, HostError> {
    if connection
        .query_row(
            "SELECT 1 FROM business_workspaces WHERE project_id = ?1",
            [project_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(sql_error)?
        .is_some()
    {
        return Err(HostError::new(
            "BUSINESS_WORKSPACE_EXISTS",
            "project already has a business workspace",
            false,
        ));
    }
    let (project_name, client_name) = connection
        .query_row(
            "SELECT name, client_name FROM projects WHERE id = ?1",
            [project_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::new("PROJECT_NOT_FOUND", "project does not exist", false))?;
    let confirmed_row = connection
        .query_row(
            "SELECT id, revision, content_json FROM requirement_briefs
             WHERE project_id = ?1 AND status = 'confirmed'",
            [project_id],
            |row| {
                let id: String = row.get(0)?;
                let revision: i64 = row.get(1)?;
                let json: String = row.get(2)?;
                let content = serde_json::from_str(&json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok((id, revision, content))
            },
        )
        .optional()
        .map_err(sql_error)?;
    let confirmed_revision = confirmed_row.as_ref().map(|(_, revision, _)| *revision);
    let confirmed = confirmed_row.map(|(id, _, content)| (id, content));
    Ok(WorkspaceCreationContext {
        project_name,
        client_name,
        confirmed,
        confirmed_revision,
    })
}

#[derive(Debug, Clone)]
struct PrefillSourceWorkspace {
    id: String,
    project_id: String,
    project_title: String,
    profile: BusinessProfile,
    status: BusinessWorkspaceStatus,
    revision: i64,
    updated_at: i64,
}

fn prefill_source_from_row(row: &Row<'_>) -> rusqlite::Result<PrefillSourceWorkspace> {
    let profile_json: String = row.get(3)?;
    let status: String = row.get(4)?;
    Ok(PrefillSourceWorkspace {
        id: row.get(0)?,
        project_id: row.get(1)?,
        project_title: row.get(2)?,
        profile: from_json_column(&profile_json)?,
        status: workspace_status_from_db(&status)?,
        revision: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn load_prefill_source_workspace(
    connection: &Connection,
    workspace_id: &str,
) -> Result<PrefillSourceWorkspace, HostError> {
    connection
        .query_row(
            "SELECT workspace.id, workspace.project_id, project.name,
                    workspace.profile_json, workspace.status, workspace.revision,
                    workspace.updated_at
             FROM business_workspaces workspace
             JOIN projects project ON project.id = workspace.project_id
             WHERE workspace.id = ?1",
            [workspace_id],
            prefill_source_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_PREFILL_SOURCE_NOT_FOUND",
                "business workspace prefill source does not exist",
                false,
            )
        })
}

fn prefill_match_kind(
    source_profile: &BusinessProfile,
    target_client_name: &str,
) -> Result<BusinessWorkspacePrefillMatchKind, HostError> {
    let target = normalized_business_identity(target_client_name);
    let customer_name = normalized_business_identity(&source_profile.customer_name);
    let customer_legal_name = normalized_business_identity(&source_profile.customer_legal_name);
    let name_matches = !target.is_empty() && target == customer_name;
    let legal_name_matches = !target.is_empty() && target == customer_legal_name;
    match (name_matches, legal_name_matches) {
        (true, true) => Ok(BusinessWorkspacePrefillMatchKind::Both),
        (true, false) => Ok(BusinessWorkspacePrefillMatchKind::CustomerName),
        (false, true) => Ok(BusinessWorkspacePrefillMatchKind::CustomerLegalName),
        (false, false) => Err(HostError::new(
            "BUSINESS_PREFILL_CUSTOMER_MISMATCH",
            "business workspace prefill source belongs to a different customer",
            false,
        )),
    }
}

fn normalized_business_identity(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(|character| character.to_lowercase())
        .collect()
}

fn reusable_field_value(profile: &BusinessProfile, field: BusinessWorkspacePrefillField) -> String {
    match field {
        BusinessWorkspacePrefillField::CustomerLegalName => profile.customer_legal_name.clone(),
        BusinessWorkspacePrefillField::CustomerTaxId => profile.customer_tax_id.clone(),
        BusinessWorkspacePrefillField::CustomerAddress => profile.customer_address.clone(),
        BusinessWorkspacePrefillField::CustomerContact => profile.customer_contact.clone(),
        BusinessWorkspacePrefillField::CustomerPhone => profile.customer_phone.clone(),
        BusinessWorkspacePrefillField::CustomerEmail => profile.customer_email.clone(),
        BusinessWorkspacePrefillField::SupplierLegalName => profile.supplier_legal_name.clone(),
        BusinessWorkspacePrefillField::SupplierTaxId => profile.supplier_tax_id.clone(),
        BusinessWorkspacePrefillField::SupplierAddress => profile.supplier_address.clone(),
        BusinessWorkspacePrefillField::SupplierContact => profile.supplier_contact.clone(),
        BusinessWorkspacePrefillField::SupplierPhone => profile.supplier_phone.clone(),
        BusinessWorkspacePrefillField::SupplierBankName => profile.supplier_bank_name.clone(),
        BusinessWorkspacePrefillField::SupplierBankAccount => profile.supplier_bank_account.clone(),
        BusinessWorkspacePrefillField::Currency => profile.currency.clone(),
        BusinessWorkspacePrefillField::DefaultTaxRateBps => {
            // A zero rate counts as "not populated" so preview and apply agree
            // on skipping it during prefill.
            if profile.default_tax_rate_bps > 0 {
                profile.default_tax_rate_bps.to_string()
            } else {
                String::new()
            }
        }
    }
}

fn reusable_field_is_populated(
    profile: &BusinessProfile,
    field: BusinessWorkspacePrefillField,
) -> bool {
    match field {
        BusinessWorkspacePrefillField::DefaultTaxRateBps => profile.default_tax_rate_bps != 0,
        _ => !reusable_field_value(profile, field).is_empty(),
    }
}

fn populated_reusable_fields(profile: &BusinessProfile) -> Vec<BusinessWorkspacePrefillField> {
    REUSABLE_PREFILL_FIELDS
        .iter()
        .copied()
        .filter(|field| reusable_field_is_populated(profile, *field))
        .collect()
}

fn apply_reusable_field(
    target: &mut BusinessProfile,
    source: &BusinessProfile,
    field: BusinessWorkspacePrefillField,
) {
    // Prefill only fills from non-empty source values; an empty field in the
    // source workspace must never clear a value the target already derived
    // (e.g. customerLegalName seeded from the project client name).
    fn fill(target: &mut String, source: &str) {
        if !source.trim().is_empty() {
            *target = source.to_string();
        }
    }
    match field {
        BusinessWorkspacePrefillField::CustomerLegalName => {
            fill(&mut target.customer_legal_name, &source.customer_legal_name)
        }
        BusinessWorkspacePrefillField::CustomerTaxId => {
            fill(&mut target.customer_tax_id, &source.customer_tax_id)
        }
        BusinessWorkspacePrefillField::CustomerAddress => {
            fill(&mut target.customer_address, &source.customer_address)
        }
        BusinessWorkspacePrefillField::CustomerContact => {
            fill(&mut target.customer_contact, &source.customer_contact)
        }
        BusinessWorkspacePrefillField::CustomerPhone => {
            fill(&mut target.customer_phone, &source.customer_phone)
        }
        BusinessWorkspacePrefillField::CustomerEmail => {
            fill(&mut target.customer_email, &source.customer_email)
        }
        BusinessWorkspacePrefillField::SupplierLegalName => {
            fill(&mut target.supplier_legal_name, &source.supplier_legal_name)
        }
        BusinessWorkspacePrefillField::SupplierTaxId => {
            fill(&mut target.supplier_tax_id, &source.supplier_tax_id)
        }
        BusinessWorkspacePrefillField::SupplierAddress => {
            fill(&mut target.supplier_address, &source.supplier_address)
        }
        BusinessWorkspacePrefillField::SupplierContact => {
            fill(&mut target.supplier_contact, &source.supplier_contact)
        }
        BusinessWorkspacePrefillField::SupplierPhone => {
            fill(&mut target.supplier_phone, &source.supplier_phone)
        }
        BusinessWorkspacePrefillField::SupplierBankName => {
            fill(&mut target.supplier_bank_name, &source.supplier_bank_name)
        }
        BusinessWorkspacePrefillField::SupplierBankAccount => fill(
            &mut target.supplier_bank_account,
            &source.supplier_bank_account,
        ),
        BusinessWorkspacePrefillField::Currency => fill(&mut target.currency, &source.currency),
        BusinessWorkspacePrefillField::DefaultTaxRateBps => {
            if source.default_tax_rate_bps > 0 {
                target.default_tax_rate_bps = source.default_tax_rate_bps
            }
        }
    }
}

fn apply_reusable_master_data(target: &mut BusinessProfile, source: &BusinessProfile) {
    for field in REUSABLE_PREFILL_FIELDS {
        apply_reusable_field(target, source, field);
    }
}

fn reusable_prefill_changes(
    target: &BusinessProfile,
    source: &BusinessProfile,
) -> Vec<BusinessWorkspacePrefillChange> {
    REUSABLE_PREFILL_FIELDS
        .iter()
        .copied()
        .map(|field| {
            let target_value = reusable_field_value(target, field);
            let source_value = reusable_field_value(source, field);
            // Mirrors apply_reusable_field: empty source values are skipped,
            // so they preview as Unchanged instead of a destructive Cleared.
            let decision = if target_value == source_value || source_value.is_empty() {
                BusinessWorkspacePrefillDecision::Unchanged
            } else if target_value.is_empty() {
                BusinessWorkspacePrefillDecision::Filled
            } else {
                BusinessWorkspacePrefillDecision::Replaced
            };
            let result_value = if source_value.is_empty() {
                target_value.clone()
            } else {
                source_value.clone()
            };
            BusinessWorkspacePrefillChange {
                field,
                target_value,
                result_value,
                source_value,
                decision,
            }
        })
        .collect()
}
fn prefill_profile(
    project_id: &str,
    project_name: String,
    client_name: String,
    confirmed: Option<(String, RequirementBriefContent)>,
) -> Result<(Option<String>, BusinessProfile), HostError> {
    let project_code = format!("PRJ-{}", &project_id.replace('-', "")[..8].to_uppercase());
    let mut profile = BusinessProfile {
        project_title: project_name,
        project_code,
        customer_name: client_name.clone(),
        customer_legal_name: client_name,
        currency: "CNY".to_string(),
        ..BusinessProfile::default()
    };
    let requirement_brief_id = confirmed.as_ref().map(|(id, _)| id.clone());
    if let Some((_, content)) = confirmed {
        profile.delivery_summary = join_sections([
            content.objective.as_str(),
            content.key_message.as_str(),
            &content.deliverables.join("\n"),
        ]);
        profile.acceptance_terms = content.acceptance_criteria.join("\n");
        profile.notes = join_labeled_sections([
            ("Constraints", content.constraints.as_slice()),
            ("Risks", content.risks.as_slice()),
        ]);
        profile.service_end_at = content.deadline_at;
        profile.line_items = content
            .deliverables
            .into_iter()
            .map(|deliverable| BusinessLineItem {
                id: Uuid::new_v4().to_string(),
                name: deliverable,
                description: String::new(),
                quantity_millis: 1_000,
                unit: "item".to_string(),
                unit_price_cents: 0,
                tax_rate_bps: 0,
                amount_cents: 0,
            })
            .collect();
    }
    Ok((requirement_brief_id, profile))
}

fn join_sections<'a>(sections: impl IntoIterator<Item = &'a str>) -> String {
    sections
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn join_labeled_sections<'a>(
    sections: impl IntoIterator<Item = (&'a str, &'a [String])>,
) -> String {
    sections
        .into_iter()
        .filter(|(_, values)| !values.is_empty())
        .map(|(label, values)| format!("{label}: {}", values.join("; ")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn update_profile(
    transaction: &Transaction<'_>,
    payload: &UpdateBusinessProfilePayload,
    expected_revision: i64,
    project_id: &str,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, project_id)?;
    let profile = normalize_profile(payload.profile.clone(), &workspace.profile.line_items)?;
    let now = now_millis();
    business_closure_service::sync_customer_from_profile(transaction, &workspace, &profile, now)?;
    let changed = transaction
        .execute(
            "UPDATE business_workspaces
             SET profile_json = ?1, customer_name_key = ?2, customer_legal_name_key = ?3,
                 revision = revision + 1, updated_at = ?4
             WHERE id = ?5 AND revision = ?6",
            params![
                serde_json::to_string(&profile).map_err(json_error)?,
                normalized_business_identity(&profile.customer_name),
                normalized_business_identity(&profile.customer_legal_name),
                now,
                workspace.id,
                expected_revision,
            ],
        )
        .map_err(sql_error)?;
    ensure_changed(changed)?;
    load_workspace(transaction, &workspace.id)
}

fn normalize_profile(
    input: BusinessProfileInput,
    existing_items: &[BusinessLineItem],
) -> Result<BusinessProfile, HostError> {
    if input.line_items.len() > MAX_LINE_ITEMS {
        return Err(HostError::validation(format!(
            "lineItems cannot contain more than {MAX_LINE_ITEMS} items"
        )));
    }
    if !(0..=10_000).contains(&input.default_tax_rate_bps) {
        return Err(HostError::validation(
            "defaultTaxRateBps must be in 0..10000",
        ));
    }
    if !(0..=MAX_MONEY_CENTS).contains(&input.project_discount_cents) {
        return Err(HostError::validation(
            "projectDiscountCents is outside the supported range",
        ));
    }
    validate_timestamp("serviceStartAt", input.service_start_at)?;
    validate_timestamp("serviceEndAt", input.service_end_at)?;
    if input
        .service_start_at
        .zip(input.service_end_at)
        .is_some_and(|(start, end)| start > end)
    {
        return Err(HostError::validation(
            "serviceStartAt cannot be after serviceEndAt",
        ));
    }
    let currency = input.currency.trim().to_ascii_uppercase();
    if currency.len() != 3 || !currency.bytes().all(|value| value.is_ascii_alphabetic()) {
        return Err(HostError::validation(
            "currency must be a three-letter ISO-style code",
        ));
    }

    let existing = existing_items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut line_items = Vec::with_capacity(input.line_items.len());
    let mut total = 0_i64;
    for item in input.line_items {
        let id = match item.id.clone() {
            Some(id) => {
                let id = normalize_uuid("lineItem id", id)?;
                if !existing.contains_key(id.as_str()) {
                    return Err(HostError::new(
                        "BUSINESS_LINE_ITEM_ID_INVALID",
                        "line item ID is server-owned and does not belong to this workspace",
                        false,
                    ));
                }
                id
            }
            None => Uuid::new_v4().to_string(),
        };
        if !seen.insert(id.clone()) {
            return Err(HostError::validation("duplicate line item ID"));
        }
        let normalized = normalize_line_item(id, item, input.tax_mode)?;
        total = total.checked_add(normalized.amount_cents).ok_or_else(|| {
            HostError::validation("line item amount total exceeds supported range")
        })?;
        if total > MAX_MONEY_CENTS {
            return Err(HostError::validation(
                "line item amount total exceeds supported range",
            ));
        }
        line_items.push(normalized);
    }
    if input.project_discount_cents > total {
        return Err(HostError::validation(
            "projectDiscountCents must not exceed the original total",
        ));
    }
    let quotation_totals = calculate_quotation_totals(&line_items, input.project_discount_cents)?;
    Ok(BusinessProfile {
        project_title: normalize_required("projectTitle", input.project_title, MAX_SHORT_CHARS)?,
        project_code: normalize_text("projectCode", input.project_code, MAX_SHORT_CHARS)?,
        customer_name: normalize_required("customerName", input.customer_name, MAX_SHORT_CHARS)?,
        customer_legal_name: normalize_text(
            "customerLegalName",
            input.customer_legal_name,
            MAX_SHORT_CHARS,
        )?,
        customer_tax_id: normalize_text("customerTaxId", input.customer_tax_id, MAX_SHORT_CHARS)?,
        customer_address: normalize_text(
            "customerAddress",
            input.customer_address,
            MAX_TEXT_CHARS,
        )?,
        customer_contact: normalize_text(
            "customerContact",
            input.customer_contact,
            MAX_SHORT_CHARS,
        )?,
        customer_phone: normalize_text("customerPhone", input.customer_phone, MAX_SHORT_CHARS)?,
        customer_email: normalize_text("customerEmail", input.customer_email, MAX_SHORT_CHARS)?,
        supplier_legal_name: normalize_text(
            "supplierLegalName",
            input.supplier_legal_name,
            MAX_SHORT_CHARS,
        )?,
        supplier_tax_id: normalize_text("supplierTaxId", input.supplier_tax_id, MAX_SHORT_CHARS)?,
        supplier_address: normalize_text(
            "supplierAddress",
            input.supplier_address,
            MAX_TEXT_CHARS,
        )?,
        supplier_contact: normalize_text(
            "supplierContact",
            input.supplier_contact,
            MAX_SHORT_CHARS,
        )?,
        supplier_phone: normalize_text("supplierPhone", input.supplier_phone, MAX_SHORT_CHARS)?,
        supplier_bank_name: normalize_text(
            "supplierBankName",
            input.supplier_bank_name,
            MAX_SHORT_CHARS,
        )?,
        supplier_bank_account: normalize_text(
            "supplierBankAccount",
            input.supplier_bank_account,
            MAX_SHORT_CHARS,
        )?,
        currency,
        default_tax_rate_bps: input.default_tax_rate_bps,
        tax_mode: input.tax_mode,
        project_discount_cents: input.project_discount_cents,
        quotation_totals: Some(quotation_totals),
        service_start_at: input.service_start_at,
        service_end_at: input.service_end_at,
        delivery_summary: normalize_text(
            "deliverySummary",
            input.delivery_summary,
            MAX_TEXT_CHARS,
        )?,
        payment_terms: normalize_text("paymentTerms", input.payment_terms, MAX_TEXT_CHARS)?,
        acceptance_terms: normalize_text(
            "acceptanceTerms",
            input.acceptance_terms,
            MAX_TEXT_CHARS,
        )?,
        notes: normalize_text("notes", input.notes, MAX_TEXT_CHARS)?,
        line_items,
    })
}

fn normalize_line_item(
    id: String,
    input: BusinessLineItemInput,
    tax_mode: BusinessTaxMode,
) -> Result<BusinessLineItem, HostError> {
    if !(1..=MAX_QUANTITY_MILLIS).contains(&input.quantity_millis) {
        return Err(HostError::validation(format!(
            "quantityMillis must be in 1..{MAX_QUANTITY_MILLIS}"
        )));
    }
    if !(0..=MAX_MONEY_CENTS).contains(&input.unit_price_cents) {
        return Err(HostError::validation(
            "unitPriceCents is outside the supported range",
        ));
    }
    if !(0..=10_000).contains(&input.tax_rate_bps) {
        return Err(HostError::validation("taxRateBps must be in 0..10000"));
    }
    let subtotal_numerator = i128::from(input.quantity_millis) * i128::from(input.unit_price_cents);
    let (numerator, denominator) = match tax_mode {
        BusinessTaxMode::TaxExclusive => (
            subtotal_numerator
                .checked_mul(10_000_i128 + i128::from(input.tax_rate_bps))
                .ok_or_else(|| {
                    HostError::validation("computed line item amount exceeds the supported range")
                })?,
            1_000_i128 * 10_000_i128,
        ),
        BusinessTaxMode::TaxInclusive => (subtotal_numerator, 1_000_i128),
    };
    let rounded = (numerator + denominator / 2) / denominator;
    if rounded > i128::from(MAX_MONEY_CENTS) {
        return Err(HostError::validation(
            "computed line item amount exceeds the supported range",
        ));
    }
    Ok(BusinessLineItem {
        id,
        name: normalize_required("line item name", input.name, MAX_SHORT_CHARS)?,
        description: normalize_text("line item description", input.description, MAX_TEXT_CHARS)?,
        quantity_millis: input.quantity_millis,
        unit: normalize_required("line item unit", input.unit, 80)?,
        unit_price_cents: input.unit_price_cents,
        tax_rate_bps: input.tax_rate_bps,
        amount_cents: rounded as i64,
    })
}

fn calculate_quotation_totals(
    items: &[BusinessLineItem],
    project_discount_cents: i64,
) -> Result<BusinessQuotationTotals, HostError> {
    let original_total_cents = items.iter().try_fold(0_i64, |total, item| {
        total
            .checked_add(item.amount_cents)
            .ok_or_else(|| HostError::validation("line item amount total exceeds supported range"))
    })?;
    if project_discount_cents > original_total_cents {
        return Err(HostError::validation(
            "projectDiscountCents must not exceed the original total",
        ));
    }

    let allocations =
        allocate_project_discount(items, project_discount_cents, original_total_cents)?;
    let mut tax_cents = 0_i64;
    for (item, discount) in items.iter().zip(allocations) {
        let discounted_gross = item.amount_cents.checked_sub(discount).ok_or_else(|| {
            HostError::validation("project discount allocation exceeds line total")
        })?;
        let denominator = 10_000_i128 + i128::from(item.tax_rate_bps);
        let line_tax = (i128::from(discounted_gross) * i128::from(item.tax_rate_bps)
            + denominator / 2)
            / denominator;
        let line_tax = i64::try_from(line_tax)
            .map_err(|_| HostError::validation("computed tax exceeds supported range"))?;
        tax_cents = tax_cents
            .checked_add(line_tax)
            .ok_or_else(|| HostError::validation("computed tax exceeds supported range"))?;
    }
    let final_total_cents = original_total_cents
        .checked_sub(project_discount_cents)
        .ok_or_else(|| HostError::validation("computed final total is invalid"))?;
    let tax_exclusive_total_cents = final_total_cents
        .checked_sub(tax_cents)
        .ok_or_else(|| HostError::validation("computed tax-exclusive total is invalid"))?;
    Ok(BusinessQuotationTotals {
        original_total_cents,
        project_discount_cents,
        tax_exclusive_total_cents,
        tax_cents,
        final_total_cents,
    })
}

fn allocate_project_discount(
    items: &[BusinessLineItem],
    discount_cents: i64,
    original_total_cents: i64,
) -> Result<Vec<i64>, HostError> {
    if discount_cents == 0 {
        return Ok(vec![0; items.len()]);
    }
    if original_total_cents <= 0 {
        return Err(HostError::validation(
            "projectDiscountCents requires a positive original total",
        ));
    }
    let denominator = i128::from(original_total_cents);
    let mut allocations = Vec::with_capacity(items.len());
    let mut remainders = Vec::with_capacity(items.len());
    let mut allocated = 0_i64;
    for (index, item) in items.iter().enumerate() {
        let numerator = i128::from(discount_cents) * i128::from(item.amount_cents);
        let base = i64::try_from(numerator / denominator)
            .map_err(|_| HostError::validation("project discount allocation overflow"))?;
        allocations.push(base);
        allocated = allocated
            .checked_add(base)
            .ok_or_else(|| HostError::validation("project discount allocation overflow"))?;
        remainders.push((index, numerator % denominator));
    }
    remainders.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let remaining = discount_cents
        .checked_sub(allocated)
        .ok_or_else(|| HostError::validation("project discount allocation overflow"))?;
    for (index, _) in remainders.into_iter().take(remaining as usize) {
        allocations[index] = allocations[index]
            .checked_add(1)
            .ok_or_else(|| HostError::validation("project discount allocation overflow"))?;
    }
    Ok(allocations)
}

fn ensure_document_prerequisites(
    workspace: &BusinessWorkspaceRecord,
    kind: &BusinessDocumentKind,
) -> Result<(), HostError> {
    match kind {
        BusinessDocumentKind::Quote => Ok(()),
        BusinessDocumentKind::Contract => {
            if workspace.current_documents.quote_document_id.is_some() {
                Ok(())
            } else {
                Err(HostError::new(
                    "BUSINESS_QUOTE_REQUIRED",
                    "contract workflow requires a generated quote",
                    false,
                ))
            }
        }
        BusinessDocumentKind::PaymentRequest | BusinessDocumentKind::Acceptance => {
            if workspace.current_documents.contract_document_id.is_some() {
                Ok(())
            } else {
                Err(HostError::new(
                    "BUSINESS_EFFECTIVE_CONTRACT_REQUIRED",
                    "payment request and acceptance workflows require an effective contract",
                    false,
                ))
            }
        }
    }
}

fn ensure_current_quote_confirmed(
    connection: &Connection,
    vault_root: &Path,
    workspace: &BusinessWorkspaceRecord,
) -> Result<(), HostError> {
    let quote_id = workspace
        .current_documents
        .quote_document_id
        .as_deref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_QUOTE_REQUIRED",
                "contract workflow requires a generated quote",
                false,
            )
        })?;
    let quote = workspace
        .documents
        .iter()
        .find(|document| document.id == quote_id)
        .ok_or_else(|| HostError::internal("current quote document is missing"))?;
    let quote_asset_id = quote.output_asset_id.as_deref().ok_or_else(|| {
        HostError::new(
            "BUSINESS_QUOTE_ASSET_REQUIRED",
            "generated quote is missing its authoritative output asset",
            false,
        )
    })?;
    let quote_asset = verified_project_asset(
        connection,
        vault_root,
        quote_asset_id,
        &workspace.project_id,
    )?;
    let confirmed = workspace.quote_confirmations.iter().any(|confirmation| {
        confirmation.quote_document_id == quote.id
            && confirmation.quote_document_revision == quote.revision
            && confirmation.quote_asset_id == quote_asset.id
            && confirmation.quote_sha256 == quote_asset.sha256
    });
    if confirmed {
        Ok(())
    } else {
        Err(HostError::new(
            "BUSINESS_QUOTE_CONFIRMATION_REQUIRED",
            "contract workflow requires customer confirmation of the current generated quote",
            false,
        ))
    }
}

fn ensure_positive_document_total(document: &BusinessDocumentRecord) -> Result<(), HostError> {
    if matches!(
        document.kind,
        BusinessDocumentKind::Quote | BusinessDocumentKind::Contract
    ) && document_total_cents(document) <= 0
    {
        return Err(HostError::new(
            "BUSINESS_DOCUMENT_AMOUNT_REQUIRED",
            "quote and contract totals must be greater than zero",
            false,
        ));
    }
    Ok(())
}

fn ensure_contract_covers_payments(
    workspace: &BusinessWorkspaceRecord,
    contract: &BusinessDocumentRecord,
) -> Result<(), HostError> {
    let contract_cents = document_total_cents(contract);
    let scheduled_cents = workspace
        .payments
        .iter()
        .filter(|payment| payment.status != BusinessPaymentStatus::Canceled)
        .try_fold(0_i64, |total, payment| {
            total.checked_add(payment.amount_cents)
        })
        .ok_or_else(|| HostError::validation("payment total exceeds supported range"))?;
    if scheduled_cents > contract_cents {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_EXCEEDS_CONTRACT",
            "non-canceled payment total cannot exceed the effective contract amount",
            false,
        ));
    }
    Ok(())
}

fn ensure_payment_request_target(
    workspace: &BusinessWorkspaceRecord,
    document: &BusinessDocumentRecord,
) -> Result<(), HostError> {
    if document.kind != BusinessDocumentKind::PaymentRequest {
        return Ok(());
    }
    let payment_id = document
        .snapshot
        .payment
        .as_ref()
        .map(|payment| payment.id.as_str())
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_PAYMENT_REQUIRED",
                "payment request requires a linked payment schedule",
                false,
            )
        })?;
    let payment = workspace
        .payments
        .iter()
        .find(|payment| payment.id == payment_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_PAYMENT_NOT_FOUND",
                "linked payment no longer exists in this business workspace",
                false,
            )
        })?;
    if matches!(
        payment.status,
        BusinessPaymentStatus::Planned
            | BusinessPaymentStatus::Requested
            | BusinessPaymentStatus::PartiallyReceived
    ) {
        // Keep this in sync with create_document: a draft that could be
        // created must remain approvable/generatable, otherwise it can only
        // be voided and the payment chain dead-ends.
        Ok(())
    } else {
        Err(HostError::new(
            "BUSINESS_PAYMENT_STATUS_INVALID",
            "payment request requires a planned, requested or partially received payment",
            false,
        ))
    }
}
fn create_acceptance_batch(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    payload: &CreateBusinessAcceptanceBatchPayload,
    expected_revision: i64,
    project_id: &str,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, project_id)?;
    if workspace.acceptance_batches.len() as i64 >= MAX_ACCEPTANCE_BATCHES_PER_WORKSPACE {
        return Err(HostError::new(
            "BUSINESS_ACCEPTANCE_BATCH_LIMIT_REACHED",
            "workspace acceptance batch limit has been reached",
            false,
        ));
    }
    for output in &payload.output_specs {
        let Some(template_asset_id) = output.template_asset_id.as_deref() else {
            continue;
        };
        let (asset, _) = asset_service::verify_ready_asset_integrity(
            transaction,
            vault_root,
            template_asset_id,
        )?;
        if asset.project_id.as_deref() != Some(workspace.project_id.as_str()) {
            return Err(HostError::new(
                "BUSINESS_TEMPLATE_ASSET_PROJECT_MISMATCH",
                "template asset belongs to a different project",
                false,
            ));
        }
        if asset.kind != crate::protocol::AssetKind::Document {
            return Err(HostError::new(
                "BUSINESS_TEMPLATE_ASSET_KIND_INVALID",
                "template asset must be a document",
                false,
            ));
        }
        if !output
            .template_source_sha256
            .as_deref()
            .is_some_and(|sha256| sha256.eq_ignore_ascii_case(&asset.sha256))
        {
            return Err(HostError::new(
                "BUSINESS_TEMPLATE_ASSET_HASH_MISMATCH",
                "template asset SHA-256 does not match templateSourceSha256",
                false,
            ));
        }
        let expected_extension = match output.format {
            BusinessDocumentFormat::Docx => "docx",
            BusinessDocumentFormat::Xlsx => "xlsx",
        };
        if !asset
            .original_name
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case(expected_extension))
        {
            return Err(HostError::new(
                "BUSINESS_TEMPLATE_ASSET_FORMAT_MISMATCH",
                format!("template asset must use .{expected_extension}"),
                false,
            ));
        }
        if output.template_key
            == document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY
        {
            ensure_approved_template_binding(
                transaction,
                &workspace.id,
                template_asset_id,
                output
                    .template_source_sha256
                    .as_deref()
                    .expect("source-backed template SHA normalized"),
                &output.template_key,
                &output.template_mapping_version,
            )?;
        }
    }
    let requirements = payload
        .requirements
        .iter()
        .map(|requirement| BusinessAcceptanceRequirementRecord {
            id: requirement
                .id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            label: requirement.label.clone(),
            kind: requirement.kind.clone(),
            required_group_count: requirement.required_group_count,
        })
        .collect::<Vec<_>>();
    let output_specs = payload
        .output_specs
        .iter()
        .map(|output| {
            let payment_application = output
                .payment_application
                .as_ref()
                .map(|data| freeze_payment_application_data(&workspace, data))
                .transpose()?;
            Ok(BusinessAcceptanceOutputSpecRecord {
                id: output
                    .id
                    .clone()
                    .unwrap_or_else(|| Uuid::new_v4().to_string()),
                output_code: output.output_code.clone(),
                document_number: output.document_number.clone(),
                title: output.title.clone(),
                template_key: output.template_key.clone(),
                template_asset_id: output.template_asset_id.clone(),
                template_source_sha256: output.template_source_sha256.clone(),
                template_mapping_version: output.template_mapping_version.clone(),
                contract_settlement: output.contract_settlement.clone(),
                service_settlement_items: output.service_settlement_items.clone(),
                payment_application,
                video_completion_acceptance: output.video_completion_acceptance.clone(),
                production_result_confirmation: output.production_result_confirmation.clone(),
                format: output.format.clone(),
                requirement_ids: output.requirement_ids.clone(),
            })
        })
        .collect::<Result<Vec<_>, HostError>>()?;
    let now = now_millis();
    transaction
        .execute(
            "INSERT INTO business_acceptance_batches
             (id, workspace_id, label, requirements_json, output_specs_json,
              revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
            params![
                Uuid::new_v4().to_string(),
                workspace.id,
                payload.label,
                serde_json::to_string(&requirements).map_err(json_error)?,
                serde_json::to_string(&output_specs).map_err(json_error)?,
                now,
            ],
        )
        .map_err(sql_error)?;
    bump_workspace(transaction, &workspace.id, expected_revision, now)?;
    load_workspace(transaction, &workspace.id)
}

fn ensure_approved_template_binding(
    connection: &Connection,
    workspace_id: &str,
    normalized_asset_id: &str,
    normalized_sha256: &str,
    template_key: &str,
    mapping_version: &str,
) -> Result<(), HostError> {
    let source = asset_service::get_asset_source(connection, normalized_asset_id)?;
    if source.source != asset_service::AssetSourceKind::NormalizedTemplate {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_VERSION_APPROVAL_REQUIRED",
            "payment template must use a normalizedTemplate Asset",
            false,
        ));
    }
    let approved: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM business_template_versions
                 WHERE workspace_id = ?1 AND normalized_asset_id = ?2
                   AND normalized_sha256 = ?3 AND template_key = ?4
                   AND mapping_version = ?5 AND status = 'approved'
             )",
            params![
                workspace_id,
                normalized_asset_id,
                normalized_sha256,
                template_key,
                mapping_version,
            ],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    if !approved {
        return Err(HostError::new(
            "BUSINESS_TEMPLATE_VERSION_APPROVAL_REQUIRED",
            "payment template must reference an approved normalized version",
            false,
        ));
    }
    Ok(())
}

fn freeze_payment_application_data(
    workspace: &BusinessWorkspaceRecord,
    input: &BusinessPaymentApplicationInput,
) -> Result<BusinessPaymentApplicationData, HostError> {
    if workspace.profile.supplier_legal_name.trim().is_empty()
        || workspace.profile.supplier_bank_name.trim().is_empty()
        || workspace.profile.supplier_bank_account.trim().is_empty()
    {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_BANK_ACCOUNT_REQUIRED",
            "payment application requires confirmed supplier and bank account data",
            false,
        ));
    }
    let payment = workspace
        .payments
        .iter()
        .find(|payment| payment.id == input.payment_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_PAYMENT_NOT_FOUND",
                "payment application references a missing payment plan",
                false,
            )
        })?;
    if matches!(
        payment.status,
        BusinessPaymentStatus::Canceled | BusinessPaymentStatus::Received
    ) {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_STATUS_INVALID",
            "payment application requires an unsettled payment plan",
            false,
        ));
    }
    let settlement_total_cents = input
        .settlement_items
        .iter()
        .try_fold(0_i64, |total, item| {
            total
                .checked_add(payment_settlement_item_amount(item, true)?)
                .ok_or_else(|| HostError::validation("payment settlement total overflowed"))
        })?;
    if settlement_total_cents <= 0 || settlement_total_cents > MAX_MONEY_CENTS {
        return Err(HostError::validation(
            "payment settlement total is outside the supported range",
        ));
    }
    let cumulative_paid_cents = workspace.financial_summary.received_cents;
    let remaining_payable_cents = settlement_total_cents
        .checked_sub(cumulative_paid_cents)
        .ok_or_else(|| HostError::validation("remaining payable calculation overflowed"))?;
    if remaining_payable_cents <= 0 {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_REMAINING_PAYABLE_INVALID",
            "settlement total must exceed cumulative received funds",
            false,
        ));
    }
    let current_payment_received = receipt_net_for_payment(&workspace.receipts, &payment.id)?;
    let payment_outstanding = payment
        .amount_cents
        .checked_sub(current_payment_received)
        .ok_or_else(|| HostError::validation("payment outstanding calculation overflowed"))?;
    if payment_outstanding != remaining_payable_cents {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_REMAINING_PAYABLE_MISMATCH",
            "payment outstanding must equal settlement total minus cumulative paid",
            false,
        ));
    }
    let (has_invoice_records, recorded_invoice_cents) =
        invoice_ledger_for_payment(workspace, &payment.id)?;
    if has_invoice_records && recorded_invoice_cents != input.invoice_amount_cents {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_INVOICE_AMOUNT_MISMATCH",
            "payment application invoice amount does not match the invoice ledger",
            false,
        ));
    }
    Ok(BusinessPaymentApplicationData {
        payment_id: input.payment_id.clone(),
        contract_title: input.contract_title.clone(),
        contract_number: input.contract_number.clone(),
        work_summary: input.work_summary.clone(),
        payment_period_start: input.payment_period_start.clone(),
        payment_period_end: input.payment_period_end.clone(),
        settlement_period: input.settlement_period.clone(),
        payment_sequence: input.payment_sequence,
        invoice_amount_cents: input.invoice_amount_cents,
        cumulative_recognized_amount_cents: input.cumulative_recognized_amount_cents,
        withheld_amount_cents: input.withheld_amount_cents,
        cumulative_paid_cents,
        settlement_total_cents,
        remaining_payable_cents,
        application_date: input.application_date.clone(),
        bank_account_profile_version: payment_bank_account_profile_version(
            &workspace.profile,
            &input.supplier_bank_routing_number,
        ),
        supplier_bank_routing_number: input.supplier_bank_routing_number.clone(),
        settlement_items: input.settlement_items.clone(),
    })
}

fn payment_bank_account_profile_version(
    profile: &BusinessProfile,
    frozen_routing_number: &str,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"bsaigc.payment-bank-account-profile.v1\0");
    for value in [
        profile.supplier_legal_name.as_str(),
        profile.supplier_bank_name.as_str(),
        profile.supplier_bank_account.as_str(),
        frozen_routing_number,
    ] {
        let bytes = value.as_bytes();
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    format!("payment-bank-account-sha256:{:X}", digest.finalize())
}

fn invoice_ledger_for_payment(
    workspace: &BusinessWorkspaceRecord,
    payment_id: &str,
) -> Result<(bool, i64), HostError> {
    workspace
        .invoices
        .iter()
        .filter(|invoice| invoice.payment_id.as_deref() == Some(payment_id))
        .try_fold((false, 0_i64), |(_, total), invoice| {
            let signed = match invoice.kind {
                BusinessInvoiceKind::Issued => invoice.amount_cents,
                BusinessInvoiceKind::Reversal => invoice
                    .amount_cents
                    .checked_neg()
                    .ok_or_else(|| HostError::validation("invoice reversal amount overflowed"))?,
            };
            total
                .checked_add(signed)
                .map(|net| (true, net))
                .ok_or_else(|| HostError::validation("invoice total overflowed"))
        })
}

fn prepare_acceptance_documents(
    transaction: &Transaction<'_>,
    payload: &PrepareBusinessAcceptanceDocumentsPayload,
    expected_revision: i64,
    project_id: &str,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, project_id)?;
    ensure_document_prerequisites(&workspace, &BusinessDocumentKind::Acceptance)?;
    let batch = workspace
        .acceptance_batches
        .iter()
        .find(|batch| batch.id == payload.batch_id)
        .cloned()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_BATCH_NOT_FOUND",
                "acceptance batch does not exist in this workspace",
                false,
            )
        })?;
    let bindings =
        load_acceptance_material_bindings(transaction, &workspace.project_id, &batch.id)?;
    let missing_count = batch
        .output_specs
        .iter()
        .filter(|spec| {
            acceptance_document_for_spec(&workspace.documents, &batch.id, spec).is_none()
        })
        .count() as i64;
    if workspace.documents.len() as i64 + missing_count > MAX_DOCUMENTS_PER_WORKSPACE {
        return Err(HostError::new(
            "BUSINESS_DOCUMENT_LIMIT_REACHED",
            "workspace document limit has been reached",
            false,
        ));
    }

    let now = now_millis();
    let mut sequence_number = workspace
        .documents
        .iter()
        .filter(|document| document.kind == BusinessDocumentKind::Acceptance)
        .map(|document| document.sequence_number)
        .max()
        .unwrap_or(0);
    for spec in &batch.output_specs {
        if let Some(document) = acceptance_document_for_spec(&workspace.documents, &batch.id, spec)
        {
            if matches!(
                document.status,
                BusinessDocumentStatus::Draft | BusinessDocumentStatus::InReview
            ) {
                let mut snapshot = document.snapshot.clone();
                refresh_acceptance_snapshot(&mut snapshot, &workspace, &batch, spec, &bindings);
                transaction
                    .execute(
                        "UPDATE business_documents
                         SET acceptance_output_spec_id = ?1, snapshot_json = ?2,
                             revision = revision + 1, updated_at = ?3
                         WHERE id = ?4 AND workspace_id = ?5 AND revision = ?6
                           AND status IN ('draft','inReview')",
                        params![
                            spec.id,
                            serde_json::to_string(&snapshot).map_err(json_error)?,
                            now,
                            document.id,
                            workspace.id,
                            document.revision,
                        ],
                    )
                    .map_err(map_document_insert_error)?;
            }
            continue;
        }

        sequence_number += 1;
        let mut snapshot = BusinessDocumentSnapshot {
            workspace_revision: workspace.revision,
            acceptance_batch_id: Some(batch.id.clone()),
            acceptance_output_spec_id: Some(spec.id.clone()),
            acceptance_batch_revision: Some(batch.revision),
            material_bindings: Vec::new(),
            template_asset_id: spec.template_asset_id.clone(),
            template_source_sha256: spec.template_source_sha256.clone(),
            template_mapping_version: spec.template_mapping_version.clone(),
            contract_settlement: spec.contract_settlement.clone(),
            service_settlement_items: spec.service_settlement_items.clone(),
            payment_application: spec.payment_application.clone(),
            video_completion_acceptance: spec.video_completion_acceptance.clone(),
            production_result_confirmation: spec.production_result_confirmation.clone(),
            customer_id: workspace.customer_id.clone(),
            customer: workspace.customer.clone(),
            profile: workspace.profile.clone(),
            payment: None,
        };
        refresh_acceptance_snapshot(&mut snapshot, &workspace, &batch, spec, &bindings);
        transaction
            .execute(
                "INSERT INTO business_documents
                 (id, workspace_id, kind, sequence_number, document_number, title,
                  template_key, status, snapshot_json, output_asset_id, output_format,
                  acceptance_batch_id, acceptance_output_spec_id, approved_at, approved_by,
                  generated_at, revision, created_at, updated_at)
                 VALUES (?1, ?2, 'acceptance', ?3, ?4, ?5, ?6, 'draft', ?7,
                         NULL, NULL, ?8, ?9, NULL, NULL, NULL, 1, ?10, ?10)",
                params![
                    Uuid::new_v4().to_string(),
                    workspace.id,
                    sequence_number,
                    spec.document_number,
                    spec.title,
                    spec.template_key,
                    serde_json::to_string(&snapshot).map_err(json_error)?,
                    batch.id,
                    spec.id,
                    now,
                ],
            )
            .map_err(map_document_insert_error)?;
    }
    bump_workspace(transaction, &workspace.id, expected_revision, now)?;
    load_workspace(transaction, &workspace.id)
}

fn acceptance_document_for_spec<'a>(
    documents: &'a [BusinessDocumentRecord],
    batch_id: &str,
    spec: &BusinessAcceptanceOutputSpecRecord,
) -> Option<&'a BusinessDocumentRecord> {
    documents.iter().find(|document| {
        document.kind == BusinessDocumentKind::Acceptance
            && document.snapshot.acceptance_batch_id.as_deref() == Some(batch_id)
            && (document.snapshot.acceptance_output_spec_id.as_deref() == Some(spec.id.as_str())
                || (document.document_number == spec.document_number
                    && document.title == spec.title
                    && document.template_key == spec.template_key))
    })
}

fn refresh_acceptance_snapshot(
    snapshot: &mut BusinessDocumentSnapshot,
    workspace: &BusinessWorkspaceRecord,
    batch: &BusinessAcceptanceBatchRecord,
    spec: &BusinessAcceptanceOutputSpecRecord,
    bindings: &[BusinessAcceptanceMaterialBinding],
) {
    snapshot.acceptance_batch_id = Some(batch.id.clone());
    snapshot.acceptance_output_spec_id = Some(spec.id.clone());
    snapshot.acceptance_batch_revision = Some(batch.revision);
    snapshot.template_asset_id = spec.template_asset_id.clone();
    snapshot.template_source_sha256 = spec.template_source_sha256.clone();
    snapshot.template_mapping_version = spec.template_mapping_version.clone();
    snapshot.contract_settlement = spec.contract_settlement.clone();
    snapshot.service_settlement_items = spec.service_settlement_items.clone();
    snapshot.payment_application = spec.payment_application.clone();
    snapshot.video_completion_acceptance = spec.video_completion_acceptance.clone();
    snapshot.production_result_confirmation = spec.production_result_confirmation.clone();
    snapshot.payment = spec.payment_application.as_ref().and_then(|data| {
        workspace
            .payments
            .iter()
            .find(|payment| payment.id == data.payment_id)
            .cloned()
    });
    snapshot.material_bindings = bindings
        .iter()
        .filter(|binding| {
            spec.requirement_ids.is_empty()
                || spec.requirement_ids.contains(&binding.requirement_id)
        })
        .cloned()
        .collect();
}

fn load_acceptance_material_bindings(
    connection: &Connection,
    project_id: &str,
    batch_id: &str,
) -> Result<Vec<BusinessAcceptanceMaterialBinding>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT material.requirement_id, material.asset_id, asset.sha256,
                    material.group_key, material.kind
             FROM business_acceptance_materials material
             JOIN assets asset ON asset.id = material.asset_id
             WHERE material.batch_id = ?1
               AND material.confirmed = 1
               AND material.duplicate_of_material_id IS NULL
               AND asset.status = 'ready'
               AND asset.project_id = ?2
             ORDER BY material.requirement_id ASC, material.group_key ASC,
                      material.asset_id ASC",
        )
        .map_err(sql_error)?;
    let bindings = statement
        .query_map(params![batch_id, project_id], |row| {
            let kind: String = row.get(4)?;
            Ok(BusinessAcceptanceMaterialBinding {
                requirement_id: row.get(0)?,
                asset_id: row.get(1)?,
                sha256: row.get(2)?,
                group_key: row.get(3)?,
                kind: acceptance_material_kind_from_db(&kind)?,
            })
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(bindings)
}

fn upsert_acceptance_material(
    transaction: &Transaction<'_>,
    payload: &UpsertBusinessAcceptanceMaterialPayload,
    expected_revision: i64,
    project_id: &str,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, project_id)?;
    let batch = workspace
        .acceptance_batches
        .iter()
        .find(|batch| batch.id == payload.batch_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_BATCH_NOT_FOUND",
                "acceptance batch does not exist in this workspace",
                false,
            )
        })?;
    let requirement = batch
        .requirements
        .iter()
        .find(|requirement| requirement.id == payload.material.requirement_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_REQUIREMENT_NOT_FOUND",
                "acceptance requirement does not exist in this batch",
                false,
            )
        })?;
    if requirement.kind != payload.material.kind {
        return Err(HostError::new(
            "BUSINESS_ACCEPTANCE_MATERIAL_KIND_MISMATCH",
            "acceptance material kind must match its requirement",
            false,
        ));
    }
    let (asset_project_id, asset_status, asset_kind) = transaction
        .query_row(
            "SELECT project_id, status, kind FROM assets WHERE id = ?1",
            [&payload.material.asset_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::new("ASSET_NOT_FOUND", "acceptance asset is missing", false))?;
    if asset_status != "ready" {
        return Err(HostError::new(
            "BUSINESS_ACCEPTANCE_ASSET_NOT_READY",
            "acceptance material asset must be ready",
            false,
        ));
    }
    if asset_project_id.as_deref() != Some(workspace.project_id.as_str()) {
        return Err(HostError::new(
            "BUSINESS_ACCEPTANCE_ASSET_PROJECT_MISMATCH",
            "acceptance material asset belongs to a different project",
            false,
        ));
    }
    if !acceptance_asset_kind_matches_requirement(&asset_kind, &requirement.kind) {
        return Err(HostError::new(
            "BUSINESS_ACCEPTANCE_ASSET_KIND_MISMATCH",
            "acceptance asset kind is incompatible with its requirement",
            false,
        ));
    }
    let material_id = payload
        .material
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    if payload.material.duplicate_of_material_id.as_deref() == Some(material_id.as_str()) {
        return Err(HostError::validation(
            "acceptance material cannot duplicate itself",
        ));
    }
    if let Some(duplicate_id) = payload.material.duplicate_of_material_id.as_deref() {
        let valid_duplicate = batch.materials.iter().any(|material| {
            material.id == duplicate_id
                && material.requirement_id == payload.material.requirement_id
        });
        if !valid_duplicate {
            return Err(HostError::new(
                "BUSINESS_ACCEPTANCE_DUPLICATE_TARGET_INVALID",
                "duplicate material must reference the same batch requirement",
                false,
            ));
        }
    }
    let existing = batch
        .materials
        .iter()
        .find(|material| material.id == material_id);
    if existing.is_none() && batch.materials.len() as i64 >= MAX_ACCEPTANCE_MATERIALS_PER_BATCH {
        return Err(HostError::new(
            "BUSINESS_ACCEPTANCE_MATERIAL_LIMIT_REACHED",
            "acceptance material limit has been reached",
            false,
        ));
    }
    if existing.is_some_and(|material| material.batch_id != batch.id) {
        return Err(HostError::new(
            "BUSINESS_ACCEPTANCE_MATERIAL_BATCH_MISMATCH",
            "acceptance material belongs to a different batch",
            false,
        ));
    }
    let now = now_millis();
    let changed = transaction
        .execute(
            "INSERT INTO business_acceptance_materials
             (id, workspace_id, batch_id, requirement_id, asset_id, kind, group_key,
              confirmed, duplicate_of_material_id, notes, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?11)
             ON CONFLICT(id) DO UPDATE SET
                 requirement_id = excluded.requirement_id,
                 asset_id = excluded.asset_id,
                 kind = excluded.kind,
                 group_key = excluded.group_key,
                 confirmed = excluded.confirmed,
                 duplicate_of_material_id = excluded.duplicate_of_material_id,
                 notes = excluded.notes,
                 revision = business_acceptance_materials.revision + 1,
                 updated_at = excluded.updated_at
             WHERE business_acceptance_materials.workspace_id = excluded.workspace_id
               AND business_acceptance_materials.batch_id = excluded.batch_id",
            params![
                material_id,
                workspace.id,
                batch.id,
                payload.material.requirement_id,
                payload.material.asset_id,
                acceptance_material_kind_to_db(&payload.material.kind),
                payload.material.group_key,
                payload.material.confirmed,
                payload.material.duplicate_of_material_id,
                payload.material.notes,
                now,
            ],
        )
        .map_err(map_acceptance_material_write_error)?;
    ensure_changed(changed)?;
    transaction
        .execute(
            "UPDATE business_acceptance_batches
             SET revision = revision + 1, updated_at = ?1
             WHERE id = ?2 AND workspace_id = ?3",
            params![now, batch.id, workspace.id],
        )
        .map_err(sql_error)?;
    synchronize_acceptance_draft_snapshots(
        transaction,
        &workspace,
        batch,
        batch.revision + 1,
        now,
    )?;
    bump_workspace(transaction, &workspace.id, expected_revision, now)?;
    load_workspace(transaction, &workspace.id)
}

fn synchronize_acceptance_draft_snapshots(
    transaction: &Transaction<'_>,
    workspace: &BusinessWorkspaceRecord,
    batch: &BusinessAcceptanceBatchRecord,
    batch_revision: i64,
    now: i64,
) -> Result<(), HostError> {
    let mut refreshed_batch = batch.clone();
    refreshed_batch.revision = batch_revision;
    let bindings =
        load_acceptance_material_bindings(transaction, &workspace.project_id, &batch.id)?;
    for document in workspace.documents.iter().filter(|document| {
        document.snapshot.acceptance_batch_id.as_deref() == Some(batch.id.as_str())
            && matches!(
                document.status,
                BusinessDocumentStatus::Draft | BusinessDocumentStatus::InReview
            )
    }) {
        let Some(spec) = batch.output_specs.iter().find(|spec| {
            document.snapshot.acceptance_output_spec_id.as_deref() == Some(spec.id.as_str())
                || (document.document_number == spec.document_number
                    && document.title == spec.title
                    && document.template_key == spec.template_key)
        }) else {
            continue;
        };
        let mut snapshot = document.snapshot.clone();
        refresh_acceptance_snapshot(&mut snapshot, workspace, &refreshed_batch, spec, &bindings);
        let changed = transaction
            .execute(
                "UPDATE business_documents
                 SET acceptance_output_spec_id = ?1, snapshot_json = ?2,
                     revision = revision + 1, updated_at = ?3
                 WHERE id = ?4 AND workspace_id = ?5 AND revision = ?6
                   AND status IN ('draft','inReview')",
                params![
                    spec.id,
                    serde_json::to_string(&snapshot).map_err(json_error)?,
                    now,
                    document.id,
                    workspace.id,
                    document.revision,
                ],
            )
            .map_err(sql_error)?;
        ensure_changed(changed)?;
    }
    Ok(())
}

fn ensure_acceptance_ready(
    workspace: &BusinessWorkspaceRecord,
    acceptance_batch_id: &str,
) -> Result<(), HostError> {
    let batch = workspace
        .acceptance_batches
        .iter()
        .find(|batch| batch.id == acceptance_batch_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_BATCH_NOT_FOUND",
                "acceptance batch does not exist in this workspace",
                false,
            )
        })?;
    if batch.readiness.is_ready {
        return Ok(());
    }
    let missing = batch
        .readiness
        .blockers
        .iter()
        .map(|blocker| {
            format!(
                "{}: required {}, provided {}, missing {}",
                blocker.requirement_label,
                blocker.required_group_count,
                blocker.provided_group_count,
                blocker.missing_group_count
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(HostError::new(
        "BUSINESS_ACCEPTANCE_NOT_READY",
        format!("acceptance batch is not ready for official documents: {missing}"),
        false,
    ))
}

fn create_document(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    payload: &CreateBusinessDocumentPayload,
    expected_revision: i64,
    project_id: &str,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, project_id)?;
    ensure_document_prerequisites(&workspace, &payload.kind)?;
    let acceptance_link = match (&payload.kind, payload.acceptance_batch_id.as_deref()) {
        (BusinessDocumentKind::Acceptance, Some(batch_id)) => {
            let batch = workspace
                .acceptance_batches
                .iter()
                .find(|batch| batch.id == batch_id)
                .ok_or_else(|| {
                    HostError::new(
                        "BUSINESS_ACCEPTANCE_BATCH_NOT_FOUND",
                        "acceptance batch does not exist in this workspace",
                        false,
                    )
                })?;
            let output_spec = batch.output_specs.iter().find(|spec| {
                spec.document_number == payload.document_number
                    && spec.title == payload.title
                    && spec.template_key == payload.template_key
            });
            let Some(output_spec) = output_spec else {
                return Err(HostError::new(
                    "BUSINESS_ACCEPTANCE_OUTPUT_SPEC_MISMATCH",
                    "acceptance document must match a configured output spec",
                    false,
                ));
            };
            let bindings =
                load_acceptance_material_bindings(transaction, &workspace.project_id, &batch.id)?;
            Some((batch.clone(), output_spec.clone(), bindings))
        }
        (BusinessDocumentKind::Acceptance, None) => None,
        (_, Some(_)) => {
            return Err(HostError::new(
                "BUSINESS_ACCEPTANCE_BATCH_NOT_ALLOWED",
                "acceptanceBatchId is only valid for acceptance documents",
                false,
            ));
        }
        (_, None) => None,
    };
    if payload.kind == BusinessDocumentKind::Contract {
        ensure_current_quote_confirmed(transaction, vault_root, &workspace)?;
    }
    if workspace.documents.len() as i64 >= MAX_DOCUMENTS_PER_WORKSPACE {
        return Err(HostError::new(
            "BUSINESS_DOCUMENT_LIMIT_REACHED",
            "workspace document limit has been reached",
            false,
        ));
    }
    let sequence_number = workspace
        .documents
        .iter()
        .filter(|document| document.kind == payload.kind)
        .map(|document| document.sequence_number)
        .max()
        .unwrap_or(0)
        + 1;
    let payment = match &payload.payment_id {
        Some(payment_id) => {
            let payment = workspace
                .payments
                .iter()
                .find(|payment| payment.id == *payment_id)
                .cloned()
                .ok_or_else(|| {
                    HostError::new(
                        "BUSINESS_PAYMENT_NOT_FOUND",
                        "payment does not exist in this business workspace",
                        false,
                    )
                })?;
            if !matches!(
                payment.status,
                BusinessPaymentStatus::Planned
                    | BusinessPaymentStatus::Requested
                    | BusinessPaymentStatus::PartiallyReceived
            ) {
                return Err(HostError::new(
                    "BUSINESS_PAYMENT_STATUS_INVALID",
                    "paymentRequest requires a planned or requested payment",
                    false,
                ));
            }
            Some(payment)
        }
        None => None,
    };
    let now = now_millis();
    let mut snapshot = BusinessDocumentSnapshot {
        workspace_revision: workspace.revision,
        template_asset_id: None,
        template_source_sha256: None,
        template_mapping_version: String::new(),
        contract_settlement: None,
        service_settlement_items: Vec::new(),
        payment_application: None,
        video_completion_acceptance: None,
        production_result_confirmation: None,
        customer_id: workspace.customer_id.clone(),
        customer: workspace.customer.clone(),
        profile: workspace.profile.clone(),
        payment,
        acceptance_batch_id: payload.acceptance_batch_id.clone(),
        acceptance_output_spec_id: None,
        acceptance_batch_revision: None,
        material_bindings: Vec::new(),
    };
    if let Some((batch, output_spec, bindings)) = &acceptance_link {
        refresh_acceptance_snapshot(&mut snapshot, &workspace, batch, output_spec, bindings);
    }
    let document = BusinessDocumentRecord {
        id: Uuid::new_v4().to_string(),
        kind: payload.kind.clone(),
        sequence_number,
        document_number: payload.document_number.clone(),
        title: payload.title.clone(),
        template_key: payload.template_key.clone(),
        status: BusinessDocumentStatus::Draft,
        snapshot,
        output_asset_id: None,
        output_format: None,
        source_asset_id: None,
        review_id: None,
        report_asset_id: None,
        evidence: None,
        manual_waiver: None,
        voided_at: None,
        voided_by: None,
        void_reason: String::new(),
        approved_at: None,
        approved_by: None,
        generated_at: None,
        revision: 1,
        created_at: now,
        updated_at: now,
    };
    transaction
        .execute(
            "INSERT INTO business_documents
             (id, workspace_id, kind, sequence_number, document_number, title,
              template_key, status, snapshot_json, output_asset_id, output_format,
               acceptance_batch_id, acceptance_output_spec_id, approved_at, approved_by,
               generated_at, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'draft', ?8,
                     NULL, NULL, ?9, ?10, NULL, NULL, NULL, 1, ?11, ?11)",
            params![
                document.id,
                workspace.id,
                document_kind_to_db(&document.kind),
                document.sequence_number,
                document.document_number,
                document.title,
                document.template_key,
                serde_json::to_string(&document.snapshot).map_err(json_error)?,
                payload.acceptance_batch_id,
                acceptance_link.as_ref().map(|(_, spec, _)| &spec.id),
                now,
            ],
        )
        .map_err(map_document_insert_error)?;
    bump_workspace(transaction, &workspace.id, expected_revision, now)?;
    load_workspace(transaction, &workspace.id)
}

fn promote_reviewed_contract(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    payload: &PromoteReviewedContractPayload,
    expected_revision: i64,
    context: &NormalizedContext,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, &context.project_id)?;
    let binding = contract_review_service::completed_review_binding(
        transaction,
        &workspace.id,
        &payload.review_id,
        &payload.report_asset_id,
    )?;
    if workspace
        .documents
        .iter()
        .any(|document| document.review_id.as_deref() == Some(binding.review_id.as_str()))
    {
        return Err(HostError::new(
            "BUSINESS_REVIEW_ALREADY_PROMOTED",
            "completed contract review is already bound to a business contract",
            false,
        ));
    }
    if workspace.documents.len() as i64 >= MAX_DOCUMENTS_PER_WORKSPACE {
        return Err(HostError::new(
            "BUSINESS_DOCUMENT_LIMIT_REACHED",
            "workspace document limit has been reached",
            false,
        ));
    }
    let sequence_number = workspace
        .documents
        .iter()
        .filter(|document| document.kind == BusinessDocumentKind::Contract)
        .map(|document| document.sequence_number)
        .max()
        .unwrap_or(0)
        + 1;
    let now = now_millis();
    let (evidence, manual_waiver) = materialize_evidence_or_waiver(
        transaction,
        vault_root,
        &workspace.project_id,
        BusinessEvidenceKind::ContractSignature,
        payload.evidence.as_ref(),
        payload.manual_waiver.as_ref(),
        &context.actor_id,
        now,
    )?;
    let document = BusinessDocumentRecord {
        id: Uuid::new_v4().to_string(),
        kind: BusinessDocumentKind::Contract,
        sequence_number,
        document_number: payload.document_number.clone(),
        title: payload.title.clone(),
        template_key: "reviewed-customer-contract".to_string(),
        status: BusinessDocumentStatus::Effective,
        snapshot: BusinessDocumentSnapshot {
            workspace_revision: workspace.revision,
            acceptance_batch_id: None,
            acceptance_output_spec_id: None,
            acceptance_batch_revision: None,
            material_bindings: Vec::new(),
            template_asset_id: None,
            template_source_sha256: None,
            template_mapping_version: String::new(),
            contract_settlement: None,
            service_settlement_items: Vec::new(),
            payment_application: None,
            video_completion_acceptance: None,
            production_result_confirmation: None,
            customer_id: workspace.customer_id.clone(),
            customer: workspace.customer.clone(),
            profile: workspace.profile.clone(),
            payment: None,
        },
        output_asset_id: None,
        output_format: None,
        source_asset_id: Some(binding.source_asset_id.clone()),
        review_id: Some(binding.review_id.clone()),
        report_asset_id: Some(binding.report_asset_id.clone()),
        evidence: evidence.clone(),
        manual_waiver: manual_waiver.clone(),
        voided_at: None,
        voided_by: None,
        void_reason: String::new(),
        approved_at: Some(now),
        approved_by: Some(context.actor_id.clone()),
        generated_at: None,
        revision: 1,
        created_at: now,
        updated_at: now,
    };
    ensure_positive_document_total(&document)?;
    ensure_contract_covers_payments(&workspace, &document)?;
    transaction
        .execute(
            "INSERT INTO business_documents
             (id, workspace_id, kind, sequence_number, document_number, title,
              template_key, status, snapshot_json, output_asset_id, output_format,
              source_asset_id, review_id, report_asset_id, evidence_json,
              manual_waiver_json, voided_at, voided_by, void_reason,
              approved_at, approved_by, generated_at, revision, created_at, updated_at)
             VALUES (?1, ?2, 'contract', ?3, ?4, ?5, ?6, 'effective', ?7,
                     NULL, NULL, ?8, ?9, ?10, ?11, ?12, NULL, NULL, '',
                     ?13, ?14, NULL, 1, ?13, ?13)",
            params![
                document.id,
                workspace.id,
                document.sequence_number,
                document.document_number,
                document.title,
                document.template_key,
                serde_json::to_string(&document.snapshot).map_err(json_error)?,
                binding.source_asset_id,
                binding.review_id,
                binding.report_asset_id,
                evidence
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(json_error)?,
                manual_waiver
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(json_error)?,
                now,
                context.actor_id,
            ],
        )
        .map_err(map_document_insert_error)?;
    bump_workspace(transaction, &workspace.id, expected_revision, now)?;
    load_workspace(transaction, &workspace.id)
}

fn change_document_status(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    payload: &ChangeBusinessDocumentStatusPayload,
    expected_revision: i64,
    context: &NormalizedContext,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, &context.project_id)?;
    let document = workspace
        .documents
        .iter()
        .find(|document| document.id == payload.document_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_DOCUMENT_NOT_FOUND",
                "business document does not exist in workspace",
                false,
            )
        })?;
    validate_document_transition(&document.kind, &document.status, &payload.status)?;
    if matches!(
        payload.status,
        BusinessDocumentStatus::InReview | BusinessDocumentStatus::Approved
    ) {
        ensure_specialized_acceptance_snapshot_ready(document)?;
    }
    if payload.status == BusinessDocumentStatus::Voided
        && document.kind == BusinessDocumentKind::Contract
        && document.status == BusinessDocumentStatus::Effective
    {
        // Voiding an effective contract while money or downstream documents
        // already reference it would zero the contract amount and turn the
        // receivable ledger negative; require the downstream chain to be
        // unwound first.
        let has_receipts = !workspace.receipts.is_empty();
        let has_invoices = workspace
            .invoices
            .iter()
            .any(|invoice| invoice.kind == BusinessInvoiceKind::Issued);
        let has_active_payment_requests = workspace.documents.iter().any(|candidate| {
            candidate.kind == BusinessDocumentKind::PaymentRequest
                && candidate.status != BusinessDocumentStatus::Voided
        });
        if has_receipts || has_invoices || has_active_payment_requests {
            return Err(HostError::new(
                "BUSINESS_CONTRACT_IN_USE",
                "void or reverse downstream payment requests, invoices and receipts before voiding an effective contract",
                false,
            ));
        }
    }
    if matches!(
        payload.status,
        BusinessDocumentStatus::Approved | BusinessDocumentStatus::Effective
    ) {
        ensure_document_prerequisites(&workspace, &document.kind)?;
        if let Some(batch_id) = document.snapshot.acceptance_batch_id.as_deref() {
            ensure_acceptance_ready(&workspace, batch_id)?;
        }
        if document.kind == BusinessDocumentKind::Contract {
            ensure_current_quote_confirmed(transaction, vault_root, &workspace)?;
        }
        ensure_positive_document_total(document)?;
        ensure_payment_request_target(&workspace, document)?;
    }
    if payload.status == BusinessDocumentStatus::Approved {
        ensure_document_approvable(document)?;
    }
    if payload.status == BusinessDocumentStatus::Effective
        && document.kind == BusinessDocumentKind::Contract
    {
        ensure_contract_covers_payments(&workspace, document)?;
    }
    let now = now_millis();
    let (evidence, manual_waiver) = if payload.status == BusinessDocumentStatus::Effective {
        let kind = match document.kind {
            BusinessDocumentKind::Contract => BusinessEvidenceKind::ContractSignature,
            BusinessDocumentKind::Acceptance => BusinessEvidenceKind::AcceptanceProof,
            _ => {
                return Err(HostError::new(
                    "BUSINESS_EVIDENCE_KIND_INVALID",
                    "only contract and acceptance documents can become effective",
                    false,
                ))
            }
        };
        materialize_evidence_or_waiver(
            transaction,
            vault_root,
            &workspace.project_id,
            kind,
            payload.evidence.as_ref(),
            payload.manual_waiver.as_ref(),
            &context.actor_id,
            now,
        )?
    } else {
        if payload.evidence.is_some() || payload.manual_waiver.is_some() {
            return Err(HostError::new(
                "BUSINESS_EVIDENCE_NOT_ALLOWED",
                "evidence and manual waiver are only accepted when a contract or acceptance becomes effective",
                false,
            ));
        }
        (document.evidence.clone(), document.manual_waiver.clone())
    };
    let (voided_at, voided_by, void_reason) = if payload.status == BusinessDocumentStatus::Voided {
        if payload.reason.trim().is_empty() {
            return Err(HostError::new(
                "BUSINESS_VOID_REASON_REQUIRED",
                "voiding a business document requires a reason",
                false,
            ));
        }
        (
            Some(now),
            Some(context.actor_id.clone()),
            payload.reason.clone(),
        )
    } else {
        (
            document.voided_at,
            document.voided_by.clone(),
            document.void_reason.clone(),
        )
    };
    let (approved_at, approved_by) = if payload.status == BusinessDocumentStatus::Approved {
        (Some(now), Some(context.actor_id.clone()))
    } else if matches!(
        payload.status,
        BusinessDocumentStatus::Draft | BusinessDocumentStatus::InReview
    ) {
        (None, None)
    } else {
        (document.approved_at, document.approved_by.clone())
    };
    let changed = transaction
        .execute(
            "UPDATE business_documents
             SET status = ?1, evidence_json = ?2, manual_waiver_json = ?3,
                 voided_at = ?4, voided_by = ?5, void_reason = ?6,
                 approved_at = ?7, approved_by = ?8,
                 revision = revision + 1, updated_at = ?9
             WHERE id = ?10 AND workspace_id = ?11 AND revision = ?12",
            params![
                document_status_to_db(&payload.status),
                evidence
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(json_error)?,
                manual_waiver
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(json_error)?,
                voided_at,
                voided_by,
                void_reason,
                approved_at,
                approved_by,
                now,
                document.id,
                workspace.id,
                document.revision,
            ],
        )
        .map_err(sql_error)?;
    ensure_changed(changed)?;
    bump_workspace(transaction, &workspace.id, expected_revision, now)?;
    load_workspace(transaction, &workspace.id)
}

fn validate_document_transition(
    kind: &BusinessDocumentKind,
    current: &BusinessDocumentStatus,
    target: &BusinessDocumentStatus,
) -> Result<(), HostError> {
    if current == target {
        return Err(HostError::validation(
            "business document already has requested status",
        ));
    }
    if *target == BusinessDocumentStatus::Generated {
        return Err(HostError::new(
            "BUSINESS_DOCUMENT_STATUS_TRANSITION_INVALID",
            "generated status is owned by generateDocument",
            false,
        ));
    }
    if *target == BusinessDocumentStatus::Effective
        && !matches!(
            kind,
            BusinessDocumentKind::Contract | BusinessDocumentKind::Acceptance
        )
    {
        return Err(HostError::new(
            "BUSINESS_EVIDENCE_KIND_INVALID",
            "only contract and acceptance documents can become effective",
            false,
        ));
    }
    let allowed = matches!(
        (current, target),
        (
            BusinessDocumentStatus::Draft,
            BusinessDocumentStatus::InReview
        ) | (
            BusinessDocumentStatus::Draft,
            BusinessDocumentStatus::Voided
        ) | (
            BusinessDocumentStatus::InReview,
            BusinessDocumentStatus::Draft
        ) | (
            BusinessDocumentStatus::InReview,
            BusinessDocumentStatus::Approved
        ) | (
            BusinessDocumentStatus::InReview,
            BusinessDocumentStatus::Voided
        ) | (
            BusinessDocumentStatus::Approved,
            BusinessDocumentStatus::InReview
        ) | (
            BusinessDocumentStatus::Approved,
            BusinessDocumentStatus::Voided
        ) | (
            BusinessDocumentStatus::Generated,
            BusinessDocumentStatus::Effective
        ) | (
            BusinessDocumentStatus::Generated,
            BusinessDocumentStatus::Voided
        ) | (
            BusinessDocumentStatus::Effective,
            BusinessDocumentStatus::Voided
        )
    );
    if allowed {
        Ok(())
    } else {
        Err(HostError::new(
            "BUSINESS_DOCUMENT_STATUS_TRANSITION_INVALID",
            "requested business document status transition is not allowed",
            false,
        ))
    }
}

fn ensure_document_approvable(document: &BusinessDocumentRecord) -> Result<(), HostError> {
    let profile = &document.snapshot.profile;
    let mut missing = Vec::new();
    if profile.project_title.is_empty() {
        missing.push("projectTitle");
    }
    if profile.customer_name.is_empty() && profile.customer_legal_name.is_empty() {
        missing.push("customerName");
    }
    if profile.supplier_legal_name.is_empty() {
        missing.push("supplierLegalName");
    }
    if profile.currency.is_empty() {
        missing.push("currency");
    }
    if profile.line_items.is_empty() {
        missing.push("lineItems");
    }
    match document.kind {
        BusinessDocumentKind::Quote => {}
        BusinessDocumentKind::Contract => {
            if profile.service_start_at.is_none() {
                missing.push("serviceStartAt");
            }
            if profile.service_end_at.is_none() {
                missing.push("serviceEndAt");
            }
            if profile.payment_terms.is_empty() {
                missing.push("paymentTerms");
            }
            if profile.acceptance_terms.is_empty() {
                missing.push("acceptanceTerms");
            }
        }
        BusinessDocumentKind::PaymentRequest => {
            if document.snapshot.payment.is_none() {
                missing.push("payment");
            }
            if profile.supplier_bank_name.is_empty() {
                missing.push("supplierBankName");
            }
            if profile.supplier_bank_account.is_empty() {
                missing.push("supplierBankAccount");
            }
            if profile.payment_terms.is_empty() {
                missing.push("paymentTerms");
            }
            if document
                .snapshot
                .payment
                .as_ref()
                .is_none_or(|payment| payment.amount_cents <= 0)
            {
                missing.push("positiveAmount");
            }
        }
        BusinessDocumentKind::Acceptance => {
            if profile.delivery_summary.is_empty() {
                missing.push("deliverySummary");
            }
            if profile.acceptance_terms.is_empty() {
                missing.push("acceptanceTerms");
            }
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(HostError::new(
            "BUSINESS_DOCUMENT_INCOMPLETE",
            format!(
                "business document snapshot is missing: {}",
                missing.join(", ")
            ),
            false,
        ))
    }
}

fn ensure_document_generatable(
    document: &BusinessDocumentRecord,
    format: &BusinessDocumentFormat,
) -> Result<(), HostError> {
    if document.status != BusinessDocumentStatus::Approved {
        return Err(HostError::new(
            "BUSINESS_DOCUMENT_NOT_APPROVED",
            "business document must be approved before generation",
            false,
        ));
    }
    ensure_specialized_acceptance_snapshot_ready(document)?;
    document_engine::validate_template(&document.kind, &document.template_key)?;
    let valid_format = matches!(
        (&document.kind, format),
        (BusinessDocumentKind::Quote, BusinessDocumentFormat::Xlsx)
            | (
                BusinessDocumentKind::Contract | BusinessDocumentKind::PaymentRequest,
                BusinessDocumentFormat::Docx
            )
            | (BusinessDocumentKind::Acceptance, _)
    );
    if !valid_format {
        return Err(HostError::new(
            "BUSINESS_DOCUMENT_FORMAT_INVALID",
            "quote documents require XLSX; contract and payment request documents require DOCX",
            false,
        ));
    }
    if document.output_asset_id.is_some()
        || document.output_format.is_some()
        || document.generated_at.is_some()
    {
        return Err(HostError::new(
            "BUSINESS_DOCUMENT_ALREADY_GENERATED",
            "business document already owns generated output",
            false,
        ));
    }
    Ok(())
}

fn ensure_specialized_acceptance_snapshot_ready(
    document: &BusinessDocumentRecord,
) -> Result<(), HostError> {
    let expected_mapping_version =
        document_engine::expected_template_mapping_version(&document.template_key);
    if let Some(expected_mapping_version) = expected_mapping_version {
        if document.snapshot.template_mapping_version != expected_mapping_version {
            return Err(HostError::new(
                "BUSINESS_TEMPLATE_MAPPING_VERSION_MISMATCH",
                "document snapshot mapping version does not match the registered template renderer",
                false,
            ));
        }
    }
    match document.template_key.as_str() {
        document_engine::BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY => {
            if document.snapshot.video_completion_acceptance.is_none()
                || document.snapshot.production_result_confirmation.is_some()
                || document.snapshot.contract_settlement.is_some()
                || !document.snapshot.service_settlement_items.is_empty()
                || document.snapshot.payment_application.is_some()
            {
                return Err(HostError::new(
                    "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_DATA_REQUIRED",
                    "video completion acceptance snapshot requires frozen data and forbids settlement payloads",
                    false,
                ));
            }
        }
        document_engine::BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY => {
            let Some(data) = document.snapshot.production_result_confirmation.as_ref() else {
                return Err(HostError::new(
                    "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_DATA_REQUIRED",
                    "production result confirmation snapshot requires frozen data",
                    false,
                ));
            };
            if !data.manually_confirmed
                || !data.clean_highlights_confirmed
                || document.snapshot.video_completion_acceptance.is_some()
                || document.snapshot.contract_settlement.is_some()
                || !document.snapshot.service_settlement_items.is_empty()
                || document.snapshot.payment_application.is_some()
            {
                return Err(HostError::new(
                    "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_DATA_REQUIRED",
                    "production result confirmation snapshot requires both confirmations and forbids other specialized payloads",
                    false,
                ));
            }
        }
        document_engine::BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY => {
            if document.snapshot.contract_settlement.is_none()
                || !document.snapshot.service_settlement_items.is_empty()
            {
                return Err(HostError::new(
                    "BUSINESS_CONTRACT_SETTLEMENT_DATA_REQUIRED",
                    "contract settlement snapshot requires frozen settlement data",
                    false,
                ));
            }
        }
        document_engine::BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY => {
            if document.snapshot.contract_settlement.is_some()
                || document.snapshot.service_settlement_items.is_empty()
            {
                return Err(HostError::new(
                    "BUSINESS_SERVICE_SETTLEMENT_DATA_REQUIRED",
                    "service settlement snapshot requires frozen service rows",
                    false,
                ));
            }
            for (index, item) in document
                .snapshot
                .service_settlement_items
                .iter()
                .enumerate()
            {
                let Some(provided_as_required) = item.provided_as_required else {
                    return Err(HostError::new(
                        "BUSINESS_SERVICE_SETTLEMENT_CONFIRMATION_REQUIRED",
                        format!(
                            "service settlement row {} requires providedAsRequired confirmation",
                            index + 1
                        ),
                        false,
                    ));
                };
                if !provided_as_required && item.remarks.trim().is_empty() {
                    return Err(HostError::new(
                        "BUSINESS_SERVICE_SETTLEMENT_REMARKS_REQUIRED",
                        format!(
                            "service settlement row {} requires remarks when service was not provided as required",
                            index + 1
                        ),
                        false,
                    ));
                }
            }
        }
        document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY => {
            let data = document
                .snapshot
                .payment_application
                .as_ref()
                .ok_or_else(|| {
                    HostError::new(
                        "BUSINESS_PAYMENT_APPLICATION_DATA_REQUIRED",
                        "payment application snapshot is missing frozen payment data",
                        false,
                    )
                })?;
            if document.snapshot.contract_settlement.is_some()
                || !document.snapshot.service_settlement_items.is_empty()
                || document
                    .snapshot
                    .payment
                    .as_ref()
                    .is_none_or(|payment| payment.id != data.payment_id)
                || data.bank_account_profile_version.trim().is_empty()
            {
                return Err(HostError::new(
                    "BUSINESS_PAYMENT_APPLICATION_DATA_REQUIRED",
                    "payment application snapshot has inconsistent frozen data",
                    false,
                ));
            }
            let settlement_total =
                data.settlement_items
                    .iter()
                    .try_fold(0_i64, |total, item| {
                        total
                            .checked_add(payment_settlement_item_amount(item, true)?)
                            .ok_or_else(|| {
                                HostError::validation("payment settlement total overflowed")
                            })
                    })?;
            let remaining = settlement_total
                .checked_sub(data.cumulative_paid_cents)
                .ok_or_else(|| HostError::validation("remaining payable overflowed"))?;
            if settlement_total != data.settlement_total_cents
                || remaining != data.remaining_payable_cents
                || remaining <= 0
            {
                return Err(HostError::new(
                    "BUSINESS_PAYMENT_REMAINING_PAYABLE_MISMATCH",
                    "frozen remaining payable does not match settlement total minus cumulative paid",
                    false,
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn ensure_payment_application_current(
    workspace: &BusinessWorkspaceRecord,
    document: &BusinessDocumentRecord,
) -> Result<(), HostError> {
    if document.template_key
        != document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY
    {
        return Ok(());
    }
    let data = document
        .snapshot
        .payment_application
        .as_ref()
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_PAYMENT_APPLICATION_DATA_REQUIRED",
                "payment application snapshot is missing frozen data",
                false,
            )
        })?;
    let frozen_profile = &document.snapshot.profile;
    if workspace.profile.supplier_legal_name != frozen_profile.supplier_legal_name
        || workspace.profile.supplier_bank_name != frozen_profile.supplier_bank_name
        || workspace.profile.supplier_bank_account != frozen_profile.supplier_bank_account
    {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_BANK_ACCOUNT_CHANGED",
            "supplier or bank account data changed after the payment application snapshot was frozen",
            false,
        ));
    }
    let frozen_bank_account_version =
        payment_bank_account_profile_version(frozen_profile, &data.supplier_bank_routing_number);
    let current_bank_account_version = payment_bank_account_profile_version(
        &workspace.profile,
        &data.supplier_bank_routing_number,
    );
    if data.bank_account_profile_version != frozen_bank_account_version
        || data.bank_account_profile_version != current_bank_account_version
    {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_BANK_ACCOUNT_CHANGED",
            "bank account version changed; the routing number remains bound to the frozen payment application input because the authoritative profile has no routing-number field",
            false,
        ));
    }
    if workspace.financial_summary.received_cents != data.cumulative_paid_cents {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_LEDGER_CHANGED",
            "receipt ledger changed after the payment application snapshot was frozen",
            false,
        ));
    }
    let payment = workspace
        .payments
        .iter()
        .find(|payment| payment.id == data.payment_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_PAYMENT_NOT_FOUND",
                "frozen payment application references a missing payment",
                false,
            )
        })?;
    if document.snapshot.payment.as_ref() != Some(payment) {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_VERSION_CHANGED",
            "payment plan changed after the payment application snapshot was frozen",
            false,
        ));
    }
    let (has_invoice_records, current_invoice_cents) =
        invoice_ledger_for_payment(workspace, &payment.id)?;
    if has_invoice_records && current_invoice_cents != data.invoice_amount_cents {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_INVOICE_LEDGER_CHANGED",
            "invoice ledger net amount changed after the payment application snapshot was frozen",
            false,
        ));
    }
    let current_payment_received = receipt_net_for_payment(&workspace.receipts, &payment.id)?;
    let outstanding = payment
        .amount_cents
        .checked_sub(current_payment_received)
        .ok_or_else(|| HostError::validation("payment outstanding overflowed"))?;
    if outstanding != data.remaining_payable_cents {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_REMAINING_PAYABLE_MISMATCH",
            "current payment outstanding no longer matches the frozen remaining payable",
            false,
        ));
    }
    Ok(())
}

fn ensure_acceptance_output_format(
    workspace: &BusinessWorkspaceRecord,
    document: &BusinessDocumentRecord,
    format: &BusinessDocumentFormat,
) -> Result<(), HostError> {
    if document.kind != BusinessDocumentKind::Acceptance {
        return Ok(());
    }
    let (Some(batch_id), Some(output_spec_id)) = (
        document.snapshot.acceptance_batch_id.as_deref(),
        document.snapshot.acceptance_output_spec_id.as_deref(),
    ) else {
        return Ok(());
    };
    let output_spec = workspace
        .acceptance_batches
        .iter()
        .find(|batch| batch.id == batch_id)
        .and_then(|batch| {
            batch
                .output_specs
                .iter()
                .find(|output_spec| output_spec.id == output_spec_id)
        })
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_ACCEPTANCE_OUTPUT_SPEC_NOT_FOUND",
                "acceptance document output spec no longer exists in its batch",
                false,
            )
        })?;
    if output_spec.format == *format {
        Ok(())
    } else {
        Err(HostError::new(
            "BUSINESS_ACCEPTANCE_OUTPUT_FORMAT_MISMATCH",
            "acceptance document format must match its configured output spec",
            false,
        ))
    }
}

fn finalize_generation(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    payload: &GenerateBusinessDocumentPayload,
    expected_revision: i64,
    project_id: &str,
    asset_id: &str,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, project_id)?;
    let document = workspace
        .documents
        .iter()
        .find(|document| document.id == payload.document_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_DOCUMENT_NOT_FOUND",
                "business document does not exist in workspace",
                false,
            )
        })?;
    ensure_acceptance_output_format(&workspace, document, &payload.format)?;
    ensure_document_generatable(document, &payload.format)?;
    ensure_document_prerequisites(&workspace, &document.kind)?;
    if let Some(batch_id) = document.snapshot.acceptance_batch_id.as_deref() {
        ensure_acceptance_ready(&workspace, batch_id)?;
    }
    if document.kind == BusinessDocumentKind::Contract {
        ensure_current_quote_confirmed(transaction, vault_root, &workspace)?;
    }
    ensure_positive_document_total(document)?;
    ensure_payment_request_target(&workspace, document)?;
    let asset_project_id = transaction
        .query_row(
            "SELECT a.project_id FROM assets a
             JOIN asset_origins origin ON origin.asset_id = a.id
             WHERE a.id = ?1 AND a.status = 'ready'
               AND origin.origin = 'businessDocument'",
            [asset_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::new("ASSET_NOT_FOUND", "generated asset is missing", false))?;
    if asset_project_id.as_deref() != Some(workspace.project_id.as_str()) {
        return Err(HostError::new(
            "BUSINESS_DOCUMENT_ASSET_PROJECT_MISMATCH",
            "generated asset belongs to a different project",
            false,
        ));
    }
    let now = now_millis();
    let changed = transaction
        .execute(
            "UPDATE business_documents
             SET status = 'generated', output_asset_id = ?1, output_format = ?2,
                 generated_at = ?3, revision = revision + 1, updated_at = ?3
             WHERE id = ?4 AND workspace_id = ?5 AND revision = ?6 AND status = 'approved'
                   AND output_asset_id IS NULL",
            params![
                asset_id,
                document_format_to_db(&payload.format),
                now,
                document.id,
                workspace.id,
                document.revision,
            ],
        )
        .map_err(sql_error)?;
    ensure_changed(changed)?;
    if document.kind == BusinessDocumentKind::PaymentRequest {
        let payment_id = document
            .snapshot
            .payment
            .as_ref()
            .map(|payment| payment.id.as_str())
            .ok_or_else(|| {
                HostError::internal("generated payment request lost its payment link")
            })?;
        transaction
            .execute(
                "UPDATE business_payments
                 SET status = 'requested', revision = revision + 1, updated_at = ?1
                 WHERE id = ?2 AND workspace_id = ?3 AND status = 'planned'",
                params![now, payment_id, workspace.id],
            )
            .map_err(sql_error)?;
    }
    bump_workspace(transaction, &workspace.id, expected_revision, now)?;
    load_workspace(transaction, &workspace.id)
}

fn normalize_payment_input(
    payment: BusinessPaymentInput,
) -> Result<BusinessPaymentInput, HostError> {
    if !(1..=MAX_MONEY_CENTS).contains(&payment.amount_cents) {
        return Err(HostError::validation(
            "payment amountCents is outside the supported range",
        ));
    }
    validate_timestamp("payment dueAt", payment.due_at)?;
    validate_timestamp("payment occurredAt", payment.occurred_at)?;
    if matches!(
        payment.status,
        BusinessPaymentStatus::Requested
            | BusinessPaymentStatus::PartiallyReceived
            | BusinessPaymentStatus::Received
    ) {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_STATUS_MANAGED",
            "requested and receipt statuses are managed by generated payment requests and the receipt ledger",
            false,
        ));
    }
    if payment.occurred_at.is_some() {
        return Err(HostError::validation(
            "payment plans cannot provide occurredAt; receipt commands own settlement timestamps",
        ));
    }
    Ok(BusinessPaymentInput {
        id: payment
            .id
            .map(|id| normalize_uuid("payment id", id))
            .transpose()?,
        label: normalize_required("payment label", payment.label, MAX_SHORT_CHARS)?,
        amount_cents: payment.amount_cents,
        due_at: payment.due_at,
        occurred_at: None,
        status: payment.status,
        reference: normalize_text("payment reference", payment.reference, MAX_SHORT_CHARS)?,
        notes: normalize_text("payment notes", payment.notes, MAX_TEXT_CHARS)?,
    })
}

fn effective_contract_amount(workspace: &BusinessWorkspaceRecord) -> Option<i64> {
    workspace
        .current_documents
        .contract_document_id
        .as_deref()
        .and_then(|id| {
            workspace
                .documents
                .iter()
                .find(|document| document.id == id)
        })
        .map(document_total_cents)
}

fn ensure_payment_within_contract(
    workspace: &BusinessWorkspaceRecord,
    current_payment_id: Option<&str>,
    amount_cents: i64,
    status: &BusinessPaymentStatus,
) -> Result<(), HostError> {
    let contract_cents = effective_contract_amount(workspace);
    if matches!(
        status,
        BusinessPaymentStatus::Requested | BusinessPaymentStatus::Received
    ) && contract_cents.is_none()
    {
        return Err(HostError::new(
            "BUSINESS_EFFECTIVE_CONTRACT_REQUIRED",
            "requested or received payment requires an effective contract",
            false,
        ));
    }
    let Some(contract_cents) = contract_cents else {
        return Ok(());
    };
    let existing_total = workspace
        .payments
        .iter()
        .filter(|payment| {
            payment.status != BusinessPaymentStatus::Canceled
                && current_payment_id.is_none_or(|id| payment.id != id)
        })
        .try_fold(0_i64, |total, payment| {
            total.checked_add(payment.amount_cents)
        })
        .ok_or_else(|| HostError::validation("payment total exceeds supported range"))?;
    let prospective_total = if *status == BusinessPaymentStatus::Canceled {
        existing_total
    } else {
        existing_total
            .checked_add(amount_cents)
            .ok_or_else(|| HostError::validation("payment total exceeds supported range"))?
    };
    if prospective_total > contract_cents {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_EXCEEDS_CONTRACT",
            "non-canceled payment total cannot exceed the effective contract amount",
            false,
        ));
    }
    Ok(())
}

fn ensure_payment_receivable(
    workspace: &BusinessWorkspaceRecord,
    payment_id: &str,
    reference: &str,
) -> Result<(), HostError> {
    let has_generated_request = workspace.documents.iter().any(|document| {
        document.kind == BusinessDocumentKind::PaymentRequest
            && document.status == BusinessDocumentStatus::Generated
            && document
                .snapshot
                .payment
                .as_ref()
                .is_some_and(|payment| payment.id == payment_id)
    });
    if !has_generated_request {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_REQUEST_DOCUMENT_REQUIRED",
            "received payment requires a generated payment request linked to this payment",
            false,
        ));
    }
    let reference_key = reference.trim().to_lowercase();
    if reference_key.is_empty() {
        return Err(HostError::validation(
            "received payment requires a bank reference",
        ));
    }
    if workspace.payments.iter().any(|payment| {
        payment.id != payment_id
            && payment.status == BusinessPaymentStatus::Received
            && payment.reference.trim().to_lowercase() == reference_key
    }) {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_REFERENCE_DUPLICATE",
            "bank reference already exists in this business workspace",
            false,
        ));
    }
    Ok(())
}

fn verified_project_asset(
    connection: &Connection,
    vault_root: &Path,
    asset_id: &str,
    project_id: &str,
) -> Result<crate::protocol::AssetRecord, HostError> {
    let (asset, _) = asset_service::verify_ready_asset_integrity(connection, vault_root, asset_id)?;
    if asset.project_id.as_deref() != Some(project_id) {
        return Err(HostError::new(
            "BUSINESS_EVIDENCE_PROJECT_MISMATCH",
            "evidence asset belongs to a different project",
            false,
        ));
    }
    Ok(asset)
}

fn materialize_evidence(
    connection: &Connection,
    vault_root: &Path,
    project_id: &str,
    kind: BusinessEvidenceKind,
    input: &BusinessEvidenceInput,
    actor_id: &str,
    recorded_at: i64,
) -> Result<BusinessEvidenceRecord, HostError> {
    let asset = verified_project_asset(connection, vault_root, &input.asset_id, project_id)?;
    Ok(BusinessEvidenceRecord {
        kind,
        asset_id: asset.id,
        sha256: asset.sha256,
        occurred_at: input.occurred_at,
        note: input.note.clone(),
        recorded_by: actor_id.to_string(),
        recorded_at,
    })
}

#[allow(clippy::too_many_arguments)]
fn materialize_evidence_or_waiver(
    connection: &Connection,
    vault_root: &Path,
    project_id: &str,
    kind: BusinessEvidenceKind,
    evidence: Option<&BusinessEvidenceInput>,
    waiver: Option<&BusinessManualWaiverInput>,
    actor_id: &str,
    now: i64,
) -> Result<
    (
        Option<BusinessEvidenceRecord>,
        Option<BusinessManualWaiverRecord>,
    ),
    HostError,
> {
    match (evidence, waiver) {
        (Some(_), Some(_)) => Err(HostError::new(
            "BUSINESS_EVIDENCE_AMBIGUOUS",
            "provide either evidence or a manual waiver, not both",
            false,
        )),
        (None, None) => Err(HostError::new(
            "BUSINESS_EVIDENCE_REQUIRED",
            "this lifecycle action requires evidence or an approved manual waiver",
            false,
        )),
        (Some(input), None) => Ok((
            Some(materialize_evidence(
                connection, vault_root, project_id, kind, input, actor_id, now,
            )?),
            None,
        )),
        (None, Some(input)) => Ok((
            None,
            Some(BusinessManualWaiverRecord {
                reason: input.reason.clone(),
                approved_by: actor_id.to_string(),
                approved_at: now,
            }),
        )),
    }
}

fn confirm_quote(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    payload: &ConfirmBusinessQuotePayload,
    expected_revision: i64,
    context: &NormalizedContext,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, &context.project_id)?;
    let quote = workspace
        .documents
        .iter()
        .find(|document| document.id == payload.quote_document_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_QUOTE_NOT_FOUND",
                "quote document does not exist in this workspace",
                false,
            )
        })?;
    if quote.kind != BusinessDocumentKind::Quote
        || quote.status != BusinessDocumentStatus::Generated
    {
        return Err(HostError::new(
            "BUSINESS_QUOTE_NOT_CONFIRMABLE",
            "only a generated quote can be confirmed by the customer",
            false,
        ));
    }
    let quote_asset_id = quote.output_asset_id.as_deref().ok_or_else(|| {
        HostError::new(
            "BUSINESS_QUOTE_ASSET_REQUIRED",
            "generated quote is missing its authoritative output asset",
            false,
        )
    })?;
    let quote_asset = verified_project_asset(
        transaction,
        vault_root,
        quote_asset_id,
        &workspace.project_id,
    )?;
    let now = now_millis();
    let evidence = materialize_evidence(
        transaction,
        vault_root,
        &workspace.project_id,
        BusinessEvidenceKind::QuoteConfirmation,
        &payload.evidence,
        &context.actor_id,
        now,
    )?;
    let result = transaction.execute(
        "INSERT INTO business_quote_confirmations
         (id, workspace_id, quote_document_id, quote_document_revision, quote_asset_id,
          quote_sha256, confirmation_version, customer_representative, evidence_json,
          notes, confirmed_by, confirmed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            Uuid::new_v4().to_string(),
            workspace.id,
            quote.id,
            quote.revision,
            quote_asset.id,
            quote_asset.sha256,
            payload.confirmation_version,
            payload.customer_representative,
            serde_json::to_string(&evidence).map_err(json_error)?,
            payload.notes,
            context.actor_id,
            now,
        ],
    );
    if let Err(error) = result {
        if is_constraint_error(&error) {
            return Err(HostError::new(
                "BUSINESS_QUOTE_CONFIRMATION_DUPLICATE",
                "this exact quote version has already been confirmed",
                false,
            ));
        }
        return Err(sql_error(error));
    }
    bump_workspace(transaction, &workspace.id, expected_revision, now)?;
    load_workspace(transaction, &workspace.id)
}

fn receipt_net_for_payment(
    receipts: &[BusinessReceiptRecord],
    payment_id: &str,
) -> Result<i64, HostError> {
    receipts
        .iter()
        .filter(|receipt| receipt.payment_id == payment_id)
        .try_fold(0_i64, |net, receipt| match receipt.kind {
            BusinessReceiptKind::Receipt => net.checked_add(receipt.amount_cents),
            BusinessReceiptKind::Reversal => net.checked_sub(receipt.amount_cents),
        })
        .ok_or_else(|| HostError::validation("receipt ledger total exceeds supported range"))
}

fn ensure_receipt_reference_available(
    transaction: &Transaction<'_>,
    reference: &str,
) -> Result<(), HostError> {
    let exists = transaction
        .query_row(
            "SELECT 1 FROM business_receipts WHERE lower(reference) = lower(?1) LIMIT 1",
            [reference],
            |_| Ok(()),
        )
        .optional()
        .map_err(sql_error)?
        .is_some();
    if exists {
        Err(HostError::new(
            "BUSINESS_RECEIPT_REFERENCE_DUPLICATE",
            "receipt reference already exists",
            false,
        ))
    } else {
        Ok(())
    }
}

fn update_payment_from_receipt_ledger(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    payment: &BusinessPaymentRecord,
    net_received: i64,
    occurred_at: i64,
    reference: &str,
) -> Result<(), HostError> {
    if net_received < 0 || net_received > payment.amount_cents {
        return Err(HostError::new(
            "BUSINESS_RECEIPT_TOTAL_INVALID",
            "receipt ledger total must remain between zero and the payment amount",
            false,
        ));
    }
    let (status, projected_occurred_at, projected_reference) = if net_received == 0 {
        (BusinessPaymentStatus::Requested, None, String::new())
    } else if net_received < payment.amount_cents {
        (
            BusinessPaymentStatus::PartiallyReceived,
            Some(occurred_at),
            reference.to_string(),
        )
    } else {
        (
            BusinessPaymentStatus::Received,
            Some(occurred_at),
            reference.to_string(),
        )
    };
    let changed = transaction
        .execute(
            "UPDATE business_payments
             SET status = ?1, occurred_at = ?2, reference = ?3,
                 revision = revision + 1, updated_at = ?4
             WHERE id = ?5 AND workspace_id = ?6 AND revision = ?7",
            params![
                payment_status_to_db(&status),
                projected_occurred_at,
                projected_reference,
                now_millis(),
                payment.id,
                workspace_id,
                payment.revision,
            ],
        )
        .map_err(sql_error)?;
    ensure_changed(changed)
}

fn record_receipt(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    payload: &RecordBusinessReceiptPayload,
    expected_revision: i64,
    context: &NormalizedContext,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, &context.project_id)?;
    if workspace.receipts.len() as i64 >= MAX_RECEIPTS_PER_WORKSPACE {
        return Err(HostError::new(
            "BUSINESS_RECEIPT_LIMIT_REACHED",
            "workspace receipt ledger limit has been reached",
            false,
        ));
    }
    let payment = workspace
        .payments
        .iter()
        .find(|payment| payment.id == payload.payment_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_PAYMENT_NOT_FOUND",
                "payment does not exist in this workspace",
                false,
            )
        })?;
    if !matches!(
        payment.status,
        BusinessPaymentStatus::Requested | BusinessPaymentStatus::PartiallyReceived
    ) {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_NOT_RECEIVABLE",
            "only requested or partially received payments can accept a receipt",
            false,
        ));
    }
    ensure_payment_receivable(&workspace, &payment.id, &payload.reference)?;
    ensure_receipt_reference_available(transaction, &payload.reference)?;
    let current_net = receipt_net_for_payment(&workspace.receipts, &payment.id)?;
    let new_net = current_net
        .checked_add(payload.amount_cents)
        .ok_or_else(|| HostError::validation("receipt ledger total exceeds supported range"))?;
    if new_net > payment.amount_cents {
        return Err(HostError::new(
            "BUSINESS_RECEIPT_EXCEEDS_PAYMENT",
            "receipt would exceed the outstanding payment amount",
            false,
        ));
    }
    let now = now_millis();
    let evidence = payload
        .evidence
        .as_ref()
        .map(|input| {
            materialize_evidence(
                transaction,
                vault_root,
                &workspace.project_id,
                BusinessEvidenceKind::ReceiptProof,
                input,
                &context.actor_id,
                now,
            )
        })
        .transpose()?;
    transaction
        .execute(
            "INSERT INTO business_receipts
             (id, workspace_id, payment_id, kind, amount_cents, occurred_at, reference,
              notes, reverses_receipt_id, evidence_json, recorded_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10, ?11)",
            params![
                Uuid::new_v4().to_string(),
                workspace.id,
                payment.id,
                receipt_kind_to_db(&BusinessReceiptKind::Receipt),
                payload.amount_cents,
                payload.occurred_at,
                payload.reference,
                payload.notes,
                evidence
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(json_error)?,
                context.actor_id,
                now,
            ],
        )
        .map_err(sql_error)?;
    update_payment_from_receipt_ledger(
        transaction,
        &workspace.id,
        payment,
        new_net,
        payload.occurred_at,
        &payload.reference,
    )?;
    bump_workspace(transaction, &workspace.id, expected_revision, now)?;
    load_workspace(transaction, &workspace.id)
}

fn reverse_receipt(
    transaction: &Transaction<'_>,
    payload: &ReverseBusinessReceiptPayload,
    expected_revision: i64,
    context: &NormalizedContext,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, &context.project_id)?;
    if workspace.receipts.len() as i64 >= MAX_RECEIPTS_PER_WORKSPACE {
        return Err(HostError::new(
            "BUSINESS_RECEIPT_LIMIT_REACHED",
            "workspace receipt ledger limit has been reached",
            false,
        ));
    }
    let original = workspace
        .receipts
        .iter()
        .find(|receipt| receipt.id == payload.receipt_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_RECEIPT_NOT_FOUND",
                "receipt does not exist in this workspace",
                false,
            )
        })?;
    if original.kind != BusinessReceiptKind::Receipt {
        return Err(HostError::new(
            "BUSINESS_RECEIPT_REVERSAL_INVALID",
            "only an original receipt can be reversed",
            false,
        ));
    }
    if payload.occurred_at < original.occurred_at {
        return Err(HostError::validation(
            "receipt reversal occurredAt cannot be earlier than the original receipt",
        ));
    }
    ensure_receipt_reference_available(transaction, &payload.reference)?;
    let already_reversed = workspace
        .receipts
        .iter()
        .filter(|receipt| {
            receipt.kind == BusinessReceiptKind::Reversal
                && receipt.reverses_receipt_id.as_deref() == Some(original.id.as_str())
        })
        .try_fold(0_i64, |total, receipt| {
            total.checked_add(receipt.amount_cents)
        })
        .ok_or_else(|| HostError::validation("receipt reversal total exceeds supported range"))?;
    let prospective_reversed = already_reversed
        .checked_add(payload.amount_cents)
        .ok_or_else(|| HostError::validation("receipt reversal total exceeds supported range"))?;
    if prospective_reversed > original.amount_cents {
        return Err(HostError::new(
            "BUSINESS_RECEIPT_REVERSAL_EXCEEDS_ORIGINAL",
            "receipt reversal cannot exceed the unreversed original amount",
            false,
        ));
    }
    let payment = workspace
        .payments
        .iter()
        .find(|payment| payment.id == original.payment_id)
        .ok_or_else(|| HostError::internal("receipt ledger references a missing payment"))?;
    let current_net = receipt_net_for_payment(&workspace.receipts, &payment.id)?;
    let new_net = current_net
        .checked_sub(payload.amount_cents)
        .ok_or_else(|| HostError::validation("receipt ledger total exceeds supported range"))?;
    let now = now_millis();
    transaction
        .execute(
            "INSERT INTO business_receipts
             (id, workspace_id, payment_id, kind, amount_cents, occurred_at, reference,
              notes, reverses_receipt_id, evidence_json, recorded_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?11)",
            params![
                Uuid::new_v4().to_string(),
                workspace.id,
                payment.id,
                receipt_kind_to_db(&BusinessReceiptKind::Reversal),
                payload.amount_cents,
                payload.occurred_at,
                payload.reference,
                payload.reason,
                original.id,
                context.actor_id,
                now,
            ],
        )
        .map_err(sql_error)?;
    update_payment_from_receipt_ledger(
        transaction,
        &workspace.id,
        payment,
        new_net,
        payload.occurred_at,
        if new_net == 0 {
            ""
        } else {
            payment.reference.as_str()
        },
    )?;
    bump_workspace(transaction, &workspace.id, expected_revision, now)?;
    load_workspace(transaction, &workspace.id)
}

fn merge_confirmed_requirement(
    profile: &BusinessProfile,
    content: RequirementBriefContent,
) -> Result<BusinessProfile, HostError> {
    let mut proposed = profile.clone();
    proposed.delivery_summary = join_sections([
        content.objective.as_str(),
        content.key_message.as_str(),
        &content.deliverables.join("\n"),
    ]);
    proposed.acceptance_terms = content.acceptance_criteria.join("\n");
    proposed.notes = join_labeled_sections([
        ("Constraints", content.constraints.as_slice()),
        ("Risks", content.risks.as_slice()),
    ]);
    proposed.service_end_at = content.deadline_at;
    let existing = profile
        .line_items
        .iter()
        .map(|item| (normalized_business_identity(&item.name), item))
        .collect::<HashMap<_, _>>();
    proposed.line_items = content
        .deliverables
        .into_iter()
        .map(|deliverable| {
            if let Some(item) = existing.get(&normalized_business_identity(&deliverable)) {
                (*item).clone()
            } else {
                BusinessLineItem {
                    id: Uuid::new_v4().to_string(),
                    name: deliverable,
                    description: String::new(),
                    quantity_millis: 1_000,
                    unit: "item".to_string(),
                    unit_price_cents: 0,
                    tax_rate_bps: 0,
                    amount_cents: 0,
                }
            }
        })
        .collect();
    proposed.quotation_totals = Some(calculate_quotation_totals(
        &proposed.line_items,
        proposed.project_discount_cents,
    )?);
    Ok(proposed)
}

fn adopt_latest_confirmed_requirement(
    transaction: &Transaction<'_>,
    payload: &AdoptLatestConfirmedRequirementPayload,
    expected_revision: i64,
    context: &NormalizedContext,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, &context.project_id)?;
    if workspace.documents.iter().any(|document| {
        matches!(
            document.status,
            BusinessDocumentStatus::Approved
                | BusinessDocumentStatus::Generated
                | BusinessDocumentStatus::Effective
                | BusinessDocumentStatus::Voided
        )
    }) {
        return Err(HostError::new(
            "BUSINESS_REQUIREMENT_ADOPTION_BLOCKED",
            "latest requirement cannot be adopted after a formal or voided document exists",
            false,
        ));
    }
    let latest = transaction
        .query_row(
            "SELECT id, revision, content_json FROM requirement_briefs
             WHERE project_id = ?1 AND status = 'confirmed'
             ORDER BY revision DESC, updated_at DESC, id DESC LIMIT 1",
            [&workspace.project_id],
            |row| {
                let id: String = row.get(0)?;
                let revision: i64 = row.get(1)?;
                let json: String = row.get(2)?;
                let content = from_json_column(&json)?;
                Ok((id, revision, content))
            },
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| {
            HostError::new(
                "CONFIRMED_REQUIREMENT_BRIEF_REQUIRED",
                "project has no confirmed requirement brief",
                false,
            )
        })?;
    if workspace.requirement_brief_id.as_deref() == Some(latest.0.as_str())
        && workspace.requirement_brief_revision == Some(latest.1)
    {
        return Err(HostError::new(
            "BUSINESS_REQUIREMENT_ALREADY_CURRENT",
            "business workspace already uses the latest confirmed requirement",
            false,
        ));
    }
    let proposed = merge_confirmed_requirement(&workspace.profile, latest.2)?;
    let now = now_millis();
    let changed = transaction
        .execute(
            "UPDATE business_workspaces
             SET requirement_brief_id = ?1, requirement_brief_revision = ?2,
                 profile_json = ?3, customer_name_key = ?4, customer_legal_name_key = ?5,
                 revision = revision + 1, updated_at = ?6
             WHERE id = ?7 AND revision = ?8",
            params![
                latest.0,
                latest.1,
                serde_json::to_string(&proposed).map_err(json_error)?,
                normalized_business_identity(&proposed.customer_name),
                normalized_business_identity(&proposed.customer_legal_name),
                now,
                workspace.id,
                expected_revision,
            ],
        )
        .map_err(sql_error)?;
    ensure_changed(changed)?;
    load_workspace(transaction, &workspace.id)
}

fn upsert_settlement_batch(
    transaction: &Transaction<'_>,
    payload: &UpsertBusinessSettlementBatchPayload,
    expected_revision: i64,
    project_id: &str,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, project_id)?;
    if payload.batch.status == BusinessSettlementBatchStatus::Voided {
        return Err(HostError::new(
            "BUSINESS_SETTLEMENT_STATUS_MANAGED",
            "void settlement batches with the dedicated void command",
            false,
        ));
    }
    if payload.batch.lines.is_empty() {
        return Err(HostError::validation(
            "settlement batch must contain at least one deliverable",
        ));
    }
    if payload.batch.lines.len() > MAX_SETTLEMENT_LINES_PER_BATCH {
        return Err(HostError::validation(format!(
            "settlement batch cannot contain more than {MAX_SETTLEMENT_LINES_PER_BATCH} lines"
        )));
    }
    let batch_id = payload
        .batch
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let existing = workspace
        .settlement_batches
        .iter()
        .find(|batch| batch.id == batch_id);
    if payload.batch.id.is_some() && existing.is_none() {
        return Err(HostError::new(
            "BUSINESS_SETTLEMENT_BATCH_NOT_FOUND",
            "settlement batch ID does not belong to this workspace",
            false,
        ));
    }
    if existing.is_some_and(|batch| batch.status == BusinessSettlementBatchStatus::Voided) {
        return Err(HostError::new(
            "BUSINESS_SETTLEMENT_BATCH_VOIDED",
            "voided settlement batches cannot be edited",
            false,
        ));
    }
    if existing.is_none()
        && workspace.settlement_batches.len() >= MAX_SETTLEMENT_BATCHES_PER_WORKSPACE
    {
        return Err(HostError::new(
            "BUSINESS_SETTLEMENT_BATCH_LIMIT_REACHED",
            "workspace settlement batch limit has been reached",
            false,
        ));
    }
    let contract_number = normalize_required(
        "contractNumber",
        payload.batch.contract_number.clone(),
        MAX_SHORT_CHARS,
    )?;
    let settlement_period = normalize_required(
        "settlementPeriod",
        payload.batch.settlement_period.clone(),
        MAX_SHORT_CHARS,
    )?;
    let notes = normalize_text("notes", payload.batch.notes.clone(), MAX_TEXT_CHARS)?;
    let mut seen_deliverables = HashSet::new();
    let mut lines = Vec::with_capacity(payload.batch.lines.len());
    for input in &payload.batch.lines {
        if !seen_deliverables.insert(input.deliverable_id.clone()) {
            return Err(HostError::new(
                "BUSINESS_SETTLEMENT_DELIVERABLE_DUPLICATE",
                "a deliverable can appear only once in a settlement batch",
                false,
            ));
        }
        if workspace.settlement_batches.iter().any(|batch| {
            batch.id != batch_id
                && batch.status != BusinessSettlementBatchStatus::Voided
                && batch
                    .lines
                    .iter()
                    .any(|line| line.deliverable_id == input.deliverable_id)
        }) {
            return Err(HostError::new(
                "BUSINESS_SETTLEMENT_DELIVERABLE_ALREADY_RESERVED",
                "deliverable is already referenced by another active settlement batch",
                false,
            ));
        }
        let (milestone_id, deliverable_name) = workspace
            .milestones
            .iter()
            .find_map(|milestone| {
                milestone
                    .deliverables
                    .iter()
                    .find(|deliverable| deliverable.id == input.deliverable_id)
                    .map(|deliverable| (milestone.id.clone(), deliverable.name.clone()))
            })
            .ok_or_else(|| {
                HostError::new(
                    "BUSINESS_SETTLEMENT_DELIVERABLE_NOT_FOUND",
                    "settlement deliverable does not exist in this workspace",
                    false,
                )
            })?;
        validate_settlement_line(input)?;
        lines.push(BusinessSettlementLineRecord {
            deliverable_id: input.deliverable_id.clone(),
            milestone_id,
            deliverable_name,
            contract_quantity_millis: input.contract_quantity_millis,
            cumulative_executed_millis: input.cumulative_executed_millis,
            current_executed_millis: input.current_executed_millis,
            cumulative_accepted_millis: input.cumulative_accepted_millis,
            current_accepted_millis: input.current_accepted_millis,
            cumulative_settled_millis: input.current_settlement_millis,
            current_settlement_millis: input.current_settlement_millis,
            remaining_quantity_millis: input.contract_quantity_millis
                - input.current_settlement_millis,
            unit: normalize_required("unit", input.unit.clone(), MAX_SHORT_CHARS)?,
            notes: normalize_text("line notes", input.notes.clone(), MAX_TEXT_CHARS)?,
        });
    }
    let now = now_millis();
    let mut batches = workspace.settlement_batches.clone();
    let record = BusinessSettlementBatchRecord {
        id: batch_id.clone(),
        workspace_id: workspace.id.clone(),
        contract_number,
        settlement_period,
        cadence: payload.batch.cadence.clone(),
        status: payload.batch.status.clone(),
        lines,
        notes,
        revision: existing.map_or(1, |batch| batch.revision + 1),
        created_at: existing.map_or(now, |batch| batch.created_at),
        updated_at: now,
        voided_at: None,
        voided_by: None,
        void_reason: String::new(),
    };
    if let Some(index) = batches.iter().position(|batch| batch.id == batch_id) {
        batches[index] = record;
    } else {
        batches.push(record);
    }
    persist_settlement_batches(transaction, &workspace, expected_revision, &batches, now)?;
    load_workspace(transaction, &workspace.id)
}

fn validate_settlement_line(input: &BusinessSettlementLineInput) -> Result<(), HostError> {
    if input.contract_quantity_millis <= 0 {
        return Err(HostError::validation(
            "contractQuantityMillis must be greater than zero",
        ));
    }
    for (field, value) in [
        ("cumulativeExecutedMillis", input.cumulative_executed_millis),
        ("currentExecutedMillis", input.current_executed_millis),
        ("cumulativeAcceptedMillis", input.cumulative_accepted_millis),
        ("currentAcceptedMillis", input.current_accepted_millis),
    ] {
        if value < 0 {
            return Err(HostError::validation(format!("{field} cannot be negative")));
        }
    }
    if input.current_settlement_millis <= 0 {
        return Err(HostError::validation(
            "currentSettlementMillis must be greater than zero",
        ));
    }
    if input.cumulative_executed_millis > input.contract_quantity_millis
        || input.current_executed_millis > input.cumulative_executed_millis
        || input.cumulative_accepted_millis > input.cumulative_executed_millis
        || input.current_accepted_millis > input.cumulative_accepted_millis
        || input.current_settlement_millis > input.cumulative_accepted_millis
        || input.current_settlement_millis > input.contract_quantity_millis
    {
        return Err(HostError::new(
            "BUSINESS_SETTLEMENT_QUANTITY_INVALID",
            "settlement quantities must satisfy current <= cumulative <= contract and settlement <= accepted",
            false,
        ));
    }
    Ok(())
}

fn void_settlement_batch(
    transaction: &Transaction<'_>,
    payload: &VoidBusinessSettlementBatchPayload,
    expected_revision: i64,
    context: &NormalizedContext,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, &context.project_id)?;
    let reason = normalize_required("reason", payload.reason.clone(), MAX_TEXT_CHARS)?;
    let mut batches = workspace.settlement_batches.clone();
    let batch = batches
        .iter_mut()
        .find(|batch| batch.id == payload.batch_id)
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_SETTLEMENT_BATCH_NOT_FOUND",
                "settlement batch does not exist in this workspace",
                false,
            )
        })?;
    if batch.status == BusinessSettlementBatchStatus::Voided {
        return Err(HostError::new(
            "BUSINESS_SETTLEMENT_BATCH_ALREADY_VOIDED",
            "settlement batch is already voided",
            false,
        ));
    }
    let now = now_millis();
    batch.status = BusinessSettlementBatchStatus::Voided;
    batch.revision += 1;
    batch.updated_at = now;
    batch.voided_at = Some(now);
    batch.voided_by = Some(context.actor_id.clone());
    batch.void_reason = reason;
    persist_settlement_batches(transaction, &workspace, expected_revision, &batches, now)?;
    load_workspace(transaction, &workspace.id)
}

fn persist_settlement_batches(
    transaction: &Transaction<'_>,
    workspace: &BusinessWorkspaceRecord,
    expected_revision: i64,
    batches: &[BusinessSettlementBatchRecord],
    now: i64,
) -> Result<(), HostError> {
    let changed = transaction
        .execute(
            "UPDATE business_workspaces
             SET settlement_batches_json = ?1, revision = revision + 1, updated_at = ?2
             WHERE id = ?3 AND revision = ?4",
            params![
                serde_json::to_string(batches).map_err(json_error)?,
                now,
                workspace.id,
                expected_revision,
            ],
        )
        .map_err(sql_error)?;
    ensure_changed(changed)
}

fn upsert_payment(
    transaction: &Transaction<'_>,
    payload: &UpsertBusinessPaymentPayload,
    expected_revision: i64,
    project_id: &str,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, project_id)?;
    let now = now_millis();
    if let Some(payment_id) = &payload.payment.id {
        let current = workspace
            .payments
            .iter()
            .find(|payment| payment.id == *payment_id)
            .ok_or_else(|| {
                HostError::new(
                    "BUSINESS_PAYMENT_NOT_FOUND",
                    "payment ID is server-owned and does not belong to this workspace",
                    false,
                )
            })?;
        let requested_cancel = current.status == BusinessPaymentStatus::Requested
            && payload.payment.status == BusinessPaymentStatus::Canceled;
        if current.status != BusinessPaymentStatus::Planned && !requested_cancel {
            return Err(HostError::new(
                "BUSINESS_PAYMENT_STATUS_MANAGED",
                "only planned payments can be edited; requested and receipt statuses are system managed",
                false,
            ));
        }
        validate_payment_transition(&current.status, &payload.payment.status)?;
        let captured_by_payment_request = workspace.documents.iter().any(|document| {
            document.kind == BusinessDocumentKind::PaymentRequest
                && document.status != BusinessDocumentStatus::Voided
                && document
                    .snapshot
                    .payment
                    .as_ref()
                    .is_some_and(|payment| payment.id == current.id)
        });
        if captured_by_payment_request {
            return Err(HostError::new(
                "BUSINESS_PAYMENT_REQUEST_FIELDS_FROZEN",
                if requested_cancel {
                    "void the active payment request document before canceling this requested payment"
                } else {
                    "payment plan is frozen after a payment request document captures it"
                },
                false,
            ));
        }
        let plan_fields_changed = current.label != payload.payment.label
            || current.amount_cents != payload.payment.amount_cents
            || current.due_at != payload.payment.due_at;
        if payload.payment.status == BusinessPaymentStatus::Canceled && plan_fields_changed {
            return Err(HostError::new(
                "BUSINESS_PAYMENT_CANCEL_FIELDS_INVALID",
                "cancel a payment plan without changing its label, amount or dueAt",
                false,
            ));
        }
        ensure_payment_within_contract(
            &workspace,
            Some(current.id.as_str()),
            payload.payment.amount_cents,
            &payload.payment.status,
        )?;
        let changed = transaction
            .execute(
                "UPDATE business_payments
                 SET label = ?1, amount_cents = ?2, due_at = ?3, occurred_at = NULL,
                     status = ?4, reference = ?5, notes = ?6,
                     revision = revision + 1, updated_at = ?7
                 WHERE id = ?8 AND workspace_id = ?9 AND revision = ?10",
                params![
                    payload.payment.label,
                    payload.payment.amount_cents,
                    payload.payment.due_at,
                    payment_status_to_db(&payload.payment.status),
                    payload.payment.reference,
                    payload.payment.notes,
                    now,
                    current.id,
                    workspace.id,
                    current.revision,
                ],
            )
            .map_err(sql_error)?;
        ensure_changed(changed)?;
    } else {
        if payload.payment.status != BusinessPaymentStatus::Planned {
            return Err(HostError::new(
                "BUSINESS_PAYMENT_STATUS_TRANSITION_INVALID",
                "new payment must start as planned",
                false,
            ));
        }
        ensure_payment_within_contract(
            &workspace,
            None,
            payload.payment.amount_cents,
            &payload.payment.status,
        )?;
        if workspace.payments.len() as i64 >= MAX_PAYMENTS_PER_WORKSPACE {
            return Err(HostError::new(
                "BUSINESS_PAYMENT_LIMIT_REACHED",
                "workspace payment limit has been reached",
                false,
            ));
        }
        transaction
            .execute(
                "INSERT INTO business_payments
                 (id, workspace_id, label, amount_cents, due_at, occurred_at, status,
                  reference, notes, revision, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, 'planned', ?6, ?7, 1, ?8, ?8)",
                params![
                    Uuid::new_v4().to_string(),
                    workspace.id,
                    payload.payment.label,
                    payload.payment.amount_cents,
                    payload.payment.due_at,
                    payload.payment.reference,
                    payload.payment.notes,
                    now,
                ],
            )
            .map_err(sql_error)?;
    }
    bump_workspace(transaction, &workspace.id, expected_revision, now)?;
    load_workspace(transaction, &workspace.id)
}

fn validate_payment_transition(
    current: &BusinessPaymentStatus,
    target: &BusinessPaymentStatus,
) -> Result<(), HostError> {
    let allowed = matches!(
        (current, target),
        (
            BusinessPaymentStatus::Planned,
            BusinessPaymentStatus::Planned | BusinessPaymentStatus::Canceled
        ) | (
            // A requested payment the customer will never pay can be canceled
            // once its payment request document has been voided; otherwise the
            // node (and archive preflight) would dead-lock forever.
            BusinessPaymentStatus::Requested,
            BusinessPaymentStatus::Canceled
        )
    );
    if allowed {
        Ok(())
    } else {
        Err(HostError::new(
            "BUSINESS_PAYMENT_STATUS_TRANSITION_INVALID",
            "only an unrequested planned payment can be edited or canceled",
            false,
        ))
    }
}

fn ensure_workspace_archivable(workspace: &BusinessWorkspaceRecord) -> Result<(), HostError> {
    let mut blockers = Vec::new();
    if workspace.current_documents.contract_document_id.is_none() {
        blockers.push("effective contract");
    }
    if workspace.current_documents.acceptance_document_id.is_none() {
        blockers.push("confirmed acceptance");
    }
    if workspace.financial_summary.contract_cents <= 0 {
        blockers.push("positive contract amount");
    }
    if workspace.financial_summary.received_cents != workspace.financial_summary.contract_cents {
        blockers.push("contract balance must be zero");
    }
    if workspace.financial_summary.received_cents > workspace.financial_summary.contract_cents
        || workspace.financial_summary.scheduled_cents > workspace.financial_summary.contract_cents
    {
        blockers.push("payments must not exceed contract amount");
    }
    if workspace.documents.iter().any(|document| {
        matches!(
            document.status,
            BusinessDocumentStatus::Draft
                | BusinessDocumentStatus::InReview
                | BusinessDocumentStatus::Approved
        )
    }) {
        blockers.push("pending document approvals");
    }
    if workspace.payments.iter().any(|payment| {
        matches!(
            payment.status,
            BusinessPaymentStatus::Planned
                | BusinessPaymentStatus::Requested
                | BusinessPaymentStatus::PartiallyReceived
        )
    }) {
        blockers.push("unsettled payments");
    }
    if workspace.lifecycle_stage != BusinessLifecycleStage::Paid {
        blockers.push("paid lifecycle stage");
    }
    if !blockers.is_empty() {
        Err(HostError::new(
            "BUSINESS_WORKSPACE_ARCHIVE_BLOCKED",
            format!(
                "business workspace cannot be archived; missing: {}",
                blockers.join(", ")
            ),
            false,
        ))
    } else {
        business_closure_service::ensure_closure_archivable(workspace)
    }
}
fn change_workspace_status(
    transaction: &Transaction<'_>,
    payload: &ChangeBusinessWorkspaceStatusPayload,
    expected_revision: i64,
    context: &NormalizedContext,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, &payload.workspace_id)?;
    ensure_workspace_owned(&workspace, expected_revision, &context.project_id)?;
    if workspace.status == payload.status {
        return Err(HostError::validation(
            "business workspace already has requested status",
        ));
    }
    if payload.status == BusinessWorkspaceStatus::Archived {
        ensure_workspace_archivable(&workspace)?;
    }
    let now = now_millis();
    // Archiving stamps the audit fields; unarchiving must preserve who
    // archived the workspace and when, so the history is never erased.
    let (archived_at, archived_by) = if payload.status == BusinessWorkspaceStatus::Archived {
        (Some(now), Some(context.actor_id.clone()))
    } else {
        (workspace.archived_at, workspace.archived_by.clone())
    };
    let changed = transaction
        .execute(
            "UPDATE business_workspaces
             SET status = ?1, archived_at = ?2, archived_by = ?3,
                 revision = revision + 1, updated_at = ?4
             WHERE id = ?5 AND revision = ?6",
            params![
                workspace_status_to_db(&payload.status),
                archived_at,
                archived_by,
                now,
                workspace.id,
                expected_revision,
            ],
        )
        .map_err(sql_error)?;
    ensure_changed(changed)?;
    load_workspace(transaction, &workspace.id)
}

fn bump_workspace(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    expected_revision: i64,
    now: i64,
) -> Result<(), HostError> {
    let changed = transaction
        .execute(
            "UPDATE business_workspaces
             SET revision = revision + 1, updated_at = ?1
             WHERE id = ?2 AND revision = ?3",
            params![now, workspace_id, expected_revision],
        )
        .map_err(sql_error)?;
    ensure_changed(changed)
}

fn mutate_closure_workspace(
    transaction: &Transaction<'_>,
    vault_root: &Path,
    workspace_id: &str,
    expected_revision: i64,
    context: &NormalizedContext,
    mutation: impl FnOnce(
        &Transaction<'_>,
        &Path,
        &BusinessWorkspaceRecord,
        i64,
    ) -> Result<(), HostError>,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let workspace = load_workspace(transaction, workspace_id)?;
    ensure_workspace_mutable(&workspace, expected_revision, &context.project_id)?;
    let now = now_millis();
    mutation(transaction, vault_root, &workspace, now)?;
    bump_workspace(transaction, &workspace.id, expected_revision, now)?;
    load_workspace(transaction, &workspace.id)
}

fn ensure_workspace_mutable(
    workspace: &BusinessWorkspaceRecord,
    expected_revision: i64,
    project_id: &str,
) -> Result<(), HostError> {
    ensure_workspace_owned(workspace, expected_revision, project_id)?;
    if workspace.status == BusinessWorkspaceStatus::Archived {
        return Err(HostError::new(
            "BUSINESS_WORKSPACE_ARCHIVED",
            "archived business workspace must be reopened before mutation",
            false,
        ));
    }
    Ok(())
}

fn ensure_workspace_owned(
    workspace: &BusinessWorkspaceRecord,
    expected_revision: i64,
    project_id: &str,
) -> Result<(), HostError> {
    if workspace.project_id != project_id {
        return Err(HostError::new(
            "BUSINESS_WORKSPACE_PROJECT_MISMATCH",
            "business workspace belongs to a different project",
            false,
        ));
    }
    if workspace.revision != expected_revision {
        return Err(HostError::conflict(format!(
            "business workspace revision is {}, request expected {}",
            workspace.revision, expected_revision
        )));
    }
    Ok(())
}

fn load_workspace(
    connection: &Connection,
    workspace_id: &str,
) -> Result<BusinessWorkspaceRecord, HostError> {
    let base = connection
        .query_row(
            "SELECT id, project_id, requirement_brief_id, requirement_brief_revision,
                    prefill_source_workspace_id, profile_json, settlement_batches_json,
                    status, archived_at, archived_by, revision, created_at, updated_at
             FROM business_workspaces WHERE id = ?1",
            [workspace_id],
            workspace_base_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| {
            HostError::new(
                "BUSINESS_WORKSPACE_NOT_FOUND",
                "business workspace does not exist",
                false,
            )
        })?;
    let documents = load_documents(connection, workspace_id)?;
    let template_versions = load_template_versions(connection, workspace_id)?;
    let acceptance_batches =
        load_acceptance_batches(connection, workspace_id, &base.project_id, &documents)?;
    let payments = load_payments(connection, workspace_id)?;
    let quote_confirmations = load_quote_confirmations(connection, workspace_id)?;
    let receipts = load_business_receipts(connection, workspace_id)?;
    let customer = business_closure_service::load_customer_for_workspace(connection, workspace_id)?;
    let milestones = business_closure_service::load_milestones(connection, workspace_id)?;
    let delivery_submissions =
        business_closure_service::load_submissions(connection, workspace_id)?;
    let invoices = business_closure_service::load_invoices(connection, workspace_id)?;
    let archive_snapshots =
        business_closure_service::load_archive_snapshots(connection, workspace_id)?;
    let mut profile = base.profile;
    business_closure_service::overlay_customer_profile(&mut profile, &customer);
    let mut workspace = BusinessWorkspaceRecord {
        id: base.id,
        project_id: base.project_id,
        customer_id: customer.id.clone(),
        customer,
        requirement_brief_id: base.requirement_brief_id,
        requirement_brief_revision: base.requirement_brief_revision,
        prefill_source_workspace_id: base.prefill_source_workspace_id,
        profile,
        documents,
        acceptance_batches,
        template_versions,
        payments,
        quote_confirmations,
        receipts,
        milestones,
        settlement_batches: base.settlement_batches,
        delivery_submissions,
        invoices,
        archive_snapshots,
        archive_integrity_status: BusinessArchiveIntegrityStatus::NotCaptured,
        status: base.status,
        archived_at: base.archived_at,
        archived_by: base.archived_by,
        lifecycle_stage: BusinessLifecycleStage::Draft,
        financial_summary: BusinessFinancialSummary::default(),
        current_documents: BusinessCurrentDocuments::default(),
        revision: base.revision,
        created_at: base.created_at,
        updated_at: base.updated_at,
    };
    enrich_workspace(&mut workspace);
    workspace.archive_integrity_status =
        business_closure_service::archive_integrity_status(&workspace);
    Ok(workspace)
}

fn load_template_versions(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<BusinessTemplateVersionRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, workspace_id, source_asset_id, source_sha256,
                    normalized_asset_id, normalized_sha256, template_key, mapping_version,
                    converter_engine, converter_version, converter_policy_version,
                    status, reviewed_by, reviewed_at, review_note,
                    revision, created_at, updated_at
             FROM business_template_versions
             WHERE workspace_id = ?1
             ORDER BY created_at ASC, id ASC",
        )
        .map_err(sql_error)?;
    let records = statement
        .query_map([workspace_id], template_version_from_row)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(records)
}

fn template_version_from_row(row: &Row<'_>) -> rusqlite::Result<BusinessTemplateVersionRecord> {
    let status: String = row.get(11)?;
    Ok(BusinessTemplateVersionRecord {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        source_asset_id: row.get(2)?,
        source_sha256: row.get(3)?,
        normalized_asset_id: row.get(4)?,
        normalized_sha256: row.get(5)?,
        template_key: row.get(6)?,
        mapping_version: row.get(7)?,
        converter_engine: row.get(8)?,
        converter_version: row.get(9)?,
        converter_policy_version: row.get(10)?,
        status: template_version_status_from_db(&status)?,
        reviewed_by: row.get(12)?,
        reviewed_at: row.get(13)?,
        review_note: row.get(14)?,
        revision: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

fn current_document(
    documents: &[BusinessDocumentRecord],
    kind: BusinessDocumentKind,
    status: BusinessDocumentStatus,
) -> Option<&BusinessDocumentRecord> {
    documents
        .iter()
        .filter(|document| document.kind == kind && document.status == status)
        .max_by_key(|document| document.sequence_number)
}

fn document_total_cents(document: &BusinessDocumentRecord) -> i64 {
    let profile = &document.snapshot.profile;
    profile
        .quotation_totals
        .as_ref()
        .map(|totals| totals.final_total_cents)
        .unwrap_or_else(|| {
            profile
                .line_items
                .iter()
                .fold(0_i64, |total, item| total.saturating_add(item.amount_cents))
                .saturating_sub(profile.project_discount_cents)
        })
}

fn derive_current_documents(documents: &[BusinessDocumentRecord]) -> BusinessCurrentDocuments {
    BusinessCurrentDocuments {
        quote_document_id: current_document(
            documents,
            BusinessDocumentKind::Quote,
            BusinessDocumentStatus::Generated,
        )
        .map(|document| document.id.clone()),
        contract_document_id: current_document(
            documents,
            BusinessDocumentKind::Contract,
            BusinessDocumentStatus::Effective,
        )
        .map(|document| document.id.clone()),
        payment_request_document_id: current_document(
            documents,
            BusinessDocumentKind::PaymentRequest,
            BusinessDocumentStatus::Generated,
        )
        .map(|document| document.id.clone()),
        acceptance_document_id: current_document(
            documents,
            BusinessDocumentKind::Acceptance,
            BusinessDocumentStatus::Effective,
        )
        .map(|document| document.id.clone()),
    }
}

fn derive_financial_summary(
    workspace: &BusinessWorkspaceRecord,
    current: &BusinessCurrentDocuments,
) -> BusinessFinancialSummary {
    let total_for = |document_id: &Option<String>| {
        document_id
            .as_deref()
            .and_then(|id| {
                workspace
                    .documents
                    .iter()
                    .find(|document| document.id == id)
            })
            .map(document_total_cents)
            .unwrap_or_default()
    };
    let scheduled_cents = workspace
        .payments
        .iter()
        .filter(|payment| payment.status != BusinessPaymentStatus::Canceled)
        .fold(0_i64, |total, payment| {
            total.saturating_add(payment.amount_cents)
        });
    let requested_cents = workspace
        .payments
        .iter()
        .filter(|payment| {
            matches!(
                payment.status,
                BusinessPaymentStatus::Requested
                    | BusinessPaymentStatus::PartiallyReceived
                    | BusinessPaymentStatus::Received
            )
        })
        .fold(0_i64, |total, payment| {
            total.saturating_add(payment.amount_cents)
        });
    let received_cents =
        workspace
            .receipts
            .iter()
            .fold(0_i64, |total, receipt| match receipt.kind {
                BusinessReceiptKind::Receipt => total.saturating_add(receipt.amount_cents),
                BusinessReceiptKind::Reversal => total.saturating_sub(receipt.amount_cents),
            });
    let contract_cents = total_for(&current.contract_document_id);
    BusinessFinancialSummary {
        quoted_cents: total_for(&current.quote_document_id),
        contract_cents,
        scheduled_cents,
        requested_cents,
        received_cents,
        // Never report a negative receivable: an over-collected or
        // contract-voided workspace clamps to zero instead of poisoning
        // customer-level ledger aggregation with negative amounts.
        outstanding_cents: contract_cents.saturating_sub(received_cents).max(0),
    }
}

fn derive_lifecycle_stage(
    workspace: &BusinessWorkspaceRecord,
    current: &BusinessCurrentDocuments,
    financial: &BusinessFinancialSummary,
) -> BusinessLifecycleStage {
    if workspace.status == BusinessWorkspaceStatus::Archived {
        BusinessLifecycleStage::Archived
    } else if current.acceptance_document_id.is_some()
        && financial.contract_cents > 0
        && financial.received_cents == financial.contract_cents
    {
        BusinessLifecycleStage::Paid
    } else if current.acceptance_document_id.is_some() {
        BusinessLifecycleStage::Accepted
    } else if current.payment_request_document_id.is_some()
        || workspace.payments.iter().any(|payment| {
            matches!(
                payment.status,
                BusinessPaymentStatus::Requested
                    | BusinessPaymentStatus::PartiallyReceived
                    | BusinessPaymentStatus::Received
            )
        })
    {
        BusinessLifecycleStage::PaymentRequested
    } else if current.contract_document_id.is_some() {
        BusinessLifecycleStage::Contracted
    } else if current.quote_document_id.is_some() {
        BusinessLifecycleStage::Quoted
    } else {
        BusinessLifecycleStage::Draft
    }
}

fn enrich_workspace(workspace: &mut BusinessWorkspaceRecord) {
    let current_documents = derive_current_documents(&workspace.documents);
    let financial_summary = derive_financial_summary(workspace, &current_documents);
    let lifecycle_stage = derive_lifecycle_stage(workspace, &current_documents, &financial_summary);
    workspace.current_documents = current_documents;
    workspace.financial_summary = financial_summary;
    workspace.lifecycle_stage = lifecycle_stage;
}

struct WorkspaceBase {
    id: String,
    project_id: String,
    requirement_brief_id: Option<String>,
    requirement_brief_revision: Option<i64>,
    prefill_source_workspace_id: Option<String>,
    profile: BusinessProfile,
    settlement_batches: Vec<BusinessSettlementBatchRecord>,
    status: BusinessWorkspaceStatus,
    archived_at: Option<i64>,
    archived_by: Option<String>,
    revision: i64,
    created_at: i64,
    updated_at: i64,
}

fn workspace_base_from_row(row: &Row<'_>) -> rusqlite::Result<WorkspaceBase> {
    let profile_json: String = row.get(5)?;
    let profile = from_json_column(&profile_json)?;
    let settlement_batches_json: String = row.get(6)?;
    let settlement_batches = from_json_column(&settlement_batches_json)?;
    let status_value: String = row.get(7)?;
    Ok(WorkspaceBase {
        id: row.get(0)?,
        project_id: row.get(1)?,
        requirement_brief_id: row.get(2)?,
        requirement_brief_revision: row.get(3)?,
        prefill_source_workspace_id: row.get(4)?,
        profile,
        settlement_batches,
        status: workspace_status_from_db(&status_value)?,
        archived_at: row.get(8)?,
        archived_by: row.get(9)?,
        revision: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn load_documents(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<BusinessDocumentRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, kind, sequence_number, document_number, title, template_key,
                    status, snapshot_json, output_asset_id, output_format,
                    source_asset_id, review_id, report_asset_id, evidence_json,
                    manual_waiver_json, voided_at, voided_by, void_reason,
                    approved_at, approved_by, generated_at, revision, created_at, updated_at,
                    acceptance_output_spec_id
             FROM business_documents WHERE workspace_id = ?1
             ORDER BY created_at ASC, id ASC",
        )
        .map_err(sql_error)?;
    let documents = statement
        .query_map([workspace_id], document_from_row)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(documents)
}

fn load_acceptance_batches(
    connection: &Connection,
    workspace_id: &str,
    project_id: &str,
    documents: &[BusinessDocumentRecord],
) -> Result<Vec<BusinessAcceptanceBatchRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, label, requirements_json, output_specs_json,
                    revision, created_at, updated_at
             FROM business_acceptance_batches WHERE workspace_id = ?1
             ORDER BY created_at ASC, id ASC",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map([workspace_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    rows.into_iter()
        .map(
            |(
                id,
                label,
                requirements_json,
                output_specs_json,
                revision,
                created_at,
                updated_at,
            )| {
                let requirements: Vec<BusinessAcceptanceRequirementRecord> =
                    serde_json::from_str(&requirements_json).map_err(json_error)?;
                let mut output_specs: Vec<BusinessAcceptanceOutputSpecRecord> =
                    serde_json::from_str(&output_specs_json).map_err(json_error)?;
                for output_spec in &mut output_specs {
                    if output_spec.output_code.is_empty() {
                        output_spec.output_code = format!("legacy-{}", output_spec.id);
                    }
                }
                let materials = load_acceptance_materials(connection, &id)?;
                let bindings = load_acceptance_material_bindings(connection, project_id, &id)?;
                let readiness = acceptance_readiness(&requirements, &bindings);
                let document_ids = documents
                    .iter()
                    .filter(|document| {
                        document.snapshot.acceptance_batch_id.as_deref() == Some(id.as_str())
                    })
                    .map(|document| document.id.clone())
                    .collect::<Vec<_>>();
                let linked_documents = output_specs
                    .iter()
                    .filter_map(|spec| acceptance_document_for_spec(documents, &id, spec))
                    .collect::<Vec<_>>();
                let all_outputs_prepared = linked_documents.len() == output_specs.len();
                let status = if all_outputs_prepared
                    && linked_documents.iter().all(|document| {
                        matches!(
                            document.status,
                            BusinessDocumentStatus::Generated | BusinessDocumentStatus::Effective
                        )
                    }) {
                    BusinessAcceptanceBatchStatus::Generated
                } else if all_outputs_prepared
                    && linked_documents.iter().all(|document| {
                        matches!(
                            document.status,
                            BusinessDocumentStatus::Approved
                                | BusinessDocumentStatus::Generated
                                | BusinessDocumentStatus::Effective
                        )
                    })
                {
                    BusinessAcceptanceBatchStatus::Approved
                } else if all_outputs_prepared {
                    BusinessAcceptanceBatchStatus::DocumentsPrepared
                } else {
                    BusinessAcceptanceBatchStatus::Collecting
                };
                Ok(BusinessAcceptanceBatchRecord {
                    id,
                    workspace_id: workspace_id.to_string(),
                    label,
                    requirements,
                    output_specs,
                    materials,
                    readiness,
                    document_ids,
                    status,
                    revision,
                    created_at,
                    updated_at,
                })
            },
        )
        .collect()
}

fn load_acceptance_materials(
    connection: &Connection,
    batch_id: &str,
) -> Result<Vec<BusinessAcceptanceMaterialRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, requirement_id, asset_id, kind, group_key, confirmed,
                    duplicate_of_material_id, notes, revision, created_at, updated_at
             FROM business_acceptance_materials WHERE batch_id = ?1
             ORDER BY created_at ASC, id ASC",
        )
        .map_err(sql_error)?;
    let materials = statement
        .query_map([batch_id], |row| {
            let kind: String = row.get(3)?;
            Ok(BusinessAcceptanceMaterialRecord {
                id: row.get(0)?,
                batch_id: batch_id.to_string(),
                requirement_id: row.get(1)?,
                asset_id: row.get(2)?,
                kind: acceptance_material_kind_from_db(&kind)?,
                group_key: row.get(4)?,
                confirmed: row.get(5)?,
                duplicate_of_material_id: row.get(6)?,
                notes: row.get(7)?,
                revision: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(materials)
}

fn acceptance_readiness(
    requirements: &[BusinessAcceptanceRequirementRecord],
    bindings: &[BusinessAcceptanceMaterialBinding],
) -> BusinessAcceptanceReadiness {
    let blockers = requirements
        .iter()
        .filter_map(|requirement| {
            let provided_group_count = bindings
                .iter()
                .filter(|binding| binding.requirement_id == requirement.id)
                .map(|binding| binding.group_key.as_str())
                .collect::<HashSet<_>>()
                .len() as u32;
            let missing_group_count = requirement
                .required_group_count
                .saturating_sub(provided_group_count);
            (missing_group_count > 0).then(|| BusinessAcceptanceBlocker {
                code: "missingMaterialGroups".to_string(),
                requirement_id: requirement.id.clone(),
                requirement_label: requirement.label.clone(),
                required_group_count: requirement.required_group_count,
                provided_group_count,
                missing_group_count,
            })
        })
        .collect::<Vec<_>>();
    BusinessAcceptanceReadiness {
        is_ready: blockers.is_empty(),
        blockers,
    }
}

fn document_from_row(row: &Row<'_>) -> rusqlite::Result<BusinessDocumentRecord> {
    let kind: String = row.get(1)?;
    let status: String = row.get(6)?;
    let snapshot_json: String = row.get(7)?;
    let format: Option<String> = row.get(9)?;
    let mut snapshot: BusinessDocumentSnapshot = from_json_column(&snapshot_json)?;
    if snapshot.acceptance_output_spec_id.is_none() {
        snapshot.acceptance_output_spec_id = row.get(24)?;
    }
    Ok(BusinessDocumentRecord {
        id: row.get(0)?,
        kind: document_kind_from_db(&kind)?,
        sequence_number: row.get(2)?,
        document_number: row.get(3)?,
        title: row.get(4)?,
        template_key: row.get(5)?,
        status: document_status_from_db(&status)?,
        snapshot,
        output_asset_id: row.get(8)?,
        output_format: format
            .map(|value| document_format_from_db(&value))
            .transpose()?,
        source_asset_id: row.get(10)?,
        review_id: row.get(11)?,
        report_asset_id: row.get(12)?,
        evidence: row
            .get::<_, Option<String>>(13)?
            .map(|json| from_json_column(&json))
            .transpose()?,
        manual_waiver: row
            .get::<_, Option<String>>(14)?
            .map(|json| from_json_column(&json))
            .transpose()?,
        voided_at: row.get(15)?,
        voided_by: row.get(16)?,
        void_reason: row.get(17)?,
        approved_at: row.get(18)?,
        approved_by: row.get(19)?,
        generated_at: row.get(20)?,
        revision: row.get(21)?,
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
    })
}

fn load_payments(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<BusinessPaymentRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, label, amount_cents, due_at, occurred_at, status,
                    reference, notes, revision, created_at, updated_at
             FROM business_payments WHERE workspace_id = ?1
             ORDER BY created_at ASC, id ASC",
        )
        .map_err(sql_error)?;
    let payments = statement
        .query_map([workspace_id], payment_from_row)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(payments)
}

fn payment_from_row(row: &Row<'_>) -> rusqlite::Result<BusinessPaymentRecord> {
    let status: String = row.get(5)?;
    Ok(BusinessPaymentRecord {
        id: row.get(0)?,
        label: row.get(1)?,
        amount_cents: row.get(2)?,
        due_at: row.get(3)?,
        occurred_at: row.get(4)?,
        status: payment_status_from_db(&status)?,
        reference: row.get(6)?,
        notes: row.get(7)?,
        revision: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn load_quote_confirmations(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<BusinessQuoteConfirmationRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, quote_document_id, quote_document_revision, quote_asset_id,
                    quote_sha256, confirmation_version, customer_representative,
                    evidence_json, notes, confirmed_by, confirmed_at
             FROM business_quote_confirmations WHERE workspace_id = ?1
             ORDER BY confirmed_at ASC, id ASC",
        )
        .map_err(sql_error)?;
    let records = statement
        .query_map([workspace_id], |row| {
            let evidence_json: String = row.get(7)?;
            Ok(BusinessQuoteConfirmationRecord {
                id: row.get(0)?,
                quote_document_id: row.get(1)?,
                quote_document_revision: row.get(2)?,
                quote_asset_id: row.get(3)?,
                quote_sha256: row.get(4)?,
                confirmation_version: row.get(5)?,
                customer_representative: row.get(6)?,
                evidence: from_json_column(&evidence_json)?,
                notes: row.get(8)?,
                confirmed_by: row.get(9)?,
                confirmed_at: row.get(10)?,
            })
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(records)
}

fn load_business_receipts(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<BusinessReceiptRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, payment_id, kind, amount_cents, occurred_at, reference, notes,
                    reverses_receipt_id, evidence_json, recorded_by, created_at
             FROM business_receipts WHERE workspace_id = ?1
             ORDER BY occurred_at ASC, created_at ASC, id ASC",
        )
        .map_err(sql_error)?;
    let records = statement
        .query_map([workspace_id], |row| {
            let kind: String = row.get(2)?;
            Ok(BusinessReceiptRecord {
                id: row.get(0)?,
                payment_id: row.get(1)?,
                kind: receipt_kind_from_db(&kind)?,
                amount_cents: row.get(3)?,
                occurred_at: row.get(4)?,
                reference: row.get(5)?,
                notes: row.get(6)?,
                reverses_receipt_id: row.get(7)?,
                evidence: row
                    .get::<_, Option<String>>(8)?
                    .map(|json| from_json_column(&json))
                    .transpose()?,
                recorded_by: row.get(9)?,
                created_at: row.get(10)?,
            })
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(records)
}

fn command_reason(command_type: &str) -> &'static str {
    match command_type {
        "businessWorkspace.create" => "创建商务工作区",
        "businessWorkspace.updateProfile" => "更新商务主数据",
        "businessWorkspace.createDocument" => "创建商务单据",
        "businessWorkspace.promoteReviewedContract" => "晋升已审合同",
        "businessWorkspace.changeDocumentStatus" => "变更商务单据状态",
        "businessWorkspace.generateDocument" => "生成商务文件",
        "businessWorkspace.upsertPayment" => "登记付款计划",
        "businessWorkspace.confirmQuote" => "确认报价",
        "businessWorkspace.recordReceipt" => "登记到账",
        "businessWorkspace.reverseReceipt" => "冲正到账",
        "businessWorkspace.adoptLatestConfirmedRequirement" => "采用最新已确认需求",
        "businessWorkspace.upsertCustomer" => "更新客户主数据",
        "businessWorkspace.assignCustomer" => "更换关联客户",
        "businessWorkspace.upsertMilestone" => "更新交付里程碑",
        "businessWorkspace.createAcceptanceBatch" => "创建验收批次",
        "businessWorkspace.prepareAcceptanceDocuments" => "准备验收文件",
        "businessWorkspace.upsertAcceptanceMaterial" => "更新验收素材",
        "businessWorkspace.registerDeliverableVersion" => "登记交付物版本",
        "businessWorkspace.recordDeliverySent" => "登记交付发送",
        "businessWorkspace.recordDeliverySignoff" => "登记客户签收",
        "businessWorkspace.recordInvoiceIssued" => "登记开票",
        "businessWorkspace.recordInvoiceRedCorrection" => "登记发票红冲",
        "businessWorkspace.attachInvoiceAsset" => "补充发票附件",
        "businessWorkspace.createArchiveSnapshot" => "生成归档完整性快照",
        "businessWorkspace.normalizeLegacyTemplate" => "规范化历史 Word 模板",
        "businessWorkspace.approveTemplateVersion" => "批准模板版本",
        "businessWorkspace.rejectTemplateVersion" => "拒绝模板版本",
        "businessWorkspace.changeStatus" => "变更商务工作区状态",
        _ => "执行商务系统命令",
    }
}
fn append_event(
    transaction: &Transaction<'_>,
    event_type: BusinessWorkspaceEventType,
    workspace: &BusinessWorkspaceRecord,
    meta: &CommandMeta,
    command_type: &str,
) -> Result<BusinessWorkspaceDomainEvent, HostError> {
    let event_id = Uuid::new_v4().to_string();
    let occurred_at = now_millis();
    let reason = command_reason(command_type);
    let payload_json = serialize_business_journal(workspace)?;
    let journal_workspace = serde_json::from_str(&payload_json).map_err(json_error)?;
    transaction
        .execute(
            "INSERT INTO business_workspace_events
             (event_id, event_type, aggregate_id, revision, occurred_at, trace_id,
              actor_id, command_id, reason, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                event_id,
                event_type_to_db(&event_type),
                workspace.id,
                workspace.revision,
                occurred_at,
                meta.context.trace_id,
                meta.context.actor_id,
                meta.command_id,
                reason,
                payload_json,
            ],
        )
        .map_err(sql_error)?;
    Ok(BusinessWorkspaceDomainEvent {
        sequence: transaction.last_insert_rowid(),
        event_id,
        event_type,
        aggregate_id: workspace.id.clone(),
        revision: workspace.revision,
        occurred_at,
        trace_id: meta.context.trace_id.clone(),
        actor_id: meta.context.actor_id.clone(),
        command_id: meta.command_id.clone(),
        reason: reason.to_string(),
        business_workspace: journal_workspace,
    })
}

fn load_event(
    connection: &Connection,
    sequence: i64,
) -> Result<BusinessWorkspaceDomainEvent, HostError> {
    connection
        .query_row(
            "SELECT sequence, event_id, event_type, aggregate_id, revision,
                    occurred_at, trace_id, actor_id, command_id, reason, payload_json
             FROM business_workspace_events WHERE sequence = ?1",
            [sequence],
            event_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::internal("committed business workspace event is missing"))
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<BusinessWorkspaceDomainEvent> {
    let event_type: String = row.get(2)?;
    let payload_json: String = row.get(10)?;
    Ok(BusinessWorkspaceDomainEvent {
        sequence: row.get(0)?,
        event_id: row.get(1)?,
        event_type: event_type_from_db(&event_type)?,
        aggregate_id: row.get(3)?,
        revision: row.get(4)?,
        occurred_at: row.get(5)?,
        trace_id: row.get(6)?,
        actor_id: row.get(7)?,
        command_id: row.get(8)?,
        reason: row.get(9)?,
        business_workspace: from_json_column(&payload_json)?,
    })
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
) -> Result<Option<BusinessWorkspaceCommandResponse>, HostError> {
    let by_key = load_receipt(connection, "idempotency_key", idempotency_key)?;
    let by_command = load_receipt(connection, "command_id", command_id)?;
    if by_key
        .as_ref()
        .is_some_and(|receipt| receipt.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "IDEMPOTENCY_KEY_REUSED",
            "idempotencyKey reused for a different business workspace request",
            false,
        ));
    }
    if by_command
        .as_ref()
        .is_some_and(|receipt| receipt.fingerprint != fingerprint)
    {
        return Err(HostError::new(
            "COMMAND_ID_REUSED",
            "commandId reused for a different business workspace request",
            false,
        ));
    }
    if let (Some(left), Some(right)) = (&by_key, &by_command) {
        if left.command_id != right.command_id || left.idempotency_key != right.idempotency_key {
            return Err(HostError::new(
                "COMMAND_IDENTITY_COLLISION",
                "command identities resolve to different business workspace requests",
                false,
            ));
        }
    }
    by_key
        .or(by_command)
        .map(|receipt| {
            let mut response: BusinessWorkspaceCommandResponse =
                serde_json::from_str(&receipt.response_json).map_err(json_error)?;
            response.business_workspace =
                load_workspace(connection, &response.receipt.aggregate_id)?;
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
                 FROM business_workspace_command_receipts WHERE {column} = ?1"
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

fn command_fingerprint(command: &NormalizedCommand) -> Result<String, HostError> {
    let meta = command.meta();
    let payload = match command {
        NormalizedCommand::Create { meta, payload }
            if meta.protocol_version == BUSINESS_WORKSPACE_LEGACY_PROTOCOL_VERSION
                && payload.prefill_source_workspace_id.is_none() =>
        {
            // Protocol 1.4 did not serialize the optional prefill field. Preserve the
            // exact legacy command identity so durable receipts still replay after upgrade.
            serde_json::json!({ "projectId": payload.project_id })
        }
        NormalizedCommand::Create { meta, payload }
            if meta.protocol_version == BUSINESS_WORKSPACE_PREVIOUS_PROTOCOL_VERSION =>
        {
            // Protocol 1.5 did not serialize customerId.
            serde_json::json!({
                "projectId": payload.project_id,
                "prefillSourceWorkspaceId": payload.prefill_source_workspace_id,
            })
        }
        NormalizedCommand::Create { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::UpdateProfile { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::CreateDocument { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::CreateAcceptanceBatch { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::PrepareAcceptanceDocuments { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::UpsertAcceptanceMaterial { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::PromoteReviewedContract { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::ChangeDocumentStatus { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::GenerateDocument { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::UpsertPayment { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::UpsertSettlementBatch { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::VoidSettlementBatch { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::ConfirmQuote { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::RecordReceipt { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::ReverseReceipt { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::AdoptLatestConfirmedRequirement { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::UpsertCustomer { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::AssignCustomer { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::UpsertMilestone { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::RegisterDeliverableVersion { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::RecordDeliverySent { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::RecordDeliverySignoff { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::RecordInvoiceIssued { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::RecordInvoiceRedCorrection { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::AttachInvoiceAsset { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::CreateArchiveSnapshot { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::NormalizeLegacyTemplate { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::ApproveTemplateVersion { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::RejectTemplateVersion { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
        NormalizedCommand::ChangeStatus { payload, .. } => {
            serde_json::to_value(payload).map_err(json_error)?
        }
    };
    let value = serde_json::json!({
        "commandType": command.command_type(),
        "protocolVersion": meta.protocol_version,
        "context": {
            "actorId": meta.context.actor_id,
            "accountId": meta.context.account_id,
            "projectId": meta.context.project_id,
        },
        "expectedRevision": meta.expected_revision,
        "payload": payload,
    });
    let bytes = serde_json::to_vec(&value).map_err(json_error)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn cleanup_generated_asset(
    connection: &mut Connection,
    vault_root: &Path,
    asset_id: &str,
) -> Result<(), HostError> {
    let _original_path =
        match asset_service::resolve_storage_path_for_cleanup(connection, vault_root, asset_id) {
            Ok(path) => path,
            Err(error) if error.code == "ASSET_NOT_FOUND" => {
                retry_pending_generated_asset_deletes(connection, vault_root, Some(asset_id))?;
                return Ok(());
            }
            Err(error) => return Err(error),
        };
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let generated_asset = transaction
        .query_row(
            "SELECT origin.origin, asset.storage_rel_path
             FROM assets asset
             JOIN asset_origins origin ON origin.asset_id = asset.id
             WHERE asset.id = ?1
               AND origin.origin IN (
                   'businessDocument','generatedArchiveManifest','generatedArchivePackage',
                   'normalizedTemplate'
               )",
            [asset_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(sql_error)?;
    let Some((_origin, storage_rel_path)) = generated_asset else {
        transaction.commit().map_err(sql_error)?;
        return Ok(());
    };
    let linked: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM business_documents WHERE output_asset_id = ?1)
                 OR EXISTS(
                    SELECT 1 FROM business_archive_snapshots
                    WHERE manifest_asset_id = ?1 OR package_asset_id = ?1
                 )
                 OR EXISTS(
                    SELECT 1 FROM business_template_versions
                    WHERE normalized_asset_id = ?1
                 )",
            [asset_id],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    if linked {
        transaction.commit().map_err(sql_error)?;
        return Ok(());
    }
    let references: i64 = transaction
        .query_row(
            "SELECT (SELECT COUNT(*) FROM assets sibling
                     WHERE sibling.storage_rel_path = target.storage_rel_path)
             FROM assets target WHERE target.id = ?1",
            [asset_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::internal("generated asset disappeared during cleanup"))?;
    if references == 1 {
        let queued_at = now_millis();
        transaction
            .execute(
                "INSERT INTO business_generated_asset_gc
                 (asset_id, storage_rel_path, queued_at, attempts, last_error, updated_at)
                 VALUES (?1, ?2, ?3, 0, '', ?3)
                 ON CONFLICT(asset_id) DO UPDATE SET
                    storage_rel_path = excluded.storage_rel_path,
                    updated_at = excluded.updated_at",
                params![asset_id, storage_rel_path, queued_at],
            )
            .map_err(sql_error)?;
    }
    let changed = transaction
        .execute(
            "DELETE FROM assets
             WHERE id = ?1
               AND EXISTS(
                   SELECT 1 FROM asset_origins
                   WHERE asset_id = ?1
                     AND origin IN (
                         'businessDocument','generatedArchiveManifest','generatedArchivePackage',
                         'normalizedTemplate'
                     )
               )
               AND NOT EXISTS(
                   SELECT 1 FROM business_documents WHERE output_asset_id = ?1
               )
               AND NOT EXISTS(
                   SELECT 1 FROM business_archive_snapshots
                   WHERE manifest_asset_id = ?1 OR package_asset_id = ?1
               )
               AND NOT EXISTS(
                   SELECT 1 FROM business_template_versions
                   WHERE normalized_asset_id = ?1
               )",
            [asset_id],
        )
        .map_err(sql_error)?;
    if changed != 1 {
        return Err(HostError::conflict(
            "generated asset changed while cleanup was in progress",
        ));
    }

    // The authoritative database deletion (and optional durable GC record) must
    // commit before the Vault object is touched. A failed commit therefore
    // leaves both the Ready asset row and its file intact.
    if let Err(error) = transaction.commit() {
        let still_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM assets WHERE id = ?1)",
                [asset_id],
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        if still_exists {
            return Err(sql_error(error));
        }
    }

    if references == 1 {
        retry_pending_generated_asset_deletes(connection, vault_root, Some(asset_id))?;
    }
    Ok(())
}

fn retry_pending_generated_asset_deletes(
    connection: &mut Connection,
    vault_root: &Path,
    asset_id: Option<&str>,
) -> Result<usize, HostError> {
    let pending = if let Some(asset_id) = asset_id {
        connection
            .query_row(
                "SELECT asset_id, storage_rel_path
                 FROM business_generated_asset_gc WHERE asset_id = ?1",
                [asset_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(sql_error)?
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        let mut statement = connection
            .prepare(
                "SELECT asset_id, storage_rel_path
                 FROM business_generated_asset_gc ORDER BY updated_at ASC, asset_id ASC",
            )
            .map_err(sql_error)?;
        let collected = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        collected
    };

    let mut removed = 0;
    for (pending_asset_id, storage_rel_path) in pending {
        let referenced: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM assets WHERE storage_rel_path = ?1)",
                [&storage_rel_path],
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        if referenced {
            connection
                .execute(
                    "DELETE FROM business_generated_asset_gc WHERE asset_id = ?1",
                    [&pending_asset_id],
                )
                .map_err(sql_error)?;
            continue;
        }

        let deletion = resolve_pending_generated_asset_cleanup_path(vault_root, &storage_rel_path)
            .and_then(|path| match fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(HostError::new(
                    "VAULT_IO",
                    format!("remove orphan generated asset failed: {error}"),
                    true,
                )),
            });
        match deletion {
            Ok(()) => {
                connection
                    .execute(
                        "DELETE FROM business_generated_asset_gc WHERE asset_id = ?1",
                        [&pending_asset_id],
                    )
                    .map_err(sql_error)?;
                removed += 1;
            }
            Err(error) => {
                connection
                    .execute(
                        "UPDATE business_generated_asset_gc
                         SET attempts = attempts + 1, last_error = ?1, updated_at = ?2
                         WHERE asset_id = ?3",
                        params![error.to_string(), now_millis(), pending_asset_id],
                    )
                    .map_err(sql_error)?;
            }
        }
    }
    Ok(removed)
}

fn resolve_pending_generated_asset_cleanup_path(
    vault_root: &Path,
    storage_rel_path: &str,
) -> Result<PathBuf, HostError> {
    let relative = Path::new(storage_rel_path);
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(HostError::new(
            "VAULT_PATH_INVALID",
            "queued Vault cleanup path is not a safe relative path",
            false,
        ));
    }
    fs::create_dir_all(vault_root).map_err(|error| {
        HostError::new(
            "VAULT_IO",
            format!("prepare Vault root for generated asset cleanup failed: {error}"),
            true,
        )
    })?;
    let resolved_vault = fs::canonicalize(vault_root).map_err(|error| {
        HostError::new(
            "VAULT_IO",
            format!("resolve Vault root for generated asset cleanup failed: {error}"),
            true,
        )
    })?;
    let candidate = resolved_vault.join(relative);
    let parent = candidate
        .parent()
        .ok_or_else(|| HostError::internal("queued Vault cleanup path does not have a parent"))?;
    let resolved_parent = fs::canonicalize(parent).map_err(|error| {
        HostError::new(
            "VAULT_IO",
            format!("resolve queued Vault cleanup parent failed: {error}"),
            true,
        )
    })?;
    if !resolved_parent.starts_with(&resolved_vault) {
        return Err(HostError::new(
            "VAULT_PATH_INVALID",
            "queued Vault cleanup path escapes the Vault root",
            false,
        ));
    }
    match fs::symlink_metadata(&candidate) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(HostError::new(
            "VAULT_PATH_INVALID",
            "queued Vault cleanup path cannot be a symbolic link",
            false,
        )),
        Ok(_) => {
            let resolved = fs::canonicalize(&candidate).map_err(|error| {
                HostError::new(
                    "VAULT_IO",
                    format!("resolve queued Vault cleanup path failed: {error}"),
                    true,
                )
            })?;
            if !resolved.starts_with(&resolved_vault) {
                return Err(HostError::new(
                    "VAULT_PATH_INVALID",
                    "queued Vault cleanup path escapes the Vault root",
                    false,
                ));
            }
            Ok(resolved)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(candidate),
        Err(error) => Err(HostError::new(
            "VAULT_IO",
            format!("inspect queued Vault cleanup path failed: {error}"),
            true,
        )),
    }
}

fn validate_deadline(deadline_at: Option<i64>) -> Result<(), HostError> {
    if deadline_at.is_some_and(|deadline| deadline < now_millis()) {
        Err(HostError::new(
            "COMMAND_DEADLINE_EXCEEDED",
            "business workspace command deadline has elapsed",
            false,
        ))
    } else {
        Ok(())
    }
}

fn validate_timestamp(field: &str, value: Option<i64>) -> Result<(), HostError> {
    if value.is_some_and(|timestamp| timestamp <= 0) {
        Err(HostError::validation(format!(
            "{field} must be a positive timestamp"
        )))
    } else {
        Ok(())
    }
}

fn normalize_required(field: &str, value: String, max: usize) -> Result<String, HostError> {
    let value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > max {
        Err(HostError::validation(format!(
            "{field} length must be 1..{max}"
        )))
    } else {
        Ok(value)
    }
}

fn normalize_text(field: &str, value: String, max: usize) -> Result<String, HostError> {
    let value = value.trim().to_string();
    if value.chars().count() > max {
        Err(HostError::validation(format!(
            "{field} exceeds {max} characters"
        )))
    } else {
        Ok(value)
    }
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

fn normalize_sha256(field: &str, value: String) -> Result<String, HostError> {
    let value = value.trim();
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(HostError::validation(format!(
            "{field} must be a 64-character hexadecimal SHA-256"
        )));
    }
    Ok(value.to_ascii_uppercase())
}

fn ensure_changed(changed: usize) -> Result<(), HostError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(HostError::conflict(
            "business workspace changed while command was being written",
        ))
    }
}

fn from_json_column<T: serde::de::DeserializeOwned>(value: &str) -> rusqlite::Result<T> {
    serde_json::from_str(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            value.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn workspace_status_to_db(status: &BusinessWorkspaceStatus) -> &'static str {
    match status {
        BusinessWorkspaceStatus::Active => "active",
        BusinessWorkspaceStatus::Archived => "archived",
    }
}

fn workspace_status_from_db(value: &str) -> rusqlite::Result<BusinessWorkspaceStatus> {
    match value {
        "active" => Ok(BusinessWorkspaceStatus::Active),
        "archived" => Ok(BusinessWorkspaceStatus::Archived),
        _ => Err(conversion_error("business workspace status", value)),
    }
}

fn document_kind_to_db(kind: &BusinessDocumentKind) -> &'static str {
    match kind {
        BusinessDocumentKind::Quote => "quote",
        BusinessDocumentKind::Contract => "contract",
        BusinessDocumentKind::PaymentRequest => "paymentRequest",
        BusinessDocumentKind::Acceptance => "acceptance",
    }
}

fn document_kind_from_db(value: &str) -> rusqlite::Result<BusinessDocumentKind> {
    match value {
        "quote" => Ok(BusinessDocumentKind::Quote),
        "contract" => Ok(BusinessDocumentKind::Contract),
        "paymentRequest" => Ok(BusinessDocumentKind::PaymentRequest),
        "acceptance" => Ok(BusinessDocumentKind::Acceptance),
        _ => Err(conversion_error("business document kind", value)),
    }
}

fn document_status_to_db(status: &BusinessDocumentStatus) -> &'static str {
    match status {
        BusinessDocumentStatus::Draft => "draft",
        BusinessDocumentStatus::InReview => "inReview",
        BusinessDocumentStatus::Approved => "approved",
        BusinessDocumentStatus::Generated => "generated",
        BusinessDocumentStatus::Effective => "effective",
        BusinessDocumentStatus::Voided => "voided",
    }
}

fn document_status_from_db(value: &str) -> rusqlite::Result<BusinessDocumentStatus> {
    match value {
        "draft" => Ok(BusinessDocumentStatus::Draft),
        "inReview" => Ok(BusinessDocumentStatus::InReview),
        "approved" => Ok(BusinessDocumentStatus::Approved),
        "generated" => Ok(BusinessDocumentStatus::Generated),
        "effective" => Ok(BusinessDocumentStatus::Effective),
        "voided" => Ok(BusinessDocumentStatus::Voided),
        _ => Err(conversion_error("business document status", value)),
    }
}

fn document_format_to_db(format: &BusinessDocumentFormat) -> &'static str {
    match format {
        BusinessDocumentFormat::Docx => "docx",
        BusinessDocumentFormat::Xlsx => "xlsx",
    }
}

fn document_format_from_db(value: &str) -> rusqlite::Result<BusinessDocumentFormat> {
    match value {
        "docx" => Ok(BusinessDocumentFormat::Docx),
        "xlsx" => Ok(BusinessDocumentFormat::Xlsx),
        _ => Err(conversion_error("business document format", value)),
    }
}

fn payment_status_to_db(status: &BusinessPaymentStatus) -> &'static str {
    match status {
        BusinessPaymentStatus::Planned => "planned",
        BusinessPaymentStatus::Requested => "requested",
        BusinessPaymentStatus::PartiallyReceived => "partiallyReceived",
        BusinessPaymentStatus::Received => "received",
        BusinessPaymentStatus::Canceled => "canceled",
    }
}

fn payment_status_from_db(value: &str) -> rusqlite::Result<BusinessPaymentStatus> {
    match value {
        "planned" => Ok(BusinessPaymentStatus::Planned),
        "requested" => Ok(BusinessPaymentStatus::Requested),
        "partiallyReceived" => Ok(BusinessPaymentStatus::PartiallyReceived),
        "received" => Ok(BusinessPaymentStatus::Received),
        "canceled" => Ok(BusinessPaymentStatus::Canceled),
        _ => Err(conversion_error("business payment status", value)),
    }
}

fn receipt_kind_to_db(kind: &BusinessReceiptKind) -> &'static str {
    match kind {
        BusinessReceiptKind::Receipt => "receipt",
        BusinessReceiptKind::Reversal => "reversal",
    }
}

fn receipt_kind_from_db(value: &str) -> rusqlite::Result<BusinessReceiptKind> {
    match value {
        "receipt" => Ok(BusinessReceiptKind::Receipt),
        "reversal" => Ok(BusinessReceiptKind::Reversal),
        _ => Err(conversion_error("business receipt kind", value)),
    }
}

fn acceptance_material_kind_to_db(kind: &BusinessAcceptanceMaterialKind) -> &'static str {
    match kind {
        BusinessAcceptanceMaterialKind::Script => "script",
        BusinessAcceptanceMaterialKind::Video => "video",
        BusinessAcceptanceMaterialKind::Screenshot => "screenshot",
        BusinessAcceptanceMaterialKind::BehindTheScenes => "behindTheScenes",
        BusinessAcceptanceMaterialKind::PublishingData => "publishingData",
        BusinessAcceptanceMaterialKind::Invoice => "invoice",
        BusinessAcceptanceMaterialKind::Proof => "proof",
        BusinessAcceptanceMaterialKind::Other => "other",
    }
}

fn acceptance_material_kind_from_db(
    value: &str,
) -> rusqlite::Result<BusinessAcceptanceMaterialKind> {
    match value {
        "script" => Ok(BusinessAcceptanceMaterialKind::Script),
        "video" => Ok(BusinessAcceptanceMaterialKind::Video),
        "screenshot" => Ok(BusinessAcceptanceMaterialKind::Screenshot),
        "behindTheScenes" => Ok(BusinessAcceptanceMaterialKind::BehindTheScenes),
        "publishingData" => Ok(BusinessAcceptanceMaterialKind::PublishingData),
        "invoice" => Ok(BusinessAcceptanceMaterialKind::Invoice),
        "proof" => Ok(BusinessAcceptanceMaterialKind::Proof),
        "other" => Ok(BusinessAcceptanceMaterialKind::Other),
        _ => Err(conversion_error("business acceptance material kind", value)),
    }
}

fn acceptance_asset_kind_matches_requirement(
    asset_kind: &str,
    requirement_kind: &BusinessAcceptanceMaterialKind,
) -> bool {
    match requirement_kind {
        BusinessAcceptanceMaterialKind::Script | BusinessAcceptanceMaterialKind::Invoice => {
            asset_kind == "document"
        }
        BusinessAcceptanceMaterialKind::Video => asset_kind == "video",
        BusinessAcceptanceMaterialKind::Screenshot => asset_kind == "image",
        BusinessAcceptanceMaterialKind::BehindTheScenes => {
            matches!(asset_kind, "image" | "video")
        }
        BusinessAcceptanceMaterialKind::PublishingData | BusinessAcceptanceMaterialKind::Proof => {
            matches!(asset_kind, "document" | "image")
        }
        BusinessAcceptanceMaterialKind::Other => asset_kind == "other",
    }
}

fn template_version_status_to_db(status: &BusinessTemplateVersionStatus) -> &'static str {
    match status {
        BusinessTemplateVersionStatus::PendingReview => "pendingReview",
        BusinessTemplateVersionStatus::Approved => "approved",
        BusinessTemplateVersionStatus::Rejected => "rejected",
    }
}

fn template_version_status_from_db(value: &str) -> rusqlite::Result<BusinessTemplateVersionStatus> {
    match value {
        "pendingReview" => Ok(BusinessTemplateVersionStatus::PendingReview),
        "approved" => Ok(BusinessTemplateVersionStatus::Approved),
        "rejected" => Ok(BusinessTemplateVersionStatus::Rejected),
        _ => Err(conversion_error("business template version status", value)),
    }
}

fn event_type_to_db(event_type: &BusinessWorkspaceEventType) -> &'static str {
    match event_type {
        BusinessWorkspaceEventType::Created => "businessWorkspace.created",
        BusinessWorkspaceEventType::ProfileUpdated => "businessWorkspace.profileUpdated",
        BusinessWorkspaceEventType::DocumentCreated => "businessWorkspace.documentCreated",
        BusinessWorkspaceEventType::ReviewedContractPromoted => {
            "businessWorkspace.reviewedContractPromoted"
        }
        BusinessWorkspaceEventType::DocumentStatusChanged => {
            "businessWorkspace.documentStatusChanged"
        }
        BusinessWorkspaceEventType::DocumentGenerated => "businessWorkspace.documentGenerated",
        BusinessWorkspaceEventType::PaymentUpserted => "businessWorkspace.paymentUpserted",
        BusinessWorkspaceEventType::SettlementBatchUpserted => {
            "businessWorkspace.settlementBatchUpserted"
        }
        BusinessWorkspaceEventType::SettlementBatchVoided => {
            "businessWorkspace.settlementBatchVoided"
        }
        BusinessWorkspaceEventType::QuoteConfirmed => "businessWorkspace.quoteConfirmed",
        BusinessWorkspaceEventType::ReceiptRecorded => "businessWorkspace.receiptRecorded",
        BusinessWorkspaceEventType::ReceiptReversed => "businessWorkspace.receiptReversed",
        BusinessWorkspaceEventType::RequirementAdopted => "businessWorkspace.requirementAdopted",
        BusinessWorkspaceEventType::CustomerUpserted => "businessWorkspace.customerUpserted",
        BusinessWorkspaceEventType::CustomerAssigned => "businessWorkspace.customerAssigned",
        BusinessWorkspaceEventType::MilestoneUpserted => "businessWorkspace.milestoneUpserted",
        BusinessWorkspaceEventType::AcceptanceBatchCreated => {
            "businessWorkspace.acceptanceBatchCreated"
        }
        BusinessWorkspaceEventType::AcceptanceDocumentsPrepared => {
            "businessWorkspace.acceptanceDocumentsPrepared"
        }
        BusinessWorkspaceEventType::AcceptanceMaterialUpserted => {
            "businessWorkspace.acceptanceMaterialUpserted"
        }
        BusinessWorkspaceEventType::DeliverableVersionRegistered => {
            "businessWorkspace.deliverableVersionRegistered"
        }
        BusinessWorkspaceEventType::DeliverySent => "businessWorkspace.deliverySent",
        BusinessWorkspaceEventType::DeliverySignoffRecorded => {
            "businessWorkspace.deliverySignoffRecorded"
        }
        BusinessWorkspaceEventType::InvoiceIssued => "businessWorkspace.invoiceIssued",
        BusinessWorkspaceEventType::InvoiceRedCorrected => "businessWorkspace.invoiceRedCorrected",
        BusinessWorkspaceEventType::InvoiceAssetAttached => {
            "businessWorkspace.invoiceAssetAttached"
        }
        BusinessWorkspaceEventType::ArchiveSnapshotPrepared => {
            "businessWorkspace.archiveSnapshotPrepared"
        }
        BusinessWorkspaceEventType::TemplateVersionNormalized => {
            "businessWorkspace.templateVersionNormalized"
        }
        BusinessWorkspaceEventType::TemplateVersionApproved => {
            "businessWorkspace.templateVersionApproved"
        }
        BusinessWorkspaceEventType::TemplateVersionRejected => {
            "businessWorkspace.templateVersionRejected"
        }
        BusinessWorkspaceEventType::StatusChanged => "businessWorkspace.statusChanged",
    }
}

fn event_type_from_db(value: &str) -> rusqlite::Result<BusinessWorkspaceEventType> {
    match value {
        "businessWorkspace.created" => Ok(BusinessWorkspaceEventType::Created),
        "businessWorkspace.profileUpdated" => Ok(BusinessWorkspaceEventType::ProfileUpdated),
        "businessWorkspace.documentCreated" => Ok(BusinessWorkspaceEventType::DocumentCreated),
        "businessWorkspace.reviewedContractPromoted" => {
            Ok(BusinessWorkspaceEventType::ReviewedContractPromoted)
        }
        "businessWorkspace.documentStatusChanged" => {
            Ok(BusinessWorkspaceEventType::DocumentStatusChanged)
        }
        "businessWorkspace.documentGenerated" => Ok(BusinessWorkspaceEventType::DocumentGenerated),
        "businessWorkspace.paymentUpserted" => Ok(BusinessWorkspaceEventType::PaymentUpserted),
        "businessWorkspace.settlementBatchUpserted" => {
            Ok(BusinessWorkspaceEventType::SettlementBatchUpserted)
        }
        "businessWorkspace.settlementBatchVoided" => {
            Ok(BusinessWorkspaceEventType::SettlementBatchVoided)
        }
        "businessWorkspace.quoteConfirmed" => Ok(BusinessWorkspaceEventType::QuoteConfirmed),
        "businessWorkspace.receiptRecorded" => Ok(BusinessWorkspaceEventType::ReceiptRecorded),
        "businessWorkspace.receiptReversed" => Ok(BusinessWorkspaceEventType::ReceiptReversed),
        "businessWorkspace.requirementAdopted" => {
            Ok(BusinessWorkspaceEventType::RequirementAdopted)
        }
        "businessWorkspace.customerUpserted" => Ok(BusinessWorkspaceEventType::CustomerUpserted),
        "businessWorkspace.customerAssigned" => Ok(BusinessWorkspaceEventType::CustomerAssigned),
        "businessWorkspace.milestoneUpserted" => Ok(BusinessWorkspaceEventType::MilestoneUpserted),
        "businessWorkspace.acceptanceBatchCreated" => {
            Ok(BusinessWorkspaceEventType::AcceptanceBatchCreated)
        }
        "businessWorkspace.acceptanceDocumentsPrepared" => {
            Ok(BusinessWorkspaceEventType::AcceptanceDocumentsPrepared)
        }
        "businessWorkspace.acceptanceMaterialUpserted" => {
            Ok(BusinessWorkspaceEventType::AcceptanceMaterialUpserted)
        }
        "businessWorkspace.deliverableVersionRegistered" => {
            Ok(BusinessWorkspaceEventType::DeliverableVersionRegistered)
        }
        "businessWorkspace.deliverySent" => Ok(BusinessWorkspaceEventType::DeliverySent),
        "businessWorkspace.deliverySignoffRecorded" => {
            Ok(BusinessWorkspaceEventType::DeliverySignoffRecorded)
        }
        "businessWorkspace.invoiceIssued" => Ok(BusinessWorkspaceEventType::InvoiceIssued),
        "businessWorkspace.invoiceRedCorrected" => {
            Ok(BusinessWorkspaceEventType::InvoiceRedCorrected)
        }
        "businessWorkspace.invoiceAssetAttached" => {
            Ok(BusinessWorkspaceEventType::InvoiceAssetAttached)
        }
        "businessWorkspace.archiveSnapshotPrepared" => {
            Ok(BusinessWorkspaceEventType::ArchiveSnapshotPrepared)
        }
        "businessWorkspace.templateVersionNormalized" => {
            Ok(BusinessWorkspaceEventType::TemplateVersionNormalized)
        }
        "businessWorkspace.templateVersionApproved" => {
            Ok(BusinessWorkspaceEventType::TemplateVersionApproved)
        }
        "businessWorkspace.templateVersionRejected" => {
            Ok(BusinessWorkspaceEventType::TemplateVersionRejected)
        }
        "businessWorkspace.statusChanged" => Ok(BusinessWorkspaceEventType::StatusChanged),
        _ => Err(conversion_error("business workspace event type", value)),
    }
}

fn conversion_error(field: &str, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        value.len(),
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unknown {field}: {value}"),
        )),
    )
}

fn map_workspace_insert_error(error: rusqlite::Error) -> HostError {
    if is_constraint_error(&error) {
        HostError::new(
            "BUSINESS_WORKSPACE_EXISTS",
            "project already has a business workspace",
            false,
        )
    } else {
        sql_error(error)
    }
}

fn map_document_insert_error(error: rusqlite::Error) -> HostError {
    if is_constraint_error(&error) {
        HostError::new(
            "BUSINESS_DOCUMENT_NUMBER_EXISTS",
            "workspace already contains this document number",
            false,
        )
    } else {
        sql_error(error)
    }
}

fn map_acceptance_material_write_error(error: rusqlite::Error) -> HostError {
    if is_constraint_error(&error) {
        HostError::new(
            "BUSINESS_ACCEPTANCE_MATERIAL_CONFLICT",
            "acceptance batch already contains this asset or the duplicate reference is invalid",
            false,
        )
    } else {
        sql_error(error)
    }
}

fn is_constraint_error(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.code == rusqlite::ErrorCode::ConstraintViolation
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
        "SQLite business workspace operation failed: {error}"
    ))
}

fn json_error(error: serde_json::Error) -> HostError {
    HostError::internal(format!("JSON business workspace operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        AssetKind, BriefRecord, BusinessAcceptanceMaterialInput, BusinessAcceptanceOutputSpecInput,
        BusinessAcceptanceRequirementInput, BusinessProductionResultConfirmationDeliveryItem,
        BusinessProductionResultConfirmationShot, BusinessProductionResultConfirmationStoryboard,
        BusinessVideoCompletionAcceptanceAssetReference,
        BusinessVideoCompletionAcceptanceDeliveryGroup,
        BusinessVideoCompletionAcceptanceScreenshot, BusinessVideoCompletionAcceptanceVideo,
    };
    use std::fs::File;
    use std::io::Read;
    use zip::ZipArchive;

    fn external_qa_fixture(relative_path: &str) -> PathBuf {
        std::env::var_os("BSAIGC_EXTERNAL_QA_FIXTURE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("tests/fixtures/synthetic/business-v1"))
            .join(relative_path)
    }

    struct TestStore {
        temporary: tempfile::TempDir,
        database_path: std::path::PathBuf,
        vault_root: std::path::PathBuf,
        connection: Connection,
        project_id: String,
    }

    impl TestStore {
        fn new() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let database_path = temporary.path().join("ledger.sqlite3");
            let vault_root = temporary.path().join("vault");
            fs::create_dir_all(&vault_root).unwrap();
            let connection = Connection::open(&database_path).unwrap();
            connection
                .execute_batch(
                    r#"
                    PRAGMA foreign_keys = ON;
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
                    "#,
                )
                .unwrap();
            asset_service::migrate(&connection).unwrap();
            crate::requirement_brief_service::migrate(&connection).unwrap();
            migrate(&connection).unwrap();
            let project_id = Uuid::new_v4().to_string();
            connection
                .execute(
                    "INSERT INTO projects
                     (id, name, client_name, brief_json, stage, revision, created_at, updated_at)
                     VALUES (?1, 'Launch Project', 'Customer Co.', ?2, 'intake', 1, 1, 1)",
                    params![
                        project_id,
                        serde_json::to_string(&BriefRecord::default()).unwrap()
                    ],
                )
                .unwrap();
            let content = RequirementBriefContent {
                objective: "Launch the campaign".to_string(),
                key_message: "Make it memorable".to_string(),
                deliverables: vec!["Hero film".to_string(), "Social cut".to_string()],
                acceptance_criteria: vec!["Customer signs off".to_string()],
                constraints: vec!["Approved claims only".to_string()],
                risks: vec!["Weather".to_string()],
                deadline_at: Some(2_000_000_000_000),
                ..RequirementBriefContent::default()
            };
            connection
                .execute(
                    "INSERT INTO requirement_briefs
                     (id, project_id, question_set_version, answers_json, content_json,
                      status, confirmed_at, confirmed_by, revision, created_at, updated_at)
                     VALUES (?1, ?2, 'requirement-brief.v1', '[]', ?3,
                             'confirmed', 1, 'operator', 1, 1, 1)",
                    params![
                        Uuid::new_v4().to_string(),
                        project_id,
                        serde_json::to_string(&content).unwrap()
                    ],
                )
                .unwrap();
            Self {
                temporary,
                database_path,
                vault_root,
                connection,
                project_id,
            }
        }

        fn reopen(&mut self) {
            let replacement = Connection::open_in_memory().unwrap();
            drop(std::mem::replace(&mut self.connection, replacement));
            self.connection = Connection::open(&self.database_path).unwrap();
            self.connection
                .execute_batch("PRAGMA foreign_keys = ON;")
                .unwrap();
            asset_service::migrate(&self.connection).unwrap();
            migrate(&self.connection).unwrap();
        }

        fn execute(
            &mut self,
            command: BusinessWorkspaceCommandEnvelope,
        ) -> BusinessWorkspaceCommandOutcome {
            execute_command(&mut self.connection, &self.vault_root, command).unwrap()
        }
    }

    fn insert_project(
        connection: &Connection,
        project_name: &str,
        client_name: &str,
        confirmed: Option<RequirementBriefContent>,
    ) -> String {
        let project_id = Uuid::new_v4().to_string();
        connection
            .execute(
                "INSERT INTO projects
                 (id, name, client_name, brief_json, stage, revision, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'intake', 1, 1, 1)",
                params![
                    project_id,
                    project_name,
                    client_name,
                    serde_json::to_string(&BriefRecord::default()).unwrap()
                ],
            )
            .unwrap();
        if let Some(content) = confirmed {
            connection
                .execute(
                    "INSERT INTO requirement_briefs
                     (id, project_id, question_set_version, answers_json, content_json,
                      status, confirmed_at, confirmed_by, revision, created_at, updated_at)
                     VALUES (?1, ?2, 'requirement-brief.v1', '[]', ?3,
                             'confirmed', 1, 'operator', 1, 1, 1)",
                    params![
                        Uuid::new_v4().to_string(),
                        project_id,
                        serde_json::to_string(&content).unwrap()
                    ],
                )
                .unwrap();
        }
        project_id
    }
    fn context(project_id: &str) -> OperationContext {
        OperationContext {
            actor_id: "operator-local".to_string(),
            account_id: None,
            project_id: Some(project_id.to_string()),
            window_id: "main".to_string(),
            trace_id: Uuid::new_v4().to_string(),
        }
    }

    fn create_command(project_id: &str) -> BusinessWorkspaceCommandEnvelope {
        BusinessWorkspaceCommandEnvelope::Create {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: CreateBusinessWorkspacePayload {
                project_id: project_id.to_string(),
                customer_id: None,
                prefill_source_workspace_id: None,
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: None,
            deadline_at: Some(now_millis() + 60_000),
        }
    }

    fn create_command_with_prefill(
        project_id: &str,
        source_workspace_id: &str,
    ) -> BusinessWorkspaceCommandEnvelope {
        let mut command = create_command(project_id);
        let BusinessWorkspaceCommandEnvelope::Create { payload, .. } = &mut command else {
            unreachable!("create helper returned another command")
        };
        payload.prefill_source_workspace_id = Some(source_workspace_id.to_string());
        command
    }
    fn profile_input(profile: &BusinessProfile) -> BusinessProfileInput {
        BusinessProfileInput {
            project_title: profile.project_title.clone(),
            project_code: profile.project_code.clone(),
            customer_name: profile.customer_name.clone(),
            customer_legal_name: profile.customer_legal_name.clone(),
            customer_tax_id: profile.customer_tax_id.clone(),
            customer_address: profile.customer_address.clone(),
            customer_contact: profile.customer_contact.clone(),
            customer_phone: profile.customer_phone.clone(),
            customer_email: profile.customer_email.clone(),
            supplier_legal_name: profile.supplier_legal_name.clone(),
            supplier_tax_id: profile.supplier_tax_id.clone(),
            supplier_address: profile.supplier_address.clone(),
            supplier_contact: profile.supplier_contact.clone(),
            supplier_phone: profile.supplier_phone.clone(),
            supplier_bank_name: profile.supplier_bank_name.clone(),
            supplier_bank_account: profile.supplier_bank_account.clone(),
            currency: profile.currency.clone(),
            default_tax_rate_bps: profile.default_tax_rate_bps,
            tax_mode: profile.tax_mode,
            project_discount_cents: profile.project_discount_cents,
            service_start_at: profile.service_start_at,
            service_end_at: profile.service_end_at,
            delivery_summary: profile.delivery_summary.clone(),
            payment_terms: profile.payment_terms.clone(),
            acceptance_terms: profile.acceptance_terms.clone(),
            notes: profile.notes.clone(),
            line_items: profile
                .line_items
                .iter()
                .map(|item| BusinessLineItemInput {
                    id: Some(item.id.clone()),
                    name: item.name.clone(),
                    description: item.description.clone(),
                    quantity_millis: item.quantity_millis,
                    unit: item.unit.clone(),
                    unit_price_cents: item.unit_price_cents,
                    tax_rate_bps: item.tax_rate_bps,
                })
                .collect(),
        }
    }

    fn update_profile_command(
        project_id: &str,
        workspace: &BusinessWorkspaceRecord,
        profile: BusinessProfileInput,
    ) -> BusinessWorkspaceCommandEnvelope {
        BusinessWorkspaceCommandEnvelope::UpdateProfile {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: Box::new(UpdateBusinessProfilePayload {
                workspace_id: workspace.id.clone(),
                profile,
            }),
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        }
    }

    fn complete_profile_input(profile: &BusinessProfile) -> BusinessProfileInput {
        let mut input = profile_input(profile);
        input.supplier_legal_name = "BSAIGC Supplier Ltd.".to_string();
        input.supplier_tax_id = "SUPPLIER-TAX-001".to_string();
        input.supplier_address = "Supplier Street 2".to_string();
        input.supplier_contact = "Bob".to_string();
        input.supplier_phone = "10010".to_string();
        input.supplier_bank_name = "Test Bank".to_string();
        input.supplier_bank_account = "622200000001".to_string();
        input.customer_tax_id = "CUSTOMER-TAX-001".to_string();
        input.customer_address = "Customer Street 1".to_string();
        input.customer_contact = "Alice".to_string();
        input.customer_phone = "10086".to_string();
        input.customer_email = "alice@example.test".to_string();
        input.service_start_at = Some(1_700_000_000_000);
        input.service_end_at = Some(1_800_000_000_000);
        input.payment_terms = "Pay within 10 days".to_string();
        input.acceptance_terms = "Written sign-off".to_string();
        input.delivery_summary = "Final film and cutdowns".to_string();
        input.line_items[0].unit_price_cents = 10_000;
        input.line_items[0].tax_rate_bps = 600;
        input
    }

    fn status_command(
        project_id: &str,
        workspace: &BusinessWorkspaceRecord,
        document_id: &str,
        status: BusinessDocumentStatus,
    ) -> BusinessWorkspaceCommandEnvelope {
        let manual_waiver = if status == BusinessDocumentStatus::Effective {
            Some(BusinessManualWaiverInput {
                reason: "test-only approved waiver".to_string(),
            })
        } else {
            None
        };
        BusinessWorkspaceCommandEnvelope::ChangeDocumentStatus {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: ChangeBusinessDocumentStatusPayload {
                workspace_id: workspace.id.clone(),
                document_id: document_id.to_string(),
                status: status.clone(),
                reason: if status == BusinessDocumentStatus::Voided {
                    "test-only void reason".to_string()
                } else {
                    String::new()
                },
                evidence: None,
                manual_waiver,
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        }
    }

    fn test_document_template(kind: &BusinessDocumentKind) -> &'static str {
        match kind {
            BusinessDocumentKind::Quote => document_engine::QUOTE_TEMPLATE_KEY,
            BusinessDocumentKind::Contract => document_engine::CONTRACT_TEMPLATE_KEY,
            BusinessDocumentKind::PaymentRequest => document_engine::PAYMENT_REQUEST_TEMPLATE_KEY,
            BusinessDocumentKind::Acceptance => document_engine::ACCEPTANCE_TEMPLATE_KEY,
        }
    }

    fn create_test_document_command(
        project_id: &str,
        workspace: &BusinessWorkspaceRecord,
        kind: BusinessDocumentKind,
        payment_id: Option<String>,
    ) -> BusinessWorkspaceCommandEnvelope {
        BusinessWorkspaceCommandEnvelope::CreateDocument {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: CreateBusinessDocumentPayload {
                workspace_id: workspace.id.clone(),
                kind: kind.clone(),
                document_number: format!("TEST-{}", Uuid::new_v4().simple()),
                title: format!("{kind:?}"),
                template_key: test_document_template(&kind).to_string(),
                payment_id,
                acceptance_batch_id: None,
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        }
    }
    fn create_test_document(
        store: &mut TestStore,
        project_id: &str,
        workspace: BusinessWorkspaceRecord,
        kind: BusinessDocumentKind,
        payment_id: Option<String>,
    ) -> (BusinessWorkspaceRecord, String) {
        let document_number = format!("TEST-{}", Uuid::new_v4().simple());
        let workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::CreateDocument {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(project_id),
                payload: CreateBusinessDocumentPayload {
                    workspace_id: workspace.id.clone(),
                    kind: kind.clone(),
                    document_number: document_number.clone(),
                    title: format!("{kind:?}"),
                    template_key: test_document_template(&kind).to_string(),
                    payment_id,
                    acceptance_batch_id: None,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        let document_id = workspace
            .documents
            .iter()
            .find(|document| document.document_number == document_number)
            .unwrap()
            .id
            .clone();
        (workspace, document_id)
    }

    fn approve_and_generate_test_document(
        store: &mut TestStore,
        project_id: &str,
        mut workspace: BusinessWorkspaceRecord,
        document_id: &str,
        format: BusinessDocumentFormat,
    ) -> (
        BusinessWorkspaceRecord,
        BusinessWorkspaceCommandEnvelope,
        String,
    ) {
        workspace = store
            .execute(status_command(
                project_id,
                &workspace,
                document_id,
                BusinessDocumentStatus::InReview,
            ))
            .response
            .business_workspace;
        workspace = store
            .execute(status_command(
                project_id,
                &workspace,
                document_id,
                BusinessDocumentStatus::Approved,
            ))
            .response
            .business_workspace;
        let generate = BusinessWorkspaceCommandEnvelope::GenerateDocument {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: GenerateBusinessDocumentPayload {
                workspace_id: workspace.id.clone(),
                document_id: document_id.to_string(),
                format,
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        };
        workspace = store.execute(generate.clone()).response.business_workspace;
        let asset_id = workspace
            .documents
            .iter()
            .find(|document| document.id == document_id)
            .and_then(|document| document.output_asset_id.clone())
            .unwrap();
        (workspace, generate, asset_id)
    }

    fn confirm_generated_quote(
        store: &mut TestStore,
        project_id: &str,
        workspace: BusinessWorkspaceRecord,
        quote_document_id: &str,
        quote_asset_id: &str,
    ) -> BusinessWorkspaceRecord {
        store
            .execute(BusinessWorkspaceCommandEnvelope::ConfirmQuote {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(project_id),
                payload: ConfirmBusinessQuotePayload {
                    workspace_id: workspace.id.clone(),
                    quote_document_id: quote_document_id.to_string(),
                    confirmation_version: "test-customer-confirmation-v1".to_string(),
                    customer_representative: "Test Customer Representative".to_string(),
                    evidence: BusinessEvidenceInput {
                        asset_id: quote_asset_id.to_string(),
                        occurred_at: Some(now_millis()),
                        note: "test-only quote confirmation evidence".to_string(),
                    },
                    notes: "test-only confirmed quote".to_string(),
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace
    }

    fn create_and_generate_test_document(
        store: &mut TestStore,
        project_id: &str,
        workspace: BusinessWorkspaceRecord,
        kind: BusinessDocumentKind,
        payment_id: Option<String>,
        format: BusinessDocumentFormat,
    ) -> (
        BusinessWorkspaceRecord,
        BusinessWorkspaceCommandEnvelope,
        String,
        String,
    ) {
        let is_quote = kind == BusinessDocumentKind::Quote;
        let (workspace, document_id) =
            create_test_document(store, project_id, workspace, kind, payment_id);
        let (mut workspace, command, asset_id) =
            approve_and_generate_test_document(store, project_id, workspace, &document_id, format);
        if is_quote {
            workspace =
                confirm_generated_quote(store, project_id, workspace, &document_id, &asset_id);
        }
        (workspace, command, document_id, asset_id)
    }

    fn prepare_effective_contract(
        store: &mut TestStore,
        project_id: &str,
        mut workspace: BusinessWorkspaceRecord,
    ) -> BusinessWorkspaceRecord {
        workspace = store
            .execute(update_profile_command(
                project_id,
                &workspace,
                complete_profile_input(&workspace.profile),
            ))
            .response
            .business_workspace;
        let (next, _, _, _) = create_and_generate_test_document(
            store,
            project_id,
            workspace,
            BusinessDocumentKind::Quote,
            None,
            BusinessDocumentFormat::Xlsx,
        );
        let (next, _, contract_id, _) = create_and_generate_test_document(
            store,
            project_id,
            next,
            BusinessDocumentKind::Contract,
            None,
            BusinessDocumentFormat::Docx,
        );
        make_test_document_effective(store, project_id, next, &contract_id)
    }
    fn prepare_paid_and_accepted_workspace(
        store: &mut TestStore,
        project_id: &str,
    ) -> BusinessWorkspaceRecord {
        let workspace = store
            .execute(create_command(project_id))
            .response
            .business_workspace;
        let workspace = prepare_effective_contract(store, project_id, workspace);
        let contract_cents = workspace.financial_summary.contract_cents;
        let workspace = upsert_test_payment(
            store,
            project_id,
            workspace,
            None,
            contract_cents,
            BusinessPaymentStatus::Planned,
            None,
            "ARCHIVE-PLAN",
            "archive test payment",
        );
        let payment_id = workspace.payments[0].id.clone();
        let (workspace, _, _, _) = create_and_generate_test_document(
            store,
            project_id,
            workspace,
            BusinessDocumentKind::PaymentRequest,
            Some(payment_id.clone()),
            BusinessDocumentFormat::Docx,
        );
        let (workspace, _, acceptance_id, _) = create_and_generate_test_document(
            store,
            project_id,
            workspace,
            BusinessDocumentKind::Acceptance,
            None,
            BusinessDocumentFormat::Docx,
        );
        let workspace = make_test_document_effective(store, project_id, workspace, &acceptance_id);
        record_test_receipt(
            store,
            project_id,
            workspace,
            &payment_id,
            contract_cents,
            1_900_000_000_000,
            "ARCHIVE-BANK",
        )
    }
    fn make_test_document_effective(
        store: &mut TestStore,
        project_id: &str,
        workspace: BusinessWorkspaceRecord,
        document_id: &str,
    ) -> BusinessWorkspaceRecord {
        store
            .execute(status_command(
                project_id,
                &workspace,
                document_id,
                BusinessDocumentStatus::Effective,
            ))
            .response
            .business_workspace
    }

    #[allow(clippy::too_many_arguments)]
    fn upsert_test_payment(
        store: &mut TestStore,
        project_id: &str,
        workspace: BusinessWorkspaceRecord,
        id: Option<String>,
        amount_cents: i64,
        status: BusinessPaymentStatus,
        occurred_at: Option<i64>,
        reference: &str,
        notes: &str,
    ) -> BusinessWorkspaceRecord {
        store
            .execute(BusinessWorkspaceCommandEnvelope::UpsertPayment {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(project_id),
                payload: UpsertBusinessPaymentPayload {
                    workspace_id: workspace.id.clone(),
                    payment: BusinessPaymentInput {
                        id,
                        label: "Deposit".to_string(),
                        amount_cents,
                        due_at: Some(2_000_000_000_000),
                        occurred_at,
                        status,
                        reference: reference.to_string(),
                        notes: notes.to_string(),
                    },
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace
    }

    fn record_test_receipt_command(
        project_id: &str,
        workspace: &BusinessWorkspaceRecord,
        payment_id: &str,
        amount_cents: i64,
        occurred_at: i64,
        reference: &str,
    ) -> BusinessWorkspaceCommandEnvelope {
        BusinessWorkspaceCommandEnvelope::RecordReceipt {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: RecordBusinessReceiptPayload {
                workspace_id: workspace.id.clone(),
                payment_id: payment_id.to_string(),
                amount_cents,
                occurred_at,
                reference: reference.to_string(),
                notes: String::new(),
                evidence: None,
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        }
    }

    fn record_test_receipt(
        store: &mut TestStore,
        project_id: &str,
        workspace: BusinessWorkspaceRecord,
        payment_id: &str,
        amount_cents: i64,
        occurred_at: i64,
        reference: &str,
    ) -> BusinessWorkspaceRecord {
        store
            .execute(record_test_receipt_command(
                project_id,
                &workspace,
                payment_id,
                amount_cents,
                occurred_at,
                reference,
            ))
            .response
            .business_workspace
    }

    fn reverse_test_receipt_command(
        project_id: &str,
        workspace: &BusinessWorkspaceRecord,
        receipt_id: &str,
        amount_cents: i64,
        occurred_at: i64,
        reference: &str,
    ) -> BusinessWorkspaceCommandEnvelope {
        BusinessWorkspaceCommandEnvelope::ReverseReceipt {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: ReverseBusinessReceiptPayload {
                workspace_id: workspace.id.clone(),
                receipt_id: receipt_id.to_string(),
                amount_cents,
                occurred_at,
                reference: reference.to_string(),
                reason: "test receipt correction".to_string(),
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        }
    }

    fn import_test_asset(
        store: &mut TestStore,
        project_id: &str,
        file_name: &str,
        bytes: &[u8],
    ) -> String {
        let path = store
            .temporary
            .path()
            .join(format!("{}-{file_name}", Uuid::new_v4()));
        fs::write(&path, bytes).unwrap();
        asset_service::import_file(
            &mut store.connection,
            &store.vault_root,
            Some(project_id),
            &path,
        )
        .unwrap()
        .id
    }

    struct RegisteredProductionResultImage {
        source_index: usize,
        file_name: String,
        mime_type: String,
        width_px: u32,
        height_px: u32,
        bytes: Vec<u8>,
    }

    fn registered_production_result_images() -> Vec<RegisteredProductionResultImage> {
        let paths = [
            external_qa_fixture("scripts/synthetic-series-01.docx"),
            external_qa_fixture("scripts/synthetic-series-02.docx"),
            external_qa_fixture("scripts/synthetic-series-03.docx"),
        ];
        let mut seen = HashSet::new();
        let mut images = Vec::new();
        for path in paths {
            let file = File::open(path).unwrap();
            let mut archive = ZipArchive::new(file).unwrap();
            for index in 0..archive.len() {
                let mut entry = archive.by_index(index).unwrap();
                if entry.is_dir() || !entry.name().starts_with("word/media/") {
                    continue;
                }
                let extension = Path::new(entry.name())
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(str::to_ascii_lowercase);
                let mime_type = match extension.as_deref() {
                    Some("png") => "image/png",
                    Some("jpg" | "jpeg") => "image/jpeg",
                    _ => continue,
                };
                let file_name = Path::new(entry.name())
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap()
                    .to_string();
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).unwrap();
                let digest = format!("{:X}", Sha256::digest(&bytes));
                if !seen.insert(digest) {
                    continue;
                }
                let (width_px, height_px) =
                    video_completion_image_dimensions(mime_type, &bytes).unwrap();
                images.push(RegisteredProductionResultImage {
                    source_index: images.len(),
                    file_name,
                    mime_type: mime_type.to_string(),
                    width_px,
                    height_px,
                    bytes,
                });
            }
        }
        images.sort_by(|left, right| {
            let left_landscape = left.width_px >= left.height_px;
            let right_landscape = right.width_px >= right.height_px;
            right_landscape
                .cmp(&left_landscape)
                .then_with(|| right.bytes.len().cmp(&left.bytes.len()))
                .then_with(|| left.source_index.cmp(&right.source_index))
        });
        assert!(images.len() >= 60, "真实脚本图片不足 60 张");
        images.truncate(60);
        images
    }

    fn production_result_confirmation_data() -> BusinessProductionResultConfirmationData {
        let shot_counts = [14_usize, 14, 13, 13];
        let mut next_shot = 1_usize;
        let storyboards = shot_counts
            .into_iter()
            .enumerate()
            .map(|(storyboard_index, shot_count)| {
                let storyboard_number = format!("SB-{:02}", storyboard_index + 1);
                let shots = (0..shot_count)
                    .map(|_| {
                        let shot_number = format!("SHOT-{next_shot:02}");
                        next_shot += 1;
                        let asset_id = Uuid::new_v4().to_string();
                        BusinessProductionResultConfirmationShot {
                            shot_number: shot_number.clone(),
                            shot_description: format!("{shot_number} 画面描述"),
                            images: vec![BusinessProductionResultConfirmationAssetReference {
                                asset_id,
                                sha256: "A".repeat(64),
                                group_key: format!("{storyboard_number}/{shot_number}/image-1"),
                                file_name: format!("{shot_number}.png"),
                                caption: format!("{shot_number} 画面"),
                            }],
                        }
                    })
                    .collect();
                BusinessProductionResultConfirmationStoryboard {
                    storyboard_number: storyboard_number.clone(),
                    title: format!("脚本章节 {}", storyboard_index + 1),
                    description: format!("{storyboard_number} 制作说明"),
                    shots,
                }
            })
            .collect();
        BusinessProductionResultConfirmationData {
            attachment_label: "附件一".to_string(),
            contract_title: "白鹅潭瑞玺制作服务合同".to_string(),
            project_title: "白鹅潭瑞玺系列视频".to_string(),
            category: "视频制作".to_string(),
            payment_amount_cents: 9_752_000,
            contract_deliverable_summary: "完成四个章节、五十四个镜号的制作成果".to_string(),
            supplier_legal_name: "广州示例文化有限公司".to_string(),
            procurement_period: "2026-05-01 至 2026-07-20".to_string(),
            delivery_items: vec![BusinessProductionResultConfirmationDeliveryItem {
                item_key: "production-delivery-1".to_string(),
                title: "系列视频制作成果".to_string(),
                deliverable_summary: "按合同完成脚本、画面和成片制作".to_string(),
                evidence_images: Vec::new(),
                storyboards,
            }],
            acceptance_description: "成果内容和数量符合合同约定".to_string(),
            penalty_or_addition: "无".to_string(),
            completion_date: "2026-07-20".to_string(),
            acceptance_date: "2026-07-28".to_string(),
            clean_highlights_confirmed: true,
            manually_confirmed: true,
        }
    }

    #[test]
    fn production_result_confirmation_normalization_freezes_confirmed_54_shot_contract() {
        let normalized =
            normalize_production_result_confirmation_data(production_result_confirmation_data())
                .unwrap();
        assert_eq!(
            normalized
                .delivery_items
                .iter()
                .flat_map(|item| item.storyboards.iter())
                .count(),
            4
        );
        assert_eq!(
            normalized
                .delivery_items
                .iter()
                .flat_map(|item| item.storyboards.iter())
                .flat_map(|storyboard| storyboard.shots.iter())
                .count(),
            54
        );

        let mut unconfirmed = production_result_confirmation_data();
        unconfirmed.clean_highlights_confirmed = false;
        assert_eq!(
            normalize_production_result_confirmation_data(unconfirmed)
                .unwrap_err()
                .code,
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_HIGHLIGHT_CONFIRMATION_REQUIRED"
        );

        let mut duplicate = production_result_confirmation_data();
        let first = duplicate.delivery_items[0].storyboards[0].shots[0].images[0].clone();
        duplicate.delivery_items[0].storyboards[0].shots[1].images[0] = first;
        assert_eq!(
            normalize_production_result_confirmation_data(duplicate)
                .unwrap_err()
                .code,
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_MATERIAL_DUPLICATE"
        );
    }
    fn video_completion_acceptance_data(
        video_asset_id: &str,
        video_file_name: &str,
        video_sha256: &str,
        screenshot_asset_id: &str,
        screenshot_sha256: &str,
        manually_confirmed: bool,
    ) -> BusinessVideoCompletionAcceptanceData {
        BusinessVideoCompletionAcceptanceData {
            contract_title: "White Goose Pond annual production contract".to_string(),
            project_title: "White Goose Pond video delivery".to_string(),
            completion_date: "2026-07-29".to_string(),
            delivery_groups: vec![BusinessVideoCompletionAcceptanceDeliveryGroup {
                group_key: "delivery-group-1".to_string(),
                name: "Campaign film".to_string(),
                service_description: "Completed master video and evidence screenshots".to_string(),
                videos: vec![BusinessVideoCompletionAcceptanceVideo {
                    title: "Campaign master".to_string(),
                    video_type: "master".to_string(),
                    content: "Approved final campaign edit".to_string(),
                    duration: "00:30".to_string(),
                    asset_reference: BusinessVideoCompletionAcceptanceAssetReference {
                        asset_id: video_asset_id.to_string(),
                        file_name: video_file_name.to_string(),
                        sha256: video_sha256.to_string(),
                        external_link: Some("https://example.com/delivery/master".to_string()),
                    },
                    screenshots: vec![BusinessVideoCompletionAcceptanceScreenshot {
                        asset_id: screenshot_asset_id.to_string(),
                        sha256: screenshot_sha256.to_string(),
                        caption: "Opening frame".to_string(),
                    }],
                }],
            }],
            acceptance_conclusion: "Delivery accepted".to_string(),
            manually_confirmed,
        }
    }

    fn create_acceptance_batch_command(
        project_id: &str,
        workspace: &BusinessWorkspaceRecord,
    ) -> BusinessWorkspaceCommandEnvelope {
        let requirement_kinds = [
            BusinessAcceptanceMaterialKind::Video,
            BusinessAcceptanceMaterialKind::Script,
            BusinessAcceptanceMaterialKind::Screenshot,
            BusinessAcceptanceMaterialKind::BehindTheScenes,
            BusinessAcceptanceMaterialKind::PublishingData,
            BusinessAcceptanceMaterialKind::Proof,
        ];
        let requirements = requirement_kinds
            .into_iter()
            .enumerate()
            .map(|(index, kind)| BusinessAcceptanceRequirementInput {
                id: Some(Uuid::new_v4().to_string()),
                label: format!("内容槽位 {}", index + 1),
                kind,
                required_group_count: if index == 0 { 4 } else { 1 },
            })
            .collect::<Vec<_>>();
        let requirement_ids = requirements
            .iter()
            .map(|requirement| requirement.id.clone().unwrap())
            .collect::<Vec<_>>();
        BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: CreateBusinessAcceptanceBatchPayload {
                workspace_id: workspace.id.clone(),
                label: "白鹅潭验收批次".to_string(),
                requirements,
                output_specs: (0..5)
                    .map(|index| BusinessAcceptanceOutputSpecInput {
                        id: None,
                        output_code: format!("acceptance-output-{}", index + 1),
                        document_number: format!("BSE-ACC-{}", index + 1),
                        title: format!("白鹅潭验收文件 {}", index + 1),
                        template_key: document_engine::ACCEPTANCE_TEMPLATE_KEY.to_string(),
                        template_asset_id: None,
                        template_source_sha256: None,
                        template_mapping_version: String::new(),
                        contract_settlement: None,
                        service_settlement_items: Vec::new(),
                        payment_application: None,
                        video_completion_acceptance: None,
                        production_result_confirmation: None,
                        format: if index == 0 {
                            BusinessDocumentFormat::Xlsx
                        } else {
                            BusinessDocumentFormat::Docx
                        },
                        requirement_ids: requirement_ids.clone(),
                    })
                    .collect(),
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        }
    }

    fn bind_first_acceptance_template_source(
        command: &mut BusinessWorkspaceCommandEnvelope,
        template_key: &str,
        format: BusinessDocumentFormat,
        asset_id: String,
        sha256: String,
    ) {
        let BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch { payload, .. } = command
        else {
            unreachable!("acceptance batch helper returned another command")
        };
        let output = &mut payload.output_specs[0];
        output.template_key = template_key.to_string();
        output.output_code = match template_key {
            document_engine::BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY => {
                "contract-settlement".to_string()
            }
            document_engine::BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY => {
                "service-settlement-list".to_string()
            }
            document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY => {
                output.payment_application = Some(BusinessPaymentApplicationInput {
                    payment_id: Uuid::new_v4().to_string(),
                    contract_title: "Test contract".to_string(),
                    contract_number: "TEST-CONTRACT-001".to_string(),
                    work_summary: "completed test deliverables".to_string(),
                    payment_period_start: "2026-07-01".to_string(),
                    payment_period_end: "2026-07-31".to_string(),
                    settlement_period: "2026-07".to_string(),
                    payment_sequence: 1,
                    invoice_amount_cents: 10_000,
                    cumulative_recognized_amount_cents: 10_000,
                    withheld_amount_cents: 0,
                    application_date: "2026-07-29".to_string(),
                    supplier_bank_routing_number: "102100000001".to_string(),
                    settlement_items: vec![BusinessPaymentSettlementItemData {
                        name: "Test service".to_string(),
                        unit: "item".to_string(),
                        contract_unit_price_cents: 10_000,
                        original_quantity_millis: 1_000,
                        settlement_quantity_millis: 1_000,
                        remarks: String::new(),
                    }],
                });
                "payment-application-settlement-calculation".to_string()
            }
            _ => output.output_code.clone(),
        };
        output.format = format;
        output.template_asset_id = Some(asset_id);
        output.template_source_sha256 = Some(sha256);
        output.template_mapping_version =
            document_engine::expected_template_mapping_version(template_key)
                .unwrap_or("unexpected-test-map.v1")
                .to_string();
    }

    fn asset_sha256(store: &TestStore, asset_id: &str) -> String {
        store
            .connection
            .query_row(
                "SELECT sha256 FROM assets WHERE id = ?1",
                [asset_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn video_completion_acceptance_hydration_reads_only_images_and_failures_are_atomic() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let (mut workspace, _) = create_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::Quote,
            None,
        );
        let video_asset_id = import_test_asset(
            &mut store,
            &project_id,
            "delivery.mp4",
            b"\0\0\0\x18ftypisom\0\0\0\0\0\0\0\0",
        );
        let screenshot_asset_id = import_test_asset(
            &mut store,
            &project_id,
            "evidence.png",
            b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x02\0\0\0\x03",
        );
        let video_asset = asset_service::get_asset(&store.connection, &video_asset_id).unwrap();
        let screenshot_sha256 = asset_sha256(&store, &screenshot_asset_id);
        let frozen = video_completion_acceptance_data(
            &video_asset_id,
            &video_asset.original_name,
            &video_asset.sha256,
            &screenshot_asset_id,
            &screenshot_sha256,
            true,
        );
        let video_requirement_id = Uuid::new_v4().to_string();
        let screenshot_requirement_id = Uuid::new_v4().to_string();
        let requirements = vec![
            BusinessAcceptanceRequirementRecord {
                id: video_requirement_id.clone(),
                label: "Final video".to_string(),
                kind: BusinessAcceptanceMaterialKind::Video,
                required_group_count: 1,
            },
            BusinessAcceptanceRequirementRecord {
                id: screenshot_requirement_id.clone(),
                label: "Evidence screenshot".to_string(),
                kind: BusinessAcceptanceMaterialKind::Screenshot,
                required_group_count: 1,
            },
        ];
        let output_spec = BusinessAcceptanceOutputSpecRecord {
            id: Uuid::new_v4().to_string(),
            output_code: "video-completion-acceptance".to_string(),
            document_number: "BSE-VIDEO-ACCEPTANCE-001".to_string(),
            title: "Video completion acceptance".to_string(),
            template_key: document_engine::BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY
                .to_string(),
            template_asset_id: None,
            template_source_sha256: None,
            template_mapping_version: document_engine::expected_template_mapping_version(
                document_engine::BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY,
            )
            .unwrap()
            .to_string(),
            contract_settlement: None,
            service_settlement_items: Vec::new(),
            payment_application: None,
            video_completion_acceptance: Some(frozen.clone()),
            production_result_confirmation: None,
            format: BusinessDocumentFormat::Docx,
            requirement_ids: vec![
                video_requirement_id.clone(),
                screenshot_requirement_id.clone(),
            ],
        };
        let batch_id = Uuid::new_v4().to_string();
        store
            .connection
            .execute(
                "INSERT INTO business_acceptance_batches
                 (id, workspace_id, label, requirements_json, output_specs_json,
                  revision, created_at, updated_at)
                 VALUES (?1, ?2, 'video acceptance', ?3, ?4, 1, 10, 10)",
                params![
                    batch_id,
                    workspace.id,
                    serde_json::to_string(&requirements).unwrap(),
                    serde_json::to_string(&vec![output_spec.clone()]).unwrap(),
                ],
            )
            .unwrap();
        for (requirement_id, asset_id, kind, created_at) in [
            (
                video_requirement_id.as_str(),
                video_asset_id.as_str(),
                "video",
                11_i64,
            ),
            (
                screenshot_requirement_id.as_str(),
                screenshot_asset_id.as_str(),
                "screenshot",
                12_i64,
            ),
        ] {
            store
                .connection
                .execute(
                    "INSERT INTO business_acceptance_materials
                     (id, workspace_id, batch_id, requirement_id, asset_id, kind,
                      group_key, confirmed, duplicate_of_material_id, notes,
                      revision, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6,
                             'delivery-group-1', 1, NULL, '', 1, ?7, ?7)",
                    params![
                        Uuid::new_v4().to_string(),
                        workspace.id,
                        batch_id,
                        requirement_id,
                        asset_id,
                        kind,
                        created_at,
                    ],
                )
                .unwrap();
        }
        let material_bindings =
            load_acceptance_material_bindings(&store.connection, &project_id, &batch_id).unwrap();
        let batch = BusinessAcceptanceBatchRecord {
            id: batch_id.clone(),
            workspace_id: workspace.id.clone(),
            label: "video acceptance".to_string(),
            requirements,
            output_specs: vec![output_spec.clone()],
            materials: Vec::new(),
            readiness: BusinessAcceptanceReadiness {
                is_ready: true,
                blockers: Vec::new(),
            },
            document_ids: Vec::new(),
            status: BusinessAcceptanceBatchStatus::DocumentsPrepared,
            revision: 1,
            created_at: 10,
            updated_at: 10,
        };
        workspace.acceptance_batches.push(batch);
        let mut document = workspace.documents[0].clone();
        document.kind = BusinessDocumentKind::Acceptance;
        document.template_key =
            document_engine::BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY.to_string();
        document.snapshot.acceptance_batch_id = Some(batch_id);
        document.snapshot.acceptance_output_spec_id = Some(output_spec.id.clone());
        document.snapshot.acceptance_batch_revision = Some(1);
        document.snapshot.material_bindings = material_bindings;
        document.snapshot.video_completion_acceptance = Some(frozen.clone());

        let hydrated = load_video_completion_acceptance_generation_data(
            &store.connection,
            &store.vault_root,
            &project_id,
            &workspace,
            &document,
        )
        .unwrap()
        .unwrap();
        assert_eq!(hydrated.delivery_groups.len(), 1);
        assert_eq!(hydrated.delivery_groups[0].videos.len(), 1);
        let hydrated_video = &hydrated.delivery_groups[0].videos[0];
        assert_eq!(hydrated_video.asset_reference.asset_id, video_asset_id);
        assert_eq!(hydrated_video.screenshots[0].image_bytes.len(), 24);
        assert_eq!(hydrated_video.screenshots[0].mime_type, "image/png");
        assert_eq!(hydrated_video.screenshots[0].width_px, 2);
        assert_eq!(hydrated_video.screenshots[0].height_px, 3);

        let persisted_before = load_workspace(&store.connection, &workspace.id).unwrap();
        let assets_before =
            asset_service::list_assets(&store.connection, Some(&project_id)).unwrap();
        let mut unconfirmed_workspace = workspace.clone();
        let mut unconfirmed_document = document.clone();
        let mut unconfirmed = frozen;
        unconfirmed.manually_confirmed = false;
        unconfirmed_workspace.acceptance_batches[0].output_specs[0].video_completion_acceptance =
            Some(unconfirmed.clone());
        unconfirmed_document.snapshot.video_completion_acceptance = Some(unconfirmed);
        let error = load_video_completion_acceptance_generation_data(
            &store.connection,
            &store.vault_root,
            &project_id,
            &unconfirmed_workspace,
            &unconfirmed_document,
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "BUSINESS_VIDEO_COMPLETION_ACCEPTANCE_CONFIRMATION_REQUIRED"
        );
        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            persisted_before
        );
        assert_eq!(
            asset_service::list_assets(&store.connection, Some(&project_id)).unwrap(),
            assets_before
        );
    }

    #[test]
    #[ignore = "requires the local registered Baietan DOCX template and three real script DOCX fixtures"]
    fn production_result_confirmation_hydrates_real_60_assets_generates_docx_and_rejects_stale_or_mismatched_state_atomically(
    ) {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let mut workspace = prepare_effective_contract(&mut store, &project_id, workspace);
        let template_source_path =
            external_qa_fixture("templates/synthetic-production-confirmation.docx");
        if !template_source_path.is_file() {
            eprintln!(
                "external QA fixture unavailable; set BSAIGC_EXTERNAL_QA_FIXTURE_ROOT to run this ignored regression"
            );
            return;
        }
        let template_source = fs::read(&template_source_path).unwrap();
        let template_asset_id = import_test_asset(
            &mut store,
            &project_id,
            &template_source_path.file_name().unwrap().to_string_lossy(),
            &template_source,
        );
        let template_source_sha256 = asset_sha256(&store, &template_asset_id);
        assert!(template_source_sha256.eq_ignore_ascii_case(
            document_engine::expected_template_source_sha256(
                document_engine::BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY,
            )
            .unwrap()
        ));

        let requirement_id = Uuid::new_v4().to_string();
        let mut frozen = production_result_confirmation_data();
        let storyboards = std::mem::take(&mut frozen.delivery_items[0].storyboards);
        let evidence_labels = [
            "合成宣传片 A",
            "合成宣传片 B",
            "合成品牌片 A",
            "合成品牌片 B",
            "合成 AIGC 视频 A",
            "合成 AIGC 视频 B",
        ];
        let evidence_references = evidence_labels
            .iter()
            .enumerate()
            .map(
                |(index, label)| BusinessProductionResultConfirmationAssetReference {
                    asset_id: Uuid::new_v4().to_string(),
                    sha256: "A".repeat(64),
                    group_key: format!("delivery-evidence-{index}"),
                    file_name: format!("delivery-evidence-{index}.jpg"),
                    caption: (*label).to_string(),
                },
            )
            .collect::<Vec<_>>();
        frozen.delivery_items = vec![
            BusinessProductionResultConfirmationDeliveryItem {
                item_key: "production-delivery-long-film".to_string(),
                title: "长视频：合成宣传片 A、合成宣传片 B".to_string(),
                deliverable_summary:
                    "形象宣传片 30-60s，含创意策划、拍摄、剪辑、动画、演员、场地、道具和配音"
                        .to_string(),
                evidence_images: evidence_references[0..2].to_vec(),
                storyboards,
            },
            BusinessProductionResultConfirmationDeliveryItem {
                item_key: "production-delivery-brand-film".to_string(),
                title: "品牌类短视频：合成品牌片 A、合成品牌片 B".to_string(),
                deliverable_summary:
                    "轻量级品牌调性片 30-60s，含创意策划、脚本、拍摄、剪辑、花字、演员和配音"
                        .to_string(),
                evidence_images: evidence_references[2..4].to_vec(),
                storyboards: Vec::new(),
            },
            BusinessProductionResultConfirmationDeliveryItem {
                item_key: "production-delivery-aigc-film".to_string(),
                title: "AIGC 类：合成 AIGC 视频 A、合成 AIGC 视频 B".to_string(),
                deliverable_summary:
                    "AIGC 创意广告视频 30-60s，含脚本、分镜、效果渲染、剪辑、平面设计和配音"
                        .to_string(),
                evidence_images: evidence_references[4..6].to_vec(),
                storyboards: Vec::new(),
            },
        ];
        let registered_images = registered_production_result_images();
        let mut material_rows = Vec::new();
        let mut expected_images = Vec::new();
        let mut next_registered_image = 0_usize;
        let mut bind_reference =
            |reference: &mut BusinessProductionResultConfirmationAssetReference| {
                let registered = &registered_images[next_registered_image];
                next_registered_image += 1;
                let asset_id = import_test_asset(
                    &mut store,
                    &project_id,
                    &registered.file_name,
                    &registered.bytes,
                );
                let asset = asset_service::get_asset(&store.connection, &asset_id).unwrap();
                assert_eq!(asset.mime_type, registered.mime_type);
                reference.asset_id = asset.id.clone();
                reference.sha256 = asset.sha256.clone();
                reference.file_name = asset.original_name.clone();
                material_rows.push((asset.id, reference.group_key.clone()));
                expected_images.push((
                    reference.asset_id.clone(),
                    reference.sha256.clone(),
                    reference.file_name.clone(),
                    asset.mime_type,
                    registered.width_px,
                    registered.height_px,
                    registered.bytes.clone(),
                ));
            };
        for reference in frozen
            .delivery_items
            .iter_mut()
            .flat_map(|item| item.evidence_images.iter_mut())
        {
            bind_reference(reference);
        }
        for reference in frozen
            .delivery_items
            .iter_mut()
            .flat_map(|item| item.storyboards.iter_mut())
            .flat_map(|storyboard| storyboard.shots.iter_mut())
            .flat_map(|shot| shot.images.iter_mut())
        {
            bind_reference(reference);
        }
        assert_eq!(next_registered_image, 60);

        let output_spec_id = Uuid::new_v4().to_string();
        let mut create_batch = create_acceptance_batch_command(&project_id, &workspace);
        let BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch { payload, .. } =
            &mut create_batch
        else {
            unreachable!("acceptance batch helper returned another command")
        };
        payload.label = "production result confirmation".to_string();
        payload.requirements = vec![BusinessAcceptanceRequirementInput {
            id: Some(requirement_id.clone()),
            label: "Storyboard evidence images".to_string(),
            kind: BusinessAcceptanceMaterialKind::Screenshot,
            required_group_count: 60,
        }];
        payload.output_specs = vec![BusinessAcceptanceOutputSpecInput {
            id: Some(output_spec_id.clone()),
            output_code: "production-result-confirmation".to_string(),
            document_number: "BSE-PRODUCTION-RESULT-001".to_string(),
            title: "Production result confirmation".to_string(),
            template_key: document_engine::BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY
                .to_string(),
            template_asset_id: Some(template_asset_id),
            template_source_sha256: Some(template_source_sha256),
            template_mapping_version: document_engine::expected_template_mapping_version(
                document_engine::BAIETAN_PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_KEY,
            )
            .unwrap()
            .to_string(),
            contract_settlement: None,
            service_settlement_items: Vec::new(),
            payment_application: None,
            video_completion_acceptance: None,
            production_result_confirmation: Some(frozen.clone()),
            format: BusinessDocumentFormat::Docx,
            requirement_ids: vec![requirement_id.clone()],
        }];
        workspace = store.execute(create_batch).response.business_workspace;
        let batch_id = workspace
            .acceptance_batches
            .iter()
            .find(|batch| {
                batch
                    .output_specs
                    .iter()
                    .any(|output| output.id == output_spec_id)
            })
            .expect("created production result acceptance batch")
            .id
            .clone();
        assert_eq!(
            workspace
                .acceptance_batches
                .iter()
                .find(|batch| batch.id == batch_id)
                .unwrap()
                .revision,
            1
        );

        for (asset_id, group_key) in material_rows {
            let previous_workspace_revision = workspace.revision;
            workspace = store
                .execute(upsert_acceptance_material_command(
                    &project_id,
                    &workspace,
                    &batch_id,
                    BusinessAcceptanceMaterialInput {
                        id: None,
                        requirement_id: requirement_id.clone(),
                        asset_id,
                        kind: BusinessAcceptanceMaterialKind::Screenshot,
                        group_key,
                        confirmed: true,
                        duplicate_of_material_id: None,
                        notes: String::new(),
                    },
                ))
                .response
                .business_workspace;
            assert_eq!(workspace.revision, previous_workspace_revision + 1);
        }
        let batch_before_prepare = workspace
            .acceptance_batches
            .iter()
            .find(|batch| batch.id == batch_id)
            .unwrap()
            .clone();
        assert_eq!(batch_before_prepare.materials.len(), 60);
        assert!(batch_before_prepare.readiness.is_ready);
        assert_eq!(batch_before_prepare.revision, 61);

        workspace = store
            .execute(prepare_acceptance_documents_command(
                &project_id,
                &workspace,
                &batch_id,
            ))
            .response
            .business_workspace;
        let batch = workspace
            .acceptance_batches
            .iter()
            .find(|batch| batch.id == batch_id)
            .unwrap()
            .clone();
        assert_eq!(
            batch.status,
            BusinessAcceptanceBatchStatus::DocumentsPrepared
        );
        assert_eq!(batch.revision, batch_before_prepare.revision);
        let document = workspace
            .documents
            .iter()
            .find(|document| {
                document.snapshot.acceptance_batch_id.as_deref() == Some(batch_id.as_str())
                    && document.snapshot.acceptance_output_spec_id.as_deref()
                        == Some(output_spec_id.as_str())
            })
            .expect("prepared production result confirmation document")
            .clone();
        assert_eq!(document.status, BusinessDocumentStatus::Draft);
        assert_eq!(
            document.snapshot.acceptance_batch_revision,
            Some(batch.revision)
        );
        assert_eq!(document.snapshot.material_bindings.len(), 60);
        let frozen_snapshot = document
            .snapshot
            .production_result_confirmation
            .as_ref()
            .unwrap();
        assert_eq!(frozen_snapshot.delivery_items.len(), 3);
        assert_eq!(
            frozen_snapshot
                .delivery_items
                .iter()
                .map(|item| item.evidence_images.len())
                .sum::<usize>(),
            6
        );
        assert_eq!(
            frozen_snapshot
                .delivery_items
                .iter()
                .flat_map(|item| item.storyboards.iter())
                .count(),
            4
        );
        assert_eq!(
            frozen_snapshot
                .delivery_items
                .iter()
                .flat_map(|item| item.storyboards.iter())
                .flat_map(|storyboard| storyboard.shots.iter())
                .count(),
            54
        );

        let hydrated = load_production_result_confirmation_generation_data(
            &store.connection,
            &store.vault_root,
            &project_id,
            &workspace,
            &document,
        )
        .unwrap()
        .unwrap();
        assert_eq!(hydrated.storyboards.len(), 4);
        let hydrated_images = hydrated
            .delivery_items
            .iter()
            .flat_map(|item| item.images.iter())
            .chain(
                hydrated
                    .storyboards
                    .iter()
                    .flat_map(|storyboard| storyboard.shots.iter())
                    .flat_map(|shot| shot.images.iter()),
            )
            .collect::<Vec<_>>();
        assert_eq!(
            hydrated
                .storyboards
                .iter()
                .flat_map(|storyboard| storyboard.shots.iter())
                .count(),
            54
        );
        assert_eq!(expected_images.len(), 60);
        assert_eq!(hydrated_images.len(), expected_images.len());
        let expected_asset_ids = expected_images
            .iter()
            .map(|(asset_id, ..)| asset_id.clone())
            .collect::<HashSet<_>>();
        let hydrated_asset_ids = hydrated_images
            .iter()
            .map(|image| image.asset_id.clone())
            .collect::<HashSet<_>>();
        assert_eq!(expected_asset_ids.len(), expected_images.len());
        assert_eq!(hydrated_asset_ids, expected_asset_ids);
        for (
            expected_asset_id,
            expected_sha256,
            expected_file_name,
            expected_mime_type,
            expected_width_px,
            expected_height_px,
            expected_bytes,
        ) in &expected_images
        {
            let matching_images = hydrated_images
                .iter()
                .filter(|image| image.asset_id == *expected_asset_id)
                .copied()
                .collect::<Vec<_>>();
            assert_eq!(matching_images.len(), 1, "{expected_file_name}");
            let image = matching_images[0];
            assert!(
                image.sha256.eq_ignore_ascii_case(expected_sha256),
                "{expected_file_name}"
            );
            assert_eq!(&image.mime_type, expected_mime_type, "{expected_file_name}");
            assert_eq!(image.width_px, *expected_width_px, "{expected_file_name}");
            assert_eq!(image.height_px, *expected_height_px, "{expected_file_name}");
            assert_eq!(&image.image_bytes, expected_bytes, "{expected_file_name}");
        }

        let document_id = document.id.clone();
        for status in [
            BusinessDocumentStatus::InReview,
            BusinessDocumentStatus::Approved,
        ] {
            workspace = store
                .execute(status_command(
                    &project_id,
                    &workspace,
                    &document_id,
                    status,
                ))
                .response
                .business_workspace;
        }
        let assets_before_generation =
            asset_service::list_assets(&store.connection, Some(&project_id)).unwrap();
        let asset_ids_before_generation = assets_before_generation
            .iter()
            .map(|asset| asset.id.clone())
            .collect::<HashSet<_>>();
        workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::GenerateDocument {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: GenerateBusinessDocumentPayload {
                    workspace_id: workspace.id.clone(),
                    document_id: document_id.clone(),
                    format: BusinessDocumentFormat::Docx,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        let document = workspace
            .documents
            .iter()
            .find(|document| document.id == document_id)
            .expect("generated production result confirmation document")
            .clone();
        assert_eq!(document.status, BusinessDocumentStatus::Generated);
        assert_eq!(document.output_format, Some(BusinessDocumentFormat::Docx));
        let output_asset_id = document.output_asset_id.clone().unwrap();
        let assets_after_generation =
            asset_service::list_assets(&store.connection, Some(&project_id)).unwrap();
        let generated_assets = assets_after_generation
            .iter()
            .filter(|asset| !asset_ids_before_generation.contains(&asset.id))
            .collect::<Vec<_>>();
        assert_eq!(generated_assets.len(), 1);
        assert_eq!(generated_assets[0].id, output_asset_id);
        let (output_asset, output_path) = asset_service::verify_ready_asset_integrity(
            &store.connection,
            &store.vault_root,
            &output_asset_id,
        )
        .unwrap();
        assert_eq!(
            output_asset.project_id.as_deref(),
            Some(project_id.as_str())
        );
        assert_eq!(output_asset.kind, AssetKind::Document);
        assert!(output_asset.original_name.ends_with(".docx"));
        assert!(output_path.starts_with(fs::canonicalize(&store.vault_root).unwrap()));
        let output_source =
            asset_service::get_asset_source(&store.connection, &output_asset_id).unwrap();
        assert_eq!(
            output_source.source,
            asset_service::AssetSourceKind::BusinessDocument
        );
        let linked_document_count: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM business_documents WHERE output_asset_id = ?1",
                [&output_asset_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(linked_document_count, 1);
        let mut generated_archive = ZipArchive::new(File::open(output_path).unwrap()).unwrap();
        let generated_media = (0..generated_archive.len())
            .filter(|index| {
                generated_archive
                    .by_index(*index)
                    .map(|entry| !entry.is_dir() && entry.name().starts_with("word/media/"))
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(generated_media, 60);
        drop(generated_archive);

        let vault_snapshot = |vault_root: &Path| {
            let mut pending_directories = vec![vault_root.to_path_buf()];
            let mut files = Vec::new();
            while let Some(directory) = pending_directories.pop() {
                for entry in fs::read_dir(&directory).unwrap() {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if entry.file_type().unwrap().is_dir() {
                        pending_directories.push(path);
                    } else {
                        files.push((
                            path.strip_prefix(vault_root).unwrap().to_path_buf(),
                            fs::read(path).unwrap(),
                        ));
                    }
                }
            }
            files.sort_by(|left, right| left.0.cmp(&right.0));
            files
        };

        let mut unconfirmed_workspace = workspace.clone();
        let mut unconfirmed_document = document.clone();
        let mut unconfirmed = frozen.clone();
        unconfirmed.manually_confirmed = false;
        unconfirmed_workspace.acceptance_batches[0].output_specs[0]
            .production_result_confirmation = Some(unconfirmed.clone());
        unconfirmed_document.snapshot.production_result_confirmation = Some(unconfirmed);
        let unconfirmed_workspace_before =
            load_workspace(&store.connection, &workspace.id).unwrap();
        let unconfirmed_assets_before =
            asset_service::list_assets(&store.connection, Some(&project_id)).unwrap();
        let unconfirmed_database_before = fs::read(&store.database_path).unwrap();
        let unconfirmed_vault_before = vault_snapshot(&store.vault_root);
        let error = load_production_result_confirmation_generation_data(
            &store.connection,
            &store.vault_root,
            &project_id,
            &unconfirmed_workspace,
            &unconfirmed_document,
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_CONFIRMATION_REQUIRED"
        );
        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            unconfirmed_workspace_before
        );
        assert_eq!(
            asset_service::list_assets(&store.connection, Some(&project_id)).unwrap(),
            unconfirmed_assets_before
        );
        assert_eq!(
            fs::read(&store.database_path).unwrap(),
            unconfirmed_database_before
        );
        assert_eq!(vault_snapshot(&store.vault_root), unconfirmed_vault_before);

        let mut stale_document = document.clone();
        stale_document.snapshot.acceptance_batch_revision = Some(batch.revision + 1);
        let stale_workspace_before = load_workspace(&store.connection, &workspace.id).unwrap();
        let stale_assets_before =
            asset_service::list_assets(&store.connection, Some(&project_id)).unwrap();
        let stale_database_before = fs::read(&store.database_path).unwrap();
        let stale_vault_before = vault_snapshot(&store.vault_root);
        let error = load_production_result_confirmation_generation_data(
            &store.connection,
            &store.vault_root,
            &project_id,
            &workspace,
            &stale_document,
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_SNAPSHOT_STALE"
        );
        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            stale_workspace_before
        );
        assert_eq!(
            asset_service::list_assets(&store.connection, Some(&project_id)).unwrap(),
            stale_assets_before
        );
        assert_eq!(
            fs::read(&store.database_path).unwrap(),
            stale_database_before
        );
        assert_eq!(vault_snapshot(&store.vault_root), stale_vault_before);

        let mut mismatched_workspace = workspace.clone();
        let mut mismatched_document = document.clone();
        let mut mismatched = frozen;
        mismatched.delivery_items[0].storyboards[0].shots[0].images[0]
            .group_key
            .push_str("-mismatch");
        mismatched_workspace.acceptance_batches[0].output_specs[0].production_result_confirmation =
            Some(mismatched.clone());
        mismatched_document.snapshot.production_result_confirmation = Some(mismatched);
        let mismatched_workspace_before = load_workspace(&store.connection, &workspace.id).unwrap();
        let mismatched_assets_before =
            asset_service::list_assets(&store.connection, Some(&project_id)).unwrap();
        let mismatched_database_before = fs::read(&store.database_path).unwrap();
        let mismatched_vault_before = vault_snapshot(&store.vault_root);
        let error = load_production_result_confirmation_generation_data(
            &store.connection,
            &store.vault_root,
            &project_id,
            &mismatched_workspace,
            &mismatched_document,
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "BUSINESS_PRODUCTION_RESULT_CONFIRMATION_MATERIAL_MISMATCH"
        );

        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            mismatched_workspace_before
        );
        assert_eq!(
            asset_service::list_assets(&store.connection, Some(&project_id)).unwrap(),
            mismatched_assets_before
        );
        assert_eq!(
            fs::read(&store.database_path).unwrap(),
            mismatched_database_before
        );
        assert_eq!(vault_snapshot(&store.vault_root), mismatched_vault_before);
    }

    fn empty_ooxml_package() -> &'static [u8] {
        b"PK\x05\x06\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"
    }

    fn payment_application_currentness_fixture() -> (BusinessWorkspaceRecord, BusinessDocumentRecord)
    {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let workspace = prepare_effective_contract(&mut store, &project_id, workspace);
        let contract_cents = workspace.financial_summary.contract_cents;
        let workspace = upsert_test_payment(
            &mut store,
            &project_id,
            workspace,
            None,
            contract_cents,
            BusinessPaymentStatus::Planned,
            None,
            "PAYMENT-CURRENTNESS-PLAN",
            "payment currentness fixture",
        );
        let payment = workspace.payments[0].clone();
        let payment_application = freeze_payment_application_data(
            &workspace,
            &BusinessPaymentApplicationInput {
                payment_id: payment.id.clone(),
                contract_title: "Currentness test contract".to_string(),
                contract_number: "CURRENTNESS-2026-001".to_string(),
                work_summary: "completed currentness test deliverables".to_string(),
                payment_period_start: "2026-07-01".to_string(),
                payment_period_end: "2026-07-31".to_string(),
                settlement_period: "2026-07".to_string(),
                payment_sequence: 1,
                invoice_amount_cents: contract_cents,
                cumulative_recognized_amount_cents: contract_cents,
                withheld_amount_cents: 0,
                application_date: "2026-07-29".to_string(),
                supplier_bank_routing_number: "102100000001".to_string(),
                settlement_items: vec![BusinessPaymentSettlementItemData {
                    name: "Production service".to_string(),
                    unit: "item".to_string(),
                    contract_unit_price_cents: contract_cents,
                    original_quantity_millis: 1_000,
                    settlement_quantity_millis: 1_000,
                    remarks: String::new(),
                }],
            },
        )
        .unwrap();
        let document = BusinessDocumentRecord {
            id: Uuid::new_v4().to_string(),
            kind: BusinessDocumentKind::Acceptance,
            sequence_number: 1,
            document_number: "PAYMENT-CURRENTNESS-1".to_string(),
            title: "Payment application currentness".to_string(),
            template_key:
                document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY
                    .to_string(),
            status: BusinessDocumentStatus::Approved,
            snapshot: BusinessDocumentSnapshot {
                workspace_revision: workspace.revision,
                acceptance_batch_id: None,
                acceptance_output_spec_id: None,
                acceptance_batch_revision: None,
                material_bindings: Vec::new(),
                template_asset_id: None,
                template_source_sha256: None,
                template_mapping_version: String::new(),
                contract_settlement: None,
                service_settlement_items: Vec::new(),
                payment_application: Some(payment_application),
                video_completion_acceptance: None,
                production_result_confirmation: None,
                customer_id: workspace.customer_id.clone(),
                customer: workspace.customer.clone(),
                profile: workspace.profile.clone(),
                payment: Some(payment),
            },
            output_asset_id: None,
            output_format: None,
            source_asset_id: None,
            review_id: None,
            report_asset_id: None,
            evidence: None,
            manual_waiver: None,
            voided_at: None,
            voided_by: None,
            void_reason: String::new(),
            approved_at: Some(1),
            approved_by: Some("reviewer".to_string()),
            generated_at: None,
            revision: 1,
            created_at: 1,
            updated_at: 1,
        };
        (workspace, document)
    }

    fn payment_currentness_invoice(
        payment_id: &str,
        kind: BusinessInvoiceKind,
        amount_cents: i64,
        original_invoice_id: Option<String>,
    ) -> crate::protocol::BusinessInvoiceRecord {
        crate::protocol::BusinessInvoiceRecord {
            id: Uuid::new_v4().to_string(),
            payment_id: Some(payment_id.to_string()),
            kind,
            status: crate::protocol::BusinessInvoiceStatus::Issued,
            invoice_code: "CURRENTNESS".to_string(),
            invoice_number: format!("INV-{}", Uuid::new_v4().simple()),
            issuer_tax_id: "SUPPLIER-TAX-001".to_string(),
            buyer_tax_id: "CUSTOMER-TAX-001".to_string(),
            currency: "CNY".to_string(),
            amount_cents,
            tax_cents: 0,
            issued_at: 1_900_000_000_000,
            original_invoice_id,
            reversal_reason: String::new(),
            artifacts: Vec::new(),
            recorded_by: "operator-local".to_string(),
            created_at: 1_900_000_000_000,
        }
    }

    fn prepare_acceptance_documents_command(
        project_id: &str,
        workspace: &BusinessWorkspaceRecord,
        batch_id: &str,
    ) -> BusinessWorkspaceCommandEnvelope {
        BusinessWorkspaceCommandEnvelope::PrepareAcceptanceDocuments {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: PrepareBusinessAcceptanceDocumentsPayload {
                workspace_id: workspace.id.clone(),
                batch_id: batch_id.to_string(),
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        }
    }

    fn upsert_acceptance_material_command(
        project_id: &str,
        workspace: &BusinessWorkspaceRecord,
        batch_id: &str,
        material: BusinessAcceptanceMaterialInput,
    ) -> BusinessWorkspaceCommandEnvelope {
        BusinessWorkspaceCommandEnvelope::UpsertAcceptanceMaterial {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: UpsertBusinessAcceptanceMaterialPayload {
                workspace_id: workspace.id.clone(),
                batch_id: batch_id.to_string(),
                material,
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        }
    }

    struct AcceptanceMaterialSpec<'a> {
        requirement_id: &'a str,
        kind: BusinessAcceptanceMaterialKind,
        group_key: &'a str,
        confirmed: bool,
        duplicate_of_material_id: Option<String>,
    }

    fn add_acceptance_material(
        store: &mut TestStore,
        project_id: &str,
        workspace: BusinessWorkspaceRecord,
        batch_id: &str,
        material: AcceptanceMaterialSpec<'_>,
    ) -> (BusinessWorkspaceRecord, String, String) {
        let (extension, bytes): (&str, &[u8]) = match material.kind {
            BusinessAcceptanceMaterialKind::Video => ("mp4", b"\0\0\0\x18ftypisom\0\0\0\0\0\0\0\0"),
            BusinessAcceptanceMaterialKind::Screenshot
            | BusinessAcceptanceMaterialKind::BehindTheScenes => {
                ("png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x02\0\0\0\x03")
            }
            BusinessAcceptanceMaterialKind::Script
            | BusinessAcceptanceMaterialKind::Invoice
            | BusinessAcceptanceMaterialKind::PublishingData
            | BusinessAcceptanceMaterialKind::Proof => ("pdf", b"%PDF-1.4 acceptance material"),
            BusinessAcceptanceMaterialKind::Other => ("bin", b"acceptance material"),
        };
        let asset_id = import_test_asset(
            store,
            project_id,
            &format!("{}.{}", material.group_key, extension),
            bytes,
        );
        let command = upsert_acceptance_material_command(
            project_id,
            &workspace,
            batch_id,
            BusinessAcceptanceMaterialInput {
                id: None,
                requirement_id: material.requirement_id.to_string(),
                asset_id: asset_id.clone(),
                kind: material.kind,
                group_key: material.group_key.to_string(),
                confirmed: material.confirmed,
                duplicate_of_material_id: material.duplicate_of_material_id,
                notes: String::new(),
            },
        );
        let workspace = store.execute(command).response.business_workspace;
        let material_id = workspace
            .acceptance_batches
            .iter()
            .find(|batch| batch.id == batch_id)
            .and_then(|batch| {
                batch
                    .materials
                    .iter()
                    .find(|material| material.asset_id == asset_id)
            })
            .expect("inserted acceptance material")
            .id
            .clone();
        (workspace, material_id, asset_id)
    }

    fn generate_real_settlement_document(
        store: &mut TestStore,
        project_id: &str,
        mut workspace: BusinessWorkspaceRecord,
        source: &Path,
        template_key: &str,
        format: BusinessDocumentFormat,
    ) -> BusinessWorkspaceRecord {
        let template_asset_id = import_test_asset(
            store,
            project_id,
            &source.file_name().unwrap().to_string_lossy(),
            &fs::read(source).unwrap(),
        );
        let template_sha256 = asset_sha256(store, &template_asset_id);
        let mut command = create_acceptance_batch_command(project_id, &workspace);
        {
            let BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch { payload, .. } =
                &mut command
            else {
                unreachable!()
            };
            payload.requirements.truncate(1);
            payload.requirements[0].required_group_count = 1;
            let requirement_id = payload.requirements[0].id.clone().unwrap();
            payload.output_specs.truncate(1);
            payload.output_specs[0].requirement_ids = vec![requirement_id];
            payload.output_specs[0].document_number = match template_key {
                document_engine::BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY => {
                    "REAL-CONTRACT-SETTLEMENT"
                }
                document_engine::BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY => {
                    "REAL-SERVICE-SETTLEMENT"
                }
                _ => unreachable!(),
            }
            .to_string();
        }
        bind_first_acceptance_template_source(
            &mut command,
            template_key,
            format.clone(),
            template_asset_id,
            template_sha256,
        );
        {
            let BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch { payload, .. } =
                &mut command
            else {
                unreachable!()
            };
            let output = &mut payload.output_specs[0];
            match template_key {
                document_engine::BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY => {
                    output.contract_settlement = Some(BusinessContractSettlementData {
                        contract_title: "White Goose Pond service contract".to_string(),
                        contract_number: "BSE-2026-001".to_string(),
                        original_contract_amount_cents: 8_480_000,
                        contract_adjustment_cents: -490_000,
                        retention_rate_bps: Some(500),
                        final_settlement_amount_cents: 7_990_000,
                    });
                }
                document_engine::BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY => {
                    output.service_settlement_items = vec![BusinessServiceSettlementItemData {
                        service_name: "Video production".to_string(),
                        period: "2026-07".to_string(),
                        description: "Master video and channel cutdowns".to_string(),
                        provided_as_required: Some(true),
                        evidence_label: "Acceptance evidence group 1".to_string(),
                        remarks: String::new(),
                    }];
                }
                _ => unreachable!(),
            }
        }
        workspace = store.execute(command).response.business_workspace;
        let batch = workspace.acceptance_batches.last().unwrap().clone();
        let requirement = batch.requirements[0].clone();
        workspace = add_acceptance_material(
            store,
            project_id,
            workspace,
            &batch.id,
            AcceptanceMaterialSpec {
                requirement_id: &requirement.id,
                kind: requirement.kind,
                group_key: "real-settlement-evidence",
                confirmed: true,
                duplicate_of_material_id: None,
            },
        )
        .0;
        workspace = store
            .execute(prepare_acceptance_documents_command(
                project_id, &workspace, &batch.id,
            ))
            .response
            .business_workspace;
        let document_id = workspace
            .documents
            .iter()
            .find(|document| {
                document.snapshot.acceptance_batch_id.as_deref() == Some(batch.id.as_str())
            })
            .unwrap()
            .id
            .clone();
        for status in [
            BusinessDocumentStatus::InReview,
            BusinessDocumentStatus::Approved,
        ] {
            workspace = store
                .execute(status_command(project_id, &workspace, &document_id, status))
                .response
                .business_workspace;
        }
        workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::GenerateDocument {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(project_id),
                payload: GenerateBusinessDocumentPayload {
                    workspace_id: workspace.id.clone(),
                    document_id: document_id.clone(),
                    format,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        let output_asset_id = workspace
            .documents
            .iter()
            .find(|document| document.id == document_id)
            .unwrap()
            .output_asset_id
            .as_deref()
            .unwrap();
        let (asset, output_path) = asset_service::verify_ready_asset_integrity(
            &store.connection,
            &store.vault_root,
            output_asset_id,
        )
        .unwrap();
        assert_eq!(asset.project_id.as_deref(), Some(project_id));
        assert!(fs::read(output_path).unwrap().starts_with(b"PK"));
        workspace
    }

    fn prepare_test_business_closure_ready(
        store: &mut TestStore,
        project_id: &str,
        mut workspace: BusinessWorkspaceRecord,
    ) -> BusinessWorkspaceRecord {
        workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::UpsertMilestone {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(project_id),
                payload: UpsertBusinessMilestonePayload {
                    workspace_id: workspace.id.clone(),
                    milestone: crate::protocol::BusinessMilestoneInput {
                        id: None,
                        title: "Final delivery".to_string(),
                        description: "Master and social cutdowns".to_string(),
                        due_at: Some(2_000_000_000_000),
                        acceptance_criteria: "Customer written signoff".to_string(),
                        required: true,
                        status: crate::protocol::BusinessMilestoneStatus::Planned,
                    },
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        let milestone_id = workspace.milestones[0].id.clone();
        let delivery_asset = import_test_asset(
            store,
            project_id,
            "final-delivery.mp4",
            b"test final delivery bytes",
        );
        workspace = store
            .execute(
                BusinessWorkspaceCommandEnvelope::RegisterDeliverableVersion {
                    command_id: Uuid::new_v4().to_string(),
                    protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                    context: context(project_id),
                    payload: RegisterBusinessDeliverableVersionPayload {
                        workspace_id: workspace.id.clone(),
                        milestone_id: milestone_id.clone(),
                        deliverable_id: None,
                        name: "Final master".to_string(),
                        required: true,
                        asset_id: delivery_asset,
                        notes: "Approved export candidate".to_string(),
                    },
                    idempotency_key: Uuid::new_v4().to_string(),
                    expected_revision: Some(workspace.revision),
                    deadline_at: None,
                },
            )
            .response
            .business_workspace;
        let version_id = workspace.milestones[0].deliverables[0].versions[0]
            .id
            .clone();
        workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::RecordDeliverySent {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(project_id),
                payload: RecordBusinessDeliverySentPayload {
                    workspace_id: workspace.id.clone(),
                    milestone_id,
                    version_ids: vec![version_id.clone()],
                    recipient: "customer@example.test".to_string(),
                    channel: "email".to_string(),
                    sent_at: 1_900_000_000_000,
                    note: "Final delivery sent".to_string(),
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        let submission_id = workspace.delivery_submissions[0].id.clone();
        let signoff_asset = import_test_asset(
            store,
            project_id,
            "delivery-signoff.txt",
            b"customer delivery acceptance",
        );
        workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::RecordDeliverySignoff {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(project_id),
                payload: RecordBusinessDeliverySignoffPayload {
                    workspace_id: workspace.id.clone(),
                    submission_id,
                    accepted_version_ids: vec![version_id],
                    rejected_version_ids: Vec::new(),
                    customer_representative: "Customer Owner".to_string(),
                    evidence: Some(BusinessEvidenceInput {
                        asset_id: signoff_asset,
                        occurred_at: Some(1_900_000_000_100),
                        note: "Written delivery acceptance".to_string(),
                    }),
                    note: "Accepted without reservation".to_string(),
                    occurred_at: 1_900_000_000_100,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        let invoice_asset =
            import_test_asset(store, project_id, "invoice.pdf", b"test invoice attachment");
        workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::RecordInvoiceIssued {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(project_id),
                payload: RecordBusinessInvoiceIssuedPayload {
                    workspace_id: workspace.id.clone(),
                    payment_id: workspace.payments.first().map(|payment| payment.id.clone()),
                    invoice_code: "TEST-CODE".to_string(),
                    invoice_number: format!("INV-{}", Uuid::new_v4().simple()),
                    amount_cents: workspace.financial_summary.contract_cents,
                    tax_cents: 0,
                    issued_at: 1_900_000_000_200,
                    asset_ids: vec![invoice_asset],
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        workspace
    }

    fn create_test_archive_snapshot(
        store: &mut TestStore,
        project_id: &str,
        workspace: BusinessWorkspaceRecord,
    ) -> BusinessWorkspaceRecord {
        store
            .execute(BusinessWorkspaceCommandEnvelope::CreateArchiveSnapshot {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(project_id),
                payload: CreateBusinessArchiveSnapshotPayload {
                    workspace_id: workspace.id.clone(),
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace
    }

    fn prepare_test_business_closure(
        store: &mut TestStore,
        project_id: &str,
        workspace: BusinessWorkspaceRecord,
    ) -> BusinessWorkspaceRecord {
        let workspace = prepare_test_business_closure_ready(store, project_id, workspace);
        create_test_archive_snapshot(store, project_id, workspace)
    }
    fn change_test_workspace_status(
        store: &mut TestStore,
        project_id: &str,
        workspace: BusinessWorkspaceRecord,
        status: BusinessWorkspaceStatus,
    ) -> BusinessWorkspaceRecord {
        store
            .execute(BusinessWorkspaceCommandEnvelope::ChangeStatus {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(project_id),
                payload: ChangeBusinessWorkspaceStatusPayload {
                    workspace_id: workspace.id.clone(),
                    status,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace
    }
    fn table_counts(connection: &Connection) -> (i64, i64, i64) {
        let count = |table: &str| {
            connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap()
        };
        (
            count("business_workspaces"),
            count("business_workspace_events"),
            count("business_workspace_command_receipts"),
        )
    }

    #[test]
    fn independent_acceptance_promotes_external_reviewed_contract_without_system_quote() {
        let mut store = TestStore::new();
        contract_review_service::migrate(&store.connection).unwrap();
        let project_id = store.project_id.clone();
        let mut workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        workspace = store
            .execute(update_profile_command(
                &project_id,
                &workspace,
                complete_profile_input(&workspace.profile),
            ))
            .response
            .business_workspace;
        assert!(workspace.current_documents.quote_document_id.is_none());
        assert!(!workspace
            .documents
            .iter()
            .any(|document| document.kind == BusinessDocumentKind::Quote));

        let source_asset_id = import_test_asset(
            &mut store,
            &project_id,
            "external-reviewed-contract.pdf",
            b"%PDF-1.4 external reviewed contract",
        );
        let report_asset_id = import_test_asset(
            &mut store,
            &project_id,
            "external-contract-review.html",
            b"<html><body>external contract review completed</body></html>",
        );
        let source_sha256 = asset_sha256(&store, &source_asset_id);
        let report_sha256 = asset_sha256(&store, &report_asset_id);
        let review_id = Uuid::new_v4().to_string();
        let report_id = Uuid::new_v4().to_string();
        let extraction_id = Uuid::new_v4().to_string();
        let report = serde_json::json!({
            "id": report_id,
            "reviewId": review_id,
            "reviewRevision": 1,
            "sourceAssetId": source_asset_id,
            "sourceAssetSha256": source_sha256,
            "extractionId": extraction_id,
            "ruleSetVersion": "external-reviewed-contract.v1",
            "agentRunIds": [],
            "format": "html",
            "reportAssetId": report_asset_id,
            "reportAssetSha256": report_sha256,
            "generatedAt": 1
        });
        store
            .connection
            .execute(
                "INSERT INTO contract_review_sessions
                 (id, workspace_id, source_asset_id, source_asset_sha256, source_file_name,
                  status, stage, extraction_id, report_asset_id, revision,
                  created_at, updated_at, completed_at, failure_json)
                 VALUES (?1, ?2, ?3, ?4, 'external-reviewed-contract.pdf',
                         'completed', 'completed', ?5, ?6, 1, 1, 1, 1, NULL)",
                params![
                    review_id,
                    workspace.id,
                    source_asset_id,
                    source_sha256,
                    extraction_id,
                    report_asset_id,
                ],
            )
            .unwrap();
        store
            .connection
            .execute(
                "INSERT INTO contract_review_reports
                 (id, review_id, report_asset_id, format, record_json, generated_at)
                 VALUES (?1, ?2, ?3, 'html', ?4, 1)",
                params![
                    report_id,
                    review_id,
                    report_asset_id,
                    serde_json::to_string(&report).unwrap(),
                ],
            )
            .unwrap();

        workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::PromoteReviewedContract {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: PromoteReviewedContractPayload {
                    workspace_id: workspace.id.clone(),
                    review_id,
                    report_asset_id,
                    document_number: "EXT-CONTRACT-001".to_string(),
                    title: "External Reviewed Contract".to_string(),
                    evidence: None,
                    manual_waiver: Some(BusinessManualWaiverInput {
                        reason: "external signed contract retained outside test fixture"
                            .to_string(),
                    }),
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        assert!(workspace.current_documents.quote_document_id.is_none());
        let contract_id = workspace
            .current_documents
            .contract_document_id
            .as_deref()
            .expect("external reviewed contract should become the effective contract");
        assert_eq!(
            workspace
                .documents
                .iter()
                .find(|document| document.id == contract_id)
                .unwrap()
                .status,
            BusinessDocumentStatus::Effective
        );

        let requirement_id = Uuid::new_v4().to_string();
        workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: CreateBusinessAcceptanceBatchPayload {
                    workspace_id: workspace.id.clone(),
                    label: "Independent acceptance".to_string(),
                    requirements: vec![BusinessAcceptanceRequirementInput {
                        id: Some(requirement_id.clone()),
                        label: "External contract delivery proof".to_string(),
                        kind: BusinessAcceptanceMaterialKind::Proof,
                        required_group_count: 1,
                    }],
                    output_specs: vec![BusinessAcceptanceOutputSpecInput {
                        id: None,
                        output_code: "independent-acceptance".to_string(),
                        document_number: "EXT-ACC-001".to_string(),
                        title: "Independent Acceptance Certificate".to_string(),
                        template_key: document_engine::ACCEPTANCE_TEMPLATE_KEY.to_string(),
                        template_asset_id: None,
                        template_source_sha256: None,
                        template_mapping_version: String::new(),
                        contract_settlement: None,
                        service_settlement_items: Vec::new(),
                        payment_application: None,
                        video_completion_acceptance: None,
                        production_result_confirmation: None,
                        format: BusinessDocumentFormat::Docx,
                        requirement_ids: vec![requirement_id],
                    }],
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        let batch = workspace.acceptance_batches[0].clone();
        let requirement = batch.requirements[0].clone();
        workspace = add_acceptance_material(
            &mut store,
            &project_id,
            workspace,
            &batch.id,
            AcceptanceMaterialSpec {
                requirement_id: &requirement.id,
                kind: requirement.kind,
                group_key: "external-delivery-proof",
                confirmed: true,
                duplicate_of_material_id: None,
            },
        )
        .0;
        assert!(workspace.acceptance_batches[0].readiness.is_ready);

        workspace = store
            .execute(prepare_acceptance_documents_command(
                &project_id,
                &workspace,
                &batch.id,
            ))
            .response
            .business_workspace;
        let acceptance_id = workspace.acceptance_batches[0].document_ids[0].clone();
        let (workspace, _, acceptance_asset_id) = approve_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            &acceptance_id,
            BusinessDocumentFormat::Docx,
        );
        let workspace =
            make_test_document_effective(&mut store, &project_id, workspace, &acceptance_id);
        let acceptance = workspace
            .documents
            .iter()
            .find(|document| document.id == acceptance_id)
            .unwrap();
        assert_eq!(acceptance.status, BusinessDocumentStatus::Effective);
        assert_eq!(
            acceptance.output_asset_id.as_deref(),
            Some(acceptance_asset_id.as_str())
        );
        assert!(asset_service::resolve_original_path(
            &store.connection,
            &store.vault_root,
            &acceptance_asset_id,
        )
        .unwrap()
        .exists());
        assert!(workspace.current_documents.quote_document_id.is_none());
    }

    #[test]
    fn acceptance_batch_blocks_approval_and_generation_until_material_groups_are_ready() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let mut workspace = prepare_effective_contract(&mut store, &project_id, workspace);

        let create_batch = create_acceptance_batch_command(&project_id, &workspace);
        let created = store.execute(create_batch.clone());
        assert_eq!(
            created.emitted_events[0].event_type,
            BusinessWorkspaceEventType::AcceptanceBatchCreated
        );
        workspace = created.response.business_workspace;
        let batch_id = workspace.acceptance_batches[0].id.clone();
        let replayed = store.execute(create_batch);
        assert!(replayed.response.replayed);
        assert_eq!(
            replayed.response.business_workspace.acceptance_batches[0].id,
            batch_id
        );

        let requirements = workspace.acceptance_batches[0].requirements.clone();
        let primary = &requirements[0];
        let (next, first_material_id, _) = add_acceptance_material(
            &mut store,
            &project_id,
            workspace,
            &batch_id,
            AcceptanceMaterialSpec {
                requirement_id: &primary.id,
                kind: primary.kind.clone(),
                group_key: "video-group-1",
                confirmed: true,
                duplicate_of_material_id: None,
            },
        );
        workspace = next;
        for group in ["video-group-2", "video-group-3"] {
            workspace = add_acceptance_material(
                &mut store,
                &project_id,
                workspace,
                &batch_id,
                AcceptanceMaterialSpec {
                    requirement_id: &primary.id,
                    kind: primary.kind.clone(),
                    group_key: group,
                    confirmed: true,
                    duplicate_of_material_id: None,
                },
            )
            .0;
        }
        workspace = add_acceptance_material(
            &mut store,
            &project_id,
            workspace,
            &batch_id,
            AcceptanceMaterialSpec {
                requirement_id: &primary.id,
                kind: primary.kind.clone(),
                group_key: "video-group-1",
                confirmed: true,
                duplicate_of_material_id: Some(first_material_id),
            },
        )
        .0;
        workspace = add_acceptance_material(
            &mut store,
            &project_id,
            workspace,
            &batch_id,
            AcceptanceMaterialSpec {
                requirement_id: &primary.id,
                kind: primary.kind.clone(),
                group_key: "video-group-4",
                confirmed: false,
                duplicate_of_material_id: None,
            },
        )
        .0;
        for requirement in requirements.iter().skip(1) {
            workspace = add_acceptance_material(
                &mut store,
                &project_id,
                workspace,
                &batch_id,
                AcceptanceMaterialSpec {
                    requirement_id: &requirement.id,
                    kind: requirement.kind.clone(),
                    group_key: &format!("{}-group-1", requirement.id),
                    confirmed: true,
                    duplicate_of_material_id: None,
                },
            )
            .0;
        }

        let blocker = &workspace.acceptance_batches[0].readiness.blockers[0];
        assert_eq!(blocker.required_group_count, 4);
        assert_eq!(blocker.provided_group_count, 3);
        assert_eq!(blocker.missing_group_count, 1);
        assert!(!workspace.acceptance_batches[0].readiness.is_ready);

        let output = workspace.acceptance_batches[0].output_specs[0].clone();
        let prepare = prepare_acceptance_documents_command(&project_id, &workspace, &batch_id);
        let prepared = store.execute(prepare.clone());
        assert_eq!(
            prepared.emitted_events[0].event_type,
            BusinessWorkspaceEventType::AcceptanceDocumentsPrepared
        );
        workspace = prepared.response.business_workspace;
        assert_eq!(workspace.acceptance_batches[0].document_ids.len(), 5);
        assert_eq!(
            workspace
                .documents
                .iter()
                .filter(|document| {
                    document.snapshot.acceptance_batch_id.as_deref() == Some(batch_id.as_str())
                })
                .count(),
            5
        );
        assert!(workspace
            .documents
            .iter()
            .filter(|document| {
                document.snapshot.acceptance_batch_id.as_deref() == Some(batch_id.as_str())
            })
            .all(|document| document.status == BusinessDocumentStatus::Draft));
        let replayed_prepare = store.execute(prepare);
        assert!(replayed_prepare.response.replayed);
        workspace = store
            .execute(prepare_acceptance_documents_command(
                &project_id,
                &workspace,
                &batch_id,
            ))
            .response
            .business_workspace;
        assert_eq!(workspace.acceptance_batches[0].document_ids.len(), 5);
        let document_id = workspace
            .documents
            .iter()
            .find(|document| {
                document.snapshot.acceptance_output_spec_id.as_deref() == Some(output.id.as_str())
            })
            .unwrap()
            .id
            .clone();
        let prepared_document = workspace
            .documents
            .iter()
            .find(|document| document.id == document_id)
            .unwrap();
        assert_eq!(
            prepared_document.snapshot.acceptance_batch_revision,
            Some(workspace.acceptance_batches[0].revision)
        );
        assert_eq!(prepared_document.snapshot.material_bindings.len(), 8);
        for binding in &prepared_document.snapshot.material_bindings {
            let sha256 = store
                .connection
                .query_row(
                    "SELECT sha256 FROM assets WHERE id = ?1 AND project_id = ?2 AND status = 'ready'",
                    params![binding.asset_id, project_id],
                    |row| row.get::<_, String>(0),
                )
                .unwrap();
            assert_eq!(binding.sha256, sha256);
        }
        workspace = store
            .execute(status_command(
                &project_id,
                &workspace,
                &document_id,
                BusinessDocumentStatus::InReview,
            ))
            .response
            .business_workspace;
        let approval_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            status_command(
                &project_id,
                &workspace,
                &document_id,
                BusinessDocumentStatus::Approved,
            ),
        )
        .unwrap_err();
        assert_eq!(approval_error.code, "BUSINESS_ACCEPTANCE_NOT_READY");

        let (next, fourth_material_id, fourth_asset_id) = add_acceptance_material(
            &mut store,
            &project_id,
            workspace,
            &batch_id,
            AcceptanceMaterialSpec {
                requirement_id: &primary.id,
                kind: primary.kind.clone(),
                group_key: "video-group-4-ready",
                confirmed: true,
                duplicate_of_material_id: None,
            },
        );
        workspace = next;
        assert!(workspace.acceptance_batches[0].readiness.is_ready);
        let refreshed_document = workspace
            .documents
            .iter()
            .find(|document| document.id == document_id)
            .unwrap();
        assert_eq!(refreshed_document.snapshot.material_bindings.len(), 9);
        assert_eq!(
            refreshed_document.snapshot.acceptance_batch_revision,
            Some(workspace.acceptance_batches[0].revision)
        );
        workspace = store
            .execute(status_command(
                &project_id,
                &workspace,
                &document_id,
                BusinessDocumentStatus::Approved,
            ))
            .response
            .business_workspace;
        let approved_snapshot = workspace
            .documents
            .iter()
            .find(|document| document.id == document_id)
            .unwrap()
            .snapshot
            .clone();
        let wrong_format_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::GenerateDocument {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: GenerateBusinessDocumentPayload {
                    workspace_id: workspace.id.clone(),
                    document_id: document_id.clone(),
                    format: BusinessDocumentFormat::Docx,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();
        assert_eq!(
            wrong_format_error.code,
            "BUSINESS_ACCEPTANCE_OUTPUT_FORMAT_MISMATCH"
        );

        workspace = store
            .execute(upsert_acceptance_material_command(
                &project_id,
                &workspace,
                &batch_id,
                BusinessAcceptanceMaterialInput {
                    id: Some(fourth_material_id.clone()),
                    requirement_id: primary.id.clone(),
                    asset_id: fourth_asset_id.clone(),
                    kind: primary.kind.clone(),
                    group_key: "video-group-4-ready".to_string(),
                    confirmed: false,
                    duplicate_of_material_id: None,
                    notes: "temporarily unconfirmed".to_string(),
                },
            ))
            .response
            .business_workspace;
        assert_eq!(
            workspace
                .documents
                .iter()
                .find(|document| document.id == document_id)
                .unwrap()
                .snapshot,
            approved_snapshot
        );
        let generate = |workspace: &BusinessWorkspaceRecord| {
            BusinessWorkspaceCommandEnvelope::GenerateDocument {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: GenerateBusinessDocumentPayload {
                    workspace_id: workspace.id.clone(),
                    document_id: document_id.clone(),
                    format: output.format.clone(),
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            }
        };
        let generation_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            generate(&workspace),
        )
        .unwrap_err();
        assert_eq!(generation_error.code, "BUSINESS_ACCEPTANCE_NOT_READY");

        workspace = store
            .execute(upsert_acceptance_material_command(
                &project_id,
                &workspace,
                &batch_id,
                BusinessAcceptanceMaterialInput {
                    id: Some(fourth_material_id),
                    requirement_id: primary.id.clone(),
                    asset_id: fourth_asset_id,
                    kind: primary.kind.clone(),
                    group_key: "video-group-4-ready".to_string(),
                    confirmed: true,
                    duplicate_of_material_id: None,
                    notes: "confirmed".to_string(),
                },
            ))
            .response
            .business_workspace;
        workspace = store
            .execute(generate(&workspace))
            .response
            .business_workspace;
        assert_eq!(
            workspace
                .documents
                .iter()
                .find(|document| document.id == document_id)
                .unwrap()
                .status,
            BusinessDocumentStatus::Generated
        );

        store.reopen();
        let restored = load_workspace(&store.connection, &workspace.id).unwrap();
        assert!(restored.acceptance_batches[0].readiness.is_ready);
        assert_eq!(restored.acceptance_batches[0].materials.len(), 11);
        assert_eq!(
            restored
                .documents
                .iter()
                .find(|document| document.id == document_id)
                .unwrap()
                .snapshot
                .acceptance_batch_id
                .as_deref(),
            Some(batch_id.as_str())
        );
        let restored_document = restored
            .documents
            .iter()
            .find(|document| document.id == document_id)
            .unwrap();
        assert_eq!(
            restored_document
                .snapshot
                .acceptance_output_spec_id
                .as_deref(),
            Some(output.id.as_str())
        );
        assert_eq!(restored_document.snapshot, approved_snapshot);
    }

    #[test]
    fn acceptance_batch_rejects_invalid_output_requirement_duplicate_code_and_template_format() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;

        let mut invalid_requirement = create_acceptance_batch_command(&project_id, &workspace);
        let BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch { payload, .. } =
            &mut invalid_requirement
        else {
            unreachable!()
        };
        payload.output_specs[0].requirement_ids[0] = Uuid::new_v4().to_string();
        let error = execute_command(
            &mut store.connection,
            &store.vault_root,
            invalid_requirement,
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "BUSINESS_ACCEPTANCE_OUTPUT_REQUIREMENT_NOT_FOUND"
        );

        let mut duplicate_code = create_acceptance_batch_command(&project_id, &workspace);
        let BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch { payload, .. } =
            &mut duplicate_code
        else {
            unreachable!()
        };
        payload.output_specs[1].output_code = payload.output_specs[0].output_code.clone();
        let error =
            execute_command(&mut store.connection, &store.vault_root, duplicate_code).unwrap_err();
        assert_eq!(error.code, "BUSINESS_ACCEPTANCE_OUTPUT_SPEC_DUPLICATE");

        let mut wrong_template_format = create_acceptance_batch_command(&project_id, &workspace);
        let BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch { payload, .. } =
            &mut wrong_template_format
        else {
            unreachable!()
        };
        payload.output_specs[0].template_key =
            document_engine::BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY.to_string();
        payload.output_specs[0].format = BusinessDocumentFormat::Docx;
        let error = execute_command(
            &mut store.connection,
            &store.vault_root,
            wrong_template_format,
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_TEMPLATE_FORMAT_MISMATCH");
    }

    #[test]
    fn specialized_settlement_data_validation_preserves_draft_unknowns_and_rejects_bad_totals() {
        let valid = normalize_contract_settlement_data(BusinessContractSettlementData {
            contract_title: " Service Contract ".to_string(),
            contract_number: " 001-A ".to_string(),
            original_contract_amount_cents: 1_000_000,
            contract_adjustment_cents: -100_000,
            retention_rate_bps: Some(0),
            final_settlement_amount_cents: 900_000,
        })
        .unwrap();
        assert_eq!(valid.contract_title, "Service Contract");
        assert_eq!(valid.contract_number, "001-A");
        assert_eq!(valid.retention_rate_bps, None);

        let mismatch = normalize_contract_settlement_data(BusinessContractSettlementData {
            final_settlement_amount_cents: 800_000,
            ..valid.clone()
        })
        .unwrap_err();
        assert_eq!(mismatch.code, "BUSINESS_CONTRACT_SETTLEMENT_TOTAL_MISMATCH");

        let fractional = normalize_contract_settlement_data(BusinessContractSettlementData {
            original_contract_amount_cents: 1_000_001,
            contract_adjustment_cents: 0,
            final_settlement_amount_cents: 1_000_001,
            ..valid
        })
        .unwrap_err();
        assert_eq!(
            fractional.code,
            "BUSINESS_CONTRACT_SETTLEMENT_FRACTIONAL_CNY_UNCONFIRMED"
        );

        let draft_row = normalize_service_settlement_item(
            0,
            BusinessServiceSettlementItemData {
                service_name: " Video production ".to_string(),
                period: " 2026-07 ".to_string(),
                description: " Delivery pending confirmation ".to_string(),
                provided_as_required: None,
                evidence_label: " Evidence group A ".to_string(),
                remarks: String::new(),
            },
        )
        .unwrap();
        assert_eq!(draft_row.provided_as_required, None);
        assert_eq!(draft_row.service_name, "Video production");
    }

    #[test]
    fn acceptance_template_rejects_unregistered_mapping_version() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let mut command = create_acceptance_batch_command(&project_id, &workspace);
        bind_first_acceptance_template_source(
            &mut command,
            document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY,
            BusinessDocumentFormat::Docx,
            Uuid::new_v4().to_string(),
            "11".repeat(32),
        );
        let BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch { payload, .. } = &mut command
        else {
            unreachable!()
        };
        payload.output_specs[0].template_mapping_version = "unregistered-map.v1".to_string();
        let error = execute_command(&mut store.connection, &store.vault_root, command).unwrap_err();
        assert_eq!(error.code, "BUSINESS_TEMPLATE_MAPPING_VERSION_MISMATCH");
    }

    #[test]
    fn acceptance_template_source_contract_rejects_missing_unexpected_and_wrong_registered_hash() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;

        let mut missing = create_acceptance_batch_command(&project_id, &workspace);
        let BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch { payload, .. } = &mut missing
        else {
            unreachable!()
        };
        payload.output_specs[0].template_key =
            document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY
                .to_string();
        payload.output_specs[0].format = BusinessDocumentFormat::Docx;
        let error = execute_command(&mut store.connection, &store.vault_root, missing).unwrap_err();
        assert_eq!(error.code, "BUSINESS_TEMPLATE_SOURCE_REQUIRED");

        let mut unexpected = create_acceptance_batch_command(&project_id, &workspace);
        bind_first_acceptance_template_source(
            &mut unexpected,
            document_engine::ACCEPTANCE_TEMPLATE_KEY,
            BusinessDocumentFormat::Docx,
            Uuid::new_v4().to_string(),
            "11".repeat(32),
        );
        let error =
            execute_command(&mut store.connection, &store.vault_root, unexpected).unwrap_err();
        assert_eq!(error.code, "BUSINESS_TEMPLATE_SOURCE_UNEXPECTED");

        let mut wrong_hash = create_acceptance_batch_command(&project_id, &workspace);
        bind_first_acceptance_template_source(
            &mut wrong_hash,
            document_engine::BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY,
            BusinessDocumentFormat::Xlsx,
            Uuid::new_v4().to_string(),
            "22".repeat(32),
        );
        let error =
            execute_command(&mut store.connection, &store.vault_root, wrong_hash).unwrap_err();
        assert_eq!(error.code, "BUSINESS_TEMPLATE_SOURCE_HASH_MISMATCH");
    }

    #[test]
    fn acceptance_template_source_asset_is_verified_and_frozen_into_prepared_snapshot() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let workspace = prepare_effective_contract(&mut store, &project_id, workspace);
        let contract_cents = workspace.financial_summary.contract_cents;
        let workspace = upsert_test_payment(
            &mut store,
            &project_id,
            workspace,
            None,
            contract_cents,
            BusinessPaymentStatus::Planned,
            None,
            "PAYMENT-APPLICATION-PLAN",
            "payment application fixture",
        );
        let payment_id = workspace.payments[0].id.clone();
        let source_asset_id = import_test_asset(
            &mut store,
            &project_id,
            "payment-application.doc",
            b"legacy payment application fixture",
        );
        let normalized_source = store
            .temporary
            .path()
            .join("payment-application-normalized.docx");
        fs::write(&normalized_source, empty_ooxml_package()).unwrap();
        let template_asset_id = asset_service::import_generated_artifact(
            &mut store.connection,
            &store.vault_root,
            &project_id,
            &normalized_source,
            asset_service::GeneratedArtifactSource::NormalizedTemplate,
            "approved-payment-template-fixture",
        )
        .unwrap()
        .id;
        let template_sha256 = asset_sha256(&store, &template_asset_id);
        let mut command = create_acceptance_batch_command(&project_id, &workspace);
        bind_first_acceptance_template_source(
            &mut command,
            document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY,
            BusinessDocumentFormat::Docx,
            template_asset_id.clone(),
            template_sha256.clone(),
        );
        let BusinessWorkspaceCommandEnvelope::CreateAcceptanceBatch { payload, .. } = &mut command
        else {
            unreachable!("acceptance command helper returned another command")
        };
        payload.output_specs[0].payment_application = Some(BusinessPaymentApplicationInput {
            payment_id: payment_id.clone(),
            contract_title: "White Goose Pond production services".to_string(),
            contract_number: "BSE-CONTRACT-2026-001".to_string(),
            work_summary: "completed the confirmed production deliverables".to_string(),
            payment_period_start: "2026-07-01".to_string(),
            payment_period_end: "2026-07-31".to_string(),
            settlement_period: "2026-07".to_string(),
            payment_sequence: 1,
            invoice_amount_cents: contract_cents,
            cumulative_recognized_amount_cents: contract_cents,
            withheld_amount_cents: 0,
            application_date: "2026-07-29".to_string(),
            supplier_bank_routing_number: "102100000001".to_string(),
            settlement_items: vec![BusinessPaymentSettlementItemData {
                name: "Production service".to_string(),
                unit: "item".to_string(),
                contract_unit_price_cents: contract_cents,
                original_quantity_millis: 1_000,
                settlement_quantity_millis: 1_000,
                remarks: "completed".to_string(),
            }],
        });

        let source_sha256 = asset_sha256(&store, &source_asset_id);
        let mapping_version = document_engine::expected_template_mapping_version(
            document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY,
        )
        .unwrap();
        let normalized_source_sha256 = source_sha256.to_ascii_uppercase();
        let normalized_template_sha256 = template_sha256.to_ascii_uppercase();
        let template_version_id = Uuid::new_v4().to_string();
        store
            .connection
            .execute(
                "INSERT INTO business_template_versions
                 (id, workspace_id, source_asset_id, source_sha256,
                  normalized_asset_id, normalized_sha256, template_key, mapping_version,
                  converter_engine, converter_version, converter_policy_version,
                  status, reviewed_by, reviewed_at, review_note, revision, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                         'microsoft-word-com', '16.0', 'word-only.v1',
                         'pendingReview', NULL, NULL, '', 1, 10, 10)",
                params![
                    template_version_id,
                    workspace.id,
                    source_asset_id,
                    normalized_source_sha256,
                    template_asset_id,
                    normalized_template_sha256,
                    document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY,
                    mapping_version,
                ],
            )
            .unwrap();

        let unapproved =
            execute_command(&mut store.connection, &store.vault_root, command.clone()).unwrap_err();
        assert_eq!(
            unapproved.code,
            "BUSINESS_TEMPLATE_VERSION_APPROVAL_REQUIRED"
        );
        store
            .connection
            .execute(
                "UPDATE business_template_versions
                 SET status = 'approved', reviewed_by = 'reviewer', reviewed_at = 20,
                     review_note = 'fixture approved', revision = 2, updated_at = 20
                 WHERE id = ?1",
                [&template_version_id],
            )
            .unwrap();

        let workspace = store.execute(command).response.business_workspace;
        let batch = &workspace.acceptance_batches[0];
        let spec = &batch.output_specs[0];
        assert_eq!(
            spec.template_asset_id.as_deref(),
            Some(template_asset_id.as_str())
        );
        assert!(spec
            .template_source_sha256
            .as_deref()
            .is_some_and(|sha256| sha256.eq_ignore_ascii_case(&template_sha256)));
        assert_eq!(
            spec.template_mapping_version,
            document_engine::expected_template_mapping_version(
                document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY
            )
            .unwrap()
        );
        let payment_application = spec.payment_application.as_ref().unwrap();
        assert_eq!(payment_application.payment_id, payment_id);
        assert_eq!(payment_application.settlement_total_cents, contract_cents);
        assert_eq!(payment_application.cumulative_paid_cents, 0);
        assert_eq!(payment_application.remaining_payable_cents, contract_cents);
        let batch_id = batch.id.clone();
        let spec_id = spec.id.clone();
        let workspace = store
            .execute(prepare_acceptance_documents_command(
                &project_id,
                &workspace,
                &batch_id,
            ))
            .response
            .business_workspace;
        let snapshot = &workspace
            .documents
            .iter()
            .find(|document| {
                document.snapshot.acceptance_output_spec_id.as_deref() == Some(spec_id.as_str())
            })
            .unwrap()
            .snapshot;
        assert_eq!(
            snapshot.template_asset_id.as_deref(),
            Some(template_asset_id.as_str())
        );
        assert!(snapshot
            .template_source_sha256
            .as_deref()
            .is_some_and(|sha256| sha256.eq_ignore_ascii_case(&template_sha256)));
        assert_eq!(
            snapshot.template_mapping_version,
            document_engine::expected_template_mapping_version(
                document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY
            )
            .unwrap()
        );
        assert_eq!(
            snapshot.payment_application.as_ref(),
            Some(payment_application)
        );
        assert_eq!(
            snapshot.payment.as_ref().map(|payment| payment.id.as_str()),
            Some(payment_id.as_str())
        );
    }

    #[test]
    fn official_payment_generation_blocks_bank_account_profile_drift() {
        let (mut workspace, document) = payment_application_currentness_fixture();
        ensure_payment_application_current(&workspace, &document).unwrap();
        let frozen = document.snapshot.payment_application.as_ref().unwrap();
        assert!(frozen
            .bank_account_profile_version
            .starts_with("payment-bank-account-sha256:"));
        for sensitive in [
            document.snapshot.profile.supplier_legal_name.as_str(),
            document.snapshot.profile.supplier_bank_name.as_str(),
            document.snapshot.profile.supplier_bank_account.as_str(),
            frozen.supplier_bank_routing_number.as_str(),
        ] {
            assert!(!frozen.bank_account_profile_version.contains(sensitive));
        }

        workspace.profile.supplier_bank_account = "622200000099".to_string();
        let error = ensure_payment_application_current(&workspace, &document).unwrap_err();
        assert_eq!(error.code, "BUSINESS_PAYMENT_BANK_ACCOUNT_CHANGED");
        assert!(!error.message.contains("622200000099"));
    }

    #[test]
    fn official_payment_generation_blocks_invoice_issue_and_red_correction_drift() {
        let (mut workspace, document) = payment_application_currentness_fixture();
        let data = document.snapshot.payment_application.as_ref().unwrap();
        let payment_id = data.payment_id.clone();
        workspace.invoices.push(payment_currentness_invoice(
            &payment_id,
            BusinessInvoiceKind::Issued,
            data.invoice_amount_cents - 1,
            None,
        ));
        let error = ensure_payment_application_current(&workspace, &document).unwrap_err();
        assert_eq!(error.code, "BUSINESS_PAYMENT_INVOICE_LEDGER_CHANGED");

        let (mut workspace, document) = payment_application_currentness_fixture();
        let data = document.snapshot.payment_application.as_ref().unwrap();
        let payment_id = data.payment_id.clone();
        let issued = payment_currentness_invoice(
            &payment_id,
            BusinessInvoiceKind::Issued,
            data.invoice_amount_cents,
            None,
        );
        let issued_id = issued.id.clone();
        workspace.invoices.push(issued);
        workspace.invoices.push(payment_currentness_invoice(
            &payment_id,
            BusinessInvoiceKind::Reversal,
            1,
            Some(issued_id),
        ));
        let error = ensure_payment_application_current(&workspace, &document).unwrap_err();
        assert_eq!(error.code, "BUSINESS_PAYMENT_INVOICE_LEDGER_CHANGED");
    }

    #[test]
    fn payment_application_full_invoice_reversal_to_zero_is_blocked() {
        let (mut workspace, document) = payment_application_currentness_fixture();
        let data = document
            .snapshot
            .payment_application
            .as_ref()
            .unwrap()
            .clone();
        let issued = payment_currentness_invoice(
            &data.payment_id,
            BusinessInvoiceKind::Issued,
            data.invoice_amount_cents,
            None,
        );
        let issued_id = issued.id.clone();
        workspace.invoices.push(issued);
        workspace.invoices.push(payment_currentness_invoice(
            &data.payment_id,
            BusinessInvoiceKind::Reversal,
            data.invoice_amount_cents,
            Some(issued_id),
        ));

        let generation_error =
            ensure_payment_application_current(&workspace, &document).unwrap_err();
        assert_eq!(
            generation_error.code,
            "BUSINESS_PAYMENT_INVOICE_LEDGER_CHANGED"
        );

        let freeze_error = freeze_payment_application_data(
            &workspace,
            &BusinessPaymentApplicationInput {
                payment_id: data.payment_id,
                contract_title: data.contract_title,
                contract_number: data.contract_number,
                work_summary: data.work_summary,
                payment_period_start: data.payment_period_start,
                payment_period_end: data.payment_period_end,
                settlement_period: data.settlement_period,
                payment_sequence: data.payment_sequence,
                invoice_amount_cents: data.invoice_amount_cents,
                cumulative_recognized_amount_cents: data.cumulative_recognized_amount_cents,
                withheld_amount_cents: data.withheld_amount_cents,
                application_date: data.application_date,
                supplier_bank_routing_number: data.supplier_bank_routing_number,
                settlement_items: data.settlement_items,
            },
        )
        .unwrap_err();
        assert_eq!(
            freeze_error.code,
            "BUSINESS_PAYMENT_INVOICE_AMOUNT_MISMATCH"
        );
    }

    #[test]
    fn acceptance_template_source_asset_rejects_foreign_tampered_and_legacy_doc_files() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let other_project_id =
            insert_project(&store.connection, "Other Project", "Other Customer", None);
        let foreign_asset_id = import_test_asset(
            &mut store,
            &other_project_id,
            "foreign-template.docx",
            empty_ooxml_package(),
        );
        let mut foreign = create_acceptance_batch_command(&project_id, &workspace);
        bind_first_acceptance_template_source(
            &mut foreign,
            document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY,
            BusinessDocumentFormat::Docx,
            foreign_asset_id.clone(),
            asset_sha256(&store, &foreign_asset_id),
        );
        let error = execute_command(&mut store.connection, &store.vault_root, foreign).unwrap_err();
        assert_eq!(error.code, "BUSINESS_TEMPLATE_ASSET_PROJECT_MISMATCH");

        let tampered_asset_id = import_test_asset(
            &mut store,
            &project_id,
            "tampered-template.docx",
            empty_ooxml_package(),
        );
        let tampered_sha256 = asset_sha256(&store, &tampered_asset_id);
        let tampered_path = asset_service::resolve_original_path(
            &store.connection,
            &store.vault_root,
            &tampered_asset_id,
        )
        .unwrap();
        fs::write(tampered_path, b"tampered template").unwrap();
        let mut tampered = create_acceptance_batch_command(&project_id, &workspace);
        bind_first_acceptance_template_source(
            &mut tampered,
            document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY,
            BusinessDocumentFormat::Docx,
            tampered_asset_id,
            tampered_sha256,
        );
        let error =
            execute_command(&mut store.connection, &store.vault_root, tampered).unwrap_err();
        assert_eq!(error.code, "VAULT_ASSET_INTEGRITY_MISMATCH");

        let legacy_asset_id = import_test_asset(
            &mut store,
            &project_id,
            "legacy-payment-application.doc",
            b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1legacy binary doc template",
        );
        let mut legacy = create_acceptance_batch_command(&project_id, &workspace);
        bind_first_acceptance_template_source(
            &mut legacy,
            document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY,
            BusinessDocumentFormat::Docx,
            legacy_asset_id.clone(),
            asset_sha256(&store, &legacy_asset_id),
        );
        let error = execute_command(&mut store.connection, &store.vault_root, legacy).unwrap_err();
        assert_eq!(error.code, "BUSINESS_TEMPLATE_ASSET_FORMAT_MISMATCH");
    }

    #[test]
    #[ignore = "requires the registered real White Goose Pond settlement templates"]
    fn real_settlement_templates_generate_through_vault_business_command_path() {
        let contract_template = std::env::var_os("BSAIGC_CONTRACT_SETTLEMENT_TEMPLATE")
            .map(PathBuf::from)
            .unwrap_or_else(|| external_qa_fixture("templates/synthetic-contract-settlement.xlsx"));
        let service_template = std::env::var_os("BSAIGC_SERVICE_SETTLEMENT_TEMPLATE")
            .map(PathBuf::from)
            .unwrap_or_else(|| external_qa_fixture("templates/synthetic-service-settlement.docx"));
        assert!(contract_template.is_file());
        assert!(service_template.is_file());

        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let workspace = prepare_effective_contract(&mut store, &project_id, workspace);
        let workspace = generate_real_settlement_document(
            &mut store,
            &project_id,
            workspace,
            &contract_template,
            document_engine::BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY,
            BusinessDocumentFormat::Xlsx,
        );
        let workspace = generate_real_settlement_document(
            &mut store,
            &project_id,
            workspace,
            &service_template,
            document_engine::BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY,
            BusinessDocumentFormat::Docx,
        );
        assert_eq!(
            workspace
                .documents
                .iter()
                .filter(|document| {
                    matches!(
                        document.template_key.as_str(),
                        document_engine::BAIETAN_CONTRACT_SETTLEMENT_TEMPLATE_KEY
                            | document_engine::BAIETAN_SERVICE_SETTLEMENT_LIST_TEMPLATE_KEY
                    ) && document.status == BusinessDocumentStatus::Generated
                })
                .count(),
            2
        );
    }

    #[test]
    fn acceptance_material_rejects_asset_from_another_project() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let workspace = store
            .execute(create_acceptance_batch_command(&project_id, &workspace))
            .response
            .business_workspace;
        let batch = &workspace.acceptance_batches[0];
        let requirement = &batch.requirements[0];
        let other_project_id =
            insert_project(&store.connection, "Other Project", "Other Customer", None);
        let foreign_asset_id = import_test_asset(
            &mut store,
            &other_project_id,
            "foreign.mp4",
            b"foreign project material",
        );
        let error = execute_command(
            &mut store.connection,
            &store.vault_root,
            upsert_acceptance_material_command(
                &project_id,
                &workspace,
                &batch.id,
                BusinessAcceptanceMaterialInput {
                    id: None,
                    requirement_id: requirement.id.clone(),
                    asset_id: foreign_asset_id,
                    kind: requirement.kind.clone(),
                    group_key: "foreign-group".to_string(),
                    confirmed: true,
                    duplicate_of_material_id: None,
                    notes: String::new(),
                },
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_ACCEPTANCE_ASSET_PROJECT_MISMATCH");
        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            workspace
        );
    }

    #[test]
    fn acceptance_prepare_replay_and_stale_retry_keep_exactly_five_documents() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let workspace = prepare_effective_contract(&mut store, &project_id, workspace);
        let workspace = store
            .execute(create_acceptance_batch_command(&project_id, &workspace))
            .response
            .business_workspace;
        let batch_id = workspace.acceptance_batches[0].id.clone();
        let prepare = prepare_acceptance_documents_command(&project_id, &workspace, &batch_id);

        let prepared = store.execute(prepare.clone());
        assert!(!prepared.response.replayed);
        let prepared_workspace = prepared.response.business_workspace;
        let prepared_revision = prepared_workspace.revision;
        let mut prepared_document_ids = prepared_workspace
            .documents
            .iter()
            .filter(|document| {
                document.snapshot.acceptance_batch_id.as_deref() == Some(batch_id.as_str())
            })
            .map(|document| document.id.clone())
            .collect::<Vec<_>>();
        prepared_document_ids.sort();
        assert_eq!(prepared_document_ids.len(), 5);
        assert_eq!(
            prepared_workspace.acceptance_batches[0].document_ids.len(),
            5
        );

        let replayed = store.execute(prepare);
        assert!(replayed.response.replayed);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(replayed.response.business_workspace, prepared_workspace);
        assert_eq!(
            replayed.response.business_workspace.revision,
            prepared_revision
        );

        let stale_prepare =
            prepare_acceptance_documents_command(&project_id, &prepared_workspace, &batch_id);
        let repeated = store.execute(prepare_acceptance_documents_command(
            &project_id,
            &prepared_workspace,
            &batch_id,
        ));
        let repeated_workspace = repeated.response.business_workspace;
        let mut repeated_document_ids = repeated_workspace
            .documents
            .iter()
            .filter(|document| {
                document.snapshot.acceptance_batch_id.as_deref() == Some(batch_id.as_str())
            })
            .map(|document| document.id.clone())
            .collect::<Vec<_>>();
        repeated_document_ids.sort();
        assert_eq!(repeated_document_ids, prepared_document_ids);
        assert_eq!(
            repeated_workspace.acceptance_batches[0].document_ids.len(),
            5
        );

        let journal_counts_before_stale = store
            .connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM business_workspace_events),
                     (SELECT COUNT(*) FROM business_workspace_command_receipts)",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap();
        let stale_error =
            execute_command(&mut store.connection, &store.vault_root, stale_prepare).unwrap_err();
        assert_eq!(stale_error.code, "REVISION_CONFLICT");
        assert_eq!(
            load_workspace(&store.connection, &repeated_workspace.id).unwrap(),
            repeated_workspace
        );
        let journal_counts_after_stale = store
            .connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM business_workspace_events),
                     (SELECT COUNT(*) FROM business_workspace_command_receipts)",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap();
        assert_eq!(journal_counts_after_stale, journal_counts_before_stale);
    }

    #[test]
    fn acceptance_material_validation_replay_and_cas_are_atomic() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let workspace = prepare_effective_contract(&mut store, &project_id, workspace);
        let workspace = store
            .execute(create_acceptance_batch_command(&project_id, &workspace))
            .response
            .business_workspace;
        let batch_id = workspace.acceptance_batches[0].id.clone();
        let requirement = workspace.acceptance_batches[0].requirements[0].clone();
        let ready_asset_id = import_test_asset(
            &mut store,
            &project_id,
            "acceptance-material.mp4",
            &[0, 0, 0, 24, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm'],
        );
        let wrong_asset_kind_id = import_test_asset(
            &mut store,
            &project_id,
            "acceptance-material.txt",
            b"wrong acceptance material kind",
        );

        let missing_asset_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            upsert_acceptance_material_command(
                &project_id,
                &workspace,
                &batch_id,
                BusinessAcceptanceMaterialInput {
                    id: None,
                    requirement_id: requirement.id.clone(),
                    asset_id: Uuid::new_v4().to_string(),
                    kind: requirement.kind.clone(),
                    group_key: "missing-asset".to_string(),
                    confirmed: true,
                    duplicate_of_material_id: None,
                    notes: String::new(),
                },
            ),
        )
        .unwrap_err();
        assert_eq!(missing_asset_error.code, "ASSET_NOT_FOUND");
        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            workspace
        );

        let mismatched_kind = match requirement.kind {
            BusinessAcceptanceMaterialKind::Video => BusinessAcceptanceMaterialKind::Script,
            _ => BusinessAcceptanceMaterialKind::Video,
        };
        let kind_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            upsert_acceptance_material_command(
                &project_id,
                &workspace,
                &batch_id,
                BusinessAcceptanceMaterialInput {
                    id: None,
                    requirement_id: requirement.id.clone(),
                    asset_id: ready_asset_id.clone(),
                    kind: mismatched_kind,
                    group_key: "wrong-kind".to_string(),
                    confirmed: true,
                    duplicate_of_material_id: None,
                    notes: String::new(),
                },
            ),
        )
        .unwrap_err();
        assert_eq!(
            kind_error.code,
            "BUSINESS_ACCEPTANCE_MATERIAL_KIND_MISMATCH"
        );
        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            workspace
        );

        let journal_counts_before_asset_kind = store
            .connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM business_workspace_events),
                     (SELECT COUNT(*) FROM business_workspace_command_receipts)",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap();
        let asset_kind_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            upsert_acceptance_material_command(
                &project_id,
                &workspace,
                &batch_id,
                BusinessAcceptanceMaterialInput {
                    id: None,
                    requirement_id: requirement.id.clone(),
                    asset_id: wrong_asset_kind_id,
                    kind: requirement.kind.clone(),
                    group_key: "wrong-asset-kind".to_string(),
                    confirmed: true,
                    duplicate_of_material_id: None,
                    notes: String::new(),
                },
            ),
        )
        .unwrap_err();
        assert_eq!(
            asset_kind_error.code,
            "BUSINESS_ACCEPTANCE_ASSET_KIND_MISMATCH"
        );
        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            workspace
        );
        let journal_counts_after_asset_kind = store
            .connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM business_workspace_events),
                     (SELECT COUNT(*) FROM business_workspace_command_receipts)",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap();
        assert_eq!(
            journal_counts_after_asset_kind,
            journal_counts_before_asset_kind
        );

        let upsert = upsert_acceptance_material_command(
            &project_id,
            &workspace,
            &batch_id,
            BusinessAcceptanceMaterialInput {
                id: None,
                requirement_id: requirement.id.clone(),
                asset_id: ready_asset_id.clone(),
                kind: requirement.kind.clone(),
                group_key: "video-group-1".to_string(),
                confirmed: true,
                duplicate_of_material_id: None,
                notes: "first".to_string(),
            },
        );
        let inserted = store.execute(upsert.clone());
        let inserted_workspace = inserted.response.business_workspace;
        assert_eq!(inserted_workspace.acceptance_batches[0].materials.len(), 1);
        let material_id = inserted_workspace.acceptance_batches[0].materials[0]
            .id
            .clone();

        let replayed = store.execute(upsert.clone());
        assert!(replayed.response.replayed);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(replayed.response.business_workspace, inserted_workspace);

        let stale_upsert = upsert_acceptance_material_command(
            &project_id,
            &inserted_workspace,
            &batch_id,
            BusinessAcceptanceMaterialInput {
                id: Some(material_id.clone()),
                requirement_id: requirement.id.clone(),
                asset_id: ready_asset_id.clone(),
                kind: requirement.kind.clone(),
                group_key: "video-group-stale".to_string(),
                confirmed: true,
                duplicate_of_material_id: None,
                notes: "stale".to_string(),
            },
        );
        let updated = store.execute(upsert_acceptance_material_command(
            &project_id,
            &inserted_workspace,
            &batch_id,
            BusinessAcceptanceMaterialInput {
                id: Some(material_id),
                requirement_id: requirement.id,
                asset_id: ready_asset_id,
                kind: requirement.kind,
                group_key: "video-group-current".to_string(),
                confirmed: true,
                duplicate_of_material_id: None,
                notes: "current".to_string(),
            },
        ));
        let updated_workspace = updated.response.business_workspace;

        let stale_error =
            execute_command(&mut store.connection, &store.vault_root, stale_upsert).unwrap_err();
        assert_eq!(stale_error.code, "REVISION_CONFLICT");
        assert_eq!(
            load_workspace(&store.connection, &updated_workspace.id).unwrap(),
            updated_workspace
        );

        let mut idempotency_collision = upsert;
        if let BusinessWorkspaceCommandEnvelope::UpsertAcceptanceMaterial {
            command_id,
            payload,
            ..
        } = &mut idempotency_collision
        {
            *command_id = Uuid::new_v4().to_string();
            payload.material.notes = "collision".to_string();
        }
        let collision_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            idempotency_collision,
        )
        .unwrap_err();
        assert_eq!(collision_error.code, "IDEMPOTENCY_KEY_REUSED");
        assert_eq!(
            load_workspace(&store.connection, &updated_workspace.id).unwrap(),
            updated_workspace
        );
    }

    #[test]
    fn customer_receivables_use_stable_tax_identity_filter_limit_and_survive_restart() {
        let mut store = TestStore::new();
        let first_project_id = store.project_id.clone();
        let first_workspace = store
            .execute(create_command(&first_project_id))
            .response
            .business_workspace;
        let mut first_workspace =
            prepare_effective_contract(&mut store, &first_project_id, first_workspace);
        let first_contract_cents = first_workspace.financial_summary.contract_cents;
        first_workspace = upsert_test_payment(
            &mut store,
            &first_project_id,
            first_workspace,
            None,
            first_contract_cents / 2,
            BusinessPaymentStatus::Planned,
            None,
            "ACME-REQUEST",
            "",
        );
        let first_payment_id = first_workspace.payments[0].id.clone();
        let (next, _, _, _) = create_and_generate_test_document(
            &mut store,
            &first_project_id,
            first_workspace,
            BusinessDocumentKind::PaymentRequest,
            Some(first_payment_id.clone()),
            BusinessDocumentFormat::Docx,
        );
        first_workspace = record_test_receipt(
            &mut store,
            &first_project_id,
            next,
            &first_payment_id,
            2_000,
            1_900_000_000_000,
            "ACME-RECEIPT",
        );
        let mut first_profile = profile_input(&first_workspace.profile);
        first_profile.customer_name = "Acme Studio".to_string();
        first_profile.customer_legal_name = "Acme Holdings".to_string();
        first_profile.customer_tax_id = "TAX-ACME-1".to_string();
        first_profile.customer_contact = "Alice Legacy".to_string();
        first_profile.customer_phone = "13800000001".to_string();
        first_profile.customer_email = "alice@acme.test".to_string();
        first_workspace = store
            .execute(update_profile_command(
                &first_project_id,
                &first_workspace,
                first_profile,
            ))
            .response
            .business_workspace;

        let second_project_id = insert_project(
            &store.connection,
            "Acme Retainer",
            "Acme Holdings",
            Some(RequirementBriefContent {
                objective: "Retainer delivery".to_string(),
                deliverables: vec!["Monthly film".to_string()],
                ..RequirementBriefContent::default()
            }),
        );
        let second_workspace = store
            .execute(create_command(&second_project_id))
            .response
            .business_workspace;
        let mut second_workspace =
            prepare_effective_contract(&mut store, &second_project_id, second_workspace);
        let second_contract_cents = second_workspace.financial_summary.contract_cents;
        let mut second_profile = profile_input(&second_workspace.profile);
        second_profile.customer_name = "Acme Holdings".to_string();
        second_profile.customer_legal_name = "Acme Group".to_string();
        second_profile.customer_tax_id = "TAX-ACME-2".to_string();
        second_profile.customer_contact = "Bob Current".to_string();
        second_profile.customer_phone = "13800000002".to_string();
        second_profile.customer_email = "bob@acme.test".to_string();
        second_workspace = store
            .execute(update_profile_command(
                &second_project_id,
                &second_workspace,
                second_profile,
            ))
            .response
            .business_workspace;

        let third_project_id = insert_project(
            &store.connection,
            "Other Project",
            "Other Customer",
            Some(RequirementBriefContent {
                objective: "Other delivery".to_string(),
                deliverables: vec!["Other film".to_string()],
                ..RequirementBriefContent::default()
            }),
        );
        let mut third_workspace = store
            .execute(create_command(&third_project_id))
            .response
            .business_workspace;
        let mut third_profile = complete_profile_input(&third_workspace.profile);
        third_profile.customer_name = "Other Customer".to_string();
        third_profile.customer_legal_name = "Other Legal Co.".to_string();
        third_profile.customer_tax_id = "TAX-OTHER".to_string();
        third_profile.customer_contact = "Carol".to_string();
        third_profile.customer_phone = "13900000003".to_string();
        third_profile.customer_email = "carol@other.test".to_string();
        third_workspace = store
            .execute(update_profile_command(
                &third_project_id,
                &third_workspace,
                third_profile,
            ))
            .response
            .business_workspace;

        store
            .connection
            .execute(
                "UPDATE business_workspaces SET updated_at = 100 WHERE id = ?1",
                [&first_workspace.id],
            )
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE business_workspaces
                 SET status = 'archived', archived_at = 200, archived_by = 'test-archive', updated_at = 200
                 WHERE id = ?1",
                [&second_workspace.id],
            )
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE business_workspaces SET updated_at = 300 WHERE id = ?1",
                [&third_workspace.id],
            )
            .unwrap();

        let summaries = list_customers(
            &store.connection,
            &ListBusinessCustomersRequest {
                query: String::new(),
                limit: None,
            },
        )
        .unwrap();
        assert_eq!(summaries.len(), 3);
        let first_acme = summaries
            .iter()
            .find(|summary| summary.customer_tax_id == "TAX-ACME-1")
            .unwrap();
        assert_eq!(first_acme.workspace_count, 1);
        assert_eq!(first_acme.active_workspace_count, 1);
        assert_eq!(first_acme.contract_cents, first_contract_cents);
        assert_eq!(first_acme.requested_cents, first_contract_cents / 2);
        assert_eq!(first_acme.received_cents, 2_000);
        assert_eq!(first_acme.outstanding_cents, first_contract_cents - 2_000);
        assert_eq!(first_acme.workspace_ids, vec![first_workspace.id.clone()]);
        let second_acme = summaries
            .iter()
            .find(|summary| summary.customer_tax_id == "TAX-ACME-2")
            .unwrap();
        assert_eq!(second_acme.workspace_count, 1);
        assert_eq!(second_acme.active_workspace_count, 0);
        assert_eq!(second_acme.contract_cents, second_contract_cents);
        assert_eq!(second_acme.workspace_ids, vec![second_workspace.id.clone()]);
        assert_ne!(first_acme.customer_id, second_acme.customer_id);

        for query in [
            "Acme Studio",
            "Acme Group",
            "TAX-ACME-1",
            "Alice Legacy",
            "13800000001",
            "alice@acme.test",
        ] {
            let filtered = list_customers(
                &store.connection,
                &ListBusinessCustomersRequest {
                    query: query.to_string(),
                    limit: Some(100),
                },
            )
            .unwrap();
            assert_eq!(filtered.len(), 1, "query {query}");
            assert_eq!(filtered[0].workspace_count, 1, "query {query}");
        }

        let limited = list_customers(
            &store.connection,
            &ListBusinessCustomersRequest {
                query: String::new(),
                limit: Some(1),
            },
        )
        .unwrap();
        assert_eq!(limited.len(), 1);
        assert!(!limited[0].customer_id.is_empty());
        assert_eq!(
            list_customers(
                &store.connection,
                &ListBusinessCustomersRequest {
                    query: String::new(),
                    limit: Some(0),
                },
            )
            .unwrap_err()
            .code,
            "VALIDATION_FAILED"
        );
        assert_eq!(
            list_customers(
                &store.connection,
                &ListBusinessCustomersRequest {
                    query: String::new(),
                    limit: Some(MAX_CUSTOMER_LIST_LIMIT + 1),
                },
            )
            .unwrap_err()
            .code,
            "VALIDATION_FAILED"
        );
        assert_eq!(
            list_customers(
                &store.connection,
                &ListBusinessCustomersRequest {
                    query: "x".repeat(MAX_SHORT_CHARS + 1),
                    limit: Some(100),
                },
            )
            .unwrap_err()
            .code,
            "VALIDATION_FAILED"
        );

        store.reopen();
        let after_restart = list_customers(
            &store.connection,
            &ListBusinessCustomersRequest {
                query: "Alice Legacy".to_string(),
                limit: Some(100),
            },
        )
        .unwrap();
        assert_eq!(after_restart.len(), 1);
        assert_eq!(after_restart[0].received_cents, 2_000);
        assert_eq!(after_restart[0].active_workspace_count, 1);
    }

    #[test]
    fn prefill_candidates_match_both_customer_identities_sort_limit_and_include_archived() {
        let mut store = TestStore::new();
        let both_project_id = store.project_id.clone();
        let both_workspace = store
            .execute(create_command(&both_project_id))
            .response
            .business_workspace;

        let name_project_id =
            insert_project(&store.connection, "Name Match Source", "Customer Co.", None);
        let mut name_workspace = store
            .execute(create_command(&name_project_id))
            .response
            .business_workspace;
        let mut name_profile = profile_input(&name_workspace.profile);
        name_profile.customer_legal_name = "Another Legal Entity".to_string();
        name_workspace = store
            .execute(update_profile_command(
                &name_project_id,
                &name_workspace,
                name_profile,
            ))
            .response
            .business_workspace;

        let legal_project_id = insert_project(
            &store.connection,
            "Legal Match Source",
            "Temporary Customer",
            None,
        );
        let mut legal_workspace = store
            .execute(create_command(&legal_project_id))
            .response
            .business_workspace;
        let mut legal_profile = profile_input(&legal_workspace.profile);
        legal_profile.customer_name = "Different Display Name".to_string();
        legal_profile.customer_legal_name = "  CUSTOMER\u{2003}co.  ".to_string();
        legal_workspace = store
            .execute(update_profile_command(
                &legal_project_id,
                &legal_workspace,
                legal_profile,
            ))
            .response
            .business_workspace;

        let mismatch_project_id = insert_project(
            &store.connection,
            "Mismatch Source",
            "Unrelated Customer",
            None,
        );
        let mismatch_workspace = store
            .execute(create_command(&mismatch_project_id))
            .response
            .business_workspace;

        store
            .connection
            .execute(
                "UPDATE business_workspaces SET updated_at = CASE id
                     WHEN ?1 THEN 200 WHEN ?2 THEN 100 WHEN ?3 THEN 300 WHEN ?4 THEN 400
                 END,
                 status = CASE WHEN id = ?1 THEN 'archived' ELSE status END
                 WHERE id IN (?1, ?2, ?3, ?4)",
                params![
                    both_workspace.id,
                    name_workspace.id,
                    legal_workspace.id,
                    mismatch_workspace.id,
                ],
            )
            .unwrap();

        let target_project_id =
            insert_project(&store.connection, "Target Project", "customer co.", None);
        let before = table_counts(&store.connection);
        let candidates = list_prefill_candidates(
            &store.connection,
            &ListBusinessWorkspacePrefillCandidatesRequest {
                target_project_id: target_project_id.clone(),
                limit: None,
            },
        )
        .unwrap();
        assert_eq!(table_counts(&store.connection), before);
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0].source_workspace_id, legal_workspace.id);
        assert_eq!(
            candidates[0].match_kind,
            BusinessWorkspacePrefillMatchKind::CustomerLegalName
        );
        assert_eq!(candidates[1].source_workspace_id, both_workspace.id);
        assert_eq!(
            candidates[1].match_kind,
            BusinessWorkspacePrefillMatchKind::Both
        );
        assert_eq!(candidates[1].status, BusinessWorkspaceStatus::Archived);
        assert_eq!(candidates[2].source_workspace_id, name_workspace.id);
        assert_eq!(
            candidates[2].match_kind,
            BusinessWorkspacePrefillMatchKind::CustomerName
        );
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.source_workspace_id == mismatch_workspace.id));

        let limited = list_prefill_candidates(
            &store.connection,
            &ListBusinessWorkspacePrefillCandidatesRequest {
                target_project_id: target_project_id.clone(),
                limit: Some(2),
            },
        )
        .unwrap();
        assert_eq!(limited, candidates[..2]);

        let invalid_zero = list_prefill_candidates(
            &store.connection,
            &ListBusinessWorkspacePrefillCandidatesRequest {
                target_project_id: target_project_id.clone(),
                limit: Some(0),
            },
        )
        .unwrap_err();
        assert_eq!(invalid_zero.code, "VALIDATION_FAILED");

        let invalid = list_prefill_candidates(
            &store.connection,
            &ListBusinessWorkspacePrefillCandidatesRequest {
                target_project_id,
                limit: Some(101),
            },
        )
        .unwrap_err();
        assert_eq!(invalid.code, "VALIDATION_FAILED");
    }

    #[test]
    fn prefill_preview_is_read_only_explains_all_fields_survives_reopen_and_matches_create() {
        let mut store = TestStore::new();
        let source_project_id = store.project_id.clone();
        let mut source_workspace = store
            .execute(create_command(&source_project_id))
            .response
            .business_workspace;
        let mut source_profile = profile_input(&source_workspace.profile);
        source_profile.customer_legal_name = String::new();
        source_profile.customer_tax_id = "TAX-100".to_string();
        source_profile.currency = "USD".to_string();
        source_profile.default_tax_rate_bps = 600;
        source_workspace = store
            .execute(update_profile_command(
                &source_project_id,
                &source_workspace,
                source_profile,
            ))
            .response
            .business_workspace;

        let target_content = RequirementBriefContent {
            objective: "Target objective".to_string(),
            deliverables: vec!["Target deliverable".to_string()],
            acceptance_criteria: vec!["Written sign-off".to_string()],
            ..RequirementBriefContent::default()
        };
        let target_project_id = insert_project(
            &store.connection,
            "Target Preview Project",
            "Customer Co.",
            Some(target_content),
        );
        let request = PreviewBusinessWorkspacePrefillRequest {
            target_project_id: target_project_id.clone(),
            source_workspace_id: source_workspace.id.clone(),
        };
        let before = table_counts(&store.connection);
        let preview = preview_prefill(&store.connection, &request).unwrap();
        assert_eq!(table_counts(&store.connection), before);
        assert_eq!(preview.changes.len(), REUSABLE_PREFILL_FIELDS.len());
        assert_eq!(
            preview
                .changes
                .iter()
                .map(|change| change.field)
                .collect::<Vec<_>>(),
            REUSABLE_PREFILL_FIELDS
        );
        assert_eq!(
            preview.match_kind,
            BusinessWorkspacePrefillMatchKind::CustomerName
        );
        let decision = |field| {
            preview
                .changes
                .iter()
                .find(|change| change.field == field)
                .unwrap()
                .decision
        };
        // An empty source value no longer clears the target-derived legal
        // name; prefill is fill-only.
        assert_eq!(
            decision(BusinessWorkspacePrefillField::CustomerLegalName),
            BusinessWorkspacePrefillDecision::Unchanged
        );
        assert_eq!(
            decision(BusinessWorkspacePrefillField::CustomerTaxId),
            BusinessWorkspacePrefillDecision::Filled
        );
        assert_eq!(
            decision(BusinessWorkspacePrefillField::Currency),
            BusinessWorkspacePrefillDecision::Replaced
        );
        assert_eq!(
            decision(BusinessWorkspacePrefillField::CustomerPhone),
            BusinessWorkspacePrefillDecision::Unchanged
        );

        store.reopen();
        let reopened_preview = preview_prefill(&store.connection, &request).unwrap();
        assert_eq!(reopened_preview, preview);
        assert_eq!(table_counts(&store.connection), before);

        let created = store
            .execute(create_command_with_prefill(
                &target_project_id,
                &source_workspace.id,
            ))
            .response
            .business_workspace;
        for change in &preview.changes {
            assert_eq!(
                reusable_field_value(&created.profile, change.field),
                change.result_value
            );
        }
        assert_eq!(created.profile.project_title, "Target Preview Project");
        assert_eq!(
            created.profile.delivery_summary,
            "Target objective\nTarget deliverable"
        );
    }

    #[test]
    fn prefill_reads_normalize_ids_and_reject_malformed_ids() {
        let mut store = TestStore::new();
        let source_project_id = store.project_id.clone();
        let source_workspace = store
            .execute(create_command(&source_project_id))
            .response
            .business_workspace;
        let target_project_id = insert_project(
            &store.connection,
            "Normalized Read Target",
            "Customer Co.",
            None,
        );
        let before = table_counts(&store.connection);

        let candidates = list_prefill_candidates(
            &store.connection,
            &ListBusinessWorkspacePrefillCandidatesRequest {
                target_project_id: format!("  {}  ", target_project_id.to_uppercase()),
                limit: Some(100),
            },
        )
        .unwrap();
        assert!(candidates
            .iter()
            .any(|candidate| candidate.source_workspace_id == source_workspace.id));

        let preview = preview_prefill(
            &store.connection,
            &PreviewBusinessWorkspacePrefillRequest {
                target_project_id: format!("  {}  ", target_project_id.to_uppercase()),
                source_workspace_id: format!("  {}  ", source_workspace.id.to_uppercase()),
            },
        )
        .unwrap();
        assert_eq!(preview.target_project_id, target_project_id);
        assert_eq!(preview.source_workspace_id, source_workspace.id);

        for error in [
            list_prefill_candidates(
                &store.connection,
                &ListBusinessWorkspacePrefillCandidatesRequest {
                    target_project_id: " not-a-uuid ".to_string(),
                    limit: None,
                },
            )
            .unwrap_err(),
            preview_prefill(
                &store.connection,
                &PreviewBusinessWorkspacePrefillRequest {
                    target_project_id: " not-a-uuid ".to_string(),
                    source_workspace_id: source_workspace.id.clone(),
                },
            )
            .unwrap_err(),
            preview_prefill(
                &store.connection,
                &PreviewBusinessWorkspacePrefillRequest {
                    target_project_id: target_project_id.clone(),
                    source_workspace_id: " not-a-uuid ".to_string(),
                },
            )
            .unwrap_err(),
        ] {
            assert_eq!(error.code, "VALIDATION_FAILED");
        }
        assert_eq!(table_counts(&store.connection), before);
    }

    #[test]
    fn identity_backfill_does_not_rewrite_valid_empty_legal_name() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let mut profile = profile_input(&workspace.profile);
        profile.customer_legal_name.clear();
        let workspace = store
            .execute(update_profile_command(&project_id, &workspace, profile))
            .response
            .business_workspace;
        store
            .connection
            .execute(
                "UPDATE business_workspaces
                 SET customer_name_key = '', customer_legal_name_key = ''
                 WHERE id = ?1",
                [&workspace.id],
            )
            .unwrap();

        migrate(&store.connection).unwrap();
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT customer_name_key, customer_legal_name_key
                     FROM business_workspaces WHERE id = ?1",
                    [&workspace.id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .unwrap(),
            ("customerco.".to_string(), String::new())
        );
        let changes_after_backfill = store.connection.total_changes();
        migrate(&store.connection).unwrap();
        assert_eq!(store.connection.total_changes(), changes_after_backfill);
    }

    #[test]
    fn profile_updates_keep_identity_indexes_in_sync_and_existing_targets_are_rejected() {
        let mut store = TestStore::new();
        let source_project_id = store.project_id.clone();
        let mut source_workspace = store
            .execute(create_command(&source_project_id))
            .response
            .business_workspace;
        let old_target = insert_project(&store.connection, "Old Target", "Customer Co.", None);
        assert_eq!(
            list_prefill_candidates(
                &store.connection,
                &ListBusinessWorkspacePrefillCandidatesRequest {
                    target_project_id: old_target.clone(),
                    limit: None,
                },
            )
            .unwrap()
            .len(),
            1
        );

        let mut renamed = profile_input(&source_workspace.profile);
        renamed.customer_name = "Renamed Customer".to_string();
        renamed.customer_legal_name = "RENAMED\u{2002}CUSTOMER".to_string();
        source_workspace = store
            .execute(update_profile_command(
                &source_project_id,
                &source_workspace,
                renamed,
            ))
            .response
            .business_workspace;
        let keys = store
            .connection
            .query_row(
                "SELECT customer_name_key, customer_legal_name_key
                 FROM business_workspaces WHERE id = ?1",
                [&source_workspace.id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap();
        assert_eq!(keys.0, "renamedcustomer");
        assert_eq!(keys.1, "renamedcustomer");
        assert!(list_prefill_candidates(
            &store.connection,
            &ListBusinessWorkspacePrefillCandidatesRequest {
                target_project_id: old_target,
                limit: None,
            },
        )
        .unwrap()
        .is_empty());

        let renamed_target = insert_project(
            &store.connection,
            "Renamed Target",
            "renamed customer",
            None,
        );
        assert_eq!(
            list_prefill_candidates(
                &store.connection,
                &ListBusinessWorkspacePrefillCandidatesRequest {
                    target_project_id: renamed_target.clone(),
                    limit: None,
                },
            )
            .unwrap()[0]
                .source_workspace_id,
            source_workspace.id
        );

        let created_target = insert_project(
            &store.connection,
            "Already Created Target",
            "Renamed Customer",
            None,
        );
        store.execute(create_command(&created_target));
        let error = list_prefill_candidates(
            &store.connection,
            &ListBusinessWorkspacePrefillCandidatesRequest {
                target_project_id: created_target,
                limit: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_WORKSPACE_EXISTS");
    }

    #[test]
    fn prefill_read_errors_do_not_write_workspace_event_or_receipt() {
        let mut store = TestStore::new();
        let source_project_id = store.project_id.clone();
        let source_workspace = store
            .execute(create_command(&source_project_id))
            .response
            .business_workspace;

        let missing_source_target = insert_project(
            &store.connection,
            "Missing Preview Source Target",
            "Customer Co.",
            None,
        );
        let mismatch_target = insert_project(
            &store.connection,
            "Mismatched Preview Target",
            "Other Customer",
            None,
        );
        let existing_target = insert_project(
            &store.connection,
            "Existing Preview Target",
            "Customer Co.",
            None,
        );
        store.execute(create_command(&existing_target));

        let storage_counts = |connection: &Connection| {
            let workspaces = connection
                .query_row("SELECT COUNT(*) FROM business_workspaces", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap();
            let events = connection
                .query_row(
                    "SELECT COUNT(*) FROM business_workspace_events",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap();
            let receipts = connection
                .query_row(
                    "SELECT COUNT(*) FROM business_workspace_command_receipts",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap();
            (workspaces, events, receipts)
        };
        let before = storage_counts(&store.connection);

        let missing_source = preview_prefill(
            &store.connection,
            &PreviewBusinessWorkspacePrefillRequest {
                target_project_id: missing_source_target,
                source_workspace_id: Uuid::new_v4().to_string(),
            },
        )
        .unwrap_err();
        assert_eq!(missing_source.code, "BUSINESS_PREFILL_SOURCE_NOT_FOUND");
        assert_eq!(storage_counts(&store.connection), before);

        let mismatch = preview_prefill(
            &store.connection,
            &PreviewBusinessWorkspacePrefillRequest {
                target_project_id: mismatch_target,
                source_workspace_id: source_workspace.id.clone(),
            },
        )
        .unwrap_err();
        assert_eq!(mismatch.code, "BUSINESS_PREFILL_CUSTOMER_MISMATCH");
        assert_eq!(storage_counts(&store.connection), before);

        let existing = preview_prefill(
            &store.connection,
            &PreviewBusinessWorkspacePrefillRequest {
                target_project_id: existing_target,
                source_workspace_id: source_workspace.id,
            },
        )
        .unwrap_err();
        assert_eq!(existing.code, "BUSINESS_WORKSPACE_EXISTS");
        assert_eq!(storage_counts(&store.connection), before);

        let missing_project = list_prefill_candidates(
            &store.connection,
            &ListBusinessWorkspacePrefillCandidatesRequest {
                target_project_id: Uuid::new_v4().to_string(),
                limit: None,
            },
        )
        .unwrap_err();
        assert_eq!(missing_project.code, "PROJECT_NOT_FOUND");
        assert_eq!(storage_counts(&store.connection), before);
    }

    #[test]
    fn historical_master_data_prefill_is_whitelisted_snapshotted_and_durable() {
        let mut store = TestStore::new();
        assert_eq!(
            normalized_business_identity(" Customer Co. "),
            normalized_business_identity("customer co.")
        );
        let source_project_id = store.project_id.clone();
        let mut source_workspace = store
            .execute(create_command(&source_project_id))
            .response
            .business_workspace;
        let mut source_profile = complete_profile_input(&source_workspace.profile);
        source_profile.customer_legal_name = "Customer Co. Legal Ltd.".to_string();
        source_profile.currency = "USD".to_string();
        source_profile.default_tax_rate_bps = 875;
        source_profile.notes = "source-only notes".to_string();
        source_workspace = store
            .execute(update_profile_command(
                &source_project_id,
                &source_workspace,
                source_profile,
            ))
            .response
            .business_workspace;
        source_workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::UpsertPayment {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&source_project_id),
                payload: UpsertBusinessPaymentPayload {
                    workspace_id: source_workspace.id.clone(),
                    payment: BusinessPaymentInput {
                        id: None,
                        label: "Source deposit".to_string(),
                        amount_cents: 50_000,
                        due_at: Some(2_000_000_000_000),
                        occurred_at: None,
                        status: BusinessPaymentStatus::Planned,
                        reference: "SOURCE-ONLY".to_string(),
                        notes: String::new(),
                    },
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(source_workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        assert_eq!(source_workspace.payments.len(), 1);

        let target_content = RequirementBriefContent {
            objective: "Target launch objective".to_string(),
            key_message: "Target key message".to_string(),
            deliverables: vec!["Target hero film".to_string()],
            acceptance_criteria: vec!["Target written acceptance".to_string()],
            constraints: vec!["Target constraint".to_string()],
            risks: vec!["Target risk".to_string()],
            deadline_at: Some(2_100_000_000_000),
            ..RequirementBriefContent::default()
        };
        let target_project_id = insert_project(
            &store.connection,
            "Target Project",
            "Customer Co.",
            Some(target_content),
        );
        let create = create_command_with_prefill(&target_project_id, &source_workspace.id);
        let created = store.execute(create.clone());
        let target_workspace = created.response.business_workspace;

        assert_eq!(
            target_workspace.prefill_source_workspace_id.as_deref(),
            Some(source_workspace.id.as_str())
        );
        assert_eq!(target_workspace.profile.project_title, "Target Project");
        assert_eq!(target_workspace.profile.customer_name, "Customer Co.");
        assert_eq!(
            target_workspace.profile.customer_legal_name,
            "Customer Co. Legal Ltd."
        );
        assert_eq!(target_workspace.profile.customer_tax_id, "CUSTOMER-TAX-001");
        assert_eq!(
            target_workspace.profile.supplier_bank_account,
            "622200000001"
        );
        assert_eq!(target_workspace.profile.currency, "USD");
        assert_eq!(target_workspace.profile.default_tax_rate_bps, 875);
        assert_eq!(target_workspace.profile.service_start_at, None);
        assert_eq!(
            target_workspace.profile.service_end_at,
            Some(2_100_000_000_000)
        );
        assert!(target_workspace
            .profile
            .delivery_summary
            .contains("Target launch objective"));
        assert!(!target_workspace
            .profile
            .delivery_summary
            .contains("Final film and cutdowns"));
        assert_eq!(
            target_workspace.profile.acceptance_terms,
            "Target written acceptance"
        );
        assert_eq!(target_workspace.profile.payment_terms, "");
        assert!(target_workspace.profile.notes.contains("Target constraint"));
        assert!(!target_workspace.profile.notes.contains("source-only notes"));
        assert_eq!(target_workspace.profile.line_items.len(), 1);
        assert_eq!(
            target_workspace.profile.line_items[0].name,
            "Target hero film"
        );
        assert!(target_workspace.documents.is_empty());
        assert!(target_workspace.payments.is_empty());

        let replayed = store.execute(create.clone());
        assert!(replayed.response.replayed);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(list(&store.connection).unwrap().len(), 2);

        let mut conflicting = create;
        let BusinessWorkspaceCommandEnvelope::Create { payload, .. } = &mut conflicting else {
            unreachable!()
        };
        payload.prefill_source_workspace_id = None;
        let conflict =
            execute_command(&mut store.connection, &store.vault_root, conflicting).unwrap_err();
        assert_eq!(conflict.code, "IDEMPOTENCY_KEY_REUSED");

        let mut changed_source_profile = profile_input(&source_workspace.profile);
        changed_source_profile.customer_tax_id = "CUSTOMER-TAX-CHANGED".to_string();
        let frozen = execute_command(
            &mut store.connection,
            &store.vault_root,
            update_profile_command(
                &source_project_id,
                &source_workspace,
                changed_source_profile,
            ),
        )
        .unwrap_err();
        assert_eq!(frozen.code, "BUSINESS_CUSTOMER_BINDING_FROZEN");
        let unchanged_target = list(&store.connection)
            .unwrap()
            .into_iter()
            .find(|workspace| workspace.id == target_workspace.id)
            .unwrap();
        assert_eq!(unchanged_target.profile.customer_tax_id, "CUSTOMER-TAX-001");

        store.reopen();
        let reopened_target = list(&store.connection)
            .unwrap()
            .into_iter()
            .find(|workspace| workspace.id == target_workspace.id)
            .unwrap();
        assert_eq!(reopened_target, unchanged_target);
        let events = replay_events(&store.connection, 0, 100).unwrap();
        let target_created = events
            .into_iter()
            .find(|event| event.aggregate_id == target_workspace.id)
            .unwrap();
        let mut expected_event_workspace = reopened_target;
        expected_event_workspace.profile.supplier_bank_name.clear();
        expected_event_workspace
            .profile
            .supplier_bank_account
            .clear();
        assert_eq!(target_created.business_workspace, expected_event_workspace);
    }

    #[test]
    fn bank_details_stay_authoritative_but_never_enter_event_or_receipt_json() {
        const BANK_NAME: &str = "Confidential Settlement Bank";
        const BANK_ACCOUNT: &str = "6222999900001111222";

        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let mut input = profile_input(&workspace.profile);
        input.supplier_bank_name = BANK_NAME.to_string();
        input.supplier_bank_account = BANK_ACCOUNT.to_string();
        let command = update_profile_command(&project_id, &workspace, input);
        let outcome = store.execute(command.clone());

        assert!(!outcome.response.replayed);
        assert_eq!(
            outcome
                .response
                .business_workspace
                .profile
                .supplier_bank_name,
            BANK_NAME
        );
        assert_eq!(
            outcome
                .response
                .business_workspace
                .profile
                .supplier_bank_account,
            BANK_ACCOUNT
        );
        assert!(outcome.emitted_events[0]
            .business_workspace
            .profile
            .supplier_bank_name
            .is_empty());
        assert!(outcome.emitted_events[0]
            .business_workspace
            .profile
            .supplier_bank_account
            .is_empty());

        let command_id = outcome.response.receipt.command_id.clone();
        fn stored_journal_json(connection: &Connection, command_id: &str) -> (String, String) {
            let event: String = connection
                .query_row(
                    "SELECT payload_json FROM business_workspace_events WHERE command_id = ?1",
                    [command_id],
                    |row| row.get(0),
                )
                .unwrap();
            let receipt: String = connection
                .query_row(
                    "SELECT response_json FROM business_workspace_command_receipts
                     WHERE command_id = ?1",
                    [command_id],
                    |row| row.get(0),
                )
                .unwrap();
            (event, receipt)
        }
        let (event_json, receipt_json) = stored_journal_json(&store.connection, &command_id);
        for raw_json in [&event_json, &receipt_json] {
            assert!(!raw_json.contains(BANK_NAME));
            assert!(!raw_json.contains(BANK_ACCOUNT));
        }

        let authoritative_profile_json: String = store
            .connection
            .query_row(
                "SELECT profile_json FROM business_workspaces WHERE id = ?1",
                [&outcome.response.business_workspace.id],
                |row| row.get(0),
            )
            .unwrap();
        let authoritative_profile: BusinessProfile =
            serde_json::from_str(&authoritative_profile_json).unwrap();
        assert_eq!(authoritative_profile.supplier_bank_name, BANK_NAME);
        assert_eq!(authoritative_profile.supplier_bank_account, BANK_ACCOUNT);

        let replayed = store.execute(command);
        assert!(replayed.response.replayed);
        assert_eq!(
            replayed
                .response
                .business_workspace
                .profile
                .supplier_bank_name,
            BANK_NAME
        );
        assert_eq!(
            replayed
                .response
                .business_workspace
                .profile
                .supplier_bank_account,
            BANK_ACCOUNT
        );

        store
            .connection
            .execute(
                "UPDATE business_workspace_events SET payload_json = ?1 WHERE command_id = ?2",
                params![
                    serde_json::to_string(&outcome.response.business_workspace).unwrap(),
                    command_id
                ],
            )
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE business_workspace_command_receipts
                 SET response_json = ?1 WHERE command_id = ?2",
                params![
                    serde_json::to_string(&outcome.response).unwrap(),
                    command_id
                ],
            )
            .unwrap();
        migrate(&store.connection).unwrap();

        let (migrated_event_json, migrated_receipt_json) =
            stored_journal_json(&store.connection, &command_id);
        for raw_json in [&migrated_event_json, &migrated_receipt_json] {
            assert!(!raw_json.contains(BANK_NAME));
            assert!(!raw_json.contains(BANK_ACCOUNT));
        }
        let authoritative_profile_json: String = store
            .connection
            .query_row(
                "SELECT profile_json FROM business_workspaces WHERE id = ?1",
                [&outcome.response.business_workspace.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(authoritative_profile_json.contains(BANK_NAME));
        assert!(authoritative_profile_json.contains(BANK_ACCOUNT));
    }

    #[test]
    fn prefill_source_validation_rolls_back_without_receipt_or_event() {
        let mut store = TestStore::new();
        let source_project_id = store.project_id.clone();
        let source_workspace = store
            .execute(create_command(&source_project_id))
            .response
            .business_workspace;
        let before_workspaces: i64 = store
            .connection
            .query_row("SELECT COUNT(*) FROM business_workspaces", [], |row| {
                row.get(0)
            })
            .unwrap();
        let before_events: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM business_workspace_events",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let before_receipts: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM business_workspace_command_receipts",
                [],
                |row| row.get(0),
            )
            .unwrap();

        let missing_project_id = insert_project(
            &store.connection,
            "Missing Source Target",
            "Customer Co.",
            None,
        );
        let missing = execute_command(
            &mut store.connection,
            &store.vault_root,
            create_command_with_prefill(&missing_project_id, &Uuid::new_v4().to_string()),
        )
        .unwrap_err();
        assert_eq!(missing.code, "BUSINESS_PREFILL_SOURCE_NOT_FOUND");

        let mismatch_project_id = insert_project(
            &store.connection,
            "Other Customer Target",
            "Other Customer",
            None,
        );
        let mismatch = execute_command(
            &mut store.connection,
            &store.vault_root,
            create_command_with_prefill(&mismatch_project_id, &source_workspace.id),
        )
        .unwrap_err();
        assert_eq!(mismatch.code, "BUSINESS_PREFILL_CUSTOMER_MISMATCH");

        assert_eq!(
            store
                .connection
                .query_row("SELECT COUNT(*) FROM business_workspaces", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            before_workspaces
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM business_workspace_events",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            before_events
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM business_workspace_command_receipts",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            before_receipts
        );
    }

    #[test]
    fn legacy_business_documents_without_review_columns_migrate_idempotently() {
        let temporary = tempfile::tempdir().unwrap();
        let database_path = temporary.path().join("legacy-business-documents.sqlite3");
        let connection = Connection::open(&database_path).unwrap();
        connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys = ON;
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
                "#,
            )
            .unwrap();
        asset_service::migrate(&connection).unwrap();
        crate::requirement_brief_service::migrate(&connection).unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE business_workspaces (
                    id TEXT PRIMARY KEY NOT NULL,
                    project_id TEXT NOT NULL UNIQUE,
                    requirement_brief_id TEXT,
                    profile_json TEXT NOT NULL,
                    status TEXT NOT NULL CHECK(status IN ('active','archived')),
                    revision INTEGER NOT NULL CHECK(revision >= 1),
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE RESTRICT,
                    FOREIGN KEY(requirement_brief_id) REFERENCES requirement_briefs(id) ON DELETE RESTRICT
                );
                CREATE TABLE business_documents (
                    id TEXT PRIMARY KEY NOT NULL,
                    workspace_id TEXT NOT NULL,
                    kind TEXT NOT NULL CHECK(kind IN ('quote','contract','paymentRequest','acceptance')),
                    sequence_number INTEGER NOT NULL CHECK(sequence_number >= 1),
                    document_number TEXT NOT NULL,
                    title TEXT NOT NULL,
                    template_key TEXT NOT NULL,
                    status TEXT NOT NULL CHECK(status IN ('draft','inReview','approved','generated','effective','voided')),
                    snapshot_json TEXT NOT NULL,
                    output_asset_id TEXT,
                    output_format TEXT CHECK(output_format IS NULL OR output_format IN ('docx','xlsx')),
                    approved_at INTEGER,
                    approved_by TEXT,
                    generated_at INTEGER,
                    revision INTEGER NOT NULL CHECK(revision >= 1),
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    UNIQUE(workspace_id, kind, sequence_number),
                    UNIQUE(workspace_id, document_number),
                    CHECK(
                        (status IN ('approved','generated','effective') AND approved_at IS NOT NULL AND approved_by IS NOT NULL)
                        OR status NOT IN ('approved','generated','effective')
                    ),
                    CHECK(
                        (status IN ('generated','effective') AND output_asset_id IS NOT NULL AND output_format IS NOT NULL AND generated_at IS NOT NULL)
                        OR status NOT IN ('generated','effective')
                    ),
                    FOREIGN KEY(workspace_id) REFERENCES business_workspaces(id) ON DELETE RESTRICT,
                    FOREIGN KEY(output_asset_id) REFERENCES assets(id) ON DELETE RESTRICT
                );
                CREATE INDEX idx_business_documents_workspace
                    ON business_documents(workspace_id, created_at ASC, id ASC);
                CREATE UNIQUE INDEX idx_business_documents_output_asset
                    ON business_documents(output_asset_id) WHERE output_asset_id IS NOT NULL;
                "#,
            )
            .unwrap();

        let project_id = Uuid::new_v4().to_string();
        let workspace_id = Uuid::new_v4().to_string();
        let document_id = Uuid::new_v4().to_string();
        let profile = BusinessProfile {
            project_title: "Legacy Contract".to_string(),
            customer_name: "Legacy Customer".to_string(),
            customer_legal_name: "Legacy Customer Co., Ltd.".to_string(),
            currency: "CNY".to_string(),
            ..BusinessProfile::default()
        };
        connection
            .execute(
                "INSERT INTO projects
                 (id, name, client_name, brief_json, stage, revision, created_at, updated_at)
                 VALUES (?1, 'Legacy Contract', 'Legacy Customer', ?2, 'intake', 1, 1, 1)",
                params![
                    project_id,
                    serde_json::to_string(&BriefRecord::default()).unwrap()
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO business_workspaces
                 (id, project_id, requirement_brief_id, profile_json, status, revision,
                  created_at, updated_at)
                 VALUES (?1, ?2, NULL, ?3, 'active', 1, 1, 1)",
                params![
                    workspace_id,
                    project_id,
                    serde_json::to_string(&profile).unwrap()
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO business_documents
                 (id, workspace_id, kind, sequence_number, document_number, title,
                  template_key, status, snapshot_json, output_asset_id, output_format,
                  approved_at, approved_by, generated_at, revision, created_at, updated_at)
                 VALUES (?1, ?2, 'contract', 1, 'LEGACY-CONTRACT-001',
                         'Legacy Contract Draft', 'contract.standard', 'draft', '{}',
                         NULL, NULL, NULL, NULL, NULL, 1, 1, 1)",
                params![document_id, workspace_id],
            )
            .unwrap();

        migrate(&connection).unwrap();

        for column in [
            "source_asset_id",
            "review_id",
            "report_asset_id",
            "evidence_json",
            "manual_waiver_json",
            "voided_at",
            "voided_by",
            "void_reason",
            "acceptance_batch_id",
            "acceptance_output_spec_id",
        ] {
            assert_eq!(
                connection
                    .query_row(
                        "SELECT EXISTS(
                             SELECT 1 FROM pragma_table_info('business_documents') WHERE name = ?1
                         )",
                        [column],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap(),
                1,
                "missing migrated column {column}"
            );
        }
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'index' AND name = 'idx_business_documents_review'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'index'
                       AND name = 'idx_business_documents_acceptance_output'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT document_number, title, snapshot_json, revision
                     FROM business_documents WHERE id = ?1",
                    [&document_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    },
                )
                .unwrap(),
            (
                "LEGACY-CONTRACT-001".to_string(),
                "Legacy Contract Draft".to_string(),
                "{}".to_string(),
                1,
            )
        );

        let changes_after_first_migration = connection.total_changes();
        migrate(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes_after_first_migration);
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM business_documents WHERE id = ?1",
                    [&document_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn version_1_4_schema_and_commands_migrate_without_losing_receipts() {
        let temporary = tempfile::tempdir().unwrap();
        let database_path = temporary.path().join("legacy.sqlite3");
        let vault_root = temporary.path().join("vault");
        fs::create_dir_all(&vault_root).unwrap();
        let mut connection = Connection::open(&database_path).unwrap();
        connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys = ON;
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
                CREATE TABLE business_workspaces (
                    id TEXT PRIMARY KEY NOT NULL,
                    project_id TEXT NOT NULL UNIQUE,
                    requirement_brief_id TEXT,
                    profile_json TEXT NOT NULL,
                    status TEXT NOT NULL CHECK(status IN ('active','archived')),
                    revision INTEGER NOT NULL CHECK(revision >= 1),
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE RESTRICT
                );
                CREATE TABLE business_workspace_command_receipts (
                    idempotency_key TEXT PRIMARY KEY NOT NULL,
                    command_id TEXT NOT NULL UNIQUE,
                    command_type TEXT NOT NULL,
                    protocol_version TEXT NOT NULL CHECK(protocol_version = '1.4'),
                    deadline_at INTEGER,
                    request_fingerprint TEXT NOT NULL CHECK(length(request_fingerprint) = 64),
                    response_json TEXT NOT NULL,
                    completed_at INTEGER NOT NULL
                );
                CREATE INDEX idx_business_workspace_receipts_completed
                    ON business_workspace_command_receipts(completed_at);
                "#,
            )
            .unwrap();
        asset_service::migrate(&connection).unwrap();
        crate::requirement_brief_service::migrate(&connection).unwrap();

        let old_project_id = insert_project(&connection, "Legacy Project", "Legacy Customer", None);
        let old_workspace_id = Uuid::new_v4().to_string();
        let old_profile = BusinessProfile {
            project_title: "Legacy Project".to_string(),
            customer_name: "Legacy Customer".to_string(),
            customer_legal_name: "Legacy Customer".to_string(),
            currency: "CNY".to_string(),
            ..BusinessProfile::default()
        };
        connection
            .execute(
                "INSERT INTO business_workspaces
                 (id, project_id, requirement_brief_id, profile_json, status, revision,
                  created_at, updated_at)
                 VALUES (?1, ?2, NULL, ?3, 'active', 1, 1, 1)",
                params![
                    old_workspace_id,
                    old_project_id,
                    serde_json::to_string(&old_profile).unwrap()
                ],
            )
            .unwrap();

        let mut legacy_command = create_command(&old_project_id);
        let (legacy_command_id, legacy_idempotency_key) = match &mut legacy_command {
            BusinessWorkspaceCommandEnvelope::Create {
                command_id,
                protocol_version,
                idempotency_key,
                deadline_at,
                ..
            } => {
                *protocol_version = BUSINESS_WORKSPACE_LEGACY_PROTOCOL_VERSION.to_string();
                *deadline_at = None;
                (command_id.clone(), idempotency_key.clone())
            }
            _ => unreachable!(),
        };
        let legacy_fingerprint = command_fingerprint(
            &normalize_command(legacy_command.clone()).expect("1.4 create command must normalize"),
        )
        .unwrap();
        let legacy_workspace = BusinessWorkspaceRecord {
            id: old_workspace_id.clone(),
            project_id: old_project_id.clone(),
            customer_id: String::new(),
            customer: Default::default(),
            requirement_brief_id: None,
            requirement_brief_revision: None,
            prefill_source_workspace_id: None,
            profile: old_profile,
            documents: Vec::new(),
            acceptance_batches: Vec::new(),
            template_versions: Vec::new(),
            payments: Vec::new(),
            quote_confirmations: Vec::new(),
            receipts: Vec::new(),
            milestones: Vec::new(),
            settlement_batches: Vec::new(),
            delivery_submissions: Vec::new(),
            invoices: Vec::new(),
            archive_snapshots: Vec::new(),
            archive_integrity_status: BusinessArchiveIntegrityStatus::NotCaptured,
            status: BusinessWorkspaceStatus::Active,
            archived_at: None,
            archived_by: None,
            lifecycle_stage: BusinessLifecycleStage::Draft,
            financial_summary: BusinessFinancialSummary::default(),
            current_documents: BusinessCurrentDocuments::default(),
            revision: 1,
            created_at: 1,
            updated_at: 1,
        };
        let legacy_response = BusinessWorkspaceCommandResponse {
            receipt: CommandReceipt {
                command_id: legacy_command_id.clone(),
                idempotency_key: legacy_idempotency_key.clone(),
                command_type: "businessWorkspace.create".to_string(),
                aggregate_id: old_workspace_id.clone(),
                revision: 1,
                last_event_sequence: 0,
                completed_at: 1,
            },
            business_workspace: legacy_workspace.clone(),
            replayed: false,
        };
        let mut legacy_response_json = serde_json::to_value(&legacy_response).unwrap();
        legacy_response_json
            .get_mut("businessWorkspace")
            .and_then(serde_json::Value::as_object_mut)
            .unwrap()
            .remove("prefillSourceWorkspaceId");
        connection
            .execute(
                "INSERT INTO business_workspace_command_receipts
                 (idempotency_key, command_id, command_type, protocol_version, deadline_at,
                  request_fingerprint, response_json, completed_at)
                 VALUES (?1, ?2, 'businessWorkspace.create', '1.4', NULL, ?3, ?4, 1)",
                params![
                    legacy_idempotency_key,
                    legacy_command_id,
                    legacy_fingerprint,
                    serde_json::to_string(&legacy_response_json).unwrap()
                ],
            )
            .unwrap();

        migrate(&connection).unwrap();
        let identity_keys = connection
            .query_row(
                "SELECT customer_name_key, customer_legal_name_key
                 FROM business_workspaces WHERE id = ?1",
                [&old_workspace_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap();
        assert_eq!(identity_keys.0, "legacycustomer");
        assert_eq!(identity_keys.1, "legacycustomer");
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'index' AND name IN (
                       'idx_business_workspaces_customer_name_key_updated',
                       'idx_business_workspaces_customer_legal_name_key_updated'
                     )",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            2
        );
        let legacy_target_id =
            insert_project(&connection, "Legacy Follow-up", " legacy CUSTOMER ", None);
        let legacy_candidates = list_prefill_candidates(
            &connection,
            &ListBusinessWorkspacePrefillCandidatesRequest {
                target_project_id: legacy_target_id.clone(),
                limit: None,
            },
        )
        .unwrap();
        assert_eq!(legacy_candidates.len(), 1);
        assert_eq!(legacy_candidates[0].source_workspace_id, old_workspace_id);
        let legacy_preview = preview_prefill(
            &connection,
            &PreviewBusinessWorkspacePrefillRequest {
                target_project_id: legacy_target_id,
                source_workspace_id: old_workspace_id.clone(),
            },
        )
        .unwrap();
        assert_eq!(legacy_preview.changes.len(), REUSABLE_PREFILL_FIELDS.len());
        migrate(&connection).unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT customer_name_key, customer_legal_name_key
                     FROM business_workspaces WHERE id = ?1",
                    [&old_workspace_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .unwrap(),
            identity_keys
        );
        let migrated = list(&connection).unwrap();
        assert_eq!(migrated.len(), 1);
        assert_eq!(migrated[0].id, legacy_workspace.id);
        assert_eq!(migrated[0].project_id, legacy_workspace.project_id);
        assert_eq!(migrated[0].profile, legacy_workspace.profile);
        assert!(!migrated[0].customer_id.is_empty());
        assert_eq!(migrated[0].customer.display_name, "Legacy Customer");
        assert_eq!(migrated[0].customer.legal_name, "Legacy Customer");
        let replayed = execute_command(&mut connection, &vault_root, legacy_command).unwrap();
        assert!(replayed.response.replayed);
        assert_eq!(replayed.response.business_workspace, migrated[0]);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM business_workspace_command_receipts
                     WHERE protocol_version = '1.4'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );

        let current_project_id =
            insert_project(&connection, "Current Project", "Current Customer", None);
        let current_command = create_command(&current_project_id);
        let current_command_id = match &current_command {
            BusinessWorkspaceCommandEnvelope::Create { command_id, .. } => command_id.clone(),
            _ => unreachable!(),
        };
        execute_command(&mut connection, &vault_root, current_command).unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT protocol_version FROM business_workspace_command_receipts
                     WHERE command_id = ?1",
                    [current_command_id],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            BUSINESS_WORKSPACE_PROTOCOL_VERSION
        );

        let compatible_project_id = insert_project(
            &connection,
            "Compatible Project",
            "Compatible Customer",
            None,
        );
        let mut compatible = create_command(&compatible_project_id);
        let compatible_command_id = match &mut compatible {
            BusinessWorkspaceCommandEnvelope::Create {
                command_id,
                protocol_version,
                ..
            } => {
                *protocol_version = BUSINESS_WORKSPACE_PREVIOUS_PROTOCOL_VERSION.to_string();
                command_id.clone()
            }
            _ => unreachable!(),
        };
        execute_command(&mut connection, &vault_root, compatible).unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT protocol_version FROM business_workspace_command_receipts
                     WHERE command_id = ?1",
                    [compatible_command_id],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            BUSINESS_WORKSPACE_PREVIOUS_PROTOCOL_VERSION
        );

        let unsupported_project_id =
            insert_project(&connection, "Unsupported Prefill", "Legacy Customer", None);
        let mut unsupported =
            create_command_with_prefill(&unsupported_project_id, &old_workspace_id);
        let BusinessWorkspaceCommandEnvelope::Create {
            protocol_version, ..
        } = &mut unsupported
        else {
            unreachable!()
        };
        *protocol_version = BUSINESS_WORKSPACE_LEGACY_PROTOCOL_VERSION.to_string();
        let error = execute_command(&mut connection, &vault_root, unsupported).unwrap_err();
        assert_eq!(error.code, "BUSINESS_PREFILL_PROTOCOL_UNSUPPORTED");
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM business_workspace_command_receipts",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            3
        );
    }

    #[test]
    fn version_1_1_business_rows_upgrade_to_closure_schema_idempotently() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let event_count: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM business_workspace_events",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let receipt_count: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM business_workspace_command_receipts",
                [],
                |row| row.get(0),
            )
            .unwrap();

        store
            .connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys = OFF;
                DROP TABLE business_archive_snapshots;
                DROP TABLE business_invoices;
                DROP TABLE business_delivery_submissions;
                DROP TABLE business_deliverable_versions;
                DROP TABLE business_delivery_milestones;
                DROP TABLE business_customer_conflicts;
                DROP TABLE business_customer_backfill;
                DROP TABLE business_workspace_customers;
                DROP TABLE business_customers;
                PRAGMA foreign_keys = ON;
                "#,
            )
            .unwrap();

        migrate(&store.connection).unwrap();
        let migrated = load_workspace(&store.connection, &workspace.id).unwrap();
        assert_eq!(migrated.id, workspace.id);
        assert_eq!(migrated.project_id, workspace.project_id);
        assert_eq!(migrated.profile, workspace.profile);
        assert_eq!(migrated.status, workspace.status);
        assert_eq!(migrated.revision, workspace.revision);
        assert!(!migrated.customer_id.is_empty());
        assert_eq!(
            migrated.customer.display_name,
            workspace.profile.customer_name
        );
        assert_eq!(
            migrated.customer.legal_name,
            workspace.profile.customer_legal_name
        );
        assert!(migrated.milestones.is_empty());
        assert!(migrated.delivery_submissions.is_empty());
        assert!(migrated.invoices.is_empty());
        assert!(migrated.archive_snapshots.is_empty());
        for table in [
            "business_customers",
            "business_workspace_customers",
            "business_customer_conflicts",
            "business_customer_backfill",
            "business_delivery_milestones",
            "business_deliverable_versions",
            "business_delivery_submissions",
            "business_invoices",
            "business_archive_snapshots",
        ] {
            assert_eq!(
                store
                    .connection
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                        [table],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap(),
                1,
                "missing 1.2 table {table}"
            );
        }
        assert_eq!(
            store
                .connection
                .query_row("SELECT COUNT(*) FROM business_customers", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM business_workspace_events",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            event_count
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM business_workspace_command_receipts",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            receipt_count
        );

        let changes_after_upgrade = store.connection.total_changes();
        migrate(&store.connection).unwrap();
        assert_eq!(store.connection.total_changes(), changes_after_upgrade);
        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            migrated
        );
    }

    #[test]
    fn baietan_tax_inclusive_profile_calculates_discounted_total_once() {
        let profile = normalize_profile(
            BusinessProfileInput {
                project_title: "Baietan".to_string(),
                customer_name: "Customer".to_string(),
                currency: "CNY".to_string(),
                tax_mode: BusinessTaxMode::TaxInclusive,
                default_tax_rate_bps: 600,
                project_discount_cents: 490_000,
                line_items: vec![BusinessLineItemInput {
                    id: None,
                    name: "Video production".to_string(),
                    description: String::new(),
                    quantity_millis: 4_000,
                    unit: "item".to_string(),
                    unit_price_cents: 2_120_000,
                    tax_rate_bps: 600,
                }],
                ..BusinessProfileInput::default()
            },
            &[],
        )
        .unwrap();

        assert_eq!(profile.line_items[0].unit_price_cents, 2_120_000);
        assert_eq!(profile.line_items[0].amount_cents, 8_480_000);
        assert_eq!(
            profile.quotation_totals,
            Some(BusinessQuotationTotals {
                original_total_cents: 8_480_000,
                project_discount_cents: 490_000,
                tax_exclusive_total_cents: 7_537_736,
                tax_cents: 452_264,
                final_total_cents: 7_990_000,
            })
        );
    }

    #[test]
    fn project_discount_cannot_exceed_original_total() {
        let error = normalize_profile(
            BusinessProfileInput {
                project_title: "Discount validation".to_string(),
                customer_name: "Customer".to_string(),
                currency: "CNY".to_string(),
                project_discount_cents: 101,
                line_items: vec![BusinessLineItemInput {
                    id: None,
                    name: "Service".to_string(),
                    description: String::new(),
                    quantity_millis: 1_000,
                    unit: "item".to_string(),
                    unit_price_cents: 100,
                    tax_rate_bps: 0,
                }],
                ..BusinessProfileInput::default()
            },
            &[],
        )
        .unwrap_err();

        assert_eq!(error.code, "VALIDATION_FAILED");
        assert!(error.message.contains("must not exceed"));
    }

    #[test]
    fn complete_durable_business_workspace_flow() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let mut create = create_command(&project_id);
        let created = store.execute(create.clone());
        let mut workspace = created.response.business_workspace;
        assert_eq!(workspace.profile.project_title, "Launch Project");
        assert_eq!(workspace.profile.customer_name, "Customer Co.");
        assert_eq!(workspace.profile.line_items.len(), 2);
        assert_eq!(workspace.profile.line_items[0].quantity_millis, 1_000);
        assert_eq!(workspace.profile.line_items[0].unit_price_cents, 0);
        assert_eq!(workspace.profile.line_items[0].amount_cents, 0);
        assert!(workspace
            .profile
            .delivery_summary
            .contains("Make it memorable"));
        assert_eq!(workspace.profile.acceptance_terms, "Customer signs off");

        let mut profile = profile_input(&workspace.profile);
        let server_line_id = profile.line_items[0].id.clone().unwrap();
        profile.supplier_legal_name = "BSAIGC Supplier Ltd.".to_string();
        profile.line_items[0].quantity_millis = 1_500;
        profile.line_items[0].unit_price_cents = 101;
        let stale_workspace = workspace.clone();
        workspace = store
            .execute(update_profile_command(&project_id, &workspace, profile))
            .response
            .business_workspace;
        assert_eq!(workspace.profile.line_items[0].id, server_line_id);
        assert_eq!(workspace.profile.line_items[0].amount_cents, 152);
        let mut forged = profile_input(&workspace.profile);
        forged.line_items[0].id = Some(Uuid::new_v4().to_string());
        assert_eq!(
            execute_command(
                &mut store.connection,
                &store.vault_root,
                update_profile_command(&project_id, &workspace, forged),
            )
            .unwrap_err()
            .code,
            "BUSINESS_LINE_ITEM_ID_INVALID"
        );
        assert_eq!(
            execute_command(
                &mut store.connection,
                &store.vault_root,
                update_profile_command(
                    &project_id,
                    &stale_workspace,
                    profile_input(&stale_workspace.profile),
                ),
            )
            .unwrap_err()
            .code,
            "REVISION_CONFLICT"
        );

        let complete_profile = complete_profile_input(&workspace.profile);
        workspace = store
            .execute(update_profile_command(
                &project_id,
                &workspace,
                complete_profile,
            ))
            .response
            .business_workspace;
        assert_eq!(workspace.profile.line_items[0].amount_cents, 15_900);

        let (next, quote_generate, quote_id, stable_asset_id) = create_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::Quote,
            None,
            BusinessDocumentFormat::Xlsx,
        );
        workspace = next;
        assert_eq!(workspace.lifecycle_stage, BusinessLifecycleStage::Quoted);
        assert_eq!(
            workspace.current_documents.quote_document_id.as_deref(),
            Some(quote_id.as_str())
        );
        assert_eq!(workspace.financial_summary.quoted_cents, 15_900);
        let quote_path = asset_service::resolve_original_path(
            &store.connection,
            &store.vault_root,
            &stable_asset_id,
        )
        .unwrap();
        assert!(fs::read(&quote_path).unwrap().starts_with(b"PK\x03\x04"));
        let mut archive = ZipArchive::new(File::open(quote_path).unwrap()).unwrap();
        archive.by_name("xl/workbook.xml").unwrap();
        archive.by_name("xl/worksheets/sheet1.xml").unwrap();

        let (next, _, contract_id, _) = create_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::Contract,
            None,
            BusinessDocumentFormat::Docx,
        );
        workspace = make_test_document_effective(&mut store, &project_id, next, &contract_id);
        assert_eq!(
            workspace.lifecycle_stage,
            BusinessLifecycleStage::Contracted
        );
        let contract_cents = workspace.financial_summary.contract_cents;
        assert_eq!(contract_cents, 15_900);

        workspace = upsert_test_payment(
            &mut store,
            &project_id,
            workspace,
            None,
            contract_cents,
            BusinessPaymentStatus::Planned,
            None,
            "PLAN-1",
            "",
        );
        let payment_id = workspace.payments[0].id.clone();
        let (next, _, payment_request_id, _) = create_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::PaymentRequest,
            Some(payment_id.clone()),
            BusinessDocumentFormat::Docx,
        );
        workspace = next;
        assert_eq!(
            workspace.payments[0].status,
            BusinessPaymentStatus::Requested
        );
        assert_eq!(
            workspace.lifecycle_stage,
            BusinessLifecycleStage::PaymentRequested
        );
        assert_eq!(
            workspace
                .current_documents
                .payment_request_document_id
                .as_deref(),
            Some(payment_request_id.as_str())
        );

        let (next, _, acceptance_id, _) = create_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::Acceptance,
            None,
            BusinessDocumentFormat::Docx,
        );
        workspace = make_test_document_effective(&mut store, &project_id, next, &acceptance_id);
        assert_eq!(workspace.lifecycle_stage, BusinessLifecycleStage::Accepted);

        workspace = record_test_receipt(
            &mut store,
            &project_id,
            workspace,
            &payment_id,
            contract_cents,
            1_900_000_000_000,
            "BANK-1",
        );
        assert_eq!(workspace.lifecycle_stage, BusinessLifecycleStage::Paid);
        assert_eq!(workspace.financial_summary.outstanding_cents, 0);
        assert_eq!(workspace.financial_summary.received_cents, contract_cents);

        workspace = prepare_test_business_closure(&mut store, &project_id, workspace);
        workspace = change_test_workspace_status(
            &mut store,
            &project_id,
            workspace,
            BusinessWorkspaceStatus::Archived,
        );
        assert_eq!(workspace.status, BusinessWorkspaceStatus::Archived);
        assert_eq!(workspace.lifecycle_stage, BusinessLifecycleStage::Archived);
        workspace = change_test_workspace_status(
            &mut store,
            &project_id,
            workspace,
            BusinessWorkspaceStatus::Active,
        );
        assert_eq!(workspace.status, BusinessWorkspaceStatus::Active);
        assert_eq!(workspace.lifecycle_stage, BusinessLifecycleStage::Paid);

        let all_events = replay_events(&store.connection, 0, 10_000).unwrap();
        assert!(all_events.len() >= 20);
        assert!(all_events
            .windows(2)
            .all(|window| window[1].sequence == window[0].sequence + 1));
        assert!(all_events.iter().all(|event| {
            !event.actor_id.is_empty() && !event.command_id.is_empty() && !event.reason.is_empty()
        }));
        assert_eq!(
            replay_events(&store.connection, 0, 0).unwrap_err().code,
            "VALIDATION_FAILED"
        );

        store.reopen();
        let generation_replay = store.execute(quote_generate);
        assert!(generation_replay.response.replayed);
        assert!(generation_replay.emitted_asset_events.is_empty());
        assert_eq!(
            generation_replay
                .response
                .business_workspace
                .documents
                .iter()
                .find(|document| document.id == quote_id)
                .and_then(|document| document.output_asset_id.as_deref()),
            Some(stable_asset_id.as_str())
        );
        assert_eq!(
            asset_service::list_assets(&store.connection, Some(&project_id))
                .unwrap()
                .len(),
            9
        );

        if let BusinessWorkspaceCommandEnvelope::Create { deadline_at, .. } = &mut create {
            *deadline_at = Some(1);
        }
        let create_replay = store.execute(create);
        assert!(create_replay.response.replayed);
        let persisted = list(&store.connection).unwrap().pop().unwrap();
        assert_eq!(persisted.lifecycle_stage, BusinessLifecycleStage::Paid);
        assert_eq!(persisted.financial_summary.outstanding_cents, 0);
    }

    #[test]
    fn protocol_status_and_format_gates_run_before_side_effects() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let mut unsupported = create_command(&project_id);
        if let BusinessWorkspaceCommandEnvelope::Create {
            protocol_version, ..
        } = &mut unsupported
        {
            *protocol_version = "1.3".to_string();
        }
        assert_eq!(
            execute_command(&mut store.connection, &store.vault_root, unsupported)
                .unwrap_err()
                .code,
            "PROTOCOL_VERSION_UNSUPPORTED"
        );
        let mut workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::CreateDocument {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: CreateBusinessDocumentPayload {
                    workspace_id: workspace.id.clone(),
                    kind: BusinessDocumentKind::Quote,
                    document_number: "Q-DRAFT".to_string(),
                    title: "Draft".to_string(),
                    template_key: document_engine::QUOTE_TEMPLATE_KEY.to_string(),
                    payment_id: None,
                    acceptance_batch_id: None,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        let error = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::GenerateDocument {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: GenerateBusinessDocumentPayload {
                    workspace_id: workspace.id.clone(),
                    document_id: workspace.documents[0].id.clone(),
                    format: BusinessDocumentFormat::Docx,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_DOCUMENT_NOT_APPROVED");
        assert!(
            asset_service::list_assets(&store.connection, Some(&project_id))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn kind_specific_approval_gates_match_the_document_contract() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let mut profile = workspace.profile.clone();
        profile.supplier_legal_name = "Supplier".to_string();

        let make_document =
            |kind: BusinessDocumentKind, profile: BusinessProfile| BusinessDocumentRecord {
                id: Uuid::new_v4().to_string(),
                kind,
                sequence_number: 1,
                document_number: "TEST-1".to_string(),
                title: "Gate test".to_string(),
                template_key: document_engine::QUOTE_TEMPLATE_KEY.to_string(),
                status: BusinessDocumentStatus::InReview,
                snapshot: BusinessDocumentSnapshot {
                    workspace_revision: workspace.revision,
                    acceptance_batch_id: None,
                    acceptance_output_spec_id: None,
                    acceptance_batch_revision: None,
                    material_bindings: Vec::new(),
                    template_asset_id: None,
                    template_source_sha256: None,
                    template_mapping_version: String::new(),
                    contract_settlement: None,
                    service_settlement_items: Vec::new(),
                    payment_application: None,
                    video_completion_acceptance: None,
                    production_result_confirmation: None,
                    customer_id: workspace.customer_id.clone(),
                    customer: workspace.customer.clone(),
                    profile,
                    payment: None,
                },
                output_asset_id: None,
                output_format: None,
                source_asset_id: None,
                review_id: None,
                report_asset_id: None,
                evidence: None,
                manual_waiver: None,
                voided_at: None,
                voided_by: None,
                void_reason: String::new(),
                approved_at: None,
                approved_by: None,
                generated_at: None,
                revision: 1,
                created_at: 1,
                updated_at: 1,
            };

        let contract = make_document(BusinessDocumentKind::Contract, profile.clone());
        let contract_error = ensure_document_approvable(&contract).unwrap_err();
        assert!(contract_error.message.contains("serviceStartAt"));
        assert!(contract_error.message.contains("paymentTerms"));

        let payment_request = make_document(BusinessDocumentKind::PaymentRequest, profile.clone());
        let payment_error = ensure_document_approvable(&payment_request).unwrap_err();
        assert!(payment_error.message.contains("supplierBankAccount"));
        assert!(payment_error.message.contains("paymentTerms"));
        assert!(payment_error.message.contains("positiveAmount"));

        profile.delivery_summary.clear();
        profile.acceptance_terms.clear();
        let acceptance = make_document(BusinessDocumentKind::Acceptance, profile);
        let acceptance_error = ensure_document_approvable(&acceptance).unwrap_err();
        assert!(acceptance_error.message.contains("deliverySummary"));
        assert!(acceptance_error.message.contains("acceptanceTerms"));
    }

    #[test]
    fn all_docx_kinds_payment_lifecycle_and_snapshots_survive_restart() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let mut workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        workspace = store
            .execute(update_profile_command(
                &project_id,
                &workspace,
                complete_profile_input(&workspace.profile),
            ))
            .response
            .business_workspace;

        let (next, _, _, _) = create_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::Quote,
            None,
            BusinessDocumentFormat::Xlsx,
        );
        workspace = next;
        let (next, contract_generate, contract_id, contract_asset_id) =
            create_and_generate_test_document(
                &mut store,
                &project_id,
                workspace,
                BusinessDocumentKind::Contract,
                None,
                BusinessDocumentFormat::Docx,
            );
        workspace = make_test_document_effective(&mut store, &project_id, next, &contract_id);
        let contract_cents = workspace.financial_summary.contract_cents;
        assert!(contract_cents > 0);

        workspace = upsert_test_payment(
            &mut store,
            &project_id,
            workspace,
            None,
            contract_cents,
            BusinessPaymentStatus::Planned,
            None,
            "PLAN-2",
            "",
        );
        let payment_id = workspace.payments[0].id.clone();
        let (next, payment_request_id) = create_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::PaymentRequest,
            Some(payment_id.clone()),
        );
        workspace = next;
        let (next, acceptance_id) = create_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::Acceptance,
            None,
        );
        workspace = next;

        let payment_request = workspace
            .documents
            .iter()
            .find(|document| document.id == payment_request_id)
            .unwrap();
        assert_eq!(
            payment_request.snapshot.payment.as_ref().unwrap().id,
            payment_id
        );
        assert_eq!(
            payment_request
                .snapshot
                .payment
                .as_ref()
                .unwrap()
                .amount_cents,
            contract_cents
        );

        let frozen_snapshots = workspace
            .documents
            .iter()
            .map(|document| (document.id.clone(), document.snapshot.clone()))
            .collect::<HashMap<_, _>>();
        let mut changed_profile = complete_profile_input(&workspace.profile);
        changed_profile.project_title = "Changed after document snapshots".to_string();
        workspace = store
            .execute(update_profile_command(
                &project_id,
                &workspace,
                changed_profile,
            ))
            .response
            .business_workspace;
        for document in &workspace.documents {
            assert_eq!(
                &document.snapshot,
                frozen_snapshots.get(&document.id).unwrap()
            );
            assert_eq!(document.snapshot.profile.project_title, "Launch Project");
        }

        let (next, payment_request_generate, payment_request_asset_id) =
            approve_and_generate_test_document(
                &mut store,
                &project_id,
                workspace,
                &payment_request_id,
                BusinessDocumentFormat::Docx,
            );
        workspace = next;
        assert_eq!(
            workspace.payments[0].status,
            BusinessPaymentStatus::Requested
        );
        let frozen_payment_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::UpsertPayment {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: UpsertBusinessPaymentPayload {
                    workspace_id: workspace.id.clone(),
                    payment: BusinessPaymentInput {
                        id: Some(payment_id.clone()),
                        label: "Deposit".to_string(),
                        amount_cents: contract_cents - 1,
                        due_at: Some(2_000_000_000_000),
                        occurred_at: None,
                        status: BusinessPaymentStatus::Requested,
                        reference: "PLAN-2".to_string(),
                        notes: String::new(),
                    },
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();
        assert_eq!(frozen_payment_error.code, "BUSINESS_PAYMENT_STATUS_MANAGED");

        let (next, acceptance_generate, acceptance_asset_id) = approve_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            &acceptance_id,
            BusinessDocumentFormat::Docx,
        );
        workspace = make_test_document_effective(&mut store, &project_id, next, &acceptance_id);
        assert_eq!(workspace.lifecycle_stage, BusinessLifecycleStage::Accepted);

        let generation_commands = vec![
            (contract_generate, contract_id, contract_asset_id),
            (
                payment_request_generate,
                payment_request_id,
                payment_request_asset_id,
            ),
            (acceptance_generate, acceptance_id, acceptance_asset_id),
        ];
        for (_, _, asset_id) in &generation_commands {
            let asset = asset_service::get_asset(&store.connection, asset_id).unwrap();
            assert_eq!(asset.project_id.as_deref(), Some(project_id.as_str()));
            assert_eq!(asset.kind, AssetKind::Document);
            let path = asset_service::resolve_original_path(
                &store.connection,
                &store.vault_root,
                asset_id,
            )
            .unwrap();
            assert!(path.starts_with(fs::canonicalize(&store.vault_root).unwrap()));
            assert!(fs::read(&path).unwrap().starts_with(b"PK\x03\x04"));
            ZipArchive::new(File::open(path).unwrap())
                .unwrap()
                .by_name("word/document.xml")
                .unwrap();
        }

        let missing_occurred = BusinessWorkspaceCommandEnvelope::UpsertPayment {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(&project_id),
            payload: UpsertBusinessPaymentPayload {
                workspace_id: workspace.id.clone(),
                payment: BusinessPaymentInput {
                    id: Some(payment_id.clone()),
                    label: "Deposit".to_string(),
                    amount_cents: contract_cents,
                    due_at: Some(2_000_000_000_000),
                    occurred_at: None,
                    status: BusinessPaymentStatus::Received,
                    reference: "BANK-2".to_string(),
                    notes: String::new(),
                },
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        };
        assert_eq!(
            execute_command(&mut store.connection, &store.vault_root, missing_occurred)
                .unwrap_err()
                .code,
            "BUSINESS_PAYMENT_STATUS_MANAGED"
        );
        workspace = record_test_receipt(
            &mut store,
            &project_id,
            workspace,
            &payment_id,
            contract_cents,
            1_900_000_000_000,
            "BANK-2",
        );
        assert_eq!(workspace.payments[0].id, payment_id);
        assert_eq!(workspace.payments[0].revision, 3);
        assert_eq!(
            workspace.payments[0].status,
            BusinessPaymentStatus::Received
        );
        assert_eq!(workspace.lifecycle_stage, BusinessLifecycleStage::Paid);
        assert_eq!(workspace.financial_summary.outstanding_cents, 0);

        let rollback_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::UpsertPayment {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: UpsertBusinessPaymentPayload {
                    workspace_id: workspace.id.clone(),
                    payment: BusinessPaymentInput {
                        id: Some(payment_id.clone()),
                        label: "Deposit".to_string(),
                        amount_cents: contract_cents,
                        due_at: Some(2_000_000_000_000),
                        occurred_at: None,
                        status: BusinessPaymentStatus::Requested,
                        reference: "BANK-2".to_string(),
                        notes: String::new(),
                    },
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();
        assert_eq!(rollback_error.code, "BUSINESS_PAYMENT_STATUS_MANAGED");
        let rewrite_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::UpsertPayment {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: UpsertBusinessPaymentPayload {
                    workspace_id: workspace.id.clone(),
                    payment: BusinessPaymentInput {
                        id: Some(payment_id.clone()),
                        label: "Deposit".to_string(),
                        amount_cents: contract_cents,
                        due_at: Some(2_000_000_000_000),
                        occurred_at: Some(1_900_000_000_000),
                        status: BusinessPaymentStatus::Received,
                        reference: "CHANGED".to_string(),
                        notes: String::new(),
                    },
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();
        assert_eq!(rewrite_error.code, "BUSINESS_PAYMENT_STATUS_MANAGED");

        store.reopen();
        let persisted = list(&store.connection).unwrap().pop().unwrap();
        assert_eq!(persisted.documents.len(), 4);
        assert_eq!(
            persisted.payments[0].status,
            BusinessPaymentStatus::Received
        );
        assert_eq!(persisted.lifecycle_stage, BusinessLifecycleStage::Paid);
        assert_eq!(persisted.financial_summary.outstanding_cents, 0);
        for (command, document_id, asset_id) in generation_commands {
            let replay = store.execute(command);
            assert!(replay.response.replayed);
            let persisted_asset_id = replay
                .response
                .business_workspace
                .documents
                .iter()
                .find(|document| document.id == document_id)
                .unwrap()
                .output_asset_id
                .as_deref();
            assert_eq!(persisted_asset_id, Some(asset_id.as_str()));
        }
    }

    #[test]
    fn payment_request_and_terminal_states_freeze_audit_fields() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let mut workspace = prepare_effective_contract(&mut store, &project_id, workspace);
        let contract_cents = workspace.financial_summary.contract_cents;

        let managed_status_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::UpsertPayment {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: UpsertBusinessPaymentPayload {
                    workspace_id: workspace.id.clone(),
                    payment: BusinessPaymentInput {
                        id: None,
                        label: "Deposit".to_string(),
                        amount_cents: contract_cents,
                        due_at: Some(2_000_000_000_000),
                        occurred_at: Some(1_900_000_000_000),
                        status: BusinessPaymentStatus::Requested,
                        reference: "REQ-1".to_string(),
                        notes: String::new(),
                    },
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();
        assert_eq!(managed_status_error.code, "BUSINESS_PAYMENT_STATUS_MANAGED");

        workspace = upsert_test_payment(
            &mut store,
            &project_id,
            workspace,
            None,
            contract_cents,
            BusinessPaymentStatus::Planned,
            None,
            "REQ-1",
            "",
        );
        let payment_id = workspace.payments[0].id.clone();
        let (next, _, _, _) = create_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::PaymentRequest,
            Some(payment_id.clone()),
            BusinessDocumentFormat::Docx,
        );
        workspace = next;
        assert_eq!(
            workspace.payments[0].status,
            BusinessPaymentStatus::Requested
        );

        for (target_status, expected_code) in [
            (
                BusinessPaymentStatus::Planned,
                "BUSINESS_PAYMENT_STATUS_MANAGED",
            ),
            (
                // Canceling a requested payment is now allowed in principle,
                // but stays blocked while an active payment request document
                // captures the node.
                BusinessPaymentStatus::Canceled,
                "BUSINESS_PAYMENT_REQUEST_FIELDS_FROZEN",
            ),
        ] {
            let error = execute_command(
                &mut store.connection,
                &store.vault_root,
                BusinessWorkspaceCommandEnvelope::UpsertPayment {
                    command_id: Uuid::new_v4().to_string(),
                    protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                    context: context(&project_id),
                    payload: UpsertBusinessPaymentPayload {
                        workspace_id: workspace.id.clone(),
                        payment: BusinessPaymentInput {
                            id: Some(payment_id.clone()),
                            label: "Changed".to_string(),
                            amount_cents: contract_cents - 1,
                            due_at: Some(2_000_000_000_001),
                            occurred_at: None,
                            status: target_status,
                            reference: "CHANGED".to_string(),
                            notes: "must stay frozen".to_string(),
                        },
                    },
                    idempotency_key: Uuid::new_v4().to_string(),
                    expected_revision: Some(workspace.revision),
                    deadline_at: None,
                },
            )
            .unwrap_err();
            assert_eq!(error.code, expected_code);
        }
    }

    #[test]
    fn command_identity_collisions_expired_deadline_and_template_registry_are_strict() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let original = create_command(&project_id);
        store.execute(original.clone());

        let foreign_project = Uuid::new_v4().to_string();
        let mutate_project = |command: &mut BusinessWorkspaceCommandEnvelope| {
            if let BusinessWorkspaceCommandEnvelope::Create {
                context, payload, ..
            } = command
            {
                context.project_id = Some(foreign_project.clone());
                payload.project_id = foreign_project.clone();
            }
        };
        let mut idempotency_collision = original.clone();
        if let BusinessWorkspaceCommandEnvelope::Create { command_id, .. } =
            &mut idempotency_collision
        {
            *command_id = Uuid::new_v4().to_string();
        }
        mutate_project(&mut idempotency_collision);
        assert_eq!(
            execute_command(
                &mut store.connection,
                &store.vault_root,
                idempotency_collision,
            )
            .unwrap_err()
            .code,
            "IDEMPOTENCY_KEY_REUSED"
        );

        let mut command_id_collision = original.clone();
        if let BusinessWorkspaceCommandEnvelope::Create {
            idempotency_key, ..
        } = &mut command_id_collision
        {
            *idempotency_key = Uuid::new_v4().to_string();
        }
        mutate_project(&mut command_id_collision);
        assert_eq!(
            execute_command(
                &mut store.connection,
                &store.vault_root,
                command_id_collision,
            )
            .unwrap_err()
            .code,
            "COMMAND_ID_REUSED"
        );

        store
            .connection
            .execute(
                "INSERT INTO projects
                 (id, name, client_name, brief_json, stage, revision, created_at, updated_at)
                 VALUES (?1, 'Late', 'Customer', ?2, 'intake', 1, 1, 1)",
                params![
                    foreign_project,
                    serde_json::to_string(&BriefRecord::default()).unwrap()
                ],
            )
            .unwrap();
        let mut expired = create_command(&foreign_project);
        if let BusinessWorkspaceCommandEnvelope::Create { deadline_at, .. } = &mut expired {
            *deadline_at = Some(1);
        }
        assert_eq!(
            execute_command(&mut store.connection, &store.vault_root, expired)
                .unwrap_err()
                .code,
            "COMMAND_DEADLINE_EXCEEDED"
        );

        let workspace = list(&store.connection).unwrap().pop().unwrap();
        for (template_key, expected) in [
            ("builtin.unknown.v1", "BUSINESS_TEMPLATE_UNKNOWN"),
            (
                document_engine::CONTRACT_TEMPLATE_KEY,
                "BUSINESS_TEMPLATE_KIND_MISMATCH",
            ),
        ] {
            let error = execute_command(
                &mut store.connection,
                &store.vault_root,
                BusinessWorkspaceCommandEnvelope::CreateDocument {
                    command_id: Uuid::new_v4().to_string(),
                    protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                    context: context(&project_id),
                    payload: CreateBusinessDocumentPayload {
                        workspace_id: workspace.id.clone(),
                        kind: BusinessDocumentKind::Quote,
                        document_number: Uuid::new_v4().to_string(),
                        title: "Invalid template".to_string(),
                        template_key: template_key.to_string(),
                        payment_id: None,
                        acceptance_batch_id: None,
                    },
                    idempotency_key: Uuid::new_v4().to_string(),
                    expected_revision: Some(workspace.revision),
                    deadline_at: None,
                },
            )
            .unwrap_err();
            assert_eq!(error.code, expected);
        }
    }

    #[test]
    fn event_receipt_failure_rolls_back_the_aggregate() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let command = create_command(&project_id);
        store
            .connection
            .execute_batch(
                r#"
                CREATE TRIGGER reject_business_receipt
                BEFORE INSERT ON business_workspace_command_receipts
                BEGIN SELECT RAISE(ABORT, 'injected'); END;
                "#,
            )
            .unwrap();
        assert_eq!(
            execute_command(&mut store.connection, &store.vault_root, command.clone())
                .unwrap_err()
                .code,
            "HOST_INTERNAL"
        );
        for table in [
            "business_workspaces",
            "business_workspace_events",
            "business_workspace_command_receipts",
        ] {
            let count: i64 = store
                .connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "{table}");
        }
        store
            .connection
            .execute_batch("DROP TRIGGER reject_business_receipt;")
            .unwrap();
        assert_eq!(store.execute(command).emitted_events.len(), 1);
    }

    #[test]
    fn reconciliation_uses_asset_provenance_not_a_user_filename_prefix() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let source = store
            .temporary
            .path()
            .join("bsaigc-business-document-user-supplied.pdf");
        fs::write(&source, b"user supplied document").unwrap();
        let asset = asset_service::import_file(
            &mut store.connection,
            &store.vault_root,
            Some(&project_id),
            &source,
        )
        .unwrap();
        let path =
            asset_service::resolve_original_path(&store.connection, &store.vault_root, &asset.id)
                .unwrap();
        assert_eq!(
            reconcile_generated_assets(&mut store.connection, &store.vault_root).unwrap(),
            0
        );
        cleanup_generated_asset(&mut store.connection, &store.vault_root, &asset.id).unwrap();
        assert!(path.exists());
        assert_eq!(
            asset_service::get_asset(&store.connection, &asset.id)
                .unwrap()
                .id,
            asset.id
        );
        let origin: String = store
            .connection
            .query_row(
                "SELECT origin FROM asset_origins WHERE asset_id = ?1",
                [&asset.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(origin, "user");
    }

    #[test]
    fn template_review_is_terminal_replayable_and_preserves_linked_asset() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let source_path = store.temporary.path().join("customer-template.doc");
        let normalized_path = store.temporary.path().join("customer-template.docx");
        fs::write(
            &source_path,
            b"{\\rtf1\\ansi immutable legacy template fixture}",
        )
        .unwrap();
        fs::write(&normalized_path, empty_ooxml_package()).unwrap();
        let source_asset = asset_service::import_file(
            &mut store.connection,
            &store.vault_root,
            Some(&project_id),
            &source_path,
        )
        .unwrap();
        let normalized_asset = asset_service::import_generated_artifact(
            &mut store.connection,
            &store.vault_root,
            &project_id,
            &normalized_path,
            asset_service::GeneratedArtifactSource::NormalizedTemplate,
            "template-review-fixture",
        )
        .unwrap();
        let normalized_vault_path = asset_service::resolve_original_path(
            &store.connection,
            &store.vault_root,
            &normalized_asset.id,
        )
        .unwrap();
        let template_version_id = Uuid::new_v4().to_string();
        let template_key =
            document_engine::BAIETAN_PAYMENT_APPLICATION_SETTLEMENT_CALCULATION_TEMPLATE_KEY;
        let mapping_version = document_engine::expected_template_mapping_version(template_key)
            .expect("registered payment template mapping");
        store
            .connection
            .execute(
                "INSERT INTO business_template_versions
                 (id, workspace_id, source_asset_id, source_sha256,
                  normalized_asset_id, normalized_sha256, template_key, mapping_version,
                  converter_engine, converter_version, converter_policy_version,
                  status, reviewed_by, reviewed_at, review_note, revision, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                         'microsoft-word-com', '16.0', 'word-only.v1',
                         'pendingReview', NULL, NULL, '', 1, 10, 10)",
                params![
                    template_version_id,
                    workspace.id,
                    source_asset.id,
                    source_asset.sha256,
                    normalized_asset.id,
                    normalized_asset.sha256,
                    template_key,
                    mapping_version,
                ],
            )
            .unwrap();

        let command = BusinessWorkspaceCommandEnvelope::ApproveTemplateVersion {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(&project_id),
            payload: ApproveBusinessTemplateVersionPayload {
                workspace_id: workspace.id.clone(),
                template_version_id: template_version_id.clone(),
                note: "Verified immutable conversion and mapping".to_string(),
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        };
        let invalid =
            execute_command(&mut store.connection, &store.vault_root, command.clone()).unwrap_err();
        assert_eq!(invalid.code, "BUSINESS_PAYMENT_TEMPLATE_INVALID");
        let fallback_template_key =
            document_engine::BAIETAN_VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY;
        let fallback_mapping_version =
            document_engine::expected_template_mapping_version(fallback_template_key).unwrap();
        store
            .connection
            .execute(
                "UPDATE business_template_versions
                 SET template_key = ?1, mapping_version = ?2
                 WHERE id = ?3",
                params![
                    fallback_template_key,
                    fallback_mapping_version,
                    template_version_id,
                ],
            )
            .unwrap();

        let approved = store.execute(command.clone());
        assert_eq!(approved.emitted_events.len(), 1);
        assert_eq!(
            approved.emitted_events[0].event_type,
            BusinessWorkspaceEventType::TemplateVersionApproved
        );
        let version = approved
            .response
            .business_workspace
            .template_versions
            .iter()
            .find(|version| version.id == template_version_id)
            .unwrap();
        assert_eq!(version.status, BusinessTemplateVersionStatus::Approved);
        assert_eq!(version.revision, 2);
        assert_eq!(version.reviewed_by.as_deref(), Some("operator-local"));

        let replayed = store.execute(command);
        assert!(replayed.response.replayed);
        assert!(replayed.emitted_events.is_empty());
        assert_eq!(
            reconcile_generated_assets(&mut store.connection, &store.vault_root).unwrap(),
            0
        );
        assert!(normalized_vault_path.exists());

        let error = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::RejectTemplateVersion {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: RejectBusinessTemplateVersionPayload {
                    workspace_id: workspace.id,
                    template_version_id,
                    note: "Cannot reverse a terminal decision".to_string(),
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(approved.response.business_workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_TEMPLATE_VERSION_TERMINAL");
    }

    #[test]
    fn reconciliation_removes_unlinked_normalized_template_asset() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let source = store.temporary.path().join("orphan-template.docx");
        fs::write(&source, empty_ooxml_package()).unwrap();
        let asset = asset_service::import_generated_artifact(
            &mut store.connection,
            &store.vault_root,
            &project_id,
            &source,
            asset_service::GeneratedArtifactSource::NormalizedTemplate,
            "orphan-normalized-template",
        )
        .unwrap();
        let path =
            asset_service::resolve_original_path(&store.connection, &store.vault_root, &asset.id)
                .unwrap();

        assert_eq!(
            reconcile_generated_assets(&mut store.connection, &store.vault_root).unwrap(),
            1
        );
        assert!(!path.exists());
        assert_eq!(
            asset_service::get_asset(&store.connection, &asset.id)
                .unwrap_err()
                .code,
            "ASSET_NOT_FOUND"
        );
    }

    #[test]
    fn document_prerequisites_effective_states_and_tax_totals_are_enforced() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let mut workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        workspace = store
            .execute(update_profile_command(
                &project_id,
                &workspace,
                complete_profile_input(&workspace.profile),
            ))
            .response
            .business_workspace;
        assert_eq!(workspace.profile.line_items[0].amount_cents, 10_600);

        let contract_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            create_test_document_command(
                &project_id,
                &workspace,
                BusinessDocumentKind::Contract,
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(contract_error.code, "BUSINESS_QUOTE_REQUIRED");
        workspace = upsert_test_payment(
            &mut store,
            &project_id,
            workspace,
            None,
            1_000,
            BusinessPaymentStatus::Planned,
            None,
            "PRE-CONTRACT",
            "",
        );
        let precontract_payment_id = workspace.payments[0].id.clone();
        let payment_request_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            create_test_document_command(
                &project_id,
                &workspace,
                BusinessDocumentKind::PaymentRequest,
                Some(precontract_payment_id),
            ),
        )
        .unwrap_err();
        assert_eq!(
            payment_request_error.code,
            "BUSINESS_EFFECTIVE_CONTRACT_REQUIRED"
        );

        let (next, _, quote_id, _) = create_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::Quote,
            None,
            BusinessDocumentFormat::Xlsx,
        );
        workspace = next;
        let quote_effective_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            status_command(
                &project_id,
                &workspace,
                &quote_id,
                BusinessDocumentStatus::Effective,
            ),
        )
        .unwrap_err();
        assert_eq!(quote_effective_error.code, "BUSINESS_EVIDENCE_KIND_INVALID");

        let (next, contract_id) = create_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::Contract,
            None,
        );
        workspace = next;
        let acceptance_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            create_test_document_command(
                &project_id,
                &workspace,
                BusinessDocumentKind::Acceptance,
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(
            acceptance_error.code,
            "BUSINESS_EFFECTIVE_CONTRACT_REQUIRED"
        );
        let (next, _, _) = approve_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            &contract_id,
            BusinessDocumentFormat::Docx,
        );
        workspace = make_test_document_effective(&mut store, &project_id, next, &contract_id);
        assert_eq!(workspace.financial_summary.quoted_cents, 10_600);
        assert_eq!(workspace.financial_summary.contract_cents, 10_600);
        assert_eq!(
            workspace.lifecycle_stage,
            BusinessLifecycleStage::Contracted
        );

        let mut zero_store = TestStore::new();
        let zero_project_id = zero_store.project_id.clone();
        let mut zero_workspace = zero_store
            .execute(create_command(&zero_project_id))
            .response
            .business_workspace;
        let mut zero_profile = complete_profile_input(&zero_workspace.profile);
        for item in &mut zero_profile.line_items {
            item.unit_price_cents = 0;
        }
        zero_workspace = zero_store
            .execute(update_profile_command(
                &zero_project_id,
                &zero_workspace,
                zero_profile,
            ))
            .response
            .business_workspace;
        let (next, zero_quote_id) = create_test_document(
            &mut zero_store,
            &zero_project_id,
            zero_workspace,
            BusinessDocumentKind::Quote,
            None,
        );
        zero_workspace = zero_store
            .execute(status_command(
                &zero_project_id,
                &next,
                &zero_quote_id,
                BusinessDocumentStatus::InReview,
            ))
            .response
            .business_workspace;
        let zero_total_error = execute_command(
            &mut zero_store.connection,
            &zero_store.vault_root,
            status_command(
                &zero_project_id,
                &zero_workspace,
                &zero_quote_id,
                BusinessDocumentStatus::Approved,
            ),
        )
        .unwrap_err();
        assert_eq!(zero_total_error.code, "BUSINESS_DOCUMENT_AMOUNT_REQUIRED");
    }

    #[test]
    fn payment_capacity_request_receipt_and_reference_guards_are_enforced() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let mut workspace = prepare_effective_contract(&mut store, &project_id, workspace);
        let contract_cents = workspace.financial_summary.contract_cents;

        let capacity_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::UpsertPayment {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: UpsertBusinessPaymentPayload {
                    workspace_id: workspace.id.clone(),
                    payment: BusinessPaymentInput {
                        id: None,
                        label: "Too much".to_string(),
                        amount_cents: contract_cents + 1,
                        due_at: None,
                        occurred_at: None,
                        status: BusinessPaymentStatus::Planned,
                        reference: String::new(),
                        notes: String::new(),
                    },
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();
        assert_eq!(capacity_error.code, "BUSINESS_PAYMENT_EXCEEDS_CONTRACT");

        let first_amount = contract_cents / 2;
        let second_amount = contract_cents - first_amount;
        workspace = upsert_test_payment(
            &mut store,
            &project_id,
            workspace,
            None,
            first_amount,
            BusinessPaymentStatus::Planned,
            None,
            "REQ-A",
            "",
        );
        let first_payment_id = workspace.payments[0].id.clone();
        let premature_receipt = execute_command(
            &mut store.connection,
            &store.vault_root,
            record_test_receipt_command(
                &project_id,
                &workspace,
                &first_payment_id,
                1,
                1_900_000_000_000,
                "PREMATURE",
            ),
        )
        .unwrap_err();
        assert_eq!(premature_receipt.code, "BUSINESS_PAYMENT_NOT_RECEIVABLE");

        let (next, _, _, _) = create_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::PaymentRequest,
            Some(first_payment_id.clone()),
            BusinessDocumentFormat::Docx,
        );
        workspace = next;
        let blank_reference = execute_command(
            &mut store.connection,
            &store.vault_root,
            record_test_receipt_command(
                &project_id,
                &workspace,
                &first_payment_id,
                1,
                1_900_000_000_000,
                "   ",
            ),
        )
        .unwrap_err();
        assert_eq!(blank_reference.code, "VALIDATION_FAILED");

        let partial_amount = first_amount / 2;
        workspace = record_test_receipt(
            &mut store,
            &project_id,
            workspace,
            &first_payment_id,
            partial_amount,
            1_900_000_000_000,
            "Bank-Ref",
        );
        assert_eq!(
            workspace.payments[0].status,
            BusinessPaymentStatus::PartiallyReceived
        );
        let over_receipt = execute_command(
            &mut store.connection,
            &store.vault_root,
            record_test_receipt_command(
                &project_id,
                &workspace,
                &first_payment_id,
                first_amount,
                1_900_000_000_001,
                "BANK-OVER",
            ),
        )
        .unwrap_err();
        assert_eq!(over_receipt.code, "BUSINESS_RECEIPT_EXCEEDS_PAYMENT");
        workspace = record_test_receipt(
            &mut store,
            &project_id,
            workspace,
            &first_payment_id,
            first_amount - partial_amount,
            1_900_000_000_002,
            "BANK-REST",
        );
        assert_eq!(
            workspace.payments[0].status,
            BusinessPaymentStatus::Received
        );

        workspace = upsert_test_payment(
            &mut store,
            &project_id,
            workspace,
            None,
            second_amount,
            BusinessPaymentStatus::Planned,
            None,
            "REQ-B",
            "",
        );
        let second_payment_id = workspace
            .payments
            .iter()
            .find(|payment| payment.status == BusinessPaymentStatus::Planned)
            .unwrap()
            .id
            .clone();
        let (next, _, _, _) = create_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::PaymentRequest,
            Some(second_payment_id.clone()),
            BusinessDocumentFormat::Docx,
        );
        workspace = next;
        let duplicate_reference = execute_command(
            &mut store.connection,
            &store.vault_root,
            record_test_receipt_command(
                &project_id,
                &workspace,
                &second_payment_id,
                second_amount,
                1_900_000_000_003,
                "bank-ref",
            ),
        )
        .unwrap_err();
        assert_eq!(
            duplicate_reference.code,
            "BUSINESS_RECEIPT_REFERENCE_DUPLICATE"
        );
    }

    #[test]
    fn receipt_reversal_ledger_restores_requested_status_and_survives_restart() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let mut workspace = prepare_effective_contract(&mut store, &project_id, workspace);
        let contract_cents = workspace.financial_summary.contract_cents;
        workspace = upsert_test_payment(
            &mut store,
            &project_id,
            workspace,
            None,
            contract_cents,
            BusinessPaymentStatus::Planned,
            None,
            "REVERSAL-PLAN",
            "",
        );
        let payment_id = workspace.payments[0].id.clone();
        let (next, _, _, _) = create_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::PaymentRequest,
            Some(payment_id.clone()),
            BusinessDocumentFormat::Docx,
        );
        workspace = record_test_receipt(
            &mut store,
            &project_id,
            next,
            &payment_id,
            contract_cents,
            1_900_000_000_000,
            "RECEIPT-FULL",
        );
        let original_receipt_id = workspace
            .receipts
            .iter()
            .find(|receipt| receipt.kind == BusinessReceiptKind::Receipt)
            .unwrap()
            .id
            .clone();
        assert_eq!(
            workspace.payments[0].status,
            BusinessPaymentStatus::Received
        );
        assert_eq!(workspace.financial_summary.received_cents, contract_cents);
        assert_eq!(workspace.financial_summary.outstanding_cents, 0);

        let partial_reversal = contract_cents / 3;
        workspace = store
            .execute(reverse_test_receipt_command(
                &project_id,
                &workspace,
                &original_receipt_id,
                partial_reversal,
                1_900_000_000_100,
                "REVERSAL-PARTIAL",
            ))
            .response
            .business_workspace;
        let reversal_id = workspace
            .receipts
            .iter()
            .find(|receipt| receipt.kind == BusinessReceiptKind::Reversal)
            .unwrap()
            .id
            .clone();
        assert_eq!(
            workspace.payments[0].status,
            BusinessPaymentStatus::PartiallyReceived
        );
        assert_eq!(
            workspace.financial_summary.received_cents,
            contract_cents - partial_reversal
        );
        assert_eq!(
            workspace.financial_summary.outstanding_cents,
            partial_reversal
        );

        let duplicate_reference = execute_command(
            &mut store.connection,
            &store.vault_root,
            reverse_test_receipt_command(
                &project_id,
                &workspace,
                &original_receipt_id,
                1,
                1_900_000_000_101,
                "reversal-partial",
            ),
        )
        .unwrap_err();
        assert_eq!(
            duplicate_reference.code,
            "BUSINESS_RECEIPT_REFERENCE_DUPLICATE"
        );

        let excessive_reversal = execute_command(
            &mut store.connection,
            &store.vault_root,
            reverse_test_receipt_command(
                &project_id,
                &workspace,
                &original_receipt_id,
                contract_cents - partial_reversal + 1,
                1_900_000_000_102,
                "REVERSAL-TOO-MUCH",
            ),
        )
        .unwrap_err();
        assert_eq!(
            excessive_reversal.code,
            "BUSINESS_RECEIPT_REVERSAL_EXCEEDS_ORIGINAL"
        );

        let reversal_of_reversal = execute_command(
            &mut store.connection,
            &store.vault_root,
            reverse_test_receipt_command(
                &project_id,
                &workspace,
                &reversal_id,
                1,
                1_900_000_000_103,
                "REVERSAL-INVALID",
            ),
        )
        .unwrap_err();
        assert_eq!(
            reversal_of_reversal.code,
            "BUSINESS_RECEIPT_REVERSAL_INVALID"
        );

        let earlier_reversal = execute_command(
            &mut store.connection,
            &store.vault_root,
            reverse_test_receipt_command(
                &project_id,
                &workspace,
                &original_receipt_id,
                1,
                1_899_999_999_999,
                "REVERSAL-EARLY",
            ),
        )
        .unwrap_err();
        assert_eq!(earlier_reversal.code, "VALIDATION_FAILED");

        workspace = store
            .execute(reverse_test_receipt_command(
                &project_id,
                &workspace,
                &original_receipt_id,
                contract_cents - partial_reversal,
                1_900_000_000_200,
                "REVERSAL-REST",
            ))
            .response
            .business_workspace;
        assert_eq!(
            workspace.payments[0].status,
            BusinessPaymentStatus::Requested
        );
        assert_eq!(workspace.financial_summary.received_cents, 0);
        assert_eq!(
            workspace.financial_summary.outstanding_cents,
            contract_cents
        );
        assert_eq!(workspace.receipts.len(), 3);

        let workspace_id = workspace.id.clone();
        store.reopen();
        let persisted = list(&store.connection)
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == workspace_id)
            .unwrap();
        assert_eq!(
            persisted.payments[0].status,
            BusinessPaymentStatus::Requested
        );
        assert_eq!(persisted.financial_summary.received_cents, 0);
        assert_eq!(
            persisted.financial_summary.outstanding_cents,
            contract_cents
        );
        assert_eq!(persisted.receipts.len(), 3);
        assert_eq!(
            persisted
                .receipts
                .iter()
                .filter(|receipt| receipt.kind == BusinessReceiptKind::Reversal)
                .count(),
            2
        );
    }

    #[test]
    fn evidence_materialization_enforces_project_integrity_and_exclusivity() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let source_path = store.temporary.path().join("signed-contract.txt");
        fs::write(&source_path, b"signed contract evidence").unwrap();
        let asset = asset_service::import_file(
            &mut store.connection,
            &store.vault_root,
            Some(&project_id),
            &source_path,
        )
        .unwrap();
        let other_project_id =
            insert_project(&store.connection, "Other Project", "Other Customer", None);
        let other_source_path = store.temporary.path().join("other-evidence.txt");
        fs::write(&other_source_path, b"other project evidence").unwrap();
        let other_asset = asset_service::import_file(
            &mut store.connection,
            &store.vault_root,
            Some(&other_project_id),
            &other_source_path,
        )
        .unwrap();

        let mismatch = materialize_evidence(
            &store.connection,
            &store.vault_root,
            &project_id,
            BusinessEvidenceKind::ContractSignature,
            &BusinessEvidenceInput {
                asset_id: other_asset.id,
                occurred_at: Some(1_900_000_000_000),
                note: "wrong project".to_string(),
            },
            "operator-local",
            1_900_000_000_100,
        )
        .unwrap_err();
        assert_eq!(mismatch.code, "BUSINESS_EVIDENCE_PROJECT_MISMATCH");

        let input = BusinessEvidenceInput {
            asset_id: asset.id.clone(),
            occurred_at: Some(1_900_000_000_000),
            note: "customer signed copy".to_string(),
        };
        let ambiguous = materialize_evidence_or_waiver(
            &store.connection,
            &store.vault_root,
            &project_id,
            BusinessEvidenceKind::ContractSignature,
            Some(&input),
            Some(&BusinessManualWaiverInput {
                reason: "should not coexist".to_string(),
            }),
            "operator-local",
            1_900_000_000_100,
        )
        .unwrap_err();
        assert_eq!(ambiguous.code, "BUSINESS_EVIDENCE_AMBIGUOUS");

        let missing = materialize_evidence_or_waiver(
            &store.connection,
            &store.vault_root,
            &project_id,
            BusinessEvidenceKind::ContractSignature,
            None,
            None,
            "operator-local",
            1_900_000_000_100,
        )
        .unwrap_err();
        assert_eq!(missing.code, "BUSINESS_EVIDENCE_REQUIRED");

        let evidence = materialize_evidence(
            &store.connection,
            &store.vault_root,
            &project_id,
            BusinessEvidenceKind::ContractSignature,
            &input,
            "operator-local",
            1_900_000_000_100,
        )
        .unwrap();
        assert_eq!(evidence.asset_id, asset.id);
        assert_eq!(evidence.sha256, asset.sha256);
        assert_eq!(evidence.kind, BusinessEvidenceKind::ContractSignature);
        assert_eq!(evidence.recorded_by, "operator-local");
        assert_eq!(evidence.occurred_at, Some(1_900_000_000_000));

        let vault_path =
            asset_service::resolve_original_path(&store.connection, &store.vault_root, &asset.id)
                .unwrap();
        fs::write(vault_path, b"tampered evidence").unwrap();
        let tampered = materialize_evidence(
            &store.connection,
            &store.vault_root,
            &project_id,
            BusinessEvidenceKind::ContractSignature,
            &input,
            "operator-local",
            1_900_000_000_200,
        )
        .unwrap_err();
        assert_eq!(tampered.code, "VAULT_ASSET_INTEGRITY_MISMATCH");
    }

    #[test]
    fn archive_requires_acceptance_full_payment_and_no_pending_documents() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let mut workspace = prepare_effective_contract(&mut store, &project_id, workspace);
        let contract_cents = workspace.financial_summary.contract_cents;

        let archive_without_acceptance = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::ChangeStatus {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: ChangeBusinessWorkspaceStatusPayload {
                    workspace_id: workspace.id.clone(),
                    status: BusinessWorkspaceStatus::Archived,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();
        assert_eq!(
            archive_without_acceptance.code,
            "BUSINESS_WORKSPACE_ARCHIVE_BLOCKED"
        );

        workspace = upsert_test_payment(
            &mut store,
            &project_id,
            workspace,
            None,
            contract_cents,
            BusinessPaymentStatus::Planned,
            None,
            "ARCHIVE-PLAN",
            "",
        );
        let payment_id = workspace.payments[0].id.clone();
        let (next, _, _, _) = create_and_generate_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::PaymentRequest,
            Some(payment_id.clone()),
            BusinessDocumentFormat::Docx,
        );
        let (next, _, acceptance_id, _) = create_and_generate_test_document(
            &mut store,
            &project_id,
            next,
            BusinessDocumentKind::Acceptance,
            None,
            BusinessDocumentFormat::Docx,
        );
        workspace = make_test_document_effective(&mut store, &project_id, next, &acceptance_id);
        let unpaid_archive_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::ChangeStatus {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: ChangeBusinessWorkspaceStatusPayload {
                    workspace_id: workspace.id.clone(),
                    status: BusinessWorkspaceStatus::Archived,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();
        assert_eq!(
            unpaid_archive_error.code,
            "BUSINESS_WORKSPACE_ARCHIVE_BLOCKED"
        );

        workspace = record_test_receipt(
            &mut store,
            &project_id,
            workspace,
            &payment_id,
            contract_cents,
            1_900_000_000_000,
            "ARCHIVE-BANK",
        );
        assert_eq!(workspace.lifecycle_stage, BusinessLifecycleStage::Paid);
        let (next, draft_quote_id) = create_test_document(
            &mut store,
            &project_id,
            workspace,
            BusinessDocumentKind::Quote,
            None,
        );
        workspace = next;
        let pending_document_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::ChangeStatus {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: ChangeBusinessWorkspaceStatusPayload {
                    workspace_id: workspace.id.clone(),
                    status: BusinessWorkspaceStatus::Archived,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();
        assert_eq!(
            pending_document_error.code,
            "BUSINESS_WORKSPACE_ARCHIVE_BLOCKED"
        );
        workspace = store
            .execute(status_command(
                &project_id,
                &workspace,
                &draft_quote_id,
                BusinessDocumentStatus::Voided,
            ))
            .response
            .business_workspace;
        workspace = prepare_test_business_closure(&mut store, &project_id, workspace);
        workspace = change_test_workspace_status(
            &mut store,
            &project_id,
            workspace,
            BusinessWorkspaceStatus::Archived,
        );
        assert_eq!(workspace.status, BusinessWorkspaceStatus::Archived);
        assert_eq!(workspace.lifecycle_stage, BusinessLifecycleStage::Archived);
        assert!(workspace.archived_at.is_some());
        assert_eq!(workspace.archived_by.as_deref(), Some("operator-local"));

        workspace = change_test_workspace_status(
            &mut store,
            &project_id,
            workspace,
            BusinessWorkspaceStatus::Active,
        );
        assert_eq!(workspace.status, BusinessWorkspaceStatus::Active);
        assert_eq!(workspace.lifecycle_stage, BusinessLifecycleStage::Paid);
        // Unarchiving keeps the archive audit trail intact.
        assert!(workspace.archived_at.is_some());
        assert_eq!(workspace.archived_by.as_deref(), Some("operator-local"));

        let workspace_id = workspace.id.clone();
        store.reopen();
        let persisted = list(&store.connection)
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == workspace_id)
            .unwrap();
        assert_eq!(persisted.status, BusinessWorkspaceStatus::Active);
        assert_eq!(persisted.lifecycle_stage, BusinessLifecycleStage::Paid);
        // The archive audit trail survives reopen after unarchiving.
        assert!(persisted.archived_at.is_some());
        assert_eq!(persisted.archived_by.as_deref(), Some("operator-local"));
    }

    #[test]
    fn legacy_workspace_and_event_json_default_new_derived_and_audit_fields() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let event = replay_events(&store.connection, 0, 1)
            .unwrap()
            .pop()
            .unwrap();

        let mut workspace_json = serde_json::to_value(&workspace).unwrap();
        let workspace_object = workspace_json.as_object_mut().unwrap();
        workspace_object.remove("lifecycleStage");
        workspace_object.remove("financialSummary");
        workspace_object.remove("currentDocuments");
        let legacy_workspace: BusinessWorkspaceRecord =
            serde_json::from_value(workspace_json).unwrap();
        assert_eq!(
            legacy_workspace.lifecycle_stage,
            BusinessLifecycleStage::Draft
        );
        assert_eq!(legacy_workspace.financial_summary.contract_cents, 0);
        assert!(legacy_workspace
            .current_documents
            .contract_document_id
            .is_none());

        let mut event_json = serde_json::to_value(&event).unwrap();
        let event_object = event_json.as_object_mut().unwrap();
        event_object.remove("actorId");
        event_object.remove("commandId");
        event_object.remove("reason");
        let legacy_event: BusinessWorkspaceDomainEvent =
            serde_json::from_value(event_json).unwrap();
        assert!(legacy_event.actor_id.is_empty());
        assert!(legacy_event.command_id.is_empty());
        assert!(legacy_event.reason.is_empty());
    }
    #[test]
    fn restart_reconciliation_removes_unlinked_generated_asset() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let mut workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let mut profile = profile_input(&workspace.profile);
        profile.supplier_legal_name = "Supplier".to_string();
        workspace = store
            .execute(update_profile_command(&project_id, &workspace, profile))
            .response
            .business_workspace;
        workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::CreateDocument {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: CreateBusinessDocumentPayload {
                    workspace_id: workspace.id.clone(),
                    kind: BusinessDocumentKind::Quote,
                    document_number: "Q-RECONCILE-1".to_string(),
                    title: "Quote".to_string(),
                    template_key: document_engine::QUOTE_TEMPLATE_KEY.to_string(),
                    payment_id: None,
                    acceptance_batch_id: None,
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        let staged = document_engine::generate_document(
            &store.vault_root,
            &workspace.documents[0],
            &BusinessDocumentFormat::Xlsx,
        )
        .unwrap();
        let asset = asset_service::import_business_document(
            &mut store.connection,
            &store.vault_root,
            &project_id,
            staged.path(),
            &Uuid::new_v4().to_string(),
        )
        .unwrap();
        let path =
            asset_service::resolve_original_path(&store.connection, &store.vault_root, &asset.id)
                .unwrap();
        let staged_path = staged.path().to_path_buf();
        let mut concurrent = Connection::open(&store.database_path).unwrap();
        concurrent
            .execute_batch("PRAGMA foreign_keys = ON;")
            .unwrap();
        assert_eq!(
            reconcile_generated_assets(&mut concurrent, &store.vault_root).unwrap(),
            0
        );
        assert!(path.exists());
        assert!(staged_path.exists());
        assert!(asset_service::get_asset(&concurrent, &asset.id).is_ok());
        drop(concurrent);
        drop(staged);
        store.reopen();
        assert_eq!(
            reconcile_generated_assets(&mut store.connection, &store.vault_root).unwrap(),
            1
        );
        assert!(!path.exists());
        assert!(
            asset_service::list_assets(&store.connection, Some(&project_id))
                .unwrap()
                .is_empty()
        );
        assert!(store.temporary.path().exists());
    }
    #[test]
    fn archive_snapshot_assets_are_durable_idempotent_and_verifiable() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = prepare_paid_and_accepted_workspace(&mut store, &project_id);
        let workspace = prepare_test_business_closure_ready(&mut store, &project_id, workspace);
        let captured_revision = workspace.revision;
        let command = BusinessWorkspaceCommandEnvelope::CreateArchiveSnapshot {
            command_id: Uuid::new_v4().to_string(),
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(&project_id),
            payload: CreateBusinessArchiveSnapshotPayload {
                workspace_id: workspace.id.clone(),
            },
            idempotency_key: Uuid::new_v4().to_string(),
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        };

        let outcome = store.execute(command.clone());
        assert!(!outcome.response.replayed);
        assert_eq!(outcome.emitted_asset_events.len(), 2);
        let workspace = outcome.response.business_workspace;
        assert_eq!(workspace.revision, captured_revision + 1);
        assert_eq!(
            workspace.archive_integrity_status,
            BusinessArchiveIntegrityStatus::Ready
        );
        assert_eq!(workspace.archive_snapshots.len(), 1);
        let snapshot = workspace.archive_snapshots[0].clone();
        assert!(!snapshot.entries.is_empty());
        assert_eq!(snapshot.captured_workspace_revision, captured_revision);
        assert_eq!(
            snapshot.captured_customer_revision,
            workspace.customer.revision
        );
        let manifest_asset_id = snapshot.manifest_asset_id.clone().unwrap();
        let package_asset_id = snapshot.package_asset_id.clone().unwrap();
        assert_ne!(manifest_asset_id, package_asset_id);

        let manifest_asset =
            asset_service::get_asset(&store.connection, &manifest_asset_id).unwrap();
        let package_asset = asset_service::get_asset(&store.connection, &package_asset_id).unwrap();
        let manifest_path = asset_service::resolve_original_path(
            &store.connection,
            &store.vault_root,
            &manifest_asset_id,
        )
        .unwrap();
        let package_path = asset_service::resolve_original_path(
            &store.connection,
            &store.vault_root,
            &package_asset_id,
        )
        .unwrap();
        let manifest_bytes = fs::read(&manifest_path).unwrap();
        let observed_manifest_sha = format!("{:x}", Sha256::digest(&manifest_bytes));
        assert_eq!(observed_manifest_sha, snapshot.manifest_sha256);
        assert_eq!(manifest_asset.sha256, snapshot.manifest_sha256);
        assert!(package_asset.size_bytes > 0);

        let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
        assert_eq!(manifest["manifestVersion"], "business-archive.v1");
        assert_eq!(manifest["snapshotId"], snapshot.id);
        assert_eq!(manifest["workspaceId"], workspace.id);
        assert_eq!(manifest["projectId"], project_id);
        assert_eq!(
            manifest["capturedWorkspaceRevision"].as_i64(),
            Some(snapshot.captured_workspace_revision)
        );
        assert_eq!(
            manifest["capturedCustomerRevision"].as_i64(),
            Some(snapshot.captured_customer_revision)
        );
        assert_eq!(
            manifest["entries"].as_array().unwrap().len(),
            snapshot.entries.len()
        );

        let mut package = ZipArchive::new(File::open(&package_path).unwrap()).unwrap();
        assert_eq!(package.len(), snapshot.entries.len() + 1);
        let packaged_manifest = {
            let mut file = package.by_name("manifest.json").unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            bytes
        };
        assert_eq!(packaged_manifest, manifest_bytes);
        for entry in &snapshot.entries {
            let packaged_bytes = {
                let mut file = package.by_name(&entry.logical_path).unwrap();
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).unwrap();
                bytes
            };
            assert_eq!(packaged_bytes.len() as i64, entry.artifact.size_bytes);
            assert_eq!(
                format!("{:x}", Sha256::digest(&packaged_bytes)),
                entry.artifact.sha256
            );
            let source_path = asset_service::resolve_original_path(
                &store.connection,
                &store.vault_root,
                &entry.artifact.asset_id,
            )
            .unwrap();
            assert_eq!(packaged_bytes, fs::read(source_path).unwrap());
        }
        drop(package);

        let asset_count = asset_service::list_assets(&store.connection, Some(&project_id))
            .unwrap()
            .len();
        let snapshot_count: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM business_archive_snapshots WHERE workspace_id = ?1",
                [&workspace.id],
                |row| row.get(0),
            )
            .unwrap();
        let replay = store.execute(command);
        assert!(replay.response.replayed);
        assert!(replay.emitted_asset_events.is_empty());
        assert_eq!(
            replay.response.business_workspace.archive_snapshots.len(),
            1
        );
        assert_eq!(
            asset_service::list_assets(&store.connection, Some(&project_id))
                .unwrap()
                .len(),
            asset_count
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM business_archive_snapshots WHERE workspace_id = ?1",
                    [&workspace.id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            snapshot_count
        );
        assert_eq!(
            reconcile_generated_assets(&mut store.connection, &store.vault_root).unwrap(),
            0
        );
        assert!(manifest_path.exists());
        assert!(package_path.exists());

        let workspace_id = workspace.id.clone();
        store.reopen();
        let persisted = list(&store.connection)
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == workspace_id)
            .unwrap();
        assert_eq!(persisted.archive_snapshots, vec![snapshot.clone()]);
        assert_eq!(
            persisted.archive_integrity_status,
            BusinessArchiveIntegrityStatus::Ready
        );
        let mut customer_changed = persisted.clone();
        customer_changed.customer.revision += 1;
        assert_eq!(
            business_closure_service::archive_integrity_status(&customer_changed),
            BusinessArchiveIntegrityStatus::Stale
        );

        let updated = store
            .execute(BusinessWorkspaceCommandEnvelope::UpsertMilestone {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: UpsertBusinessMilestonePayload {
                    workspace_id: persisted.id.clone(),
                    milestone: crate::protocol::BusinessMilestoneInput {
                        id: None,
                        title: "Optional follow-up".to_string(),
                        description: "Post-archive follow-up item".to_string(),
                        due_at: None,
                        acceptance_criteria: "Optional".to_string(),
                        required: false,
                        status: crate::protocol::BusinessMilestoneStatus::Planned,
                    },
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(persisted.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        assert_eq!(
            updated.archive_integrity_status,
            BusinessArchiveIntegrityStatus::Stale
        );
    }

    #[test]
    fn archive_snapshot_rejects_tampered_vault_assets_without_side_effects() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = prepare_paid_and_accepted_workspace(&mut store, &project_id);
        let workspace = prepare_test_business_closure_ready(&mut store, &project_id, workspace);
        let tampered_asset_id = workspace.milestones[0].deliverables[0].versions[0]
            .artifact
            .asset_id
            .clone();
        let tampered_path = asset_service::resolve_original_path(
            &store.connection,
            &store.vault_root,
            &tampered_asset_id,
        )
        .unwrap();
        fs::write(tampered_path, b"tampered after registration").unwrap();
        let before_counts = table_counts(&store.connection);
        let before_assets = asset_service::list_assets(&store.connection, Some(&project_id))
            .unwrap()
            .len();
        let error = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::CreateArchiveSnapshot {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: CreateBusinessArchiveSnapshotPayload {
                    workspace_id: workspace.id.clone(),
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "VAULT_ASSET_INTEGRITY_MISMATCH");
        assert_eq!(table_counts(&store.connection), before_counts);
        assert_eq!(
            asset_service::list_assets(&store.connection, Some(&project_id))
                .unwrap()
                .len(),
            before_assets
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM business_archive_snapshots WHERE workspace_id = ?1",
                    [&workspace.id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM asset_origins
                     WHERE origin IN ('generatedArchiveManifest','generatedArchivePackage')",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn archive_status_rejects_tampered_package_without_state_event_or_receipt() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = prepare_paid_and_accepted_workspace(&mut store, &project_id);
        let workspace = prepare_test_business_closure(&mut store, &project_id, workspace);
        let package_asset_id = workspace.archive_snapshots[0]
            .package_asset_id
            .clone()
            .unwrap();
        let package_path = asset_service::resolve_original_path(
            &store.connection,
            &store.vault_root,
            &package_asset_id,
        )
        .unwrap();
        fs::write(package_path, b"tampered archive package").unwrap();

        let before_counts = table_counts(&store.connection);
        let before_revision = workspace.revision;
        let before_status = workspace.status.clone();
        let command_id = Uuid::new_v4().to_string();
        let idempotency_key = Uuid::new_v4().to_string();
        let error = execute_command(
            &mut store.connection,
            &store.vault_root,
            BusinessWorkspaceCommandEnvelope::ChangeStatus {
                command_id: command_id.clone(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: ChangeBusinessWorkspaceStatusPayload {
                    workspace_id: workspace.id.clone(),
                    status: BusinessWorkspaceStatus::Archived,
                },
                idempotency_key: idempotency_key.clone(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            },
        )
        .unwrap_err();

        assert_eq!(
            error.code,
            business_closure_service::BUSINESS_ARCHIVE_INTEGRITY_ERROR_CODE
        );
        assert_eq!(table_counts(&store.connection), before_counts);
        let persisted = load_workspace(&store.connection, &workspace.id).unwrap();
        assert_eq!(persisted.status, before_status);
        assert_eq!(persisted.revision, before_revision);
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM business_workspace_command_receipts
                     WHERE command_id = ?1 OR idempotency_key = ?2",
                    params![command_id, idempotency_key],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn archive_package_export_verification_allows_regular_and_valid_assets_but_rejects_tampering() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let regular_asset_id = import_test_asset(
            &mut store,
            &project_id,
            "ordinary-export.txt",
            b"ordinary asset",
        );
        verify_archive_package_for_export(&store.connection, &store.vault_root, &regular_asset_id)
            .unwrap();

        let workspace = prepare_paid_and_accepted_workspace(&mut store, &project_id);
        let workspace = prepare_test_business_closure(&mut store, &project_id, workspace);
        let package_asset_id = workspace.archive_snapshots[0]
            .package_asset_id
            .clone()
            .unwrap();
        verify_archive_package_for_export(&store.connection, &store.vault_root, &package_asset_id)
            .unwrap();

        let package_path = asset_service::resolve_original_path(
            &store.connection,
            &store.vault_root,
            &package_asset_id,
        )
        .unwrap();
        fs::write(package_path, b"tampered export package").unwrap();
        let error = verify_archive_package_for_export(
            &store.connection,
            &store.vault_root,
            &package_asset_id,
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            business_closure_service::BUSINESS_ARCHIVE_INTEGRITY_ERROR_CODE
        );
    }

    #[test]
    fn archive_package_export_verification_rejects_multiple_snapshot_links() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = prepare_paid_and_accepted_workspace(&mut store, &project_id);
        let workspace = prepare_test_business_closure(&mut store, &project_id, workspace);
        let snapshot = workspace.archive_snapshots[0].clone();
        let package_asset_id = snapshot.package_asset_id.clone().unwrap();
        store
            .connection
            .execute(
                "INSERT INTO business_archive_snapshots
                 (id, workspace_id, captured_workspace_revision, captured_customer_revision,
                  manifest_sha256, manifest_asset_id, package_asset_id, record_json, generated_at)
                 SELECT ?1, workspace_id, captured_workspace_revision, captured_customer_revision,
                        manifest_sha256, manifest_asset_id, package_asset_id, record_json, generated_at + 1
                 FROM business_archive_snapshots WHERE id = ?2",
                params![Uuid::new_v4().to_string(), snapshot.id],
            )
            .unwrap();

        let error = verify_archive_package_for_export(
            &store.connection,
            &store.vault_root,
            &package_asset_id,
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            business_closure_service::BUSINESS_ARCHIVE_INTEGRITY_ERROR_CODE
        );
        assert!(error.message.contains("multiple snapshots"));
    }

    #[test]
    fn reconciliation_removes_unlinked_archive_manifest_and_package_assets() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let manifest_source = store.temporary.path().join("orphan-manifest.json");
        let package_source = store.temporary.path().join("orphan-archive.zip");
        fs::write(&manifest_source, br#"{"orphan":true}"#).unwrap();
        fs::write(&package_source, b"PK\x05\x06empty-archive").unwrap();
        let manifest = asset_service::import_generated_artifact(
            &mut store.connection,
            &store.vault_root,
            &project_id,
            &manifest_source,
            asset_service::GeneratedArtifactSource::ArchiveManifest,
            "orphan-snapshot:manifest",
        )
        .unwrap();
        let package = asset_service::import_generated_artifact(
            &mut store.connection,
            &store.vault_root,
            &project_id,
            &package_source,
            asset_service::GeneratedArtifactSource::ArchivePackage,
            "orphan-snapshot:package",
        )
        .unwrap();
        let manifest_path = asset_service::resolve_original_path(
            &store.connection,
            &store.vault_root,
            &manifest.id,
        )
        .unwrap();
        let package_path =
            asset_service::resolve_original_path(&store.connection, &store.vault_root, &package.id)
                .unwrap();
        assert!(manifest_path.exists());
        assert!(package_path.exists());
        assert_eq!(
            reconcile_generated_assets(&mut store.connection, &store.vault_root).unwrap(),
            2
        );
        assert!(!manifest_path.exists());
        assert!(!package_path.exists());
        assert_eq!(
            asset_service::get_asset(&store.connection, &manifest.id)
                .unwrap_err()
                .code,
            "ASSET_NOT_FOUND"
        );
        assert_eq!(
            asset_service::get_asset(&store.connection, &package.id)
                .unwrap_err()
                .code,
            "ASSET_NOT_FOUND"
        );
    }

    #[test]
    fn generated_asset_cleanup_commit_failure_preserves_ready_asset_and_vault_file() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let source = store.temporary.path().join("commit-guard-manifest.json");
        fs::write(&source, br#"{"guarded":true}"#).unwrap();
        let asset = asset_service::import_generated_artifact(
            &mut store.connection,
            &store.vault_root,
            &project_id,
            &source,
            asset_service::GeneratedArtifactSource::ArchiveManifest,
            "commit-guard:manifest",
        )
        .unwrap();
        let asset_path =
            asset_service::resolve_original_path(&store.connection, &store.vault_root, &asset.id)
                .unwrap();
        store
            .connection
            .execute_batch(
                "CREATE TABLE cleanup_commit_guard (
                    asset_id TEXT NOT NULL,
                    FOREIGN KEY(asset_id) REFERENCES assets(id)
                        DEFERRABLE INITIALLY DEFERRED
                 );",
            )
            .unwrap();
        store
            .connection
            .execute(
                "INSERT INTO cleanup_commit_guard(asset_id) VALUES (?1)",
                [&asset.id],
            )
            .unwrap();

        let error = cleanup_generated_asset(&mut store.connection, &store.vault_root, &asset.id)
            .unwrap_err();
        assert_eq!(error.code, "HOST_INTERNAL");
        assert!(asset_path.exists());
        assert_eq!(
            asset_service::get_asset(&store.connection, &asset.id)
                .unwrap()
                .id,
            asset.id
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM business_generated_asset_gc WHERE asset_id = ?1",
                    [&asset.id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn generated_asset_cleanup_retries_physical_delete_after_database_commit() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let source = store.temporary.path().join("retry-manifest.json");
        fs::write(&source, br#"{"retry":true}"#).unwrap();
        let asset = asset_service::import_generated_artifact(
            &mut store.connection,
            &store.vault_root,
            &project_id,
            &source,
            asset_service::GeneratedArtifactSource::ArchiveManifest,
            "retry-delete:manifest",
        )
        .unwrap();
        let asset_path =
            asset_service::resolve_original_path(&store.connection, &store.vault_root, &asset.id)
                .unwrap();
        fs::remove_file(&asset_path).unwrap();
        fs::create_dir(&asset_path).unwrap();
        fs::write(asset_path.join("blocker"), b"force remove_file failure").unwrap();

        cleanup_generated_asset(&mut store.connection, &store.vault_root, &asset.id).unwrap();
        assert_eq!(
            asset_service::get_asset(&store.connection, &asset.id)
                .unwrap_err()
                .code,
            "ASSET_NOT_FOUND"
        );
        let (attempts, last_error): (i64, String) = store
            .connection
            .query_row(
                "SELECT attempts, last_error FROM business_generated_asset_gc WHERE asset_id = ?1",
                [&asset.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(attempts, 1);
        assert!(!last_error.is_empty());
        assert!(asset_path.exists());

        fs::remove_dir_all(&asset_path).unwrap();
        assert_eq!(
            reconcile_generated_assets(&mut store.connection, &store.vault_root).unwrap(),
            0
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM business_generated_asset_gc WHERE asset_id = ?1",
                    [&asset.id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        assert!(!asset_path.exists());
    }

    #[test]
    fn archive_snapshot_preparation_rejects_an_active_sqlite_transaction() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let payload = CreateBusinessArchiveSnapshotPayload {
            workspace_id: workspace.id.clone(),
        };
        let transaction = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        let result = prepare_archive_snapshot_outside_transaction(
            &transaction,
            &store.vault_root,
            &workspace,
            &payload,
            &Uuid::new_v4().to_string(),
            "operator-local",
            now_millis(),
        );
        let error = match result {
            Ok(_) => panic!("archive preparation unexpectedly ran inside a transaction"),
            Err(error) => error,
        };
        assert_eq!(error.code, "HOST_INTERNAL");
        assert!(error.message.contains("outside a SQLite transaction"));
    }

    fn add_test_settlement_deliverable(
        store: &mut TestStore,
        project_id: &str,
        workspace: BusinessWorkspaceRecord,
        name: &str,
    ) -> (BusinessWorkspaceRecord, String) {
        let workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::UpsertMilestone {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(project_id),
                payload: UpsertBusinessMilestonePayload {
                    workspace_id: workspace.id.clone(),
                    milestone: crate::protocol::BusinessMilestoneInput {
                        id: None,
                        title: format!("{name} milestone"),
                        description: format!("{name} execution batch"),
                        due_at: None,
                        acceptance_criteria: "Customer confirmation".to_string(),
                        required: true,
                        status: crate::protocol::BusinessMilestoneStatus::Planned,
                    },
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        let milestone_id = workspace.milestones.last().unwrap().id.clone();
        let asset_id = import_test_asset(
            store,
            project_id,
            &format!("{}.txt", name.replace(' ', "-")),
            name.as_bytes(),
        );
        let workspace = store
            .execute(
                BusinessWorkspaceCommandEnvelope::RegisterDeliverableVersion {
                    command_id: Uuid::new_v4().to_string(),
                    protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                    context: context(project_id),
                    payload: RegisterBusinessDeliverableVersionPayload {
                        workspace_id: workspace.id.clone(),
                        milestone_id: milestone_id.clone(),
                        deliverable_id: None,
                        name: name.to_string(),
                        required: true,
                        asset_id,
                        notes: String::new(),
                    },
                    idempotency_key: Uuid::new_v4().to_string(),
                    expected_revision: Some(workspace.revision),
                    deadline_at: None,
                },
            )
            .response
            .business_workspace;
        let deliverable_id = workspace
            .milestones
            .iter()
            .find(|milestone| milestone.id == milestone_id)
            .unwrap()
            .deliverables[0]
            .id
            .clone();
        (workspace, deliverable_id)
    }

    fn settlement_batch_command(
        project_id: &str,
        workspace: &BusinessWorkspaceRecord,
        deliverable_id: &str,
        period: &str,
        cadence: crate::protocol::BusinessSettlementCadence,
        command_id: String,
        idempotency_key: String,
    ) -> BusinessWorkspaceCommandEnvelope {
        BusinessWorkspaceCommandEnvelope::UpsertSettlementBatch {
            command_id,
            protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
            context: context(project_id),
            payload: UpsertBusinessSettlementBatchPayload {
                workspace_id: workspace.id.clone(),
                batch: crate::protocol::BusinessSettlementBatchInput {
                    id: None,
                    contract_number: "ANNUAL-2026-001".to_string(),
                    settlement_period: period.to_string(),
                    cadence,
                    status: BusinessSettlementBatchStatus::Confirmed,
                    lines: vec![BusinessSettlementLineInput {
                        deliverable_id: deliverable_id.to_string(),
                        contract_quantity_millis: 10_000,
                        cumulative_executed_millis: 10_000,
                        current_executed_millis: 10_000,
                        cumulative_accepted_millis: 10_000,
                        current_accepted_millis: 10_000,
                        current_settlement_millis: 10_000,
                        unit: "item".to_string(),
                        notes: String::new(),
                    }],
                    notes: String::new(),
                },
            },
            idempotency_key,
            expected_revision: Some(workspace.revision),
            deadline_at: None,
        }
    }

    #[test]
    fn annual_settlement_runs_two_quarters_and_one_off_without_duplicate_deliverables() {
        let mut store = TestStore::new();
        let project_id = store.project_id.clone();
        let workspace = store
            .execute(create_command(&project_id))
            .response
            .business_workspace;
        let (workspace, q1_deliverable) =
            add_test_settlement_deliverable(&mut store, &project_id, workspace, "Q1 film");
        let (workspace, q2_deliverable) =
            add_test_settlement_deliverable(&mut store, &project_id, workspace, "Q2 film");
        let (mut workspace, one_off_deliverable) =
            add_test_settlement_deliverable(&mut store, &project_id, workspace, "One-off event");

        let stale_workspace = workspace.clone();
        let mut invalid_quantity = settlement_batch_command(
            &project_id,
            &workspace,
            &q1_deliverable,
            "2026-Q0",
            crate::protocol::BusinessSettlementCadence::Quarterly,
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
        );
        if let BusinessWorkspaceCommandEnvelope::UpsertSettlementBatch { payload, .. } =
            &mut invalid_quantity
        {
            payload.batch.lines[0].current_settlement_millis = 10_001;
        }
        let invalid_quantity_error =
            execute_command(&mut store.connection, &store.vault_root, invalid_quantity)
                .unwrap_err();
        assert_eq!(
            invalid_quantity_error.code,
            "BUSINESS_SETTLEMENT_QUANTITY_INVALID"
        );
        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            workspace
        );

        let command_id = Uuid::new_v4().to_string();
        let idempotency_key = Uuid::new_v4().to_string();
        let first_command = settlement_batch_command(
            &project_id,
            &workspace,
            &q1_deliverable,
            "2026-Q1",
            crate::protocol::BusinessSettlementCadence::Quarterly,
            command_id,
            idempotency_key.clone(),
        );
        let first = store.execute(first_command.clone()).response;
        assert!(!first.replayed);
        let replay = store.execute(first_command).response;
        assert!(replay.replayed);
        assert_eq!(replay.business_workspace.settlement_batches.len(), 1);
        workspace = replay.business_workspace;

        let stale_revision_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            settlement_batch_command(
                &project_id,
                &stale_workspace,
                &q2_deliverable,
                "2026-Q2-stale",
                crate::protocol::BusinessSettlementCadence::Quarterly,
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
            ),
        )
        .unwrap_err();
        assert_eq!(stale_revision_error.code, "REVISION_CONFLICT");
        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            workspace
        );

        let idempotency_collision = settlement_batch_command(
            &project_id,
            &workspace,
            &q2_deliverable,
            "2026-Q2-collision",
            crate::protocol::BusinessSettlementCadence::Quarterly,
            Uuid::new_v4().to_string(),
            idempotency_key,
        );
        let idempotency_collision_error = execute_command(
            &mut store.connection,
            &store.vault_root,
            idempotency_collision,
        )
        .unwrap_err();
        assert_eq!(idempotency_collision_error.code, "IDEMPOTENCY_KEY_REUSED");
        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            workspace
        );

        workspace = store
            .execute(settlement_batch_command(
                &project_id,
                &workspace,
                &q2_deliverable,
                "2026-Q2",
                crate::protocol::BusinessSettlementCadence::Quarterly,
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
            ))
            .response
            .business_workspace;
        workspace = store
            .execute(settlement_batch_command(
                &project_id,
                &workspace,
                &one_off_deliverable,
                "2026-07-29",
                crate::protocol::BusinessSettlementCadence::OneOff,
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
            ))
            .response
            .business_workspace;
        assert_eq!(workspace.settlement_batches.len(), 3);
        assert!(workspace.settlement_batches.iter().all(|batch| {
            batch.lines[0].cumulative_settled_millis == 10_000
                && batch.lines[0].remaining_quantity_millis == 0
        }));

        let duplicate = execute_command(
            &mut store.connection,
            &store.vault_root,
            settlement_batch_command(
                &project_id,
                &workspace,
                &q1_deliverable,
                "2026-Q3",
                crate::protocol::BusinessSettlementCadence::Quarterly,
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
            ),
        )
        .unwrap_err();
        assert_eq!(
            duplicate.code,
            "BUSINESS_SETTLEMENT_DELIVERABLE_ALREADY_RESERVED"
        );
        assert_eq!(
            load_workspace(&store.connection, &workspace.id).unwrap(),
            workspace
        );

        let first_batch_id = workspace.settlement_batches[0].id.clone();
        workspace = store
            .execute(BusinessWorkspaceCommandEnvelope::VoidSettlementBatch {
                command_id: Uuid::new_v4().to_string(),
                protocol_version: BUSINESS_WORKSPACE_PROTOCOL_VERSION.to_string(),
                context: context(&project_id),
                payload: VoidBusinessSettlementBatchPayload {
                    workspace_id: workspace.id.clone(),
                    batch_id: first_batch_id,
                    reason: "Customer requested corrected quarter".to_string(),
                },
                idempotency_key: Uuid::new_v4().to_string(),
                expected_revision: Some(workspace.revision),
                deadline_at: None,
            })
            .response
            .business_workspace;
        assert_eq!(
            workspace.settlement_batches[0].status,
            BusinessSettlementBatchStatus::Voided
        );
        workspace = store
            .execute(settlement_batch_command(
                &project_id,
                &workspace,
                &q1_deliverable,
                "2026-Q1-corrected",
                crate::protocol::BusinessSettlementCadence::Quarterly,
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
            ))
            .response
            .business_workspace;
        assert_eq!(workspace.settlement_batches.len(), 4);

        let workspace_id = workspace.id.clone();
        store.reopen();
        let persisted = load_workspace(&store.connection, &workspace_id).unwrap();
        assert_eq!(persisted.settlement_batches, workspace.settlement_batches);
    }
}
